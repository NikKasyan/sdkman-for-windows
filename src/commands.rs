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
    api::{Api, Version},
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
        Command::Complete { words } => complete(&state, &words),
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
            let java_table = candidate.eq_ignore_ascii_case("java")
                && remote_versions
                    .iter()
                    .any(|version| version.vendor.is_some());
            if java_table {
                print_java_table_header();
            }
            let mut printed = BTreeSet::new();
            for version in remote_versions {
                let installed_marker = installed.contains(&version.value);
                print_list_version_or_java_row(
                    state,
                    &candidate,
                    &version,
                    installed_marker,
                    current.as_deref(),
                    java_table,
                )?;
                printed.insert(version.value);
            }
            for version in installed {
                if printed.insert(version.clone()) {
                    let version = Version::local(version);
                    print_list_version_or_java_row(
                        state,
                        &candidate,
                        &version,
                        true,
                        current.as_deref(),
                        java_table,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn print_java_table_header() {
    println!(
        " {:<14} | {:<3} | {:<12} | {:<7} | {:<9} | Identifier",
        "Vendor", "Use", "Version", "Dist", "Status"
    );
    println!("{}", "-".repeat(78));
}

fn print_list_version_or_java_row(
    state: &State,
    candidate: &str,
    version: &Version,
    installed: bool,
    current: Option<&Path>,
    java_table: bool,
) -> Result<()> {
    if java_table {
        print_java_table_row(state, candidate, version, installed, current)
    } else {
        print_list_version(state, candidate, &version.value, installed, current)
    }
}

fn print_java_table_row(
    state: &State,
    candidate: &str,
    version: &Version,
    installed: bool,
    current: Option<&Path>,
) -> Result<()> {
    let use_marker =
        if installed && installed_version_is_current(state, candidate, &version.value, current)? {
            ">"
        } else {
            ""
        };
    let status = if installed { "installed" } else { "" };
    let vendor = version.vendor.as_deref().unwrap_or("");
    let display_version = version.display_version.as_deref().unwrap_or(&version.value);
    let distribution = version.distribution.as_deref().unwrap_or("local");
    println!(
        " {:<14} | {:<3} | {:<12} | {:<7} | {:<9} | {}",
        vendor, use_marker, display_version, distribution, status, version.value
    );
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
    Ok(paths_match(&record.path, current))
}

fn paths_match(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn install(
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
        Some(version) if local_path.is_some() => version,
        Some(version) => resolve_install_version(state, candidate, &version)?,
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

fn resolve_install_version(state: &State, candidate: &str, requested: &str) -> Result<String> {
    let api = Api::new(state)?;
    let versions = api.versions(candidate, state.config.offline_mode)?;
    if versions.iter().any(|version| version.value == requested) {
        return Ok(requested.to_string());
    }

    let matches = versions
        .into_iter()
        .filter(|version| version_matches_prefix(version, requested))
        .collect::<Vec<_>>();
    match matches.len() {
        0 => bail!("no {candidate} version matches '{requested}'"),
        1 => Ok(matches[0].value.clone()),
        _ if state.config.auto_answer => Ok(matches[0].value.clone()),
        _ => select_version(candidate, requested, &matches),
    }
}

fn version_matches_prefix(version: &Version, prefix: &str) -> bool {
    version.value.starts_with(prefix)
        || version
            .display_version
            .as_deref()
            .is_some_and(|display| display.starts_with(prefix))
}

fn select_version(candidate: &str, requested: &str, versions: &[Version]) -> Result<String> {
    println!("Multiple {candidate} versions match '{requested}':");
    for (index, version) in versions.iter().enumerate() {
        println!("{:>3}) {}", index + 1, format_version_choice(version));
    }
    print!("Select version (1-{}): ", versions.len());
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let choice = answer
        .trim()
        .parse::<usize>()
        .context("selection must be a number")?;
    if choice == 0 || choice > versions.len() {
        bail!("selection must be between 1 and {}", versions.len());
    }
    Ok(versions[choice - 1].value.clone())
}

fn format_version_choice(version: &Version) -> String {
    let mut text = version.value.clone();
    if let Some(vendor) = &version.vendor {
        text.push_str("  ");
        text.push_str(vendor);
    }
    if let Some(display_version) = &version.display_version {
        if display_version != &version.value {
            text.push_str("  ");
            text.push_str(display_version);
        }
    }
    text
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
    if current.exists() && paths_match(&current, &record.path) {
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

fn complete(state: &State, words: &[String]) -> Result<()> {
    for item in completions(state, words) {
        println!("{item}");
    }
    Ok(())
}

fn completions(state: &State, words: &[String]) -> Vec<String> {
    let words = trim_command_name(words);
    let command = words.first().map(String::as_str).unwrap_or_default();
    if words.len() <= 1 {
        return matching(COMMANDS, command);
    }

    let current = words.last().map(String::as_str).unwrap_or_default();
    match command {
        "install" => complete_install(state, words, current),
        "use" | "default" | "uninstall" | "rm" | "home" => {
            complete_installed_version_command(state, words, current)
        }
        "list" | "current" => complete_candidate(state, current, false),
        "flush" => matching(&["archives", "tmp", "metadata", "all"], current),
        "offline" => matching(&["enable", "disable"], current),
        "env" => matching(&["init", "install", "clear"], current),
        "config" => complete_config(words, current),
        _ => Vec::new(),
    }
}

fn trim_command_name(words: &[String]) -> &[String] {
    if words
        .first()
        .and_then(|word| Path::new(word).file_stem())
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("sdk"))
    {
        &words[1..]
    } else {
        words
    }
}

fn complete_install(state: &State, words: &[String], current: &str) -> Vec<String> {
    match words.len() {
        2 => complete_candidate(state, current, true),
        3 => words
            .get(1)
            .map(|candidate| complete_install_versions(state, candidate, current))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn complete_installed_version_command(
    state: &State,
    words: &[String],
    current: &str,
) -> Vec<String> {
    match words.len() {
        2 => complete_candidate(state, current, false),
        3 => words
            .get(1)
            .map(|candidate| complete_installed_versions(state, candidate, current))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn complete_candidate(state: &State, prefix: &str, include_remote: bool) -> Vec<String> {
    let mut candidates = state.installed_candidates().unwrap_or_default();
    if include_remote && !state.config.offline_mode {
        if let Ok(remote) = Api::new(state).and_then(|api| api.candidates(false)) {
            candidates.extend(remote.into_iter().map(|candidate| candidate.name));
        }
    }
    candidates.sort();
    candidates.dedup();
    matching_owned(candidates, prefix)
}

fn complete_install_versions(state: &State, candidate: &str, prefix: &str) -> Vec<String> {
    Api::new(state)
        .and_then(|api| api.versions(candidate, state.config.offline_mode))
        .map(|versions| {
            matching_owned(
                versions
                    .into_iter()
                    .map(|version| version.value)
                    .collect::<Vec<_>>(),
                prefix,
            )
        })
        .unwrap_or_else(|_| complete_installed_versions(state, candidate, prefix))
}

fn complete_installed_versions(state: &State, candidate: &str, prefix: &str) -> Vec<String> {
    matching_owned(
        state.installed_versions(candidate).unwrap_or_default(),
        prefix,
    )
}

fn complete_config(words: &[String], current: &str) -> Vec<String> {
    match words.len() {
        2 => matching(&["set"], current),
        3 if words.get(1).is_some_and(|word| word == "set") => matching(CONFIG_KEYS, current),
        _ => Vec::new(),
    }
}

fn matching(values: &[&str], prefix: &str) -> Vec<String> {
    matching_owned(
        values.iter().map(|value| (*value).to_string()).collect(),
        prefix,
    )
}

fn matching_owned(mut values: Vec<String>, prefix: &str) -> Vec<String> {
    values.sort();
    values.dedup();
    values
        .into_iter()
        .filter(|value| value.starts_with(prefix))
        .collect()
}

fn version() -> Result<()> {
    println!("SDKMAN for Windows");
    println!("native: {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

const COMMANDS: &[&str] = &[
    "init",
    "list",
    "install",
    "uninstall",
    "rm",
    "use",
    "default",
    "current",
    "home",
    "env",
    "offline",
    "update",
    "flush",
    "config",
    "version",
];

const CONFIG_KEYS: &[&str] = &[
    "sdkman_auto_answer",
    "sdkman_insecure_ssl",
    "sdkman_curl_connect_timeout",
    "sdkman_curl_max_time",
    "sdkman_colour_enable",
    "sdkman_debug_mode",
    "sdkman_healthcheck_enable",
    "sdkman_auto_env",
    "sdkman_offline_mode",
];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn paths_match_after_canonicalization() {
        let root = TempDir::new().unwrap();
        let nested = root.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();

        let direct = root.path().join("a").join("b");
        let parent_relative = root.path().join("a").join("..").join("a").join("b");

        assert!(paths_match(&direct, &parent_relative));
    }

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
