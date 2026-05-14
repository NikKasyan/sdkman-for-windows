use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{self, Write},
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
    let current = env!("CARGO_PKG_VERSION");
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("sdkman-windows/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(state.config.curl_max_time))
        .build()?;

    let url = "https://api.github.com/repos/NikKasyan/sdkman-for-windows/releases/latest";
    let body = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .context("could not reach GitHub releases")?
        .text()
        .context("unexpected response from GitHub releases API")?;
    let json: serde_json::Value =
        serde_json::from_str(&body).context("could not parse GitHub releases API response")?;

    let latest = json["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    if latest.is_empty() {
        bail!("could not determine latest version from GitHub releases API");
    }

    if latest == current {
        println!("SDKMAN for Windows is up to date (v{current}).");
    } else {
        println!("Update available: v{current} -> v{latest}");
        println!();
        println!("Download the latest release and run install.ps1:");
        println!("  https://github.com/NikKasyan/sdkman-for-windows/releases/latest");
    }
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
