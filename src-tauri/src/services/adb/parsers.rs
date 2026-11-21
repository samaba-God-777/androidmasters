//! Pure parsers for adb / fastboot output (unit tested with real-world samples).

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Android booted, USB debugging authorized.
    Adb,
    Unauthorized,
    Offline,
    NoPermissions,
    Recovery,
    Sideload,
    Rescue,
    Bootloader,
    Fastboot,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbEntry {
    pub serial: String,
    pub mode: Mode,
    pub state: String,
    pub usb_path: Option<String>,
    pub product: Option<String>,
    pub model: Option<String>,
    pub codename: Option<String>,
    pub transport_id: Option<String>,
}

const ATTR_KEYS: &[&str] = &["usb:", "product:", "model:", "device:", "transport_id:"];

pub fn parse_devices(output: &str) -> Vec<AdbEntry> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("List of devices") && !l.starts_with('*') && !l.starts_with("adb "))
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 2 {
                return None;
            }
            let attr_start = tokens.iter().skip(1).position(|t| ATTR_KEYS.iter().any(|k| t.starts_with(k))).map_or(tokens.len(), |p| p + 1);
            let state = tokens[1..attr_start].join(" ");
            let attr = |key: &str| tokens[attr_start..].iter().find_map(|t| t.strip_prefix(key)).map(str::to_string);
            let mode = match state.as_str() {
                "device" => Mode::Adb,
                "unauthorized" => Mode::Unauthorized,
                "offline" => Mode::Offline,
                "recovery" => Mode::Recovery,
                "sideload" => Mode::Sideload,
                "rescue" => Mode::Rescue,
                "bootloader" => Mode::Bootloader,
                s if s.starts_with("no permissions") => Mode::NoPermissions,
                _ => Mode::Unknown,
            };
            Some(AdbEntry {
                serial: tokens[0].to_string(),
                mode,
                state,
                usb_path: attr("usb:"),
                product: attr("product:"),
                model: attr("model:").map(|m| m.replace('_', " ")),
                codename: attr("device:"),
                transport_id: attr("transport_id:"),
            })
        })
        .collect()
}

pub fn parse_fastboot_devices(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|l| {
            let mut t = l.split_whitespace();
            match (t.next(), t.next()) {
                (Some(serial), Some("fastboot" | "fastbootd")) => Some(serial.to_string()),
                _ => None,
            }
        })
        .collect()
}

pub fn parse_getprop(output: &str) -> BTreeMap<String, String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix('[')?;
            let (key, value) = rest.split_once("]: [")?;
            Some((key.to_string(), value.strip_suffix(']')?.to_string()))
        })
        .collect()
}

/// `fastboot getvar <name>` prints "name: value" (or "(bootloader) name:value") on stderr.
pub fn parse_getvar(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|l| {
        let l = l.trim().trim_start_matches("(bootloader)").trim();
        let value = l.strip_prefix(name)?.strip_prefix(':')?.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Battery {
    pub level: Option<u8>,
    pub status: Option<&'static str>,
    pub health: Option<&'static str>,
    pub temperature_c: Option<f32>,
    pub source: Option<String>,
}

pub fn parse_battery(output: &str) -> Battery {
    let mut v: BTreeMap<String, String> = BTreeMap::new();
    for line in output.lines() {
        if let Some((k, val)) = line.trim().split_once(':') {
            v.insert(k.trim().to_lowercase(), val.trim().to_string());
        }
    }
    let int = |k: &str| v.get(k).and_then(|s| s.parse::<i64>().ok());
    let scale = int("scale").filter(|s| *s > 0).unwrap_or(100);
    let sources: Vec<&str> = ["ac", "usb", "wireless", "dock"]
        .into_iter()
        .filter(|s| v.get(&format!("{s} powered")).is_some_and(|x| x == "true"))
        .collect();
    Battery {
        level: int("level").map(|l| ((l * 100) / scale).clamp(0, 100) as u8),
        status: int("status").and_then(|s| match s {
            2 => Some("Cargando"),
            3 => Some("Descargando"),
            4 => Some("Sin cargar"),
            5 => Some("Completa"),
            _ => None,
        }),
        health: int("health").and_then(|h| match h {
            2 => Some("Buena"),
            3 => Some("Sobrecalentada"),
            4 => Some("Agotada"),
            5 => Some("Sobretensión"),
            6 => Some("Fallo"),
            7 => Some("Fría"),
            _ => None,
        }),
        temperature_c: int("temperature").map(|t| t as f32 / 10.0),
        source: Some(if sources.is_empty() { "Batería".to_string() } else { sources.join(", ").to_uppercase() }),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Storage {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// `df -k /data`; toybox may wrap long filesystem names onto two lines.
pub fn parse_df(output: &str, mount: &str) -> Option<Storage> {
    let tokens: Vec<&str> = output.lines().filter(|l| !l.to_lowercase().starts_with("filesystem")).flat_map(str::split_whitespace).collect();
    tokens.windows(6).find_map(|w| {
        if w[5] != mount {
            return None;
        }
        let total = w[1].parse::<u64>().ok()?;
        let avail = w[3].parse::<u64>().ok()?;
        Some(Storage { total_bytes: total * 1024, available_bytes: avail * 1024 })
    })
}

#[cfg(test)]
pub mod samples {
    pub const PIXEL_GETPROP: &str = "[ro.product.manufacturer]: [Google]\n[ro.product.brand]: [google]\n[ro.product.model]: [Pixel 7]\n\
[ro.product.name]: [panther]\n[ro.product.device]: [panther]\n[ro.build.version.release]: [14]\n[ro.build.version.sdk]: [34]\n\
[ro.build.version.security_patch]: [2024-02-05]\n[ro.build.display.id]: [UQ1A.240205.004]\n[ro.serialno]: [28141FDH2000K7]\n\
[ro.boot.flash.locked]: [1]\n[sys.boot_completed]: [1]\n";
    pub const BATTERY: &str = "Current Battery Service state:\n  AC powered: false\n  USB powered: true\n  status: 2\n  health: 2\n  level: 76\n  scale: 100\n  temperature: 312\n";
    pub const DF: &str = "Filesystem       1K-blocks     Used Available Use% Mounted on\n/dev/block/dm-48 113281488 98765432  14516056  88% /data\n";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_with_every_state() {
        let out = "List of devices attached\n\
28141FDH2000K7         device usb:1-1 product:panther model:Pixel_7 device:panther transport_id:3\n\
R58N12ABCDE            unauthorized usb:1-2 transport_id:4\n\
0123456789ABCDEF       no permissions (missing udev rules? user is in the plugdev group); see [http://developer.android.com/tools/device.html] usb:1-3 transport_id:5\n\
192.168.1.20:5555      offline transport_id:6\n\
ZY22ABC                recovery usb:2-1 transport_id:7\n";
        let d = parse_devices(out);
        assert_eq!(d.len(), 5);
        assert_eq!((d[0].mode, d[0].model.as_deref(), d[0].usb_path.as_deref()), (Mode::Adb, Some("Pixel 7"), Some("1-1")));
        assert_eq!(d[1].mode, Mode::Unauthorized);
        assert_eq!((d[2].mode, d[2].usb_path.as_deref()), (Mode::NoPermissions, Some("1-3")));
        assert_eq!(d[3].mode, Mode::Offline);
        assert_eq!(d[4].mode, Mode::Recovery);
        assert!(parse_devices("List of devices attached\n\n").is_empty());
        assert!(parse_devices("* daemon not running; starting now at tcp:5037\n* daemon started successfully\nList of devices attached\n").is_empty());
    }

    #[test]
    fn getprop_battery_df_fastboot() {
        let p = parse_getprop(samples::PIXEL_GETPROP);
        assert_eq!(p["ro.product.model"], "Pixel 7");
        let b = parse_battery(samples::BATTERY);
        assert_eq!((b.level, b.status, b.health, b.temperature_c), (Some(76), Some("Cargando"), Some("Buena"), Some(31.2)));
        assert_eq!(b.source.as_deref(), Some("USB"));
        assert_eq!(parse_df(samples::DF, "/data").unwrap().available_bytes, 14516056 * 1024);
        assert_eq!(parse_df("Filesystem 1K-blocks Used Available Use% Mounted on\n/dev/block/by-name/userdata\n 1000 400 600 40% /data\n", "/data").unwrap().total_bytes, 1000 * 1024);
        assert_eq!(parse_fastboot_devices("ABC\tfastboot\nDEF\tfastbootd\n"), vec!["ABC", "DEF"]);
        assert_eq!(parse_getvar("product: panther\nFinished. Total time: 0.001s", "product").as_deref(), Some("panther"));
        assert_eq!(parse_getvar("(bootloader) unlocked:no", "unlocked").as_deref(), Some("no"));
    }
}
