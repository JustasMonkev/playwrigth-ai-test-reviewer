use crate::Limits;
use crate::endpoints::Endpoints;
use crate::temporal::Temporal;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

#[derive(Debug, Default, Serialize)]
pub struct NetworkReport {
    pub log_entries: usize,
    pub requests: usize,
    pub failed: usize,
    pub slow: usize,
    pub incomplete: usize,
    pub unknown_duration: usize,
    pub slow_threshold_ms: u64,
    pub evidence: Vec<NetworkRequest>,
    pub truncated: bool,
    pub endpoints: Vec<crate::EndpointLatency>,
    pub endpoints_truncated: bool,
    pub endpoint_omitted_requests: usize,
    pub temporal_context_truncated: bool,
}

impl NetworkReport {
    pub(crate) fn absent(&self) -> bool {
        self.log_entries == 0
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkRequest {
    pub method: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_error: Option<String>,
    pub failed: bool,
    pub slow: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub temporal_candidates: Vec<crate::TemporalCandidate>,
}

#[derive(Deserialize)]
struct Record {
    #[serde(rename = "type")]
    kind: String,
    snapshot: Option<Snapshot>,
}

#[derive(Deserialize)]
struct Snapshot {
    request: Request,
    response: Response,
    time: Option<f64>,
    pageref: Option<String>,
    #[serde(rename = "_monotonicTime")]
    monotonic_time: Option<f64>,
    timings: Option<Timings>,
}

#[derive(Deserialize)]
struct Request {
    method: String,
    url: String,
}

#[derive(Deserialize)]
struct Response {
    status: Option<i64>,
    #[serde(rename = "_failureText")]
    failure: Option<String>,
}

#[derive(Deserialize)]
struct Timings {
    wait: Option<f64>,
}

struct Ranked {
    request: NetworkRequest,
    sequence: usize,
    stream: String,
    page: Option<String>,
    start: Option<f64>,
}

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.request
            .failed
            .cmp(&other.request.failed)
            .then_with(|| {
                self.request
                    .duration_ms
                    .unwrap_or(-1.0)
                    .total_cmp(&other.request.duration_ms.unwrap_or(-1.0))
            })
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

pub(crate) struct NetworkAccumulator {
    report: NetworkReport,
    retained: BinaryHeap<Reverse<Ranked>>,
    capacity: usize,
    endpoints: Endpoints,
}

impl NetworkAccumulator {
    pub(crate) fn new(limits: &Limits, already_retained: usize, endpoint_retained: usize) -> Self {
        Self {
            report: NetworkReport {
                slow_threshold_ms: limits.slow_ms,
                ..NetworkReport::default()
            },
            retained: BinaryHeap::new(),
            endpoints: Endpoints::new(limits.max_endpoints.saturating_sub(endpoint_retained)),
            capacity: limits.max_network.saturating_sub(already_retained),
        }
    }

    pub(crate) fn start_log(&mut self) {
        self.report.log_entries += 1;
    }

    pub(crate) fn read(&mut self, frame: &[u8], max_text: usize, stream: &str) -> Result<()> {
        let record: Record = serde_json::from_slice(frame)?;
        if record.kind != "resource-snapshot" {
            return Ok(());
        }
        let snapshot = record
            .snapshot
            .context("resource-snapshot is missing snapshot")?;
        self.report.requests += 1;
        let status = snapshot
            .response
            .status
            .filter(|status| (100..=599).contains(status))
            .map(|s| s as u16);
        let transport = snapshot
            .response
            .failure
            .as_deref()
            .filter(|s| !s.is_empty());
        let failed = transport.is_some() || status.is_some_and(|s| s >= 400);
        let duration = snapshot.time.filter(|t| t.is_finite() && *t >= 0.0);
        let slow = duration.is_some_and(|t| t >= self.report.slow_threshold_ms as f64);
        self.report.failed += usize::from(failed);
        self.report.slow += usize::from(slow);
        self.report.incomplete += usize::from(status.is_none() && transport.is_none());
        self.report.unknown_duration += usize::from(duration.is_none());
        let url = redact_url(&snapshot.request.url);
        self.endpoints
            .read(&snapshot.request.method, &url, duration, failed, max_text);
        if !failed && !slow {
            return Ok(());
        }
        if self.capacity == 0 {
            self.report.truncated = true;
            return Ok(());
        }
        let mut request = NetworkRequest {
            method: snapshot.request.method,
            url,
            temporal_candidates: Vec::new(),
            status,
            duration_ms: duration,
            wait_ms: snapshot
                .timings
                .and_then(|t| t.wait)
                .filter(|t| t.is_finite() && *t >= 0.0),
            transport_error: transport.map(transport_code),
            failed,
            slow,
        };
        for value in [&mut request.method, &mut request.url]
            .into_iter()
            .chain(request.transport_error.iter_mut())
        {
            if value.len() > max_text {
                let mut end = max_text;
                while !value.is_char_boundary(end) {
                    end -= 1;
                }
                value.truncate(end);
                self.report.truncated = true;
            }
        }
        let ranked = Reverse(Ranked {
            request,
            sequence: self.report.requests,
            stream: stream.into(),
            page: snapshot.pageref,
            start: snapshot.monotonic_time,
        });
        if self.retained.len() < self.capacity {
            self.retained.push(ranked);
        } else {
            self.report.truncated = true;
            if ranked.0 > self.retained.peek().unwrap().0 {
                *self.retained.peek_mut().unwrap() = ranked;
            }
        }
        Ok(())
    }

    pub(crate) fn finish(mut self, temporal: &Temporal) -> NetworkReport {
        for ranked in self.retained.into_sorted_vec() {
            let mut ranked = ranked.0;
            let (candidates, truncated) = temporal.candidates(
                &ranked.stream,
                ranked.page.as_deref(),
                ranked.start,
                ranked.request.duration_ms,
            );
            ranked.request.temporal_candidates = candidates;
            self.report.temporal_context_truncated |= truncated;
            self.report.evidence.push(ranked.request);
        }
        self.report.temporal_context_truncated |=
            temporal.truncated && !self.report.evidence.is_empty();
        self.report.endpoint_omitted_requests = self.endpoints.omitted_requests;
        self.report.endpoints_truncated = self.endpoints.omitted_requests != 0;
        self.report.truncated |=
            self.report.endpoints_truncated || self.report.temporal_context_truncated;
        self.report.endpoints = self.endpoints.finish();
        self.report
    }
}

pub(crate) fn redact_url(url: &str) -> String {
    let base = url.split(['?', '#']).next().unwrap_or("");
    let Some((scheme, rest)) = base.split_once("://") else {
        return "<non-http-url>".into();
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return "<non-http-url>".into();
    }
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, ""), |(host, path)| (host, path));
    let host = authority.rsplit('@').next().unwrap_or("");
    if host.is_empty() || host.contains(['\\', ' ']) {
        return "<invalid-url>".into();
    }
    format!("{scheme}://{host}/{path}")
}

fn transport_code(message: &str) -> String {
    if let Some(code) = message.strip_prefix("net::ERR_") {
        let code: String = code
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        if !code.is_empty() {
            return format!("net::ERR_{code}");
        }
    }
    "request failed".into()
}
