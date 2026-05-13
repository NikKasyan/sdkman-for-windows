use crate::config::Config;
use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct State {
    pub root: PathBuf,
    pub config: Config,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallRecord {
    pub candidate: String,
    pub version: String,
    pub path: PathBuf,
    pub local: bool,
}

impl State {
    pub fn load() -> Result<Self> {
        let root = match env::var_os("SDKMAN_WINDOWS_DIR") {
            Some(value) => PathBuf::from(value),
            None => BaseDirs::new()
                .context("could not determine user profile directory")?
                .home_dir()
                .join(".sdkman-windows"),
        };
        let config = Config::read(&root.join("etc").join("config"))?;
        Ok(Self { root, config })
    }

    pub fn init(&self) -> Result<()> {
        for path in [
            self.candidates_dir(),
            self.archives_dir(),
            self.tmp_dir(),
            self.var_dir(),
            self.etc_dir(),
            self.shims_dir(),
            self.bin_dir(),
        ] {
            fs::create_dir_all(path)?;
        }
        let config_path = self.config_path();
        if !config_path.exists() {
            self.config.write(&config_path)?;
        }
        Ok(())
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub fn candidates_dir(&self) -> PathBuf {
        self.root.join("candidates")
    }
    pub fn archives_dir(&self) -> PathBuf {
        self.root.join("archives")
    }
    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }
    pub fn var_dir(&self) -> PathBuf {
        self.root.join("var")
    }
    pub fn etc_dir(&self) -> PathBuf {
        self.root.join("etc")
    }
    pub fn shims_dir(&self) -> PathBuf {
        self.root.join("shims")
    }
    pub fn metadata_dir(&self) -> PathBuf {
        self.var_dir().join("metadata")
    }
    pub fn config_path(&self) -> PathBuf {
        self.etc_dir().join("config")
    }

    pub fn candidate_dir(&self, candidate: &str) -> PathBuf {
        self.candidates_dir().join(candidate)
    }

    pub fn version_dir(&self, candidate: &str, version: &str) -> PathBuf {
        self.candidate_dir(candidate).join(version)
    }

    pub fn current_link(&self, candidate: &str) -> PathBuf {
        self.candidate_dir(candidate).join("current")
    }

    pub fn record_path(&self, candidate: &str, version: &str) -> PathBuf {
        self.version_dir(candidate, version)
            .join(".sdkman-windows.json")
    }

    pub fn install_record(&self, candidate: &str, version: &str) -> Result<Option<InstallRecord>> {
        let path = self.record_path(candidate, version);
        if !path.exists() {
            let version_dir = self.version_dir(candidate, version);
            if version_dir.exists() {
                return Ok(Some(InstallRecord {
                    candidate: candidate.to_string(),
                    version: version.to_string(),
                    path: version_dir,
                    local: false,
                }));
            }
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }

    pub fn write_record(&self, record: &InstallRecord) -> Result<()> {
        let path = self.record_path(&record.candidate, &record.version);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(record)?)?;
        Ok(())
    }

    pub fn installed_versions(&self, candidate: &str) -> Result<Vec<String>> {
        let dir = self.candidate_dir(candidate);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut versions = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy() == "current" {
                continue;
            }
            if entry.file_type()?.is_dir() {
                versions.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        versions.sort();
        Ok(versions)
    }

    pub fn installed_candidates(&self) -> Result<Vec<String>> {
        let dir = self.candidates_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut candidates = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                candidates.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        candidates.sort();
        Ok(candidates)
    }

    pub fn active_home(&self, candidate: &str, version: Option<&str>) -> Result<Option<PathBuf>> {
        if let Some(version) = version {
            return Ok(self.install_record(candidate, version)?.map(|r| r.path));
        }

        let sdkman_session_home = env::var_os(session_home_var(candidate)).map(PathBuf::from);
        let conventional_home = conventional_home(candidate);
        let env_home = sdkman_session_home.clone().or(conventional_home);
        let link = self.current_link(candidate);
        let current_home = link.exists().then(|| resolve_linkish(&link));

        if sdkman_session_home.is_some() {
            return Ok(env_home);
        }

        if let Some(current_home) = current_home {
            if current_link_wins_path(self, &current_home, env_home.as_deref()) {
                return Ok(Some(current_home));
            }
            if let Some(env_home) = env_home {
                return Ok(Some(env_home));
            }
            return Ok(Some(current_home));
        }

        Ok(env_home)
    }
}

pub fn session_home_var(candidate: &str) -> String {
    format!(
        "SDKMAN_{}_HOME",
        candidate.to_ascii_uppercase().replace('-', "_")
    )
}

fn conventional_home_var(candidate: &str) -> Option<String> {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return None;
    }
    Some(format!(
        "{}_HOME",
        candidate.to_ascii_uppercase().replace('-', "_")
    ))
}

fn conventional_home(candidate: &str) -> Option<PathBuf> {
    if let Some(home_var) = conventional_home_var(candidate) {
        if let Some(session) = env::var_os(home_var) {
            return Some(PathBuf::from(session));
        }
    }
    None
}

fn current_link_wins_path(state: &State, current_home: &Path, env_home: Option<&Path>) -> bool {
    let Some(env_home) = env_home else {
        return true;
    };
    let Some(current_rank) =
        path_rank(&state.shims_dir()).or_else(|| path_rank(&current_home.join("bin")))
    else {
        return false;
    };
    let env_rank = path_rank(&env_home.join("bin")).unwrap_or(usize::MAX);
    current_rank <= env_rank
}

fn path_rank(path: &Path) -> Option<usize> {
    env::var_os("PATH")
        .map(|path_value| {
            env::split_paths(&path_value)
                .enumerate()
                .find_map(|(index, entry)| paths_match(&entry, path).then_some(index))
        })
        .unwrap_or(None)
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

pub fn display_path(path: &Path) -> String {
    strip_windows_verbatim_prefix(&path.display().to_string())
}

fn strip_windows_verbatim_prefix(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{stripped}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

fn resolve_linkish(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[test]
    fn display_path_strips_windows_verbatim_drive_prefix() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\C:\Users\example\.sdkman-windows"),
            r"C:\Users\example\.sdkman-windows"
        );
    }

    #[test]
    fn display_path_strips_windows_verbatim_unc_prefix() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\UNC\server\share\.sdkman-windows"),
            r"\\server\share\.sdkman-windows"
        );
    }

    #[test]
    fn display_path_leaves_normal_paths_unchanged() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"C:\Users\example\.sdkman-windows"),
            r"C:\Users\example\.sdkman-windows"
        );
    }

    #[test]
    fn conventional_home_var_uses_candidate_home_name() {
        assert_eq!(conventional_home_var("java").as_deref(), Some("JAVA_HOME"));
        assert_eq!(
            conventional_home_var("springboot").as_deref(),
            Some("SPRINGBOOT_HOME")
        );
        assert_eq!(
            conventional_home_var("visualvm").as_deref(),
            Some("VISUALVM_HOME")
        );
    }

    #[test]
    fn active_home_falls_back_to_conventional_home_variable() {
        let _guard = env_lock();
        let root = tempfile::TempDir::new().unwrap();
        let home = root.path().join("sample-home");
        let state = State {
            root: root.path().join("sdkman"),
            config: Config::default(),
        };

        env::remove_var("SDKMAN_SAMPLE_HOME");
        env::set_var("SAMPLE_HOME", &home);
        let active = state.active_home("sample", None).unwrap();
        env::remove_var("SAMPLE_HOME");

        assert_eq!(active, Some(home));
    }

    #[test]
    fn active_home_prefers_sdkman_session_home_over_conventional_home() {
        let _guard = env_lock();
        let root = tempfile::TempDir::new().unwrap();
        let conventional = root.path().join("conventional-home");
        let sdkman = root.path().join("sdkman-home");
        let state = State {
            root: root.path().join("sdkman"),
            config: Config::default(),
        };

        env::set_var("SAMPLE_HOME", &conventional);
        env::set_var("SDKMAN_SAMPLE_HOME", &sdkman);
        let active = state.active_home("sample", None).unwrap();
        env::remove_var("SAMPLE_HOME");
        env::remove_var("SDKMAN_SAMPLE_HOME");

        assert_eq!(active, Some(sdkman));
    }

    #[test]
    fn active_home_prefers_sdkman_session_home_even_when_shims_win_path() {
        let _guard = env_lock();
        let root = tempfile::TempDir::new().unwrap();
        let state = State {
            root: root.path().join("sdkman"),
            config: Config::default(),
        };
        let current = state.current_link("sample");
        let session_home = root.path().join("session-home");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(state.shims_dir()).unwrap();
        let previous_path = env::var_os("PATH");

        env::set_var("PATH", state.shims_dir());
        env::remove_var("SAMPLE_HOME");
        env::set_var("SDKMAN_SAMPLE_HOME", &session_home);
        let active = state.active_home("sample", None).unwrap();
        env::remove_var("SDKMAN_SAMPLE_HOME");
        restore_path(previous_path);

        assert_eq!(active, Some(session_home));
    }

    #[test]
    fn active_home_prefers_current_link_when_shims_win_path() {
        let _guard = env_lock();
        let root = tempfile::TempDir::new().unwrap();
        let state = State {
            root: root.path().join("sdkman"),
            config: Config::default(),
        };
        let current = state.current_link("sample");
        let stale_home = root.path().join("stale-home");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(stale_home.join("bin")).unwrap();
        fs::create_dir_all(state.shims_dir()).unwrap();
        let previous_path = env::var_os("PATH");

        env::set_var(
            "PATH",
            env::join_paths([state.shims_dir(), stale_home.join("bin")]).unwrap(),
        );
        env::set_var("SAMPLE_HOME", &stale_home);
        env::remove_var("SDKMAN_SAMPLE_HOME");
        let active = state.active_home("sample", None).unwrap();
        env::remove_var("SAMPLE_HOME");
        restore_path(previous_path);

        assert!(paths_match(&active.unwrap(), &current));
    }

    #[test]
    fn active_home_prefers_env_home_when_env_bin_wins_path() {
        let _guard = env_lock();
        let root = tempfile::TempDir::new().unwrap();
        let state = State {
            root: root.path().join("sdkman"),
            config: Config::default(),
        };
        let current = state.current_link("sample");
        let session_home = root.path().join("session-home");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(session_home.join("bin")).unwrap();
        fs::create_dir_all(state.shims_dir()).unwrap();
        let previous_path = env::var_os("PATH");

        env::set_var(
            "PATH",
            env::join_paths([session_home.join("bin"), state.shims_dir()]).unwrap(),
        );
        env::set_var("SAMPLE_HOME", &session_home);
        env::remove_var("SDKMAN_SAMPLE_HOME");
        let active = state.active_home("sample", None).unwrap();
        env::remove_var("SAMPLE_HOME");
        restore_path(previous_path);

        assert!(paths_match(&active.unwrap(), &session_home));
    }

    fn restore_path(previous_path: Option<std::ffi::OsString>) {
        if let Some(previous_path) = previous_path {
            env::set_var("PATH", previous_path);
        } else {
            env::remove_var("PATH");
        }
    }
}
