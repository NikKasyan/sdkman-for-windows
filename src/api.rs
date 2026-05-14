use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::{fs, path::PathBuf, time::Duration};

use crate::state::State;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Version {
    pub value: String,
    pub display_version: Option<String>,
    pub distribution: Option<String>,
    pub vendor: Option<String>,
}

impl Version {
    pub fn local(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            display_version: Some(value.clone()),
            distribution: Some("local".to_string()),
            vendor: Some("Local".to_string()),
            value,
        }
    }

    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            display_version: None,
            distribution: None,
            vendor: None,
        }
    }
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
        let base =
            std::env::var("SDKMAN_API").unwrap_or_else(|_| "https://api.sdkman.io/2".to_string());
        Ok(Self {
            client,
            base,
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
        let urls = version_urls(&self.base, candidate);

        if offline {
            let text = fs::read_to_string(path)
                .context("metadata cache is unavailable in offline mode")?;
            return Ok(parse_versions(&text));
        }

        let mut last_err: Option<anyhow::Error> = None;
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
                Err(e) => {
                    last_err = Some(anyhow::anyhow!("{url}: {e}"));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no URLs to try")))
            .with_context(|| format!("failed to fetch versions for {candidate}"))
    }

    pub fn download_url(&self, candidate: &str, version: &str) -> Vec<String> {
        let platforms = download_platforms(candidate);
        platforms
            .into_iter()
            .map(|p| format!("{}/broker/download/{candidate}/{version}/{p}", self.base))
            .collect()
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn refresh(&self) -> Result<()> {
        let _ = self.candidates(false)?;
        Ok(())
    }

    /// Fetches the latest SDKMAN broadcast message. Returns `Some(message)` only
    /// when the message differs from the locally cached copy, so callers can
    /// display it exactly once per change.
    pub fn broadcast(&self) -> Option<String> {
        let url = format!("{}/broadcast/latest", self.base);
        let text = self
            .client
            .get(&url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        let path = self.cache.join("broadcast.txt");
        let cached = fs::read_to_string(&path).unwrap_or_default();
        if trimmed == cached.trim() {
            return None;
        }
        let _ = fs::create_dir_all(&self.cache);
        let _ = fs::write(&path, trimmed);
        Some(trimmed.to_string())
    }

    fn get_cached(&self, url: &str, path: &PathBuf, offline: bool) -> Result<String> {
        if offline {
            return fs::read_to_string(path)
                .context("metadata cache is unavailable in offline mode");
        }
        let text = self
            .client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .with_context(|| format!("failed to fetch {url}"))?;
        fs::create_dir_all(&self.cache)?;
        fs::write(path, &text)?;
        Ok(text)
    }
}

fn version_urls(base: &str, candidate: &str) -> Vec<String> {
    if candidate.eq_ignore_ascii_case("java") {
        return vec![
            format!("{base}/candidates/java/windowsx64/versions/list?installed="),
            format!("{base}/candidates/java/windows/versions/all"),
            format!("{base}/candidates/java/win/versions/all"),
        ];
    }
    vec![
        format!("{base}/candidates/{candidate}/windows/versions/all"),
        format!("{base}/candidates/{candidate}/win/versions/all"),
    ]
}

fn download_platforms(candidate: &str) -> Vec<&'static str> {
    if candidate.eq_ignore_ascii_case("java") {
        vec!["windowsx64", "windows", "win"]
    } else {
        vec!["windows", "win", "windowsx64", "generic"]
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
                    let name = item
                        .as_str()
                        .or_else(|| {
                            item.get("candidate")
                                .or_else(|| item.get("name"))
                                .and_then(|v| v.as_str())
                        })?
                        .to_string();
                    Some(Candidate { name })
                })
                .collect();
        }
    }

    let mut result = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(name) = json
                .get("candidate")
                .or_else(|| json.get("name"))
                .and_then(|v| v.as_str())
            {
                result.push(Candidate {
                    name: name.to_string(),
                });
            }
            continue;
        }
        // If all comma-separated tokens are identifier-like (no spaces), the line is a
        // flat list of candidate names (the format /candidates/all actually returns).
        if line.contains(',') {
            let tokens: Vec<&str> = line
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .collect();
            if tokens.iter().all(|t| !t.contains(' ')) {
                for token in tokens {
                    result.push(Candidate {
                        name: token.to_string(),
                    });
                }
                continue;
            }
            // Tokens with spaces: only the first token is a candidate name.
            if let Some(&name) = tokens.first() {
                result.push(Candidate {
                    name: name.to_string(),
                });
            }
        } else {
            result.push(Candidate {
                name: line.to_string(),
            });
        }
    }
    result
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
                        return Some(Version::new(value));
                    }
                    let value = item
                        .get("version")
                        .or_else(|| item.get("candidateVersion"))
                        .or_else(|| item.get("id"))?
                        .as_str()?
                        .to_string();
                    Some(Version::new(value))
                })
                .collect();
        }
    }

    let table_versions = parse_versions_table(text);
    if !table_versions.is_empty() {
        return table_versions;
    }

    let normalized = text.replace([',', '|'], "\n");
    normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('{') && !line.starts_with('['))
        .map(Version::new)
        .collect()
}

fn parse_versions_table(text: &str) -> Vec<Version> {
    text.lines()
        .filter_map(|line| {
            if !line.contains('|') {
                return None;
            }
            let columns = line.split('|').map(str::trim).collect::<Vec<_>>();
            if columns.len() < 6 {
                return None;
            }
            let identifier = columns.last()?;
            if identifier.is_empty() || identifier.eq_ignore_ascii_case("identifier") {
                return None;
            }
            Some(Version {
                value: identifier.to_string(),
                display_version: Some(columns[2].to_string()),
                distribution: Some(columns[3].to_string()),
                vendor: if columns[0].is_empty() {
                    None
                } else {
                    Some(columns[0].to_string())
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_candidates_from_json_array_of_strings() {
        let candidates = parse_candidates(r#"["java","maven"]"#);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].name, "java");
        assert_eq!(candidates[1].name, "maven");
    }

    #[test]
    fn parses_candidates_from_json_object_array() {
        let candidates = parse_candidates(
            r#"{"candidates":[{"candidate":"java","description":"JVMs"},{"name":"maven"}]}"#,
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].name, "java");
        assert_eq!(candidates[1].name, "maven");
    }

    #[test]
    fn parses_candidates_from_line_based_fallbacks() {
        let candidates =
            parse_candidates("java\n{\"candidate\":\"maven\",\"description\":\"Build tool\"}\n");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].name, "java");
        assert_eq!(candidates[1].name, "maven");
    }

    #[test]
    fn parses_candidates_from_flat_comma_separated_list() {
        let candidates = parse_candidates("ant,maven,tomcat");

        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].name, "ant");
        assert_eq!(candidates[1].name, "maven");
        assert_eq!(candidates[2].name, "tomcat");
    }

    #[test]
    fn parses_versions_from_json_array_of_strings() {
        let versions = parse_versions(r#"["21.0.4-tem","17.0.12-tem"]"#);

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].value, "21.0.4-tem");
        assert_eq!(versions[1].value, "17.0.12-tem");
    }

    #[test]
    fn parses_versions_from_json_object_array() {
        let versions = parse_versions(
            r#"{"versions":[{"version":"21.0.4-tem"},{"candidateVersion":"17.0.12-tem"},{"id":"11.0.24-tem"}]}"#,
        );

        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].value, "21.0.4-tem");
        assert_eq!(versions[1].value, "17.0.12-tem");
        assert_eq!(versions[2].value, "11.0.24-tem");
    }

    #[test]
    fn parses_versions_from_delimited_fallbacks() {
        let versions = parse_versions("21.0.4-tem,17.0.12-tem|11.0.24-tem");

        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].value, "21.0.4-tem");
        assert_eq!(versions[1].value, "17.0.12-tem");
        assert_eq!(versions[2].value, "11.0.24-tem");
    }

    #[test]
    fn parses_versions_from_sdkman_table_output() {
        let versions = parse_versions(
            r#"
 Vendor        | Use | Version      | Dist    | Status     | Identifier
--------------------------------------------------------------------------------
 Temurin       |     | 25.0.3       | tem     |            | 25.0.3-tem
               |     | 21.0.11      | tem     |            | 21.0.11-tem
"#,
        );

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].value, "25.0.3-tem");
        assert_eq!(versions[0].vendor.as_deref(), Some("Temurin"));
        assert_eq!(versions[0].display_version.as_deref(), Some("25.0.3"));
        assert_eq!(versions[0].distribution.as_deref(), Some("tem"));
        assert_eq!(versions[1].value, "21.0.11-tem");
        assert_eq!(versions[1].vendor, None);
    }

    #[test]
    fn java_versions_use_sdkman_table_endpoint_first() {
        let urls = version_urls("https://api.sdkman.io/2", "java");

        assert_eq!(
            urls[0],
            "https://api.sdkman.io/2/candidates/java/windowsx64/versions/list?installed="
        );
    }

    #[test]
    fn java_downloads_use_windows_x64_platform_first() {
        let platforms = download_platforms("java");
        assert_eq!(platforms[0], "windowsx64");
        let platforms = download_platforms("JAVA");
        assert_eq!(platforms[0], "windowsx64");
    }

    #[test]
    fn non_java_downloads_keep_windows_platform_first() {
        let platforms = download_platforms("maven");
        assert_eq!(platforms[0], "windows");
    }

    #[test]
    fn parsers_ignore_empty_and_malformed_input() {
        assert!(parse_candidates("\n\n").is_empty());
        assert!(parse_candidates(r#"{"unexpected":[]}"#).is_empty());
        assert!(parse_versions("\n\n").is_empty());
        assert!(parse_versions(r#"{"unexpected":[]}"#).is_empty());
    }
}
