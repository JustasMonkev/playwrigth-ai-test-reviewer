use crate::Location;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Event {
    #[serde(default, rename = "type")]
    pub(super) kind: String,
    pub(super) origin: Option<String>,
    pub(super) wall_time: Option<f64>,
    pub(super) monotonic_time: Option<f64>,
    pub(super) call_id: Option<String>,
    pub(super) step_id: Option<String>,
    pub(super) title: Option<String>,
    pub(super) api_name: Option<String>,
    pub(super) class: Option<String>,
    pub(super) method: Option<String>,
    pub(super) params: Option<Params>,
    pub(super) start_time: Option<f64>,
    pub(super) end_time: Option<f64>,
    pub(super) error: Option<TraceError>,
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) stack: Vec<Location>,
    pub(super) location: Option<Location>,
    #[serde(default)]
    pub(super) log: Vec<Log>,
    pub(super) metadata: Option<Box<Event>>,
    pub(super) result: Option<ActionResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Params {
    pub(super) selector: Option<String>,
    pub(super) is_soft: Option<bool>,
    pub(super) soft: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ActionResult {
    #[serde(default)]
    pub(super) log: Vec<Log>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum TraceError {
    Text(String),
    Object {
        message: Option<String>,
        name: Option<String>,
        value: Option<String>,
    },
}

impl TraceError {
    pub(super) fn message(self) -> String {
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
pub(super) enum Log {
    Text(String),
    Object { message: String },
}

impl Log {
    pub(super) fn message(self) -> String {
        match self {
            Self::Text(text) | Self::Object { message: text } => text,
        }
    }
}
