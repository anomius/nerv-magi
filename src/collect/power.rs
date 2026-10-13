//! 1 Hz power sampler: battery, AC adapter and platform profile from sysfs; battery details
//! (`PowerExtra`: health, cycles, volts, amps, model) every 10 s.

use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{BatStatus, Battery, PowerExtra, PowerState, Profile, Update};

const EXTRA_EVERY: u64 = 10;

fn read_str(p: &Path) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// Some drivers report `current_now`/`power_now` signed (negative while discharging).
fn read_abs(p: &Path) -> Option<u64> {
    read_str(p)?.parse::<i64>().ok().map(i64::unsigned_abs)
}

fn power_supplies(root: &Path) -> Vec<std::path::PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(root.join("class/power_supply"))
        .map(|it| it.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    // A wireless mouse or headset reports `scope=Device`: never the machine's battery or charger.
    entries.retain(|d| read_str(&d.join("scope")).as_deref() != Some("Device"));
    entries
}

fn first_battery(root: &Path) -> Option<std::path::PathBuf> {
    power_supplies(root).into_iter().find(|d| read_str(&d.join("type")).as_deref() == Some("Battery"))
}

/// Health (full / design, charge_* or energy_*), cycle count, voltage, current and model of the
/// first battery; all `None` without one.
pub fn read_power_extra(root: &Path) -> PowerExtra {
    let Some(dir) = first_battery(root) else { return PowerExtra::default() };
    let ratio = |full: &str, design: &str| match (read_abs(&dir.join(full)), read_abs(&dir.join(design))) {
        (Some(f), Some(d)) if d > 0 => Some((f as f64 * 100.0 / d as f64) as f32),
        _ => None,
    };
    PowerExtra {
        health_pct: ratio("charge_full", "charge_full_design").or_else(|| ratio("energy_full", "energy_full_design")),
        cycles: read_str(&dir.join("cycle_count")).and_then(|c| c.parse().ok()),
        voltage_v: read_abs(&dir.join("voltage_now")).map(|v| (v as f64 / 1e6) as f32),
        current_a: read_abs(&dir.join("current_now")).map(|a| (a as f64 / 1e6) as f32),
        model: read_str(&dir.join("model_name")).filter(|m| !m.is_empty()),
    }
}

fn read_battery(dir: &Path) -> Battery {
    let status = BatStatus::parse(&read_str(&dir.join("status")).unwrap_or_default());
    // energy_* (µWh, µW) when the driver has them, else charge_* (µAh, µA).
    let (now, full, rate) = if dir.join("energy_now").is_file() {
        (read_abs(&dir.join("energy_now")), read_abs(&dir.join("energy_full")), read_abs(&dir.join("power_now")))
    } else {
        (read_abs(&dir.join("charge_now")), read_abs(&dir.join("charge_full")), read_abs(&dir.join("current_now")))
    };
    let percent = read_str(&dir.join("capacity"))
        .and_then(|c| c.parse::<f32>().ok())
        .or_else(|| match (now, full) {
            (Some(n), Some(f)) if f > 0 => Some(n as f32 * 100.0 / f as f32),
            _ => None,
        })
        .unwrap_or(0.0);
    let hours_x3600 = |amount: u64, rate: u64| amount.saturating_mul(3600) / rate;
    let seconds_left = match (status, now, full, rate) {
        (_, _, _, None | Some(0)) => None,
        (BatStatus::Discharging, Some(n), _, Some(r)) => Some(hours_x3600(n, r)),
        (BatStatus::Charging, Some(n), Some(f), Some(r)) => Some(hours_x3600(f.saturating_sub(n), r)),
        _ => None,
    };
    let watts = read_abs(&dir.join("power_now")).map(|p| p as f32 / 1e6).or_else(|| {
        let (i, v) = (read_abs(&dir.join("current_now"))?, read_abs(&dir.join("voltage_now"))?);
        Some((i as f64 * v as f64 / 1e12) as f32)
    });
    Battery { percent, status, seconds_left, watts }
}

/// `root` is `/sys` normally (a tempdir in tests). Missing files leave fields unknown, never panic.
pub fn read_power(root: &Path) -> PowerState {
    let mut ac_online = false;
    let mut battery = None;
    for dir in power_supplies(root) {
        match read_str(&dir.join("type")).as_deref() {
            Some("Battery") if battery.is_none() => battery = Some(read_battery(&dir)),
            // USB covers USB-C PD chargers (e.g. `ucsi-source-psy-*`), which never show as Mains.
            Some("Mains" | "USB") => ac_online |= read_str(&dir.join("online")).as_deref() == Some("1"),
            _ => {}
        }
    }
    let profile = Profile::parse(&read_str(&root.join("firmware/acpi/platform_profile")).unwrap_or_default());
    PowerState { ac_online, battery, profile }
}

pub fn spawn(tx: Sender<Update>) {
    thread::spawn(move || {
        let sys = Path::new("/sys");
        for tick in 0u64.. {
            let started = Instant::now();
            if tx.send(Update::Power(read_power(sys))).is_err() {
                return;
            }
            if tick % EXTRA_EVERY == 0 && tx.send(Update::PowerExtra(read_power_extra(sys))).is_err() {
                return;
            }
            thread::sleep(Duration::from_secs(1).saturating_sub(started.elapsed()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sysfs(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (p, v) in files {
            let path = d.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, v).unwrap();
        }
        d
    }

    #[test]
    fn a_peripheral_battery_is_not_the_machine_battery() {
        // A desktop: no BAT*, only a wireless mouse (scope=Device), which sorts first.
        let d = sysfs(&[
            ("class/power_supply/AAA-mouse/type", "Battery\n"),
            ("class/power_supply/AAA-mouse/scope", "Device\n"),
            ("class/power_supply/AAA-mouse/capacity", "40\n"),
            ("class/power_supply/AAA-mouse/status", "Discharging\n"),
            ("class/power_supply/AAA-mouse/model_name", "Wireless Mouse\n"),
        ]);
        assert!(read_power(d.path()).battery.is_none());
        assert!(read_power_extra(d.path()).model.is_none());
    }

    #[test]
    fn charge_based_battery_discharging() {
        let d = sysfs(&[
            ("class/power_supply/BAT1/type", "Battery\n"),
            ("class/power_supply/BAT1/status", "Discharging\n"),
            ("class/power_supply/BAT1/capacity", "84\n"),
            ("class/power_supply/BAT1/charge_now", "3213000\n"),
            ("class/power_supply/BAT1/charge_full", "3826000\n"),
            ("class/power_supply/BAT1/current_now", "1987000\n"),
            ("class/power_supply/BAT1/voltage_now", "15625000\n"),
            ("class/power_supply/ACAD/type", "Mains\n"),
            ("class/power_supply/ACAD/online", "0\n"),
            ("firmware/acpi/platform_profile", "balanced\n"),
        ]);
        let p = read_power(d.path());
        assert!(!p.ac_online);
        assert_eq!(p.profile, Profile::Balanced);
        let b = p.battery.unwrap();
        assert_eq!(b.percent, 84.0);
        assert_eq!(b.status, BatStatus::Discharging);
        assert_eq!(b.seconds_left, Some(5821)); // 3213000/1987000 h
        assert!((b.watts.unwrap() - 31.05).abs() < 0.1);
    }

    #[test]
    fn energy_based_battery_charging_counts_to_full() {
        let d = sysfs(&[
            ("class/power_supply/BAT0/type", "Battery\n"),
            ("class/power_supply/BAT0/status", "Charging\n"),
            ("class/power_supply/BAT0/capacity", "50\n"),
            ("class/power_supply/BAT0/energy_now", "30000000\n"),
            ("class/power_supply/BAT0/energy_full", "60000000\n"),
            ("class/power_supply/BAT0/power_now", "30000000\n"),
            ("class/power_supply/AC/type", "Mains\n"),
            ("class/power_supply/AC/online", "1\n"),
        ]);
        let p = read_power(d.path());
        assert!(p.ac_online);
        assert_eq!(p.battery.unwrap().seconds_left, Some(3600));
        assert!(matches!(p.profile, Profile::Unknown(_)));
    }

    #[test]
    fn no_battery_no_panic() {
        let d = sysfs(&[("class/power_supply/AC/type", "Mains\n"), ("class/power_supply/AC/online", "1\n")]);
        let p = read_power(d.path());
        assert!(p.battery.is_none());
        assert!(p.ac_online);
    }

    #[test]
    fn zero_current_means_unknown_time() {
        let d = sysfs(&[
            ("class/power_supply/BAT1/type", "Battery\n"),
            ("class/power_supply/BAT1/status", "Full\n"),
            ("class/power_supply/BAT1/capacity", "100\n"),
            ("class/power_supply/BAT1/charge_now", "3826000\n"),
            ("class/power_supply/BAT1/current_now", "0\n"),
        ]);
        assert_eq!(read_power(d.path()).battery.unwrap().seconds_left, None);
    }

    #[test]
    fn usb_c_charger_counts_as_external_power() {
        let d = sysfs(&[
            ("class/power_supply/ACAD/type", "Mains\n"),
            ("class/power_supply/ACAD/online", "0\n"),
            ("class/power_supply/ucsi-source-psy-USBC000:001/type", "USB\n"),
            ("class/power_supply/ucsi-source-psy-USBC000:001/online", "1\n"),
        ]);
        assert!(read_power(d.path()).ac_online);
    }

    #[test]
    fn battery_extra_health_cycles_volts_amps_model() {
        let d = sysfs(&[
            ("class/power_supply/ACAD/type", "Mains\n"),
            ("class/power_supply/BAT1/type", "Battery\n"),
            ("class/power_supply/BAT1/charge_full", "3826000\n"),
            ("class/power_supply/BAT1/charge_full_design", "4416000\n"),
            ("class/power_supply/BAT1/cycle_count", "0\n"),
            ("class/power_supply/BAT1/model_name", "A32-K55\n"),
            ("class/power_supply/BAT1/voltage_now", "15580000\n"),
            ("class/power_supply/BAT1/current_now", "1062000\n"),
        ]);
        let x = read_power_extra(d.path());
        assert!((x.health_pct.unwrap() - 86.64).abs() < 0.01, "{:?}", x.health_pct);
        assert_eq!(x.cycles, Some(0));
        assert_eq!(x.model.as_deref(), Some("A32-K55"));
        assert!((x.voltage_v.unwrap() - 15.58).abs() < 1e-4);
        assert!((x.current_a.unwrap() - 1.062).abs() < 1e-4);
    }

    #[test]
    fn battery_extra_energy_based_and_absent() {
        let d = sysfs(&[
            ("class/power_supply/BAT0/type", "Battery\n"),
            ("class/power_supply/BAT0/energy_full", "45000000\n"),
            ("class/power_supply/BAT0/energy_full_design", "50000000\n"),
        ]);
        let x = read_power_extra(d.path());
        assert_eq!(x.health_pct, Some(90.0));
        assert_eq!((x.cycles, x.voltage_v, x.model), (None, None, None));
        let none = tempfile::tempdir().unwrap();
        assert_eq!(read_power_extra(none.path()), PowerExtra::default());
    }

    #[test]
    fn empty_root_is_all_unknown() {
        let d = tempfile::tempdir().unwrap();
        let p = read_power(d.path());
        assert!(!p.ac_online && p.battery.is_none());
        assert!(matches!(p.profile, Profile::Unknown(_)));
    }
}
