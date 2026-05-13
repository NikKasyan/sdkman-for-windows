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
