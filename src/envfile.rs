use anyhow::{bail, Result};
use std::{collections::BTreeMap, fs, path::Path};

pub fn parse(path: &Path) -> Result<BTreeMap<String, String>> {
    if !path.exists() {
        bail!(
            ".sdkmanrc not found in {}",
            path.parent().unwrap_or(Path::new(".")).display()
        );
    }
    parse_text(&fs::read_to_string(path)?)
}

pub fn parse_text(text: &str) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((candidate, version)) = line.split_once('=') {
            values.insert(candidate.trim().to_string(), version.trim().to_string());
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sdkmanrc() {
        let values = parse_text("java=21-tem\nmaven=3.9.9\n").unwrap();
        assert_eq!(values["java"], "21-tem");
        assert_eq!(values["maven"], "3.9.9");
    }
}
