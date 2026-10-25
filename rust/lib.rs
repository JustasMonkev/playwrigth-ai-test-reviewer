mod diagnostics;
mod endpoints;
mod evidence;
mod history;
mod network;
mod report;
mod results;
mod temporal;
mod trace;

pub use evidence::{Evidence, EvidenceCatalog, ExtractedEvidence, extract_evidence, list_evidence};
pub use history::{
    HistoryObservation, HistoryReport, StoredRun, StoredTest, read_history, record_history,
};
pub use network::{NetworkReport, NetworkRequest};
pub use report::{Failure, Location, Report, Summary, compare};
pub use results::{
    AttemptStatus, ResultSummary, TestAttempt, TestIdentity, TestOutcome, TestResult,
    TestRunReport, TraceAttachment, analyze_results,
};
pub use trace::{Limits, analyze, analyze_all};

pub use diagnostics::{ConsoleDiagnostic, ConsoleLocation, ConsoleReport};
pub use endpoints::EndpointLatency;
pub use temporal::TemporalCandidate;
