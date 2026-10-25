use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttemptStatus {
    Passed,
    Failed,
    TimedOut,
    Skipped,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestOutcome {
    Expected,
    Unexpected,
    Flaky,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TestIdentity {
    pub project_id: String,
    pub project_name: String,
    pub repeat_index: usize,
    pub browser: Option<String>,
    pub file: String,
    pub title_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceAttachment {
    pub name: String,
    pub content_type: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAttempt {
    pub status: AttemptStatus,
    pub retry: u64,
    pub duration_ms: f64,
    pub errors: Vec<String>,
    pub traces: Vec<TraceAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub identity: TestIdentity,
    pub identity_truncated: bool,
    pub expected_status: AttemptStatus,
    pub outcome: TestOutcome,
    pub attempts: Vec<TestAttempt>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResultSummary {
    pub total: usize,
    pub expected: usize,
    pub unexpected: usize,
    pub flaky: usize,
    pub skipped: usize,
    pub attempts: usize,
    pub timed_out: usize,
    pub interrupted: usize,
    pub runner_errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunReport {
    pub schema_version: u8,
    pub source: String,
    pub summary: ResultSummary,
    pub tests: Vec<TestResult>,
    pub runner_errors: Vec<String>,
    pub truncated: bool,
}

impl TestRunReport {
    pub fn has_failures(&self) -> bool {
        self.summary.unexpected != 0
            || self.summary.runner_errors != 0
            || self.summary.interrupted != 0
    }
}

pub(crate) fn outcome(
    statuses: impl Iterator<Item = AttemptStatus>,
    expected_status: AttemptStatus,
) -> TestOutcome {
    let (mut expected, mut unexpected, mut skipped) = (0, 0, 0);
    for status in statuses {
        if status == AttemptStatus::Skipped {
            skipped += usize::from(expected_status == AttemptStatus::Skipped);
        } else if status != AttemptStatus::Interrupted {
            if status == expected_status {
                expected += 1;
            } else {
                unexpected += 1;
            }
        }
    }
    if expected == 0 && unexpected == 0 {
        TestOutcome::Skipped
    } else if unexpected == 0 {
        TestOutcome::Expected
    } else if expected == 0 && skipped == 0 {
        TestOutcome::Unexpected
    } else {
        TestOutcome::Flaky
    }
}
