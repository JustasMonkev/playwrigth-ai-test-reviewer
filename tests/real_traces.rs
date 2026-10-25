use playwright_ai_test_reviewer::{Limits, analyze, compare};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn captured_browser_outcomes_preserve_locator_and_soft_assertion_evidence() {
    for (name, actions, failed, soft) in [
        ("browser-pass.zip", 14, 0, 0),
        ("browser-hard.zip", 15, 1, 0),
        ("browser-soft.zip", 19, 1, 1),
    ] {
        for max_failures in [0, 20] {
            let report = analyze(
                &fixture(name),
                &Limits {
                    max_failures,
                    ..Limits::default()
                },
            )
            .unwrap();
            assert_eq!(report.summary.actions, actions, "{name}");
            assert_eq!(report.summary.failed, failed, "{name}");
            assert_eq!(report.summary.soft_failures, soft, "{name}");
            assert_eq!(report.summary.incomplete, 0, "{name}");
            assert_eq!(report.summary.passed, actions - failed, "{name}");
            assert_eq!(report.failures.len(), failed.min(max_failures), "{name}");
            if let Some(failure) = report.failures.first() {
                assert!(
                    failure.selector.as_deref().unwrap().contains("heading"),
                    "{name}"
                );
                assert!(failure.error.contains("Saved"), "{name}");
                assert!(failure.error.contains("Pending"), "{name}");
                assert!(!failure.logs.is_empty(), "{name}");
                assert!(
                    failure
                        .location
                        .as_ref()
                        .unwrap()
                        .file
                        .ends_with("trace.spec.cjs"),
                    "{name}"
                );
                assert_eq!(failure.soft, soft == 1, "{name}");
            }
            if failed > 0 && max_failures == 0 {
                assert!(report.truncated, "{name}");
            }
        }
    }
}

#[test]
fn captured_legacy_traces_preserve_actions_and_incomplete_evidence() {
    for (name, actions, incomplete) in [
        ("test-trace1.zip", 14, 0),
        ("test-trace2.zip", 14, 0),
        ("trace-1.31.zip", 3, 0),
        ("trace-1.37.zip", 22, 1),
        ("trace-library-1.46.zip", 5, 0),
        ("trace-remote-time-diff.zip", 19, 0),
    ] {
        let report = analyze(&fixture(name), &Limits::default()).unwrap();
        assert_eq!(report.summary.actions, actions, "{name}");
        assert_eq!(report.summary.passed, actions - incomplete, "{name}");
        assert_eq!(report.summary.incomplete, incomplete, "{name}");
        assert_eq!(report.summary.failed, 0, "{name}");
        assert!(report.summary.events > 0, "{name}");
    }
}

#[test]
fn captured_failure_comparison_has_one_persistent_failure() {
    let report = analyze(&fixture("browser-hard.zip"), &Limits::default()).unwrap();
    let comparison = compare(&report, &report);
    assert_eq!(comparison["persistent"].as_array().unwrap().len(), 1);
    assert!(comparison["introduced"].as_array().unwrap().is_empty());
    assert!(comparison["resolved"].as_array().unwrap().is_empty());
}

#[test]
fn real_trace_cli_emits_bounded_json_and_opt_in_failure_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_pw-review"))
        .args(["analyze", "--fail-on-error"])
        .arg(fixture("browser-hard.zip"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() <= 65_536);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["report"]["summary"]["failed"], 1);
}

#[test]
fn interleaved_recordings_keep_unmatched_step_candidates() {
    let path =
        std::env::temp_dir().join(format!("pw-review-interleaved-{}.zip", std::process::id()));
    let mut source = ZipArchive::new(File::open(fixture("browser-hard.zip")).unwrap()).unwrap();
    let mut streams = Vec::new();
    for name in ["0-trace.trace", "test.trace"] {
        let mut data = String::new();
        source
            .by_name(name)
            .unwrap()
            .read_to_string(&mut data)
            .unwrap();
        streams.push(data);
    }
    for recordings in [2, 64] {
        for runner_first in [false, true] {
            let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
            for (group, stream) in [usize::from(runner_first), usize::from(!runner_first)]
                .into_iter()
                .enumerate()
            {
                for recording in 0..recordings {
                    archive
                        .start_file(
                            format!("{group}-{recording}.trace"),
                            SimpleFileOptions::default(),
                        )
                        .unwrap();
                    for line in streams[stream].lines() {
                        let mut event: Value = serde_json::from_str(line).unwrap();
                        if event["type"] == "context-options" {
                            let wall_time = event["wallTime"].as_u64().unwrap();
                            event["wallTime"] = Value::from(wall_time + recording as u64 * 60_000);
                        }
                        writeln!(archive, "{event}").unwrap();
                    }
                }
            }
            fs::write(&path, archive.finish().unwrap().into_inner()).unwrap();
            let _temporary = TemporaryTrace(path.clone());
            for max_failures in [0, recordings] {
                let report = analyze(
                    &path,
                    &Limits {
                        max_failures,
                        ..Limits::default()
                    },
                )
                .unwrap();
                assert_eq!(
                    report.summary.actions,
                    15 * recordings,
                    "runner first: {runner_first}"
                );
                assert_eq!(
                    report.summary.failed, recordings,
                    "runner first: {runner_first}"
                );
                assert_eq!(
                    report.summary.passed,
                    14 * recordings,
                    "runner first: {runner_first}"
                );
                assert_eq!(report.summary.trace_entries, 2 * recordings);
                assert_eq!(report.failures.len(), recordings.min(max_failures));
                if max_failures == 0 {
                    assert!(report.truncated);
                }
                for failure in &report.failures {
                    assert!(failure.selector.as_deref().unwrap().contains("heading"));
                    assert!(failure.error.contains("Saved"));
                    assert!(failure.location.is_some());
                    assert!(!failure.logs.is_empty());
                }
            }
            let error = analyze(
                &path,
                &Limits {
                    max_events: 46 * recordings - 1,
                    ..Limits::default()
                },
            )
            .unwrap_err();
            assert!(format!("{error:#}").contains("max_events"));
        }
    }
}

struct TemporaryTrace(PathBuf);

impl Drop for TemporaryTrace {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}
