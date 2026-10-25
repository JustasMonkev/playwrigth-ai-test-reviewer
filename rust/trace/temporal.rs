use super::events::Event;
use super::{Action, Analysis, Origin};
use crate::temporal::{ActionWindow, TemporalCandidate};

impl Analysis<'_> {
    pub(super) fn record_temporal(&mut self, action: &Action, event: &Event) {
        if action.recording.origin != Some(Origin::Library) {
            return;
        }
        let Some(((start, end), (page, call_id))) = action
            .start_time
            .zip(event.end_time)
            .zip(action.page_id.as_ref().zip(action.call_id.as_ref()))
        else {
            return;
        };
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end < start {
            return;
        }
        if call_id.len() > self.limits.max_text {
            self.temporal.truncated = true;
            return;
        }
        let name = self.text(action.action.clone());
        self.temporal.add(ActionWindow {
            stream: action.recording.stream.clone(),
            page: page.clone(),
            start,
            end,
            candidate: TemporalCandidate {
                relationship: "temporal_overlap",
                call_id: call_id.clone(),
                trace_entry: action.recording.entry,
                action: name,
                failed: event.error.is_some(),
            },
        });
    }
}
