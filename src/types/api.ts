/** Types mirrored from the Rust backend (src-tauri/src/commands). */

export type ErrorCode =
  | "database"
  | "validation"
  | "forbidden"
  | "not_found"
  | "consent_required"
  | "session_expired"
  | "device"
  | "command"
  | "io"
  | "internal"
  | "unavailable";

export interface AppErrorPayload {
  code: ErrorCode;
  message: string;
}

export interface AppInfo {
  version: string;
  schema_version: number;
  database_ok: boolean;
  platform: string;
  arch: string;
}

export interface BackupResult {
  path: string;
  size_bytes: number;
}

export type Role = "admin" | "technician";

export interface User {
  id: number;
  username: string;
  display_name: string;
  role: Role;
  active: boolean;
  last_login: string | null;
}

export interface AuthStatus {
  has_users: boolean;
  user: User | null;
}

export type OrderStatus = "open" | "in_progress" | "pending_verification" | "completed" | "delivered" | "cancelled";

export interface AuditEntry {
  id: number;
  created_at: string;
  event: string;
  severity: "info" | "warning" | "critical";
  user: string | null;
  details: Record<string, unknown> | null;
}

export interface DashboardStats {
  orders_active: number;
  orders_pending_verification: number;
  orders_completed_30d: number;
  diagnostics_30d: number;
  entries_pending_verification: number;
  orders_by_status: Array<{ status: OrderStatus; count: number }>;
  operations_14d: Array<{ day: string; count: number }>;
  recent_activity: AuditEntry[];
}

export type DeviceMode =
  | "adb" | "unauthorized" | "offline" | "no_permissions" | "recovery" | "sideload" | "rescue" | "bootloader" | "fastboot" | "unknown";

export interface Battery {
  level: number | null;
  status: string | null;
  health: string | null;
  temperature_c: number | null;
  source: string | null;
}

export interface DeviceView {
  id: string;
  serial_masked: string;
  mode: DeviceMode;
  network: boolean;
  usb_path: string | null;
  manufacturer: string | null;
  brand: string | null;
  model: string | null;
  product: string | null;
  codename: string | null;
  android_version: string | null;
  api_level: number | null;
  security_patch: string | null;
  build_id: string | null;
  bootloader_locked: boolean | null;
  battery: Battery | null;
  storage: { total_bytes: number; available_bytes: number } | null;
  identified: boolean;
  missing: string[];
  first_seen: string;
  last_seen: string;
  identify_error: string | null;
}

export interface ConnectionEvent {
  at: string;
  kind: "connected" | "disconnected" | "mode_changed" | "error";
  device: string | null;
  mode: DeviceMode | null;
  message: string;
}

export interface ToolStatus {
  adb_path: string | null;
  adb_version: string | null;
  fastboot_path: string | null;
  last_scan: string | null;
  last_error: string | null;
}

export interface DevicesSnapshot {
  devices: DeviceView[];
  tools: ToolStatus;
  events: ConnectionEvent[];
}

export type CheckStatus = "pass" | "info" | "warn" | "fail";

export interface CheckItem {
  key: string;
  title: string;
  status: CheckStatus;
  detail: string;
  recommendation: string;
}

export interface ToolsSettings {
  platform_tools_dir: string | null;
  adb_path: string | null;
  fastboot_path: string | null;
}

export type ResetOrigin = "not_reset" | "settings" | "recovery_or_buttons" | "unknown";

export interface FrpObservation {
  asks_previous_account: boolean | null;
  owner_or_authorized: boolean | null;
  reset_origin: ResetOrigin | null;
  second_hand: boolean | null;
  password_changed_recently: boolean | null;
  knows_credentials: boolean | null;
  can_recover_account: boolean | null;
  has_purchase_proof: boolean | null;
}

export type FrpOutcome =
  | "incomplete" | "no_indication" | "stop_no_authorization" | "owner_signs_in" | "wait_security_period" | "account_recovery" | "manufacturer_support";

export interface FrpAssessment {
  outcome: FrpOutcome;
  headline: string;
  possible_causes: string[];
  next_steps: string[];
  documentation: string[];
  stop: boolean;
  pending_questions: string[];
}

export interface AccountSummary {
  types: Array<{ label: string; count: number; protects: boolean }>;
}

export interface FrpIndicators {
  items: CheckItem[];
  accounts: AccountSummary | null;
}

export interface FrpRecord {
  id: number;
  created_at: string;
  device: string;
  outcome: FrpOutcome;
  author: string | null;
}

export interface ProcedureLink {
  label: string;
  url: string;
}

export interface ProcedureReview {
  version: number;
  reviewed_by: string;
  reviewed_at: string;
  note: string;
  current: boolean;
}

export interface Procedure {
  id: string;
  kind: "account_removal" | "account_recovery" | "transfer" | string;
  service: string;
  title: string;
  version: number;
  android_min: number | null;
  android_max: number | null;
  summary: string;
  steps: string[];
  verification: string[];
  links: ProcedureLink[];
  notes: string[];
  review: ProcedureReview | null;
}

export interface BrandModule {
  brand: string;
  name: string;
  updated: string;
  support_links: ProcedureLink[];
  procedures: Procedure[];
}

export interface ProcedureCatalog {
  brands: BrandModule[];
  device_brand: string | null;
}

export interface AccountCheck {
  id: number;
  created_at: string;
  summary: AccountSummary;
  google_count: number;
  protected_count: number;
  author: string | null;
}

export interface AccountCheckResult {
  check: AccountCheck;
  previous: AccountCheck | null;
  removal_verified: boolean;
}

export interface ChecklistItem {
  key: string;
  label: string;
  needs_account_evidence: boolean;
  state: { done: boolean; note: string; evidence: string; by: number | null; at: string | null };
}

export interface Checklist {
  id: number;
  device_id: number | null;
  device_label: string;
  status: "in_progress" | "complete";
  created_at: string;
  updated_at: string;
  items: ChecklistItem[];
}

export type ResetMethod = "settings" | "recovery";
export type ResetStatus = "prepared" | "confirmed" | "pending_verification" | "verified" | "failed" | "cancelled";

export interface ResetPreflight {
  items: CheckItem[];
  ready: boolean;
  checklist_id: number | null;
}

export interface ResetRecord {
  id: number;
  device_id: number | null;
  device_label: string;
  checklist_id: number | null;
  method: ResetMethod;
  status: ResetStatus;
  confirmed_at: string | null;
  confirmed_by: string | null;
  owner_name: string;
  actions: Array<{ at: string; kind: string; ok: boolean; detail: string }>;
  verification: {
    at: string; setup_wizard_shown: boolean; asks_previous_account: boolean; disconnect_observed: string | null;
    note: string; verified: boolean; message: string;
  } | null;
  created_at: string;
  instructions: string[];
}

export interface ResetChallenge {
  token: string;
  phrase: string;
  expires_in_seconds: number;
}

export type RequestType = "diagnose" | "accounts" | "frp" | "reset" | "transfer" | "maintenance";
export type ConsentStatus = "pending" | "granted" | "denied" | "revoked";
export type OwnershipMethod = "owner_present_id" | "purchase_proof" | "authorized_representative" | "company_device";
export type EntryKind = "diagnosis" | "instruction" | "action" | "verification" | "consent" | "incident" | "note";
export type EntryStatus = "ok" | "failed" | "pending_verification" | "info";

export interface OrderSummary {
  id: number;
  number: string;
  customer: string;
  device: string;
  request_type: RequestType;
  status: OrderStatus;
  consent_status: ConsentStatus;
  technician: string | null;
  received_at: string;
  updated_at: string;
  pending_verification: number;
}

export interface OrderEntry {
  id: number;
  created_at: string;
  kind: EntryKind;
  title: string;
  detail: string;
  status: EntryStatus;
  author: string | null;
  source: string;
  verifies_entry_id: number | null;
  verified_by_entry_id: number | null;
}

export interface Order {
  id: number;
  number: string;
  customer_id: number;
  customer: string;
  customer_contact: string;
  device_id: number | null;
  device_label: string;
  device: {
    manufacturer: string; model: string; serial_masked: string; android_version: string; api_level: number | null; security_patch: string;
  } | null;
  request_type: RequestType;
  reported_problem: string;
  status: OrderStatus;
  consent_status: ConsentStatus;
  consent_by: string;
  consent_at: string | null;
  ownership_method: OwnershipMethod | "";
  technician: string | null;
  received_at: string;
  delivered_at: string | null;
  created_at: string;
  updated_at: string;
  notes: string;
  entries: OrderEntry[];
  next_statuses: OrderStatus[];
}

export interface NewOrder {
  customer_name: string;
  customer_contact: string;
  device_label: string;
  request_type: RequestType;
  reported_problem: string;
}

export interface NewEntry {
  kind: EntryKind;
  title: string;
  detail: string;
  status: EntryStatus;
  verifies_entry_id: number | null;
}

export interface OrderFilter {
  status?: OrderStatus | "active" | null;
  query?: string;
  from?: string | null;
  to?: string | null;
  limit?: number | null;
}

export interface LinkCandidate {
  source: string;
  created_at: string;
  kind: EntryKind;
  title: string;
  detail: string;
  status: EntryStatus;
  verifies_source: string | null;
  linked: boolean;
}

export interface SavedFile {
  path: string;
  name: string;
  folder: string;
}

export interface Count {
  key: string;
  label: string;
  count: number;
}

export interface PeriodSummary {
  from: string;
  to: string;
  orders: number;
  by_status: Count[];
  by_type: Count[];
  consent: Count[];
  entries_by_kind: Count[];
  pending_verification: number;
  avg_days_to_delivery: number | null;
  resets: Count[];
  frp_assessments: number;
  frp_stopped: number;
}

export interface ReportSettings {
  workshop_name: string;
  reports_dir: string;
}
