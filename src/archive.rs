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

/// Download from the first successful URL, detect the archive extension from the response
/// headers or URL, and write to `dest_dir/{base_name}{ext}`.  Returns the final archive path.
pub fn download_with_fallback(
    client: &Client,
    urls: &[String],
    dest_dir: &Path,
    base_name: &str,
) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir)?;
    let mut last_err: Option<anyhow::Error> = None;
    for url in urls {
        match client.get(url).send() {
            Ok(resp) => match resp.error_for_status() {
                Ok(mut response) => {
                    let content_disposition = response
                        .headers()
                        .get("content-disposition")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let ext = archive_extension(content_disposition.as_deref(), url);
                    let archive_path = dest_dir.join(format!("{base_name}{ext}"));
                    let total = response.content_length();
                    let mut file = File::create(&archive_path)?;
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
                    return Ok(archive_path);
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

/// Determine the archive file extension from a `Content-Disposition` header value or URL.
/// Returns one of `.tar.gz`, `.tgz`, `.tar`, `.zip` (default: `.zip`).
pub fn archive_extension(content_disposition: Option<&str>, url: &str) -> &'static str {
    let filename = content_disposition
        .and_then(|cd| {
            cd.split(';').find_map(|part| {
                let part = part.trim();
                part.strip_prefix("filename=")
                    .or_else(|| part.strip_prefix("filename*=UTF-8''"))
                    .map(|s| s.trim_matches('"').to_ascii_lowercase())
            })
        })
        .unwrap_or_else(|| url.to_ascii_lowercase());

    if filename.contains(".tar.gz") || filename.contains(".tgz") {
        if filename.contains(".tgz") {
            ".tgz"
        } else {
            ".tar.gz"
        }
    } else if filename.contains(".tar") {
        ".tar"
    } else {
        ".zip"
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
    // If bin/ is at the top level, target_dir is already the SDK root.
    if target_dir.join("bin").exists() {
        return Ok(target_dir.to_path_buf());
    }
    // Single nested directory → treat it as the SDK root (typical archive layout).
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        return Ok(entries[0].path());
    }
    for entry in &entries {
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
    use tempfile::TempDir;

    #[test]
    fn progress_bar_renders_start_middle_and_end() {
        assert_eq!(progress_bar(0, 100), "[>---------------------------]");
        assert_eq!(progress_bar(50, 100), "[==============>-------------]");
        assert_eq!(progress_bar(100, 100), "[============================]");
    }

    #[test]
    fn archive_extension_from_content_disposition() {
        assert_eq!(
            archive_extension(Some("attachment; filename=\"jdk-21.tar.gz\""), ""),
            ".tar.gz"
        );
        assert_eq!(
            archive_extension(Some("attachment; filename=sdk.tgz"), ""),
            ".tgz"
        );
        assert_eq!(
            archive_extension(Some("attachment; filename=sdk.zip"), ""),
            ".zip"
        );
        assert_eq!(
            archive_extension(Some("attachment; filename=sdk.tar"), ""),
            ".tar"
        );
    }

    #[test]
    fn archive_extension_falls_back_to_url() {
        assert_eq!(
            archive_extension(None, "https://host/sdk-21.tar.gz"),
            ".tar.gz"
        );
        assert_eq!(archive_extension(None, "https://host/sdk-21.tgz"), ".tgz");
        assert_eq!(archive_extension(None, "https://host/sdk-21.zip"), ".zip");
        assert_eq!(archive_extension(None, "https://host/sdk-21.tar"), ".tar");
        assert_eq!(archive_extension(None, "https://host/sdk-21"), ".zip");
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn make_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut builder = tar::Builder::new(enc);
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, std::io::Cursor::new(data))
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn extract_zip_with_nested_sdk_root() {
        let zip_bytes = make_zip(&[
            ("sdk-21/bin/java.exe", b"fake-java"),
            ("sdk-21/lib/rt.jar", b"fake-rt"),
        ]);
        let tmp = TempDir::new().unwrap();
        let zip_path = tmp.path().join("sdk.zip");
        fs::write(&zip_path, &zip_bytes).unwrap();

        let extract_dir = TempDir::new().unwrap();
        let root = extract(&zip_path, extract_dir.path()).unwrap();

        assert!(root.join("bin").join("java.exe").exists());
        assert!(root.join("lib").join("rt.jar").exists());
    }

    #[test]
    fn extract_tar_gz_with_nested_sdk_root() {
        let tar_bytes = make_tar_gz(&[
            ("sdk-21/bin/mvn", b"fake-mvn"),
            ("sdk-21/lib/plexus.jar", b"fake-jar"),
        ]);
        let tmp = TempDir::new().unwrap();
        let tgz_path = tmp.path().join("sdk.tar.gz");
        fs::write(&tgz_path, &tar_bytes).unwrap();

        let extract_dir = TempDir::new().unwrap();
        let root = extract(&tgz_path, extract_dir.path()).unwrap();

        assert!(root.join("bin").join("mvn").exists());
    }

    #[test]
    fn normalize_root_single_top_level_dir() {
        let tmp = TempDir::new().unwrap();
        let inner = tmp.path().join("jdk-21");
        fs::create_dir(&inner).unwrap();
        fs::create_dir(inner.join("bin")).unwrap();
        let root = normalize_root(tmp.path()).unwrap();
        assert_eq!(root, inner);
    }

    #[test]
    fn normalize_root_bin_at_top() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("bin")).unwrap();
        let root = normalize_root(tmp.path()).unwrap();
        assert_eq!(root, tmp.path());
    }

    #[test]
    fn zip_slip_entries_are_skipped() {
        let zip_bytes = make_zip(&[
            ("safe/file.txt", b"ok"),
            ("../../evil.txt", b"should not exist"),
        ]);
        let tmp = TempDir::new().unwrap();
        let zip_path = tmp.path().join("malicious.zip");
        fs::write(&zip_path, &zip_bytes).unwrap();

        let extract_dir = TempDir::new().unwrap();
        extract_zip(&zip_path, extract_dir.path()).unwrap();

        assert!(extract_dir.path().join("safe").join("file.txt").exists());
        // The traversal entry must not have created a file outside extract_dir
        let evil = extract_dir.path().parent().unwrap().join("evil.txt");
        assert!(!evil.exists());
    }
}
