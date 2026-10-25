use playwright_ai_test_reviewer::{Limits, analyze, analyze_all, compare};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::{ZipWriter, write::SimpleFileOptions};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-diagnostics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn raw(&self, name: &str, events: &[Value]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, serde_json::to_vec(events).unwrap()).unwrap();
        path
    }
    fn zip(&self, name: &str, entries: &[(&str, Vec<Value>)]) -> PathBuf {
        let path = self.0.join(name);
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        for (name, events) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            for event in events {
                writeln!(zip, "{event}").unwrap();
            }
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
fn console(message: &str) -> Value {
    json!({"type":"console","messageType":"error","text":message,"args":[{"value":"ARGS_SECRET"}],"location":{"url":"https://u:SECRET@app.test/app.js?q=SECRET#SECRET","lineNumber":0,"columnNumber":7},"time":10,"pageId":"page@1"})
}
fn uncaught() -> Value {
    json!({"type":"event","class":"BrowserContext","method":"pageError","params":{"error":{"error":{"message":"Uncaught problem","stack":"STACK_SECRET","name":"Error"}}},"time":11,"pageId":"page@1"})
}
fn context(origin: &str) -> Value {
    json!({"type":"context-options","origin":origin,"wallTime":10000,"monotonicTime":100})
}
fn action(id: &str, page: &str, start: f64, end: f64, failed: bool) -> Value {
    let mut event = json!({"type":"action","callId":id,"apiName":"page.click","pageId":page,"startTime":start,"endTime":end});
    if failed {
        event["error"] = json!("timeout");
    }
    event
}
fn request(url: &str, duration: f64) -> Value {
    json!({"type":"resource-snapshot","snapshot":{"request":{"method":"GET","url":url},"response":{"status":500},"time":duration,"pageref":"page@1","_monotonicTime":110}})
}

#[test]
fn console_error_and_actual_nested_page_error_shapes_are_bounded_and_not_test_outcomes() {
    let work = Workspace::new();
    let path = work.raw("errors.trace", &[console("Browser problem"), uncaught()]);
    let report = analyze(&path, &Limits::default()).unwrap();
    assert_eq!(
        (report.console.errors, report.console.uncaught_exceptions),
        (1, 1)
    );
    assert_eq!(report.console.evidence[1].message, "Uncaught problem");
    assert_eq!(
        report.console.evidence[0]
            .location
            .as_ref()
            .unwrap()
            .line_number,
        Some(0)
    );
    assert_eq!(report.summary.failed, 0);
    assert!(!report.has_failures());
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("SECRET"));
    assert!(encoded.contains("https://app.test/app.js"));
    for max_text in [0, 1, 4] {
        let bounded = analyze(
            &path,
            &Limits {
                max_text,
                ..Limits::default()
            },
        )
        .unwrap();
        assert!(
            bounded
                .console
                .evidence
                .iter()
                .all(|e| e.message.len() <= max_text
                    && e.location.as_ref().is_none_or(|l| l.url.len() <= max_text))
        );
        assert!(bounded.truncated && bounded.console.truncated);
    }
}

#[test]
fn later_uncaught_exceptions_displace_console_details_and_batch_caps_preserve_totals() {
    let work = Workspace::new();
    let path = work.raw(
        "errors.trace",
        &[console("ééé"), console("second"), uncaught()],
    );
    for cap in [0, 1] {
        let reports = analyze_all(
            &[path.clone(), path.clone()],
            &Limits {
                max_console: cap,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(
            reports
                .iter()
                .map(|r| r.console.evidence.len())
                .sum::<usize>(),
            cap
        );
        assert!(reports.iter().all(|r| r.console.errors == 2
            && r.console.uncaught_exceptions == 1
            && r.console.truncated
            && r.truncated));
        if cap == 1 {
            assert_eq!(reports[0].console.evidence[0].kind, "uncaught_exception");
        }
    }
}

#[test]
fn ignored_console_locations_and_unrelated_records_stay_compatible_and_count_events() {
    let work = Workspace::new();
    let path = work.raw(
        "ignored.trace",
        &[
            json!({"type":"console","messageType":"log","location":{"url":"x","lineNumber":0}}),
            json!({"type":"event","method":"console","params":{"message":"object@1"}}),
            action("a", "page@1", 1.0, 2.0, false),
        ],
    );
    let report = analyze(&path, &Limits::default()).unwrap();
    assert!(
        serde_json::to_value(&report)
            .unwrap()
            .get("console")
            .is_none()
    );
    assert_eq!(report.summary.events, 3);
    assert!(
        analyze(
            &path,
            &Limits {
                max_events: 2,
                ..Limits::default()
            }
        )
        .is_err()
    );
    let thrown = work.raw("primitive.trace", &[json!({"type":"event","method":"pageError","params":{"error":{"value":{"s":"thrown string"}}}})]);
    assert_eq!(
        analyze(&thrown, &Limits::default())
            .unwrap()
            .console
            .evidence[0]
            .message,
        "thrown string"
    );
}

#[test]
fn temporal_candidates_require_the_same_library_stream_page_and_clock_overlap() {
    let work = Workspace::new();
    let path = work.zip(
        "temporal.zip",
        &[
            ("scope/one.network", vec![request("https://app/api", 10.0)]),
            (
                "scope/one.trace",
                vec![
                    context("library"),
                    action("failed", "page@1", 100.0, 125.0, true),
                    action("different-page", "page@2", 100.0, 125.0, true),
                    action("non-overlap", "page@1", 130.0, 150.0, true),
                ],
            ),
            (
                "scope/two.trace",
                vec![
                    context("library"),
                    action("other-stream", "page@1", 100.0, 125.0, true),
                ],
            ),
            (
                "scope/runner.network",
                vec![request("https://app/runner", 10.0)],
            ),
            (
                "scope/runner.trace",
                vec![
                    context("testRunner"),
                    action("other-clock", "page@1", 100.0, 125.0, true),
                ],
            ),
        ],
    );
    let report = analyze(&path, &Limits::default()).unwrap();
    let one = report
        .network
        .evidence
        .iter()
        .find(|r| r.url.ends_with("/api"))
        .unwrap();
    assert_eq!(one.temporal_candidates.len(), 1);
    let candidate = &one.temporal_candidates[0];
    assert_eq!(candidate.call_id, "failed");
    assert_eq!(candidate.relationship, "temporal_overlap");
    assert!(candidate.failed);
    assert!(
        report
            .failures
            .iter()
            .any(|f| f.call_id.as_ref() == Some(&candidate.call_id)
                && f.trace_entry == Some(candidate.trace_entry))
    );
    assert!(
        report
            .network
            .evidence
            .iter()
            .find(|r| r.url.ends_with("/runner"))
            .unwrap()
            .temporal_candidates
            .is_empty()
    );
}

#[test]
fn temporal_unknown_page_or_time_never_guesses_and_inline_after_actions_still_joins() {
    let work = Workspace::new();
    for field in ["pageref", "_monotonicTime"] {
        let mut missing = request("https://app/missing", -1.0);
        missing["snapshot"].as_object_mut().unwrap().remove(field);
        let path = work.raw(
            "missing.trace",
            &[
                context("library"),
                action("a", "page@1", 100.0, 120.0, false),
                missing,
            ],
        );
        assert!(
            analyze(&path, &Limits::default()).unwrap().network.evidence[0]
                .temporal_candidates
                .is_empty()
        );
    }
    let mut events = vec![context("library")];
    for n in 0..7 {
        events.push(action(&format!("a{n}"), "page@1", 100.0, 120.0, n == 6));
    }
    events.push(request("https://app/bounded", -1.0));
    let path = work.raw("bounded.trace", &events);
    let report = analyze(
        &path,
        &Limits {
            max_network: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(report.network.evidence[0].temporal_candidates.len(), 4);
    assert!(report.network.evidence[0].temporal_candidates[0].failed);
    assert!(report.network.temporal_context_truncated && report.truncated);
}

#[test]
fn endpoint_aggregates_cover_all_requests_without_inventing_unknown_durations() {
    let work = Workspace::new();
    let mut post = request("https://app/item/1?token=SECRET", 5.0);
    post["snapshot"]["request"]["method"] = json!("POST");
    let path = work.raw(
        "endpoints.network",
        &[
            request("https://u:SECRET@app/item/1?a=SECRET", 0.0),
            request("https://app/item/1#SECRET", 10.0),
            request("https://app/item/1", -1.0),
            request("https://app/item/2", 30.0),
            post,
        ],
    );
    let report = analyze(&path, &Limits::default()).unwrap();
    assert_eq!(report.network.endpoints.len(), 3);
    let endpoint = report
        .network
        .endpoints
        .iter()
        .find(|e| e.method == "GET" && e.url.ends_with("/1"))
        .unwrap();
    assert_eq!(
        (
            endpoint.requests,
            endpoint.known_duration,
            endpoint.unknown_duration,
            endpoint.failed
        ),
        (3, 2, 1, 3)
    );
    assert_eq!(
        (endpoint.min_ms, endpoint.max_ms, endpoint.mean_ms),
        (Some(0.0), Some(10.0), Some(5.0))
    );
    assert!(!serde_json::to_string(&report).unwrap().contains("SECRET"));
    let bounded = analyze_all(
        &[path.clone(), path],
        &Limits {
            max_endpoints: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        bounded
            .iter()
            .map(|r| r.network.endpoints.len())
            .sum::<usize>(),
        1
    );
    assert_eq!(bounded[0].network.endpoint_omitted_requests, 2);
    assert_eq!(bounded[1].network.endpoint_omitted_requests, 5);
    assert!(
        bounded
            .iter()
            .all(|r| r.network.endpoints_truncated && r.truncated)
    );
}

#[test]
fn endpoint_caps_never_merge_clipped_keys_or_invalid_urls_and_empty_logs_stay_explicit() {
    let work = Workspace::new();
    let path = work.raw(
        "long.network",
        &[
            request("https://app/one", 1.0),
            request("https://app/two", 2.0),
            request("data:SECRET", 3.0),
        ],
    );
    for limits in [
        Limits {
            max_endpoints: 0,
            ..Limits::default()
        },
        Limits {
            max_text: 1,
            ..Limits::default()
        },
    ] {
        let report = analyze(&path, &limits).unwrap();
        assert_eq!(report.network.requests, 3);
        assert!(report.network.endpoints.is_empty());
        assert_eq!(report.network.endpoint_omitted_requests, 3);
    }
    let empty = work.raw("empty.network", &[]);
    let report = analyze(&empty, &Limits::default()).unwrap();
    assert!(report.network.endpoints.is_empty());
    assert!(!report.network.endpoints_truncated);
}

#[test]
fn endpoint_comparison_only_matches_exact_keys_and_only_known_sample_means_have_deltas() {
    let work = Workspace::new();
    let before = work.raw(
        "before.network",
        &[
            request("https://app/one?q=1", 10.0),
            request("https://app/unknown", -1.0),
            request("https://app/old", 50.0),
        ],
    );
    let after = work.raw(
        "after.network",
        &[
            request("https://app/one?q=2", 30.0),
            request("https://app/one", 50.0),
            request("https://app/unknown", 2.0),
            request("https://app/new", 20.0),
        ],
    );
    let comparison = compare(
        &analyze(&before, &Limits::default()).unwrap(),
        &analyze(&after, &Limits::default()).unwrap(),
    );
    let endpoints = comparison["network"]["endpoint_comparisons"]
        .as_array()
        .unwrap();
    assert_eq!(endpoints.len(), 2);
    let one = endpoints
        .iter()
        .find(|e| e["url"] == "https://app/one")
        .unwrap();
    assert_eq!(one["before"]["known_duration"], 1);
    assert_eq!(one["after"]["known_duration"], 2);
    assert_eq!(one["mean_delta_ms"], 30.0);
    assert!(
        endpoints
            .iter()
            .find(|e| e["url"] == "https://app/unknown")
            .unwrap()
            .get("mean_delta_ms")
            .is_none()
    );
}
