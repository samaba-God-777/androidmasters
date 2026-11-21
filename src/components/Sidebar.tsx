import { ShieldCheck } from "lucide-react";
import { CURRENT_PHASE, ROUTES, type RouteId } from "@/modules/navigation";
import { cn } from "@/utils/cn";

const GROUPS = ["Laboratorio", "Diagnóstico", "Gestión", "Sistema"] as const;

export function Sidebar({ current, onNavigate }: { current: RouteId; onNavigate: (id: RouteId) => void }) {
  return (
    <nav aria-label="Módulos" className="flex h-full w-60 shrink-0 flex-col border-r border-ink-800 bg-ink-925">
      <div className="flex items-center gap-2.5 px-5 pt-5 pb-6">
        <span className="flex size-8 items-center justify-center rounded-lg bg-brand-500/15">
          <ShieldCheck className="size-[18px] text-brand-400" aria-hidden />
        </span>
        <div className="leading-tight">
          <p className="text-[15px] font-semibold tracking-tight">AndroidMasters</p>
          <p className="text-[11px] text-ink-400">Servicio técnico Android</p>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto px-3 pb-4">
        {GROUPS.map((group) => (
          <div key={group} className="mb-4">
            <p className="px-2 pb-1.5 text-[11px] font-medium text-ink-500">{group}</p>
            <ul className="space-y-0.5">
              {ROUTES.filter((r) => r.group === group).map((r) => {
                const active = r.id === current;
                const pending = r.phase > CURRENT_PHASE;
                const Icon = r.icon;
                return (
                  <li key={r.id}>
                    <button
                      type="button"
                      onClick={() => onNavigate(r.id)}
                      aria-current={active ? "page" : undefined}
                      className={cn(
                        "flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left text-[13px]",
                        active ? "bg-ink-800 font-medium text-ink-100" : "text-ink-300 hover:bg-ink-850 hover:text-ink-100",
                      )}
                    >
                      <Icon className={cn("size-4", active ? "text-brand-400" : "text-ink-400")} aria-hidden />
                      <span className="flex-1">{r.label}</span>
                      {pending && <span className="text-[10px] text-ink-500">fase {r.phase}</span>}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
    </nav>
  );
}
