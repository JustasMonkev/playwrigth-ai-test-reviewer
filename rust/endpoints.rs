use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct EndpointLatency {
    pub method: String,
    pub url: String,
    pub requests: usize,
    pub known_duration: usize,
    pub unknown_duration: usize,
    pub failed: usize,
    pub min_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub mean_ms: Option<f64>,
}

pub(crate) struct Endpoints {
    retained: BTreeMap<(String, String), EndpointLatency>,
    capacity: usize,
    pub(crate) omitted_requests: usize,
}

impl Endpoints {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            retained: BTreeMap::new(),
            capacity,
            omitted_requests: 0,
        }
    }

    pub(crate) fn read(
        &mut self,
        method: &str,
        url: &str,
        duration: Option<f64>,
        failed: bool,
        max_text: usize,
    ) {
        let key = (method.to_owned(), url.to_owned());
        if url.starts_with('<')
            || method.len() > max_text
            || url.len() > max_text
            || (!self.retained.contains_key(&key) && self.retained.len() >= self.capacity)
        {
            self.omitted_requests += 1;
            return;
        }
        let endpoint = self.retained.entry(key).or_insert_with(|| EndpointLatency {
            method: method.into(),
            url: url.into(),
            requests: 0,
            known_duration: 0,
            unknown_duration: 0,
            failed: 0,
            min_ms: None,
            max_ms: None,
            mean_ms: None,
        });
        endpoint.requests += 1;
        endpoint.failed += usize::from(failed);
        if let Some(value) = duration {
            endpoint.known_duration += 1;
            endpoint.min_ms = Some(endpoint.min_ms.map_or(value, |min| min.min(value)));
            endpoint.max_ms = Some(endpoint.max_ms.map_or(value, |max| max.max(value)));
            endpoint.mean_ms = Some(endpoint.mean_ms.map_or(value, |mean| {
                mean + (value - mean) / endpoint.known_duration as f64
            }));
        } else {
            endpoint.unknown_duration += 1;
        }
    }

    pub(crate) fn finish(self) -> Vec<EndpointLatency> {
        self.retained.into_values().collect()
    }
}
