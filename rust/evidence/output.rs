use anyhow::{Context, Result, bail};
use std::path::Path;

#[cfg(unix)]
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use nix::fcntl::{OFlag, open, openat};
    use nix::sys::stat::Mode;
    use std::fs::File;
    use std::io::Write;
    use std::path::Component;

    let name = path.file_name().context("Output must name a new file")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let mut directory = open(
        if path.is_absolute() { "/" } else { "." },
        flags,
        Mode::empty(),
    )?;
    for part in parent.components() {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                directory = openat(&directory, name, flags, Mode::empty())
                    .context("Output parent must be an existing directory without symlinks")?
            }
            _ => bail!("Output parent traversal is not allowed"),
        }
    }
    let fd = openat(
        &directory,
        name,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )
    .context("Output must be a new regular file (no overwrite)")?;
    let mut file = File::from(fd);
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = nix::unistd::unlinkat(&directory, name, nix::unistd::UnlinkatFlags::NoRemoveDir);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn write_new(_path: &Path, _bytes: &[u8]) -> Result<()> {
    bail!("Secure evidence extraction currently requires Unix; listing is portable")
}
