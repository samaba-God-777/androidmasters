//! Live device registry. Polls adb / fastboot, identifies authorized devices and reports
//! connections, disconnections and mode changes. Only detected facts are stored: fields the
//! device does not report stay empty and are listed in `missing`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::security::crypto::{mask_serial, Vault};
use crate::services::adb::client::Tools;
use crate::services::adb::parsers::{AdbEntry, Battery, Mode, Storage};
use crate::util::now;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceView {
    /// Anonymous, stable id (prefix of the keyed serial fingerprint). The UI never gets the serial.
    pub id: String,
    pub serial_masked: String,
    pub mode: Mode,
    pub network: bool,
    pub usb_path: Option<String>,
    pub manufacturer: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub product: Option<String>,
    pub codename: Option<String>,
    pub android_version: Option<String>,
    pub api_level: Option<u32>,
    pub security_patch: Option<String>,
    pub build_id: Option<String>,
    pub bootloader_locked: Option<bool>,
    pub battery: Option<Battery>,
    pub storage: Option<Storage>,
    pub identified: bool,
    pub missing: Vec<&'static str>,
    pub first_seen: String,
    pub last_seen: String,
    pub identify_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Connected,
    Disconnected,
    ModeChanged,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionEvent {
    pub at: String,
    pub kind: EventKind,
    pub device: Option<String>,
    pub mode: Option<Mode>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ToolStatus {
    pub adb_path: Option<String>,
    pub adb_version: Option<String>,
    pub fastboot_path: Option<String>,
    pub last_scan: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub devices: Vec<DeviceView>,
    pub tools: ToolStatus,
    pub events: Vec<ConnectionEvent>,
}

struct Tracked {
    serial: String,
    view: DeviceView,
}

#[derive(Default)]
struct Inner {
    devices: BTreeMap<String, Tracked>,
    tools: ToolStatus,
    events: VecDeque<ConnectionEvent>,
    adb_version_for: Option<String>,
    last_disconnect: BTreeMap<String, String>,
}

#[derive(Default)]
pub struct DeviceMonitor {
    inner: Mutex<Inner>,
}

pub struct ScanResult {
    pub changed: bool,
    pub events: Vec<ConnectionEvent>,
}

const MAX_EVENTS: usize = 200;

fn label(view: &DeviceView) -> String {
    match (&view.manufacturer, &view.model) {
        (Some(m), Some(model)) => format!("{m} {model}"),
        (None, Some(model)) => model.clone(),
        _ => view.serial_masked.clone(),
    }
}

pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Adb => "Android (ADB autorizado)",
        Mode::Unauthorized => "ADB sin autorizar",
        Mode::Offline => "Fuera de línea",
        Mode::NoPermissions => "Sin permisos USB",
        Mode::Recovery => "Recovery",
        Mode::Sideload => "Recovery (sideload)",
        Mode::Rescue => "Rescate",
        Mode::Bootloader | Mode::Fastboot => "Bootloader (Fastboot)",
        Mode::Unknown => "Desconocido",
    }
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn first(props: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| props.get(*k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()))
}

impl DeviceMonitor {
    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        Snapshot {
            devices: inner.devices.values().map(|t| t.view.clone()).collect(),
            tools: inner.tools.clone(),
            events: inner.events.iter().rev().cloned().collect(),
        }
    }

    /// When the device with this id was last seen disconnecting (e.g. rebooting after a reset).
    pub fn last_disconnect(&self, id: &str) -> Option<String> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).last_disconnect.get(id).cloned()
    }

    /// Full serial for internal use by services (never returned to the UI except to admins on request).
    pub fn serial_for(&self, id: &str) -> AppResult<String> {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner
            .devices
            .values()
            .find(|t| t.view.id == id)
            .map(|t| t.serial.clone())
            .ok_or_else(|| AppError::Device("El dispositivo ya no está conectado.".into()))
    }

    pub fn view(&self, id: &str) -> AppResult<DeviceView> {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.devices.values().find(|t| t.view.id == id).map(|t| t.view.clone()).ok_or_else(|| AppError::Device("El dispositivo ya no está conectado.".into()))
    }

    fn push_events(inner: &mut Inner, events: &[ConnectionEvent]) {
        for e in events {
            inner.events.push_back(e.clone());
        }
        while inner.events.len() > MAX_EVENTS {
            inner.events.pop_front();
        }
    }

    /// One detection pass. `conn` is used to remember identified devices and log identifications.
    pub fn scan(&self, tools: &Tools, conn: &Connection, vault: &Vault) -> ScanResult {
        let mut events = Vec::new();
        let mut changed = false;
        let previous_error;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let adb_path = tools.adb.as_ref().map(|p| p.display().to_string());
            if inner.tools.adb_path != adb_path {
                changed = true;
                inner.adb_version_for = None;
            }
            inner.tools.adb_path = adb_path.clone();
            inner.tools.fastboot_path = tools.fastboot.as_ref().map(|p| p.display().to_string());
            if adb_path.is_some() && inner.adb_version_for != adb_path {
                inner.tools.adb_version = tools.adb_version();
                inner.adb_version_for = adb_path;
            }
            previous_error = inner.tools.last_error.clone();
        }

        let (entries, error) = match (tools.adb.is_some(), tools.devices()) {
            (false, _) => (Vec::new(), Some("No se encontró adb (Android SDK Platform-Tools).".to_string())),
            (true, Ok(e)) => (e, None),
            (true, Err(e)) => (Vec::new(), Some(e.to_string())),
        };
        let mut found: Vec<AdbEntry> = entries;
        if let Ok(fb) = tools.fastboot_devices() {
            for serial in fb {
                if !found.iter().any(|e| e.serial == serial) {
                    found.push(AdbEntry { serial, mode: Mode::Fastboot, state: "fastboot".into(), usb_path: None, product: None, model: None, codename: None, transport_id: None });
                }
            }
        }
        if error != previous_error {
            changed = true;
            if let Some(message) = &error {
                events.push(ConnectionEvent { at: now(), kind: EventKind::Error, device: None, mode: None, message: message.clone() });
            }
        }

        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.tools.last_error = error;
        inner.tools.last_scan = Some(now());

        let current: Vec<String> = found.iter().map(|e| e.serial.clone()).collect();
        let gone: Vec<String> = inner.devices.keys().filter(|s| !current.contains(s)).cloned().collect();
        for serial in gone {
            if let Some(t) = inner.devices.remove(&serial) {
                changed = true;
                inner.last_disconnect.insert(t.view.id.clone(), now());
                events.push(ConnectionEvent {
                    at: now(),
                    kind: EventKind::Disconnected,
                    device: Some(label(&t.view)),
                    mode: Some(t.view.mode),
                    message: format!("{} se desconectó", label(&t.view)),
                });
            }
        }

        for entry in found {
            let existing = inner.devices.get(&entry.serial).map(|t| (t.view.mode, t.view.first_seen.clone(), t.view.clone()));
            match existing {
                Some((mode, _, _)) if mode == entry.mode => {
                    if let Some(t) = inner.devices.get_mut(&entry.serial) {
                        t.view.last_seen = now();
                    }
                }
                other => {
                    // New device or mode change: identify outside the lock-free fast path.
                    drop(inner);
                    let mut view = identify(tools, vault, &entry);
                    if let Some((old_mode, first_seen, old)) = &other {
                        view.first_seen = first_seen.clone();
                        carry_identity(old, &mut view);
                        events.push(ConnectionEvent {
                            at: now(),
                            kind: EventKind::ModeChanged,
                            device: Some(label(&view)),
                            mode: Some(view.mode),
                            message: format!("{}: {} → {}", label(&view), mode_label(*old_mode), mode_label(view.mode)),
                        });
                    } else {
                        events.push(ConnectionEvent {
                            at: now(),
                            kind: EventKind::Connected,
                            device: Some(label(&view)),
                            mode: Some(view.mode),
                            message: format!("{} conectado · {}", label(&view), mode_label(view.mode)),
                        });
                    }
                    if view.identified {
                        if let Err(e) = remember(conn, vault, &entry.serial, &view) {
                            log::warn!("Could not store device: {e}");
                        }
                    }
                    changed = true;
                    inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
                    inner.devices.insert(entry.serial.clone(), Tracked { serial: entry.serial, view });
                }
            }
        }
        Self::push_events(&mut inner, &events);
        ScanResult { changed, events }
    }

    /// Re-reads battery and storage (and identity) of a connected, authorized device.
    pub fn refresh(&self, id: &str, tools: &Tools, conn: &Connection, vault: &Vault) -> AppResult<DeviceView> {
        let serial = self.serial_for(id)?;
        let old = self.view(id)?;
        if old.mode != Mode::Adb {
            return Err(AppError::Device(format!("Solo se puede leer con ADB autorizado. Estado actual: {}.", mode_label(old.mode))));
        }
        let entry = AdbEntry {
            serial: serial.clone(), mode: old.mode, state: "device".into(), usb_path: old.usb_path.clone(), product: old.product.clone(),
            model: old.model.clone(), codename: old.codename.clone(), transport_id: None,
        };
        let mut view = identify(tools, vault, &entry);
        view.first_seen = old.first_seen.clone();
        if let Some(err) = &view.identify_error {
            return Err(AppError::Device(err.clone()));
        }
        remember(conn, vault, &serial, &view)?;
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(t) = inner.devices.get_mut(&serial) {
            t.view = view.clone();
        }
        Ok(view)
    }
}

fn carry_identity(old: &DeviceView, new: &mut DeviceView) {
    macro_rules! keep {
        ($($f:ident),*) => { $( if new.$f.is_none() { new.$f = old.$f.clone(); } )* };
    }
    keep!(manufacturer, brand, model, product, codename, android_version, api_level, security_patch, build_id);
}

fn identify(tools: &Tools, vault: &Vault, entry: &AdbEntry) -> DeviceView {
    let fingerprint = vault.fingerprint(&entry.serial);
    let mut view = DeviceView {
        id: fingerprint[..16].to_string(),
        serial_masked: mask_serial(&entry.serial),
        mode: entry.mode,
        network: entry.serial.contains(':') && entry.usb_path.is_none(),
        usb_path: entry.usb_path.clone(),
        manufacturer: None,
        brand: None,
        model: entry.model.clone(),
        product: entry.product.clone(),
        codename: entry.codename.clone(),
        android_version: None,
        api_level: None,
        security_patch: None,
        build_id: None,
        bootloader_locked: None,
        battery: None,
        storage: None,
        identified: false,
        missing: Vec::new(),
        first_seen: now(),
        last_seen: now(),
        identify_error: None,
    };
    match entry.mode {
        Mode::Adb | Mode::Recovery => match tools.getprop(&entry.serial) {
            Ok(p) => {
                view.manufacturer = first(&p, &["ro.product.manufacturer", "ro.product.vendor.manufacturer"]).map(|m| title_case(&m));
                view.brand = first(&p, &["ro.product.brand", "ro.product.vendor.brand"]);
                view.model = first(&p, &["ro.product.model", "ro.product.vendor.model"]).or(view.model);
                view.product = first(&p, &["ro.product.name"]).or(view.product);
                view.codename = first(&p, &["ro.product.device", "ro.build.product"]).or(view.codename);
                view.android_version = first(&p, &["ro.build.version.release", "ro.build.version.release_or_codename"]);
                view.api_level = first(&p, &["ro.build.version.sdk"]).and_then(|v| v.parse().ok());
                view.security_patch = first(&p, &["ro.build.version.security_patch"]);
                view.build_id = first(&p, &["ro.build.display.id", "ro.build.id"]);
                view.bootloader_locked = match first(&p, &["ro.boot.flash.locked"]).as_deref() {
                    Some("1") => Some(true),
                    Some("0") => Some(false),
                    _ => match first(&p, &["ro.boot.vbmeta.device_state"]).as_deref() {
                        Some("locked") => Some(true),
                        Some("unlocked") => Some(false),
                        _ => None,
                    },
                };
                view.identified = view.model.is_some();
                if entry.mode == Mode::Adb {
                    view.battery = tools.battery(&entry.serial);
                    view.storage = tools.storage(&entry.serial);
                }
            }
            Err(e) => view.identify_error = Some(e.to_string()),
        },
        Mode::Fastboot | Mode::Bootloader => {
            view.codename = tools.getvar(&entry.serial, "product").or(view.codename);
            view.bootloader_locked = match tools.getvar(&entry.serial, "unlocked").as_deref() {
                Some("yes") => Some(false),
                Some("no") => Some(true),
                _ => None,
            };
        }
        _ => {}
    }
    let fields: [(&'static str, bool); 6] = [
        ("Fabricante", view.manufacturer.is_none()),
        ("Modelo", view.model.is_none()),
        ("Versión de Android", view.android_version.is_none()),
        ("Nivel de API", view.api_level.is_none()),
        ("Parche de seguridad", view.security_patch.is_none()),
        ("Batería", view.battery.is_none()),
    ];
    view.missing = fields.into_iter().filter(|(_, m)| *m).map(|(n, _)| n).collect();
    view
}

/// Stores the identified device (encrypted serial + fingerprint) and logs the identification.
fn remember(conn: &Connection, vault: &Vault, serial: &str, view: &DeviceView) -> AppResult<()> {
    let fingerprint = vault.fingerprint(serial);
    let ts = now();
    conn.execute(
        "INSERT INTO devices (fingerprint, serial_masked, serial_encrypted, manufacturer, brand, model, codename, android_version, api_level, security_patch, first_seen, last_seen) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11) \
         ON CONFLICT(fingerprint) DO UPDATE SET manufacturer = excluded.manufacturer, brand = excluded.brand, model = excluded.model, \
         codename = excluded.codename, android_version = excluded.android_version, api_level = excluded.api_level, \
         security_patch = excluded.security_patch, last_seen = excluded.last_seen",
        params![
            fingerprint, view.serial_masked, vault.encrypt(serial)?, view.manufacturer.clone().unwrap_or_default(), view.brand.clone().unwrap_or_default(),
            view.model.clone().unwrap_or_default(), view.codename.clone().unwrap_or_default(), view.android_version.clone().unwrap_or_default(),
            view.api_level, view.security_patch.clone().unwrap_or_default(), ts
        ],
    )?;
    let device_id: i64 = conn.query_row("SELECT id FROM devices WHERE fingerprint = ?1", params![fingerprint], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO operations (started_at, finished_at, duration_ms, device_id, operation, category, result, details_json) \
         VALUES (?1, ?1, 0, ?2, 'Identificación del dispositivo', 'usb', 'success', ?3)",
        params![ts, device_id, serde_json::json!({ "mode": view.mode, "missing": view.missing }).to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::services::adb::parsers::samples;
    use crate::services::adb::runner::fake::FakeRunner;
    use std::path::PathBuf;
    use std::sync::Arc;

    const PIXEL: &str = "28141FDH2000K7";

    fn pixel_runner() -> Arc<FakeRunner> {
        let r = Arc::new(FakeRunner::default());
        r.on("adb version", "Android Debug Bridge version 1.0.41\nVersion 37.0.1\n");
        r.on("adb devices -l", &format!("List of devices attached\n{PIXEL} device usb:1-1 product:panther model:Pixel_7 device:panther transport_id:3\n"));
        r.on("fastboot devices", "");
        r.on(&format!("adb -s {PIXEL} shell getprop"), samples::PIXEL_GETPROP);
        r.on(&format!("adb -s {PIXEL} shell dumpsys battery"), samples::BATTERY);
        r.on(&format!("adb -s {PIXEL} shell df -k /data"), samples::DF);
        r
    }

    fn tools(r: Arc<FakeRunner>) -> Tools {
        Tools { runner: r, adb: Some(PathBuf::from("/x/adb")), fastboot: Some(PathBuf::from("/x/fastboot")) }
    }

    #[test]
    fn identifies_and_never_exposes_the_serial() {
        let db = Database::open_in_memory().unwrap();
        let vault = Vault::ephemeral();
        let monitor = DeviceMonitor::default();
        let result = monitor.scan(&tools(pixel_runner()), &db.conn(), &vault);
        assert!(result.changed);
        assert!(matches!(result.events[0].kind, EventKind::Connected));
        let snap = monitor.snapshot();
        let d = &snap.devices[0];
        assert_eq!((d.manufacturer.as_deref(), d.model.as_deref(), d.android_version.as_deref()), (Some("Google"), Some("Pixel 7"), Some("14")));
        assert_eq!((d.api_level, d.security_patch.as_deref(), d.bootloader_locked), (Some(34), Some("2024-02-05"), Some(true)));
        assert_eq!(d.battery.as_ref().unwrap().level, Some(76));
        assert!(d.missing.is_empty());
        let json = serde_json::to_string(&snap).unwrap();
        assert!(!json.contains(PIXEL), "the UI snapshot never contains the full serial");
        assert_eq!(monitor.serial_for(&d.id).unwrap(), PIXEL);
        let conn = db.conn();
        let stored: String = conn.query_row("SELECT serial_encrypted FROM devices", [], |r| r.get(0)).unwrap();
        assert_eq!(vault.decrypt(&stored).unwrap(), PIXEL);
        let ops: i64 = conn.query_row("SELECT COUNT(*) FROM operations WHERE category = 'usb'", [], |r| r.get(0)).unwrap();
        assert_eq!(ops, 1);
    }

    #[test]
    fn unchanged_scan_is_quiet_then_disconnect_and_mode_change_are_reported() {
        let db = Database::open_in_memory().unwrap();
        let vault = Vault::ephemeral();
        let runner = pixel_runner();
        let monitor = DeviceMonitor::default();
        monitor.scan(&tools(runner.clone()), &db.conn(), &vault);
        assert!(!monitor.scan(&tools(runner.clone()), &db.conn(), &vault).changed);
        runner.on("adb devices -l", &format!("List of devices attached\n{PIXEL} recovery usb:1-1 transport_id:9\n"));
        runner.on(&format!("adb -s {PIXEL} shell getprop"), "");
        let r = monitor.scan(&tools(runner.clone()), &db.conn(), &vault);
        assert!(matches!(r.events[0].kind, EventKind::ModeChanged));
        assert_eq!(monitor.snapshot().devices[0].model.as_deref(), Some("Pixel 7"), "identity carried over");
        runner.on("adb devices -l", "List of devices attached\n\n");
        let r = monitor.scan(&tools(runner), &db.conn(), &vault);
        assert!(matches!(r.events[0].kind, EventKind::Disconnected));
        assert!(monitor.snapshot().devices.is_empty());
        let id = &r.events[0].device;
        assert!(id.is_some());
    }

    #[test]
    fn unauthorized_devices_are_listed_but_not_identified() {
        let db = Database::open_in_memory().unwrap();
        let r = Arc::new(FakeRunner::default());
        r.on("adb devices -l", "List of devices attached\nR58N12ABCDE unauthorized usb:1-2 transport_id:4\n").on("fastboot devices", "");
        let monitor = DeviceMonitor::default();
        monitor.scan(&tools(r), &db.conn(), &Vault::ephemeral());
        let d = &monitor.snapshot().devices[0];
        assert_eq!(d.mode, Mode::Unauthorized);
        assert!(!d.identified);
        assert!(d.missing.contains(&"Fabricante"));
        let stored: i64 = db.conn().query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, 0, "nothing is stored for devices that were not identified");
    }

    #[test]
    fn missing_adb_is_reported_once() {
        let db = Database::open_in_memory().unwrap();
        let monitor = DeviceMonitor::default();
        let none = Tools { runner: Arc::new(FakeRunner::default()), adb: None, fastboot: None };
        let first = monitor.scan(&none, &db.conn(), &Vault::ephemeral());
        assert!(matches!(first.events[0].kind, EventKind::Error));
        assert!(monitor.scan(&none, &db.conn(), &Vault::ephemeral()).events.is_empty(), "the same error is not repeated");
        assert!(monitor.snapshot().tools.last_error.unwrap().contains("Platform-Tools"));
    }
}
