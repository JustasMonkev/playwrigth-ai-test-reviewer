mod input;
mod output;

use crate::Limits;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct EvidenceCatalog {
    pub schema_version: u32,
    pub source: String,
    pub total: usize,
    pub items: Vec<Evidence>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct Evidence {
    pub id: String,
    pub kind: &'static str,
    pub page_id: Option<String>,
    pub frame_id: Option<String>,
    pub timestamp_ms: Option<f64>,
    pub action_id: Option<String>,
    pub snapshot_name: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub resource_status: &'static str,
    pub bytes: Option<u64>,
    pub text_truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct ExtractedEvidence {
    pub schema_version: u32,
    pub id: String,
    pub kind: &'static str,
    pub bytes: u64,
    pub output: String,
}

pub fn list_evidence(path: &Path, limits: &Limits) -> Result<EvidenceCatalog> {
    let mut input = input::Input::open(path, limits)?;
    let mut catalog = EvidenceCatalog {
        schema_version: 1,
        source: path.to_string_lossy().into_owned(),
        total: 0,
        items: Vec::new(),
        truncated: false,
    };
    input.scan(limits, |id, value, raw, resources| {
        if let Some((item, _)) = describe(id, value, raw, resources, limits)? {
            catalog.total += 1;
            if catalog.items.len() < limits.max_evidence {
                catalog.truncated |= item.text_truncated;
                catalog.items.push(item);
            } else {
                catalog.truncated = true;
            }
        }
        Ok(())
    })?;
    Ok(catalog)
}

enum Selected {
    Record(Vec<u8>),
    Resource(String),
}

pub fn extract_evidence(
    path: &Path,
    id: &str,
    output: &Path,
    limits: &Limits,
) -> Result<ExtractedEvidence> {
    let mut input = input::Input::open(path, limits)?;
    let mut selected = None;
    let used = input.scan(limits, |candidate, value, raw, resources| {
        if let Some((item, resource)) = describe(candidate, value, raw, resources, limits)?
            && item.id == id
        {
            if !matches!(item.resource_status, "available" | "inline") {
                bail!("Selected evidence resource is {}", item.resource_status);
            }
            selected = Some((
                item,
                resource.map_or_else(|| Selected::Record(raw.to_vec()), Selected::Resource),
            ));
        }
        Ok(())
    })?;
    let (item, reference) = selected.context("Unknown evidence ID")?;
    let bytes = match reference {
        Selected::Resource(name) => input.resource(&name, limits.max_bytes.saturating_sub(used))?,
        Selected::Record(bytes) => bytes,
    };
    output::write_new(output, &bytes)?;
    Ok(ExtractedEvidence {
        schema_version: 1,
        id: item.id,
        kind: item.kind,
        bytes: bytes.len() as u64,
        output: output.to_string_lossy().into_owned(),
    })
}

fn describe(
    id: String,
    value: &Value,
    raw: &[u8],
    resources: Option<&input::Resources>,
    limits: &Limits,
) -> Result<Option<(Evidence, Option<String>)>> {
    let kind = value.get("type").and_then(Value::as_str);
    let (kind, data, resource, status, bytes) = match kind {
        Some("frame-snapshot") => {
            let snapshot = value
                .get("snapshot")
                .filter(|v| v.is_object())
                .context("frame-snapshot is missing snapshot object")?;
            if !snapshot.get("html").is_some_and(Value::is_array) {
                bail!("frame-snapshot is missing recorded html array");
            }
            (
                "frame-snapshot",
                snapshot,
                None,
                "inline",
                Some(raw.len() as u64),
            )
        }
        Some("screencast-frame") => {
            let name = value
                .get("sha1")
                .and_then(Value::as_str)
                .context("screencast-frame is missing sha1")?;
            if name.is_empty()
                || name.len() > 255
                || name == "."
                || name == ".."
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"@._-".contains(&b))
            {
                bail!("Unsafe screenshot resource reference");
            }
            let name = format!("resources/{name}");
            let bytes = resources.and_then(|map| map.get(&name)).copied();
            if bytes.is_some_and(|size| size > limits.max_bytes) {
                bail!("Screenshot resource exceeds max_bytes");
            }
            let status = match (resources, bytes) {
                (None, _) => "unavailable_raw_trace",
                (_, None) => "missing",
                (_, Some(_)) => "available",
            };
            ("screencast-frame", value, Some(name), status, bytes)
        }
        _ => return Ok(None),
    };
    let dimensions = if kind == "frame-snapshot" {
        data.get("viewport")
    } else {
        Some(data)
    };
    let mut text_truncated = false;
    let mut text = |key| {
        data.get(key).and_then(Value::as_str).map(|value| {
            let mut end = value.len().min(limits.max_text);
            while !value.is_char_boundary(end) {
                end -= 1;
            }
            text_truncated |= end < value.len();
            value[..end].to_owned()
        })
    };
    Ok(Some((
        Evidence {
            id,
            kind,
            page_id: text("pageId"),
            frame_id: text("frameId"),
            timestamp_ms: data.get("timestamp").and_then(Value::as_f64),
            action_id: text("callId"),
            snapshot_name: text("snapshotName"),
            width: dimensions
                .and_then(|v| v.get("width"))
                .and_then(Value::as_u64),
            height: dimensions
                .and_then(|v| v.get("height"))
                .and_then(Value::as_u64),
            resource_status: status,
            bytes,
            text_truncated,
        },
        resource,
    )))
}
