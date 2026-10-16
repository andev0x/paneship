//! Lightweight system health detector (opt-in).
//!
//! Reports high CPU pressure (1-minute load average vs. core count) and a low,
//! discharging battery. All probes are cheap syscalls/file reads; on platforms
//! without a supported source the detector simply contributes nothing.

use super::{EnvironmentInfo, EnvironmentKind};

/// High-load threshold: fraction of the core count at which load is surfaced.
const LOAD_PRESSURE_THRESHOLD: f64 = 0.9;
/// Battery percentage at or below which a discharge is surfaced.
const LOW_BATTERY_PERCENT: u8 = 20;

/// Detect CPU pressure and low battery, if any.
pub fn detect() -> Vec<EnvironmentInfo> {
    let mut entries = Vec::new();

    if let Some(label) = load_pressure() {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Load, label));
    }
    if let Some(label) = battery_low() {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Battery, label));
    }

    entries
}

fn load_pressure() -> Option<String> {
    let load = current_load_average()?;
    let ncpu = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    load_label(load, ncpu)
}

fn load_label(load: f64, ncpu: usize) -> Option<String> {
    let cores = ncpu.max(1) as f64;
    if load / cores >= LOAD_PRESSURE_THRESHOLD {
        Some(format!("{load:.2}"))
    } else {
        None
    }
}

#[cfg(unix)]
fn current_load_average() -> Option<f64> {
    let mut loads = [0f64; 1];
    // SAFETY: `getloadavg` writes up to `nelem` f64 values into the buffer.
    let read = unsafe { libc::getloadavg(loads.as_mut_ptr(), 1) };
    (read >= 1).then_some(loads[0])
}

#[cfg(not(unix))]
fn current_load_average() -> Option<f64> {
    None
}

fn battery_low() -> Option<String> {
    battery_label(read_battery())
}

fn battery_label(battery: Option<(u8, String)>) -> Option<String> {
    let (capacity, status) = battery?;
    let discharging =
        !status.eq_ignore_ascii_case("Charging") && !status.eq_ignore_ascii_case("Full");
    (discharging && capacity <= LOW_BATTERY_PERCENT).then(|| format!("{capacity}%"))
}

#[cfg(target_os = "linux")]
fn read_battery() -> Option<(u8, String)> {
    let entries = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("BAT") {
            continue;
        }
        let capacity = std::fs::read_to_string(entry.path().join("capacity"))
            .ok()?
            .trim()
            .parse::<u8>()
            .ok()?;
        let status = std::fs::read_to_string(entry.path().join("status"))
            .unwrap_or_default()
            .trim()
            .to_string();
        return Some((capacity, status));
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_battery() -> Option<(u8, String)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_label_only_when_pressured() {
        assert_eq!(load_label(0.5, 8), None);
        assert_eq!(load_label(7.2, 8).as_deref(), Some("7.20"));
        assert!(load_label(1.0, 1).is_some());
    }

    #[test]
    fn battery_label_only_when_low_and_discharging() {
        assert_eq!(
            battery_label(Some((15, "Discharging".to_string()))).as_deref(),
            Some("15%")
        );
        assert_eq!(battery_label(Some((15, "Charging".to_string()))), None);
        assert_eq!(battery_label(Some((80, "Discharging".to_string()))), None);
        assert_eq!(battery_label(None), None);
    }
}
