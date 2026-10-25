use super::{AttemptStatus, TestOutcome};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct InputReport {
    pub suites: Vec<InputSuite>,
    #[serde(default)]
    pub config: InputConfig,
    #[serde(default)]
    pub errors: Vec<InputError>,
}

#[derive(Default, Deserialize)]
pub(super) struct InputConfig {
    #[serde(default)]
    pub projects: Vec<InputProject>,
}

#[derive(Deserialize)]
pub(super) struct InputProject {
    pub id: String,
    #[serde(default, rename = "use")]
    pub use_options: InputUse,
}

#[derive(Default, Deserialize)]
pub(super) struct InputUse {
    #[serde(default, rename = "browserName")]
    pub browser_name: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct InputSuite {
    pub title: String,
    #[serde(default)]
    pub specs: Vec<InputSpec>,
    #[serde(default)]
    pub suites: Vec<InputSuite>,
}

#[derive(Deserialize)]
pub(super) struct InputSpec {
    pub title: String,
    pub file: String,
    pub tests: Vec<InputTest>,
}

#[derive(Deserialize)]
pub(super) struct InputTest {
    #[serde(default, rename = "projectId")]
    pub project_id: String,
    #[serde(default, rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "expectedStatus")]
    pub expected_status: AttemptStatus,
    pub status: Option<TestOutcome>,
    pub results: Vec<InputAttempt>,
}

#[derive(Deserialize)]
pub(super) struct InputAttempt {
    pub status: AttemptStatus,
    pub retry: u64,
    pub duration: f64,
    #[serde(default)]
    pub error: Option<InputError>,
    #[serde(default)]
    pub errors: Vec<InputError>,
    #[serde(default)]
    pub attachments: Vec<InputAttachment>,
}

#[derive(Deserialize)]
pub(super) struct InputAttachment {
    pub name: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct InputError {
    pub message: Option<String>,
    pub value: Option<String>,
    pub stack: Option<String>,
}

impl InputError {
    pub fn message(self) -> String {
        self.message
            .or(self.value)
            .or(self.stack)
            .unwrap_or_else(|| "Unspecified runner error".to_owned())
    }
}
