//! Versioned library of official procedures (one JSON per manufacturer in `src-tauri/procedures`).
//! Procedures are guidance for the owner; they are displayed, never executed. A workshop administrator
//! reviews each procedure; the review is tied to the procedure version and lapses when it changes.

use std::sync::OnceLock;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::audit::{self, Severity};
use crate::util::now;

const SOURCES: &[(&str, &str)] = &[
    ("google", include_str!("../../procedures/google.json")),
    ("samsung", include_str!("../../procedures/samsung.json")),
    ("xiaomi", include_str!("../../procedures/xiaomi.json")),
    ("motorola", include_str!("../../procedures/motorola.json")),
    ("huawei", include_str!("../../procedures/huawei.json")),
    ("oppo", include_str!("../../procedures/oppo.json")),
    ("realme", include_str!("../../procedures/realme.json")),
    ("vivo", include_str!("../../procedures/vivo.json")),
    ("tecno", include_str!("../../procedures/tecno.json")),
    ("infinix", include_str!("../../procedures/infinix.json")),
    ("pixel", include_str!("../../procedures/pixel.json")),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    pub id: String,
    pub kind: String,
    pub service: String,
    pub title: String,
    pub version: i64,
    #[serde(default)]
    pub android_min: Option<u32>,
    #[serde(default)]
    pub android_max: Option<u32>,
    pub summary: String,
    pub steps: Vec<String>,
    pub verification: Vec<String>,
    pub links: Vec<Link>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brand {
    pub brand: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub updated: String,
    pub support_links: Vec<Link>,
    pub procedures: Vec<Procedure>,
}

pub fn library() -> &'static [Brand] {
    static LIB: OnceLock<Vec<Brand>> = OnceLock::new();
    LIB.get_or_init(|| {
        SOURCES
            .iter()
            .map(|(key, json)| serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid procedure file {key}.json: {e}")))
            .collect()
    })
}

/// Brand key for a device, from its manufacturer / brand properties. `None` = no specific module.
pub fn brand_for(manufacturer: Option<&str>, brand: Option<&str>) -> Option<&'static str> {
    let candidates: Vec<String> = [brand, manufacturer].into_iter().flatten().map(|s| s.trim().to_lowercase()).collect();
    library().iter().find(|b| {
        b.aliases.iter().any(|alias| candidates.iter().any(|c| c == alias || c.starts_with(&format!("{alias} "))))
    }).map(|b| b.brand.as_str())
}

#[derive(Debug, Clone, Serialize)]
pub struct Review {
    pub version: i64,
    pub reviewed_by: String,
    pub reviewed_at: String,
    pub note: String,
    /// False when the procedure changed after it was reviewed.
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcedureView {
    #[serde(flatten)]
    pub procedure: Procedure,
    pub review: Option<Review>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrandView {
    pub brand: String,
    pub name: String,
    pub updated: String,
    pub support_links: Vec<Link>,
    pub procedures: Vec<ProcedureView>,
}

fn review_for(conn: &Connection, p: &Procedure) -> AppResult<Option<Review>> {
    Ok(conn
        .query_row(
            "SELECT r.version, u.display_name, r.reviewed_at, r.note FROM procedure_reviews r JOIN users u ON u.id = r.reviewed_by WHERE r.procedure_id = ?1",
            params![p.id],
            |r| Ok(Review { version: r.get(0)?, reviewed_by: r.get(1)?, reviewed_at: r.get(2)?, note: r.get(3)?, current: false }),
        )
        .optional()?
        .map(|mut r| {
            r.current = r.version == p.version;
            r
        }))
}

pub fn catalog(conn: &Connection) -> AppResult<Vec<BrandView>> {
    library()
        .iter()
        .map(|b| {
            Ok(BrandView {
                brand: b.brand.clone(),
                name: b.name.clone(),
                updated: b.updated.clone(),
                support_links: b.support_links.clone(),
                procedures: b
                    .procedures
                    .iter()
                    .map(|p| Ok(ProcedureView { procedure: p.clone(), review: review_for(conn, p)? }))
                    .collect::<AppResult<_>>()?,
            })
        })
        .collect()
}

pub fn mark_reviewed(conn: &Connection, user_id: i64, procedure_id: &str, note: &str) -> AppResult<()> {
    let p = library()
        .iter()
        .flat_map(|b| b.procedures.iter())
        .find(|p| p.id == procedure_id)
        .ok_or_else(|| AppError::NotFound("Procedimiento no encontrado.".into()))?;
    conn.execute(
        "INSERT INTO procedure_reviews (procedure_id, version, reviewed_by, reviewed_at, note) VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(procedure_id) DO UPDATE SET version = excluded.version, reviewed_by = excluded.reviewed_by, \
         reviewed_at = excluded.reviewed_at, note = excluded.note",
        params![p.id, p.version, user_id, now(), note.trim()],
    )?;
    audit::record(conn, "procedure_reviewed", Severity::Info, Some(user_id), "", serde_json::json!({ "procedure": p.id, "version": p.version }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use std::collections::HashSet;

    #[test]
    fn library_covers_required_brands_and_is_well_formed() {
        let keys: HashSet<&str> = library().iter().map(|b| b.brand.as_str()).collect();
        for required in ["google", "samsung", "xiaomi", "motorola", "huawei", "oppo", "realme", "vivo", "tecno", "infinix", "pixel"] {
            assert!(keys.contains(required), "{required} missing");
        }
        let mut ids = HashSet::new();
        for b in library() {
            assert!(!b.procedures.is_empty(), "{}", b.brand);
            assert!(b.procedures.iter().any(|p| p.kind == "transfer"), "{} needs a transfer procedure", b.brand);
            for p in &b.procedures {
                assert!(ids.insert(p.id.clone()), "duplicate id {}", p.id);
                assert!(p.version >= 1 && !p.steps.is_empty() && !p.verification.is_empty(), "{}", p.id);
                for l in p.links.iter().chain(&b.support_links) {
                    assert!(l.url.starts_with("https://"), "{} has a non-https link", p.id);
                }
                let text = format!("{} {}", p.title, p.steps.join(" ")).to_lowercase();
                for banned in ["bypass", "omitir frp", "saltar", "apk", "exploit", "talkback", "código secreto"] {
                    assert!(!text.contains(banned), "{} mentions '{banned}'", p.id);
                }
            }
        }
    }

    #[test]
    fn brand_detection() {
        assert_eq!(brand_for(Some("samsung"), Some("samsung")), Some("samsung"));
        assert_eq!(brand_for(Some("Xiaomi"), Some("Redmi")), Some("xiaomi"));
        assert_eq!(brand_for(Some("Xiaomi"), Some("POCO")), Some("xiaomi"));
        assert_eq!(brand_for(Some("Google"), Some("google")), Some("pixel"));
        assert_eq!(brand_for(Some("TECNO MOBILE LIMITED"), Some("TECNO")), Some("tecno"));
        assert_eq!(brand_for(Some("Fairphone"), Some("Fairphone")), None);
        assert_eq!(brand_for(None, None), None);
    }

    #[test]
    fn review_is_tied_to_the_version() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('a','Ana','admin','h','t')", []).unwrap();
        mark_reviewed(&conn, 1, "samsung-account-remove", "Probado en A14").unwrap();
        let samsung = catalog(&conn).unwrap().into_iter().find(|b| b.brand == "samsung").unwrap();
        let p = samsung.procedures.iter().find(|p| p.procedure.id == "samsung-account-remove").unwrap();
        assert!(p.review.as_ref().unwrap().current);
        conn.execute("UPDATE procedure_reviews SET version = 0", []).unwrap();
        let again = catalog(&conn).unwrap().into_iter().find(|b| b.brand == "samsung").unwrap();
        let p = again.procedures.iter().find(|p| p.procedure.id == "samsung-account-remove").unwrap();
        assert!(!p.review.as_ref().unwrap().current, "an old review does not cover the new version");
        assert!(mark_reviewed(&conn, 1, "nope", "").is_err());
    }
}
