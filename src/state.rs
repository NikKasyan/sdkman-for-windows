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
        if let Some(session) = env::var_os(session_home_var(candidate)) {
            return Ok(Some(PathBuf::from(session)));
        }
        let link = self.current_link(candidate);
        if link.exists() {
            return Ok(Some(resolve_linkish(&link)));
        }
        Ok(None)
    }
}

pub fn session_home_var(candidate: &str) -> String {
    format!(
        "SDKMAN_{}_HOME",
        candidate.to_ascii_uppercase().replace('-', "_")
    )
}

fn resolve_linkish(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
