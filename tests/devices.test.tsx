import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { DeviceView, DevicesSnapshot, User } from "@/types/api";

const state = vi.hoisted(() => ({
  snapshot: null as DevicesSnapshot | null,
  user: null as User | null,
  diagnose: vi.fn(),
}));

vi.mock("@/hooks/useDevices", () => ({ useDevices: () => ({ snapshot: state.snapshot, error: null }) }));
vi.mock("@/hooks/useSession", () => ({ useCurrentUser: () => state.user }));
vi.mock("@/services/api", async (importOriginal) => {
  const real = await importOriginal<typeof import("@/services/api")>();
  return { ...real, api: { ...real.api, devices: { ...real.api.devices, diagnose: state.diagnose } } };
});

import { DevicesPage } from "@/pages/DevicesPage";
import { UsbPage } from "@/pages/UsbPage";
import { Topbar } from "@/components/Topbar";
import { Toaster } from "@/components/Toaster";

const tools = { adb_path: "/opt/homebrew/bin/adb", adb_version: "37.0.1", fastboot_path: "/opt/homebrew/bin/fastboot", last_scan: "2026-11-21T10:00:00", last_error: null };
const pixel: DeviceView = {
  id: "a1b2c3d4e5f60718", serial_masked: "281*********K7", mode: "adb", network: false, usb_path: "1-1", manufacturer: "Google", brand: "google",
  model: "Pixel 7", product: "panther", codename: "panther", android_version: "14", api_level: 34, security_patch: "2024-02-05", build_id: "UQ1A",
  bootloader_locked: true, battery: { level: 76, status: "Cargando", health: "Buena", temperature_c: 31.2, source: "USB" },
  storage: { total_bytes: 116000000000, available_bytes: 14800000000 }, identified: true, missing: [], first_seen: "2026-11-21T10:00:00",
  last_seen: "2026-11-21T10:00:00", identify_error: null,
};
const unauthorized: DeviceView = {
  ...pixel, id: "ffff000011112222", serial_masked: "R58******DE", mode: "unauthorized", manufacturer: null, brand: null, model: null, product: null,
  codename: null, android_version: null, api_level: null, security_patch: null, build_id: null, bootloader_locked: null, battery: null, storage: null,
  identified: false, missing: ["Fabricante", "Modelo"],
};
const admin: User = { id: 1, username: "ana", display_name: "Ana", role: "admin", active: true, last_login: null };

beforeEach(() => {
  state.user = admin;
  state.diagnose.mockReset();
});

describe("devices page", () => {
  it("explains how to connect when nothing is attached", () => {
    state.snapshot = { devices: [], tools, events: [] };
    render(<Toaster><DevicesPage onNavigate={() => {}} /></Toaster>);
    expect(screen.getByText("No hay dispositivos conectados")).toBeInTheDocument();
  });

  it("shows detected data, masks the serial and marks what was not detected", () => {
    state.snapshot = { devices: [pixel, unauthorized], tools, events: [] };
    render(<Toaster><DevicesPage onNavigate={() => {}} /></Toaster>);
    expect(screen.getAllByText("Google Pixel 7").length).toBeGreaterThan(0);
    expect(screen.getByText("2024-02-05")).toBeInTheDocument();
    expect(screen.getAllByText("281*********K7").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /Mostrar número de serie completo/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Dispositivo R58/ }));
    expect(screen.getByText("Depuración USB sin autorizar")).toBeInTheDocument();
    expect(screen.getAllByText("No detectado").length).toBeGreaterThan(5);
    expect(screen.getByRole("button", { name: /Actualizar lectura/ })).toBeDisabled();
  });

  it("technicians cannot reveal the serial", () => {
    state.user = { ...admin, role: "technician" };
    state.snapshot = { devices: [pixel], tools, events: [] };
    render(<Toaster><DevicesPage onNavigate={() => {}} /></Toaster>);
    expect(screen.queryByRole("button", { name: /Mostrar número de serie/ })).toBeNull();
  });

  it("tells the user when detection is not working", () => {
    state.snapshot = { devices: [], tools: { ...tools, adb_path: null, last_error: "No se encontró adb (Android SDK Platform-Tools)." }, events: [] };
    const onNavigate = vi.fn();
    render(<Toaster><DevicesPage onNavigate={onNavigate} /></Toaster>);
    fireEvent.click(screen.getByRole("button", { name: "Configurar Platform-Tools" }));
    expect(onNavigate).toHaveBeenCalledWith("settings");
  });
});

describe("usb page", () => {
  it("runs the backend diagnosis and shows its results", async () => {
    state.snapshot = { devices: [pixel], tools, events: [{ at: "2026-11-21T10:00:00", kind: "connected", device: "Google Pixel 7", mode: "adb", message: "Google Pixel 7 conectado" }] };
    state.diagnose.mockResolvedValue([
      { key: "adb", title: "Herramienta adb", status: "pass", detail: "/opt/homebrew/bin/adb", recommendation: "" },
      { key: "mode", title: "Estado de la conexión", status: "warn", detail: "ADB sin autorizar", recommendation: "Acepte el aviso" },
    ]);
    render(<Toaster><UsbPage /></Toaster>);
    expect(screen.getByText("Google Pixel 7 conectado")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Diagnosticar" }));
    expect(await screen.findByText("Acepte el aviso")).toBeInTheDocument();
    expect(state.diagnose).toHaveBeenCalledWith(null);
  });
});

describe("live indicator", () => {
  it.each([
    [{ devices: [], tools, events: [] }, "Sin dispositivos"],
    [{ devices: [pixel], tools, events: [] }, "1 dispositivo"],
    [{ devices: [], tools: { ...tools, last_error: "x" }, events: [] }, "Detección detenida"],
  ])("shows the connection state", (snapshot, text) => {
    state.snapshot = snapshot as DevicesSnapshot;
    render(<Topbar user={admin} onSignOut={() => {}} onOpenDevices={() => {}} />);
    expect(screen.getByRole("button", { name: new RegExp(text) })).toBeInTheDocument();
  });
});
