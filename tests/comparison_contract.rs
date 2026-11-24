use playwright_ai_test_reviewer::{Limits, Report, analyze, compare};
use serde_json::{Value, json};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn report(events: &[Value], max_text: usize) -> Report {
    report_with_limits(
        events,
        Limits {
            max_text,
            ..Limits::default()
        },
    )
}

fn report_with_limits(events: &[Value], limits: Limits) -> Report {
    let path = std::env::temp_dir().join(format!(
        "pw-review-comparison-{}-{}.trace",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let contents: String = events.iter().map(|event| format!("{event}\n")).collect();
    fs::write(&path, contents).unwrap();
    let result = analyze(&path, &limits);
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

#[test]
fn omitted_failure_details_do_not_prove_an_identity_was_absent() {
    let a = failure();
    let mut b = failure();
    b["params"]["selector"] = json!("#publish");
    let mut c = failure();
    c["params"]["selector"] = json!("#cancel");
    let partial = report_with_limits(
        &[a.clone(), b.clone(), c.clone()],
        Limits {
            max_failures: 2,
            ..Limits::default()
        },
    );
    let complete = report(&[a, c], Limits::default().max_text);

    for (before, after, uncertain_side, confirmed) in [
        (&partial, &complete, "after", "resolved"),
        (&complete, &partial, "before", "introduced"),
    ] {
        let diff = compare(before, after);
        assert_eq!(diff["persistent"][0]["before"]["selector"], "#save");
        assert_eq!(diff[confirmed][0]["selector"], "#publish");
        let other = if confirmed == "introduced" {
            "resolved"
        } else {
            "introduced"
        };
        assert_eq!(diff[other], json!([]));
        assert_eq!(
            diff["unclassified"][uncertain_side][0]["selector"],
            "#cancel"
        );
        assert_eq!(diff["truncated"], true);
    }

    let no_details = report_with_limits(
        &[b],
        Limits {
            max_failures: 0,
            ..Limits::default()
        },
    );
    let none_retained = compare(&no_details, &no_details);
    assert_eq!(none_retained["unclassified"]["omitted_before"], 1);
    assert_eq!(none_retained["unclassified"]["omitted_after"], 1);
}
