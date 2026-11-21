import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { api, SESSION_EXPIRED_EVENT } from "@/services/api";
import { SessionProvider, useSession } from "@/hooks/useSession";
import { AccessPage } from "@/pages/AccessPage";

function Shell() {
  const { state } = useSession();
  if (state.status === "signed_out") return <AccessPage mode="login" notice={state.notice} />;
  return <p>{state.status === "signed_in" ? `Hola ${state.user.display_name}` : state.status}</p>;
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  invoke.mockReset();
});

describe("idle session", () => {
  it("an expired session returns to sign-in with an explanation", async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    invoke.mockImplementation(async (command: string) => {
      if (command === "auth_status") return { has_users: true, user: { id: 1, username: "ana", display_name: "Ana", role: "admin", active: true, last_login: null } };
      throw { code: "session_expired", message: "La sesión se cerró por inactividad. Vuelva a iniciar sesión." };
    });
    render(<SessionProvider><Shell /></SessionProvider>);
    expect(await screen.findByText("Hola Ana")).toBeInTheDocument();
    const heard = vi.fn();
    window.addEventListener(SESSION_EXPIRED_EVENT, heard);
    await act(async () => {
      await expect(api.dashboardStats()).rejects.toMatchObject({ code: "session_expired" });
    });
    window.removeEventListener(SESSION_EXPIRED_EVENT, heard);
    expect(heard).toHaveBeenCalledOnce();
    expect(await screen.findByText(/se cerró por inactividad/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Entrar" })).toBeInTheDocument();
  });

  it("other errors do not sign the user out", async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    invoke.mockRejectedValue({ code: "forbidden", message: "Su rol no permite esta acción." });
    const heard = vi.fn();
    window.addEventListener(SESSION_EXPIRED_EVENT, heard);
    await expect(api.users.list()).rejects.toMatchObject({ code: "forbidden" });
    window.removeEventListener(SESSION_EXPIRED_EVENT, heard);
    expect(heard).not.toHaveBeenCalled();
  });
});
