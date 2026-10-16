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

/// Read battery state on macOS via `pmset -g batt`.
///
/// This shells out to a platform helper (there is no stable file interface),
/// which is why health detection runs under the looser
/// [`super::HEALTH_TIMEOUT`] budget rather than the strict 10 ms probe budget.
#[cfg(target_os = "macos")]
fn read_battery() -> Option<(u8, String)> {
    let output = std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_pmset_battery(&String::from_utf8_lossy(&output.stdout))
}

/// Parse the `pmset -g batt` output for the first battery percentage/status.
///
/// Example line: `-InternalBattery-0 (id=1234567)\t91%; discharging; 4:12 remaining`.
fn parse_pmset_battery(text: &str) -> Option<(u8, String)> {
    for line in text.lines() {
        let Some(pct_index) = line.find('%') else {
            continue;
        };
        let digits: String = line[..pct_index]
            .chars()
            .rev()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let Ok(capacity) = digits.parse::<u8>() else {
            continue;
        };
        let status = line[pct_index + 1..]
            .split(';')
            .nth(1)
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        return Some((capacity, status));
    }
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
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

    #[test]
    fn pmset_output_parses_capacity_and_status() {
        let discharging = "\
Now drawing from 'Battery Power'
 -InternalBattery-0 (id=1234567)\t76%; discharging; 4:12 remaining present: true";
        assert_eq!(
            parse_pmset_battery(discharging),
            Some((76, "discharging".to_string()))
        );

        let charging = "\
Now drawing from 'AC Power'
 -InternalBattery-0 (id=1234567)\t100%; charging; (no estimate) present: true";
        assert_eq!(
            parse_pmset_battery(charging),
            Some((100, "charging".to_string()))
        );
    }

    #[test]
    fn pmset_without_battery_yields_nothing() {
        assert_eq!(parse_pmset_battery("Now drawing from 'AC Power'\n"), None);
    }

    #[test]
    fn pmset_capacity_feeds_status_only_when_discharging() {
        let parsed =
            parse_pmset_battery(" -InternalBattery-0 (id=1)\t12%; discharging; 1:00 present: true");
        assert_eq!(battery_label(parsed).as_deref(), Some("12%"));
    }
}
