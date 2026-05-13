use anyhow::Result;
use std::path::Path;

use crate::{api::Api, state::State};

const COMMANDS: &[&str] = &[
    "init",
    "list",
    "ls",
    "install",
    "i",
    "uninstall",
    "rm",
    "use",
    "default",
    "d",
    "current",
    "c",
    "home",
    "h",
    "env",
    "offline",
    "update",
    "upgrade",
    "selfupdate",
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

pub(super) fn complete(state: &State, words: &[String]) -> Result<()> {
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
        "install" | "i" => complete_install(state, words, current),
        "use" | "default" | "d" | "uninstall" | "rm" | "home" | "h" => {
            complete_installed_version_command(state, words, current)
        }
        "list" | "ls" | "current" | "c" => complete_candidate(state, current, false),
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
        .and_then(|w| Path::new(w).file_stem())
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("sdk"))
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
            .map(|c| complete_install_versions(state, c, current))
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
            .map(|c| complete_installed_versions(state, c, current))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn complete_candidate(state: &State, prefix: &str, include_remote: bool) -> Vec<String> {
    let mut candidates = state.installed_candidates().unwrap_or_default();
    if include_remote && !state.config.offline_mode {
        if let Ok(remote) = Api::new(state).and_then(|api| api.candidates(false)) {
            candidates.extend(remote.into_iter().map(|c| c.name));
        }
    }
    candidates.sort();
    candidates.dedup();
    matching_owned(candidates, prefix)
}

fn complete_install_versions(state: &State, candidate: &str, prefix: &str) -> Vec<String> {
    Api::new(state)
        .and_then(|api| api.versions(candidate, state.config.offline_mode))
        .map(|versions| matching_owned(versions.into_iter().map(|v| v.value).collect(), prefix))
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
        3 if words.get(1).is_some_and(|w| w == "set") => matching(CONFIG_KEYS, current),
        _ => Vec::new(),
    }
}

fn matching(values: &[&str], prefix: &str) -> Vec<String> {
    matching_owned(values.iter().map(|v| v.to_string()).collect(), prefix)
}

fn matching_owned(mut values: Vec<String>, prefix: &str) -> Vec<String> {
    values.sort();
    values.dedup();
    values.into_iter().filter(|v| v.starts_with(prefix)).collect()
}
