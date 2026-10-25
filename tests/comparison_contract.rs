use playwright_ai_test_reviewer::{Limits, Report, analyze, compare};
use serde_json::{Value, json};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn report(events: &[Value], max_text: usize) -> Report {
    let path = std::env::temp_dir().join(format!(
        "pw-review-comparison-{}-{}.trace",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let contents: String = events.iter().map(|event| format!("{event}\n")).collect();
    fs::write(&path, contents).unwrap();
    let result = analyze(
        &path,
        &Limits {
            max_text,
            ..Limits::default()
        },
    );
    fs::remove_file(path).unwrap();
    result.unwrap()
}

fn failure() -> Value {
    json!({"type":"action", "apiName":"locator.click", "startTime":1,
        "endTime":2, "params":{"selector":"#save"},
        "location":{"file":"checkout.spec.ts","line":12,"column":7},
        "error":"old timeout"})
}

#[test]
fn truncated_identity_fields_do_not_create_persistent_failures() {
    for pointer in ["/apiName", "/params/selector", "/location/file"] {
        for max_text in [0, 1, Limits::default().max_text] {
            let prefix = "x".repeat(max_text.max(1));
            let mut before = failure();
            let mut after = failure();
            *before.pointer_mut(pointer).unwrap() = json!(format!("{prefix}old"));
            *after.pointer_mut(pointer).unwrap() = json!(format!("{prefix}new"));
            let before = report(&[before], max_text);
            let after = report(&[after], max_text);
            let comparison = compare(&before, &after);
            assert_eq!(comparison["persistent"], json!([]), "{pointer}, {max_text}");
            assert_eq!(comparison["introduced"].as_array().unwrap().len(), 1);
            assert_eq!(comparison["resolved"].as_array().unwrap().len(), 1);
            assert_eq!(comparison["truncated"], true);
            let encoded = serde_json::to_value(&before.failures[0]).unwrap();
            assert!(encoded.get("identity").is_none());
            assert_eq!(encoded.as_object().unwrap().len(), 7);
        }
    }
}

#[test]
fn same_identity_matches_across_different_text_limits_and_error_messages() {
    let before = failure();
    let mut after = failure();
    after["error"] = json!("different timeout wording");
    for max_text in [0, 1, Limits::default().max_text] {
        let comparison = compare(
            &report(std::slice::from_ref(&before), max_text),
            &report(std::slice::from_ref(&after), Limits::default().max_text),
        );
        assert_eq!(comparison["persistent"].as_array().unwrap().len(), 1);
        assert_eq!(comparison["introduced"], json!([]));
        assert_eq!(comparison["resolved"], json!([]));
    }
}

#[test]
fn merged_failures_compare_using_retained_selector_and_location() {
    let mut browser = failure();
    browser["stepId"] = json!("step@1");
    browser["class"] = json!("Frame");
    browser.as_object_mut().unwrap().remove("location");
    let mut runner = failure();
    runner["stepId"] = json!("step@1");
    runner["class"] = json!("Test");
    runner.as_object_mut().unwrap().remove("params");
    for events in [vec![browser.clone(), runner.clone()], vec![runner, browser]] {
        let merged = report(&events, 1);
        assert_eq!(merged.summary.failed, 1);
        let full = report(&[failure()], Limits::default().max_text);
        let comparison = compare(&merged, &full);
        assert_eq!(comparison["persistent"].as_array().unwrap().len(), 1);
        assert_eq!(comparison["introduced"], json!([]));
        assert_eq!(comparison["resolved"], json!([]));
    }
}
