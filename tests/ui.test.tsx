import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Sidebar } from "@/components/Sidebar";
import { ConfirmDialog } from "@/components/Dialog";
import { ModulePendingPage } from "@/pages/ModulePendingPage";
import { ActivityChart } from "@/pages/DashboardPage";
import { validateSetup } from "@/pages/AccessPage";
import { routeById, ROUTES, CURRENT_PHASE } from "@/modules/navigation";
import { formatRelative } from "@/utils/format";
import { eventLabel } from "@/utils/labels";

describe("first-run validation mirrors backend rules", () => {
  it("accepts a valid administrator", () => {
    expect(validateSetup("ana.perez", "Ana Pérez", "clave-segura-1", "clave-segura-1")).toBeNull();
  });
  it.each([
    ["A", "Ana", "clave-segura-1", "clave-segura-1", /Usuario/],
    ["ana", " ", "clave-segura-1", "clave-segura-1", /nombre/],
    ["ana", "Ana", "corta", "corta", /al menos/],
    ["ana", "Ana", "12345678901", "12345678901", /solo números/],
    ["ana", "Ana", "clave-segura-1", "otra-clave-12", /no coinciden/],
  ])("rejects %s / %s", (u, n, p, c, message) => {
    expect(validateSetup(u, n, p, c)).toMatch(message);
  });
});

describe("navigation", () => {
  it("lists every module required by the specification", () => {
    expect(ROUTES.map((r) => r.id)).toEqual(["dashboard", "devices", "usb", "frp", "google", "brands", "reset", "orders", "reports", "settings"]);
  });
  it("shows every module as available and navigates", () => {
    const onNavigate = vi.fn();
    render(<Sidebar current="dashboard" onNavigate={onNavigate} />);
    expect(screen.getByRole("button", { name: /Dashboard/ })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: /Órdenes de servicio/ })).not.toHaveTextContent("fase");
    expect(screen.getByRole("button", { name: /Reportes/ })).not.toHaveTextContent("fase");
    expect(screen.getByRole("button", { name: /Restablecimiento/ })).not.toHaveTextContent("fase");
    expect(screen.getByRole("button", { name: /Cuentas Google/ })).not.toHaveTextContent("fase");
    expect(screen.getByRole("button", { name: /Diagnóstico FRP/ })).not.toHaveTextContent("fase");
    expect(screen.getByRole("button", { name: /Configuración/ })).not.toHaveTextContent("fase");
    fireEvent.click(screen.getByRole("button", { name: /Órdenes de servicio/ }));
    expect(onNavigate).toHaveBeenCalledWith("orders");
  });
});

describe("no simulated data", () => {
  it("pending modules describe themselves and offer no actions", () => {
    const route = { ...routeById("orders"), phase: CURRENT_PHASE + 1, summary: "Módulo de prueba sin implementar." };
    render(<ModulePendingPage route={route} />);
    expect(screen.getByText(new RegExp(`fase ${CURRENT_PHASE + 1}`))).toBeInTheDocument();
    expect(screen.getByText(/Módulo de prueba/)).toBeInTheDocument();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });
  it("empty activity shows an empty state, not invented bars", () => {
    const days = Array.from({ length: 14 }, (_, i) => ({ day: `2026-11-${String(i + 1).padStart(2, "0")}`, count: 0 }));
    render(<ActivityChart days={days} />);
    expect(screen.getByText(/Sin operaciones/)).toBeInTheDocument();
    expect(screen.queryByRole("img")).toBeNull();
  });
});

describe("confirmation dialog", () => {
  it("requires the exact phrase before confirming", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog open onClose={() => {}} onConfirm={onConfirm} title="Borrar" destructive phrase="ERASE-K7">
        <p>Se borrarán los datos.</p>
      </ConfirmDialog>,
    );
    const confirm = screen.getByRole("button", { name: "Confirmar", hidden: true });
    expect(confirm).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { hidden: true }), { target: { value: "erase-k7" } });
    expect(confirm).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { hidden: true }), { target: { value: "ERASE-K7" } });
    expect(confirm).toBeEnabled();
    fireEvent.click(confirm);
    expect(onConfirm).toHaveBeenCalledOnce();
  });
});

describe("formatting", () => {
  it("formats relative times and audit events in Spanish", () => {
    const now = new Date("2026-11-21T10:00:00");
    expect(formatRelative("2026-11-21T09:59:50", now)).toBe("ahora");
    expect(formatRelative("2026-11-21T09:30:00", now)).toBe("hace 30 minutos");
    expect(eventLabel("login_failed")).toBe("Intento de acceso fallido");
    expect(eventLabel("custom_event")).toBe("custom event");
  });
});

describe("dialog focus", () => {
  it("starts in the first field, not on the close button", async () => {
    const { Dialog } = await import("@/components/Dialog");
    render(
      <Dialog open onClose={() => {}} title="Nueva cuenta">
        <input aria-label="Usuario" />
        <input aria-label="Nombre" />
      </Dialog>,
    );
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Usuario", hidden: true }));
  });
});
