use anyhow::Result;
use std::{collections::BTreeSet, fs, path::Path};

use crate::state::State;

pub fn regenerate(state: &State) -> Result<()> {
    fs::create_dir_all(state.shims_dir())?;
    clear_existing(&state.shims_dir())?;
    for candidate in state.installed_candidates()? {
        let current = state.current_link(&candidate);
        let bin = current.join("bin");
        if bin.exists() {
            generate_for_bin(&state.shims_dir(), &bin)?;
        }
    }
    Ok(())
}

fn clear_existing(shim_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(shim_dir)? {
        let entry = entry?;
        let path = entry.path();
        if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("cmd" | "ps1")
        ) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn generate_for_bin(shim_dir: &Path, bin: &Path) -> Result<()> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(bin)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "exe" | "cmd" | "bat" | "ps1") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if names.insert(stem.clone()) {
            write_cmd_shim(shim_dir, &stem, &path)?;
            write_ps1_shim(shim_dir, &stem, &path)?;
        }
    }
    Ok(())
}

fn write_cmd_shim(shim_dir: &Path, name: &str, target: &Path) -> Result<()> {
    let text = format!("@echo off\r\n\"{}\" %*\r\n", target.display());
    fs::write(shim_dir.join(format!("{name}.cmd")), text)?;
    Ok(())
}

fn write_ps1_shim(shim_dir: &Path, name: &str, target: &Path) -> Result<()> {
    let escaped = target.display().to_string().replace('\'', "''");
    let text = format!("& '{escaped}' @args\nexit $LASTEXITCODE\n");
    fs::write(shim_dir.join(format!("{name}.ps1")), text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generates_cmd_and_powershell_shims_for_supported_files() {
        let temp = TempDir::new().unwrap();
        let bin = temp.path().join("bin");
        let shims = temp.path().join("shims");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&shims).unwrap();

        for name in ["java.exe", "mvn.cmd", "gradle.bat", "tool.ps1"] {
            fs::write(bin.join(name), "").unwrap();
        }

        generate_for_bin(&shims, &bin).unwrap();

        for name in ["java", "mvn", "gradle", "tool"] {
            assert!(shims.join(format!("{name}.cmd")).exists());
            assert!(shims.join(format!("{name}.ps1")).exists());
        }
    }

    #[test]
    fn ignores_unsupported_files() {
        let temp = TempDir::new().unwrap();
        let bin = temp.path().join("bin");
        let shims = temp.path().join("shims");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&shims).unwrap();
        fs::write(bin.join("notes.txt"), "").unwrap();
        fs::write(bin.join("plain"), "").unwrap();

        generate_for_bin(&shims, &bin).unwrap();

        assert!(fs::read_dir(&shims).unwrap().next().is_none());
    }

    #[test]
    fn duplicate_stems_create_one_shim_pair() {
        let temp = TempDir::new().unwrap();
        let bin = temp.path().join("bin");
        let shims = temp.path().join("shims");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&shims).unwrap();
        fs::write(bin.join("java.exe"), "").unwrap();
        fs::write(bin.join("java.cmd"), "").unwrap();

        generate_for_bin(&shims, &bin).unwrap();

        let shim_names = fs::read_dir(&shims)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(shim_names.len(), 2);
        assert!(shim_names.contains(&"java.cmd".to_string()));
        assert!(shim_names.contains(&"java.ps1".to_string()));
    }
}
