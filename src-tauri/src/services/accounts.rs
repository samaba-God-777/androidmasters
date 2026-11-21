//! Privacy-preserving account presence check.
//!
//! Reads `dumpsys account` and keeps ONLY account *types* and *counts* ("Google: 1"). Account names
//! (e-mail addresses) are discarded inside the parser: they are never returned, logged or stored.
//! Nothing here signs in, signs out or removes an account.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::adb::client::Tools;

/// Known account types. `protects` = linked to Factory Reset Protection / anti-theft.
const TYPES: &[(&str, &str, bool)] = &[
    ("com.google", "Google", true),
    ("com.osp.app.signin", "Samsung Account", true),
    ("com.xiaomi", "Xiaomi Account", true),
    ("com.huawei.hwid", "HUAWEI ID", true),
    ("com.hihonor.id", "HONOR ID", true),
    ("com.motorola.blur.service.bsutils.motorola1", "Motorola ID", false),
    ("com.lenovo.lsf.account", "Lenovo ID", false),
    ("com.heytap.usercenter", "HeyTap (OPPO / realme / OnePlus)", true),
    ("com.oplus.account", "OPPO / realme / OnePlus Account", true),
    ("com.coloros.account", "OPPO Account", true),
    ("com.oneplus.account", "OnePlus Account", true),
    ("com.bbk.account", "vivo Account", true),
    ("com.vivo.account", "vivo Account", true),
    ("com.transsion.account", "Tecno / Infinix (Transsion) Account", false),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountType {
    pub label: String,
    pub count: u32,
    pub protects: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct AccountSummary {
    pub types: Vec<AccountType>,
}

impl AccountSummary {
    pub fn google(&self) -> u32 {
        self.types.iter().filter(|t| t.label == "Google").map(|t| t.count).sum()
    }
    pub fn protected(&self) -> Vec<&AccountType> {
        self.types.iter().filter(|t| t.protects).collect()
    }
}

fn line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Only the type is captured; the name group is matched but never read.
    RE.get_or_init(|| Regex::new(r"Account \{name=[^}]*?, type=([A-Za-z0-9_.\-]+)\}").expect("valid regex"))
}

pub fn parse_account_types(dumpsys: &str) -> AccountSummary {
    let mut counts: BTreeMap<(String, bool), u32> = BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    for cap in line_re().captures_iter(dumpsys) {
        // dumpsys lists each account more than once (per user and in caches): count each full line once.
        if !seen.insert(cap.get(0).map(|m| m.as_str().to_string()).unwrap_or_default()) {
            continue;
        }
        let raw = &cap[1];
        let (label, protects) = TYPES
            .iter()
            .find(|(t, _, _)| *t == raw)
            .map(|(_, l, p)| ((*l).to_string(), *p))
            .unwrap_or_else(|| ("Otro tipo de cuenta".to_string(), false));
        *counts.entry((label, protects)).or_default() += 1;
    }
    AccountSummary { types: counts.into_iter().map(|((label, protects), count)| AccountType { label, count, protects }).collect() }
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountCheck {
    pub id: i64,
    pub created_at: String,
    pub summary: AccountSummary,
    pub google_count: u32,
    pub protected_count: u32,
    pub author: Option<String>,
}

/// Stores a check (types and counts only) so later checks can verify that accounts were removed.
pub fn record_check(conn: &rusqlite::Connection, device_id: i64, user_id: i64, summary: &AccountSummary) -> AppResult<AccountCheck> {
    let google = summary.google();
    let protected: u32 = summary.protected().iter().map(|t| t.count).sum();
    let ts = crate::util::now();
    conn.execute(
        "INSERT INTO account_checks (device_id, created_at, author_id, summary_json, google_count, protected_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![device_id, ts, user_id, serde_json::to_string(&summary.types).unwrap_or_default(), google, protected],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO operations (started_at, finished_at, device_id, user_id, operation, category, result, details_json)          VALUES (?1, ?1, ?2, ?3, 'Comprobación de cuentas', 'accounts', 'success', ?4)",
        rusqlite::params![ts, device_id, user_id, serde_json::json!({ "google": google, "protected": protected }).to_string()],
    )?;
    Ok(AccountCheck { id, created_at: ts, summary: summary.clone(), google_count: google, protected_count: protected, author: None })
}

pub fn checks_for(conn: &rusqlite::Connection, device_id: i64, limit: i64) -> AppResult<Vec<AccountCheck>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.created_at, c.summary_json, c.google_count, c.protected_count, u.display_name FROM account_checks c          LEFT JOIN users u ON u.id = c.author_id WHERE c.device_id = ?1 ORDER BY c.id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![device_id, limit], |r| {
        let json: String = r.get(2)?;
        Ok(AccountCheck {
            id: r.get(0)?,
            created_at: r.get(1)?,
            summary: AccountSummary { types: serde_json::from_str(&json).unwrap_or_default() },
            google_count: r.get(3)?,
            protected_count: r.get(4)?,
            author: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn read_account_types(tools: &Tools, serial: &str) -> AppResult<AccountSummary> {
    let out = tools.adb(Some(serial), &["shell", "dumpsys", "account"], std::time::Duration::from_secs(20))?;
    if !out.ok() {
        return Err(AppError::Device(format!("No se pudo consultar la lista de cuentas: {}", out.failure())));
    }
    Ok(parse_account_types(&out.stdout))
}

#[cfg(test)]
pub const SAMPLE_DUMPSYS: &str = "User UserInfo{0:Propietario:c13}:\n  Accounts: 3\n    Account {name=ana.perez@gmail.com, type=com.google}\n\
    Account {name=trabajo@empresa.com, type=com.google}\n    Account {name=ana@samsung.example, type=com.osp.app.signin}\n\n  \
Active Sessions: 0\n  RegisteredServicesCache: 4 services\n    Account {name=ana.perez@gmail.com, type=com.google}\n    \
Account {name=x@y.z, type=com.whatsapp}\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_types_and_counts_never_names() {
        let s = parse_account_types(SAMPLE_DUMPSYS);
        assert_eq!(s.google(), 2, "duplicate cache lines are not double counted");
        let samsung = s.types.iter().find(|t| t.label == "Samsung Account").unwrap();
        assert!(samsung.protects);
        assert!(s.types.iter().any(|t| t.label == "Otro tipo de cuenta" && !t.protects));
        let json = serde_json::to_string(&s).unwrap();
        for secret in ["ana.perez", "gmail", "empresa.com", "samsung.example", "x@y.z"] {
            assert!(!json.contains(secret), "{secret} leaked");
        }
        assert!(parse_account_types("Accounts: 0\n").types.is_empty());
    }
}
