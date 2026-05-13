use anyhow::{Context, Result};
use serde::Serialize;
use std::{collections::BTreeMap, env, fs};

use crate::{
    cli::EnvAction,
    envfile,
    state::{display_path, session_home_var, State},
};

use super::EmitMode;

#[derive(Serialize)]
pub(super) struct EnvUpdate {
    set: BTreeMap<String, String>,
    prepend_path: Vec<String>,
    message: String,
}

const ENV_JSON_PREFIX: &str = "__SDKMAN_ENV_JSON__";

pub(super) fn use_version(
    state: &State,
    candidate: &str,
    version: Option<String>,
    emit: EmitMode,
) -> Result<()> {
    state.init()?;
    super::ensure_candidate_exists(state, candidate)?;
    let version = super::resolve_installed_version(state, candidate, version.as_deref())?;
    let record = state
        .install_record(candidate, &version)?
        .context("version is not installed")?;
    let bin = record.path.join("bin");
    if emit != EmitMode::None {
        let mut set = BTreeMap::new();
        set.insert(
            session_home_var(candidate),
            record.path.display().to_string(),
        );
        set.insert(
            format!("{}_HOME", candidate.to_ascii_uppercase().replace('-', "_")),
            record.path.display().to_string(),
        );
        emit_update(
            emit,
            &EnvUpdate {
                set,
                prepend_path: if bin.exists() {
                    vec![bin.display().to_string()]
                } else {
                    Vec::new()
                },
                message: format!("Using {candidate} version {version} in this shell."),
            },
        )?;
    } else {
        println!("Use the PowerShell wrapper for session switching: sdk use {candidate} {version}");
        println!("Home: {}", display_path(&record.path));
    }
    Ok(())
}

pub(super) fn env_cmd(state: &State, action: EnvAction, emit: EmitMode) -> Result<()> {
    state.init()?;
    let rc = env::current_dir()?.join(".sdkmanrc");
    match action {
        EnvAction::Init => {
            if rc.exists() {
                println!(".sdkmanrc already exists");
            } else {
                fs::write(
                    &rc,
                    "# Add candidate versions, for example:\n# java=21.0.4-tem\n",
                )?;
                println!("Created {}", rc.display());
            }
        }
        EnvAction::Clear => {
            if rc.exists() {
                fs::remove_file(&rc)?;
            }
            println!("Removed {}", rc.display());
        }
        EnvAction::Install => {
            let values = envfile::parse(&rc)?;
            let mut update = EnvUpdate {
                set: BTreeMap::new(),
                prepend_path: Vec::new(),
                message: "Applied .sdkmanrc".to_string(),
            };
            for (candidate, version) in values {
                let record = state
                    .install_record(&candidate, &version)?
                    .with_context(|| format!("{candidate} {version} is not installed"))?;
                if emit != EmitMode::None {
                    update.set.insert(
                        session_home_var(&candidate),
                        record.path.display().to_string(),
                    );
                    update.set.insert(
                        format!(
                            "{}_HOME",
                            candidate.to_ascii_uppercase().replace('-', "_")
                        ),
                        record.path.display().to_string(),
                    );
                    let bin = record.path.join("bin");
                    if bin.exists() {
                        update.prepend_path.push(bin.display().to_string());
                    }
                } else {
                    println!("{candidate}={version} -> {}", display_path(&record.path));
                }
            }
            if emit != EmitMode::None {
                emit_update(emit, &update)?;
            }
        }
    }
    Ok(())
}

fn emit_update(mode: EmitMode, update: &EnvUpdate) -> Result<()> {
    match mode {
        EmitMode::None => {}
        EmitMode::PowerShell => println!("{ENV_JSON_PREFIX}{}", serde_json::to_string(update)?),
        EmitMode::Cmd => {
            for (key, value) in &update.set {
                println!("set \"{}={}\"", key, value);
            }
            for path in update.prepend_path.iter().rev() {
                println!("set \"PATH={};%PATH%\"", path);
            }
            if !update.message.is_empty() {
                println!("echo {}", update.message);
            }
        }
    }
    Ok(())
}
