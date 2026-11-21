import { useEffect, useState } from "react";
import { FileDown, FileSpreadsheet, FileText, FolderOpen, Save } from "lucide-react";
import { Button } from "@/components/Button";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { TextInput } from "@/components/Field";
import { PageHeader, Panel } from "@/components/Surface";
import { Table } from "@/components/Table";
import { useToast } from "@/components/Toaster";
import { useAsync } from "@/hooks/useAsync";
import { useCurrentUser } from "@/hooks/useSession";
import { api, type ApiError } from "@/services/api";
import type { Count, OrderSummary, SavedFile } from "@/types/api";
import { cn } from "@/utils/cn";
import { formatDateTime } from "@/utils/format";
import { ORDER_STATUS, REQUEST_TYPE } from "@/utils/labels";

/** Local calendar date as YYYY-MM-DD. */
export function isoDay(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

function daysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return isoDay(d);
}

const PRESETS = [
  { id: "7", label: "7 días", range: () => [daysAgo(6), isoDay(new Date())] },
  { id: "30", label: "30 días", range: () => [daysAgo(29), isoDay(new Date())] },
  { id: "month", label: "Este mes", range: () => { const d = new Date(); return [isoDay(new Date(d.getFullYear(), d.getMonth(), 1)), isoDay(d)]; } },
  { id: "90", label: "90 días", range: () => [daysAgo(89), isoDay(new Date())] },
] as const;

function CountList({ items, empty = "—" }: { items: Count[]; empty?: string }) {
  const max = Math.max(1, ...items.map((i) => i.count));
  if (items.length === 0) return <p className="text-[13px] text-ink-400">{empty}</p>;
  return (
    <ul className="space-y-2">
      {items.map((i) => (
        <li key={i.key} className="text-[13px]">
          <div className="flex justify-between gap-3"><span className="text-ink-200">{i.label}</span><span className="tabular-nums text-ink-100">{i.count}</span></div>
          <div className="mt-1 h-1.5 rounded-full bg-ink-800">
            <div className="h-full rounded-full bg-brand-500/70" style={{ width: `${(i.count / max) * 100}%` }} />
          </div>
        </li>
      ))}
    </ul>
  );
}

function Stat({ label, value, hint, tone }: { label: string; value: string | number; hint?: string; tone?: "warn" }) {
  return (
    <div className="rounded-xl border border-ink-800 bg-ink-900 px-4 py-3.5">
      <p className="text-xs text-ink-400">{label}</p>
      <p className={cn("mt-1 text-2xl font-semibold tabular-nums", tone === "warn" ? "text-warn" : "text-ink-100")}>{value}</p>
      {hint && <p className="mt-0.5 text-xs text-ink-400">{hint}</p>}
    </div>
  );
}

export function ReportsPage() {
  const toast = useToast();
  const me = useCurrentUser();
  const [preset, setPreset] = useState<string>("30");
  const [[from, to], setRange] = useState<[string, string]>(() => PRESETS[1].range() as [string, string]);
  const summary = useAsync(() => api.reports.summary(from, to), [from, to]);
  const orders = useAsync(() => api.orders.list({ from, to, limit: 5000 }), [from, to]);
  const [busy, setBusy] = useState<string | null>(null);
  const [saved, setSaved] = useState<SavedFile | null>(null);

  async function generate(key: string, fn: () => Promise<SavedFile>) {
    setBusy(key);
    try {
      setSaved(await fn());
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  const s = summary.data;
  const validRange = from !== "" && to !== "" && from <= to;

  return (
    <>
      <PageHeader title="Reportes" description="Resumen del periodo con datos reales, reporte PDF por orden o por periodo y exportación de órdenes a CSV." />
      <Panel className="mb-5">
        <div className="flex flex-wrap items-end gap-3">
          <div role="radiogroup" aria-label="Periodo" className="flex rounded-lg border border-ink-700 bg-ink-925 p-0.5">
            {PRESETS.map((p) => (
              <button key={p.id} type="button" role="radio" aria-checked={preset === p.id}
                      onClick={() => { setPreset(p.id); setRange(p.range() as [string, string]); setSaved(null); }}
                      className={cn("pressable rounded-md px-3 py-1.5 text-[13px]", preset === p.id ? "bg-brand-500/20 text-brand-300" : "text-ink-400 hover:text-ink-100")}>
                {p.label}
              </button>
            ))}
          </div>
          <div className="w-40"><TextInput label="Desde" type="date" value={from} max={to} onChange={(e) => { setPreset(""); setRange([e.target.value, to]); }} /></div>
          <div className="w-40"><TextInput label="Hasta" type="date" value={to} min={from} onChange={(e) => { setPreset(""); setRange([from, e.target.value]); }} /></div>
          <div className="ml-auto flex flex-wrap gap-2">
            <Button icon={FileSpreadsheet} disabled={!validRange || busy !== null} loading={busy === "csv"} onClick={() => generate("csv", () => api.reports.exportCsv(from, to))}>Exportar CSV</Button>
            <Button variant="primary" icon={FileText} disabled={!validRange || busy !== null} loading={busy === "period"} onClick={() => generate("period", () => api.reports.periodPdf(from, to))}>Reporte del periodo (PDF)</Button>
          </div>
        </div>
        {saved && (
          <Notice tone="ok" className="mt-4" title="Archivo guardado">
            <span data-selectable>{saved.name}</span> en {saved.folder}.{" "}
            <button type="button" className="font-medium text-brand-300 underline-offset-2 hover:underline"
                    onClick={() => api.reports.reveal(saved.path).catch((e: ApiError) => toast(e.message, "error"))}>
              <FolderOpen className="mr-1 inline size-3.5" aria-hidden />Mostrar en la carpeta
            </button>
          </Notice>
        )}
      </Panel>

      {summary.error && <Notice tone="danger" className="mb-5">{summary.error.message}</Notice>}
      {!s && !summary.error && <Spinner />}
      {s && (
        <>
          <div className="mb-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <Stat label="Órdenes recibidas" value={s.orders} />
            <Stat label="Pendiente de verificación" value={s.pending_verification} hint="entradas sin evidencia" tone={s.pending_verification > 0 ? "warn" : undefined} />
            <Stat label="Tiempo medio hasta la entrega" value={s.avg_days_to_delivery === null ? "—" : `${s.avg_days_to_delivery.toLocaleString("es", { maximumFractionDigits: 1 })} d`}
                  hint={s.avg_days_to_delivery === null ? "sin entregas en el periodo" : undefined} />
            <Stat label="Diagnósticos FRP" value={s.frp_assessments} hint={s.frp_stopped ? `${s.frp_stopped} detenidos sin autorización` : undefined} />
          </div>
          <div className="mb-5 grid gap-5 lg:grid-cols-2 2xl:grid-cols-4">
            <Panel title="Por estado"><CountList items={s.by_status} empty="Sin órdenes" /></Panel>
            <Panel title="Por tipo de servicio"><CountList items={s.by_type} empty="Sin órdenes" /></Panel>
            <Panel title="Registro del servicio"><CountList items={s.entries_by_kind} empty="Sin entradas" /></Panel>
            <Panel title="Restablecimientos"><CountList items={s.resets} empty="Sin restablecimientos" /></Panel>
          </div>
        </>
      )}

      <Panel title="Órdenes del periodo">
        {orders.error && <Notice tone="danger">{orders.error.message}</Notice>}
        {orders.data && (
          <Table<OrderSummary>
            rows={orders.data}
            rowKey={(o) => o.id}
            empty={<EmptyState icon={FileText} title="No hay órdenes en este periodo" />}
            columns={[
              { key: "number", header: "Número", render: (o) => <span className="tabular-nums">{o.number}</span> },
              { key: "date", header: "Recibida", render: (o) => formatDateTime(o.received_at) },
              { key: "customer", header: "Cliente", render: (o) => o.customer },
              { key: "device", header: "Equipo", render: (o) => o.device },
              { key: "type", header: "Servicio", render: (o) => REQUEST_TYPE[o.request_type] },
              { key: "status", header: "Estado", render: (o) => (
                <span className="flex items-center gap-1.5">
                  <Badge tone={ORDER_STATUS[o.status].tone}>{ORDER_STATUS[o.status].label}</Badge>
                  {o.pending_verification > 0 && o.status !== "pending_verification" && <Badge tone="warn">{o.pending_verification} pend.</Badge>}
                </span>
              ) },
              { key: "pdf", header: "", className: "text-right", render: (o) => (
                <Button size="sm" variant="ghost" icon={FileDown} loading={busy === `order-${o.id}`} disabled={busy !== null}
                        onClick={() => generate(`order-${o.id}`, () => api.orders.report(o.id))}>PDF</Button>
              ) },
            ]}
          />
        )}
      </Panel>

      {me.role === "admin" && <WorkshopSettings />}
    </>
  );
}

function WorkshopSettings() {
  const toast = useToast();
  const settings = useAsync(api.reports.settings);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (settings.data) setName(settings.data.workshop_name);
  }, [settings.data]);
  return (
    <Panel title="Encabezado de los reportes" className="mt-5" description={settings.data ? `Los archivos se guardan en ${settings.data.reports_dir}.` : undefined}>
      <div className="flex max-w-xl items-end gap-2">
        <div className="flex-1"><TextInput label="Nombre del taller" placeholder="AndroidMasters" value={name} maxLength={80} onChange={(e) => setName(e.target.value)} /></div>
        <Button icon={Save} loading={busy} disabled={!settings.data || name === settings.data.workshop_name} onClick={async () => {
          setBusy(true);
          try {
            await api.reports.setSettings(name);
            await settings.reload();
            toast("Encabezado guardado", "ok");
          } catch (e) {
            toast((e as ApiError).message, "error");
          } finally {
            setBusy(false);
          }
        }}>Guardar</Button>
      </div>
    </Panel>
  );
}
