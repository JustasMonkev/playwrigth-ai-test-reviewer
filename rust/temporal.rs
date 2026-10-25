use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TemporalCandidate {
    pub relationship: &'static str,
    pub call_id: String,
    pub trace_entry: usize,
    pub action: String,
    pub failed: bool,
}

pub(crate) struct ActionWindow {
    pub(crate) stream: String,
    pub(crate) page: String,
    pub(crate) start: f64,
    pub(crate) end: f64,
    pub(crate) candidate: TemporalCandidate,
}

pub(crate) struct Temporal {
    windows: Vec<ActionWindow>,
    capacity: usize,
    pub(crate) truncated: bool,
}

impl Temporal {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            windows: Vec::new(),
            capacity,
            truncated: false,
        }
    }
    pub(crate) fn add(&mut self, window: ActionWindow) {
        if self.windows.len() < self.capacity {
            self.windows.push(window);
        } else {
            self.truncated = true;
            if window.candidate.failed
                && let Some(index) = self.windows.iter().position(|w| !w.candidate.failed)
            {
                self.windows[index] = window;
            }
        }
    }
    pub(crate) fn candidates(
        &self,
        stream: &str,
        page: Option<&str>,
        start: Option<f64>,
        duration: Option<f64>,
    ) -> (Vec<TemporalCandidate>, bool) {
        let Some((page, start)) = page.zip(start) else {
            return (Vec::new(), false);
        };
        let end = start + duration.unwrap_or(0.0);
        if !start.is_finite() || start < 0.0 || !end.is_finite() {
            return (Vec::new(), false);
        }
        let mut matches: Vec<_> = self
            .windows
            .iter()
            .filter(|w| w.stream == stream && w.page == page && w.start <= end && start <= w.end)
            .collect();
        matches.sort_by_key(|w| {
            (
                !w.candidate.failed,
                w.candidate.trace_entry,
                &w.candidate.call_id,
            )
        });
        let truncated = matches.len() > 4;
        (
            matches
                .into_iter()
                .take(4)
                .map(|w| w.candidate.clone())
                .collect(),
            truncated,
        )
    }
}
