//! Kubernetes context/namespace detector.
//!
//! Reads `current-context` (and that context's `namespace`) from the active
//! kubeconfig — `$KUBECONFIG` first, then `~/.kube/config` — using a tiny
//! line-oriented YAML reader rather than pulling in a full YAML dependency.
//! In-cluster service-account state is honoured when present.

use super::{home_dir, read_bounded, EnvironmentInfo, EnvironmentKind};
use std::path::PathBuf;

/// Detect the active Kubernetes context, if any.
pub fn detect() -> Option<EnvironmentInfo> {
    if let Some(info) = service_account() {
        return Some(info);
    }

    for path in candidate_paths() {
        if let Some(text) = read_bounded(&path) {
            if let Some(info) = parse_kubeconfig(&text) {
                return Some(info);
            }
        }
    }

    None
}

fn service_account() -> Option<EnvironmentInfo> {
    std::env::var_os("KUBERNETES_SERVICE_HOST")?;
    let host = std::env::var("KUBERNETES_SERVICE_HOST").unwrap_or_default();
    let namespace =
        std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

    let label = match namespace {
        Some(ns) => format!("in-cluster/{ns}"),
        None => format!("in-cluster@{host}"),
    };
    Some(EnvironmentInfo::new(EnvironmentKind::Kubernetes, label))
}

fn candidate_paths() -> Vec<PathBuf> {
    if let Some(raw) = std::env::var_os("KUBECONFIG") {
        let paths: Vec<PathBuf> = std::env::split_paths(&raw).collect();
        if !paths.is_empty() {
            return paths;
        }
    }

    home_dir()
        .map(|home| vec![home.join(".kube").join("config")])
        .unwrap_or_default()
}

/// Parse `current-context` and its namespace from a kubeconfig document.
fn parse_kubeconfig(text: &str) -> Option<EnvironmentInfo> {
    let current = field(text.lines(), "current-context:")?;
    let namespace = context_blocks(text)
        .iter()
        .find(|block| field(block.lines(), "name:").as_deref() == Some(current.as_str()))
        .and_then(|block| field(block.lines(), "namespace:"));

    let label = match namespace {
        Some(ns) if !ns.is_empty() && ns != "default" => format!("{current}/{ns}"),
        _ => current,
    };
    Some(EnvironmentInfo::new(EnvironmentKind::Kubernetes, label))
}

/// First `key: value` occurrence across `lines`.
fn field<'a, I: Iterator<Item = &'a str>>(lines: I, key: &str) -> Option<String> {
    lines.into_iter().find_map(|line| {
        let trimmed = line.trim();
        let rest = trimmed.strip_prefix(key)?;
        let value = rest.trim().trim_matches('"').trim_matches('\'').trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

/// Split the top-level `contexts:` sequence into individual entry blocks.
fn context_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_contexts = false;
    let mut current: Vec<String> = Vec::new();

    let flush = |block: &mut Vec<String>, blocks: &mut Vec<String>| {
        if !block.is_empty() {
            blocks.push(block.join("\n"));
            block.clear();
        }
    };

    for line in text.lines() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // Top-level map keys are unindented; a `-` list item (which may also sit
        // at column 0 under `contexts:`) is handled below.
        if indent == 0 && !trimmed.starts_with('-') {
            if in_contexts {
                flush(&mut current, &mut blocks);
            }
            in_contexts = trimmed.starts_with("contexts:");
            continue;
        }

        if !in_contexts {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix('-') {
            flush(&mut current, &mut blocks);
            let rest = rest.trim();
            if !rest.is_empty() {
                current.push(rest.to_string());
            }
        } else if !current.is_empty() {
            current.push(trimmed.to_string());
        }
    }
    if in_contexts {
        flush(&mut current, &mut blocks);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: dev-cluster
contexts:
- context:
    cluster: dev
    namespace: team-a
    user: dev
  name: dev-cluster
- context:
    cluster: prod
    user: ops
  name: prod-cluster
clusters:
- cluster:
    server: https://dev.example
  name: dev
"#;

    #[test]
    fn reads_context_and_namespace() {
        let info = parse_kubeconfig(KUBECONFIG).expect("context should parse");
        assert_eq!(info.kind, EnvironmentKind::Kubernetes);
        assert_eq!(info.label, "dev-cluster/team-a");
    }

    #[test]
    fn omits_default_namespace() {
        let text = KUBECONFIG.replace("namespace: team-a", "namespace: default");
        let info = parse_kubeconfig(&text).unwrap();
        assert_eq!(info.label, "dev-cluster");
    }

    #[test]
    fn context_without_namespace_is_just_the_name() {
        let text = KUBECONFIG.replace("    namespace: team-a\n", "");
        let info = parse_kubeconfig(&text).unwrap();
        assert_eq!(info.label, "dev-cluster");
    }

    #[test]
    fn missing_current_context_yields_nothing() {
        assert!(parse_kubeconfig("apiVersion: v1\n").is_none());
    }
}
