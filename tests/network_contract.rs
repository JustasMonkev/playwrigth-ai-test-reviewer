use playwright_ai_test_reviewer::{Limits, analyze, analyze_all, compare};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::{ZipWriter, write::SimpleFileOptions};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-network-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn zip(&self, name: &str, events: &[Value]) -> PathBuf {
        let path = self.0.join(name);
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file("trace.trace", SimpleFileOptions::default())
            .unwrap();
        writeln!(
            zip,
            "{}",
            json!({"type":"action","method":"click","startTime":1,"endTime":2})
        )
        .unwrap();
        zip.start_file(
            "trace.network",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        for event in events {
            writeln!(zip, "{event}").unwrap();
        }
        zip.finish().unwrap();
        path
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn request(url: &str, status: i64, time: f64) -> Value {
    json!({"type":"resource-snapshot","snapshot":{"request":{"method":"GET","url":url},
        "response":{"status":status},"time":time,"timings":{"wait":time}}})
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/browser-network.zip")
}

#[test]
fn captured_network_distinguishes_slow_http_and_transport_errors() {
    let report = analyze(&fixture(), &Limits::default()).unwrap();
    let net = &report.network;
    assert_eq!(
        (
            net.requests,
            net.failed,
            net.slow,
            net.unknown_duration,
            net.incomplete
        ),
        (6, 3, 1, 2, 0)
    );
    assert_eq!(report.summary.failed, 0);
    assert_eq!(net.evidence.len(), 4);
    let slow = net
        .evidence
        .iter()
        .find(|r| r.url.ends_with("/slow200"))
        .unwrap();
    assert_eq!(slow.duration_ms, Some(1202.295));
    assert_eq!(slow.wait_ms, Some(1201.761));
    assert_eq!(slow.status, Some(200));
    let aborted = net
        .evidence
        .iter()
        .find(|r| r.url.ends_with("/aborted"))
        .unwrap();
    assert_eq!(aborted.transport_error.as_deref(), Some("net::ERR_ABORTED"));
    assert_eq!(aborted.duration_ms, None);
    assert_eq!(aborted.status, None);
    assert!(!report.has_failures());
}

#[test]
fn detail_cap_ranks_late_failures_then_slowest_requests_and_preserves_totals() {
    let work = Workspace::new();
    let path = work.zip(
        "rank.zip",
        &[
            request("https://app/fast", 200, 1.0),
            request("https://app/slow", 200, 1000.0),
            request("https://app/slower", 200, 5000.0),
            request("https://app/error", 500, 1.0),
        ],
    );
    let report = analyze(
        &path,
        &Limits {
            max_network: 2,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(report.network.requests, 4);
    assert_eq!((report.network.failed, report.network.slow), (1, 2));
    assert_eq!(
        report
            .network
            .evidence
            .iter()
            .map(|r| r.url.as_str())
            .collect::<Vec<_>>(),
        ["https://app/error", "https://app/slower"]
    );
    assert!(report.truncated && report.network.truncated);
    let empty = analyze(
        &path,
        &Limits {
            max_network: 0,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!((empty.network.failed, empty.network.slow), (1, 2));
    assert!(empty.network.evidence.is_empty() && empty.network.truncated);
}

#[test]
fn unknown_timing_and_status_are_not_success_or_zero_latency() {
    let work = Workspace::new();
    let path = work.zip(
        "unknown.zip",
        &[
            request("https://app/pending", -1, -1.0),
            request("https://app/error", 503, -1.0),
        ],
    );
    let report = analyze(&path, &Limits::default()).unwrap();
    assert_eq!(
        (
            report.network.failed,
            report.network.slow,
            report.network.incomplete,
            report.network.unknown_duration
        ),
        (1, 0, 1, 2)
    );
    assert_eq!(report.network.evidence[0].duration_ms, None);
}

#[test]
fn endpoint_redaction_precedes_clipping_and_never_exports_headers_or_bodies() {
    let work = Workspace::new();
    let mut event = request(
        "https://user:SECRET@app.test/path?token=SECRET#SECRET",
        500,
        1.0,
    );
    event["snapshot"]["request"]["headers"] = json!([{"name":"Authorization","value":"SECRET"}]);
    event["snapshot"]["response"]["content"] = json!({"text":"SECRET"});
    event["snapshot"]["response"]["_failureText"] = json!("failure with SECRET");
    let path = work.zip("redacted.zip", &[event, request("data:SECRET", 500, 1.0)]);
    for max_text in [0, 8, 2048] {
        let report = analyze(
            &path,
            &Limits {
                max_text,
                ..Limits::default()
            },
        )
        .unwrap();
        let encoded = serde_json::to_string(&report).unwrap();
        assert!(!encoded.contains("SECRET"));
        assert!(!encoded.contains("Authorization"));
        assert!(
            report
                .network
                .evidence
                .iter()
                .all(|r| r.url.len() <= max_text)
        );
        if max_text == 2048 {
            assert_eq!(report.network.evidence[0].url, "https://app.test/path");
            assert_eq!(report.network.evidence[1].url, "<non-http-url>");
        }
    }
}

#[test]
fn network_records_share_batch_event_byte_and_evidence_budgets() {
    let paths = [fixture(), fixture()];
    let reports = analyze_all(
        &paths,
        &Limits {
            max_network: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        reports
            .iter()
            .map(|r| r.network.evidence.len())
            .sum::<usize>(),
        1
    );
    assert!(
        reports
            .iter()
            .all(|r| r.network.failed == 3 && r.network.truncated)
    );
    assert!(
        analyze_all(
            &paths,
            &Limits {
                max_events: 17,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        analyze_all(
            &paths,
            &Limits {
                max_bytes: 5500,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        analyze_all(
            &paths,
            &Limits {
                max_events: 18,
                max_bytes: 5776,
                ..Limits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn network_compare_preserves_missing_capture_and_cli_exit_semantics() {
    let before = analyze(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/browser-pass.zip"),
        &Limits::default(),
    )
    .unwrap();
    let after = analyze(&fixture(), &Limits::default()).unwrap();
    let comparison = compare(&before, &after);
    assert_eq!(comparison["network"]["before"]["log_entries"], 0);
    assert_eq!(comparison["network"]["after"]["failed"], 3);
    let output = Command::new(env!("CARGO_BIN_EXE_pw-review"))
        .arg("analyze")
        .arg(fixture())
        .args(["--fail-on-error", "--slow-ms", "2000", "--max-network", "1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["report"]["network"]["slow"], 0);
    assert_eq!(
        json["report"]["network"]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn raw_inline_and_empty_network_streams_have_explicit_observation_limits() {
    let work = Workspace::new();
    let event = request("https://app/slow", 200, 1000.0);
    for extension in ["network", "trace", "jsonl"] {
        let path = work.0.join(format!("raw.{extension}"));
        fs::write(&path, event.to_string()).unwrap();
        assert_eq!(analyze(&path, &Limits::default()).unwrap().network.slow, 1);
    }
    let empty = work.zip("empty.zip", &[]);
    let report = analyze(&empty, &Limits::default()).unwrap();
    assert_eq!(
        (report.network.log_entries, report.network.requests),
        (1, 0)
    );
    let raw_empty = work.0.join("empty.network");
    for contents in ["", "[]"] {
        fs::write(&raw_empty, contents).unwrap();
        let report = analyze(&raw_empty, &Limits::default()).unwrap();
        assert_eq!(
            (report.network.log_entries, report.network.requests),
            (1, 0)
        );
    }
    let zip_empty = work.0.join("network-only.zip");
    let mut archive = ZipWriter::new(File::create(&zip_empty).unwrap());
    archive
        .start_file("empty.network", SimpleFileOptions::default())
        .unwrap();
    archive.finish().unwrap();
    let report = analyze(&zip_empty, &Limits::default()).unwrap();
    assert_eq!(
        (report.network.log_entries, report.network.requests),
        (1, 0)
    );
    let invalid = work.0.join("invalid.network");
    fs::write(&invalid, "{not json}").unwrap();
    assert!(analyze(&invalid, &Limits::default()).is_err());
}

#[test]
fn thresholds_http_boundaries_and_missing_values_remain_distinct() {
    let work = Workspace::new();
    let mut missing = request("https://app/missing", 0, 0.0);
    missing["snapshot"]["response"]
        .as_object_mut()
        .unwrap()
        .remove("status");
    missing["snapshot"].as_object_mut().unwrap().remove("time");
    let path = work.zip(
        "bounds.zip",
        &[
            request("https://app/redirect", 302, 999.99),
            request("https://app/boundary", 200, 1000.0),
            request("https://app/client-error", 400, 0.0),
            request("https://app/server-error", 599, 0.0),
            request("https://app/unknown", 0, -1.0),
            missing,
        ],
    );
    let report = analyze(&path, &Limits::default()).unwrap();
    assert_eq!(
        (
            report.network.requests,
            report.network.failed,
            report.network.slow
        ),
        (6, 2, 1)
    );
    assert_eq!(
        (report.network.incomplete, report.network.unknown_duration),
        (2, 2)
    );
}

#[test]
fn json_trace_fallback_survives_neighboring_network_logs() {
    let work = Workspace::new();
    let path = work.0.join("fallback.zip");
    let mut archive = ZipWriter::new(File::create(&path).unwrap());
    archive
        .start_file("actions.jsonl", SimpleFileOptions::default())
        .unwrap();
    writeln!(
        archive,
        "{}",
        json!({"type":"action","method":"click","error":"failed"})
    )
    .unwrap();
    archive
        .start_file("trace.network", SimpleFileOptions::default())
        .unwrap();
    writeln!(archive, "{}", request("https://app/error", 500, 1.0)).unwrap();
    archive.finish().unwrap();
    let report = analyze(&path, &Limits::default()).unwrap();
    assert_eq!((report.summary.failed, report.network.failed), (1, 1));
}
