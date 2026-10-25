use super::*;

fn parse(input: &[u8], limits: &Limits) -> Result<Report> {
    let mut usage = Usage::default();
    let mut analysis = Analysis {
        network: NetworkAccumulator::new(limits, 0),
        limits,
        usage: &mut usage,
        report: Report {
            schema_version: 1,
            source: "test.trace".into(),
            summary: Summary::default(),
            failures: Vec::new(),
            truncated: false,
            network: crate::NetworkReport::default(),
        },
        recognized: 0,
        completed_steps: HashMap::new(),
        last_error: None,
        wall_time: TimeSpan::default(),
        unanchored_time: TimeSpan::default(),
    };
    analysis.read_trace(input, Path::new(""))?;
    Ok(analysis.report)
}

#[test]
fn joins_interleaved_actions_logs_and_soft_failures() {
    let input = br##"{"type":"before","callId":"one","apiName":"expect.soft.toBeVisible","startTime":10,"params":{"selector":"#a"},"stack":[{"file":"test.ts","line":7,"column":4}]}
    {"type":"before","callId":"two","apiName":"page.click","startTime":11}
    {"type":"log","callId":"one","message":"waiting for #a"}
    {"type":"after","callId":"two","endTime":12}
    {"type":"after","callId":"one","endTime":20,"error":{"message":"not visible"}}"##;
    let report = parse(input, &Limits::default()).unwrap();
    assert_eq!(
        (
            report.summary.actions,
            report.summary.passed,
            report.summary.failed
        ),
        (2, 1, 1)
    );
    assert_eq!(report.summary.soft_failures, 1);
    assert_eq!(report.summary.duration_ms, 10.0);
    assert_eq!(report.failures[0].logs, ["waiting for #a"]);
    assert_eq!(report.failures[0].duration_ms, Some(10.0));
    assert_eq!(report.failures[0].location.as_ref().unwrap().line, Some(7));
}

#[test]
fn supports_arrays_concatenation_and_incomplete_actions() {
    for input in [
        r#"[{"type":"before","callId":"a","method":"click"},{"type":"after","callId":"a"},{"type":"before","callId":"b","method":"click"}]"#,
        r#"{"type":"before","callId":"a","method":"click"}{"type":"after","callId":"a"}{"type":"before","callId":"b","method":"click"}"#,
    ] {
        let report = parse(input.as_bytes(), &Limits::default()).unwrap();
        assert_eq!((report.summary.passed, report.summary.incomplete), (1, 1));
        assert!(!report.has_failures());
    }
}

#[test]
fn rejects_corrupt_and_over_budget_events() {
    for input in [
        r#"[{"type":"before","callId":"a"},]"#,
        "{",
        "[{}",
        "[{}]{}",
        "{}garbage",
        "[]x",
    ] {
        assert!(
            parse(input.as_bytes(), &Limits::default()).is_err(),
            "{input}"
        );
    }
    assert!(
        parse(
            b"{}{}",
            &Limits {
                max_events: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        parse(
            b"{}{}",
            &Limits {
                max_bytes: 3,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn caps_failures_and_utf8_text_without_losing_failure_count() {
    let report = parse(br#"{"type":"after","callId":"a","error":"failure"}{"type":"after","callId":"b","error":"failure"}"#, &Limits { max_failures: 1, max_text: 3, ..Limits::default() }).unwrap();
    assert_eq!(report.failure_count(), 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].error, "fai");
    assert!(report.truncated);
    let report = parse(
        "{\"type\":\"after\",\"callId\":\"a\",\"error\":\"💥💥\"}".as_bytes(),
        &Limits {
            max_text: 5,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(report.failures[0].error, "💥");
}

#[test]
fn reads_legacy_actions_without_retaining_snapshots() {
    let input = br#"{"type":"action","metadata":{"id":"one","type":"Page","method":"click","startTime":2,"endTime":5,"params":{"selector":"button"},"error":{"message":"timeout"},"log":[{"message":"waiting"}]}}{"type":"frame-snapshot","snapshot":{"html":["html",{},"large DOM"]}}"#;
    let report = parse(input, &Limits::default()).unwrap();
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.failures[0].action, "Page.click");
    assert_eq!(report.failures[0].logs, ["waiting"]);
    assert_eq!(report.summary.duration_ms, 3.0);
}

#[test]
fn does_not_invent_success_for_unrecognized_or_orphan_actions() {
    for input in [
        r#"{"type":"action"}"#,
        r#"{"type":"after","callId":"a"}"#,
        r#"{"type":"before","callId":"a"}"#,
    ] {
        assert!(parse(input.as_bytes(), &Limits::default()).is_err());
    }
}

#[test]
fn merged_soft_metadata_updates_totals_even_when_evidence_is_not_retained() {
    let input = br#"{"type":"before","callId":"browser","stepId":"step","class":"Frame","method":"expect","startTime":11}{"type":"after","callId":"browser","endTime":12,"error":"failed"}{"type":"before","callId":"test","stepId":"step","class":"Test","method":"expect","startTime":10,"params":{"isSoft":true}}{"type":"after","callId":"test","endTime":13,"error":"detailed failure"}"#;
    for max_failures in [0, 1] {
        let report = parse(
            input,
            &Limits {
                max_failures,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(report.summary.failed, 1);
        assert_eq!(report.summary.actions, 1);
        assert_eq!(report.summary.soft_failures, 1);
        if let Some(failure) = report.failures.first() {
            assert!(failure.soft);
        }
    }
}

#[test]
fn trace_entry_duration_spans_overlapping_and_sequential_streams() {
    let limits = Limits::default();
    let mut usage = Usage::default();
    let mut analysis = Analysis {
        network: NetworkAccumulator::new(&limits, 0),
        limits: &limits,
        usage: &mut usage,
        report: Report {
            schema_version: 1,
            source: "test.zip".into(),
            summary: Summary::default(),
            failures: Vec::new(),
            truncated: false,
            network: crate::NetworkReport::default(),
        },
        recognized: 0,
        completed_steps: HashMap::new(),
        last_error: None,
        wall_time: TimeSpan::default(),
        unanchored_time: TimeSpan::default(),
    };
    for (start, end) in [(10, 20), (12, 18), (22, 25)] {
        let input =
            format!(r#"{{"type":"action","method":"click","startTime":{start},"endTime":{end}}}"#);
        analysis
            .read_trace(input.as_bytes(), Path::new(""))
            .unwrap();
    }
    assert_eq!(analysis.report.summary.duration_ms, 15.0);
}
