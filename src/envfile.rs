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
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((candidate, version)) = line.split_once('=') {
            let candidate = candidate.trim();
            let version = version.trim();
            let line_number = index + 1;
            if candidate.is_empty() {
                bail!(".sdkmanrc line {line_number} has an empty candidate");
            }
            if version.is_empty() {
                bail!(".sdkmanrc line {line_number} has an empty version");
            }
            if values
                .insert(candidate.to_string(), version.to_string())
                .is_some()
            {
                bail!(".sdkmanrc line {line_number} duplicates candidate {candidate}");
            }
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

    #[test]
    fn rejects_empty_candidate() {
        let error = parse_text("=21-tem\n").unwrap_err().to_string();
        assert!(error.contains(".sdkmanrc line 1 has an empty candidate"));
    }

    #[test]
    fn rejects_empty_version() {
        let error = parse_text("java=\n").unwrap_err().to_string();
        assert!(error.contains(".sdkmanrc line 1 has an empty version"));
    }

    #[test]
    fn rejects_duplicate_candidate() {
        let error = parse_text("java=21-tem\njava=17-tem\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains(".sdkmanrc line 2 duplicates candidate java"));
    }
}
