import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Order, OrderEntry, OrderSummary, User } from "@/types/api";

const s = vi.hoisted(() => ({
  orders: {
    list: vi.fn(), get: vi.fn(), create: vi.fn(), consent: vi.fn(), addEntry: vi.fn(), setStatus: vi.fn(), setNotes: vi.fn(),
    candidates: vi.fn(), link: vi.fn(), report: vi.fn(),
  },
  reports: { summary: vi.fn(), periodPdf: vi.fn(), exportCsv: vi.fn(), settings: vi.fn(), setSettings: vi.fn(), reveal: vi.fn() },
  user: { id: 1, username: "ana", display_name: "Ana", role: "technician", active: true, last_login: null } as User,
}));
vi.mock("@/hooks/useDevices", () => ({
  useDevices: () => ({ snapshot: { devices: [], tools: { adb_path: "/x/adb", adb_version: null, fastboot_path: null, last_scan: null, last_error: null }, events: [] }, error: null }),
}));
vi.mock("@/hooks/useSession", () => ({ useCurrentUser: () => s.user }));
vi.mock("@/services/api", async (orig) => {
  const real = await orig<typeof import("@/services/api")>();
  return { ...real, api: { ...real.api, orders: s.orders, reports: s.reports } };
});

import { OrdersPage } from "@/pages/OrdersPage";
import { ReportsPage } from "@/pages/ReportsPage";
import { Toaster } from "@/components/Toaster";

const entry = (patch: Partial<OrderEntry>): OrderEntry => ({
  id: 1, created_at: "2026-11-21T10:00:00", kind: "note", title: "Orden creada", detail: "", status: "info", author: "Ana",
  source: "", verifies_entry_id: null, verified_by_entry_id: null, ...patch,
});
const order = (patch: Partial<Order> = {}): Order => ({
  id: 7, number: "OS-2026-0007", customer_id: 1, customer: "Luis Gómez", customer_contact: "", device_id: null, device_label: "Moto G54", device: null,
  request_type: "accounts", reported_problem: "Vender el teléfono", status: "open", consent_status: "pending", consent_by: "", consent_at: null,
  ownership_method: "", technician: "Ana", received_at: "2026-11-21T10:00:00", delivered_at: null, created_at: "2026-11-21T10:00:00",
  updated_at: "2026-11-21T10:00:00", notes: "", entries: [entry({})], next_statuses: ["in_progress", "completed", "cancelled"], ...patch,
});
const summary = (o: Order): OrderSummary => ({
  id: o.id, number: o.number, customer: o.customer, device: o.device_label, request_type: o.request_type, status: o.status,
  consent_status: o.consent_status, technician: o.technician, received_at: o.received_at, updated_at: o.updated_at, pending_verification: 0,
});

beforeEach(() => {
  vi.clearAllMocks();
  s.user = { ...s.user, role: "technician" };
  s.orders.list.mockResolvedValue([summary(order())]);
  s.orders.get.mockResolvedValue(order());
  s.orders.candidates.mockResolvedValue([]);
});

async function dialogTitled(title: string): Promise<HTMLElement> {
  const heading = await screen.findByRole("heading", { name: title, hidden: true });
  return heading.closest("dialog") as HTMLElement;
}

async function openOrder() {
  render(<Toaster><OrdersPage /></Toaster>);
  fireEvent.click(await screen.findByRole("button", { name: /OS-2026-0007/ }));
  await screen.findByRole("heading", { name: "OS-2026-0007" });
}

describe("service orders", () => {
  it("needs a customer and a device before creating", async () => {
    s.orders.create.mockResolvedValue(order());
    render(<Toaster><OrdersPage /></Toaster>);
    fireEvent.click(screen.getByRole("button", { name: "Nueva orden" }));
    const dialog = await screen.findByRole("dialog", { hidden: true });
    const create = within(dialog).getByRole("button", { name: "Crear orden", hidden: true });
    expect(create).toBeDisabled();
    fireEvent.change(within(dialog).getByLabelText("Cliente"), { target: { value: "Luis Gómez" } });
    expect(create).toBeDisabled();
    fireEvent.change(within(dialog).getByLabelText("Marca y modelo"), { target: { value: "Moto G54" } });
    fireEvent.change(within(dialog).getByLabelText("Tipo de servicio"), { target: { value: "accounts" } });
    fireEvent.click(create);
    await waitFor(() => expect(s.orders.create).toHaveBeenCalledWith(null, expect.objectContaining({ customer_name: "Luis Gómez", device_label: "Moto G54", request_type: "accounts" })));
  });

  it("records consent only with a name and how ownership was checked", async () => {
    s.orders.consent.mockResolvedValue(order({ consent_status: "granted", consent_by: "Luis Gómez", ownership_method: "owner_present_id", consent_at: "2026-11-21T10:05:00" }));
    await openOrder();
    const button = screen.getByRole("button", { name: "Registrar consentimiento" });
    expect(button).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Cómo se comprobó"), { target: { value: "owner_present_id" } });
    expect(button).toBeEnabled();
    fireEvent.click(button);
    await waitFor(() => expect(s.orders.consent).toHaveBeenCalledWith(7, "granted", "Luis Gómez", "owner_present_id", ""));
    expect(await screen.findByText(/Autorizado por Luis Gómez/)).toBeInTheDocument();
  });

  it("never offers executed actions for manual entries and flags pending verification", async () => {
    const pending = entry({ id: 2, kind: "instruction", title: "Quitar la cuenta Google desde Ajustes", status: "pending_verification" });
    s.orders.get.mockResolvedValue(order({ status: "in_progress", consent_status: "granted", entries: [entry({}), pending], next_statuses: ["pending_verification", "completed", "cancelled"] }));
    s.orders.addEntry.mockResolvedValue(order({ status: "in_progress", consent_status: "granted", entries: [entry({}), { ...pending, verified_by_entry_id: 3 }, entry({ id: 3, kind: "verification", status: "ok", verifies_entry_id: 2, title: "Cuenta retirada" })] }));
    await openOrder();
    expect(screen.getByText(/1 entrada pendiente de verificación/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancelar orden…" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Añadir entrada" }));
    let dialog = await dialogTitled("Añadir entrada");
    const kinds = within(dialog).getAllByRole("option", { hidden: true }).map((o) => o.textContent);
    expect(kinds).toContain("Instrucción");
    expect(kinds).not.toContain("Acción ejecutada");
    expect(kinds).not.toContain("Consentimiento");
    fireEvent.change(within(dialog).getByLabelText("Tipo"), { target: { value: "instruction" } });
    const statuses = within(within(dialog).getByLabelText("Estado")).getAllByRole("option", { hidden: true }).map((o) => o.textContent);
    expect(statuses).not.toContain("Correcto");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar", hidden: true }));

    fireEvent.click(screen.getByRole("button", { name: "Registrar verificación" }));
    dialog = await dialogTitled("Registrar verificación");
    fireEvent.click(within(dialog).getByRole("button", { name: "Guardar", hidden: true }));
    await waitFor(() => expect(s.orders.addEntry).toHaveBeenCalledWith(7, expect.objectContaining({ kind: "verification", status: "ok", verifies_entry_id: 2 })));
    expect(await screen.findByText("Verificado")).toBeInTheDocument();
  });

  it("links recorded work and generates the PDF report", async () => {
    s.orders.candidates.mockResolvedValue([
      { source: "frp:4", created_at: "2026-11-21T09:00:00", kind: "diagnosis", title: "Diagnóstico FRP: sin indicios de FRP", detail: "", status: "info", verifies_source: null, linked: false },
      { source: "accounts:2", created_at: "2026-11-21T09:30:00", kind: "diagnosis", title: "Ya vinculada", detail: "", status: "info", verifies_source: null, linked: true },
    ]);
    s.orders.link.mockResolvedValue(order());
    s.orders.report.mockResolvedValue({ path: "/tmp/r/Reporte OS-2026-0007.pdf", name: "Reporte OS-2026-0007.pdf", folder: "/tmp/r" });
    s.reports.reveal.mockResolvedValue(undefined);
    await openOrder();
    expect(await screen.findByText("Diagnóstico FRP: sin indicios de FRP")).toBeInTheDocument();
    expect(screen.queryByText("Ya vinculada")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Añadir 1 registro a la orden" }));
    await waitFor(() => expect(s.orders.link).toHaveBeenCalledWith(7, ["frp:4"]));
    fireEvent.click(screen.getByRole("button", { name: "Generar reporte PDF" }));
    expect(await screen.findByText("Reporte OS-2026-0007.pdf")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Mostrar en la carpeta/ }));
    expect(s.reports.reveal).toHaveBeenCalledWith("/tmp/r/Reporte OS-2026-0007.pdf");
  });

  it("only administrators can cancel, and need a reason", async () => {
    s.user = { ...s.user, role: "admin" };
    s.orders.setStatus.mockResolvedValue(order({ status: "cancelled", next_statuses: [] }));
    await openOrder();
    fireEvent.click(screen.getByRole("button", { name: "Cancelar orden…" }));
    const dialog = await dialogTitled("Cancelar la orden");
    const confirm = within(dialog).getByRole("button", { name: "Cancelar orden", hidden: true });
    expect(confirm).toBeDisabled();
    fireEvent.change(within(dialog).getByLabelText("Motivo"), { target: { value: "El cliente desiste" } });
    fireEvent.click(confirm);
    await waitFor(() => expect(s.orders.setStatus).toHaveBeenCalledWith(7, "cancelled", "El cliente desiste"));
  });
});

describe("reports", () => {
  it("shows real counts and exports the selected period", async () => {
    s.reports.summary.mockResolvedValue({
      from: "2026-11-01", to: "2026-11-30", orders: 3, by_status: [{ key: "completed", label: "Completada", count: 2 }], by_type: [], consent: [],
      entries_by_kind: [], pending_verification: 1, avg_days_to_delivery: null, resets: [], frp_assessments: 2, frp_stopped: 1,
    });
    s.orders.list.mockResolvedValue([summary(order())]);
    s.reports.exportCsv.mockResolvedValue({ path: "/tmp/r/x.csv", name: "Ordenes.csv", folder: "/tmp/r" });
    render(<Toaster><ReportsPage /></Toaster>);
    expect(await screen.findByText("1 detenidos sin autorización")).toBeInTheDocument();
    expect(screen.getByText("sin entregas en el periodo")).toBeInTheDocument();
    expect(await screen.findByText("OS-2026-0007")).toBeInTheDocument();
    expect(screen.queryByText("Encabezado de los reportes")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "7 días" }));
    fireEvent.click(screen.getByRole("button", { name: "Exportar CSV" }));
    await waitFor(() => expect(s.reports.exportCsv).toHaveBeenCalled());
    const [from, to] = s.reports.exportCsv.mock.calls[0]! as [string, string];
    expect(from <= to).toBe(true);
    expect(await screen.findByText("Ordenes.csv")).toBeInTheDocument();
  });
});
