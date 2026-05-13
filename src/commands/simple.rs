use anyhow::{bail, Context, Result};
use std::fs;

use crate::{
    api::Api,
    cli::{ConfigAction, FlushTarget, OfflineAction},
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
    Api::new(state)?
        .refresh()
        .context("failed to refresh candidate metadata")?;
    println!("Candidate metadata refreshed.");
    Ok(())
}

pub(super) fn unsupported(command: &str, guidance: &str) -> Result<()> {
    bail!("sdk {command} is not supported yet. {guidance}")
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
