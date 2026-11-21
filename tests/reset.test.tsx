import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ResetPreflight, ResetRecord } from "@/types/api";

const s = vi.hoisted(() => ({
  preflight: vi.fn(), prepare: vi.fn(), challenge: vi.fn(), confirm: vi.fn(), assist: vi.fn(), manualStart: vi.fn(),
  verify: vi.fn(), cancel: vi.fn(), list: vi.fn(),
}));
vi.mock("@/hooks/useDevices", () => ({
  useDevices: () => ({ snapshot: { devices: [], tools: { adb_path: "/x/adb", adb_version: null, fastboot_path: null, last_scan: null, last_error: null }, events: [] }, error: null }),
}));
vi.mock("@/services/api", async (orig) => {
  const real = await orig<typeof import("@/services/api")>();
  return { ...real, api: { ...real.api, reset: s } };
});

import { ResetPage } from "@/pages/ResetPage";
import { Toaster } from "@/components/Toaster";

const ok = (key: string) => ({ key, title: key, status: "pass" as const, detail: "", recommendation: "" });
const ready: ResetPreflight = { ready: true, checklist_id: 1, items: [ok("identity"), ok("authorization"), ok("checklist")] };
const record = (patch: Partial<ResetRecord>): ResetRecord => ({
  id: 3, device_id: null, device_label: "Samsung A14", checklist_id: 1, method: "settings", status: "prepared", confirmed_at: null, confirmed_by: null,
  owner_name: "", actions: [], verification: null, created_at: "2026-11-21T10:00:00",
  instructions: ["El propietario desbloquea el teléfono.", "Abre Ajustes › Administración general › Restablecer."], ...patch,
});

async function openManual() {
  render(<Toaster><ResetPage onNavigate={nav} /></Toaster>);
  fireEvent.change(screen.getByLabelText("Marca y modelo"), { target: { value: "Samsung A14" } });
  fireEvent.click(screen.getByRole("button", { name: "Comprobar" }));
  await waitFor(() => expect(s.preflight).toHaveBeenCalledWith(null, "Samsung A14"));
}
const nav = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  s.list.mockResolvedValue([]);
  s.challenge.mockResolvedValue({ token: "tok", phrase: "BORRAR-3", expires_in_seconds: 300 });
});

describe("factory reset wizard", () => {
  it("cannot prepare while preflight has failures", async () => {
    s.preflight.mockResolvedValue({ ready: false, checklist_id: null, items: [{ key: "authorization", title: "Autorización del propietario", status: "fail", detail: "No registrada", recommendation: "Sin autorización no se puede restablecer." }] });
    await openManual();
    expect(await screen.findByText("Sin autorización no se puede restablecer.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar restablecimiento" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /Abrir Cuentas Google/ }));
    expect(nav).toHaveBeenCalledWith("google");
  });

  it("requires the authorizing person and the exact phrase, then shows owner instructions", async () => {
    s.preflight.mockResolvedValue(ready);
    s.prepare.mockResolvedValue(record({}));
    s.confirm.mockResolvedValue(record({ status: "confirmed", confirmed_at: "2026-11-21T10:01:00", confirmed_by: "Ana", owner_name: "Luis Gómez" }));
    await openManual();
    fireEvent.click(await screen.findByRole("button", { name: "Preparar restablecimiento" }));
    fireEvent.click(await screen.findByRole("button", { name: /Confirmar borrado…/ }));
    const dialog = await screen.findByRole("dialog", { hidden: true });
    const confirm = within(dialog).getByRole("button", { name: "Confirmar borrado", hidden: true });
    await within(dialog).findByLabelText("Escriba BORRAR-3 para confirmar");
    fireEvent.change(within(dialog).getByLabelText("Escriba BORRAR-3 para confirmar"), { target: { value: "BORRAR-3" } });
    expect(confirm).toBeDisabled();
    fireEvent.change(within(dialog).getByLabelText("Nombre de quien autoriza el borrado"), { target: { value: "Luis Gómez" } });
    expect(confirm).toBeEnabled();
    fireEvent.click(confirm);
    await waitFor(() => expect(s.confirm).toHaveBeenCalledWith(3, "tok", "BORRAR-3", "Luis Gómez"));
    expect(await screen.findByText("Pasos en el teléfono")).toBeInTheDocument();
    expect(screen.getByText("Instrucción")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Abrir Ajustes en el teléfono" })).toBeNull();
  });

  it("verification needs both answers and points to FRP when an account is requested", async () => {
    s.preflight.mockResolvedValue(ready);
    s.prepare.mockResolvedValue(record({ status: "pending_verification", confirmed_at: "t", confirmed_by: "Ana" }));
    s.verify.mockResolvedValue(record({
      status: "failed", confirmed_at: "t",
      verification: { at: "t", setup_wizard_shown: true, asks_previous_account: true, disconnect_observed: null, note: "", verified: false,
                      message: "El equipo pide una cuenta anterior (FRP): las cuentas no se retiraron." },
    }));
    await openManual();
    fireEvent.click(await screen.findByRole("button", { name: "Preparar restablecimiento" }));
    const register = await screen.findByRole("button", { name: "Registrar verificación" });
    expect(register).toBeDisabled();
    fireEvent.click(within(screen.getByRole("radiogroup", { name: /asistente de configuración/ })).getByRole("radio", { name: "Sí" }));
    fireEvent.click(within(screen.getByRole("radiogroup", { name: /cuenta Google o del fabricante/ })).getByRole("radio", { name: "Sí" }));
    fireEvent.click(register);
    expect(await screen.findByText("Restablecimiento no verificado")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ir a Diagnóstico FRP" }));
    expect(nav).toHaveBeenCalledWith("frp");
  });
});
