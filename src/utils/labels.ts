import type { CheckStatus, ConsentStatus, DeviceMode, DeviceView, EntryKind, EntryStatus, OrderStatus, OwnershipMethod, RequestType, ResetStatus, Role } from "@/types/api";
import type { Tone } from "@/components/Feedback";

export const ROLE_LABEL: Record<Role, string> = { admin: "Administrador", technician: "Técnico" };

export const ORDER_STATUS: Record<OrderStatus, { label: string; tone: Tone }> = {
  open: { label: "Abierta", tone: "info" },
  in_progress: { label: "En curso", tone: "brand" },
  pending_verification: { label: "Pendiente de verificación", tone: "warn" },
  completed: { label: "Completada", tone: "ok" },
  delivered: { label: "Entregada", tone: "neutral" },
  cancelled: { label: "Cancelada", tone: "neutral" },
};

const EVENTS: Record<string, string> = {
  login: "Inició sesión",
  logout: "Cerró sesión",
  session_expired: "Sesión cerrada por inactividad",
  login_failed: "Intento de acceso fallido",
  user_created: "Cuenta creada",
  user_enabled: "Cuenta activada",
  user_disabled: "Cuenta desactivada",
  user_role_changed: "Rol cambiado",
  user_password_reset: "Contraseña restablecida",
  password_changed: "Cambió su contraseña",
  backup_created: "Copia de seguridad creada",
  serial_revealed: "Número de serie mostrado",
  adb_server_restarted: "Servidor ADB reiniciado",
  settings_changed: "Configuración cambiada",
  frp_assessment_recorded: "Diagnóstico FRP registrado",
  procedure_reviewed: "Procedimiento revisado",
  transfer_checklist_completed: "Lista de transferencia completada",
  reset_confirmed: "Restablecimiento confirmado",
  reset_verified: "Restablecimiento verificado",
  reset_failed: "Restablecimiento no verificado",
  reset_cancelled: "Restablecimiento cancelado",
  frp_stopped_no_authorization: "FRP: servicio detenido sin autorización",
  order_created: "Orden creada",
  order_consent_granted: "Consentimiento concedido",
  order_consent_denied: "Consentimiento no concedido",
  order_consent_revoked: "Consentimiento revocado",
  order_status_changed: "Estado de orden cambiado",
  order_work_linked: "Trabajo vinculado a una orden",
  order_report_generated: "Reporte de orden generado",
  period_report_generated: "Reporte del periodo generado",
  orders_exported: "Órdenes exportadas a CSV",
};

export function eventLabel(event: string): string {
  return EVENTS[event] ?? event.replaceAll("_", " ");
}

export const MODE: Record<DeviceMode, { label: string; tone: Tone }> = {
  adb: { label: "ADB autorizado", tone: "ok" },
  unauthorized: { label: "Sin autorizar", tone: "warn" },
  offline: { label: "Fuera de línea", tone: "danger" },
  no_permissions: { label: "Sin permisos USB", tone: "danger" },
  recovery: { label: "Recovery", tone: "info" },
  sideload: { label: "Recovery (sideload)", tone: "info" },
  rescue: { label: "Rescate", tone: "info" },
  bootloader: { label: "Bootloader", tone: "info" },
  fastboot: { label: "Fastboot", tone: "info" },
  unknown: { label: "Desconocido", tone: "neutral" },
};

export const CHECK: Record<CheckStatus, { label: string; tone: Tone }> = {
  pass: { label: "Correcto", tone: "ok" },
  info: { label: "Información", tone: "info" },
  warn: { label: "Atención", tone: "warn" },
  fail: { label: "Fallo", tone: "danger" },
};

export function deviceName(d: Pick<DeviceView, "manufacturer" | "model" | "serial_masked">): string {
  if (d.manufacturer && d.model) return `${d.manufacturer} ${d.model}`;
  return d.model ?? `Dispositivo ${d.serial_masked}`;
}

export function formatBytes(value: number | null | undefined): string {
  if (value == null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = value;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export const RESET_STATUS: Record<ResetStatus, { label: string; tone: Tone }> = {
  prepared: { label: "Preparado", tone: "neutral" },
  confirmed: { label: "Confirmado", tone: "warn" },
  pending_verification: { label: "Pendiente de verificación", tone: "warn" },
  verified: { label: "Verificado", tone: "ok" },
  failed: { label: "No verificado", tone: "danger" },
  cancelled: { label: "Cancelado", tone: "neutral" },
};

export const REQUEST_TYPE: Record<RequestType, string> = {
  diagnose: "Diagnóstico",
  accounts: "Gestión de cuentas",
  frp: "Revisión FRP",
  reset: "Restablecimiento",
  transfer: "Preparación para transferencia",
  maintenance: "Mantenimiento",
};

export const CONSENT: Record<ConsentStatus, { label: string; tone: Tone }> = {
  pending: { label: "Consentimiento pendiente", tone: "warn" },
  granted: { label: "Consentimiento concedido", tone: "ok" },
  denied: { label: "Sin consentimiento", tone: "danger" },
  revoked: { label: "Consentimiento revocado", tone: "danger" },
};

export const OWNERSHIP: Record<OwnershipMethod, string> = {
  owner_present_id: "Propietario presente con identificación",
  purchase_proof: "Factura o prueba de compra a su nombre",
  authorized_representative: "Representante con autorización escrita",
  company_device: "Equipo de empresa: responsable autorizado",
};

export const ENTRY_KIND: Record<EntryKind, { label: string; tone: Tone; hint: string }> = {
  diagnosis: { label: "Diagnóstico", tone: "info", hint: "Información detectada o comunicada por el equipo." },
  instruction: { label: "Instrucción", tone: "brand", hint: "Pasos que debe realizar el propietario. No es una operación realizada." },
  action: { label: "Acción ejecutada", tone: "ok", hint: "Operación realizada por AndroidMasters." },
  verification: { label: "Verificación", tone: "ok", hint: "Evidencia posterior de que algo funcionó o no." },
  consent: { label: "Consentimiento", tone: "brand", hint: "Autorización del propietario." },
  incident: { label: "Incidencia", tone: "danger", hint: "Problema o motivo por el que se detuvo el servicio." },
  note: { label: "Nota", tone: "neutral", hint: "Observación interna." },
};

export const ENTRY_STATUS: Record<EntryStatus, { label: string; tone: Tone }> = {
  ok: { label: "Correcto", tone: "ok" },
  failed: { label: "Fallido", tone: "danger" },
  pending_verification: { label: "Pendiente de verificación", tone: "warn" },
  info: { label: "Informativo", tone: "neutral" },
};

/** Statuses a technician may choose for each kind (mirrors the backend rules). */
export const ENTRY_STATUSES: Partial<Record<EntryKind, EntryStatus[]>> = {
  diagnosis: ["info", "ok", "failed"],
  instruction: ["pending_verification", "info"],
  verification: ["ok", "failed"],
  incident: ["failed", "info"],
  note: ["info"],
};
