mod actions;
mod events;
mod frames;
#[cfg(test)]
mod tests;

use crate::{Location, Report, Summary};
use anyhow::{Context, Result, bail};
use events::{Event, TraceError};
use frames::Frames;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

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

struct Action {
    action: String,
    selector: Option<String>,
    location: Option<Location>,
    start_time: Option<f64>,
    logs: Vec<String>,
    soft: bool,
    step_id: Option<String>,
    test_level: bool,
    recording: Recording,
}

struct Completion {
    test_level: bool,
    failed: bool,
    soft: bool,
    index: Option<usize>,
    recording: Recording,
    start_time: Option<f64>,
    end_time: Option<f64>,
    selector: Option<String>,
    location: Option<Location>,
}

#[derive(Clone, Copy, PartialEq)]
enum Origin {
    Library,
    TestRunner,
}

#[derive(Clone, Default)]
struct Recording {
    scope: PathBuf,
    entry: usize,
    origin: Option<Origin>,
    time_offset: Option<f64>,
}

#[derive(Default)]
struct Usage {
    bytes: u64,
    events: usize,
    failures: usize,
}

struct Analysis<'a> {
    limits: &'a Limits,
    usage: &'a mut Usage,
    report: Report,
    recognized: usize,
    completed_steps: HashMap<(PathBuf, String), [VecDeque<Completion>; 2]>,
    last_error: Option<(String, Option<Location>)>,
    first_time: Option<f64>,
    last_time: Option<f64>,
}

pub fn analyze(path: &Path, limits: &Limits) -> Result<Report> {
    analyze_one(path, limits, &mut Usage::default())
}

pub fn analyze_all(paths: &[PathBuf], limits: &Limits) -> Result<Vec<Report>> {
    let mut usage = Usage::default();
    paths
        .iter()
        .map(|path| analyze_one(path, limits, &mut usage))
        .collect()
}

fn analyze_one(path: &Path, limits: &Limits, usage: &mut Usage) -> Result<Report> {
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
        usage,
        report: Report {
            schema_version: 1,
            source: path.to_string_lossy().into_owned(),
            summary: Summary::default(),
            failures: Vec::new(),
            truncated: false,
        },
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
            if entry.size() > limits.max_bytes.saturating_sub(analysis.usage.bytes) {
                bail!(
                    "Decompressed trace data exceeds max_bytes ({})",
                    limits.max_bytes
                );
            }
            analysis
                .read_trace(entry, Path::new(&name).parent().unwrap_or(Path::new("")))
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
            .read_trace(file, Path::new(""))
            .with_context(|| format!("Invalid trace {}", path.display()))?;
    }
    if analysis.recognized == 0 {
        bail!("Input contains no supported Playwright action events");
    }
    Ok(analysis.report)
}

impl Analysis<'_> {
    fn read_trace(&mut self, reader: impl Read, scope: &Path) -> Result<()> {
        self.last_error = None;
        let mut frames = Frames::new(
            reader,
            self.limits.max_bytes.saturating_sub(self.usage.bytes),
        );
        let mut actions = HashMap::<String, Action>::new();
        self.report.summary.trace_entries += 1;
        let mut recording = Recording {
            scope: scope.to_owned(),
            entry: self.report.summary.trace_entries,
            ..Recording::default()
        };
        while let Some(frame) = frames.next()? {
            if self.usage.events >= self.limits.max_events {
                bail!("Trace exceeds max_events ({})", self.limits.max_events);
            }
            let mut event: Event = serde_json::from_slice(&frame).with_context(|| {
                format!("Malformed JSON at event {}", self.report.summary.events + 1)
            })?;
            self.report.summary.events += 1;
            self.usage.events += 1;
            for time in [event.start_time, event.end_time].into_iter().flatten() {
                self.first_time = Some(self.first_time.map_or(time, |old| old.min(time)));
                self.last_time = Some(self.last_time.map_or(time, |old| old.max(time)));
            }
            match event.kind.as_str() {
                "context-options" => {
                    recording.origin = match event.origin.as_deref() {
                        Some("library") => Some(Origin::Library),
                        Some("testRunner") => Some(Origin::TestRunner),
                        _ => None,
                    };
                    recording.time_offset = event
                        .wall_time
                        .zip(event.monotonic_time)
                        .map(|(wall, monotonic)| wall - monotonic)
                        .filter(|offset| offset.is_finite());
                }
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
                    let action = self.action(&mut event, &recording);
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
                        self.action(&mut event, &recording)
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
                    let action = self.action(&mut event, &recording);
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
                        .take()
                        .is_some_and(|(previous, previous_location)| {
                            previous == message && previous_location.as_ref() == location
                        })
                    {
                        continue;
                    }
                    event.error = Some(TraceError::Text(message));
                    self.report.summary.actions += 1;
                    let action = self.action(&mut event, &recording);
                    self.finish(action, event);
                }
                _ => {}
            }
        }
        self.usage.bytes += frames.bytes;
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
}
