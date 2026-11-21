//! Service reports: per-order PDF, period summary (PDF) and CSV export.
//!
//! Reports keep the SKILL.md distinction visible (diagnosis / instruction / executed action / verification),
//! flag everything still pending verification, show the serial only masked and never contain passwords,
//! codes, tokens or account names (none of these are stored in the first place).

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::services::orders::{EntryKind, EntryStatus, Order, OrderStatus, OrderSummary, OwnershipMethod, RequestType};
use crate::services::pdf::{self, Document, Rgb, BAND, BRAND, CONTENT_W, DANGER, INFO, INK, MARGIN, MUTED, OK, PAGE_H, PAGE_W, WARN};

pub const PRIVACY_NOTE: &str = "Este reporte no contiene contraseñas, códigos de verificación, tokens ni nombres o correos de cuentas. \
El número de serie se muestra enmascarado.";
pub const LEGEND: &str = "Diagnóstico: información detectada o comunicada por el equipo. Instrucción: pasos para el propietario; \
no son operaciones realizadas. Acción ejecutada: operación realizada por AndroidMasters. Verificación: evidencia posterior del resultado.";

/// "2026-11-21T13:29:05" → "21/11/2026 13:29".
pub fn format_dt(value: &str) -> String {
    let date = format_date(value);
    match value.get(11..16) {
        Some(time) => format!("{date} {time}"),
        None => date,
    }
}

/// "2026-11-21…" → "21/11/2026".
pub fn format_date(value: &str) -> String {
    match (value.get(0..4), value.get(5..7), value.get(8..10)) {
        (Some(y), Some(m), Some(d)) => format!("{d}/{m}/{y}"),
        _ => value.to_string(),
    }
}

fn kind_color(kind: &str, status: &str) -> Rgb {
    match (kind, status) {
        (_, "pending_verification") => WARN,
        ("verification", "failed") | ("incident", _) | ("consent", "failed") => DANGER,
        ("verification", _) | ("action", "ok") => OK,
        ("diagnosis", _) => INFO,
        ("instruction", _) | ("consent", _) => BRAND,
        _ => MUTED,
    }
}

fn header_band(doc: &mut Document, workshop: &str, title: &str, right_top: &str, right_bottom: &str) {
    let h = 74.0;
    doc.rect(0.0, PAGE_H - h, PAGE_W, h, BAND);
    doc.rect(0.0, PAGE_H - h, PAGE_W, 2.0, BRAND);
    doc.text_at(MARGIN, PAGE_H - 34.0, 15.0, true, INK, workshop);
    doc.text_at(MARGIN, PAGE_H - 52.0, 10.0, false, MUTED, title);
    let w = pdf::text_width(right_top, 12.0, true);
    doc.text_at(PAGE_W - MARGIN - w, PAGE_H - 34.0, 12.0, true, INK, right_top);
    let w = pdf::text_width(right_bottom, 8.5, false);
    doc.text_at(PAGE_W - MARGIN - w, PAGE_H - 52.0, 8.5, false, MUTED, right_bottom);
    doc.y = PAGE_H - h - 14.0;
}

fn entry_block(doc: &mut Document, entry: &crate::services::orders::Entry, verified_by: Option<&str>) {
    let x = MARGIN + 12.0;
    let w = CONTENT_W - 12.0;
    let title_lines = pdf::wrap(&entry.title, 10.0, true, w);
    let detail_lines = if entry.detail.trim().is_empty() { Vec::new() } else { pdf::wrap(&entry.detail, 9.0, false, w) };
    let height = 12.0 + title_lines.len() as f32 * 13.5 + detail_lines.len() as f32 * 12.2 + 14.0;
    doc.ensure(height.min(600.0) + 6.0);
    let color = kind_color(&entry.kind, &entry.status);
    let top = doc.y;
    let mut status = EntryStatus::label(&entry.status).to_string();
    if entry.status == "pending_verification" {
        status = match verified_by {
            Some(v) => format!("Verificado después ({v})"),
            None => "PENDIENTE DE VERIFICACIÓN".into(),
        };
    }
    let label = format!("{} · {}", EntryKind::label(&entry.kind).to_uppercase(), status);
    doc.y -= 10.0;
    doc.text_at(x, doc.y, 7.5, true, color, &label);
    for line in &title_lines {
        doc.ensure(13.5);
        doc.y -= 13.5;
        doc.text_at(x, doc.y + 2.0, 10.0, true, INK, line);
    }
    for line in &detail_lines {
        doc.ensure(12.2);
        doc.y -= 12.2;
        doc.text_at(x, doc.y + 2.0, 9.0, false, INK, line);
    }
    doc.ensure(12.0);
    doc.y -= 12.0;
    let meta = format!("{} · {}", format_dt(&entry.created_at), entry.author.as_deref().unwrap_or("Sistema"));
    doc.text_at(x, doc.y + 1.0, 7.5, false, MUTED, &meta);
    // Left bar spanning the block (on the current page).
    let bottom = doc.y - 2.0;
    let bar_top = if top > bottom { top } else { PAGE_H - MARGIN };
    doc.rect(MARGIN, bottom, 2.5, bar_top - bottom, color);
    doc.y -= 8.0;
}

pub fn order_pdf(order: &Order, workshop: &str, generated_by: &str, generated_at: &str) -> Vec<u8> {
    let footer = format!("{workshop} · Orden {} · Generado por {generated_by}", order.number);
    let mut doc = Document::new(&format!("Reporte de servicio {}", order.number), &footer);
    header_band(&mut doc, workshop, "Reporte de servicio", &order.number, &format!("Generado el {}", format_dt(generated_at)));

    let status_line = format!(
        "Estado: {}   ·   Consentimiento: {}   ·   Tipo de servicio: {}",
        OrderStatus::label(&order.status),
        consent_label(&order.consent_status),
        RequestType::label(&order.request_type)
    );
    doc.paragraph(&status_line, 9.5, true, INK);

    doc.heading("Cliente y equipo");
    let mut rows: Vec<(&str, String)> = vec![("Cliente", order.customer.clone()), ("Contacto", order.customer_contact.clone()), ("Equipo", order.device_name())];
    if let Some(d) = &order.device {
        rows.push(("Número de serie", d.serial_masked.clone()));
        rows.push(("Android", format!("{}{}", d.android_version, d.api_level.map(|a| format!(" (API {a})")).unwrap_or_default())));
        rows.push(("Parche de seguridad", d.security_patch.clone()));
        rows.push(("Identificación", "Detectado por USB (ADB)".into()));
    } else {
        rows.push(("Identificación", "Marca y modelo indicados por el técnico (equipo no conectado)".into()));
    }
    rows.push(("Técnico", order.technician.clone().unwrap_or_default()));
    rows.push(("Recibido", format_dt(&order.received_at)));
    rows.push(("Entregado", order.delivered_at.as_deref().map(format_dt).unwrap_or_default()));
    doc.key_values(&rows);

    doc.heading("Consentimiento del propietario");
    doc.key_values(&[
        ("Estado", consent_label(&order.consent_status).to_string()),
        ("Autorizado por", order.consent_by.clone()),
        ("Comprobación", OwnershipMethod::label(&order.ownership_method).to_string()),
        ("Fecha", order.consent_at.as_deref().map(format_dt).unwrap_or_default()),
    ]);

    doc.heading("Problema reportado");
    doc.paragraph(if order.reported_problem.trim().is_empty() { "—" } else { &order.reported_problem }, 9.5, false, INK);

    let unverified = order.unverified();
    if !unverified.is_empty() {
        doc.space(10.0);
        let titles: Vec<String> = unverified.iter().map(|e| format!("• {}", e.title)).collect();
        let text = format!("Pendiente de verificación ({}): no debe darse por completado.\n{}", unverified.len(), titles.join("\n"));
        let lines = pdf::wrap(&text, 9.0, false, CONTENT_W - 20.0);
        let h = lines.len() as f32 * 12.5 + 12.0;
        doc.ensure(h + 4.0);
        doc.rect(MARGIN, doc.y - h, CONTENT_W, h, Rgb(1.0, 0.96, 0.88));
        doc.rect(MARGIN, doc.y - h, 2.5, h, WARN);
        doc.y -= 4.0;
        for (i, line) in lines.iter().enumerate() {
            doc.y -= 12.5;
            doc.text_at(MARGIN + 12.0, doc.y + 2.0, 9.0, i == 0, INK, line);
        }
        doc.y -= 8.0;
    }

    doc.heading("Registro del servicio");
    doc.paragraph(LEGEND, 8.0, false, MUTED);
    doc.space(6.0);
    let by_id: std::collections::HashMap<i64, &crate::services::orders::Entry> = order.entries.iter().map(|e| (e.id, e)).collect();
    for entry in &order.entries {
        let verified_by = entry.verified_by_entry_id.and_then(|id| by_id.get(&id)).map(|v| {
            format!("{}: {}", EntryStatus::label(&v.status).to_lowercase(), format_dt(&v.created_at))
        });
        entry_block(&mut doc, entry, verified_by.as_deref());
    }

    if !order.notes.trim().is_empty() {
        doc.heading("Observaciones");
        doc.paragraph(&order.notes, 9.5, false, INK);
    }

    doc.ensure(110.0);
    doc.space(50.0);
    let half = (CONTENT_W - 40.0) / 2.0;
    let y = doc.y;
    doc.line(MARGIN, y, MARGIN + half, y, MUTED, 0.6);
    doc.line(MARGIN + half + 40.0, y, PAGE_W - MARGIN, y, MUTED, 0.6);
    doc.text_at(MARGIN, y - 12.0, 8.5, false, MUTED, &format!("Firma del cliente ({})", order.customer));
    doc.text_at(MARGIN + half + 40.0, y - 12.0, 8.5, false, MUTED, &format!("Firma del técnico ({})", order.technician.as_deref().unwrap_or("")));
    doc.y = y - 30.0;
    doc.paragraph(PRIVACY_NOTE, 7.5, false, MUTED);
    doc.finish(generated_at)
}

pub fn consent_label(s: &str) -> &'static str {
    match s {
        "granted" => "Concedido",
        "denied" => "No concedido",
        "revoked" => "Revocado",
        _ => "Pendiente",
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Count {
    pub key: String,
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct PeriodSummary {
    pub from: String,
    pub to: String,
    pub orders: i64,
    pub by_status: Vec<Count>,
    pub by_type: Vec<Count>,
    pub consent: Vec<Count>,
    pub entries_by_kind: Vec<Count>,
    pub pending_verification: i64,
    pub avg_days_to_delivery: Option<f64>,
    pub resets: Vec<Count>,
    pub frp_assessments: i64,
    pub frp_stopped: i64,
}

pub fn validate_range(from: &str, to: &str) -> AppResult<()> {
    let parse = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| AppError::Validation(format!("Fecha no válida: «{s}».")));
    if parse(from)? > parse(to)? {
        return Err(AppError::Validation("La fecha inicial es posterior a la final.".into()));
    }
    Ok(())
}

fn counts(conn: &Connection, sql: &str, from: &str, to: &str, label: fn(&str) -> &'static str) -> AppResult<Vec<Count>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![from, to], |r| {
        let key: String = r.get(0)?;
        Ok(Count { label: label(&key).to_string(), key, count: r.get(1)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

const IN_RANGE: &str = "substr(received_at, 1, 10) BETWEEN ?1 AND ?2";

pub fn period_summary(conn: &Connection, from: &str, to: &str) -> AppResult<PeriodSummary> {
    validate_range(from, to)?;
    let one = |sql: &str| -> AppResult<i64> { Ok(conn.query_row(sql, params![from, to], |r| r.get(0))?) };
    let reset_label: fn(&str) -> &'static str = |s| match s {
        "verified" => "Verificados",
        "failed" => "No verificados",
        "cancelled" => "Cancelados",
        "prepared" => "Preparados sin confirmar",
        _ => "Pendientes de verificación",
    };
    Ok(PeriodSummary {
        from: from.into(),
        to: to.into(),
        orders: one(&format!("SELECT COUNT(*) FROM service_orders WHERE {IN_RANGE}"))?,
        by_status: counts(conn, &format!("SELECT status, COUNT(*) FROM service_orders WHERE {IN_RANGE} GROUP BY status ORDER BY COUNT(*) DESC"), from, to, OrderStatus::label)?,
        by_type: counts(conn, &format!("SELECT request_type, COUNT(*) FROM service_orders WHERE {IN_RANGE} GROUP BY request_type ORDER BY COUNT(*) DESC"), from, to, RequestType::label)?,
        consent: counts(conn, &format!("SELECT consent_status, COUNT(*) FROM service_orders WHERE {IN_RANGE} GROUP BY consent_status ORDER BY COUNT(*) DESC"), from, to, consent_label)?,
        entries_by_kind: counts(
            conn,
            "SELECT e.kind, COUNT(*) FROM order_entries e JOIN service_orders o ON o.id = e.order_id WHERE substr(o.received_at, 1, 10) BETWEEN ?1 AND ?2 AND e.kind <> 'note' GROUP BY e.kind ORDER BY COUNT(*) DESC",
            from,
            to,
            EntryKind::label,
        )?,
        pending_verification: one(
            "SELECT COUNT(*) FROM order_entries e JOIN service_orders o ON o.id = e.order_id WHERE substr(o.received_at, 1, 10) BETWEEN ?1 AND ?2 \
             AND o.status <> 'cancelled' AND e.status = 'pending_verification' AND NOT EXISTS (SELECT 1 FROM order_entries v WHERE v.verifies_entry_id = e.id)",
        )?,
        avg_days_to_delivery: conn.query_row(
            &format!("SELECT AVG(julianday(delivered_at) - julianday(received_at)) FROM service_orders WHERE {IN_RANGE} AND delivered_at IS NOT NULL"),
            params![from, to],
            |r| r.get(0),
        )?,
        resets: counts(conn, "SELECT status, COUNT(*) FROM resets WHERE substr(created_at, 1, 10) BETWEEN ?1 AND ?2 GROUP BY status ORDER BY COUNT(*) DESC", from, to, reset_label)?,
        frp_assessments: one("SELECT COUNT(*) FROM frp_assessments WHERE substr(created_at, 1, 10) BETWEEN ?1 AND ?2")?,
        frp_stopped: one("SELECT COUNT(*) FROM frp_assessments WHERE substr(created_at, 1, 10) BETWEEN ?1 AND ?2 AND outcome = 'stop_no_authorization'")?,
    })
}

fn join_counts(c: &[Count]) -> String {
    if c.is_empty() {
        return "—".into();
    }
    c.iter().map(|c| format!("{}: {}", c.label, c.count)).collect::<Vec<_>>().join(" · ")
}

pub fn period_pdf(summary: &PeriodSummary, orders: &[OrderSummary], workshop: &str, generated_by: &str, generated_at: &str) -> Vec<u8> {
    let range = format!("{} – {}", format_date(&summary.from), format_date(&summary.to));
    let footer = format!("{workshop} · Reporte del periodo {range} · Generado por {generated_by}");
    let mut doc = Document::new(&format!("Reporte del periodo {range}"), &footer);
    header_band(&mut doc, workshop, "Reporte del periodo", &range, &format!("Generado el {}", format_dt(generated_at)));

    doc.heading("Resumen");
    doc.key_values(&[
        ("Órdenes recibidas", summary.orders.to_string()),
        ("Por estado", join_counts(&summary.by_status)),
        ("Por tipo de servicio", join_counts(&summary.by_type)),
        ("Consentimiento", join_counts(&summary.consent)),
        ("Registro", join_counts(&summary.entries_by_kind)),
        ("Pendiente de verificación", format!("{} entradas sin evidencia", summary.pending_verification)),
        (
            "Tiempo medio hasta la entrega",
            summary.avg_days_to_delivery.map(|d| format!("{:.1} días", d).replace('.', ",")).unwrap_or_else(|| "Sin entregas en el periodo".into()),
        ),
        ("Restablecimientos", join_counts(&summary.resets)),
        ("Diagnósticos FRP", format!("{} ({} detenidos por falta de autorización)", summary.frp_assessments, summary.frp_stopped)),
    ]);

    doc.heading("Órdenes del periodo");
    if orders.is_empty() {
        doc.paragraph("No hay órdenes recibidas en este periodo.", 9.5, false, MUTED);
    } else {
        let rows: Vec<Vec<String>> = orders
            .iter()
            .map(|o| {
                vec![
                    o.number.clone(),
                    format_date(&o.received_at),
                    o.customer.clone(),
                    o.device.clone(),
                    RequestType::label(&o.request_type).to_string(),
                    OrderStatus::label(&o.status).to_string(),
                    if o.pending_verification > 0 { o.pending_verification.to_string() } else { "—".into() },
                ]
            })
            .collect();
        doc.table(&["Número", "Recibida", "Cliente", "Equipo", "Servicio", "Estado", "Pend."], &[0.15, 0.11, 0.18, 0.18, 0.16, 0.15, 0.07], &rows);
    }
    doc.space(16.0);
    doc.paragraph(PRIVACY_NOTE, 7.5, false, MUTED);
    doc.finish(generated_at)
}

/// Spreadsheet-safe cell: neutralizes formula injection and quotes when needed (`;` separator for Spanish Excel).
fn csv_cell(value: &str) -> String {
    let mut v = value.replace(['\r', '\n'], " ");
    if v.starts_with(['=', '+', '-', '@', '\t']) {
        v.insert(0, '\'');
    }
    if v.contains([';', '"']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

pub fn orders_csv(orders: &[OrderSummary]) -> String {
    let mut out = String::from("\u{feff}Número;Recibida;Cliente;Equipo;Tipo de servicio;Estado;Consentimiento;Técnico;Pendientes de verificación\r\n");
    for o in orders {
        let cells = [
            o.number.clone(),
            format_dt(&o.received_at),
            o.customer.clone(),
            o.device.clone(),
            RequestType::label(&o.request_type).into(),
            OrderStatus::label(&o.status).into(),
            consent_label(&o.consent_status).into(),
            o.technician.clone().unwrap_or_default(),
            o.pending_verification.to_string(),
        ];
        out.push_str(&cells.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(";"));
        out.push_str("\r\n");
    }
    out
}

/// Writes `bytes` to `dir/name.ext`, adding " (2)", " (3)"… instead of overwriting an existing file.
pub fn save(dir: &Path, name: &str, ext: &str, bytes: &[u8]) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '-' }).collect();
    let mut path = dir.join(format!("{safe}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{safe} ({n}).{ext}"));
        n += 1;
    }
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::services::orders::{self, ConsentDecision, NewEntry, NewOrder};

    fn sample(conn: &Connection) -> Order {
        conn.execute("INSERT INTO users (username, display_name, role, password_hash, created_at) VALUES ('t','Técnico Uno','technician','h','t')", []).unwrap();
        conn.execute(
            "INSERT INTO devices (fingerprint, serial_masked, serial_encrypted, manufacturer, model, android_version, api_level, first_seen, last_seen) \
             VALUES ('f','R58***DE','x','Samsung','SM-A145','14',34,'t','t')",
            [],
        )
        .unwrap();
        let o = orders::create(conn, 1, Some(1), &NewOrder {
            customer_name: "José Pérez (Ñandú)".into(),
            customer_contact: "=HYPERLINK(\"x\")".into(),
            device_label: String::new(),
            request_type: RequestType::Accounts,
            reported_problem: "Quiere vender el teléfono; pide retirar su cuenta.".into(),
        })
        .unwrap();
        let o = orders::record_consent(conn, 1, o.id, ConsentDecision::Granted, "José Pérez", Some(OwnershipMethod::OwnerPresentId), "").unwrap();
        orders::add_entry(conn, 1, o.id, &NewEntry {
            kind: EntryKind::Instruction,
            title: "Quitar la cuenta Google desde Ajustes".into(),
            detail: "Ajustes › Cuentas › Quitar cuenta".into(),
            status: EntryStatus::PendingVerification,
            verifies_entry_id: None,
        })
        .unwrap()
    }

    #[test]
    fn order_pdf_marks_pending_and_keeps_privacy() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let order = sample(&conn);
        let bytes = order_pdf(&order, "Taller Centro", "Técnico Uno", "2026-11-21T13:29:00");
        let text = String::from_utf8_lossy(&bytes);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(text.contains("PENDIENTE DE VERIFICACI\\323N"), "pending entries are flagged");
        assert!(text.contains("INSTRUCCI\\323N"), "kind label shown");
        assert!(text.contains("R58***DE") && !text.contains("serial_encrypted"));
        assert!(text.contains("Jos\\351 P\\351rez \\(\\321and\\372\\)"));
    }

    #[test]
    fn period_summary_counts_and_pdf() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        let order = sample(&conn);
        let day = &order.received_at[..10];
        let s = period_summary(&conn, day, day).unwrap();
        assert_eq!(s.orders, 1);
        assert_eq!(s.pending_verification, 1);
        assert_eq!(s.by_type, vec![Count { key: "accounts".into(), label: "Gestión de cuentas".into(), count: 1 }]);
        assert!(period_summary(&conn, "2026-12-01", "2026-11-01").is_err());
        assert!(period_summary(&conn, "ayer", "hoy").is_err());
        let empty = period_summary(&conn, "2000-01-01", "2000-01-31").unwrap();
        assert_eq!(empty.orders, 0);
        let list = orders::list(&conn, &orders::OrderFilter { from: Some(day.into()), to: Some(day.into()), ..Default::default() }).unwrap();
        let pdf = period_pdf(&s, &list, "Taller", "Ana", "2026-11-21T13:00:00");
        assert!(String::from_utf8_lossy(&pdf).contains(&order.number));
    }

    #[test]
    fn csv_neutralizes_formulas() {
        assert_eq!(csv_cell("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(csv_cell("-2"), "'-2");
        assert_eq!(csv_cell("a;b"), "\"a;b\"");
        assert_eq!(csv_cell("di \"hola\""), "\"di \"\"hola\"\"\"");
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn();
        sample(&conn);
        let list = orders::list(&conn, &Default::default()).unwrap();
        let csv = orders_csv(&list);
        assert!(csv.starts_with('\u{feff}') && csv.lines().count() == 2);
        assert!(!csv.contains("HYPERLINK"), "contact is not exported");
    }

    #[test]
    fn save_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let a = save(dir.path(), "Reporte OS-2026-0001", "pdf", b"a").unwrap();
        let b = save(dir.path(), "Reporte OS-2026-0001", "pdf", b"b").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read(&a).unwrap(), b"a");
        let odd = save(dir.path(), "../x/y", "csv", b"c").unwrap();
        assert_eq!(odd.parent().unwrap(), dir.path(), "no path traversal");
    }
}
