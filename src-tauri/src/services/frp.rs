//! Google Factory Reset Protection (FRP) diagnosis.
//!
//! FRP is an anti-theft protection. This module only (1) reads read-only indicators from the device,
//! (2) evaluates what the technician observed and the owner answered, and (3) records the result.
//! It contains no way to bypass, disable or remove FRP, and never will.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::accounts::{self, AccountSummary};
use crate::services::adb::client::Tools;
use crate::services::adb::parsers::Mode;
use crate::services::audit::{self, Severity};
use crate::services::checks::{CheckItem, CheckStatus::*};
use crate::services::devices::{mode_label, DeviceView};
use crate::util::now;

/// How the device was last reset, as reported by the owner/technician.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetOrigin {
    NotReset,
    Settings,
    RecoveryOrButtons,
    Unknown,
}

/// What the technician observed and the owner answered. `None` = not answered yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub asks_previous_account: Option<bool>,
    pub owner_or_authorized: Option<bool>,
    pub reset_origin: Option<ResetOrigin>,
    pub second_hand: Option<bool>,
    pub password_changed_recently: Option<bool>,
    pub knows_credentials: Option<bool>,
    pub can_recover_account: Option<bool>,
    pub has_purchase_proof: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Incomplete,
    NoIndication,
    StopNoAuthorization,
    OwnerSignsIn,
    WaitSecurityPeriod,
    AccountRecovery,
    ManufacturerSupport,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Incomplete => "incomplete",
            Outcome::NoIndication => "no_indication",
            Outcome::StopNoAuthorization => "stop_no_authorization",
            Outcome::OwnerSignsIn => "owner_signs_in",
            Outcome::WaitSecurityPeriod => "wait_security_period",
            Outcome::AccountRecovery => "account_recovery",
            Outcome::ManufacturerSupport => "manufacturer_support",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Assessment {
    pub outcome: Outcome,
    pub headline: String,
    /// Possible reasons for the lock, derived from the answers (diagnosis, not certainty).
    pub possible_causes: Vec<String>,
    /// Instructions for the OWNER. Nothing here is executed by AndroidMasters.
    pub next_steps: Vec<String>,
    pub documentation: Vec<String>,
    pub stop: bool,
    /// Questions still needed to reach a recommendation.
    pub pending_questions: Vec<String>,
}

const DOCUMENTATION: &[&str] = &[
    "Comprobante de compra (factura o recibo) a nombre del propietario, con el modelo del equipo",
    "Identificación oficial del propietario",
    "En equipos de empresa: autorización escrita de la empresa e identificación de quien la presenta",
];

fn causes(o: &Observation) -> Vec<String> {
    let mut c = Vec::new();
    match o.reset_origin {
        Some(ResetOrigin::RecoveryOrButtons) => c.push(
            "Se restableció desde Recovery o con los botones mientras la cuenta Google seguía en el equipo: Android lo trata como un \
             restablecimiento no autorizado y pide la cuenta anterior."
                .into(),
        ),
        Some(ResetOrigin::Settings) => c.push(
            "Se restableció desde Ajustes sin quitar antes la cuenta Google: la protección puede seguir vinculada a esa cuenta.".into(),
        ),
        Some(ResetOrigin::Unknown) => c.push("Se desconoce cómo se restableció el equipo.".into()),
        _ => {}
    }
    if o.second_hand == Some(true) {
        c.push("Equipo de segunda mano: la cuenta solicitada puede pertenecer al propietario anterior, que no la retiró.".into());
    }
    if o.password_changed_recently == Some(true) {
        c.push("La contraseña de la cuenta cambió hace poco: Google puede impedir usarla en un equipo restablecido durante un periodo de seguridad.".into());
    }
    c
}

/// Pure decision logic. Authorization is always checked first, as the mandatory workflow requires.
pub fn assess(o: &Observation) -> Assessment {
    let mut a = Assessment {
        outcome: Outcome::Incomplete,
        headline: String::new(),
        possible_causes: causes(o),
        next_steps: Vec::new(),
        documentation: Vec::new(),
        stop: false,
        pending_questions: Vec::new(),
    };
    if o.owner_or_authorized.is_none() {
        a.pending_questions.push("¿La persona es el propietario o está autorizada por él?".into());
    }
    if o.asks_previous_account.is_none() {
        a.pending_questions.push("¿El equipo pide la cuenta Google sincronizada anteriormente?".into());
    }
    if o.owner_or_authorized == Some(false) {
        a.outcome = Outcome::StopNoAuthorization;
        a.stop = true;
        a.headline = "Detener: no se ha demostrado la propiedad ni la autorización.".into();
        a.next_steps = vec![
            "No continúe con el servicio de este equipo.".into(),
            "Recomiende contactar al vendedor o al propietario anterior para que retire el equipo de su cuenta.".into(),
            "O acudir al fabricante o a un servicio autorizado con comprobante de compra.".into(),
        ];
        a.documentation = DOCUMENTATION.iter().map(|d| d.to_string()).collect();
        return a;
    }
    if !a.pending_questions.is_empty() {
        a.headline = "Faltan datos para recomendar un procedimiento.".into();
        return a;
    }
    if o.asks_previous_account == Some(false) {
        a.outcome = Outcome::NoIndication;
        a.headline = "Sin indicios de FRP: el equipo no pide una cuenta anterior.".into();
        a.next_steps = vec!["Continúe con el servicio solicitado.".into()];
        return a;
    }
    match o.knows_credentials {
        None => {
            a.pending_questions.push("¿El propietario conoce el correo y la contraseña de esa cuenta?".into());
            a.headline = "Falta saber si el propietario conoce las credenciales.".into();
        }
        Some(true) if o.password_changed_recently == Some(true) => {
            a.outcome = Outcome::WaitSecurityPeriod;
            a.headline = "El propietario conoce la cuenta, pero la contraseña cambió hace poco.".into();
            a.next_steps = vec![
                "Explique al propietario que Google puede aplicar un periodo de espera de seguridad tras un cambio de contraseña.".into(),
                "Pasado ese periodo, el propietario inicia sesión en el equipo durante la configuración con su propia cuenta.".into(),
                "Siga los mensajes que muestre Google en el equipo o en el correo del propietario.".into(),
            ];
        }
        Some(true) => {
            a.outcome = Outcome::OwnerSignsIn;
            a.headline = "El propietario inicia sesión con la cuenta que usaba en el equipo.".into();
            a.next_steps = vec![
                "El propietario escribe su correo y contraseña en el equipo durante la configuración. El técnico no las escribe ni las ve.".into(),
                "Si el equipo se va a transferir, el propietario retira después la cuenta desde Ajustes antes del siguiente restablecimiento.".into(),
            ];
        }
        Some(false) if o.can_recover_account != Some(false) => {
            a.outcome = Outcome::AccountRecovery;
            a.headline = "El propietario recupera su cuenta Google por el canal oficial.".into();
            a.next_steps = vec![
                "Desde cualquier navegador, el propietario abre la página oficial de recuperación de cuentas de Google y sigue sus pasos.".into(),
                "Tras cambiar la contraseña, Google puede aplicar un periodo de espera de seguridad; siga sus indicaciones.".into(),
                "Después, el propietario inicia sesión en el equipo durante la configuración.".into(),
            ];
        }
        Some(false) => {
            a.outcome = Outcome::ManufacturerSupport;
            a.headline = "El propietario debe contactar al fabricante o a un servicio autorizado.".into();
            a.next_steps = vec![
                "El propietario contacta con el soporte oficial del fabricante o un centro de servicio autorizado.".into(),
                "Debe presentar la documentación indicada abajo.".into(),
            ];
            if o.has_purchase_proof != Some(true) {
                a.next_steps.push("Sin comprobante de compra es posible que el fabricante no pueda ayudar.".into());
            }
            a.documentation = DOCUMENTATION.iter().map(|d| d.to_string()).collect();
        }
    }
    a
}

/// Read-only indicators from the connected device. Detected facts only.
pub fn indicators(tools: &Tools, view: &DeviceView, serial: &str) -> (Vec<CheckItem>, Option<AccountSummary>) {
    let mut items = Vec::new();
    if view.mode != Mode::Adb {
        items.push(
            CheckItem::new("mode", "Límites de la detección", Info, mode_label(view.mode)).advise(
                "Tras un restablecimiento la depuración USB queda desactivada, así que FRP no se puede detectar por USB: registre lo que \
                 muestra la pantalla de configuración del equipo.",
            ),
        );
        return (items, None);
    }
    let supported = match view.api_level {
        Some(api) if api >= 22 => CheckItem::new("frp_support", "Protección FRP disponible", Info, format!("Android {} (API {api}): incluye FRP", view.android_version.as_deref().unwrap_or("?"))),
        Some(api) => CheckItem::new("frp_support", "Protección FRP disponible", Info, format!("API {api}: anterior a Android 5.1, sin FRP de Google")),
        None => CheckItem::new("frp_support", "Protección FRP disponible", Info, "Versión de Android no detectada"),
    };
    items.push(supported);
    if let Ok(out) = tools.adb(Some(serial), &["shell", "getprop", "ro.frp.pst"], std::time::Duration::from_secs(10)) {
        let value = out.stdout.trim();
        items.push(CheckItem::new(
            "frp_storage",
            "Almacenamiento de la protección",
            Info,
            if out.ok() && !value.is_empty() { "El equipo declara una partición de protección de restablecimiento" } else { "El equipo no lo informa" },
        ));
    }
    let summary = match accounts::read_account_types(tools, serial) {
        Ok(s) => s,
        Err(e) => {
            items.push(CheckItem::new("accounts", "Cuentas en el equipo", Warn, e.to_string()));
            return (items, None);
        }
    };
    let google = summary.google();
    items.push(if google > 0 {
        CheckItem::new("google", "Cuenta Google en el equipo", Warn, format!("{google} cuenta(s) Google con sesión iniciada")).advise(
            "Si el equipo se restablece sin que el propietario retire antes la cuenta desde Ajustes, pedirá esa cuenta al configurarlo (FRP).",
        )
    } else {
        CheckItem::new("google", "Cuenta Google en el equipo", Pass, "Ninguna cuenta Google con sesión iniciada")
    });
    let others: Vec<String> = summary.protected().into_iter().filter(|t| t.label != "Google").map(|t| format!("{} ({})", t.label, t.count)).collect();
    if !others.is_empty() {
        items.push(
            CheckItem::new("brand_accounts", "Cuentas del fabricante con protección antirrobo", Warn, others.join(", "))
                .advise("El propietario debe cerrar sesión y desactivar la búsqueda del dispositivo desde los ajustes oficiales."),
        );
    }
    (items, Some(summary))
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordedAssessment {
    pub id: i64,
    pub created_at: String,
    pub device: String,
    pub outcome: String,
    pub author: Option<String>,
}

pub fn record(
    conn: &Connection,
    user_id: i64,
    device_id: Option<i64>,
    device_label: &str,
    observation: &Observation,
    notes: &str,
) -> AppResult<(i64, Assessment)> {
    let assessment = assess(observation);
    if assessment.outcome == Outcome::Incomplete {
        return Err(AppError::Validation(format!("Faltan respuestas: {}", assessment.pending_questions.join(" "))));
    }
    if device_id.is_none() && device_label.trim().is_empty() {
        return Err(AppError::Validation("Seleccione el dispositivo conectado o escriba su marca y modelo.".into()));
    }
    let data = serde_json::json!({
        "observation": observation,
        "device_label": device_label.trim(),
        "notes": notes.trim(),
        "possible_causes": assessment.possible_causes,
        "next_steps": assessment.next_steps,
    });
    let ts = now();
    conn.execute(
        "INSERT INTO frp_assessments (device_id, created_at, observations_json, outcome, author_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![device_id, ts, data.to_string(), assessment.outcome.as_str(), user_id],
    )?;
    let id = conn.last_insert_rowid();
    let result = if assessment.stop { "rejected" } else { "success" };
    conn.execute(
        "INSERT INTO operations (started_at, finished_at, device_id, user_id, operation, category, result, details_json) \
         VALUES (?1, ?1, ?2, ?3, 'Diagnóstico FRP', 'frp', ?4, ?5)",
        params![ts, device_id, user_id, result, serde_json::json!({ "assessment_id": id, "outcome": assessment.outcome }).to_string()],
    )?;
    audit::record(
        conn,
        if assessment.stop { "frp_stopped_no_authorization" } else { "frp_assessment_recorded" },
        if assessment.stop { Severity::Warning } else { Severity::Info },
        Some(user_id),
        "",
        serde_json::json!({ "assessment_id": id, "outcome": assessment.outcome }),
    )?;
    Ok((id, assessment))
}

pub fn history(conn: &Connection, limit: i64) -> AppResult<Vec<RecordedAssessment>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.created_at, COALESCE(NULLIF(TRIM(d.manufacturer || ' ' || d.model), ''), json_extract(f.observations_json, '$.device_label'), '—'), \
         f.outcome, u.display_name FROM frp_assessments f LEFT JOIN devices d ON d.id = f.device_id LEFT JOIN users u ON u.id = f.author_id \
         ORDER BY f.id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(RecordedAssessment { id: r.get(0)?, created_at: r.get(1)?, device: r.get(2)?, outcome: r.get(3)?, author: r.get(4)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::security::crypto::Vault;
    use crate::services::adb::parsers::samples;
    use crate::services::adb::runner::fake::FakeRunner;
    use crate::services::devices::DeviceMonitor;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn obs() -> Observation {
        Observation { asks_previous_account: Some(true), owner_or_authorized: Some(true), ..Default::default() }
    }

    #[test]
    fn authorization_comes_first_and_stops_the_process() {
        let a = assess(&Observation { owner_or_authorized: Some(false), ..Default::default() });
        assert_eq!(a.outcome, Outcome::StopNoAuthorization);
        assert!(a.stop);
        assert!(!a.documentation.is_empty());
        let incomplete = assess(&Observation::default());
        assert_eq!(incomplete.outcome, Outcome::Incomplete);
        assert_eq!(incomplete.pending_questions.len(), 2);
    }

    #[test]
    fn outcomes_follow_the_official_path() {
        assert_eq!(assess(&Observation { asks_previous_account: Some(false), ..obs() }).outcome, Outcome::NoIndication);
        assert_eq!(assess(&obs()).outcome, Outcome::Incomplete, "needs to know about credentials");
        assert_eq!(assess(&Observation { knows_credentials: Some(true), ..obs() }).outcome, Outcome::OwnerSignsIn);
        assert_eq!(
            assess(&Observation { knows_credentials: Some(true), password_changed_recently: Some(true), ..obs() }).outcome,
            Outcome::WaitSecurityPeriod
        );
        assert_eq!(assess(&Observation { knows_credentials: Some(false), ..obs() }).outcome, Outcome::AccountRecovery);
        let support = assess(&Observation { knows_credentials: Some(false), can_recover_account: Some(false), ..obs() });
        assert_eq!(support.outcome, Outcome::ManufacturerSupport);
        assert!(support.next_steps.iter().any(|s| s.contains("comprobante")));
    }

    #[test]
    fn causes_are_derived_from_answers() {
        let a = assess(&Observation { reset_origin: Some(ResetOrigin::RecoveryOrButtons), second_hand: Some(true), ..obs() });
        assert_eq!(a.possible_causes.len(), 2);
        assert!(a.possible_causes[0].contains("Recovery"));
    }

    #[test]
    fn instructions_never_claim_an_action_was_performed() {
        let all = [
            assess(&Observation { knows_credentials: Some(true), ..obs() }),
            assess(&Observation { knows_credentials: Some(false), ..obs() }),
            assess(&Observation { knows_credentials: Some(false), can_recover_account: Some(false), ..obs() }),
        ];
        for a in all {
            for step in a.next_steps {
                let s = step.to_lowercase();
                assert!(!s.contains("se eliminó") && !s.contains("desbloqueado") && !s.contains("bypass"), "{step}");
            }
        }
    }

    #[test]
    fn records_result_operation_and_audit_but_not_incomplete_ones() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('t','Tec','technician','h','t')", []).unwrap();
        assert!(record(&conn, 1, None, "Samsung A14", &Observation::default(), "").is_err());
        assert!(record(&conn, 1, None, "  ", &Observation { knows_credentials: Some(true), ..obs() }, "").is_err(), "needs a device");
        let (id, a) = record(&conn, 1, None, "Samsung A14", &Observation { owner_or_authorized: Some(false), ..obs() }, "Sin factura").unwrap();
        assert!(a.stop);
        let h = history(&conn, 10).unwrap();
        assert_eq!((h[0].id, h[0].device.as_str(), h[0].outcome.as_str()), (id, "Samsung A14", "stop_no_authorization"));
        let ops: String = conn.query_row("SELECT result FROM operations WHERE category = 'frp'", [], |r| r.get(0)).unwrap();
        assert_eq!(ops, "rejected");
        let audit: String = conn.query_row("SELECT event FROM audit_log ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(audit, "frp_stopped_no_authorization");
    }

    #[test]
    fn indicators_from_a_connected_device() {
        const PIXEL: &str = "28141FDH2000K7";
        let r = Arc::new(FakeRunner::default());
        r.on("adb devices -l", &format!("List of devices attached\n{PIXEL} device usb:1-1 model:Pixel_7 transport_id:3\n")).on("fastboot devices", "");
        r.on(&format!("adb -s {PIXEL} shell getprop"), samples::PIXEL_GETPROP);
        r.on(&format!("adb -s {PIXEL} shell getprop ro.frp.pst"), "/dev/block/by-name/frp\n");
        r.on(&format!("adb -s {PIXEL} shell dumpsys account"), crate::services::accounts::SAMPLE_DUMPSYS);
        let tools = Tools { runner: r, adb: Some(PathBuf::from("/x/adb")), fastboot: None };
        let db = Database::open_in_memory().unwrap();
        let monitor = DeviceMonitor::default();
        monitor.scan(&tools, &db.conn(), &Vault::ephemeral());
        let view = monitor.snapshot().devices[0].clone();
        let (items, summary) = indicators(&tools, &view, PIXEL);
        let google = items.iter().find(|i| i.key == "google").unwrap();
        assert_eq!(google.status, Warn);
        assert!(items.iter().any(|i| i.key == "brand_accounts" && i.detail.contains("Samsung")));
        assert_eq!(summary.unwrap().google(), 2);
        let all = serde_json::to_string(&items).unwrap();
        assert!(!all.contains("gmail") && !all.contains(PIXEL));
    }
}
