import { Activity, ClipboardList, Smartphone } from "lucide-react";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { PageHeader, Panel } from "@/components/Surface";
import { useAsync } from "@/hooks/useAsync";
import { useDevices } from "@/hooks/useDevices";
import { api } from "@/services/api";
import type { DashboardStats } from "@/types/api";
import { formatDay, formatRelative } from "@/utils/format";
import { MODE, ORDER_STATUS, deviceName, eventLabel } from "@/utils/labels";
import { cn } from "@/utils/cn";

function Kpi({ label, value, hint, tone }: { label: string; value: number; hint: string; tone?: "warn" }) {
  return (
    <div className="px-5 py-4">
      <p className="text-[13px] text-ink-400">{label}</p>
      <p className={cn("tabular mt-1 text-[28px] font-semibold tracking-tight", tone === "warn" && value > 0 ? "text-warn" : "text-ink-100")}>
        {value}
      </p>
      <p className="text-xs text-ink-500">{hint}</p>
    </div>
  );
}

export function ActivityChart({ days }: { days: DashboardStats["operations_14d"] }) {
  const max = Math.max(1, ...days.map((d) => d.count));
  const total = days.reduce((sum, d) => sum + d.count, 0);
  if (total === 0) {
    return (
      <EmptyState icon={Activity} title="Sin operaciones en los últimos 14 días">
        Aparecerán aquí cuando se diagnostiquen o atiendan dispositivos.
      </EmptyState>
    );
  }
  return (
    <figure>
      <div className="flex h-36 items-end gap-1.5" role="img" aria-label={`${total} operaciones en 14 días`}>
        {days.map((d) => (
          <div key={d.day} className="flex h-full flex-1 flex-col items-center justify-end" title={`${formatDay(d.day)}: ${d.count}`}>
            <div className="w-full rounded-sm bg-brand-500/70" style={{ height: `${(d.count / max) * 100}%`, minHeight: d.count ? 3 : 0 }} />
          </div>
        ))}
      </div>
      <figcaption className="mt-2 flex justify-between text-[11px] text-ink-500">
        <span>{formatDay(days[0]!.day)}</span>
        <span>{formatDay(days[days.length - 1]!.day)}</span>
      </figcaption>
    </figure>
  );
}

function ConnectedDevices() {
  const { snapshot, error } = useDevices();
  if (error) return <Notice tone="danger">{error.message}</Notice>;
  if (!snapshot) return <Spinner />;
  if (snapshot.tools.last_error) return <Notice tone="warn" title="Detección detenida">{snapshot.tools.last_error}</Notice>;
  if (snapshot.devices.length === 0) {
    return (
      <EmptyState icon={Smartphone} title="Ningún dispositivo conectado">
        Conecte un teléfono por USB con la depuración USB activada.
      </EmptyState>
    );
  }
  return (
    <ul className="divide-y divide-ink-800">
      {snapshot.devices.map((d) => (
        <li key={d.id} className="flex items-center justify-between gap-3 py-2 text-[13px]">
          <span className="min-w-0">
            <span className="block truncate text-ink-100">{deviceName(d)}</span>
            <span className="text-xs text-ink-500">{d.android_version ? `Android ${d.android_version}` : d.serial_masked}</span>
          </span>
          <Badge tone={MODE[d.mode].tone}>{MODE[d.mode].label}</Badge>
        </li>
      ))}
    </ul>
  );
}

export function DashboardPage() {
  const { data, error, loading } = useAsync(api.dashboardStats);

  return (
    <>
      <PageHeader title="Dashboard" description="Estado del laboratorio calculado a partir de los registros guardados en este equipo." />
      {loading && !data && <Spinner />}
      {error && <Notice tone="danger" title="No se pudieron cargar las estadísticas">{error.message}</Notice>}
      {data && (
        <div className="space-y-5">
          <div className="grid grid-cols-2 divide-ink-800 rounded-xl border border-ink-800 bg-ink-900 lg:grid-cols-4 lg:divide-x [&>*:nth-child(-n+2)]:border-b [&>*:nth-child(-n+2)]:border-ink-800 lg:[&>*:nth-child(-n+2)]:border-b-0">
            <Kpi label="Órdenes activas" value={data.orders_active} hint="abiertas, en curso o por verificar" />
            <Kpi label="Por verificar" value={data.entries_pending_verification} hint="acciones sin evidencia de resultado" tone="warn" />
            <Kpi label="Diagnósticos" value={data.diagnostics_30d} hint="últimos 30 días" />
            <Kpi label="Órdenes completadas" value={data.orders_completed_30d} hint="últimos 30 días" />
          </div>

          <div className="grid gap-5 xl:grid-cols-[1fr_380px]">
            <Panel title="Operaciones" description="Últimos 14 días">
              <ActivityChart days={data.operations_14d} />
            </Panel>
            <Panel title="Dispositivos conectados" description="En tiempo real">
              <ConnectedDevices />
            </Panel>
          </div>

          <div className="grid gap-5 xl:grid-cols-[380px_1fr]">
            <Panel title="Órdenes por estado">
              {data.orders_by_status.length === 0 ? (
                <EmptyState icon={ClipboardList} title="Todavía no hay órdenes de servicio" />
              ) : (
                <ul className="space-y-2">
                  {data.orders_by_status.map((s) => (
                    <li key={s.status} className="flex items-center justify-between">
                      <Badge tone={ORDER_STATUS[s.status].tone}>{ORDER_STATUS[s.status].label}</Badge>
                      <span className="tabular text-sm text-ink-200">{s.count}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Panel>
            <Panel title="Actividad reciente" description="Registro de auditoría">
              {data.recent_activity.length === 0 ? (
                <EmptyState icon={Activity} title="Sin actividad registrada" />
              ) : (
                <ol className="divide-y divide-ink-800">
                  {data.recent_activity.map((a) => (
                    <li key={a.id} className="flex items-center justify-between gap-4 py-2 text-[13px]">
                      <span className="flex items-center gap-2">
                        {a.severity !== "info" && <span className={cn("size-1.5 rounded-full", a.severity === "critical" ? "bg-danger" : "bg-warn")} aria-hidden />}
                        <span className="text-ink-200">{eventLabel(a.event)}</span>
                        {a.user && <span className="text-ink-500">· {a.user}</span>}
                      </span>
                      <time dateTime={a.created_at} className="shrink-0 text-xs text-ink-500">
                        {formatRelative(a.created_at)}
                      </time>
                    </li>
                  ))}
                </ol>
              )}
            </Panel>
          </div>
        </div>
      )}
    </>
  );
}
