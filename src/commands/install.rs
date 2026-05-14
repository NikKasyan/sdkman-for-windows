use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};
use tempfile::TempDir;

use crate::{
    api::{Api, Version},
    archive,
    config::Config,
    fslink, shims,
    state::{InstallRecord, State},
};

pub(super) fn install(
    state: &State,
    candidate: &str,
    version: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<()> {
    state.init()?;
    if local_path.is_none() && state.config.offline_mode {
        bail!("install requires network while offline mode is enabled");
    }
    let version = match version {
        Some(v) if local_path.is_some() => v,
        Some(v) => resolve_install_version(state, candidate, &v)?,
        None => select_install_version(state, candidate)?,
    };

    if let Some(local_path) = local_path {
        if !local_path.exists() {
            bail!("local path does not exist: {}", local_path.display());
        }
        let version_dir = state.version_dir(candidate, &version);
        fs::create_dir_all(&version_dir)?;
        state.write_record(&InstallRecord {
            candidate: candidate.to_string(),
            version: version.clone(),
            path: fs::canonicalize(local_path)?,
            local: true,
        })?;
        println!("Registered {candidate} {version} as local install.");
    } else {
        download_and_register(state, candidate, &version)?;
    }

    if should_set_default(&state.config)? {
        default_version(state, candidate, Some(version))?;
    }
    Ok(())
}

pub(super) fn uninstall(state: &State, candidate: &str, version: Option<String>) -> Result<()> {
    state.init()?;
    super::ensure_candidate_exists(state, candidate)?;
    let version = super::resolve_installed_version(state, candidate, version.as_deref())?;
    let record = state
        .install_record(candidate, &version)?
        .context("version is not installed")?;
    let version_dir = state.version_dir(candidate, &version);
    if record.local {
        fs::remove_file(state.record_path(candidate, &version)).ok();
        if version_dir.exists() {
            fs::remove_dir(version_dir).ok();
        }
        println!("Deregistered local {candidate} {version}.");
    } else {
        fs::remove_dir_all(&version_dir)?;
        println!("Uninstalled {candidate} {version}.");
    }
    let current = state.current_link(candidate);
    if current.exists() && super::paths_match(&current, &record.path) {
        fslink::remove_linkish(&current).ok();
    }
    shims::regenerate(state)?;
    Ok(())
}

pub(super) fn default_version(
    state: &State,
    candidate: &str,
    version: Option<String>,
) -> Result<()> {
    state.init()?;
    super::ensure_candidate_exists(state, candidate)?;
    let version = super::resolve_installed_version(state, candidate, version.as_deref())?;
    let record = state
        .install_record(candidate, &version)?
        .context("version is not installed")?;
    fslink::replace_dir_link(&state.current_link(candidate), &record.path)?;
    shims::regenerate(state)?;
    println!("Default {candidate} version set to {version}.");
    Ok(())
}

pub(super) fn download_and_register(state: &State, candidate: &str, version: &str) -> Result<()> {
    let api = Api::new(state)?;
    let base_name = format!("{candidate}-{version}");
    println!("Downloading: {candidate} {version}");
    let urls = api.download_url(candidate, version);
    let archive_path =
        archive::download_with_fallback(api.client(), &urls, &state.archives_dir(), &base_name)
            .with_context(|| format!("failed to download {candidate} {version}"))?;
    let tmp = TempDir::new_in(state.tmp_dir())?;
    println!("Installing: {candidate} {version}");
    let normalized = match archive::extract(&archive_path, tmp.path()) {
        Ok(path) => path,
        Err(e) => {
            let _ = fs::remove_file(&archive_path);
            return Err(e).with_context(|| format!("failed to extract {candidate} {version}"));
        }
    };
    let final_dir = state.version_dir(candidate, version);
    if let Err(e) = archive::move_normalized(&normalized, &final_dir) {
        let _ = fs::remove_file(&archive_path);
        if final_dir.exists() {
            let _ = fs::remove_dir_all(&final_dir);
        }
        return Err(e);
    }
    state.write_record(&InstallRecord {
        candidate: candidate.to_string(),
        version: version.to_string(),
        path: final_dir,
        local: false,
    })?;
    Ok(())
}

fn select_install_version(state: &State, candidate: &str) -> Result<String> {
    let api = Api::new(state)?;
    let versions = api.versions(candidate, state.config.offline_mode)?;
    if versions.is_empty() {
        bail!("no versions available");
    }
    if state.config.auto_answer {
        return Ok(versions[0].value.clone());
    }
    super::picker::select_version(state, candidate, "all", &versions)
}

fn resolve_install_version(state: &State, candidate: &str, requested: &str) -> Result<String> {
    let api = Api::new(state)?;
    let versions = api.versions(candidate, state.config.offline_mode)?;
    if versions.iter().any(|v| v.value == requested) {
        return Ok(requested.to_string());
    }
    let matches = versions
        .into_iter()
        .filter(|v| version_matches_prefix(v, requested))
        .collect::<Vec<_>>();
    match matches.len() {
        0 => bail!("no {candidate} version matches '{requested}'"),
        1 => Ok(matches[0].value.clone()),
        _ if state.config.auto_answer => Ok(matches[0].value.clone()),
        _ => super::picker::select_version(state, candidate, requested, &matches),
    }
}

fn version_matches_prefix(version: &Version, prefix: &str) -> bool {
    version.value.starts_with(prefix)
        || version
            .display_version
            .as_deref()
            .is_some_and(|d| d.starts_with(prefix))
}

fn should_set_default(config: &Config) -> Result<bool> {
    if config.auto_answer {
        return Ok(true);
    }
    print!("Set as default? (Y/n): ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().is_empty() || answer.trim().eq_ignore_ascii_case("y"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, state::State};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn partial_install_version_resolves_from_cached_versions() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("var").join("metadata")).unwrap();
        fs::write(
            root.path()
                .join("var")
                .join("metadata")
                .join("java-versions.txt"),
            "25.0.3-tem,21.0.11-tem",
        )
        .unwrap();
        let state = State {
            root: root.path().to_path_buf(),
            config: Config {
                offline_mode: true,
                ..Config::default()
            },
        };
        let version = resolve_install_version(&state, "java", "25").unwrap();
        assert_eq!(version, "25.0.3-tem");
    }
}
