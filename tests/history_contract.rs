use playwright_ai_test_reviewer::{
    Limits, TestRunReport, analyze_results, read_history, record_history,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn report(&self, statuses: &[&str], project: &str, file: &str, title: &str) -> TestRunReport {
        let path = self.0.join("input.json");
        let value = json!({"suites":[{"title":"suite","specs":[{"title":title,"file":file,"tests":[{
            "projectId":project,"projectName":project,"expectedStatus":"passed",
            "results":statuses.iter().enumerate().map(|(retry,status)|json!({"status":status,"retry":retry,"duration":1})).collect::<Vec<_>>()
        }]}]}],"errors":[]});
        fs::write(&path, value.to_string()).unwrap();
        analyze_results(&path, &Limits::default()).unwrap()
    }
    fn store(&self) -> PathBuf {
        self.0.join("store")
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn persistence_is_explicit_and_duplicate_ids_never_overwrite() {
    let workspace = Workspace::new();
    let report = workspace.report(&["passed"], "chromium", "test.ts", "save");
    assert!(!workspace.store().exists());
    assert!(
        read_history(&workspace.store(), &Limits::default())
            .unwrap()
            .runs
            .is_empty()
    );
    assert!(!workspace.store().exists());
    record_history(
        &workspace.store(),
        "run-1",
        "abc",
        &report,
        &Limits::default(),
    )
    .unwrap();
    let original = fs::read(workspace.store().join("run-1.json")).unwrap();
    let failed = workspace.report(&["failed"], "chromium", "test.ts", "save");
    assert!(
        record_history(
            &workspace.store(),
            "run-1",
            "def",
            &failed,
            &Limits::default()
        )
        .is_err()
    );
    assert_eq!(
        fs::read(workspace.store().join("run-1.json")).unwrap(),
        original
    );
    let history = read_history(&workspace.store(), &Limits::default()).unwrap();
    assert_eq!(history.runs.len(), 1);
    assert_eq!(history.runs[0].commit, "abc");
    assert_eq!(fs::read_dir(workspace.store()).unwrap().count(), 1);
}

#[test]
fn retry_recovery_and_cross_commit_mixed_results_are_not_conflated() {
    let workspace = Workspace::new();
    for (run_id, commit, statuses) in [
        ("a", "commit1", vec!["failed"]),
        ("b", "commit2", vec!["passed"]),
        ("c", "commit2", vec!["failed", "passed"]),
    ] {
        let report = workspace.report(&statuses, "chromium", "test.ts", "save");
        record_history(
            &workspace.store(),
            run_id,
            commit,
            &report,
            &Limits::default(),
        )
        .unwrap();
    }
    let history = read_history(&workspace.store(), &Limits::default()).unwrap();
    let observation = &history.observations[0];
    assert_eq!(observation.runs, 3);
    assert_eq!(
        (
            observation.expected,
            observation.unexpected,
            observation.flaky
        ),
        (1, 1, 1)
    );
    assert_eq!(observation.retry_recoveries, 1);
    assert!(observation.mixed_outcomes);
    assert!(!observation.same_commit_mixed_outcomes);
    assert_eq!(observation.commits, vec!["commit1", "commit2"]);
    let report = workspace.report(&["failed"], "chromium", "test.ts", "save");
    record_history(
        &workspace.store(),
        "d",
        "commit2",
        &report,
        &Limits::default(),
    )
    .unwrap();
    assert!(
        read_history(&workspace.store(), &Limits::default())
            .unwrap()
            .observations[0]
            .same_commit_mixed_outcomes
    );
}

#[test]
fn project_file_and_title_changes_do_not_merge_history() {
    let workspace = Workspace::new();
    for (id, project, file, title) in [
        ("a", "chrome", "a.ts", "save"),
        ("b", "firefox", "a.ts", "save"),
        ("c", "chrome", "b.ts", "save"),
        ("d", "chrome", "a.ts", "other"),
    ] {
        let report = workspace.report(&["passed"], project, file, title);
        record_history(
            &workspace.store(),
            id,
            "commit",
            &report,
            &Limits::default(),
        )
        .unwrap();
    }
    assert_eq!(
        read_history(&workspace.store(), &Limits::default())
            .unwrap()
            .observations
            .len(),
        4
    );
}

#[test]
fn traversal_invalid_metadata_and_incomplete_reports_are_rejected() {
    let workspace = Workspace::new();
    let mut report = workspace.report(&["passed"], "chrome", "a.ts", "save");
    for id in ["../escape", "", "x/y", "a.json", "é"] {
        assert!(
            record_history(&workspace.store(), id, "abc", &report, &Limits::default()).is_err()
        );
    }
    assert!(record_history(&workspace.store(), "ok", "", &report, &Limits::default()).is_err());
    assert!(!workspace.store().exists());
    report.tests[0].identity_truncated = true;
    assert!(record_history(&workspace.store(), "ok", "abc", &report, &Limits::default()).is_err());
    report.tests[0].identity_truncated = false;
    report.tests.clear();
    assert!(record_history(&workspace.store(), "ok", "abc", &report, &Limits::default()).is_err());
}

#[test]
fn history_reads_bound_aggregate_bytes_and_items_and_reject_corruption() {
    let workspace = Workspace::new();
    let report = workspace.report(&["passed"], "chrome", "a.ts", "save");
    record_history(&workspace.store(), "a", "abc", &report, &Limits::default()).unwrap();
    let size = fs::metadata(workspace.store().join("a.json"))
        .unwrap()
        .len();
    let limits = Limits {
        max_bytes: size,
        ..Limits::default()
    };
    assert_eq!(
        read_history(&workspace.store(), &limits)
            .unwrap()
            .runs
            .len(),
        1
    );
    record_history(&workspace.store(), "b", "abc", &report, &Limits::default()).unwrap();
    assert!(read_history(&workspace.store(), &limits).is_err());
    assert!(
        read_history(
            &workspace.store(),
            &Limits {
                max_events: 4,
                ..Limits::default()
            }
        )
        .is_err()
    );
    fs::write(workspace.store().join("b.json"), "not json").unwrap();
    assert!(read_history(&workspace.store(), &Limits::default()).is_err());
}

#[test]
fn atomic_concurrent_duplicate_writers_publish_exactly_one_complete_record() {
    let workspace = Workspace::new();
    let report = workspace.report(&["passed"], "chrome", "a.ts", "save");
    let store = workspace.store();
    let outcomes = std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|_| {
                scope.spawn(|| record_history(&store, "same", "abc", &report, &Limits::default()))
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap().is_ok())
            .collect::<Vec<_>>()
    });
    assert_eq!(outcomes.into_iter().filter(|success| *success).count(), 1);
    assert_eq!(
        read_history(&store, &Limits::default()).unwrap().runs.len(),
        1
    );
    assert_eq!(fs::read_dir(store).unwrap().count(), 1);
}

#[test]
fn history_validates_filename_schema_outcome_and_summary() {
    let workspace = Workspace::new();
    let report = workspace.report(&["passed"], "chrome", "a.ts", "save");
    let run = record_history(&workspace.store(), "a", "abc", &report, &Limits::default()).unwrap();
    let original = serde_json::to_value(run).unwrap();
    for (key, value) in [
        ("schema_version", json!(99)),
        ("run_id", json!("b")),
        ("summary", json!({"total":0})),
    ] {
        let mut invalid = original.clone();
        invalid[key] = value;
        fs::write(workspace.store().join("a.json"), invalid.to_string()).unwrap();
        assert!(read_history(&workspace.store(), &Limits::default()).is_err());
    }
    let mut invalid = original.clone();
    invalid["tests"][0]["outcome"] = json!("unexpected");
    fs::write(workspace.store().join("a.json"), invalid.to_string()).unwrap();
    assert!(read_history(&workspace.store(), &Limits::default()).is_err());
}

#[cfg(unix)]
#[test]
fn history_rejects_symlink_entries_and_collision_does_not_follow_them() {
    let workspace = Workspace::new();
    let report = workspace.report(&["passed"], "chrome", "a.ts", "save");
    fs::create_dir(workspace.store()).unwrap();
    let sentinel = workspace.0.join("sentinel");
    fs::write(&sentinel, "keep").unwrap();
    std::os::unix::fs::symlink(&sentinel, workspace.store().join("a.json")).unwrap();
    assert!(record_history(&workspace.store(), "a", "abc", &report, &Limits::default()).is_err());
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "keep");
    assert!(read_history(&workspace.store(), &Limits::default()).is_err());
}

#[test]
fn detail_truncation_does_not_prevent_complete_history_records() {
    let workspace = Workspace::new();
    let mut report = workspace.report(&["failed", "passed"], "chrome", "a.ts", "save");
    report.truncated = true;
    let limits = Limits {
        max_failures: 0,
        ..Limits::default()
    };
    record_history(&workspace.store(), "a", "abc", &report, &limits).unwrap();
    let history = read_history(&workspace.store(), &limits).unwrap();
    assert_eq!(history.observations[0].retry_recoveries, 1);
    assert_eq!(history.runs[0].tests[0].attempts.len(), 2);
}

#[test]
fn exact_history_event_limits_round_trip_and_lower_limits_do_not_publish() {
    let workspace = Workspace::new();
    let report = workspace.report(&["passed"], "chrome", "a.ts", "save");
    assert!(
        record_history(
            &workspace.store(),
            "too-small",
            "abc",
            &report,
            &Limits {
                max_events: 2,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(!workspace.store().exists());
    let limits = Limits {
        max_events: 3,
        ..Limits::default()
    };
    record_history(&workspace.store(), "exact", "abc", &report, &limits).unwrap();
    assert_eq!(
        read_history(&workspace.store(), &limits)
            .unwrap()
            .runs
            .len(),
        1
    );
    let empty_store = workspace.0.join("empty-store");
    let input = workspace.0.join("empty.json");
    fs::write(&input, r#"{"suites":[],"errors":[]}"#).unwrap();
    let empty = analyze_results(&input, &Limits::default()).unwrap();
    assert!(
        record_history(
            &empty_store,
            "zero",
            "abc",
            &empty,
            &Limits {
                max_events: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(!empty_store.exists());
    let limits = Limits {
        max_events: 1,
        ..Limits::default()
    };
    record_history(&empty_store, "one", "abc", &empty, &limits).unwrap();
    assert_eq!(read_history(&empty_store, &limits).unwrap().runs.len(), 1);
}

#[cfg(unix)]
#[test]
fn history_fifo_entry_is_rejected_without_waiting_for_a_writer() {
    let workspace = Workspace::new();
    fs::create_dir(workspace.store()).unwrap();
    nix::unistd::mkfifo(
        &workspace.store().join("fifo.json"),
        nix::sys::stat::Mode::S_IRUSR,
    )
    .unwrap();
    assert!(read_history(&workspace.store(), &Limits::default()).is_err());
}
