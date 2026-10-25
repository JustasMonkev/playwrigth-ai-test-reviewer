use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

#[cfg(unix)]
use nix::sys::signal::{Signal, killpg};
#[cfg(unix)]
use nix::unistd::Pid;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::thread;

pub struct Downloaded {
    pub traces: Vec<(String, PathBuf)>,
    _directory: TempDirectory,
}

#[derive(Deserialize)]
struct ArtifactList {
    total_count: usize,
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize)]
struct Artifact {
    id: u64,
    name: String,
    size_in_bytes: u64,
    expired: bool,
}

pub fn download(
    repo: &str,
    run_id: u64,
    name: &str,
    max_bytes: u64,
    timeout_secs: u64,
) -> Result<Downloaded> {
    validate_repo(repo)?;
    ensure!(
        !name.is_empty() && name.len() <= 1024,
        "artifact name must contain 1 to 1024 bytes"
    );
    let directory = TempDirectory::new()?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let metadata_path = directory.0.join("metadata.json");
    gh(
        &[
            "api".into(),
            format!("repos/{repo}/actions/runs/{run_id}/artifacts"),
            "--method".into(),
            "GET".into(),
            "--raw-field".into(),
            format!("name={name}"),
            "--raw-field".into(),
            "per_page=100".into(),
        ],
        &metadata_path,
        1_048_576,
        deadline,
    )?;
    let listing: ArtifactList = serde_json::from_reader(File::open(&metadata_path)?)
        .context("gh returned invalid artifact metadata")?;
    ensure!(
        listing.total_count <= 100,
        "artifact name matches more than 100 artifacts; select a unique name"
    );
    let mut matches = listing
        .artifacts
        .into_iter()
        .filter(|artifact| artifact.name == name);
    let artifact = matches
        .next()
        .context("no artifact matched the requested name")?;
    ensure!(
        matches.next().is_none(),
        "multiple artifacts have this name; select a unique artifact"
    );
    ensure!(!artifact.expired, "GitHub artifact has expired");
    ensure!(
        artifact.size_in_bytes <= max_bytes,
        "GitHub artifact exceeds --max-bytes"
    );
    let archive_path = directory.0.join("artifact.zip");
    gh(
        &[
            "api".into(),
            format!("repos/{repo}/actions/artifacts/{}/zip", artifact.id),
            "--method".into(),
            "GET".into(),
        ],
        &archive_path,
        max_bytes,
        deadline,
    )?;
    let traces = unpack_traces(&archive_path, &directory.0, max_bytes)?;
    Ok(Downloaded {
        traces,
        _directory: directory,
    })
}

fn validate_repo(repo: &str) -> Result<()> {
    let components: Vec<_> = repo.split('/').collect();
    ensure!(
        components.len() == 2
            && components.iter().all(|part| {
                !part.is_empty()
                    && *part != "."
                    && *part != ".."
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            }),
        "--repo must have the form OWNER/REPO"
    );
    Ok(())
}

fn unpack_traces(
    archive_path: &Path,
    directory: &Path,
    max_bytes: u64,
) -> Result<Vec<(String, PathBuf)>> {
    let mut archive =
        ZipArchive::new(File::open(archive_path)?).context("GitHub artifact is not a valid ZIP")?;
    ensure!(
        archive.len() <= 10_000,
        "GitHub artifact exceeds 10000 ZIP entries"
    );
    let mut traces = Vec::new();
    let mut extracted = 0;
    let mut direct_trace = false;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("cannot read artifact ZIP entry")?;
        let name = entry
            .name()
            .context("invalid artifact ZIP entry name")?
            .into_owned();
        validate_entry(&name, entry.unix_mode())?;
        if entry.is_dir() {
            continue;
        }
        let extension = Path::new(&name).extension().and_then(|ext| ext.to_str());
        if extension == Some("trace") {
            direct_trace = true;
        }
        if !extension.is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) {
            continue;
        }
        ensure!(traces.len() < 100, "GitHub artifact exceeds 100 trace ZIPs");
        let remaining = max_bytes.saturating_sub(extracted);
        ensure!(
            entry.size() <= remaining,
            "extracted artifact exceeds --max-bytes"
        );
        let path = directory.join(format!("trace-{}.zip", traces.len()));
        let mut output = File::create(&path)?;
        extracted += copy_limited(&mut entry, &mut output, remaining)
            .context("extracted artifact exceeds --max-bytes or cannot be read")?;
        traces.push((name, path));
    }
    ensure!(
        !(direct_trace && !traces.is_empty()),
        "artifact mixes trace files with nested ZIPs; upload one format"
    );
    if direct_trace {
        traces.push(("artifact.zip".into(), archive_path.to_owned()));
    }
    ensure!(
        !traces.is_empty(),
        "artifact contains no trace ZIPs or .trace files"
    );
    traces.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(traces)
}

fn validate_entry(name: &str, mode: Option<u32>) -> Result<()> {
    ensure!(
        !name.is_empty()
            && !name.contains(['\\', ':', '\0'])
            && !Path::new(name).components().any(|part| {
                matches!(
                    part,
                    Component::Prefix(_) | Component::RootDir | Component::ParentDir
                )
            }),
        "unsafe path in artifact ZIP"
    );
    if let Some(mode) = mode {
        let kind = mode & 0o170000;
        ensure!(
            kind == 0 || kind == 0o100000 || kind == 0o040000,
            "artifact ZIP contains a symlink or special file"
        );
    }
    Ok(())
}

fn copy_limited(input: &mut impl Read, output: &mut impl Write, limit: u64) -> io::Result<u64> {
    let mut total = 0;
    let mut buffer = [0; 16_384];
    loop {
        let length = input.read(&mut buffer)?;
        if length == 0 {
            return Ok(total);
        }
        if length as u64 > limit.saturating_sub(total) {
            return Err(io::Error::other("byte limit exceeded"));
        }
        output.write_all(&buffer[..length])?;
        total += length as u64;
    }
}

#[cfg(not(unix))]
fn gh(_args: &[String], _output_path: &Path, _max_bytes: u64, _deadline: Instant) -> Result<()> {
    bail!(
        "GitHub artifact downloads are supported on macOS and Linux; download traces manually and use analyze on this platform"
    )
}

#[cfg(unix)]
fn gh(args: &[String], output_path: &Path, max_bytes: u64, deadline: Instant) -> Result<()> {
    ensure!(Instant::now() < deadline, "gh exceeded --timeout-secs");
    let mut output = File::create(output_path)?;
    let child = Command::new("gh")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "cat")
        .env_remove("GH_FORCE_TTY")
        .env_remove("GH_DEBUG")
        .process_group(0)
        .spawn()
        .context("cannot run gh; install GitHub CLI and run gh auth login")?;
    let mut child = ChildGuard {
        group: Some(Pid::from_raw(
            i32::try_from(child.id()).context("gh process ID is out of range")?,
        )),
        child,
    };
    let mut stdout = child
        .child
        .stdout
        .take()
        .context("cannot capture gh stdout")?;
    let mut stderr = child
        .child
        .stderr
        .take()
        .context("cannot capture gh stderr")?;
    let failed = AtomicBool::new(false);
    thread::scope(|scope| {
        let stdout_task = scope.spawn(|| {
            let result = copy_limited(&mut stdout, &mut output, max_bytes);
            if result.is_err() {
                failed.store(true, Ordering::Relaxed);
            }
            result
        });
        let stderr_task = scope.spawn(|| {
            let mut bytes = Vec::new();
            let result = copy_limited(&mut stderr, &mut bytes, 16_384);
            if result.is_err() {
                failed.store(true, Ordering::Relaxed);
            }
            result.map(|_| bytes)
        });
        let status = loop {
            if failed.load(Ordering::Relaxed) {
                child.terminate();
                break child.child.wait().context("cannot reap gh");
            }
            if Instant::now() >= deadline {
                child.terminate();
                let _ = child.child.wait();
                break Err(anyhow::anyhow!("gh exceeded --timeout-secs"));
            }
            match child.child.try_wait() {
                Ok(Some(status)) => {
                    child.terminate();
                    break Ok(status);
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    child.terminate();
                    let _ = child.child.wait();
                    break Err(error).context("cannot wait for gh");
                }
            }
        };
        let stdout_result = stdout_task
            .join()
            .map_err(|_| anyhow::anyhow!("gh stdout reader failed"))?;
        let stderr_result = stderr_task
            .join()
            .map_err(|_| anyhow::anyhow!("gh stderr reader failed"))?;
        let status = status?;
        stdout_result.context("gh response exceeds byte limit or cannot be saved")?;
        let stderr = stderr_result.context("gh stderr exceeds byte limit or cannot be read")?;
        if !status.success() {
            bail!(
                "gh failed ({status}): {}",
                String::from_utf8_lossy(&stderr).trim()
            );
        }
        Ok(())
    })
}

#[cfg(unix)]
struct ChildGuard {
    child: Child,
    group: Option<Pid>,
}

#[cfg(unix)]
impl ChildGuard {
    fn terminate(&mut self) {
        if let Some(group) = self.group.take() {
            let _ = killpg(group, Signal::SIGKILL);
        }
        let _ = self.child.kill();
    }
}

#[cfg(unix)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.terminate();
        let _ = self.child.wait();
    }
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Result<Self> {
        let seed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for attempt in 0..100 {
            let path = std::env::temp_dir()
                .join(format!("pw-review-{}-{seed}-{attempt}", std::process::id()));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).context("cannot create private temporary directory");
                }
            }
        }
        bail!("cannot create a unique temporary directory")
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_archive_names_and_symlinks_are_rejected() {
        for name in [
            "../trace.zip",
            "/trace.zip",
            "dir/../../trace.zip",
            "C:\\trace.zip",
            "dir\\trace.zip",
        ] {
            assert!(validate_entry(name, None).is_err(), "{name}");
        }
        assert!(validate_entry("trace.zip", Some(0o120777)).is_err());
        assert!(validate_entry("test-results/example/trace.zip", Some(0o100644)).is_ok());
    }

    #[test]
    fn bounded_copy_does_not_write_excess_bytes() {
        let mut bytes = Vec::new();
        assert!(copy_limited(&mut &b"12345"[..], &mut bytes, 4).is_err());
        assert!(bytes.len() <= 4);
        let mut bytes = Vec::new();
        assert_eq!(copy_limited(&mut &b"1234"[..], &mut bytes, 4).unwrap(), 4);
        assert_eq!(bytes, b"1234");
    }

    #[test]
    fn repository_cannot_change_api_endpoint() {
        for repo in [
            "../repo",
            "a/../b",
            "a/b?name=x",
            "a/b#x",
            "https://github.com/a/b",
            "a/{repo}",
        ] {
            assert!(validate_repo(repo).is_err(), "{repo}");
        }
        assert!(validate_repo("owner/repo-name_1.0").is_ok());
    }
}
