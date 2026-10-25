mod store;

use crate::{AttemptStatus, Limits, ResultSummary, TestIdentity, TestOutcome, TestRunReport};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTest {
    pub identity: TestIdentity,
    pub expected_status: AttemptStatus,
    pub outcome: TestOutcome,
    pub attempts: Vec<AttemptStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredRun {
    pub schema_version: u8,
    pub run_id: String,
    pub commit: String,
    pub summary: ResultSummary,
    pub tests: Vec<StoredTest>,
}

#[derive(Debug, Serialize)]
pub struct HistoryObservation {
    pub identity: TestIdentity,
    pub runs: usize,
    pub commits: Vec<String>,
    pub expected: usize,
    pub unexpected: usize,
    pub flaky: usize,
    pub skipped: usize,
    pub retry_recoveries: usize,
    pub mixed_outcomes: bool,
    pub same_commit_mixed_outcomes: bool,
}

#[derive(Debug, Serialize)]
pub struct HistoryReport {
    pub schema_version: u8,
    pub runs: Vec<StoredRun>,
    pub observations: Vec<HistoryObservation>,
}

pub fn record_history(
    store: &Path,
    run_id: &str,
    commit: &str,
    report: &TestRunReport,
    limits: &Limits,
) -> Result<StoredRun> {
    if report.tests.iter().any(|test| test.identity_truncated) {
        bail!("Cannot record truncated test identities");
    }
    let run = StoredRun {
        schema_version: 1,
        run_id: run_id.to_owned(),
        commit: commit.to_owned(),
        summary: report.summary.clone(),
        tests: report
            .tests
            .iter()
            .map(|test| StoredTest {
                identity: test.identity.clone(),
                expected_status: test.expected_status,
                outcome: test.outcome,
                attempts: test.attempts.iter().map(|attempt| attempt.status).collect(),
            })
            .collect(),
    };
    validate(&run, limits)?;
    store::write(store, &run, limits)?;
    Ok(run)
}

pub fn read_history(store: &Path, limits: &Limits) -> Result<HistoryReport> {
    let runs = store::read(store, limits)?;
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for run in &runs {
        for test in &run.tests {
            grouped.entry(&test.identity).or_default().push((run, test));
        }
    }
    let observations = grouped
        .into_iter()
        .map(|(identity, entries)| {
            let mut observation = HistoryObservation {
                identity: identity.clone(),
                runs: entries.len(),
                commits: Vec::new(),
                expected: 0,
                unexpected: 0,
                flaky: 0,
                skipped: 0,
                retry_recoveries: 0,
                mixed_outcomes: false,
                same_commit_mixed_outcomes: false,
            };
            let mut by_commit = BTreeMap::<&str, (bool, bool)>::new();
            for (run, test) in entries {
                let outcomes = by_commit.entry(&run.commit).or_default();
                match test.outcome {
                    TestOutcome::Expected => {
                        observation.expected += 1;
                        outcomes.0 = true;
                    }
                    TestOutcome::Unexpected => {
                        observation.unexpected += 1;
                        outcomes.1 = true;
                    }
                    TestOutcome::Flaky => {
                        observation.flaky += 1;
                    }
                    TestOutcome::Skipped => {
                        observation.skipped += 1;
                    }
                }
                if test.outcome == TestOutcome::Flaky
                    && test.attempts.last() == Some(&test.expected_status)
                {
                    observation.retry_recoveries += 1;
                }
            }
            observation.commits = by_commit
                .keys()
                .map(|commit| (*commit).to_owned())
                .collect();
            observation.mixed_outcomes = observation.expected > 0 && observation.unexpected > 0;
            observation.same_commit_mixed_outcomes = by_commit
                .values()
                .any(|&(expected, unexpected)| expected && unexpected);
            observation
        })
        .collect();
    Ok(HistoryReport {
        schema_version: 1,
        runs,
        observations,
    })
}

pub(super) fn validate(run: &StoredRun, limits: &Limits) -> Result<()> {
    if run.schema_version != 1 {
        bail!("Unsupported history schema_version");
    }
    if !valid_run_id(&run.run_id) {
        bail!("run_id must be 1..128 ASCII letters, digits, '-' or '_'");
    }
    if run.commit.is_empty() || run.commit.len() > limits.max_text {
        bail!("commit must be nonempty and fit max_text");
    }
    let mut identities = BTreeSet::new();
    let mut counted = ResultSummary::default();
    if record_count(run) > limits.max_events {
        bail!("History exceeds max_events");
    }
    for test in &run.tests {
        if crate::results::outcome(test.attempts.iter().copied(), test.expected_status)
            != test.outcome
        {
            bail!("History outcome disagrees with attempts");
        }
        let identity = &test.identity;
        if identity.title_path.len() > limits.max_events {
            bail!("History identity exceeds max_events");
        }
        for text in [&identity.project_id, &identity.project_name, &identity.file]
            .into_iter()
            .chain(identity.browser.iter())
            .chain(identity.title_path.iter())
        {
            if text.len() > limits.max_text {
                bail!("History identity exceeds max_text");
            }
        }
        if !identities.insert(identity) {
            bail!("Duplicate test identity in history");
        }
        counted.total += 1;
        match test.outcome {
            TestOutcome::Expected => counted.expected += 1,
            TestOutcome::Unexpected => counted.unexpected += 1,
            TestOutcome::Flaky => counted.flaky += 1,
            TestOutcome::Skipped => counted.skipped += 1,
        }
        counted.attempts += test.attempts.len();
        counted.timed_out += test
            .attempts
            .iter()
            .filter(|&&status| status == AttemptStatus::TimedOut)
            .count();
        counted.interrupted += test
            .attempts
            .iter()
            .filter(|&&status| status == AttemptStatus::Interrupted)
            .count();
    }
    counted.runner_errors = run.summary.runner_errors;
    if serde_json::to_value(counted)? != serde_json::to_value(&run.summary)? {
        bail!("History summary disagrees with test records");
    }
    Ok(())
}

pub(super) fn valid_run_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(super) fn record_count(run: &StoredRun) -> usize {
    run.tests.iter().fold(1usize, |count, test| {
        count.saturating_add(1).saturating_add(test.attempts.len())
    })
}
