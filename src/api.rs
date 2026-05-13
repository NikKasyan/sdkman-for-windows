use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::{fs, path::PathBuf, time::Duration};

use crate::state::State;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct Version {
    pub value: String,
}

pub struct Api {
    client: Client,
    base: String,
    cache: PathBuf,
}

impl Api {
    pub fn new(state: &State) -> Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(state.config.insecure_ssl)
            .connect_timeout(Duration::from_secs(state.config.curl_connect_timeout))
            .timeout(Duration::from_secs(state.config.curl_max_time))
            .user_agent("sdkman-windows/0.1")
            .build()?;
        Ok(Self {
            client,
            base: "https://api.sdkman.io/2".to_string(),
            cache: state.metadata_dir(),
        })
    }

    pub fn candidates(&self, offline: bool) -> Result<Vec<Candidate>> {
        let path = self.cache.join("candidates.txt");
        let text = self.get_cached(&format!("{}/candidates/all", self.base), &path, offline)?;
        Ok(parse_candidates(&text))
    }

    pub fn versions(&self, candidate: &str, offline: bool) -> Result<Vec<Version>> {
        let path = self.cache.join(format!("{candidate}-versions.txt"));
        let urls = [
            format!("{}/candidates/{candidate}/windows/versions/all", self.base),
            format!("{}/candidates/{candidate}/win/versions/all", self.base),
        ];

        if offline {
            let text = fs::read_to_string(path)
                .context("metadata cache is unavailable in offline mode")?;
            return Ok(parse_versions(&text));
        }

        for url in urls {
            match self
                .client
                .get(&url)
                .send()
                .and_then(|r| r.error_for_status())
            {
                Ok(resp) => {
                    let text = resp.text()?;
                    fs::create_dir_all(&self.cache)?;
                    fs::write(&path, &text)?;
                    return Ok(parse_versions(&text));
                }
                Err(_) => continue,
            }
        }
        bail!("could not fetch versions for {candidate}")
    }

    pub fn download_url(&self, candidate: &str, version: &str) -> String {
        format!(
            "{}/broker/download/{candidate}/{version}/windows",
            self.base
        )
    }

    pub fn refresh(&self) -> Result<()> {
        let _ = self.candidates(false)?;
        Ok(())
    }

    fn get_cached(&self, url: &str, path: &PathBuf, offline: bool) -> Result<String> {
        if offline {
            return fs::read_to_string(path)
                .context("metadata cache is unavailable in offline mode");
        }
        let text = self.client.get(url).send()?.error_for_status()?.text()?;
        fs::create_dir_all(&self.cache)?;
        fs::write(path, &text)?;
        Ok(text)
    }
}

fn parse_candidates(text: &str) -> Vec<Candidate> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        let items = json
            .as_array()
            .or_else(|| json.get("candidates").and_then(|v| v.as_array()));
        if let Some(items) = items {
            return items
                .iter()
                .filter_map(|item| {
                    if let Some(name) = item.as_str() {
                        return Some(Candidate {
                            name: name.to_string(),
                            description: String::new(),
                        });
                    }
                    let name = item
                        .get("candidate")
                        .or_else(|| item.get("name"))?
                        .as_str()?
                        .to_string();
                    let description = item
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(Candidate { name, description })
                })
                .collect();
        }
    }

    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let name = json
                    .get("candidate")
                    .or_else(|| json.get("name"))?
                    .as_str()?
                    .to_string();
                let description = json
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                return Some(Candidate { name, description });
            }
            let mut parts = line.splitn(2, ',');
            let name = parts.next()?.trim().to_string();
            let description = parts.next().unwrap_or("").trim().to_string();
            Some(Candidate { name, description })
        })
        .collect()
}

fn parse_versions(text: &str) -> Vec<Version> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        let items = json
            .as_array()
            .or_else(|| json.get("versions").and_then(|v| v.as_array()));
        if let Some(items) = items {
            return items
                .iter()
                .filter_map(|item| {
                    if let Some(value) = item.as_str() {
                        return Some(Version {
                            value: value.to_string(),
                        });
                    }
                    let value = item
                        .get("version")
                        .or_else(|| item.get("candidateVersion"))
                        .or_else(|| item.get("id"))?
                        .as_str()?
                        .to_string();
                    Some(Version { value })
                })
                .collect();
        }
    }

    let normalized = text.replace([',', '|'], "\n");
    normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('{') && !line.starts_with('['))
        .map(|value| Version {
            value: value.to_string(),
        })
        .collect()
}
