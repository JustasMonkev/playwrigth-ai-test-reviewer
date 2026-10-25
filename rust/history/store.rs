use super::{StoredRun, record_count, valid_run_id, validate};
use crate::Limits;
use crate::results::read_opened;
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(super) fn write(store: &Path, run: &StoredRun, limits: &Limits) -> Result<()> {
    let bytes = serde_json::to_vec(run)?;
    if bytes.len() as u64 > limits.max_bytes {
        bail!("History record exceeds max_bytes");
    }
    fs::create_dir_all(store).context("Cannot create history store")?;
    let target = store.join(format!("{}.json", run.run_id));
    let mut temporary_file = None;
    for _ in 0..128 {
        let path = store.join(format!(
            ".pending-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                temporary_file = Some((path, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("Cannot create history temporary file"),
        }
    }
    let (temporary, mut file) =
        temporary_file.context("Too many history temporary-file collisions")?;
    let result = (|| -> Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::hard_link(&temporary, &target)
            .context("Cannot publish history run (run_id may already exist)")?;
        Ok(())
    })();
    drop(file);
    let cleanup = fs::remove_file(&temporary);
    result?;
    cleanup.context("History published but temporary cleanup failed")?;
    Ok(())
}

pub(super) fn read(store: &Path, limits: &Limits) -> Result<Vec<StoredRun>> {
    let entries = match fs::read_dir(store) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("Cannot read history store"),
    };
    let mut runs = Vec::new();
    let mut bytes_used = 0;
    let mut items_used = 0usize;
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.enumerate() {
        if index >= limits.max_events {
            bail!("History directory exceeds max_events");
        }
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let bytes = read_opened(
            open_history(&path)?,
            limits.max_bytes.saturating_sub(bytes_used),
        )?;
        bytes_used += bytes.len() as u64;
        let run: StoredRun = serde_json::from_slice(&bytes).context("Invalid history record")?;
        validate(&run, limits)?;
        if !valid_run_id(&run.run_id)
            || path.file_stem().and_then(|value| value.to_str()) != Some(&run.run_id)
        {
            bail!("History filename disagrees with run_id");
        }
        if !ids.insert(run.run_id.clone()) {
            bail!("Duplicate history run_id");
        }
        items_used = items_used.saturating_add(record_count(&run));
        if items_used > limits.max_events {
            bail!("History exceeds aggregate max_events");
        }
        runs.push(run);
    }
    runs.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    Ok(runs)
}

fn open_history(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .context("Cannot open history entry without following symlinks")?;
    if file.metadata()?.file_type().is_symlink() {
        bail!("History entries must not be symlinks");
    }
    Ok(file)
}
