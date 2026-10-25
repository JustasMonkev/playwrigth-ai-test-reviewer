use playwright_ai_test_reviewer::{Limits, analyze, compare};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-review-contract-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn archive(&self, name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let path = self.0.join(name);
        let mut archive = ZipWriter::new(File::create(&path).unwrap());
        for (name, contents) in entries {
            archive
                .start_file(
                    *name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .unwrap();
            archive.write_all(contents.as_bytes()).unwrap();
        }
        archive.finish().unwrap();
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn lines(events: &[Value]) -> String {
    events.iter().map(|event| format!("{event}\n")).collect()
}

fn action(call_id: &str, title: &str, selector: &str, error: Option<&str>) -> String {
    let mut after = json!({"type":"after","callId":call_id,"endTime":15.5});
    if let Some(message) = error {
        after["error"] = json!({"name":"Error","message":message});
    }
    lines(&[
        json!({"type":"before","callId":call_id,"startTime":10.0,
            "class":"Frame","method":"click","apiName":title,"title":title,
            "params":{"selector":selector},
            "stack":[{"file":"checkout.spec.ts","line":12,"column":7}]}),
        after,
    ])
}

fn analyze_json(path: &Path, limits: &Limits) -> Value {
    serde_json::to_value(analyze(path, limits).unwrap()).unwrap()
}

#[test]
fn passing_actions_have_no_failures() {
    let workspace = Workspace::new();
    let trace = action("call@1", "locator.click", "#save", None);
    let path = workspace.archive("pass.zip", &[("trace.trace", &trace)]);
    let report = analyze_json(&path, &Limits::default());
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["summary"]["actions"], 1);
    assert_eq!(report["summary"]["passed"], 1);
    assert_eq!(report["summary"]["failed"], 0);
    assert_eq!(report["failures"], json!([]));
}

#[test]
fn failure_keeps_locator_location_duration_and_log_evidence() {
    let workspace = Workspace::new();
    let trace = action("call@1", "locator.click", "#save", Some("Timeout 100ms"));
    let (before, after) = trace.split_once('\n').unwrap();
    let trace = format!(
        "{before}\n{}\n{after}",
        json!({"type":"log","callId":"call@1","message":"waiting for #save"})
    );
    let path = workspace.archive("failure.zip", &[("trace.trace", &trace)]);
    let report = analyze_json(&path, &Limits::default());
    assert_eq!(report["summary"]["failed"], 1);
    let failure = &report["failures"][0];
    assert_eq!(failure["selector"], "#save");
    assert_eq!(failure["error"], "Timeout 100ms");
    assert_eq!(failure["location"]["file"], "checkout.spec.ts");
    assert_eq!(failure["location"]["line"], 12);
    assert_eq!(failure["duration_ms"], 5.5);
    assert!(
        failure["logs"]
            .as_array()
            .unwrap()
            .contains(&json!("waiting for #save"))
    );
}

#[test]
fn separate_trace_streams_do_not_share_call_id_state() {
    let workspace = Workspace::new();
    let first = action("call@1", "first action", "#first", Some("first failed"));
    let second = action("call@1", "second action", "#second", Some("second failed"));
    let path = workspace.archive(
        "streams.zip",
        &[("first.trace", &first), ("second.trace", &second)],
    );
    let report = analyze_json(&path, &Limits::default());
    assert_eq!(report["summary"]["trace_entries"], 2);
    assert_eq!(report["summary"]["failed"], 2);
    let failures = report["failures"].as_array().unwrap();
    assert!(failures.iter().any(|item| item["selector"] == "#first"));
    assert!(failures.iter().any(|item| item["selector"] == "#second"));
}

#[test]
fn soft_assertion_is_not_lost_when_later_actions_pass() {
    let workspace = Workspace::new();
    let mut trace = action(
        "expect@1",
        "Expect \"soft toHaveText\"",
        "internal:role=heading",
        Some("Expected Saved, received Pending"),
    );
    trace.push_str(&action("call@2", "locator.click", "#continue", None));
    let path = workspace.archive("soft.zip", &[("test.trace", &trace)]);
    let report = analyze_json(&path, &Limits::default());
    assert_eq!(report["summary"]["failed"], 1);
    assert_eq!(report["summary"]["passed"], 1);
    assert_eq!(report["summary"]["soft_failures"], 1);
    assert_eq!(report["failures"][0]["soft"], true);
}

#[test]
fn comparison_matches_actions_after_call_ids_and_error_wording_change() {
    let workspace = Workspace::new();
    let before = action("call@7", "locator.click", "#save", Some("old timeout"));
    let after = action("call@901", "locator.click", "#save", Some("new timeout"));
    let before = workspace.archive("before.zip", &[("trace.trace", &before)]);
    let after = workspace.archive("after.zip", &[("trace.trace", &after)]);
    let result = compare(
        &analyze(&before, &Limits::default()).unwrap(),
        &analyze(&after, &Limits::default()).unwrap(),
    );
    assert_eq!(result["persistent"].as_array().unwrap().len(), 1);
    assert_eq!(result["introduced"], json!([]));
    assert_eq!(result["resolved"], json!([]));
}

#[test]
fn comparison_reports_new_and_resolved_failures() {
    let workspace = Workspace::new();
    let before = action("call@1", "locator.click", "#old", Some("old failed"));
    let after = action("call@1", "locator.click", "#new", Some("new failed"));
    let before = workspace.archive("before.zip", &[("trace.trace", &before)]);
    let after = workspace.archive("after.zip", &[("trace.trace", &after)]);
    let result = compare(
        &analyze(&before, &Limits::default()).unwrap(),
        &analyze(&after, &Limits::default()).unwrap(),
    );
    assert_eq!(result["introduced"][0]["selector"], "#new");
    assert_eq!(result["resolved"][0]["selector"], "#old");
    assert_eq!(result["persistent"], json!([]));
}

#[test]
fn missing_invalid_and_trace_free_archives_are_errors() {
    let workspace = Workspace::new();
    assert!(analyze(&workspace.0.join("missing.zip"), &Limits::default()).is_err());
    let invalid = workspace.0.join("invalid.zip");
    fs::write(&invalid, "not a zip archive").unwrap();
    assert!(analyze(&invalid, &Limits::default()).is_err());
    let empty = workspace.archive("no-traces.zip", &[("readme.txt", "hello")]);
    assert!(analyze(&empty, &Limits::default()).is_err());
}

#[test]
fn malformed_json_does_not_silently_become_a_passing_report() {
    let workspace = Workspace::new();
    let path = workspace.archive("broken.zip", &[("trace.trace", "{not json}\n")]);
    assert!(analyze(&path, &Limits::default()).is_err());
}

#[test]
fn incomplete_actions_are_not_counted_as_passed() {
    let workspace = Workspace::new();
    let trace = lines(&[json!({"type":"before","callId":"call@1",
        "apiName":"locator.click","startTime":1,"params":{"selector":"#save"}})]);
    let path = workspace.archive("incomplete.zip", &[("trace.trace", &trace)]);
    let report = analyze_json(&path, &Limits::default());
    assert_eq!(report["summary"]["incomplete"], 1);
    assert_eq!(report["summary"]["passed"], 0);
}

#[test]
fn failure_cap_retains_total_count_and_marks_truncation() {
    let workspace = Workspace::new();
    let mut trace = action("call@1", "first", "#first", Some("first failed"));
    trace.push_str(&action(
        "call@2",
        "second",
        "#second",
        Some("second failed"),
    ));
    let path = workspace.archive("failures.zip", &[("trace.trace", &trace)]);
    let limits = Limits {
        max_failures: 1,
        ..Limits::default()
    };
    let report = analyze_json(&path, &limits);
    assert_eq!(report["summary"]["failed"], 2);
    assert_eq!(report["failures"].as_array().unwrap().len(), 1);
    assert_eq!(report["truncated"], true);
}

#[test]
fn text_cap_is_utf8_safe_and_marks_truncation() {
    let workspace = Workspace::new();
    let trace = action("call@1", "locator.click", "#save", Some("é🦀é🦀é🦀é🦀"));
    let path = workspace.archive("unicode.zip", &[("trace.trace", &trace)]);
    let limits = Limits {
        max_text: 7,
        ..Limits::default()
    };
    let report = analyze_json(&path, &limits);
    assert!(report["failures"][0]["error"].as_str().unwrap().len() <= 7);
    assert_eq!(report["truncated"], true);
}

#[test]
fn compressed_trace_cannot_evade_uncompressed_byte_limit() {
    let workspace = Workspace::new();
    let trace = format!("{}\n", " ".repeat(64 * 1024));
    let path = workspace.archive("compressed.zip", &[("trace.trace", &trace)]);
    assert!(fs::metadata(&path).unwrap().len() < 4096);
    let limits = Limits {
        max_bytes: 4096,
        ..Limits::default()
    };
    assert!(analyze(&path, &limits).is_err());
}

#[test]
fn archive_is_analyzed_without_extracting_entry_paths() {
    let workspace = Workspace::new();
    let trace = action("call@1", "locator.click", "#save", None);
    let path = workspace.archive("paths.zip", &[("../outside.trace", &trace)]);
    let result = analyze(&path, &Limits::default());
    assert!(!workspace.0.parent().unwrap().join("outside.trace").exists());
    if let Ok(report) = result {
        assert_eq!(report.summary.passed, 1);
    }
}
