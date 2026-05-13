use anyhow::{bail, Context, Result};
use std::{fs, io, path::Path, process::Command};

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
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()))
        }
    };

    remove_linkish_with_metadata(path, &metadata)
        .with_context(|| format!("failed to remove {}", path.display()))
}

#[cfg(windows)]
fn remove_linkish_with_metadata(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    let is_reparse_point = metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    let is_directory = metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0;
    if is_reparse_point && is_directory {
        fs::remove_dir(path)?;
    } else if is_reparse_point || metadata.is_file() {
        fs::remove_file(path)?;
    } else {
        bail!("refusing to remove normal directory {}", path.display());
    }
    Ok(())
}

#[cfg(not(windows))]
fn remove_linkish_with_metadata(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)?;
    } else {
        bail!("refusing to remove normal directory {}", path.display());
    }
    Ok(())
}

#[cfg(windows)]
fn create_dir_link(link: &Path, target: &Path) -> Result<()> {
    use std::os::windows::fs::symlink_dir;
    if symlink_dir(target, link).is_ok() {
        return Ok(());
    }
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut message = format!(
            "failed to create link {} -> {}",
            link.display(),
            target.display()
        );
        if !stdout.trim().is_empty() {
            message.push_str(&format!("\nstdout: {}", stdout.trim()));
        }
        if !stderr.trim().is_empty() {
            message.push_str(&format!("\nstderr: {}", stderr.trim()));
        }
        anyhow::bail!(message)
    }
}

#[cfg(not(windows))]
fn create_dir_link(link: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Chain;
    use tempfile::TempDir;

    #[test]
    fn remove_linkish_ignores_missing_path() {
        let temp = TempDir::new().unwrap();

        remove_linkish(&temp.path().join("missing")).unwrap();
    }

    #[test]
    fn remove_linkish_refuses_normal_directory() {
        let temp = TempDir::new().unwrap();
        let normal_dir = temp.path().join("current");
        fs::create_dir(&normal_dir).unwrap();

        let error = remove_linkish(&normal_dir).unwrap_err();

        assert!(normal_dir.exists());
        assert!(Chain::new(error.as_ref()).any(|cause| cause
            .to_string()
            .contains("refusing to remove normal directory")));
    }

    #[test]
    fn remove_linkish_removes_file() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("current");
        fs::write(&file, "").unwrap();

        remove_linkish(&file).unwrap();

        assert!(!file.exists());
    }
}
