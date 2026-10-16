//! Container / virtualization detector.
//!
//! Uses cheap, non-blocking probes only: the `/.dockerenv` sentinel, the
//! `container` environment variable, and a bounded read of `/proc/1/cgroup`.
//! No container runtime is ever contacted.

use super::{EnvironmentInfo, EnvironmentKind};
use std::path::Path;

/// Detect whether the process is running inside a container.
pub fn detect() -> Option<EnvironmentInfo> {
    if Path::new("/.dockerenv").exists() {
        return Some(info("docker"));
    }
    if Path::new("/run/.containerenv").exists() {
        return Some(info("podman"));
    }

    if let Some(label) = parse_env_marker(std::env::var("container").ok().as_deref()) {
        return Some(info(label));
    }

    let contents = std::fs::read_to_string("/proc/1/cgroup").ok()?;
    parse_cgroup(&contents).map(info)
}

fn parse_env_marker(value: Option<&str>) -> Option<&'static str> {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "docker" => Some("docker"),
        "podman" => Some("podman"),
        "kubernetes" | "kube" => Some("k8s"),
        "" | "0" | "false" => None,
        _ => Some("container"),
    }
}

fn parse_cgroup(contents: &str) -> Option<&'static str> {
    let lower = contents.to_ascii_lowercase();
    if lower.contains("kubepods") {
        Some("k8s")
    } else if lower.contains("docker") {
        Some("docker")
    } else if lower.contains("podman") {
        Some("podman")
    } else if lower.contains("lxc") {
        Some("lxc")
    } else {
        None
    }
}

fn info(label: &str) -> EnvironmentInfo {
    EnvironmentInfo::new(EnvironmentKind::Docker, label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_cgroup_markers() {
        assert_eq!(parse_cgroup("/kubepods/besteffort/pod123/abc"), Some("k8s"));
        assert_eq!(parse_cgroup("12:cpuset:/docker/abcdef"), Some("docker"));
        assert_eq!(parse_cgroup("/user.slice"), None);
    }

    #[test]
    fn recognises_env_markers() {
        assert_eq!(parse_env_marker(Some("docker")), Some("docker"));
        assert_eq!(parse_env_marker(Some("  PODMAN ")), Some("podman"));
        assert_eq!(parse_env_marker(Some("0")), None);
        assert_eq!(parse_env_marker(None), None);
        assert_eq!(parse_env_marker(Some("firecracker")), Some("container"));
    }
}
