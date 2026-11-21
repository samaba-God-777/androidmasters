import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import type { DashboardStats, User } from "@/types/api";

// Test-only doubles for the backend; the app itself never fabricates data.
const backend = vi.hoisted(() => ({
  dashboardStats: vi.fn(),
  appInfo: vi.fn(),
  usersList: vi.fn(),
  currentUser: { current: null as User | null },
}));

vi.mock("@/services/api", async (importOriginal) => {
  const real = await importOriginal<typeof import("@/services/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      dashboardStats: backend.dashboardStats,
      appInfo: backend.appInfo,
      users: { ...real.api.users, list: backend.usersList },
    },
  };
});
vi.mock("@/hooks/useDevices", () => ({
  useDevices: () => ({ snapshot: { devices: [], tools: { adb_path: "/opt/homebrew/bin/adb", adb_version: "37.0.1", fastboot_path: null, last_scan: null, last_error: null }, events: [] }, error: null }),
}));
vi.mock("@/hooks/useSession", () => ({
  useCurrentUser: () => backend.currentUser.current,
  useSession: () => ({ state: { status: "signed_in", user: backend.currentUser.current } }),
}));

import { DashboardPage } from "@/pages/DashboardPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { Toaster } from "@/components/Toaster";
import { ApiError } from "@/services/api";

const days = Array.from({ length: 14 }, (_, i) => ({ day: `2026-11-${String(i + 8).padStart(2, "0")}`, count: i === 13 ? 3 : 0 }));
const stats: DashboardStats = {
  orders_active: 4,
  orders_pending_verification: 1,
  orders_completed_30d: 7,
  diagnostics_30d: 12,
  entries_pending_verification: 2,
  orders_by_status: [{ status: "in_progress", count: 3 }, { status: "pending_verification", count: 1 }],
  operations_14d: days,
  recent_activity: [{ id: 1, created_at: "2026-11-21T10:00:00", event: "login", severity: "info", user: "Ana", details: {} }],
};
const admin: User = { id: 1, username: "ana", display_name: "Ana Pérez", role: "admin", active: true, last_login: null };
const tech: User = { ...admin, id: 2, username: "luis", display_name: "Luis", role: "technician" };

beforeEach(() => {
  vi.clearAllMocks();
  backend.appInfo.mockResolvedValue({ version: "0.1.0", schema_version: 1, database_ok: true, platform: "macos", arch: "aarch64" });
});

describe("dashboard", () => {
  it("shows the backend's numbers and states USB detection honestly", async () => {
    backend.dashboardStats.mockResolvedValue(stats);
    render(<DashboardPage />);
    expect(await screen.findByText("12")).toBeInTheDocument();
    expect(screen.getByText("Órdenes activas").nextSibling).toHaveTextContent("4");
    expect(screen.getByText("En curso")).toBeInTheDocument();
    expect(screen.getByText("Inició sesión")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "3 operaciones en 14 días" })).toBeInTheDocument();
    expect(screen.getByText("Ningún dispositivo conectado")).toBeInTheDocument();
  });

  it("shows the backend error instead of numbers", async () => {
    backend.dashboardStats.mockRejectedValue(new ApiError("database", "Base de datos bloqueada"));
    render(<DashboardPage />);
    expect(await screen.findByText("Base de datos bloqueada")).toBeInTheDocument();
    expect(screen.queryByText("Órdenes activas")).toBeNull();
  });
});

describe("settings by role", () => {
  it("administrators manage users and backups", async () => {
    backend.currentUser.current = admin;
    backend.usersList.mockResolvedValue([admin, tech]);
    render(<Toaster><SettingsPage /></Toaster>);
    expect(await screen.findByText("Usuarios y roles")).toBeInTheDocument();
    expect(await screen.findByText("@luis")).toBeInTheDocument();
    expect(screen.getByText("Copias de seguridad")).toBeInTheDocument();
    // Nobody can deactivate their own account from the list.
    expect(screen.getAllByRole("button", { name: "Desactivar" })).toHaveLength(1);
  });

  it("technicians only see their account and system information", async () => {
    backend.currentUser.current = tech;
    render(<Toaster><SettingsPage /></Toaster>);
    await waitFor(() => expect(screen.getByText(/esquema v1/)).toBeInTheDocument());
    expect(screen.queryByText("Usuarios y roles")).toBeNull();
    expect(screen.queryByText("Copias de seguridad")).toBeNull();
    expect(backend.usersList).not.toHaveBeenCalled();
  });
});
