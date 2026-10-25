use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    #[serde(skip)]
    pub(crate) identity: FailureIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_entry: Option<usize>,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub error: String,
    pub logs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    pub soft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FailureIdentity {
    pub(crate) action: String,
    pub(crate) selector: Option<String>,
    pub(crate) location: Option<Location>,
}

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub trace_entries: usize,
    pub events: usize,
    pub actions: usize,
    pub passed: usize,
    pub failed: usize,
    pub incomplete: usize,
    pub soft_failures: usize,
    pub duration_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u8,
    pub source: String,
    pub summary: Summary,
    pub failures: Vec<Failure>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "crate::NetworkReport::absent")]
    pub network: crate::NetworkReport,
    #[serde(skip_serializing_if = "crate::ConsoleReport::absent")]
    pub console: crate::ConsoleReport,
}

impl Report {
    pub fn has_failures(&self) -> bool {
        self.summary.failed != 0
    }

    pub fn failure_count(&self) -> usize {
        self.summary.failed
    }
}

pub fn compare(before: &Report, after: &Report) -> Value {
    let mut previous = BTreeMap::<_, VecDeque<_>>::new();
    for failure in &before.failures {
        previous
            .entry(&failure.identity)
            .or_default()
            .push_back(failure);
    }
    let mut introduced = Vec::new();
    let mut persistent = Vec::new();
    for failure in &after.failures {
        match previous
            .get_mut(&failure.identity)
            .and_then(VecDeque::pop_front)
        {
            Some(old) => persistent.push(json!({"before": old, "after": failure})),
            None => introduced.push(failure),
        }
    }
    let resolved: Vec<_> = previous.into_values().flatten().collect();
    let mut result = json!({
        "schema_version": 1,
        "before": { "source": before.source, "failed": before.failure_count() },
        "after": { "source": after.source, "failed": after.failure_count() },
        "introduced": introduced,
        "resolved": resolved,
        "persistent": persistent,
        "truncated": before.truncated || after.truncated,
    });
    if !before.network.absent() || !after.network.absent() {
        let endpoint_comparisons: Vec<_> = after
            .network
            .endpoints
            .iter()
            .filter_map(|new| {
                let old = before
                    .network
                    .endpoints
                    .iter()
                    .find(|old| old.method == new.method && old.url == new.url)?;
                let mut comparison =
                    json!({"method":new.method,"url":new.url,"before":old,"after":new});
                if let Some((old_mean, new_mean)) = old.mean_ms.zip(new.mean_ms) {
                    comparison["mean_delta_ms"] = json!(new_mean - old_mean);
                }
                Some(comparison)
            })
            .collect();
        result["network"] = json!({"before": before.network, "after": after.network, "endpoint_comparisons": endpoint_comparisons});
    }
    if !before.console.absent() || !after.console.absent() {
        result["console"] = json!({"before":before.console,"after":after.console});
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(errors: &[&str]) -> Report {
        Report {
            schema_version: 1,
            source: "trace.zip".into(),
            summary: Summary {
                failed: errors.len(),
                ..Summary::default()
            },
            failures: errors
                .iter()
                .map(|error| Failure {
                    identity: FailureIdentity {
                        action: "locator.click".into(),
                        selector: Some("#submit".into()),
                        location: Some(Location {
                            file: "test.ts".into(),
                            line: Some(4),
                            column: Some(2),
                        }),
                    },
                    call_id: None,
                    trace_entry: None,
                    action: "locator.click".into(),
                    selector: Some("#submit".into()),
                    error: (*error).into(),
                    logs: Vec::new(),
                    location: Some(Location {
                        file: "test.ts".into(),
                        line: Some(4),
                        column: Some(2),
                    }),
                    duration_ms: None,
                    soft: false,
                })
                .collect(),
            truncated: false,
            network: crate::NetworkReport::default(),
            console: crate::ConsoleReport::default(),
        }
    }

    #[test]
    fn comparison_matches_repeated_actions_without_using_error_text() {
        let result = compare(&report(&["old", "old again"]), &report(&["changed"]));
        assert_eq!(result["persistent"].as_array().unwrap().len(), 1);
        assert_eq!(result["resolved"].as_array().unwrap().len(), 1);
        assert_eq!(result["introduced"], json!([]));
    }
}
