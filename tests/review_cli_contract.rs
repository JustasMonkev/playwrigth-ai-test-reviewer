use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-review-workflow-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
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
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn json(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn error(output: &Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "runtime");
}

#[test]
fn actual_runner_results_keep_outcomes_and_opt_in_failure_exit() {
    let path = fixture("results-playwright-1.56.json");
    let output = cli().arg("results").arg(&path).output().unwrap();
    assert!(output.status.success());
    let value = json(&output);
    assert_eq!(value["command"], "results");
    assert_eq!(value["report"]["summary"]["unexpected"], 3);
    assert_eq!(value["report"]["summary"]["flaky"], 1);
    assert_eq!(value["report"]["summary"]["skipped"], 1);
    let failed = cli()
        .arg("results")
        .arg(&path)
        .args(["--fail-on-error", "--max-failures", "0"])
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(json(&failed)["report"]["summary"]["unexpected"], 3);
    let bounded = cli()
        .arg("results")
        .arg(path)
        .args(["--max-output-bytes", "10"])
        .output()
        .unwrap();
    error(&bounded);
    let passed = cli()
        .arg("results")
        .arg(fixture("results-playwright-1.56-pass.json"))
        .arg("--fail-on-error")
        .output()
        .unwrap();
    assert!(passed.status.success());
    assert_eq!(json(&passed)["report"]["summary"]["unexpected"], 0);
    assert_eq!(json(&passed)["report"]["summary"]["flaky"], 1);
}

#[test]
fn history_is_explicit_no_overwrite_and_historical_failures_are_not_current_exit_status() {
    let work = Workspace::new();
    let store = work.0.join("history");
    let record = || {
        cli()
            .args(["history", "record"])
            .arg(fixture("results-playwright-1.56.json"))
            .arg("--store")
            .arg(&store)
            .args(["--run-id", "run-1", "--commit", "abc123", "--fail-on-error"])
            .output()
            .unwrap()
    };
    let output = record();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json(&output)["command"], "history record");
    error(&record());
    let show = cli()
        .args(["history", "show", "--store"])
        .arg(&store)
        .arg("--fail-on-error")
        .output()
        .unwrap();
    assert!(show.status.success());
    assert_eq!(json(&show)["command"], "history show");
    let before = fs::read_dir(&store).unwrap().count();
    let invalid = cli()
        .args(["history", "record"])
        .arg(fixture("results-playwright-1.56.json"))
        .arg("--store")
        .arg(&store)
        .args(["--run-id", "../escape", "--commit", "abc123"])
        .output()
        .unwrap();
    error(&invalid);
    assert_eq!(fs::read_dir(&store).unwrap().count(), before);
    assert!(!work.0.join("escape").exists());
}

#[cfg(unix)]
#[test]
fn visual_catalog_selects_one_real_image_and_extracts_without_overwriting() {
    let work = Workspace::new();
    let trace = fixture("browser-visual.zip");
    let listed = cli()
        .args(["evidence", "list"])
        .arg(&trace)
        .output()
        .unwrap();
    assert!(listed.status.success());
    let catalog = json(&listed);
    let items = catalog["catalog"]["items"].as_array().unwrap();
    let image = items
        .iter()
        .find(|item| item["kind"] == "screencast-frame")
        .unwrap();
    let image_id = image["id"].as_str().unwrap();
    let target = work.0.join("selected.jpeg");
    let extract = || {
        cli()
            .args(["evidence", "extract"])
            .arg(&trace)
            .arg("--id")
            .arg(image_id)
            .arg("--output")
            .arg(&target)
            .output()
            .unwrap()
    };
    let extracted = extract();
    assert!(
        extracted.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted.stderr)
    );
    let bytes = fs::read(&target).unwrap();
    assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
    assert_eq!(json(&extracted)["evidence"]["bytes"], bytes.len());
    error(&extract());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    let capped = cli()
        .args(["evidence", "list"])
        .arg(&trace)
        .args(["--max-evidence", "0"])
        .output()
        .unwrap();
    let capped = json(&capped);
    assert_eq!(capped["catalog"]["items"].as_array().unwrap().len(), 0);
    assert_eq!(capped["catalog"]["truncated"], true);
    let invalid_target = work.0.join("missing.jpeg");
    let invalid = cli()
        .args(["evidence", "extract"])
        .arg(&trace)
        .args(["--id", "missing"])
        .arg("--output")
        .arg(&invalid_target)
        .output()
        .unwrap();
    error(&invalid);
    assert!(!invalid_target.exists());
}

#[test]
fn console_and_endpoint_limits_reach_analysis_without_changing_action_exit_policy() {
    let console = cli()
        .arg("analyze")
        .arg(fixture("browser-console.trace"))
        .args(["--max-console", "0", "--fail-on-error"])
        .output()
        .unwrap();
    assert!(console.status.success());
    let report = json(&console);
    assert_eq!(report["report"]["summary"]["failed"], 0);
    assert_eq!(report["report"]["console"]["errors"], 1);
    assert_eq!(report["report"]["console"]["uncaught_exceptions"], 1);
    assert_eq!(
        report["report"]["console"]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(report["report"]["console"]["truncated"], true);
    let network = cli()
        .arg("analyze")
        .arg(fixture("browser-network.zip"))
        .args(["--max-endpoints", "0", "--fail-on-error"])
        .output()
        .unwrap();
    assert!(network.status.success());
    let network = json(&network);
    assert_eq!(network["report"]["network"]["requests"], 6);
    assert_eq!(
        network["report"]["network"]["endpoints"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(network["report"]["network"]["endpoints_truncated"], true);
}

#[test]
fn actual_browser_capture_links_slow_requests_to_candidate_actions() {
    let output = cli()
        .arg("analyze")
        .arg(fixture("browser-review.zip"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = json(&output);
    let report = &value["report"];
    assert_eq!(report["console"]["uncaught_exceptions"], 1);
    assert!(
        report["console"]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["message"] == "review-console-error")
    );
    assert_eq!(report["network"]["requests"], 3);
    assert_eq!(report["network"]["failed"], 1);
    assert_eq!(report["network"]["slow"], 1);
    let slow = report["network"]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["url"].as_str().unwrap().ends_with("/slow"))
        .unwrap();
    assert_eq!(slow["duration_ms"], 1151.709);
    assert!(!slow["temporal_candidates"].as_array().unwrap().is_empty());
    assert!(
        slow["temporal_candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["call_id"] == "call@10")
    );
}

#[cfg(unix)]
#[test]
fn visual_inputs_reject_special_files_without_waiting() {
    use std::time::{Duration, Instant};
    let work = Workspace::new();
    let fifo = work.0.join("input.trace");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
    for operation in ["list", "extract"] {
        let mut command = cli();
        command.args(["evidence", operation]).arg(&fifo).args([
            "--max-bytes",
            "1",
            "--max-events",
            "1",
        ]);
        if operation == "extract" {
            command
                .args(["--id", "e0-r0", "--output"])
                .arg(work.0.join("output.json"));
        }
        let mut child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("evidence {operation} blocked on a non-regular input");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        error(&child.wait_with_output().unwrap());
    }
    assert!(!work.0.join("output.json").exists());
}
