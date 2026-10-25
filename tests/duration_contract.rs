use playwright_ai_test_reviewer::{Limits, analyze};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::{ZipWriter, write::SimpleFileOptions};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn duration(streams: &[Vec<Value>]) -> anyhow::Result<f64> {
    let path = std::env::temp_dir().join(format!(
        "pw-review-duration-{}-{}.zip",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut archive = ZipWriter::new(File::create(&path).unwrap());
    for (index, events) in streams.iter().enumerate() {
        archive
            .start_file(format!("{index}.trace"), SimpleFileOptions::default())
            .unwrap();
        for event in events {
            writeln!(archive, "{event}").unwrap();
        }
    }
    archive.finish().unwrap();
    let result = analyze(&path, &Limits::default()).map(|report| report.summary.duration_ms);
    fs::remove_file(path).unwrap();
    result
}

fn stream(monotonic: f64, wall: f64, legacy: bool) -> Vec<Value> {
    let action = json!({"method":"click", "startTime":monotonic + 2.0,
        "endTime":monotonic + 5.0});
    let events = if legacy {
        vec![json!({"type":"action","metadata":action})]
    } else {
        vec![
            json!({"type":"before","callId":"a","method":"click",
                "startTime":action["startTime"]}),
            json!({"type":"after","callId":"a","endTime":action["endTime"]}),
        ]
    };
    let mut stream = vec![json!({"type":"context-options",
        "wallTime":wall,"monotonicTime":monotonic})];
    stream.extend(events);
    stream
}

#[test]
fn modern_and_legacy_streams_share_normalized_duration() {
    for legacy in [false, true] {
        let streams = [
            stream(100.0, 10_000.0, false),
            stream(50_000.0, 10_010.0, legacy),
        ];
        assert_eq!(duration(&streams).unwrap(), 13.0);
        assert_eq!(
            duration(&[streams[1].clone(), streams[0].clone()]).unwrap(),
            13.0
        );
    }
}

#[test]
fn missing_clock_anchor_is_not_mixed_with_epoch_timestamps() {
    let anchored = stream(100.0, 1_700_000_000_000.0, false);
    let mut unanchored = stream(50_000.0, 0.0, true);
    unanchored.remove(0);
    assert_eq!(
        duration(&[anchored.clone(), unanchored.clone()]).unwrap(),
        3.0
    );
    unanchored[0]["metadata"]["endTime"] = json!(50_102.0);
    assert_eq!(duration(&[anchored, unanchored]).unwrap(), 100.0);
}

#[test]
fn overflowing_normalized_timestamp_is_an_error() {
    let events = vec![
        json!({"type":"context-options","wallTime":1e308,"monotonicTime":0}),
        json!({"type":"action","method":"click","startTime":1e308,"endTime":1e308}),
    ];
    assert!(duration(&[events]).is_err());
}
