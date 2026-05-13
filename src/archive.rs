use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tar::Archive;
use zip::ZipArchive;

pub fn download_with_fallback(
    client: &Client,
    urls: &Vec<String>,
    archive_path: &Path,
) -> Result<()> {
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut last_err: Option<anyhow::Error> = None;
    for url in urls {
        match client.get(url).send() {
            Ok(resp) => match resp.error_for_status() {
                Ok(mut response) => {
                    let total = response.content_length();
                    let mut file = File::create(archive_path)?;
                    let mut progress = Progress::new("In progress", total, ProgressUnit::Bytes);
                    let mut downloaded = 0;
                    let mut buffer = [0; 64 * 1024];
                    loop {
                        let read = response.read(&mut buffer)?;
                        if read == 0 {
                            break;
                        }
                        file.write_all(&buffer[..read])?;
                        downloaded += read as u64;
                        progress.update(downloaded)?;
                    }
                    progress.finish(downloaded)?;
                    return Ok(());
                }
                Err(e) => {
                    last_err = Some(anyhow::anyhow!("{} -> {}", url, e));
                    continue;
                }
            },
            Err(e) => {
                last_err = Some(anyhow::anyhow!("{} -> {}", url, e));
                continue;
            }
        }
    }
    if let Some(e) = last_err {
        Err(e)
    } else {
        Err(anyhow::anyhow!("no urls to try"))
    }
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
        extract_tar(
            Archive::new(GzDecoder::new(File::open(archive_path)?)),
            target_dir,
        )?;
    } else if file_name.ends_with(".tar") {
        extract_tar(Archive::new(File::open(archive_path)?), target_dir)?;
    } else {
        bail!("unsupported archive type: {}", archive_path.display());
    }
    normalize_root(target_dir)
}

fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut zip = ZipArchive::new(file)?;
    let total = zip.len() as u64;
    let mut progress = Progress::new("Extracting", Some(total), ProgressUnit::Items);
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(path) = entry.enclosed_name().map(|p| target_dir.join(p)) else {
            progress.update(i as u64 + 1)?;
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
        progress.update(i as u64 + 1)?;
    }
    progress.finish(total)?;
    Ok(())
}

fn extract_tar<R: Read>(mut archive: Archive<R>, target_dir: &Path) -> Result<()> {
    let mut progress = Progress::new("Extracting", None, ProgressUnit::Items);
    let mut extracted = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        entry.unpack_in(target_dir)?;
        extracted += 1;
        progress.update(extracted)?;
    }
    progress.finish(extracted)?;
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

#[derive(Clone, Copy)]
enum ProgressUnit {
    Bytes,
    Items,
}

struct Progress {
    label: &'static str,
    total: Option<u64>,
    unit: ProgressUnit,
    last_draw: Instant,
}

impl Progress {
    fn new(label: &'static str, total: Option<u64>, unit: ProgressUnit) -> Self {
        Self {
            label,
            total,
            unit,
            last_draw: Instant::now() - Duration::from_secs(1),
        }
    }

    fn update(&mut self, current: u64) -> Result<()> {
        if self.last_draw.elapsed() < Duration::from_millis(200) {
            return Ok(());
        }
        self.draw(current)?;
        self.last_draw = Instant::now();
        Ok(())
    }

    fn finish(&mut self, current: u64) -> Result<()> {
        self.draw(current)?;
        println!();
        Ok(())
    }

    fn draw(&self, current: u64) -> Result<()> {
        print!("\r{}: {}", self.label, self.render(current));
        io::stdout().flush()?;
        Ok(())
    }

    fn render(&self, current: u64) -> String {
        match (self.unit, self.total) {
            (ProgressUnit::Bytes, Some(total)) if total > 0 => format!(
                "{} {:>6.1}% ({:.1}/{:.1} MB)",
                progress_bar(current, total),
                current as f64 * 100.0 / total as f64,
                bytes_to_mb(current),
                bytes_to_mb(total)
            ),
            (ProgressUnit::Bytes, _) => format!("{:.1} MB", bytes_to_mb(current)),
            (ProgressUnit::Items, Some(total)) if total > 0 => {
                format!(
                    "{:>6.1}% ({current}/{total} files)",
                    current as f64 * 100.0 / total as f64
                )
            }
            (ProgressUnit::Items, _) => format!("{current} files"),
        }
    }
}

fn progress_bar(current: u64, total: u64) -> String {
    const WIDTH: u64 = 28;
    if total == 0 {
        return "[----------------------------]".to_string();
    }
    let filled = ((current.min(total) * WIDTH) + total / 2) / total;
    let mut bar = String::with_capacity((WIDTH + 2) as usize);
    bar.push('[');
    for index in 0..WIDTH {
        if index < filled {
            bar.push('=');
        } else if index == filled && filled < WIDTH {
            bar.push('>');
        } else {
            bar.push('-');
        }
    }
    bar.push(']');
    bar
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar_renders_start_middle_and_end() {
        assert_eq!(progress_bar(0, 100), "[>---------------------------]");
        assert_eq!(progress_bar(50, 100), "[==============>-------------]");
        assert_eq!(progress_bar(100, 100), "[============================]");
    }
}
