use playwright_ai_test_reviewer::{Limits, Report, analyze};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

fn report(actions: &[Value], request: Value, limits: Limits) -> Report {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "pw-temporal-{}-{}.trace",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut events = vec![json!({"type":"context-options","origin":"library"})];
    events.extend_from_slice(actions);
    events.push(request);
    std::fs::write(&path, serde_json::to_vec(&events).unwrap()).unwrap();
    let result = analyze(&path, &limits);
    std::fs::remove_file(path).unwrap();
    result.unwrap()
}
fn request() -> Value {
    json!({"type":"resource-snapshot","snapshot":{"request":{"method":"GET","url":"https://app/api"},"response":{"status":500},"pageref":"p","_monotonicTime":10,"time":2}})
}

#[test]
fn split_actions_keep_failed_call_ids_and_qualify_interval_boundary_overlap() {
    let report = report(
        &[
            json!({"type":"before","callId":"call@1","pageId":"p","method":"click","startTime":1}),
            json!({"type":"after","callId":"call@1","endTime":10,"error":{"message":"failure"}}),
        ],
        request(),
        Limits::default(),
    );
    assert_eq!(report.failures[0].call_id.as_deref(), Some("call@1"));
    let candidates = &report.network.evidence[0].temporal_candidates;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].call_id, "call@1");
    assert_eq!(
        report.failures[0].trace_entry,
        Some(candidates[0].trace_entry)
    );
}

#[test]
fn negative_or_reversed_times_do_not_form_candidates() {
    for (start, end) in [(-1, 20), (20, 1)] {
        let result = report(
            &[
                json!({"type":"action","callId":"a","pageId":"p","method":"click","startTime":start,"endTime":end}),
            ],
            request(),
            Limits::default(),
        );
        assert!(result.network.evidence[0].temporal_candidates.is_empty());
    }
    let mut negative = request();
    negative["snapshot"]["_monotonicTime"] = json!(-1);
    let result = report(
        &[
            json!({"type":"action","callId":"a","pageId":"p","method":"click","startTime":0,"endTime":20}),
        ],
        negative,
        Limits::default(),
    );
    assert!(result.network.evidence[0].temporal_candidates.is_empty());
}

#[test]
fn output_cap_is_reported_independently_of_the_window_retention_cap() {
    let actions:Vec<_>=(0..7).map(|id|json!({"type":"action","callId":format!("a{id}"),"pageId":"p","method":"click","startTime":0,"endTime":20})).collect();
    let result = report(
        &actions,
        request(),
        Limits {
            max_network: 2,
            ..Limits::default()
        },
    );
    assert_eq!(result.network.evidence[0].temporal_candidates.len(), 4);
    assert!(result.network.temporal_context_truncated && result.truncated);
}
