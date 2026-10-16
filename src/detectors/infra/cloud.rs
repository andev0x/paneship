//! Cloud profile detector (AWS / GCP / Azure).
//!
//! Resolves the *active* profile identifier from environment variables first,
//! then from each provider's own configuration file. Reads are bounded and no
//! network access or CLI subprocess is ever performed.

use super::{home_dir, read_bounded, EnvironmentInfo, EnvironmentKind};
use std::path::PathBuf;

/// Detect all active cloud profiles, in stable order (AWS, GCP, Azure).
pub fn detect() -> Vec<EnvironmentInfo> {
    let mut entries = Vec::new();
    if let Some(profile) = aws_profile() {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Aws, profile));
    }
    if let Some(project) = gcp_profile() {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Gcp, project));
    }
    if let Some(subscription) = azure_profile() {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Azure, subscription));
    }
    entries
}

// ---------------------------------------------------------------------------
// AWS
// ---------------------------------------------------------------------------

fn aws_profile() -> Option<String> {
    for key in ["AWS_PROFILE", "AWS_DEFAULT_PROFILE", "AWS_VAULT"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // Fall back to a `[default]` section in the shared config file.
    let path = std::env::var_os("AWS_CONFIG_FILE")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".aws").join("config")))?;
    let text = read_bounded(&path)?;
    let section = first_ini_section(&text)?;
    (section == "default").then_some(section)
}

/// Name of the first `[section]` header in an INI-style file.
fn first_ini_section(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('[') else {
            continue;
        };
        let Some(name) = rest.strip_suffix(']') else {
            continue;
        };
        let name = name.trim().trim_start_matches("profile").trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// GCP
// ---------------------------------------------------------------------------

fn gcp_profile() -> Option<String> {
    for key in [
        "CLOUDSDK_CORE_PROJECT",
        "GOOGLE_CLOUD_PROJECT",
        "GCLOUD_PROJECT",
    ] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    if let Ok(value) = std::env::var("CLOUDSDK_ACTIVE_CONFIG_NAME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let config_dir = std::env::var_os("CLOUDSDK_CONFIG")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".config").join("gcloud")))?;
    let name = std::fs::read_to_string(config_dir.join("active_config")).ok()?;
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

// ---------------------------------------------------------------------------
// Azure
// ---------------------------------------------------------------------------

fn azure_profile() -> Option<String> {
    let config_dir = std::env::var_os("AZURE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".azure")))?;
    let text = read_bounded(&config_dir.join("azureProfile.json"))?;
    parse_azure_profile(&text)
}

fn parse_azure_profile(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let subscriptions = value.get("subscriptions")?.as_array()?;

    let pick = subscriptions
        .iter()
        .find(|entry| entry.get("isDefault").and_then(|flag| flag.as_bool()) == Some(true))
        .or_else(|| subscriptions.first())?;

    pick.get("name")
        .and_then(|name| name.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_first_ini_section_and_strips_profile_prefix() {
        let text = "[default]\nregion = us-east-1\n[profile work]\nregion = eu-west-1\n";
        assert_eq!(first_ini_section(text).as_deref(), Some("default"));

        let work = "[profile work]\nregion = eu-west-1\n";
        assert_eq!(first_ini_section(work).as_deref(), Some("work"));
    }

    #[test]
    fn azure_profile_prefers_default_subscription() {
        let json = r#"{
            "subscriptions": [
                {"name": "Personal", "isDefault": false},
                {"name": "Production", "isDefault": true}
            ]
        }"#;
        assert_eq!(parse_azure_profile(json).as_deref(), Some("Production"));
    }

    #[test]
    fn azure_profile_falls_back_to_first_subscription() {
        let json = r#"{"subscriptions": [{"name": "Only"}]}"#;
        assert_eq!(parse_azure_profile(json).as_deref(), Some("Only"));
    }

    #[test]
    fn azure_profile_without_subscriptions_is_none() {
        assert!(parse_azure_profile("{}").is_none());
        assert!(parse_azure_profile("not json").is_none());
    }
}
