import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Checklist, DeviceView, DevicesSnapshot, Procedure, ProcedureCatalog, User } from "@/types/api";

const s = vi.hoisted(() => ({
  user: null as User | null,
  snapshot: null as DevicesSnapshot | null,
  review: vi.fn(),
  check: vi.fn(),
  history: vi.fn(),
  open: vi.fn(),
  set: vi.fn(),
  catalog: vi.fn(),
}));
vi.mock("@/hooks/useSession", () => ({ useCurrentUser: () => s.user }));
vi.mock("@/hooks/useDevices", () => ({ useDevices: () => ({ snapshot: s.snapshot, error: null }) }));
vi.mock("@/services/api", async (orig) => {
  const real = await orig<typeof import("@/services/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      procedures: { review: s.review, catalog: s.catalog },
      accounts: { check: s.check, history: s.history },
      checklist: { open: s.open, set: s.set },
    },
  };
});

import { ProcedureCard, androidWarning } from "@/components/ProcedureCard";
import { AccountCheckPanel } from "@/components/AccountCheckPanel";
import { TransferChecklist } from "@/components/TransferChecklist";
import { BrandsPage } from "@/pages/BrandsPage";
import { Toaster } from "@/components/Toaster";

const admin: User = { id: 1, username: "ana", display_name: "Ana", role: "admin", active: true, last_login: null };
const proc: Procedure = {
  id: "samsung-account-remove", kind: "account_removal", service: "Samsung Account", title: "Cerrar sesión en la cuenta Samsung", version: 2,
  android_min: 8, android_max: null, summary: "Resumen", steps: ["Paso uno"], verification: ["Verificación"], links: [{ label: "Cuenta Samsung", url: "https://account.samsung.com" }],
  notes: [], review: null,
};
const device: DeviceView = {
  id: "abc", serial_masked: "R58***DE", mode: "adb", network: false, usb_path: "1-1", manufacturer: "Samsung", brand: "samsung", model: "SM-A145", product: null,
  codename: null, android_version: "7", api_level: 24, security_patch: null, build_id: null, bootloader_locked: null, battery: null, storage: null,
  identified: true, missing: [], first_seen: "2026-11-21T10:00:00", last_seen: "2026-11-21T10:00:00", identify_error: null,
};
const tools = { adb_path: "/x/adb", adb_version: null, fastboot_path: null, last_scan: null, last_error: null };

beforeEach(() => {
  vi.clearAllMocks();
  s.user = admin;
  s.snapshot = { devices: [], tools, events: [] };
  s.history.mockResolvedValue([]);
});

describe("procedure card", () => {
  it("is always owner guidance and shows its review state", () => {
    const { rerender } = render(<Toaster><ProcedureCard procedure={proc} canReview={false} onReviewed={() => {}} /></Toaster>);
    expect(screen.getByText("Instrucción para el propietario")).toBeInTheDocument();
    expect(screen.getByText("Pendiente de revisión del taller")).toBeInTheDocument();
    rerender(<Toaster><ProcedureCard procedure={{ ...proc, review: { version: 1, reviewed_by: "Ana", reviewed_at: "2026-11-01T10:00:00", note: "", current: false } }} canReview={false} onReviewed={() => {}} /></Toaster>);
    expect(screen.getByText("Revisión desactualizada (v1)")).toBeInTheDocument();
  });

  it("warns when the device runs an older Android and lets admins review", async () => {
    s.review.mockResolvedValue(undefined);
    const onReviewed = vi.fn();
    render(<Toaster><ProcedureCard procedure={proc} androidVersion="7" canReview onReviewed={onReviewed} defaultOpen /></Toaster>);
    expect(screen.getByText(/Escrito para Android 8 o posterior/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Marcar como revisado" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirmar revisión", hidden: true }));
    await waitFor(() => expect(s.review).toHaveBeenCalledWith("samsung-account-remove", ""));
    expect(onReviewed).toHaveBeenCalled();
  });

  it("hides the review action from technicians", () => {
    render(<Toaster><ProcedureCard procedure={proc} canReview={false} onReviewed={() => {}} defaultOpen /></Toaster>);
    expect(screen.queryByRole("button", { name: "Marcar como revisado" })).toBeNull();
    expect(androidWarning({ android_min: 8, android_max: null }, "14")).toBeNull();
  });
});

describe("account check", () => {
  it("shows types and counts and confirms a verified removal", async () => {
    s.check.mockResolvedValue({
      check: { id: 2, created_at: "2026-11-21T10:05:00", summary: { types: [] }, google_count: 0, protected_count: 0, author: "Ana" },
      previous: { id: 1, created_at: "2026-11-21T10:00:00", summary: { types: [{ label: "Google", count: 1, protects: true }] }, google_count: 1, protected_count: 1, author: "Ana" },
      removal_verified: true,
    });
    render(<Toaster><AccountCheckPanel device={device} /></Toaster>);
    fireEvent.click(screen.getByRole("button", { name: "Comprobar cuentas en el equipo" }));
    expect(await screen.findByText("Retiro de cuentas verificado")).toBeInTheDocument();
    expect(screen.getByText("No hay cuentas con sesión iniciada")).toBeInTheDocument();
  });

  it("cannot check an unauthorized device", () => {
    render(<Toaster><AccountCheckPanel device={{ ...device, mode: "unauthorized", identified: false }} /></Toaster>);
    expect(screen.getByRole("button", { name: "Comprobar cuentas en el equipo" })).toBeDisabled();
  });
});

describe("transfer checklist", () => {
  const base: Checklist = {
    id: 7, device_id: null, device_label: "Samsung A14", status: "in_progress", created_at: "t", updated_at: "t",
    items: [{ key: "google_removed", label: "Cuentas Google retiradas desde Ajustes", needs_account_evidence: true, state: { done: false, note: "", evidence: "", by: null, at: null } }],
  };
  it("only opens a list for a typed device when asked, then asks for written on-screen verification", async () => {
    s.open.mockResolvedValue(base);
    s.set.mockResolvedValue({ ...base, items: [{ ...base.items[0]!, state: { done: true, note: "Pantalla de cuentas vacía", evidence: "Verificado en pantalla por el técnico: Pantalla de cuentas vacía", by: 1, at: "2026-11-21T10:00:00" } }] });
    const { rerender } = render(<Toaster><TransferChecklist deviceId={null} deviceLabel="Sam" /></Toaster>);
    rerender(<Toaster><TransferChecklist deviceId={null} deviceLabel="Samsung A14" /></Toaster>);
    expect(s.open).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Abrir lista para «Samsung A14»" }));
    expect(s.open).toHaveBeenCalledTimes(1);
    expect(s.open).toHaveBeenCalledWith(null, "Samsung A14");
    fireEvent.click(await screen.findByRole("button", { name: /Cuentas Google retiradas/ }));
    fireEvent.change(screen.getByLabelText("Qué verificó"), { target: { value: "Pantalla de cuentas vacía" } });
    fireEvent.click(screen.getByRole("button", { name: "Registrar", hidden: true }));
    await waitFor(() => expect(s.set).toHaveBeenCalledWith(7, "google_removed", true, "Pantalla de cuentas vacía"));
    expect(await screen.findByText(/Verificado en pantalla por el técnico/)).toBeInTheDocument();
  });
});

describe("brands page", () => {
  const catalog: ProcedureCatalog = {
    device_brand: "samsung",
    brands: ["google", "samsung", "xiaomi", "motorola", "huawei", "oppo", "realme", "vivo", "tecno", "infinix", "pixel"].map((b) => ({
      brand: b, name: b === "xiaomi" ? "Xiaomi / Mi Account" : b[0]!.toUpperCase() + b.slice(1), updated: "2026-11-21",
      support_links: [], procedures: [{ ...proc, id: `${b}-x`, title: `Procedimiento ${b}` }],
    })),
  };
  it("lists the ten brand modules, preselects the connected device's brand and searches", async () => {
    s.snapshot = { devices: [device], tools, events: [] };
    s.catalog.mockResolvedValue(catalog);
    render(<Toaster><BrandsPage /></Toaster>);
    expect(await screen.findByRole("heading", { name: "Procedimiento samsung" })).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Fabricantes" });
    expect(nav.querySelectorAll("li button")).toHaveLength(10);
    fireEvent.change(screen.getByLabelText("Buscar marca"), { target: { value: "mi acc" } });
    expect(nav.querySelectorAll("li button")).toHaveLength(1);
  });
});
