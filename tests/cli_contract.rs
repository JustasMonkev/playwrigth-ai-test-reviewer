use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-review-cli-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn trace(&self, name: &str, failed: bool) -> PathBuf {
        let path = self.0.join(name);
        let mut after = json!({"type":"after","callId":"call@1","endTime":2});
        if failed {
            after["error"] = json!({"message":"button never appeared"});
        }
        fs::write(
            &path,
            format!(
                "{}\n{after}\n",
                json!({"type":"before","callId":"call@1","startTime":1,
                    "apiName":"locator.click","params":{"selector":"#save"}})
            ),
        )
        .unwrap();
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pw-review"))
}

fn stdout_json(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn assert_error(output: &Output, kind: &str) -> Value {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema_version"], 1);
    assert_eq!(error["error"]["kind"], kind);
    assert!(!error["error"]["message"].as_str().unwrap().is_empty());
    error
}

#[test]
fn analyze_emits_compact_machine_readable_report() {
    let workspace = Workspace::new();
    let trace = workspace.trace("pass.trace", false);
    let output = cli().arg("analyze").arg(trace).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    let value = stdout_json(&output);
    assert_eq!(value["command"], "analyze");
    assert_eq!(value["report"]["summary"]["failed"], 0);
}

#[test]
fn failure_exit_status_is_opt_in_and_keeps_report() {
    let workspace = Workspace::new();
    let trace = workspace.trace("failure.trace", true);
    let output = cli().arg("analyze").arg(&trace).output().unwrap();
    assert!(output.status.success());
    assert_eq!(stdout_json(&output)["report"]["summary"]["failed"], 1);
    let output = cli()
        .args(["--fail-on-error", "analyze"])
        .arg(trace)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout_json(&output)["report"]["summary"]["failed"], 1);
}

#[test]
fn compare_exit_status_uses_after_trace_not_resolved_failure() {
    let workspace = Workspace::new();
    let failed = workspace.trace("failed.trace", true);
    let passed = workspace.trace("passed.trace", false);
    let output = cli()
        .args(["compare", "--fail-on-error"])
        .arg(&failed)
        .arg(&passed)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        stdout_json(&output)["comparison"]["resolved"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let output = cli()
        .args(["compare", "--fail-on-error"])
        .arg(passed)
        .arg(failed)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stdout_json(&output)["comparison"]["introduced"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn bad_arguments_and_missing_inputs_are_json_errors() {
    for args in [
        vec!["--unknown"],
        vec!["analyze"],
        vec![
            "github",
            "0",
            "--repo",
            "owner/repo",
            "--artifact",
            "traces",
        ],
        vec!["analyze", "missing.trace", "--max-events", "-1"],
    ] {
        assert_error(&cli().args(args).output().unwrap(), "usage");
    }
    assert_error(
        &cli()
            .args(["analyze", "/definitely-missing-pw-review.trace"])
            .output()
            .unwrap(),
        "runtime",
    );
}

#[test]
fn output_budget_never_emits_partial_json() {
    let workspace = Workspace::new();
    let trace = workspace.trace("failed.trace", true);
    for limit in ["0", "16", "128"] {
        let output = cli()
            .arg("analyze")
            .arg(&trace)
            .args(["--max-output-bytes", limit])
            .output()
            .unwrap();
        let error = assert_error(&output, "runtime");
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains("max-output-bytes")
        );
    }
}

#[test]
fn prompt_and_pretty_flags_work_after_subcommand() {
    let workspace = Workspace::new();
    let trace = workspace.trace("pass.trace", false);
    let output = cli()
        .arg("analyze")
        .arg(trace)
        .args(["--prompt", "--pretty"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let (prompt, value) = text.split_once('\n').unwrap();
    assert!(prompt.contains("untrusted data"));
    let value: Value = serde_json::from_str(value).unwrap();
    assert_eq!(value["command"], "analyze");
}

#[test]
fn input_and_event_budgets_reject_incomplete_evidence() {
    let workspace = Workspace::new();
    let trace = workspace.trace("pass.trace", false);
    for flags in [["--max-bytes", "1"], ["--max-events", "1"]] {
        let output = cli()
            .arg("analyze")
            .arg(&trace)
            .args(flags)
            .output()
            .unwrap();
        assert_error(&output, "runtime");
    }
}

#[cfg(unix)]
fn mock_github(workspace: &Workspace, body: &str) -> Command {
    use std::os::unix::fs::PermissionsExt;
    let bin = workspace.0.join("bin");
    let scratch = workspace.0.join("scratch");
    fs::create_dir(&bin).unwrap();
    fs::create_dir(&scratch).unwrap();
    let gh = bin.join("gh");
    fs::write(&gh, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o700)).unwrap();
    let mut command = cli();
    command
        .args([
            "github",
            "123",
            "--repo",
            "owner/repo",
            "--artifact",
            "traces",
        ])
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("TMPDIR", &scratch);
    command
}

#[cfg(unix)]
fn assert_scratch_empty(workspace: &Workspace) {
    assert_eq!(
        fs::read_dir(workspace.0.join("scratch")).unwrap().count(),
        0
    );
}

#[test]
#[cfg(unix)]
fn github_subprocess_failure_is_reported_and_cleans_up() {
    let workspace = Workspace::new();
    let output = mock_github(
        &workspace,
        "printf 'authentication unavailable' >&2\nexit 17",
    )
    .output()
    .unwrap();
    let error = assert_error(&output, "runtime");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("authentication unavailable")
    );
    assert_scratch_empty(&workspace);
}

#[test]
#[cfg(unix)]
fn github_timeout_terminates_descendants_and_cleans_up() {
    let workspace = Workspace::new();
    let started = std::time::Instant::now();
    let output = mock_github(&workspace, "sleep 30 &\nwait")
        .args(["--timeout-secs", "1"])
        .output()
        .unwrap();
    let error = assert_error(&output, "runtime");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timeout-secs")
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert_scratch_empty(&workspace);
}

#[test]
#[cfg(unix)]
fn github_download_limit_applies_to_actual_bytes_and_cleans_up() {
    let workspace = Workspace::new();
    let output = mock_github(&workspace,
        "case \"$2\" in\n  */artifacts/321/zip) head -c 8192 /dev/zero ;;\n  *) printf '%s' '{\"total_count\":1,\"artifacts\":[{\"id\":321,\"name\":\"traces\",\"size_in_bytes\":100,\"expired\":false}]}' ;;\nesac")
        .args(["--max-bytes", "4096"])
        .output()
        .unwrap();
    assert_error(&output, "runtime");
    assert_scratch_empty(&workspace);
}

#[test]
#[cfg(unix)]
fn github_artifact_success_analyzes_nested_zip_and_cleans_up() {
    let workspace = Workspace::new();
    let trace = workspace.trace("failed.trace", true);
    let inner = workspace.0.join("trace.zip");
    let mut archive = ZipWriter::new(File::create(&inner).unwrap());
    archive
        .start_file("test.trace", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(&fs::read(trace).unwrap()).unwrap();
    archive.finish().unwrap();
    let artifact = workspace.0.join("artifact.zip");
    let mut archive = ZipWriter::new(File::create(&artifact).unwrap());
    archive
        .start_file(
            "test-results/failure/trace.zip",
            SimpleFileOptions::default(),
        )
        .unwrap();
    archive.write_all(&fs::read(&inner).unwrap()).unwrap();
    archive.finish().unwrap();
    let mut command = mock_github(
        &workspace,
        "case \"$2\" in\n  */artifacts/321/zip) cat \"$MOCK_ARTIFACT\" ;;\n  *) printf '%s' '{\"total_count\":1,\"artifacts\":[{\"id\":321,\"name\":\"traces\",\"size_in_bytes\":100,\"expired\":false}]}' ;;\nesac",
    );
    let output = command
        .env("MOCK_ARTIFACT", artifact)
        .arg("--fail-on-error")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value = stdout_json(&output);
    assert_eq!(value["command"], "github");
    assert_eq!(value["reports"].as_array().unwrap().len(), 1);
    assert_eq!(value["reports"][0]["report"]["summary"]["failed"], 1);
    assert!(
        !value["reports"][0]["report"]["source"]
            .as_str()
            .unwrap()
            .contains("pw-review-")
    );
    assert_scratch_empty(&workspace);
}

#[test]
#[cfg(unix)]
fn github_rejects_path_traversal_and_cleans_up() {
    let workspace = Workspace::new();
    let artifact = workspace.0.join("artifact.zip");
    let mut archive = ZipWriter::new(File::create(&artifact).unwrap());
    archive
        .start_file("../escape.trace", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"{}\n").unwrap();
    archive.finish().unwrap();
    let mut command = mock_github(
        &workspace,
        "case \"$2\" in\n  */artifacts/321/zip) cat \"$MOCK_ARTIFACT\" ;;\n  *) printf '%s' '{\"total_count\":1,\"artifacts\":[{\"id\":321,\"name\":\"traces\",\"size_in_bytes\":100,\"expired\":false}]}' ;;\nesac",
    );
    let output = command.env("MOCK_ARTIFACT", artifact).output().unwrap();
    assert_error(&output, "runtime");
    assert!(!workspace.0.join("escape.trace").exists());
    assert_scratch_empty(&workspace);
}

#[cfg(unix)]
fn github_with_two_traces(workspace: &Workspace, events: usize) -> (Command, usize) {
    let event = "{\"type\":\"action\",\"method\":\"click\",\"error\":\"failed\"}\n";
    let contents = event.repeat(events);
    let mut inner = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    inner
        .start_file(
            "test.trace",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    inner.write_all(contents.as_bytes()).unwrap();
    let bytes = inner.finish().unwrap().into_inner();
    let artifact = workspace.0.join("artifact.zip");
    let mut archive = ZipWriter::new(File::create(&artifact).unwrap());
    for name in ["first/trace.zip", "second/trace.zip"] {
        archive
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    archive.finish().unwrap();
    let mut command = mock_github(
        workspace,
        "case \"$2\" in\n  */artifacts/321/zip) cat \"$MOCK_ARTIFACT\" ;;\n  *) printf '%s' '{\"total_count\":1,\"artifacts\":[{\"id\":321,\"name\":\"traces\",\"size_in_bytes\":100,\"expired\":false}]}' ;;\nesac",
    );
    command.env("MOCK_ARTIFACT", artifact);
    (command, contents.len())
}

#[test]
#[cfg(unix)]
fn github_event_budget_is_shared_by_all_nested_traces() {
    let workspace = Workspace::new();
    let (mut command, _) = github_with_two_traces(&workspace, 600);
    let output = command.args(["--max-events", "1000"]).output().unwrap();
    let error = assert_error(&output, "runtime");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("max_events")
    );
    assert_scratch_empty(&workspace);
}

#[test]
#[cfg(unix)]
fn github_decompressed_byte_budget_is_shared_by_all_nested_traces() {
    let workspace = Workspace::new();
    let (mut command, bytes) = github_with_two_traces(&workspace, 600);
    let output = command
        .args(["--max-bytes", &(bytes + 1000).to_string()])
        .output()
        .unwrap();
    let error = assert_error(&output, "runtime");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("max_bytes")
    );
    assert_scratch_empty(&workspace);
}

#[test]
#[cfg(unix)]
fn github_retained_failure_budget_preserves_totals_and_truncation() {
    for max_failures in [0, 1] {
        let workspace = Workspace::new();
        let (mut command, _) = github_with_two_traces(&workspace, 3);
        let output = command
            .args(["--max-failures", &max_failures.to_string()])
            .output()
            .unwrap();
        assert!(output.status.success());
        let value = stdout_json(&output);
        let reports = value["reports"].as_array().unwrap();
        assert_eq!(
            reports
                .iter()
                .map(|item| item["report"]["summary"]["failed"].as_u64().unwrap())
                .sum::<u64>(),
            6
        );
        assert_eq!(
            reports
                .iter()
                .map(|item| item["report"]["failures"].as_array().unwrap().len())
                .sum::<usize>(),
            max_failures
        );
        assert!(
            reports
                .iter()
                .all(|item| item["report"]["truncated"] == true)
        );
        assert_scratch_empty(&workspace);
    }
}
