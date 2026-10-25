use super::events::Event;
use super::{Action, Analysis, Completion, MAX_LOGS, Origin, Recording};
use crate::Failure;
use crate::report::FailureIdentity;

const WALL_TIME_RESOLUTION_MS: f64 = 1.0;

impl Analysis<'_> {
    pub(super) fn action(&mut self, event: &mut Event, recording: &Recording) -> Action {
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
        let location = event
            .location
            .take()
            .or_else(|| event.stack.drain(..).next());
        Action {
            action: name,
            selector: params.selector,
            location,
            start_time: event.start_time,
            logs: Vec::new(),
            soft,
            step_id: event.step_id.take(),
            test_level: event.class.as_deref() == Some("Test"),
            recording: recording.clone(),
        }
    }

    pub(super) fn add_log(&mut self, logs: &mut Vec<String>, message: String) {
        if logs.len() < MAX_LOGS {
            logs.push(self.text(message));
        } else {
            self.report.truncated = true;
        }
    }

    pub(super) fn finish(&mut self, mut action: Action, event: Event) {
        let key = action
            .step_id
            .as_ref()
            .map(|step_id| (action.recording.scope.clone(), step_id.clone()));
        let role = usize::from(action.test_level);
        let merged = key.as_ref().and_then(|key| {
            let candidates = &mut self.completed_steps.get_mut(key)?[1 - role];
            let index = candidates
                .iter()
                .position(|candidate| candidate.matches(&action, &event))?;
            candidates.remove(index)
        });
        let pending_key = key.filter(|_| merged.is_none());
        let mut completion = Completion::new(&action, &event);
        if merged.is_some() {
            self.report.summary.actions -= 1;
        }
        let Some(error) = event.error else {
            self.last_error = None;
            if merged.is_some() {
                return;
            }
            self.report.summary.passed += 1;
            if let Some(key) = pending_key {
                self.completed_steps.entry(key).or_default()[role].push_back(completion);
            }
            return;
        };
        let error = error.message();
        self.last_error = (event.kind != "error" && action.test_level && action.location.is_some())
            .then(|| (error.clone(), action.location.clone()));
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
        completion.soft = action.soft;
        if index.is_none() && self.usage.failures >= self.limits.max_failures {
            self.report.truncated = true;
            if let Some(key) = pending_key {
                self.completed_steps.entry(key).or_default()[role].push_back(completion);
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
        let identity = FailureIdentity {
            action: action.action.clone(),
            selector: action.selector.clone(),
            location: action.location.clone(),
        };
        let mut location = action.location;
        if let Some(location) = &mut location {
            location.file = self.text(std::mem::take(&mut location.file));
        }
        let failure = Failure {
            identity,
            action: self.text(action.action),
            selector: action.selector.map(|selector| self.text(selector)),
            error,
            logs: action.logs,
            location,
            duration_ms,
            soft: action.soft,
        };
        let index = if let Some(index) = index {
            let previous = &mut self.report.failures[index];
            previous.identity.location = failure
                .identity
                .location
                .or(previous.identity.location.take());
            previous.identity.selector = previous
                .identity
                .selector
                .take()
                .or(failure.identity.selector);
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
            self.usage.failures += 1;
            self.report.failures.len() - 1
        };
        if let Some(key) = pending_key {
            completion.index = Some(index);
            self.completed_steps.entry(key).or_default()[role].push_back(completion);
        }
    }
}

impl Completion {
    fn new(action: &Action, event: &Event) -> Self {
        Self {
            test_level: action.test_level,
            failed: event.error.is_some(),
            soft: action.soft,
            index: None,
            recording: action.recording.clone(),
            start_time: action.start_time,
            end_time: event.end_time,
            selector: action.selector.clone(),
            location: action.location.clone(),
        }
    }

    fn matches(&self, action: &Action, event: &Event) -> bool {
        if self.test_level == action.test_level
            || self
                .selector
                .as_ref()
                .zip(action.selector.as_ref())
                .is_some_and(|(a, b)| a != b)
            || self
                .location
                .as_ref()
                .zip(action.location.as_ref())
                .is_some_and(|(a, b)| a != b)
        {
            return false;
        }
        let same_stream = self.recording.entry == action.recording.entry;
        if !same_stream
            && !matches!(
                (self.recording.origin, action.recording.origin),
                (Some(Origin::Library), Some(Origin::TestRunner))
                    | (Some(Origin::TestRunner), Some(Origin::Library))
            )
        {
            return false;
        }
        let offsets = if same_stream {
            Some((0.0, 0.0))
        } else {
            self.recording.time_offset.zip(action.recording.time_offset)
        };
        let Some(((previous_start, previous_end), (start, end), (previous_offset, offset))) = self
            .start_time
            .zip(self.end_time)
            .zip(action.start_time.zip(event.end_time))
            .zip(offsets)
            .map(|((previous, current), offsets)| (previous, current, offsets))
        else {
            return false;
        };
        let previous = (
            previous_start + previous_offset,
            previous_end + previous_offset,
        );
        let current = (start + offset, end + offset);
        let (runner, browser) = if self.test_level {
            (previous, current)
        } else {
            (current, previous)
        };
        let rounding = if same_stream {
            0.0
        } else {
            WALL_TIME_RESOLUTION_MS
        };
        runner.0 <= browser.0 + rounding && browser.1 <= runner.1 + rounding
    }
}
