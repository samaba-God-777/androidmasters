import { useState } from "react";
import { Cable, History, Power, Stethoscope } from "lucide-react";
import { Button } from "@/components/Button";
import { CheckList } from "@/components/CheckList";
import { Select } from "@/components/Field";
import { Badge, EmptyState, Notice } from "@/components/Feedback";
import { PageHeader, Panel } from "@/components/Surface";
import { useToast } from "@/components/Toaster";
import { useDevices } from "@/hooks/useDevices";
import { api, type ApiError } from "@/services/api";
import type { CheckItem } from "@/types/api";
import { formatRelative } from "@/utils/format";
import { MODE, deviceName } from "@/utils/labels";
import { cn } from "@/utils/cn";

const EVENT_DOT = { connected: "bg-ok", disconnected: "bg-ink-500", mode_changed: "bg-info", error: "bg-danger" } as const;

export function UsbPage() {
  const toast = useToast();
  const { snapshot } = useDevices();
  const devices = snapshot?.devices ?? [];
  const [target, setTarget] = useState<string>("");
  const [result, setResult] = useState<{ items: CheckItem[]; target: string } | null>(null);
  const [busy, setBusy] = useState<"diagnose" | "restart" | null>(null);
  const tools = snapshot?.tools;
  const selected = target && devices.some((d) => d.id === target) ? target : "";

  async function diagnose() {
    setBusy("diagnose");
    try {
      const items = await api.devices.diagnose(selected || null);
      const name = devices.find((d) => d.id === selected);
      setResult({ items, target: name ? deviceName(name) : "Conexión general" });
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }
  async function restart() {
    setBusy("restart");
    try {
      await api.devices.restartServer();
      toast("Servidor ADB reiniciado", "ok");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  return (
    <>
      <PageHeader title="Conexión USB" description="Estado de ADB, autorización de depuración USB, desconexiones y diagnóstico técnico de la conexión." />
      <div className="grid gap-5 xl:grid-cols-[1fr_380px]">
        <div className="space-y-5">
          <Panel title="Herramientas" actions={<Button size="sm" icon={Power} loading={busy === "restart"} disabled={!tools?.adb_path} onClick={restart}>Reiniciar servidor ADB</Button>}>
            {tools?.last_error && <Notice tone="warn" className="mb-4">{tools.last_error}</Notice>}
            <dl className="grid grid-cols-[120px_1fr] gap-x-6 gap-y-2 text-[13px]">
              <dt className="text-ink-400">adb</dt>
              <dd data-selectable>{tools?.adb_path ? `${tools.adb_path} · ${tools.adb_version ?? "versión desconocida"}` : <span className="text-danger">No encontrado</span>}</dd>
              <dt className="text-ink-400">fastboot</dt>
              <dd data-selectable>{tools?.fastboot_path ?? <span className="text-ink-500">No encontrado</span>}</dd>
              <dt className="text-ink-400">Última revisión</dt>
              <dd>{tools?.last_scan ? formatRelative(tools.last_scan) : "—"}</dd>
            </dl>
          </Panel>
          <Panel title="Diagnóstico de conexión" description="Comprueba herramientas, servidor ADB, controladores, autorización y comunicación con el teléfono.">
            <div className="flex flex-wrap items-end gap-3">
              <div className="w-72">
                <Select label="Qué diagnosticar" value={selected} onChange={(e) => setTarget(e.target.value)}
                        options={[{ value: "", label: "Conexión general (sin dispositivo)" }, ...devices.map((d) => ({ value: d.id, label: `${deviceName(d)} · ${MODE[d.mode].label}` }))]} />
              </div>
              <Button variant="primary" icon={Stethoscope} loading={busy === "diagnose"} onClick={diagnose}>Diagnosticar</Button>
            </div>
            {result && (
              <div className="mt-5">
                <p className="mb-1 text-xs text-ink-400">Resultado · {result.target}</p>
                <CheckList items={result.items} />
              </div>
            )}
          </Panel>
        </div>
        <Panel title="Actividad de conexión" description="Desde que se abrió la aplicación">
          {(snapshot?.events.length ?? 0) === 0 ? (
            <EmptyState icon={History} title="Sin eventos todavía">Conexiones, desconexiones y cambios de modo aparecerán aquí.</EmptyState>
          ) : (
            <ol className="space-y-3">
              {snapshot!.events.slice(0, 40).map((e, i) => (
                <li key={`${e.at}-${i}`} className="flex gap-2.5 text-[13px]">
                  <span className={cn("mt-1.5 size-1.5 shrink-0 rounded-full", EVENT_DOT[e.kind])} aria-hidden />
                  <div className="min-w-0">
                    <p className="text-ink-200">{e.message}</p>
                    <p className="text-xs text-ink-500">
                      {formatRelative(e.at)}
                      {e.mode && <> · <Badge tone={MODE[e.mode].tone} className="ml-1">{MODE[e.mode].label}</Badge></>}
                    </p>
                  </div>
                </li>
              ))}
            </ol>
          )}
          <p className="mt-4 flex items-center gap-1.5 text-xs text-ink-500">
            <Cable className="size-3.5" aria-hidden /> La lista se actualiza sola cada 2 segundos.
          </p>
        </Panel>
      </div>
    </>
  );
}
