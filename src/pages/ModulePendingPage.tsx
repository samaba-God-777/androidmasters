import { Construction } from "lucide-react";
import { PageHeader, Panel } from "@/components/Surface";
import type { RouteDef } from "@/modules/navigation";

/** Honest page for modules scheduled for a later phase: describes the module, offers no fake actions. */
export function ModulePendingPage({ route }: { route: RouteDef }) {
  return (
    <>
      <PageHeader title={route.label} description={route.summary} />
      <Panel>
        <div className="flex items-start gap-3 text-sm">
          <Construction className="mt-0.5 size-4 shrink-0 text-ink-400" aria-hidden />
          <div>
            <p className="font-medium text-ink-100">Este módulo se implementa en la fase {route.phase}.</p>
            <p className="mt-1 text-ink-400">
              Hasta entonces no muestra datos ni acciones: AndroidMasters nunca simula dispositivos, diagnósticos ni resultados.
            </p>
          </div>
        </div>
      </Panel>
    </>
  );
}
