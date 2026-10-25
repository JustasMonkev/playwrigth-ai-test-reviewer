use crate::{Failure, Location, Report, Summary};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_LOGS: usize = 8;

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_bytes: u64,
    pub max_events: usize,
    pub max_failures: usize,
    pub max_text: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 256 * 1024 * 1024,
            max_events: 1_000_000,
            max_failures: 20,
            max_text: 2_048,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    #[serde(default, rename = "type")]
    kind: String,
    call_id: Option<String>,
    step_id: Option<String>,
    title: Option<String>,
    api_name: Option<String>,
    class: Option<String>,
    method: Option<String>,
    params: Option<Params>,
    start_time: Option<f64>,
    end_time: Option<f64>,
    error: Option<TraceError>,
    message: Option<String>,
    #[serde(default)]
    stack: Vec<Location>,
    location: Option<Location>,
    #[serde(default)]
    log: Vec<Log>,
    metadata: Option<Box<Event>>,
    result: Option<ActionResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Params {
    selector: Option<String>,
    is_soft: Option<bool>,
    soft: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ActionResult {
    #[serde(default)]
    log: Vec<Log>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TraceError {
    Text(String),
    Object {
        message: Option<String>,
        name: Option<String>,
        value: Option<String>,
    },
}

impl TraceError {
    fn message(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Object {
                message,
                name,
                value,
            } => message
                .or(value)
                .or(name)
                .unwrap_or_else(|| "Trace records an error without a message".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Log {
    Text(String),
    Object { message: String },
}

impl Log {
    fn message(self) -> String {
        match self {
            Self::Text(text) | Self::Object { message: text } => text,
        }
    }
}

struct Action {
    action: String,
    selector: Option<String>,
    location: Option<Location>,
    start_time: Option<f64>,
    logs: Vec<String>,
    soft: bool,
    step_id: Option<String>,
    test_level: bool,
}

struct Completion {
    test_level: bool,
    failed: bool,
    soft: bool,
    index: Option<usize>,
}

struct Analysis<'a> {
    limits: &'a Limits,
    report: Report,
    bytes: u64,
    recognized: usize,
    completed_steps: HashMap<String, Completion>,
    last_error: Option<(String, Option<Location>)>,
    first_time: Option<f64>,
    last_time: Option<f64>,
}

pub fn analyze(path: &Path, limits: &Limits) -> Result<Report> {
    let mut file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    let size = file.metadata()?.len();
    if size > limits.max_bytes {
        bail!("Input exceeds max_bytes ({} > {})", size, limits.max_bytes);
    }
    let mut signature = [0; 4];
    let count = file.read(&mut signature)?;
    file.seek(SeekFrom::Start(0))?;
    let is_zip = count >= 2 && signature[..2] == *b"PK";
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let mut analysis = Analysis {
        limits,
        report: Report {
            schema_version: 1,
            source: path.to_string_lossy().into_owned(),
            summary: Summary::default(),
            failures: Vec::new(),
            truncated: false,
        },
        bytes: 0,
        recognized: 0,
        completed_steps: HashMap::new(),
        last_error: None,
        first_time: None,
        last_time: None,
    };
    if is_zip || extension.eq_ignore_ascii_case("zip") {
        let mut archive = zip::ZipArchive::new(file).context("Invalid ZIP archive")?;
        let all_names = archive
            .file_names()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|name| name.into_owned())
            .collect::<Vec<_>>();
        let mut names: Vec<_> = all_names
            .iter()
            .filter(|name| name.ends_with(".trace"))
            .cloned()
            .collect();
        if names.is_empty() {
            names = all_names
                .iter()
                .filter(|name| {
                    !name.starts_with("resources/")
                        && (name.ends_with(".json") || name.ends_with(".jsonl"))
                })
                .cloned()
                .collect();
        }
        names.sort();
        if names.is_empty() {
            bail!("ZIP archive contains no .trace, .json, or .jsonl event entries");
        }
        for name in names {
            let entry = archive
                .by_name(&name)
                .with_context(|| format!("Cannot read ZIP entry {name}"))?;
            if entry.size() > limits.max_bytes.saturating_sub(analysis.bytes) {
                bail!(
                    "Decompressed trace data exceeds max_bytes ({})",
                    limits.max_bytes
                );
            }
            analysis
                .read_trace(entry)
                .with_context(|| format!("Invalid trace entry {name}"))?;
        }
    } else {
        if !["trace", "json", "jsonl"]
            .iter()
            .any(|supported| extension.eq_ignore_ascii_case(supported))
        {
            bail!("Unsupported input format; expected .zip, .trace, .json, or .jsonl");
        }
        analysis
            .read_trace(file)
            .with_context(|| format!("Invalid trace {}", path.display()))?;
    }
    if analysis.recognized == 0 {
        bail!("Input contains no supported Playwright action events");
    }
    Ok(analysis.report)
}

impl Analysis<'_> {
    fn read_trace(&mut self, reader: impl Read) -> Result<()> {
        let mut frames = Frames::new(reader, self.limits.max_bytes.saturating_sub(self.bytes));
        let mut actions = HashMap::<String, Action>::new();
        self.report.summary.trace_entries += 1;
        while let Some(frame) = frames.next()? {
            if self.report.summary.events >= self.limits.max_events {
                bail!("Trace exceeds max_events ({})", self.limits.max_events);
            }
            let mut event: Event = serde_json::from_slice(&frame).with_context(|| {
                format!("Malformed JSON at event {}", self.report.summary.events + 1)
            })?;
            self.report.summary.events += 1;
            for time in [event.start_time, event.end_time].into_iter().flatten() {
                self.first_time = Some(self.first_time.map_or(time, |old| old.min(time)));
                self.last_time = Some(self.last_time.map_or(time, |old| old.max(time)));
            }
            match event.kind.as_str() {
                "before" => {
                    self.last_error = None;
                    self.recognized += 1;
                    let id = event
                        .call_id
                        .take()
                        .context("before event is missing callId")?;
                    if event.api_name.is_none() && event.title.is_none() && event.method.is_none() {
                        bail!("before event has no supported action name");
                    }
                    if actions.contains_key(&id) {
                        bail!("Duplicate before event for callId {id}");
                    }
                    self.report.summary.actions += 1;
                    let action = self.action(&mut event);
                    actions.insert(id, action);
                }
                "after" => {
                    self.recognized += 1;
                    let id = event
                        .call_id
                        .take()
                        .context("after event is missing callId")?;
                    if !actions.contains_key(&id) && event.error.is_none() {
                        bail!("after event has no matching before event or recorded error");
                    }
                    let action = actions.remove(&id).unwrap_or_else(|| {
                        self.report.summary.actions += 1;
                        self.action(&mut event)
                    });
                    self.finish(action, event);
                }
                "log" => {
                    if let Some(action) = event.call_id.as_ref().and_then(|id| actions.get_mut(id))
                        && let Some(message) = event.message.take()
                    {
                        self.add_log(&mut action.logs, message);
                    }
                }
                "action" => {
                    self.recognized += 1;
                    self.report.summary.actions += 1;
                    if let Some(metadata) = event.metadata.take() {
                        event = *metadata;
                    }
                    if event.api_name.is_none()
                        && event.title.is_none()
                        && event.method.is_none()
                        && event.error.is_none()
                    {
                        bail!("action event has no supported action metadata");
                    }
                    for time in [event.start_time, event.end_time].into_iter().flatten() {
                        self.first_time = Some(self.first_time.map_or(time, |old| old.min(time)));
                        self.last_time = Some(self.last_time.map_or(time, |old| old.max(time)));
                    }
                    let action = self.action(&mut event);
                    self.finish(action, event);
                }
                "error" => {
                    self.recognized += 1;
                    if event.error.is_none() {
                        event.error =
                            Some(TraceError::Text(event.message.take().unwrap_or_else(
                                || "Trace records an error without a message".into(),
                            )));
                    }
                    let message = event.error.take().unwrap().message();
                    let location = event.location.as_ref().or_else(|| event.stack.first());
                    if self
                        .last_error
                        .as_ref()
                        .is_some_and(|(previous, previous_location)| {
                            previous == &message && previous_location.as_ref() == location
                        })
                    {
                        self.last_error = None;
                        continue;
                    }
                    event.error = Some(TraceError::Text(message));
                    self.report.summary.actions += 1;
                    let action = self.action(&mut event);
                    self.finish(action, event);
                }
                _ => {}
            }
        }
        self.bytes += frames.bytes;
        self.report.summary.incomplete += actions.len();
        if let (Some(first), Some(last)) = (self.first_time, self.last_time) {
            self.report.summary.duration_ms = (last - first).max(0.0);
            if !self.report.summary.duration_ms.is_finite() {
                bail!("Trace timestamps exceed the supported duration range");
            }
        }
        Ok(())
    }

    fn text(&mut self, mut text: String) -> String {
        if text.len() > self.limits.max_text {
            let mut end = self.limits.max_text;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            self.report.truncated = true;
        }
        text
    }

    fn action(&mut self, event: &mut Event) -> Action {
        let soft_title = event
            .title
            .as_ref()
            .is_some_and(|title| title.starts_with("Expect \"soft "));
        let name = event
            .api_name
            .take()
            .or_else(|| event.title.take())
            .unwrap_or_else(|| match (&event.class, &event.method) {
                (Some(class), Some(method)) => format!("{class}.{method}"),
                (_, Some(method))
                    if !["before", "after", "action", ""].contains(&event.kind.as_str()) =>
                {
                    format!("{}.{}", event.kind, method)
                }
                (_, Some(method)) => method.clone(),
                _ if event.kind == "error" => "test error".into(),
                _ => "unknown action".into(),
            });
        let params = event.params.take().unwrap_or_default();
        let soft = name.contains("expect.soft")
            || soft_title
            || params.is_soft == Some(true)
            || params.soft == Some(true);
        let mut location = event
            .location
            .take()
            .or_else(|| event.stack.drain(..).next());
        if let Some(location) = &mut location {
            location.file = self.text(std::mem::take(&mut location.file));
        }
        Action {
            action: self.text(name),
            selector: params.selector.map(|selector| self.text(selector)),
            location,
            start_time: event.start_time,
            logs: Vec::new(),
            soft,
            step_id: event.step_id.take(),
            test_level: event.class.as_deref() == Some("Test"),
        }
    }

    fn add_log(&mut self, logs: &mut Vec<String>, message: String) {
        if logs.len() < MAX_LOGS {
            logs.push(self.text(message));
        } else {
            self.report.truncated = true;
        }
    }

    fn finish(&mut self, mut action: Action, event: Event) {
        let previous = action
            .step_id
            .as_ref()
            .and_then(|step_id| self.completed_steps.remove(step_id));
        let merged = previous.filter(|previous| previous.test_level != action.test_level);
        if merged.is_some() {
            self.report.summary.actions -= 1;
        }
        let Some(error) = event.error else {
            self.last_error = None;
            if merged.is_some() {
                return;
            }
            self.report.summary.passed += 1;
            if let Some(step_id) = action.step_id {
                self.completed_steps.insert(
                    step_id,
                    Completion {
                        test_level: action.test_level,
                        failed: false,
                        soft: false,
                        index: None,
                    },
                );
            }
            return;
        };
        let error = error.message();
        self.last_error = Some((error.clone(), action.location.clone()));
        if !merged.as_ref().is_some_and(|previous| previous.failed) {
            self.report.summary.failed += 1;
            if merged.is_some() {
                self.report.summary.passed -= 1;
            }
        }
        let previously_soft = merged.as_ref().is_some_and(|previous| previous.soft);
        if action.soft && !previously_soft {
            self.report.summary.soft_failures += 1;
        }
        action.soft |= previously_soft;
        let index = merged.as_ref().and_then(|previous| previous.index);
        if index.is_none() && self.report.failures.len() >= self.limits.max_failures {
            self.report.truncated = true;
            if let Some(step_id) = action.step_id {
                self.completed_steps.insert(
                    step_id,
                    Completion {
                        test_level: action.test_level,
                        failed: true,
                        soft: action.soft,
                        index: None,
                    },
                );
            }
            return;
        }
        for log in event
            .log
            .into_iter()
            .chain(event.result.into_iter().flat_map(|result| result.log))
        {
            self.add_log(&mut action.logs, log.message());
        }
        let error = self.text(error);
        let duration_ms = action
            .start_time
            .zip(event.end_time)
            .map(|(start, end)| (end - start).max(0.0));
        let failure = Failure {
            action: action.action,
            selector: action.selector,
            error,
            logs: action.logs,
            location: action.location,
            duration_ms,
            soft: action.soft,
        };
        let index = if let Some(index) = index {
            let previous = &mut self.report.failures[index];
            previous.location = failure.location.or(previous.location.take());
            previous.selector = previous.selector.take().or(failure.selector);
            previous.soft |= failure.soft;
            if action.test_level {
                previous.error = failure.error;
            }
            if previous.logs.is_empty() {
                previous.logs = failure.logs;
            }
            index
        } else {
            self.report.failures.push(failure);
            self.report.failures.len() - 1
        };
        if let Some(step_id) = action.step_id {
            self.completed_steps.insert(
                step_id,
                Completion {
                    test_level: action.test_level,
                    failed: true,
                    soft: action.soft,
                    index: Some(index),
                },
            );
        }
    }
}

struct Frames<R> {
    reader: BufReader<R>,
    bytes: u64,
    max_bytes: u64,
    started: bool,
    array: bool,
    needs_separator: bool,
    done: bool,
}

impl<R: Read> Frames<R> {
    fn new(reader: R, max_bytes: u64) -> Self {
        Self {
            reader: BufReader::new(reader),
            bytes: 0,
            max_bytes,
            started: false,
            array: false,
            needs_separator: false,
            done: false,
        }
    }

    fn byte(&mut self) -> Result<Option<u8>> {
        let Some(&byte) = self.reader.fill_buf()?.first() else {
            return Ok(None);
        };
        self.reader.consume(1);
        self.bytes += 1;
        if self.bytes > self.max_bytes {
            bail!(
                "Decompressed trace data exceeds max_bytes ({})",
                self.max_bytes
            );
        }
        Ok(Some(byte))
    }

    fn non_space(&mut self) -> Result<Option<u8>> {
        while let Some(byte) = self.byte()? {
            if !matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
                return Ok(Some(byte));
            }
        }
        Ok(None)
    }

    fn end_array(&mut self) -> Result<Option<Vec<u8>>> {
        if self.non_space()?.is_some() {
            bail!("Unexpected data following JSON array");
        }
        self.done = true;
        Ok(None)
    }

    fn next(&mut self) -> Result<Option<Vec<u8>>> {
        if self.done {
            return Ok(None);
        }
        let mut next = self.non_space()?;
        if !self.started {
            self.started = true;
            if next == Some(b'[') {
                self.array = true;
                next = self.non_space()?;
                if next == Some(b']') {
                    return self.end_array();
                }
            }
        }
        if self.array && self.needs_separator {
            match next {
                Some(b']') => return self.end_array(),
                Some(b',') => next = self.non_space()?,
                _ => bail!("Expected comma or closing bracket in JSON array"),
            }
        }
        match next {
            None if !self.array => {
                self.done = true;
                return Ok(None);
            }
            Some(b'{') => {}
            _ => bail!("Expected a JSON event object"),
        }
        let mut data = vec![b'{'];
        let mut depth = 1usize;
        let mut quoted = false;
        let mut escaped = false;
        while depth > 0 {
            let byte = self.byte()?.context("Unexpected end of JSON event")?;
            if data.len() >= MAX_EVENT_BYTES {
                bail!(
                    "JSON event exceeds the {} byte per-event limit",
                    MAX_EVENT_BYTES
                );
            }
            data.push(byte);
            if quoted {
                if escaped {
                    escaped = false
                } else if byte == b'\\' {
                    escaped = true
                } else if byte == b'"' {
                    quoted = false
                }
            } else {
                match byte {
                    b'"' => quoted = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => depth -= 1,
                    _ => {}
                }
            }
        }
        self.needs_separator = true;
        Ok(Some(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &[u8], limits: &Limits) -> Result<Report> {
        let mut analysis = Analysis {
            limits,
            report: Report {
                schema_version: 1,
                source: "test.trace".into(),
                summary: Summary::default(),
                failures: Vec::new(),
                truncated: false,
            },
            bytes: 0,
            recognized: 0,
            completed_steps: HashMap::new(),
            last_error: None,
            first_time: None,
            last_time: None,
        };
        analysis.read_trace(input)?;
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
        let input = br#"{"type":"before","callId":"browser","stepId":"step","class":"Frame","method":"expect"}{"type":"after","callId":"browser","error":"failed"}{"type":"before","callId":"test","stepId":"step","class":"Test","method":"expect","params":{"isSoft":true}}{"type":"after","callId":"test","error":"detailed failure"}"#;
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
        let mut analysis = Analysis {
            limits: &limits,
            report: Report {
                schema_version: 1,
                source: "test.zip".into(),
                summary: Summary::default(),
                failures: Vec::new(),
                truncated: false,
            },
            bytes: 0,
            recognized: 0,
            completed_steps: HashMap::new(),
            last_error: None,
            first_time: None,
            last_time: None,
        };
        for (start, end) in [(10, 20), (12, 18), (22, 25)] {
            let input = format!(
                r#"{{"type":"action","method":"click","startTime":{start},"endTime":{end}}}"#
            );
            analysis.read_trace(input.as_bytes()).unwrap();
        }
        assert_eq!(analysis.report.summary.duration_ms, 15.0);
    }
}
