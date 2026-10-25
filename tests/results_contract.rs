use playwright_ai_test_reviewer::{AttemptStatus, Limits, TestOutcome, analyze_results};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Input(PathBuf);
impl Input {
    fn new(value: impl AsRef<[u8]>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-results-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, value).unwrap();
        Self(path)
    }
    fn json(value: Value) -> Self {
        Self::new(value.to_string())
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn report(expected: &str, statuses: &[&str]) -> Value {
    json!({"suites":[{"title":"checkout.spec.ts","specs":[{"title":"saves","file":"checkout.spec.ts","tests":[{
        "projectId":"chromium","projectName":"desktop","expectedStatus":expected,
        "results":statuses.iter().enumerate().map(|(retry,status)|json!({"status":status,"retry":retry,"duration":12.5})).collect::<Vec<_>>()
    }]}]}],"errors":[]})
}

#[test]
fn real_playwright_capture_preserves_outcomes_and_retries() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/results-playwright-1.56.json");
    let run = analyze_results(&path, &Limits::default()).unwrap();
    assert_eq!((run.summary.total, run.summary.attempts), (7, 11));
    assert_eq!(
        (
            run.summary.expected,
            run.summary.unexpected,
            run.summary.flaky,
            run.summary.skipped
        ),
        (2, 3, 1, 1)
    );
    assert!(run.has_failures());
    let flaky = run
        .tests
        .iter()
        .find(|test| test.outcome == TestOutcome::Flaky)
        .unwrap();
    assert_eq!(
        flaky
            .attempts
            .iter()
            .map(|attempt| attempt.status)
            .collect::<Vec<_>>(),
        vec![AttemptStatus::Failed, AttemptStatus::Passed]
    );
    assert_eq!(flaky.attempts[1].retry, 1);
}

#[test]
fn expected_failure_unexpected_pass_and_skip_are_distinct() {
    for (expected, statuses, outcome, failure) in [
        ("failed", vec!["failed"], TestOutcome::Expected, false),
        ("failed", vec!["passed"], TestOutcome::Unexpected, true),
        ("skipped", vec!["skipped"], TestOutcome::Skipped, false),
        ("passed", vec![], TestOutcome::Skipped, false),
        ("passed", vec!["timedOut"], TestOutcome::Unexpected, true),
        ("passed", vec!["interrupted"], TestOutcome::Skipped, true),
    ] {
        let input = Input::json(report(expected, &statuses));
        let run = analyze_results(&input.0, &Limits::default()).unwrap();
        assert_eq!(run.tests[0].outcome, outcome);
        assert_eq!(run.has_failures(), failure);
    }
}

#[test]
fn detail_caps_preserve_totals_and_attachment_paths_are_not_opened() {
    let mut value = report("passed", &["failed", "passed"]);
    value["errors"] = json!([{"message":"runner failed"}]);
    value["suites"][0]["specs"][0]["tests"][0]["results"][0]["errors"] =
        json!([{"message":"long failure"}]);
    value["suites"][0]["specs"][0]["tests"][0]["results"][0]["attachments"] = json!([{"name":"trace","contentType":"application/zip","path":"/does/not/exist.zip","body":"ignored"}]);
    let input = Input::json(value);
    let run = analyze_results(&input.0, &Limits::default()).unwrap();
    assert_eq!(
        run.tests[0].attempts[0].traces[0].path.as_deref(),
        Some("/does/not/exist.zip")
    );
    let limits = Limits {
        max_failures: 0,
        ..Limits::default()
    };
    let capped = analyze_results(&input.0, &limits).unwrap();
    assert_eq!(
        (
            capped.summary.total,
            capped.summary.attempts,
            capped.summary.runner_errors
        ),
        (1, 2, 1)
    );
    assert!(capped.has_failures() && capped.truncated);
    assert_eq!(capped.tests[0].attempts.len(), 2);
    assert!(capped.tests[0].attempts[0].errors.is_empty());
    assert!(capped.tests[0].attempts[0].traces.is_empty());
    assert!(capped.runner_errors.is_empty());
}

#[test]
fn identity_includes_project_browser_nested_title_and_repeat_index() {
    let mut value = report("passed", &["passed"]);
    value["config"] = json!({"projects":[{"id":"chromium","use":{"browserName":"chromium"}}]});
    let test = value["suites"][0]["specs"][0]["tests"][0].clone();
    value["suites"][0]["specs"][0]["tests"]
        .as_array_mut()
        .unwrap()
        .push(test);
    let suite = value["suites"][0].clone();
    value["suites"] = json!([{"title":"outer","suites":[suite]}]);
    let input = Input::json(value);
    let run = analyze_results(&input.0, &Limits::default()).unwrap();
    assert_eq!(run.tests[0].identity.browser.as_deref(), Some("chromium"));
    assert_eq!(
        run.tests[0].identity.title_path,
        vec!["outer", "checkout.spec.ts", "saves"]
    );
    assert_eq!(run.tests[0].identity.repeat_index, 0);
    assert_eq!(run.tests[1].identity.repeat_index, 1);
}

#[test]
fn malformed_schema_unknown_status_and_inconsistent_outcomes_fail() {
    for value in [
        json!({}),
        json!({"suites":{}}),
        report("passed", &["mystery"]),
    ] {
        assert!(analyze_results(&Input::json(value).0, &Limits::default()).is_err());
    }
    let mut value = report("passed", &["failed"]);
    value["suites"][0]["specs"][0]["tests"][0]["status"] = json!("expected");
    assert!(analyze_results(&Input::json(value).0, &Limits::default()).is_err());
    let mut negative = report("passed", &["passed"]);
    negative["suites"][0]["specs"][0]["tests"][0]["results"][0]["duration"] = json!(-1);
    assert!(analyze_results(&Input::json(negative).0, &Limits::default()).is_err());
    assert!(analyze_results(&Input::new("{not json").0, &Limits::default()).is_err());
}

#[test]
fn bytes_events_and_unicode_text_are_bounded() {
    let input = Input::json(report("passed", &["failed"]));
    for limits in [
        Limits {
            max_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_events: 0,
            ..Limits::default()
        },
    ] {
        assert!(analyze_results(&input.0, &limits).is_err());
    }
    let mut value = report("passed", &["passed"]);
    value["suites"][0]["specs"][0]["title"] = json!("ééé");
    let limits = Limits {
        max_text: 3,
        ..Limits::default()
    };
    let run = analyze_results(&Input::json(value).0, &limits).unwrap();
    assert_eq!(run.tests[0].identity.title_path.last().unwrap(), "é");
    assert!(run.tests[0].identity_truncated && run.truncated);
}

#[test]
fn runner_errors_fail_an_empty_run_even_without_details() {
    let input = Input::json(json!({"suites":[],"errors":[{"value":"startup failed"}]}));
    let run = analyze_results(
        &input.0,
        &Limits {
            max_failures: 0,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(run.summary.total, 0);
    assert_eq!(run.summary.runner_errors, 1);
    assert!(run.has_failures());
}

#[test]
fn real_playwright_passing_capture_keeps_recovery_without_failing_the_run() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/results-playwright-1.56-pass.json");
    let run = analyze_results(&path, &Limits::default()).unwrap();
    assert_eq!((run.summary.total, run.summary.attempts), (7, 8));
    assert_eq!(
        (
            run.summary.expected,
            run.summary.unexpected,
            run.summary.flaky,
            run.summary.skipped
        ),
        (5, 0, 1, 1)
    );
    assert!(!run.has_failures());
}

#[test]
fn runner_errors_obey_record_limits_independently_of_retained_details() {
    for error_count in 0..=2 {
        let input = Input::json(json!({
            "suites": [],
            "errors": (0..error_count).map(|_| json!({"message":"runner failure"})).collect::<Vec<_>>()
        }));
        for max_events in 0..=2 {
            for max_failures in [0, 1, 2] {
                let result = analyze_results(
                    &input.0,
                    &Limits {
                        max_events,
                        max_failures,
                        ..Limits::default()
                    },
                );
                if error_count > max_events {
                    assert!(result.unwrap_err().to_string().contains("max_events"));
                } else {
                    let run = result.unwrap();
                    assert_eq!(run.summary.runner_errors, error_count);
                    assert_eq!(run.runner_errors.len(), error_count.min(max_failures));
                    assert_eq!(run.has_failures(), error_count != 0);
                    assert_eq!(run.truncated, error_count > max_failures);
                }
            }
        }
    }
}

#[test]
fn runner_errors_share_the_suite_test_and_attempt_record_budget() {
    let mut value = report("passed", &["failed", "passed"]);
    value["errors"] = json!([{"message":"first"}, {"message":"second"}]);
    let input = Input::json(value);
    let mut limits = Limits {
        max_events: 6,
        max_failures: 0,
        ..Limits::default()
    };
    assert!(
        analyze_results(&input.0, &limits)
            .unwrap_err()
            .to_string()
            .contains("max_events")
    );
    limits.max_events = 7;
    let run = analyze_results(&input.0, &limits).unwrap();
    assert_eq!(
        (
            run.summary.total,
            run.summary.attempts,
            run.summary.runner_errors
        ),
        (1, 2, 2)
    );
    assert_eq!(run.tests[0].outcome, TestOutcome::Flaky);
    assert!(run.runner_errors.is_empty() && run.has_failures() && run.truncated);
}
