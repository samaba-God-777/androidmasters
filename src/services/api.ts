import { invoke } from "@tauri-apps/api/core";
import type {
  ConsentStatus, LinkCandidate, NewEntry, NewOrder, Order, OrderFilter, OrderStatus, OrderSummary, OwnershipMethod, PeriodSummary, ReportSettings, SavedFile,
  AccountCheck, AccountCheckResult, ResetChallenge, ResetMethod, ResetPreflight, ResetRecord, AppErrorPayload, AppInfo, AuthStatus, Checklist, ProcedureCatalog, FrpAssessment, FrpIndicators, FrpObservation, FrpRecord, BackupResult, CheckItem, DashboardStats, DeviceView, DevicesSnapshot, ErrorCode, Role, ToolsSettings, User,
} from "@/types/api";

/** Error thrown by every backend call. `message` is safe to show to the technician. */
export class ApiError extends Error {
  readonly code: ErrorCode;
  constructor(code: ErrorCode, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
  }
}

function isPayload(value: unknown): value is AppErrorPayload {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (isPayload(error)) return new ApiError(error.code, String(error.message));
  if (typeof error === "string") return new ApiError("internal", error);
  return new ApiError("internal", "Error inesperado. Revise el registro de la aplicación.");
}

/** True when running inside the Tauri shell (false in a plain browser / tests). */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const SESSION_EXPIRED_EVENT = "am:session-expired";

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri()) {
    throw new ApiError("unavailable", "Abra AndroidMasters como aplicación de escritorio para conectar con el backend.");
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    const e = toApiError(error);
    // The backend closed an idle session: the shell returns to the sign-in screen.
    if (e.code === "session_expired") window.dispatchEvent(new CustomEvent(SESSION_EXPIRED_EVENT, { detail: e.message }));
    throw e;
  }
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  createBackup: () => call<BackupResult>("create_backup"),
  authStatus: () => call<AuthStatus>("auth_status"),
  createFirstAdmin: (username: string, displayName: string, password: string) =>
    call<User>("create_first_admin", { username, displayName, password }),
  login: (username: string, password: string) => call<User>("login", { username, password }),
  logout: () => call<void>("logout"),
  changePassword: (current: string, newPassword: string) => call<void>("change_password", { current, newPassword }),
  users: {
    list: () => call<User[]>("users_list"),
    create: (username: string, displayName: string, password: string, role: Role) =>
      call<User>("users_create", { username, displayName, password, role }),
    setActive: (id: number, active: boolean) => call<User>("users_set_active", { id, active }),
    setRole: (id: number, role: Role) => call<User>("users_set_role", { id, role }),
    resetPassword: (id: number, password: string) => call<void>("users_reset_password", { id, password }),
  },
  dashboardStats: () => call<DashboardStats>("dashboard_stats"),
  devices: {
    snapshot: () => call<DevicesSnapshot>("devices_snapshot"),
    refresh: (id: string) => call<DeviceView>("device_refresh", { id }),
    revealSerial: (id: string) => call<string>("device_reveal_serial", { id }),
    diagnose: (id: string | null) => call<CheckItem[]>("usb_diagnose", { id }),
    restartServer: () => call<void>("adb_restart_server"),
  },
  frp: {
    indicators: (id: string) => call<FrpIndicators>("frp_indicators", { id }),
    assess: (observation: FrpObservation) => call<FrpAssessment>("frp_assess", { observation }),
    record: (device: string | null, deviceLabel: string, observation: FrpObservation, notes: string) =>
      call<{ id: number; assessment: FrpAssessment }>("frp_record", { device, deviceLabel, observation, notes }),
    history: () => call<FrpRecord[]>("frp_history"),
  },
  procedures: {
    catalog: (device: string | null) => call<ProcedureCatalog>("procedures_catalog", { device }),
    review: (procedureId: string, note: string) => call<void>("procedure_review", { procedureId, note }),
  },
  accounts: {
    check: (device: string) => call<AccountCheckResult>("accounts_check", { device }),
    history: (device: string) => call<AccountCheck[]>("accounts_history", { device }),
  },
  checklist: {
    open: (device: string | null, deviceLabel: string) => call<Checklist>("checklist_open", { device, deviceLabel }),
    set: (id: number, key: string, done: boolean, note: string) => call<Checklist>("checklist_set", { id, key, done, note }),
  },
  reset: {
    preflight: (device: string | null, deviceLabel: string) => call<ResetPreflight>("reset_preflight", { device, deviceLabel }),
    prepare: (device: string | null, deviceLabel: string, method: ResetMethod) => call<ResetRecord>("reset_prepare", { device, deviceLabel, method }),
    challenge: (id: number) => call<ResetChallenge>("reset_challenge", { id }),
    confirm: (id: number, token: string, typed: string, ownerName: string) => call<ResetRecord>("reset_confirm", { id, token, typed, ownerName }),
    assist: (id: number, device: string, action: "open_settings" | "reboot_recovery") => call<ResetRecord>("reset_assist", { id, device, action }),
    manualStart: (id: number) => call<ResetRecord>("reset_manual_start", { id }),
    verify: (id: number, device: string | null, setupWizardShown: boolean, asksPreviousAccount: boolean, note: string) =>
      call<ResetRecord>("reset_verify", { id, device, setupWizardShown, asksPreviousAccount, note }),
    cancel: (id: number) => call<ResetRecord>("reset_cancel", { id }),
    list: () => call<ResetRecord[]>("resets_list"),
  },
  orders: {
    list: (filter: OrderFilter) => call<OrderSummary[]>("orders_list", { filter }),
    get: (id: number) => call<Order>("order_get", { id }),
    create: (device: string | null, order: NewOrder) => call<Order>("order_create", { device, order }),
    consent: (id: number, decision: Exclude<ConsentStatus, "pending">, byName: string, method: OwnershipMethod | null, note: string) =>
      call<Order>("order_consent", { id, decision, byName, method, note }),
    addEntry: (id: number, entry: NewEntry) => call<Order>("order_add_entry", { id, entry }),
    setStatus: (id: number, status: OrderStatus, note: string) => call<Order>("order_set_status", { id, status, note }),
    setNotes: (id: number, notes: string) => call<Order>("order_set_notes", { id, notes }),
    candidates: (id: number) => call<LinkCandidate[]>("order_candidates", { id }),
    link: (id: number, sources: string[]) => call<Order>("order_link", { id, sources }),
    report: (id: number) => call<SavedFile>("order_report", { id }),
  },
  reports: {
    summary: (from: string, to: string) => call<PeriodSummary>("reports_summary", { from, to }),
    periodPdf: (from: string, to: string) => call<SavedFile>("reports_period_pdf", { from, to }),
    exportCsv: (from: string, to: string) => call<SavedFile>("reports_export_csv", { from, to }),
    settings: () => call<ReportSettings>("report_settings"),
    setSettings: (workshopName: string) => call<ReportSettings>("report_settings_set", { workshopName }),
    reveal: (path: string) => call<void>("reveal_report", { path }),
  },
  tools: {
    get: () => call<ToolsSettings>("tools_settings"),
    set: (dir: string | null) => call<ToolsSettings>("tools_settings_set", { dir }),
  },
};

export const EVENTS = { snapshot: "devices://snapshot", connection: "devices://event" } as const;
