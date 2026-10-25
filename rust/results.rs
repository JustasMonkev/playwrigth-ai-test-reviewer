mod input;
mod model;

use crate::Limits;
use anyhow::{Context, Result, bail};
use input::{InputReport, InputSuite, InputTest};
pub use model::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn analyze_results(path: &Path, limits: &Limits) -> Result<TestRunReport> {
    let bytes = read_bounded(path, limits.max_bytes)?;
    let input: InputReport =
        serde_json::from_slice(&bytes).context("Invalid Playwright JSON report")?;
    let mut usage = input.errors.len();
    if usage > limits.max_events {
        bail!("Results exceed max_events");
    }
    let mut report = TestRunReport {
        schema_version: 1,
        source: path.to_string_lossy().into_owned(),
        summary: ResultSummary::default(),
        tests: Vec::new(),
        runner_errors: Vec::new(),
        truncated: false,
    };
    let browsers: BTreeMap<_, _> = input
        .config
        .projects
        .into_iter()
        .map(|project| (project.id, project.use_options.browser_name))
        .collect();
    report.summary.runner_errors = input.errors.len();
    report.runner_errors = input
        .errors
        .into_iter()
        .take(limits.max_failures)
        .map(|error| bounded(&error.message(), limits.max_text, &mut report.truncated))
        .collect();
    report.truncated |= report.summary.runner_errors > report.runner_errors.len();
    let mut identities = BTreeSet::new();
    for suite in input.suites {
        visit(suite, &[], &browsers, limits, &mut usage, &mut report)?;
    }
    for test in &report.tests {
        if !test.identity_truncated && !identities.insert(&test.identity) {
            bail!("Duplicate test identity in results");
        }
    }
    Ok(report)
}

fn visit(
    suite: InputSuite,
    parents: &[String],
    browsers: &BTreeMap<String, Option<String>>,
    limits: &Limits,
    usage: &mut usize,
    report: &mut TestRunReport,
) -> Result<()> {
    charge(usage, limits)?;
    let mut titles = parents.to_vec();
    if !suite.title.is_empty() {
        titles.push(suite.title);
    }
    for spec in suite.specs {
        charge(usage, limits)?;
        let mut title_path = titles.clone();
        title_path.push(spec.title);
        let mut repeats = BTreeMap::new();
        for test in spec.tests {
            charge(usage, limits)?;
            let mut identity_truncated = false;
            let browser = browsers
                .get(&test.project_id)
                .and_then(Clone::clone)
                .map(|value| bounded(&value, limits.max_text, &mut identity_truncated));
            let repeat_index = repeats
                .entry((test.project_id.clone(), test.project_name.clone()))
                .or_insert(0);
            let identity = TestIdentity {
                repeat_index: *repeat_index,
                project_id: bounded(&test.project_id, limits.max_text, &mut identity_truncated),
                project_name: bounded(&test.project_name, limits.max_text, &mut identity_truncated),
                browser,
                file: bounded(&spec.file, limits.max_text, &mut identity_truncated),
                title_path: title_path
                    .iter()
                    .map(|title| bounded(title, limits.max_text, &mut identity_truncated))
                    .collect(),
            };
            *repeat_index += 1;
            let analyzed = analyze_test(test, identity, identity_truncated, limits, usage, report)?;
            report.tests.push(analyzed);
        }
    }
    for child in suite.suites {
        visit(child, &titles, browsers, limits, usage, report)?;
    }
    Ok(())
}

fn analyze_test(
    test: InputTest,
    identity: TestIdentity,
    identity_truncated: bool,
    limits: &Limits,
    usage: &mut usize,
    report: &mut TestRunReport,
) -> Result<TestResult> {
    let mut attempts = Vec::new();
    for result in test.results {
        charge(usage, limits)?;
        if !result.duration.is_finite() || result.duration < 0.0 {
            bail!("Invalid attempt duration");
        }
        report.summary.attempts += 1;
        match result.status {
            AttemptStatus::Interrupted => report.summary.interrupted += 1,
            AttemptStatus::TimedOut => report.summary.timed_out += 1,
            _ => {}
        }
        let mut errors = result.errors;
        if errors.is_empty() {
            errors.extend(result.error);
        }
        let error_count = errors.len();
        let errors = errors
            .into_iter()
            .take(limits.max_failures)
            .map(|error| bounded(&error.message(), limits.max_text, &mut report.truncated))
            .collect::<Vec<_>>();
        let trace_count = result
            .attachments
            .iter()
            .filter(|attachment| attachment.name == "trace")
            .count();
        let traces = result
            .attachments
            .into_iter()
            .filter(|attachment| attachment.name == "trace")
            .take(limits.max_failures)
            .map(|attachment| TraceAttachment {
                name: bounded(&attachment.name, limits.max_text, &mut report.truncated),
                content_type: bounded(
                    &attachment.content_type,
                    limits.max_text,
                    &mut report.truncated,
                ),
                path: attachment
                    .path
                    .map(|value| bounded(&value, limits.max_text, &mut report.truncated)),
            })
            .collect::<Vec<_>>();
        report.truncated |= error_count > errors.len() || trace_count > traces.len();
        attempts.push(TestAttempt {
            status: result.status,
            retry: result.retry,
            duration_ms: result.duration,
            errors,
            traces,
        });
    }
    let outcome = model::outcome(
        attempts.iter().map(|attempt| attempt.status),
        test.expected_status,
    );
    if let Some(reported) = test.status
        && reported != outcome
    {
        bail!("Reported test outcome disagrees with attempts");
    }
    report.summary.total += 1;
    match outcome {
        TestOutcome::Expected => report.summary.expected += 1,
        TestOutcome::Unexpected => report.summary.unexpected += 1,
        TestOutcome::Flaky => report.summary.flaky += 1,
        TestOutcome::Skipped => report.summary.skipped += 1,
    }
    report.truncated |= identity_truncated;
    Ok(TestResult {
        identity,
        identity_truncated,
        expected_status: test.expected_status,
        outcome,
        attempts,
    })
}

fn charge(usage: &mut usize, limits: &Limits) -> Result<()> {
    if *usage >= limits.max_events {
        bail!("Results exceed max_events");
    }
    *usage += 1;
    Ok(())
}

pub(crate) fn bounded(value: &str, limit: usize, truncated: &mut bool) -> String {
    if value.len() <= limit {
        return value.to_owned();
    }
    *truncated = true;
    let mut end = limit.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

pub(crate) fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    if !std::fs::metadata(path)
        .with_context(|| format!("Cannot inspect {}", path.display()))?
        .is_file()
    {
        bail!("Input must be a regular file");
    }
    let file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    read_opened(file, max_bytes)
}

pub(crate) fn read_opened(file: File, max_bytes: u64) -> Result<Vec<u8>> {
    if !file.metadata()?.is_file() {
        bail!("Input must be a regular file");
    }
    if file.metadata()?.len() > max_bytes {
        bail!("Input exceeds max_bytes");
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        bail!("Input exceeds max_bytes");
    }
    Ok(bytes)
}
