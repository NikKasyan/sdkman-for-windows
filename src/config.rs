use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub auto_answer: bool,
    pub insecure_ssl: bool,
    pub curl_connect_timeout: u64,
    pub curl_max_time: u64,
    pub colour_enable: bool,
    pub debug_mode: bool,
    pub healthcheck_enable: bool,
    pub auto_env: bool,
    pub offline_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_answer: false,
            insecure_ssl: false,
            curl_connect_timeout: 5,
            curl_max_time: 60,
            colour_enable: true,
            debug_mode: false,
            healthcheck_enable: true,
            auto_env: false,
            offline_mode: false,
        }
    }
}

impl Config {
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        Self::from_str(&fs::read_to_string(path)?)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_properties())?;
        Ok(())
    }

    pub fn from_str(text: &str) -> Result<Self> {
        let mut values = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        let mut cfg = Self::default();
        cfg.auto_answer = bool_key(&values, "sdkman_auto_answer", cfg.auto_answer);
        cfg.insecure_ssl = bool_key(&values, "sdkman_insecure_ssl", cfg.insecure_ssl);
        cfg.curl_connect_timeout = int_key(
            &values,
            "sdkman_curl_connect_timeout",
            cfg.curl_connect_timeout,
        );
        cfg.curl_max_time = int_key(&values, "sdkman_curl_max_time", cfg.curl_max_time);
        cfg.colour_enable = bool_key(&values, "sdkman_colour_enable", cfg.colour_enable);
        cfg.debug_mode = bool_key(&values, "sdkman_debug_mode", cfg.debug_mode);
        cfg.healthcheck_enable =
            bool_key(&values, "sdkman_healthcheck_enable", cfg.healthcheck_enable);
        cfg.auto_env = bool_key(&values, "sdkman_auto_env", cfg.auto_env);
        cfg.offline_mode = bool_key(&values, "sdkman_offline_mode", cfg.offline_mode);
        Ok(cfg)
    }

    pub fn to_properties(&self) -> String {
        format!(
            "sdkman_auto_answer={}\nsdkman_insecure_ssl={}\nsdkman_curl_connect_timeout={}\nsdkman_curl_max_time={}\nsdkman_colour_enable={}\nsdkman_debug_mode={}\nsdkman_healthcheck_enable={}\nsdkman_auto_env={}\nsdkman_offline_mode={}\n",
            self.auto_answer,
            self.insecure_ssl,
            self.curl_connect_timeout,
            self.curl_max_time,
            self.colour_enable,
            self.debug_mode,
            self.healthcheck_enable,
            self.auto_env,
            self.offline_mode
        )
    }

    pub fn set_key(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "sdkman_auto_answer" => self.auto_answer = parse_bool(key, value)?,
            "sdkman_insecure_ssl" => self.insecure_ssl = parse_bool(key, value)?,
            "sdkman_curl_connect_timeout" => self.curl_connect_timeout = parse_int(key, value)?,
            "sdkman_curl_max_time" => self.curl_max_time = parse_int(key, value)?,
            "sdkman_colour_enable" => self.colour_enable = parse_bool(key, value)?,
            "sdkman_debug_mode" => self.debug_mode = parse_bool(key, value)?,
            "sdkman_healthcheck_enable" => self.healthcheck_enable = parse_bool(key, value)?,
            "sdkman_auto_env" => self.auto_env = parse_bool(key, value)?,
            "sdkman_offline_mode" => self.offline_mode = parse_bool(key, value)?,
            _ => bail!("unknown config key: {key}"),
        }
        Ok(())
    }
}

fn bool_key(values: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    values
        .get(key)
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn int_key(values: &BTreeMap<String, String>, key: &str, default: u64) -> u64 {
    values
        .get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value {
        value if value.eq_ignore_ascii_case("true") => Ok(true),
        value if value.eq_ignore_ascii_case("false") => Ok(false),
        _ => bail!("{key} expects true or false"),
    }
}

fn parse_int(key: &str, value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("{key} expects a non-negative integer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sdkman_keys() {
        let cfg = Config::from_str("sdkman_auto_answer=true\nsdkman_curl_max_time=9\n").unwrap();
        assert!(cfg.auto_answer);
        assert_eq!(cfg.curl_max_time, 9);
    }
}
