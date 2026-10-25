use crate::network::redact_url;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize)]
pub struct ConsoleReport {
    pub errors: usize,
    pub uncaught_exceptions: usize,
    pub evidence: Vec<ConsoleDiagnostic>,
    pub truncated: bool,
}
impl ConsoleReport {
    pub(crate) fn absent(&self) -> bool {
        self.errors + self.uncaught_exceptions == 0
    }
}

#[derive(Debug, Serialize)]
pub struct ConsoleDiagnostic {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ConsoleLocation>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLocation {
    pub url: String,
    pub line_number: Option<u64>,
    pub column_number: Option<u64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    text: Option<String>,
    location: Option<ConsoleLocation>,
    params: Option<Params>,
}
#[derive(Deserialize)]
struct Params {
    error: Option<SerializedError>,
}
#[derive(Deserialize)]
struct SerializedError {
    error: Option<Error>,
    value: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct Error {
    message: Option<String>,
    name: Option<String>,
}

pub(crate) struct Diagnostics {
    report: ConsoleReport,
    capacity: usize,
}
impl Diagnostics {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            report: ConsoleReport::default(),
            capacity,
        }
    }
    pub(crate) fn read(&mut self, frame: &[u8], max_text: usize) -> Result<bool> {
        #[derive(Deserialize)]
        struct Header {
            #[serde(default, rename = "type")]
            kind: String,
            method: Option<serde_json::Value>,
            #[serde(rename = "messageType")]
            message_type: Option<serde_json::Value>,
        }
        let header: Header = serde_json::from_slice(frame)?;
        let uncaught = header.kind == "event"
            && header.method.as_ref().and_then(serde_json::Value::as_str) == Some("pageError");
        if !uncaught
            && !(header.kind == "console"
                && header
                    .message_type
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
                    == Some("error"))
        {
            return Ok(false);
        }
        let record: Record = serde_json::from_slice(frame)?;
        self.report.uncaught_exceptions += usize::from(uncaught);
        self.report.errors += usize::from(!uncaught);
        let message = if uncaught {
            record
                .params
                .and_then(|p| p.error)
                .and_then(|serialized| {
                    serialized
                        .error
                        .and_then(|e| e.message.or(e.name))
                        .or_else(|| {
                            serialized.value.and_then(|v| {
                                v.get("s")
                                    .and_then(serde_json::Value::as_str)
                                    .map(str::to_owned)
                            })
                        })
                })
                .unwrap_or_else(|| "Uncaught browser exception".into())
        } else {
            record.text.unwrap_or_default()
        };
        let mut diagnostic = ConsoleDiagnostic {
            kind: if uncaught {
                "uncaught_exception"
            } else {
                "console_error"
            },
            message,
            location: record.location,
        };
        if let Some(location) = &mut diagnostic.location {
            location.url = redact_url(&location.url);
        }
        for text in std::iter::once(&mut diagnostic.message)
            .chain(diagnostic.location.iter_mut().map(|l| &mut l.url))
        {
            if text.len() > max_text {
                let mut end = max_text;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
                self.report.truncated = true;
            }
        }
        if self.report.evidence.len() < self.capacity {
            self.report.evidence.push(diagnostic);
        } else {
            self.report.truncated = true;
            if uncaught
                && let Some(index) = self
                    .report
                    .evidence
                    .iter()
                    .rposition(|d| d.kind == "console_error")
            {
                self.report.evidence[index] = diagnostic;
            }
        }
        Ok(true)
    }
    pub(crate) fn finish(self) -> ConsoleReport {
        self.report
    }
}
