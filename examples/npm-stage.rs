use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde_json::{Map, Value, json};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    about = "Stage npm packages from already-built pw-review binaries; never overwrite output"
)]
struct Args {
    output: PathBuf,
    #[arg(required = true, value_name = "TARGET=BINARY")]
    binaries: Vec<String>,
}

struct Platform {
    target: &'static str,
    os: &'static str,
    cpu: &'static str,
    libc: Option<&'static str>,
}

const PLATFORMS: &[Platform] = &[
    Platform {
        target: "aarch64-apple-darwin",
        os: "darwin",
        cpu: "arm64",
        libc: None,
    },
    Platform {
        target: "x86_64-apple-darwin",
        os: "darwin",
        cpu: "x64",
        libc: None,
    },
    Platform {
        target: "aarch64-unknown-linux-gnu",
        os: "linux",
        cpu: "arm64",
        libc: Some("glibc"),
    },
    Platform {
        target: "x86_64-unknown-linux-gnu",
        os: "linux",
        cpu: "x64",
        libc: Some("glibc"),
    },
];

impl Platform {
    fn package(&self) -> String {
        format!(
            "{NAME}-{}-{}{}",
            self.os,
            self.cpu,
            if self.libc.is_some() { "-gnu" } else { "" }
        )
    }

    fn is_host(&self) -> bool {
        self.os
            == match std::env::consts::OS {
                "macos" => "darwin",
                other => other,
            }
            && self.cpu
                == match std::env::consts::ARCH {
                    "aarch64" => "arm64",
                    "x86_64" => "x64",
                    other => other,
                }
            && (self.libc.is_none() || cfg!(target_env = "gnu"))
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut binaries = Vec::new();
    for input in &args.binaries {
        let (target, path) = input.split_once('=').context("expected TARGET=BINARY")?;
        let platform = PLATFORMS
            .iter()
            .find(|platform| platform.target == target)
            .with_context(|| format!("unsupported Rust target: {target}"))?;
        ensure!(
            !binaries
                .iter()
                .any(|(previous, _): &(&Platform, PathBuf)| previous.target == target),
            "duplicate target: {target}"
        );
        let path = PathBuf::from(path);
        validate_binary(platform, &path)?;
        binaries.push((platform, path));
    }
    fs::create_dir(&args.output).with_context(|| {
        format!(
            "cannot create new output directory {}",
            args.output.display()
        )
    })?;
    if let Err(error) = stage(&args.output, &binaries) {
        fs::remove_dir_all(&args.output).context("cannot remove incomplete staging directory")?;
        return Err(error);
    }
    println!("Staged {NAME}@{VERSION} in {}", args.output.display());
    Ok(())
}

fn validate_binary(platform: &Platform, path: &Path) -> Result<()> {
    ensure!(
        fs::metadata(path)
            .with_context(|| format!("cannot read {}", path.display()))?
            .is_file(),
        "binary must be a regular file: {}",
        path.display()
    );
    let mut header = [0_u8; 20];
    fs::File::open(path)?.read_exact(&mut header)?;
    let valid = match platform.os {
        "darwin" => {
            let cpu: u32 = if platform.cpu == "arm64" {
                0x0100_000c
            } else {
                0x0100_0007
            };
            header[..4] == [0xcf, 0xfa, 0xed, 0xfe]
                && header[4..8] == cpu.to_le_bytes()
                && header[12..16] == 2_u32.to_le_bytes()
        }
        "linux" => {
            let machine: u16 = if platform.cpu == "arm64" { 183 } else { 62 };
            header[..6] == [0x7f, b'E', b'L', b'F', 2, 1]
                && matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
                && header[18..20] == machine.to_le_bytes()
        }
        _ => false,
    };
    ensure!(valid, "binary header does not match {}", platform.target);
    if platform.is_host() {
        let output = Command::new(path.canonicalize()?)
            .arg("--version")
            .output()
            .context("cannot run host binary --version")?;
        ensure!(
            output.status.success() && output.stdout == format!("pw-review {VERSION}\n").as_bytes(),
            "host binary must report pw-review {VERSION}"
        );
    }
    Ok(())
}

fn manifest(name: &str) -> Value {
    json!({
        "name": name,
        "version": VERSION,
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "license": env!("CARGO_PKG_LICENSE"),
        "repository": {"type":"git", "url":env!("CARGO_PKG_REPOSITORY")},
        "engines": {"node":">=22", "npm":">=11"}
    })
}

fn write_manifest(directory: &Path, manifest: &Value) -> Result<()> {
    fs::write(
        directory.join("package.json"),
        format!("{}\n", serde_json::to_string_pretty(manifest)?),
    )?;
    fs::write(directory.join("LICENSE"), include_str!("../LICENSE"))?;
    Ok(())
}

fn stage(output: &Path, binaries: &[(&Platform, PathBuf)]) -> Result<()> {
    let mut dependencies = Map::new();
    let mut platforms = Vec::new();
    for (platform, binary) in binaries {
        let name = platform.package();
        let directory = output.join(&name);
        fs::create_dir(&directory)?;
        fs::create_dir(directory.join("bin"))?;
        let destination = directory.join("bin/pw-review");
        fs::copy(binary, &destination)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(destination, fs::Permissions::from_mode(0o755))?;
        }
        let mut native = manifest(&name);
        native["os"] = json!([platform.os]);
        native["cpu"] = json!([platform.cpu]);
        if let Some(libc) = platform.libc {
            native["libc"] = json!([libc]);
        }
        native["files"] = json!(["bin/pw-review", "LICENSE"]);
        write_manifest(&directory, &native)?;
        dependencies.insert(name.clone(), json!(VERSION));
        let mut entry = json!({"os":platform.os, "cpu":platform.cpu, "package":name});
        if let Some(libc) = platform.libc {
            entry["libc"] = json!(libc);
        }
        platforms.push(entry);
    }
    let directory = output.join(NAME);
    fs::create_dir(&directory)?;
    fs::write(
        directory.join("pw-review.cjs"),
        include_str!("../npm/pw-review.cjs"),
    )?;
    fs::write(
        directory.join("README.md"),
        include_str!("../npm/README.md"),
    )?;
    let mut wrapper = manifest(NAME);
    wrapper["bin"] = json!({"pw-review":"pw-review.cjs"});
    wrapper["files"] = json!(["pw-review.cjs", "README.md", "LICENSE"]);
    wrapper["optionalDependencies"] = json!(dependencies);
    wrapper["pwReviewPlatforms"] = json!(platforms);
    write_manifest(&directory, &wrapper)
}
