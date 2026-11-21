import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, EVENTS, inTauri, toApiError, type ApiError } from "@/services/api";
import { useToast } from "@/components/Toaster";
import type { ConnectionEvent, DevicesSnapshot } from "@/types/api";

interface DevicesState {
  snapshot: DevicesSnapshot | null;
  error: ApiError | null;
}

const DevicesContext = createContext<DevicesState>({ snapshot: null, error: null });

/** Live device list: initial snapshot, then pushed updates from the backend monitor (no polling from the UI). */
export function DevicesProvider({ children }: { children: ReactNode }) {
  const toast = useToast();
  const [state, setState] = useState<DevicesState>({ snapshot: null, error: null });

  useEffect(() => {
    let active = true;
    const unlisten: Array<() => void> = [];
    api.devices
      .snapshot()
      .then((snapshot) => active && setState({ snapshot, error: null }))
      .catch((e) => active && setState({ snapshot: null, error: toApiError(e) }));
    if (inTauri()) {
      void listen<DevicesSnapshot>(EVENTS.snapshot, (e) => setState({ snapshot: e.payload, error: null })).then((u) => unlisten.push(u));
      void listen<ConnectionEvent>(EVENTS.connection, (e) => {
        const ev = e.payload;
        if (ev.kind === "connected") toast(ev.message, "ok");
        else if (ev.kind === "disconnected" || ev.kind === "mode_changed") toast(ev.message, "info");
        else toast(ev.message, "error");
      }).then((u) => unlisten.push(u));
    }
    return () => {
      active = false;
      unlisten.forEach((u) => u());
    };
  }, [toast]);

  return <DevicesContext.Provider value={state}>{children}</DevicesContext.Provider>;
}

export function useDevices(): DevicesState {
  return useContext(DevicesContext);
}
