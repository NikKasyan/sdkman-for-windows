use anyhow::{Context, Result};
use std::{fs, path::Path, process::Command};

pub fn replace_dir_link(link: &Path, target: &Path) -> Result<()> {
    if link.exists() {
        remove_linkish(link)?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    create_dir_link(link, target)
}

pub fn remove_linkish(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir(path)
        .or_else(|_| fs::remove_file(path))
        .with_context(|| format!("failed to remove {}", path.display()))
}

#[cfg(windows)]
fn create_dir_link(link: &Path, target: &Path) -> Result<()> {
    use std::os::windows::fs::symlink_dir;
    if symlink_dir(target, link).is_ok() {
        return Ok(());
    }
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "failed to create link {} -> {}",
            link.display(),
            target.display()
        )
    }
}

#[cfg(not(windows))]
fn create_dir_link(link: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}
