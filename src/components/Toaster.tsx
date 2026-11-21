import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from "react";
import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { cn } from "@/utils/cn";

type ToastTone = "ok" | "error" | "info";
interface Toast {
  id: number;
  tone: ToastTone;
  message: string;
  leaving?: boolean;
}

const ToastContext = createContext<(message: string, tone?: ToastTone) => void>(() => {});

export function useToast() {
  return useContext(ToastContext);
}

const ICONS = { ok: CircleCheck, error: CircleAlert, info: Info } as const;
const TONES = { ok: "text-ok", error: "text-danger", info: "text-info" } as const;

export function Toaster({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((all) => all.map((t) => (t.id === id ? { ...t, leaving: true } : t)));
    window.setTimeout(() => setToasts((all) => all.filter((t) => t.id !== id)), 160);
  }, []);

  const show = useCallback(
    (message: string, tone: ToastTone = "info") => {
      const id = nextId.current++;
      setToasts((all) => [...all.slice(-3), { id, tone, message }]);
      // Errors stay longer: they usually need reading.
      window.setTimeout(() => dismiss(id), tone === "error" ? 8000 : 4500);
    },
    [dismiss],
  );

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div aria-live="polite" className="pointer-events-none fixed right-5 bottom-5 z-50 flex w-[360px] flex-col gap-2">
        {toasts.map((t) => {
          const Icon = ICONS[t.tone];
          return (
            <div
              key={t.id}
              data-leaving={t.leaving || undefined}
              role={t.tone === "error" ? "alert" : "status"}
              className="am-toast pointer-events-auto flex items-start gap-3 rounded-lg border border-ink-700 bg-ink-850 px-3.5 py-3 text-[13px] text-ink-100 shadow-xl shadow-black/40"
            >
              <Icon className={cn("mt-0.5 size-4 shrink-0", TONES[t.tone])} aria-hidden />
              <p className="flex-1">{t.message}</p>
              <button type="button" onClick={() => dismiss(t.id)} aria-label="Cerrar aviso" className="text-ink-400 hover:text-ink-100">
                <X className="size-3.5" aria-hidden />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}
