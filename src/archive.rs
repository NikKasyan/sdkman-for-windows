use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};
use tar::Archive;
use zip::ZipArchive;

pub fn download(client: &Client, url: &str, archive_path: &Path) -> Result<()> {
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut response = client.get(url).send()?.error_for_status()?;
    let mut file = File::create(archive_path)?;
    io::copy(&mut response, &mut file)?;
    Ok(())
}

pub fn extract(archive_path: &Path, target_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(target_dir)?;
    let file_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if file_name.ends_with(".zip") {
        extract_zip(archive_path, target_dir)?;
    } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        let file = File::open(archive_path)?;
        Archive::new(GzDecoder::new(file)).unpack(target_dir)?;
    } else if file_name.ends_with(".tar") {
        Archive::new(File::open(archive_path)?).unpack(target_dir)?;
    } else {
        bail!("unsupported archive type: {}", archive_path.display());
    }
    normalize_root(target_dir)
}

fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut zip = ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(path) = entry.enclosed_name().map(|p| target_dir.join(p)) else {
            continue;
        };
        if entry.is_dir() {
            fs::create_dir_all(path)?;
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(path)?;
            io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

fn normalize_root(target_dir: &Path) -> Result<PathBuf> {
    let entries = fs::read_dir(target_dir)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        return Ok(entries[0].path());
    }
    if target_dir.join("bin").exists() {
        return Ok(target_dir.to_path_buf());
    }
    for entry in entries {
        let candidate = entry.path();
        if candidate.join("bin").exists() {
            return Ok(candidate);
        }
    }
    Ok(target_dir.to_path_buf())
}

pub fn move_normalized(normalized: &Path, final_dir: &Path) -> Result<()> {
    if final_dir.exists() {
        bail!("{} already exists", final_dir.display());
    }
    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(normalized, final_dir).with_context(|| {
        format!(
            "failed to move {} to {}",
            normalized.display(),
            final_dir.display()
        )
    })?;
    Ok(())
}
