//! Command policy for `adb` and `fastboot`.
//!
//! * **Allow-list, not deny-list:** only known commands run; everything else is rejected.
//! * **Hard blocks:** anything touching anti-theft (FRP, Find My Device, Knox Guard), account
//!   databases, setup-wizard state, IMEI/baseband identifiers or carrier locks is FORBIDDEN and no
//!   setting can enable it.
//! * Destructive commands are only allowed together with an explicit, single-use confirmation
//!   (enforced by the services that run them).

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    ReadOnly,
    StateChange,
    Destructive,
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Decision {
    pub allowed: bool,
    pub risk: Risk,
    pub reason: String,
}

impl Decision {
    fn allow(risk: Risk) -> Self {
        Self { allowed: true, risk, reason: String::new() }
    }
    fn deny(risk: Risk, reason: impl Into<String>) -> Self {
        Self { allowed: false, risk, reason: reason.into() }
    }
}

pub const FORBIDDEN_REASON: &str = "Bloqueado por la política de seguridad: el comando afecta la protección antirrobo \
(FRP, Find My Device, Knox Guard), datos de cuentas, identificadores IMEI/baseband o bloqueos de operador.";

const FORBIDDEN_KEYWORDS: &[&str] = &[
    "frp", "persistent", "gsf.login", "setupwizard", "user_setup_complete", "device_provisioned", "accounts.db",
    "accounts_ce.db", "accounts_de.db", "iphonesubinfo", "imei", "meid", "efs", "nvram", "nvdata", "modemst", "qcn",
    "simlock", "network_unlock", "kgclient", "knox", "finddevice", "findmyphone", "com.samsung.android.fmm", "clouddpc",
    "managedprovisioning", "oeminfo", "sys.usb.config", "diag",
];

const ADB_READ: &[&str] = &["devices", "version", "get-state", "get-serialno", "get-devpath", "start-server"];
const ADB_REBOOT_TARGETS: &[&str] = &["recovery", "bootloader", "sideload", "sideload-auto-reboot", "fastboot"];
const ADB_WAIT: &[&str] =
    &["wait-for-device", "wait-for-usb-device", "wait-for-recovery", "wait-for-sideload", "wait-for-bootloader", "wait-for-any"];
const DUMPSYS_SERVICES: &[&str] =
    &["battery", "batterystats", "diskstats", "meminfo", "cpuinfo", "thermalservice", "power", "usb", "account"];
/// Allowed only for AndroidMasters' own privacy-filtered checks; never for free-text commands.
pub const INTERNAL_ONLY: &[(&str, &str)] = &[("dumpsys", "account")];
const DF_PATHS: &[&str] = &["/data", "/system", "/vendor", "/cache", "/storage/emulated", "/sdcard", "/metadata"];
const PROC_FILES: &[&str] = &["/proc/version", "/proc/cpuinfo", "/proc/meminfo", "/proc/uptime", "/proc/loadavg"];
const SETTINGS_READ: &[(&str, &str)] = &[
    ("global", "device_name"),
    ("global", "adb_enabled"),
    ("global", "development_settings_enabled"),
    ("global", "boot_count"),
];
const SETTINGS_INTENTS: &[&str] = &[
    "android.settings.SETTINGS",
    "android.settings.SYNC_SETTINGS",
    "android.settings.DEVICE_INFO_SETTINGS",
    "android.settings.INTERNAL_STORAGE_SETTINGS",
];
const FASTBOOT_FLASHABLE: &[&str] = &[
    "bootloader", "radio", "boot", "init_boot", "vendor_boot", "recovery", "dtbo", "vbmeta", "vbmeta_system", "vbmeta_vendor",
    "system", "system_ext", "vendor", "vendor_dlkm", "product", "odm", "super", "pvmfw",
];
const FASTBOOT_ERASABLE: &[&str] = &["userdata", "cache", "metadata"];
const FASTBOOT_PROTECTED: &[&str] = &[
    "frp", "config", "persist", "persistent", "modem", "modemst1", "modemst2", "fsg", "fsc", "efs", "sec_efs", "nvram",
    "nvdata", "nvcfg", "protect1", "protect2", "oeminfo", "devinfo", "misc", "param", "steady", "keystore", "keymaster", "cid",
    "proinfo",
];
const FASTBOOT_REBOOT_TARGETS: &[&str] = &["bootloader", "recovery", "fastboot"];

fn safe_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9._:/=,@%+\-]+$").expect("valid regex"))
}

fn unlock_code() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(0x)?[A-Za-z0-9]{8,64}$").expect("valid regex"))
}

fn has_forbidden_keyword(tokens: &[&str]) -> bool {
    let joined = tokens.join(" ").to_lowercase();
    FORBIDDEN_KEYWORDS.iter().any(|k| joined.contains(k))
}

fn strip_slot(name: &str) -> String {
    let lower = name.to_lowercase();
    lower.strip_suffix("_a").or_else(|| lower.strip_suffix("_b")).unwrap_or(&lower).to_string()
}

/// Validate `adb` arguments (without the executable and without `-s SERIAL`).
pub fn validate_adb(args: &[&str]) -> Decision {
    let Some((&command, rest)) = args.split_first() else {
        return Decision::deny(Risk::ReadOnly, "Comando adb vacío");
    };
    if command == "shell" {
        return validate_shell(rest);
    }
    if command == "sideload" {
        // Only argument is a local file path (not interpreted by the device).
        return if rest.len() == 1 && rest[0].to_lowercase().ends_with(".zip") {
            Decision::allow(Risk::Destructive)
        } else {
            Decision::deny(Risk::Destructive, "adb sideload requiere exactamente un paquete .zip")
        };
    }
    if has_forbidden_keyword(args) {
        return Decision::deny(Risk::Forbidden, FORBIDDEN_REASON);
    }
    if ADB_READ.contains(&command) {
        let ok = if command == "devices" { rest.is_empty() || rest == ["-l"] } else { rest.is_empty() };
        return if ok { Decision::allow(Risk::ReadOnly) } else { Decision::deny(Risk::ReadOnly, "Opciones no admitidas") };
    }
    if command == "kill-server" && rest.is_empty() {
        return Decision::allow(Risk::StateChange);
    }
    if ADB_WAIT.contains(&command) && rest.is_empty() {
        return Decision::allow(Risk::ReadOnly);
    }
    if command == "reboot" {
        return if rest.is_empty() || (rest.len() == 1 && ADB_REBOOT_TARGETS.contains(&rest[0])) {
            Decision::allow(Risk::StateChange)
        } else {
            Decision::deny(Risk::StateChange, "Destino de reinicio no admitido")
        };
    }
    if command == "logcat" {
        return validate_logcat(rest);
    }
    Decision::deny(Risk::ReadOnly, format!("El comando adb '{command}' no está en la lista permitida"))
}

fn validate_logcat(rest: &[&str]) -> Decision {
    if !rest.contains(&"-d") {
        return Decision::deny(Risk::ReadOnly, "logcat debe ser un volcado (-d)");
    }
    let mut i = 0;
    while i < rest.len() {
        let token = rest[i];
        let next = rest.get(i + 1).copied();
        match token {
            "-d" => i += 1,
            "-t" if next.is_some_and(|n| n.parse::<u32>().is_ok_and(|v| (1..=20_000).contains(&v))) => i += 2,
            "-b" if next.is_some_and(|n| ["main", "system", "crash", "events", "radio", "kernel", "default", "all"].contains(&n)) => {
                i += 2
            }
            "-v" if next.is_some_and(|n| ["brief", "threadtime", "time", "long", "process", "tag"].contains(&n)) => i += 2,
            t if t.len() == 3 && t.starts_with("*:") && "VDIWEF".contains(&t[2..]) => i += 1,
            other => return Decision::deny(Risk::ReadOnly, format!("Opción de logcat no admitida: {other}")),
        }
    }
    Decision::allow(Risk::ReadOnly)
}

/// Validate a device shell command given as tokens.
pub fn validate_shell(tokens: &[&str]) -> Decision {
    let Some((&cmd, rest)) = tokens.split_first() else {
        return Decision::deny(Risk::ReadOnly, "No se permiten shells interactivos");
    };
    if let Some(bad) = tokens.iter().find(|t| !safe_token().is_match(t)) {
        return Decision::deny(Risk::ReadOnly, format!("Caracteres no válidos en '{bad}' (no se admite sintaxis de shell)"));
    }
    // getprop is read-only; reading persist.* keys (e.g. boot reason history) is fine.
    if cmd == "getprop" && rest.len() <= 1 {
        return Decision::allow(Risk::ReadOnly);
    }
    if has_forbidden_keyword(tokens) {
        return Decision::deny(Risk::Forbidden, FORBIDDEN_REASON);
    }
    let read = Decision::allow(Risk::ReadOnly);
    match (cmd, rest) {
        ("dumpsys", [service]) if DUMPSYS_SERVICES.contains(service) => read,
        ("df", args) if args.iter().all(|a| *a == "-k" || *a == "-h" || DF_PATHS.contains(a)) => read,
        ("cat", [file]) if PROC_FILES.contains(file) => read,
        ("uptime" | "getenforce" | "id", []) => read,
        ("echo", args) if args.len() <= 4 => read,
        ("wm", ["size"] | ["density"]) => read,
        ("pm", ["list", "features"]) => read,
        ("settings", ["get", ns, key]) if SETTINGS_READ.contains(&(*ns, *key)) => read,
        ("am", ["start", "-a", intent]) if SETTINGS_INTENTS.contains(intent) => Decision::allow(Risk::StateChange),
        _ => Decision::deny(Risk::ReadOnly, format!("El comando de shell '{cmd}' no está en la lista permitida")),
    }
}

/// Validate `fastboot` arguments (without the executable and `-s SERIAL`).
pub fn validate_fastboot(args: &[&str]) -> Decision {
    let mut tokens = args;
    if tokens.is_empty() {
        return Decision::deny(Risk::ReadOnly, "Comando fastboot vacío");
    }
    let mut wipe = false;
    if tokens[0] == "-w" {
        wipe = true;
        tokens = &tokens[1..];
        if tokens.is_empty() {
            return Decision::allow(Risk::Destructive);
        }
    }
    let (command, rest) = (tokens[0], &tokens[1..]);
    if (command == "erase" || command == "flash") && !rest.is_empty() && FASTBOOT_PROTECTED.contains(&strip_slot(rest[0]).as_str()) {
        return Decision::deny(Risk::Forbidden, format!("La partición '{}' está protegida y no se puede modificar", rest[0]));
    }
    // Local file paths (flash/update) are not interpreted by the device.
    let checked: &[&str] = match command {
        "flash" => &tokens[..tokens.len().min(2)],
        "update" => &tokens[..1],
        _ => tokens,
    };
    if command != "getvar" && has_forbidden_keyword(checked) {
        return Decision::deny(Risk::Forbidden, FORBIDDEN_REASON);
    }
    if wipe && command != "update" {
        return Decision::deny(Risk::Destructive, "-w solo puede combinarse con 'update'");
    }
    match (command, rest) {
        ("devices", [] | ["-l"]) => Decision::allow(Risk::ReadOnly),
        ("getvar", [var]) if safe_token().is_match(var) => Decision::allow(Risk::ReadOnly),
        ("reboot", []) => Decision::allow(Risk::StateChange),
        ("reboot", [target]) if FASTBOOT_REBOOT_TARGETS.contains(target) => Decision::allow(Risk::StateChange),
        ("reboot-bootloader", []) => Decision::allow(Risk::StateChange),
        ("flashing", ["get_unlock_ability"]) => Decision::allow(Risk::ReadOnly),
        ("flashing", ["unlock" | "lock" | "unlock_critical" | "lock_critical"]) => Decision::allow(Risk::Destructive),
        ("oem", ["device-info"] | ["get_unlock_data"]) => Decision::allow(Risk::ReadOnly),
        ("oem", ["unlock", code]) if unlock_code().is_match(code) => Decision::allow(Risk::Destructive),
        ("erase", [part]) if FASTBOOT_ERASABLE.contains(&strip_slot(part).as_str()) => Decision::allow(Risk::Destructive),
        ("flash", [part, file])
            if FASTBOOT_FLASHABLE.contains(&strip_slot(part).as_str())
                && [".img", ".bin", ".mbn"].iter().any(|ext| file.to_lowercase().ends_with(ext)) =>
        {
            Decision::allow(Risk::Destructive)
        }
        ("update", [zip]) if zip.to_lowercase().ends_with(".zip") => Decision::allow(Risk::Destructive),
        _ => Decision::deny(Risk::ReadOnly, format!("El comando fastboot '{command}' no está en la lista permitida")),
    }
}

/// Parse a command typed by a technician in the console. Shell operators, quotes, global options,
/// destructive commands and internal-only queries are rejected.
pub fn parse_console_command(text: &str) -> Result<Vec<String>, String> {
    let text = text.trim();
    let text = text.strip_prefix("adb ").map(str::trim).unwrap_or(text);
    if text.is_empty() {
        return Err("Escriba un comando".into());
    }
    if text.chars().any(|c| ";|&`$<>\\\"'\n\r(){}*?!~".contains(c)) {
        return Err("No se admiten operadores de shell, comillas ni comodines".into());
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens[0].starts_with('-') {
        return Err("Seleccione el dispositivo en la aplicación en lugar de usar opciones globales".into());
    }
    if tokens[0] == "shell" && tokens.len() >= 3 && INTERNAL_ONLY.contains(&(tokens[1], tokens[2])) {
        return Err("Esta consulta expone datos personales; use el módulo de Cuentas, que muestra solo tipos y cantidades".into());
    }
    let decision = validate_adb(&tokens);
    if !decision.allowed {
        return Err(decision.reason);
    }
    if decision.risk >= Risk::Destructive {
        return Err("Las operaciones destructivas solo se ejecutan desde su módulo, con confirmación".into());
    }
    Ok(tokens.into_iter().map(String::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adb(cmd: &str) -> Decision {
        validate_adb(&cmd.split_whitespace().collect::<Vec<_>>())
    }
    fn fb(cmd: &str) -> Decision {
        validate_fastboot(&cmd.split_whitespace().collect::<Vec<_>>())
    }

    #[test]
    fn allows_read_only_diagnostics() {
        for cmd in ["devices -l", "get-state", "reboot recovery", "shell getprop", "shell getprop persist.sys.boot.reason.history",
                    "shell dumpsys battery", "shell df -k /data", "logcat -d -t 100 -v threadtime -b default"] {
            assert!(adb(cmd).allowed, "{cmd}");
        }
    }

    #[test]
    fn rejects_unknown_and_shell_syntax() {
        for cmd in ["push a /sdcard", "pull /data/system", "install x.apk", "root", "remount", "shell", "shell rm -rf /sdcard",
                    "shell getprop;reboot", "shell cat /sdcard/DCIM/a.jpg", "logcat", "reboot edl", "shell setprop ro.x 1"] {
            assert!(!adb(cmd).allowed, "{cmd}");
        }
    }

    #[test]
    fn frp_account_imei_and_antitheft_are_forbidden() {
        for cmd in [
            "shell content insert --uri content://settings/secure --bind name:s:user_setup_complete",
            "shell pm uninstall --user 0 com.google.android.gsf.login",
            "shell service call iphonesubinfo 1",
            "shell am start -n com.google.android.setupwizard/.Main",
            "shell pm disable-user com.samsung.android.kgclient",
            "shell setprop sys.usb.config diag,adb",
        ] {
            let d = adb(cmd);
            assert!(!d.allowed && d.risk == Risk::Forbidden, "{cmd}");
        }
        for cmd in ["erase frp", "erase config", "flash persist p.img", "erase modemst1", "flash frp_a x.img", "oem frp-erase"] {
            let d = fb(cmd);
            assert!(!d.allowed && d.risk == Risk::Forbidden, "{cmd}");
        }
    }

    #[test]
    fn fastboot_risk_levels() {
        assert_eq!(fb("getvar all").risk, Risk::ReadOnly);
        assert_eq!(fb("reboot recovery").risk, Risk::StateChange);
        assert_eq!(fb("-w").risk, Risk::Destructive);
        assert_eq!(fb("flashing unlock").risk, Risk::Destructive);
        assert!(fb("flash boot_a /x/prefs/boot.img").allowed, "'efs' inside a local path is not a keyword hit");
        assert!(!fb("boot twrp.img").allowed);
        assert!(!fb("erase system").allowed);
        assert!(!fb("-w reboot").allowed);
    }

    #[test]
    fn console_rejects_operators_destructive_and_internal_queries() {
        assert_eq!(parse_console_command("adb shell getprop ro.product.model").unwrap(), ["shell", "getprop", "ro.product.model"]);
        for bad in ["shell getprop | grep x", "shell 'reboot'", "-s ABC shell id", "sideload ota.zip", "shell rm x",
                    "shell dumpsys account"] {
            assert!(parse_console_command(bad).is_err(), "{bad}");
        }
        assert!(validate_shell(&["dumpsys", "account"]).allowed, "internal privacy-filtered check stays possible");
    }
}
