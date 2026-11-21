import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { api, ApiError, SESSION_EXPIRED_EVENT } from "@/services/api";
import type { User } from "@/types/api";

type SessionState =
  | { status: "loading" }
  | { status: "unavailable"; error: ApiError }
  | { status: "setup" }
  | { status: "signed_out"; notice?: string }
  | { status: "signed_in"; user: User };

interface SessionApi {
  state: SessionState;
  signIn: (username: string, password: string) => Promise<void>;
  createFirstAdmin: (username: string, displayName: string, password: string) => Promise<void>;
  signOut: () => Promise<void>;
  refresh: () => Promise<void>;
}

const SessionContext = createContext<SessionApi | null>(null);

export function SessionProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<SessionState>({ status: "loading" });

  const refresh = useCallback(async () => {
    try {
      const status = await api.authStatus();
      if (!status.has_users) setState({ status: "setup" });
      else if (status.user) setState({ status: "signed_in", user: status.user });
      else setState({ status: "signed_out" });
    } catch (e) {
      setState({ status: "unavailable", error: e as ApiError });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const onExpired = (e: Event) => setState({ status: "signed_out", notice: (e as CustomEvent<string>).detail });
    window.addEventListener(SESSION_EXPIRED_EVENT, onExpired);
    return () => window.removeEventListener(SESSION_EXPIRED_EVENT, onExpired);
  }, []);

  const value: SessionApi = {
    state,
    refresh,
    signIn: async (username, password) => {
      const user = await api.login(username, password);
      setState({ status: "signed_in", user });
    },
    createFirstAdmin: async (username, displayName, password) => {
      const user = await api.createFirstAdmin(username, displayName, password);
      setState({ status: "signed_in", user });
    },
    signOut: async () => {
      await api.logout();
      setState({ status: "signed_out" });
    },
  };
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionApi {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error("useSession must be used inside <SessionProvider>");
  return ctx;
}

export function useCurrentUser(): User {
  const { state } = useSession();
  if (state.status !== "signed_in") throw new Error("No signed-in user");
  return state.user;
}
