use anyhow::{bail, Context, Result};
use clap::CommandFactory;
use reqwest::blocking::Client;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

use crate::{
    api::Api,
    archive,
    cli::{Args, Command, ConfigAction, EnvAction, FlushTarget, OfflineAction},
    config::Config,
    envfile, fslink, shims,
    state::{display_path, session_home_var, InstallRecord, State},
};

#[derive(Serialize)]
struct EnvUpdate {
    set: BTreeMap<String, String>,
    prepend_path: Vec<String>,
    message: String,
}

pub fn execute(args: Args, state: State) -> Result<()> {
    let emit = EmitMode::from_args(args.emit_env, args.emit_cmd);
    let Some(command) = args.command else {
        Args::command().print_help()?;
        println!();
        return Ok(());
    };
    match command {
        Command::Init => init(&state),
        Command::List { candidate } => list(&state, candidate),
        Command::Install {
            candidate,
            version,
            local_path,
        } => install(&state, &candidate, version, local_path),
        Command::Uninstall { candidate, version } => uninstall(&state, &candidate, &version),
        Command::Use { candidate, version } => use_version(&state, &candidate, &version, emit),
        Command::Default { candidate, version } => default_version(&state, &candidate, &version),
        Command::Current { candidate } => current(&state, candidate),
        Command::Home { candidate, version } => home(&state, &candidate, version),
        Command::Env { action } => env_cmd(&state, action, emit),
        Command::Offline { action } => offline(&state, action),
        Command::Update => update(&state),
        Command::Flush { target } => flush(&state, target.unwrap_or(FlushTarget::All)),
        Command::Config { action } => config(&state, action),
        Command::Version => version(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EmitMode {
    None,
    PowerShell,
    Cmd,
}

impl EmitMode {
    fn from_args(emit_env: bool, emit_cmd: bool) -> Self {
        if emit_cmd {
            Self::Cmd
        } else if emit_env {
            Self::PowerShell
        } else {
            Self::None
        }
    }
}

fn init(state: &State) -> Result<()> {
    state.init()?;
    println!(
        "Initialized SDKMAN for Windows at {}",
        display_path(&state.root)
    );
    Ok(())
}

fn list(state: &State, candidate: Option<String>) -> Result<()> {
    state.init()?;
    match candidate {
        None => {
            let api = Api::new(state)?;
            println!("Available Candidates");
            for candidate in api.candidates(state.config.offline_mode)? {
                if candidate.description.is_empty() {
                    println!("{}", candidate.name);
                } else {
                    println!("{:<18} {}", candidate.name, candidate.description);
                }
            }
        }
        Some(candidate) => {
            let installed = state.installed_versions(&candidate)?;
            let current = state.active_home(&candidate, None)?;
            if state.config.offline_mode {
                println!("Offline Mode: only showing installed {candidate} versions");
                for version in installed {
                    print_list_version(state, &candidate, &version, true, current.as_deref())?;
                }
                return Ok(());
            }
            let api = Api::new(state)?;
            println!("Available {candidate} Versions");
            let remote_versions = api.versions(&candidate, false)?;
            let mut printed = BTreeSet::new();
            for version in remote_versions {
                let installed_marker = installed.contains(&version.value);
                print_list_version(
                    state,
                    &candidate,
                    &version.value,
                    installed_marker,
                    current.as_deref(),
                )?;
                printed.insert(version.value);
            }
            for version in installed {
                if printed.insert(version.clone()) {
                    print_list_version(state, &candidate, &version, true, current.as_deref())?;
                }
            }
        }
    }
    Ok(())
}

fn print_list_version(
    state: &State,
    candidate: &str,
    version: &str,
    installed: bool,
    current: Option<&Path>,
) -> Result<()> {
    let installed_marker = if installed { "*" } else { " " };
    let current_marker =
        if installed && installed_version_is_current(state, candidate, version, current)? {
            ">"
        } else {
            " "
        };
    println!("{current_marker} {installed_marker} {version}");
    Ok(())
}

fn installed_version_is_current(
    state: &State,
    candidate: &str,
    version: &str,
    current: Option<&Path>,
) -> Result<bool> {
    let Some(current) = current else {
        return Ok(false);
    };
    let Some(record) = state.install_record(candidate, version)? else {
        return Ok(false);
    };
    Ok(record.path == current)
}

fn install(
    state: &State,
    candidate: &str,
    version: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<()> {
    state.init()?;
    let version = match version {
        Some(version) => version,
        None => Api::new(state)?
            .versions(candidate, state.config.offline_mode)?
            .first()
            .context("no versions available")?
            .value
            .clone(),
    };

    if let Some(local_path) = local_path {
        if !local_path.exists() {
            bail!("local path does not exist: {}", local_path.display());
        }
        let version_dir = state.version_dir(candidate, &version);
        fs::create_dir_all(&version_dir)?;
        let record = InstallRecord {
            candidate: candidate.to_string(),
            version: version.clone(),
            path: fs::canonicalize(local_path)?,
            local: true,
        };
        state.write_record(&record)?;
        println!("Registered {candidate} {version} as local install.");
    } else {
        if state.config.offline_mode {
            bail!("install requires network while offline mode is enabled");
        }
        let api = Api::new(state)?;
        let client = Client::builder().build()?;
        let archive_name = format!("{candidate}-{version}.zip");
        let archive_path = state.archives_dir().join(archive_name);
        println!("Downloading: {candidate} {version}");
        archive::download(
            &client,
            &api.download_url(candidate, &version),
            &archive_path,
        )?;
        let tmp = TempDir::new_in(state.tmp_dir())?;
        println!("Installing: {candidate} {version}");
        let normalized = archive::extract(&archive_path, tmp.path())?;
        let final_dir = state.version_dir(candidate, &version);
        archive::move_normalized(&normalized, &final_dir)?;
        state.write_record(&InstallRecord {
            candidate: candidate.to_string(),
            version: version.clone(),
            path: final_dir,
            local: false,
        })?;
    }

    if should_set_default(&state.config)? {
        default_version(state, candidate, &version)?;
    }
    Ok(())
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

fn uninstall(state: &State, candidate: &str, version: &str) -> Result<()> {
    state.init()?;
    let record = state
        .install_record(candidate, version)?
        .context("version is not installed")?;
    let version_dir = state.version_dir(candidate, version);
    if record.local {
        fs::remove_file(state.record_path(candidate, version)).ok();
        if version_dir.exists() {
            fs::remove_dir(version_dir).ok();
        }
        println!("Deregistered local {candidate} {version}.");
    } else {
        fs::remove_dir_all(&version_dir)?;
        println!("Uninstalled {candidate} {version}.");
    }
    let current = state.current_link(candidate);
    if current.exists() && fs::canonicalize(&current).ok() == fs::canonicalize(&record.path).ok() {
        fslink::remove_linkish(&current).ok();
    }
    shims::regenerate(state)?;
    Ok(())
}

fn use_version(state: &State, candidate: &str, version: &str, emit: EmitMode) -> Result<()> {
    state.init()?;
    let record = state
        .install_record(candidate, version)?
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
        let update = EnvUpdate {
            set,
            prepend_path: if bin.exists() {
                vec![bin.display().to_string()]
            } else {
                Vec::new()
            },
            message: format!("Using {candidate} version {version} in this shell."),
        };
        emit_update(emit, &update)?;
    } else {
        println!("Use the PowerShell wrapper for session switching: sdk use {candidate} {version}");
        println!("Home: {}", display_path(&record.path));
    }
    Ok(())
}

fn default_version(state: &State, candidate: &str, version: &str) -> Result<()> {
    state.init()?;
    let record = state
        .install_record(candidate, version)?
        .context("version is not installed")?;
    fslink::replace_dir_link(&state.current_link(candidate), &record.path)?;
    shims::regenerate(state)?;
    println!("Default {candidate} version set to {version}.");
    Ok(())
}

fn current(state: &State, candidate: Option<String>) -> Result<()> {
    state.init()?;
    if let Some(candidate) = candidate {
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

fn home(state: &State, candidate: &str, version: Option<String>) -> Result<()> {
    state.init()?;
    let home = state
        .active_home(candidate, version.as_deref())?
        .context("version is not installed or active")?;
    println!("{}", display_path(&home));
    Ok(())
}

fn env_cmd(state: &State, action: EnvAction, emit: EmitMode) -> Result<()> {
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
                        format!("{}_HOME", candidate.to_ascii_uppercase().replace('-', "_")),
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
        EmitMode::PowerShell => println!("{}", serde_json::to_string(update)?),
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

fn offline(state: &State, action: OfflineAction) -> Result<()> {
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

fn update(state: &State) -> Result<()> {
    state.init()?;
    if state.config.offline_mode {
        bail!("update requires network while offline mode is enabled");
    }
    Api::new(state)?.refresh()?;
    println!("Candidate metadata refreshed.");
    Ok(())
}

fn flush(state: &State, target: FlushTarget) -> Result<()> {
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

fn config(state: &State, action: Option<ConfigAction>) -> Result<()> {
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

fn version() -> Result<()> {
    println!("SDKMAN for Windows");
    println!("native: {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
