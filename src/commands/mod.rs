use anyhow::{bail, Result};
use clap::CommandFactory;
use std::{fs, path::Path};

use crate::{
    api::{Api, Version},
    cli::{Args, Command, FlushTarget, Order},
    state::State,
};

mod complete;
mod env;
mod install;
mod list;
mod picker;
mod simple;

pub fn execute(args: Args, state: State) -> Result<()> {
    let emit = EmitMode::from_args(args.emit_env, args.emit_cmd);
    let Some(command) = args.command else {
        Args::command().print_help()?;
        println!();
        return Ok(());
    };
    match command {
        Command::Init => simple::init(&state),
        Command::List { candidate, order } => list::list(&state, candidate, order),
        Command::Install {
            candidate,
            version,
            local_path,
        } => install::install(&state, &candidate, version, local_path),
        Command::Uninstall { candidate, version } => install::uninstall(&state, &candidate, version),
        Command::Use { candidate, version } => env::use_version(&state, &candidate, version, emit),
        Command::Default { candidate, version } => install::default_version(&state, &candidate, version),
        Command::Current { candidate } => simple::current(&state, candidate),
        Command::Home { candidate, version } => simple::home(&state, &candidate, version),
        Command::Env { action } => env::env_cmd(&state, action, emit),
        Command::Offline { action } => simple::offline(&state, action),
        Command::Update => simple::update(&state),
        Command::Upgrade => simple::unsupported(
            "upgrade",
            "Automatic SDK upgrades are not implemented yet. Use `sdk install <candidate> <version>` and `sdk default <candidate> <version>` explicitly.",
        ),
        Command::Selfupdate => simple::unsupported(
            "selfupdate",
            "Self-update is not implemented yet. Download a newer release artifact and run install.ps1 again.",
        ),
        Command::Flush { target } => simple::flush(&state, target.unwrap_or(FlushTarget::All)),
        Command::Config { action } => simple::config(&state, action),
        Command::Version => simple::version(),
        Command::Complete { words } => complete::complete(&state, &words),
    }
}

// Shell-emit mode: how env updates reach the calling wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EmitMode {
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

// --- Shared utilities (visible to all child modules) ---

fn ensure_candidate_exists(state: &State, candidate: &str) -> Result<()> {
    let installed = state.installed_candidates()?;
    if installed.iter().any(|c| c.eq_ignore_ascii_case(candidate)) {
        return Ok(());
    }
    if state.config.offline_mode {
        bail!("Unknown candidate: {}", candidate);
    }
    let api = Api::new(state)?;
    let remote = api.candidates(state.config.offline_mode)?;
    if remote
        .iter()
        .any(|c| c.name.eq_ignore_ascii_case(candidate))
    {
        Ok(())
    } else {
        bail!("Unknown candidate: {}", candidate)
    }
}

fn resolve_installed_version(
    state: &State,
    candidate: &str,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(requested) = requested {
        if state.install_record(candidate, requested)?.is_some() {
            return Ok(requested.to_string());
        }
    }
    let versions = state
        .installed_versions(candidate)?
        .into_iter()
        .filter(|v| requested.is_none_or(|req| v.starts_with(req)))
        .map(Version::local)
        .collect::<Vec<_>>();
    match versions.len() {
        0 => match requested {
            Some(req) => bail!("{candidate} {req} is not installed"),
            None => bail!("no installed {candidate} versions"),
        },
        1 => Ok(versions[0].value.clone()),
        _ if state.config.auto_answer => Ok(versions[0].value.clone()),
        _ => picker::select_version(
            state,
            candidate,
            requested.unwrap_or("installed"),
            &versions,
        ),
    }
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
        (Ok(l), Ok(r)) => l == r,
        _ => false,
    }
}

fn sort_versions_by_vendor_and_version(versions: &mut [Version], order: Order) {
    fn parse_semver(s: &str) -> (i64, i64, i64, String) {
        let mut parts = s.splitn(2, '-');
        let main = parts.next().unwrap_or("");
        let suffix = parts.next().unwrap_or("").to_string();
        let nums = main
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<i64>().unwrap_or(0))
            .collect::<Vec<_>>();
        (
            *nums.first().unwrap_or(&0),
            *nums.get(1).unwrap_or(&0),
            *nums.get(2).unwrap_or(&0),
            suffix,
        )
    }

    versions.sort_by(|a, b| {
        let sa = a.display_version.as_deref().unwrap_or(&a.value);
        let sb = b.display_version.as_deref().unwrap_or(&b.value);
        let (am, an, ap, asuf) = parse_semver(sa);
        let (bm, bn, bp, bsuf) = parse_semver(sb);
        match am.cmp(&bm).then(an.cmp(&bn)).then(ap.cmp(&bp)) {
            std::cmp::Ordering::Equal => {
                let va = a.vendor.as_deref().unwrap_or("").to_lowercase();
                let vb = b.vendor.as_deref().unwrap_or("").to_lowercase();
                match va.cmp(&vb) {
                    std::cmp::Ordering::Equal => match asuf.cmp(&bsuf) {
                        std::cmp::Ordering::Equal => sa.cmp(sb),
                        ord => ord,
                    },
                    ord => ord,
                }
            }
            ord => ord,
        }
    });

    if let Order::Desc = order {
        versions.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn paths_match_after_canonicalization() {
        let root = TempDir::new().unwrap();
        let nested = root.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let direct = root.path().join("a").join("b");
        let via_parent = root.path().join("a").join("..").join("a").join("b");
        assert!(paths_match(&direct, &via_parent));
    }

    #[test]
    fn missing_installed_version_resolves_single_installed_version() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("candidates").join("java").join("21-local")).unwrap();
        let state = State {
            root: root.path().to_path_buf(),
            config: Config::default(),
        };
        let version = resolve_installed_version(&state, "java", None).unwrap();
        assert_eq!(version, "21-local");
    }

    #[test]
    fn missing_installed_version_fails_when_none_are_installed() {
        let root = TempDir::new().unwrap();
        let state = State {
            root: root.path().to_path_buf(),
            config: Config::default(),
        };
        let error = resolve_installed_version(&state, "java", None).unwrap_err();
        assert!(error.to_string().contains("no installed java versions"));
    }
}
