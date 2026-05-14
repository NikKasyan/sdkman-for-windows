use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{self, Write},
    process::Command,
};

use crate::{
    api::Api,
    cli::{ConfigAction, FlushTarget, OfflineAction, Order},
    state::{display_path, State},
};

pub(super) fn init(state: &State) -> Result<()> {
    state.init()?;
    println!(
        "Initialized SDKMAN for Windows at {}",
        display_path(&state.root)
    );
    Ok(())
}

pub(super) fn current(state: &State, candidate: Option<String>) -> Result<()> {
    state.init()?;
    if let Some(candidate) = candidate {
        super::ensure_candidate_exists(state, &candidate)?;
        match state.active_home(&candidate, None)? {
            Some(home) => println!("Using {candidate} at {}", display_path(&home)),
            None => println!("No {candidate} version is currently in use."),
        }
        return Ok(());
    }
    println!("Using:");
    for candidate in state.installed_candidates()? {
        if let Some(home) = state.active_home(&candidate, None)? {
            println!("{candidate}: {}", display_path(&home));
        }
    }
    Ok(())
}

pub(super) fn home(state: &State, candidate: &str, version: Option<String>) -> Result<()> {
    state.init()?;
    super::ensure_candidate_exists(state, candidate)?;
    let home = state
        .active_home(candidate, version.as_deref())?
        .context("version is not installed or active")?;
    println!("{}", display_path(&home));
    Ok(())
}

pub(super) fn offline(state: &State, action: OfflineAction) -> Result<()> {
    state.init()?;
    let mut cfg = state.config.clone();
    cfg.offline_mode = matches!(action, OfflineAction::Enable);
    cfg.write(&state.config_path())?;
    println!(
        "{}",
        if cfg.offline_mode {
            "Forced offline mode enabled."
        } else {
            "Online mode re-enabled!"
        }
    );
    Ok(())
}

pub(super) fn update(state: &State) -> Result<()> {
    state.init()?;
    if state.config.offline_mode {
        bail!("update requires network while offline mode is enabled");
    }
    let api = Api::new(state)?;
    api.refresh()
        .context("failed to refresh candidate metadata")?;
    println!("Candidate metadata refreshed.");
    if let Some(msg) = api.broadcast() {
        println!();
        println!("==> SDKMAN Broadcast:");
        println!("{msg}");
    }
    Ok(())
}

pub(super) fn upgrade(state: &State, candidate: Option<String>) -> Result<()> {
    state.init()?;
    if state.config.offline_mode {
        bail!("upgrade requires network while offline mode is enabled");
    }

    let candidates: Vec<String> = match candidate {
        Some(c) => {
            super::ensure_candidate_exists(state, &c)?;
            vec![c]
        }
        None => state.installed_candidates()?,
    };

    if candidates.is_empty() {
        println!("No installed candidates to check.");
        return Ok(());
    }

    let api = Api::new(state)?;

    // Collect (candidate, current_version, latest_version) triples.
    let mut upgrades: Vec<(String, String, String)> = Vec::new();
    for candidate in &candidates {
        let Some(current) = current_default_version(state, candidate)? else {
            continue;
        };
        let mut versions = match api.versions(candidate, false) {
            Ok(v) if !v.is_empty() => v,
            _ => continue,
        };
        super::sort_versions_by_vendor_and_version(&mut versions, Order::Desc);
        let latest = &versions[0].value;
        if *latest != current {
            upgrades.push((candidate.clone(), current, latest.clone()));
        }
    }

    if upgrades.is_empty() {
        println!("All installed candidates are up to date.");
        return Ok(());
    }

    println!("Upgrades available:");
    for (candidate, current, latest) in &upgrades {
        println!("  {candidate}: {current} -> {latest}");
    }
    println!();

    for (candidate, _current, latest) in upgrades {
        if !prompt_yes(&format!("Upgrade {candidate} to {latest}?"), &state.config)? {
            println!("Skipping {candidate}.");
            continue;
        }
        if state.install_record(&candidate, &latest)?.is_none() {
            super::install::download_and_register(state, &candidate, &latest)?;
        }
        super::install::default_version(state, &candidate, Some(latest))?;
    }
    Ok(())
}

pub(super) fn selfupdate(state: &State) -> Result<()> {
    if state.config.offline_mode {
        bail!("selfupdate requires network while offline mode is enabled");
    }
    state.init()?;
    let current = env!("CARGO_PKG_VERSION");
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("sdkman-windows/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(state.config.curl_max_time))
        .build()?;

    println!("Checking for updates...");
    let api_url = "https://api.github.com/repos/NikKasyan/sdkman-for-windows/releases/latest";
    let body = client
        .get(api_url)
        .send()
        .and_then(|r| r.error_for_status())
        .context("could not reach GitHub releases")?
        .text()
        .context("unexpected response from GitHub releases API")?;
    let json: serde_json::Value =
        serde_json::from_str(&body).context("could not parse GitHub releases API response")?;

    let tag = json["tag_name"].as_str().unwrap_or("").trim();
    let latest = tag.trim_start_matches('v');

    if latest.is_empty() {
        bail!("could not determine latest version from GitHub releases API");
    }

    if latest == current {
        println!("SDKMAN for Windows is up to date (v{current}).");
        return Ok(());
    }

    println!("Update available: v{current} -> v{latest}");

    if !prompt_yes("Install update now?", &state.config)? {
        println!("Update cancelled.");
        return Ok(());
    }

    let download_url = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|a| {
                a["name"].as_str().map_or(false, |n| {
                    let lower = n.to_ascii_lowercase();
                    lower.ends_with(".zip") && !lower.contains(".sha256")
                })
            })
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!("https://github.com/NikKasyan/sdkman-for-windows/releases/download/{tag}/sdkman-windows-{tag}.zip")
        });

    let stage_dir = state.tmp_dir().join(format!("selfupdate-{latest}"));
    fs::create_dir_all(&stage_dir)?;

    println!("Downloading v{latest}...");
    let archive_path = crate::archive::download_with_fallback(
        &client,
        &[download_url],
        &stage_dir,
        "sdkman-windows",
    )?;

    let extract_dir = stage_dir.join("extracted");
    crate::archive::extract(&archive_path, &extract_dir)?;

    let install_ps1 = extract_dir.join("install.ps1");
    let sdk_exe_new = extract_dir.join("target").join("release").join("sdk.exe");

    if !install_ps1.exists() {
        bail!("install.ps1 not found in release archive");
    }
    if !sdk_exe_new.exists() {
        bail!("sdk.exe not found in release archive");
    }

    fn esc(s: &str) -> String {
        s.replace('\'', "''")
    }

    let script = format!(
        "Start-Sleep -Seconds 2\n\
         & '{}' -SdkExe '{}' -InstallDir '{}' -SkipLocalSdkDiscovery -SkipProfileUpdate -UnblockScripts\n\
         if (Test-Path -LiteralPath '{}') {{ Remove-Item -Recurse -Force -LiteralPath '{}' }}\n",
        esc(&install_ps1.to_string_lossy()),
        esc(&sdk_exe_new.to_string_lossy()),
        esc(&state.root.to_string_lossy()),
        esc(&stage_dir.to_string_lossy()),
        esc(&stage_dir.to_string_lossy()),
    );

    let script_path = stage_dir.join("_update.ps1");
    fs::write(&script_path, &script)?;

    Command::new("powershell")
        .args([
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-NonInteractive",
            "-File",
            script_path.to_str().context("non-UTF-8 script path")?,
        ])
        .spawn()
        .context("failed to spawn update installer")?;

    println!("Installing v{latest} in the background.");
    println!("Open a new terminal in a few moments to use the updated version.");
    Ok(())
}

pub(super) fn flush(state: &State, target: FlushTarget) -> Result<()> {
    state.init()?;
    let targets = match target {
        FlushTarget::Archives => vec![state.archives_dir()],
        FlushTarget::Tmp => vec![state.tmp_dir()],
        FlushTarget::Metadata => vec![state.metadata_dir()],
        FlushTarget::All => vec![state.archives_dir(), state.tmp_dir(), state.metadata_dir()],
    };
    for dir in targets {
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
    }
    println!("Flush complete.");
    Ok(())
}

pub(super) fn config(state: &State, action: Option<ConfigAction>) -> Result<()> {
    state.init()?;
    match action {
        Some(ConfigAction::Set { key, value }) => {
            let mut cfg = state.config.clone();
            cfg.set_key(&key, &value)?;
            cfg.write(&state.config_path())?;
            println!("{key}={value}");
        }
        None => {
            println!("Config: {}", state.config_path().display());
            print!("{}", state.config.to_properties());
        }
    }
    Ok(())
}

pub(super) fn version() -> Result<()> {
    println!("SDKMAN for Windows");
    println!("native: {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

// Returns the version string that the candidate's `current` link resolves to,
// or None if no default is set or the link target can't be matched.
fn current_default_version(state: &State, candidate: &str) -> Result<Option<String>> {
    let link = state.current_link(candidate);
    if !link.exists() {
        return Ok(None);
    }
    for version in state.installed_versions(candidate)? {
        if let Some(record) = state.install_record(candidate, &version)? {
            if super::paths_match(&record.path, &link) {
                return Ok(Some(version));
            }
        }
    }
    Ok(None)
}

fn prompt_yes(question: &str, config: &crate::config::Config) -> Result<bool> {
    if config.auto_answer {
        return Ok(true);
    }
    print!("{question} (Y/n): ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().is_empty() || answer.trim().eq_ignore_ascii_case("y"))
}
