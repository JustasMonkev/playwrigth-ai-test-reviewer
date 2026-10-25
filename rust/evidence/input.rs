use crate::{Limits, trace::frames::Frames};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path};
use zip::ZipArchive;

pub(super) type Resources = HashMap<String, u64>;

pub(super) enum Input {
    Raw(File),
    Zip {
        archive: ZipArchive<File>,
        entries: Vec<String>,
        resources: Resources,
    },
}

impl Input {
    pub(super) fn open(path: &Path, limits: &Limits) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NONBLOCK);
        }
        let mut file = options
            .open(path)
            .with_context(|| format!("Cannot open {}", path.display()))?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            bail!("Evidence input must be a regular file");
        }
        if metadata.len() > limits.max_bytes {
            bail!("Input exceeds max_bytes");
        }
        let mut signature = [0; 2];
        let count = file.read(&mut signature)?;
        file.seek(SeekFrom::Start(0))?;
        if count == 2 && signature == *b"PK"
            || path
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("zip"))
        {
            let mut archive = ZipArchive::new(file).context("Invalid ZIP archive")?;
            if archive.has_duplicate_names() {
                bail!("Duplicate ZIP members are not supported");
            }
            if archive.len() > limits.max_events {
                bail!("ZIP member count exceeds max_events");
            }
            let mut names = Vec::new();
            let mut resources = Resources::new();
            for index in 0..archive.len() {
                let entry = archive.by_index(index)?;
                let name = entry.name()?.into_owned();
                if name.len() > 4096
                    || name.contains('\\')
                    || name.contains('\0')
                    || Path::new(&name)
                        .components()
                        .any(|part| !matches!(part, Component::Normal(_)))
                {
                    bail!("Unsafe ZIP member path");
                }
                if entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
                {
                    bail!("ZIP symlinks are not supported");
                }
                if name.starts_with("resources/") {
                    resources.insert(name.clone(), entry.size());
                }
                names.push(name);
            }
            let has_trace = names.iter().any(|name| name.ends_with(".trace"));
            let mut entries: Vec<_> = names
                .into_iter()
                .filter(|name| {
                    name.ends_with(".trace")
                        || !has_trace
                            && !name.starts_with("resources/")
                            && (name.ends_with(".json") || name.ends_with(".jsonl"))
                })
                .collect();
            entries.sort();
            if entries.is_empty() {
                bail!("ZIP contains no trace event entries");
            }
            Ok(Self::Zip {
                archive,
                entries,
                resources,
            })
        } else {
            let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
            if !["trace", "json", "jsonl"]
                .iter()
                .any(|v| extension.eq_ignore_ascii_case(v))
            {
                bail!("Unsupported evidence input; expected .zip, .trace, .json, or .jsonl");
            }
            Ok(Self::Raw(file))
        }
    }

    pub(super) fn scan(
        &mut self,
        limits: &Limits,
        mut visit: impl FnMut(String, &Value, &[u8], Option<&Resources>) -> Result<()>,
    ) -> Result<u64> {
        let mut bytes = 0;
        let mut events = 0;
        let mut read =
            |reader: &mut dyn Read, entry: usize, resources: Option<&Resources>| -> Result<()> {
                let mut frames = Frames::new(reader, limits.max_bytes.saturating_sub(bytes));
                let mut record = 0;
                while let Some(raw) = frames.next()? {
                    if events >= limits.max_events {
                        bail!("Trace exceeds max_events");
                    }
                    let value: Value =
                        serde_json::from_slice(&raw).context("Malformed trace JSON")?;
                    visit(format!("e{entry}-r{record}"), &value, &raw, resources)?;
                    events += 1;
                    record += 1;
                }
                bytes += frames.bytes;
                Ok(())
            };
        match self {
            Self::Raw(file) => read(file, 0, None)?,
            Self::Zip {
                archive,
                entries,
                resources,
            } => {
                for (index, name) in entries.iter().enumerate() {
                    let mut entry = archive.by_name(name)?;
                    read(&mut entry, index, Some(resources))?;
                }
            }
        }
        Ok(bytes)
    }

    pub(super) fn resource(&mut self, name: &str, max_bytes: u64) -> Result<Vec<u8>> {
        let Self::Zip { archive, .. } = self else {
            bail!("Raw trace has no archive resources");
        };
        let mut resource = archive
            .by_name(name)
            .context("Missing screenshot resource")?;
        if resource.size() > max_bytes {
            bail!("Screenshot and scanned trace data exceed max_bytes");
        }
        let mut bytes = Vec::new();
        resource
            .by_ref()
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_bytes {
            bail!("Screenshot exceeds max_bytes");
        }
        Ok(bytes)
    }
}
