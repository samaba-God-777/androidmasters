import { useCallback, useEffect, useState } from "react";
import { ClipboardList, Plus, Search } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { Select, TextArea, TextInput } from "@/components/Field";
import { OrderDetail } from "@/components/OrderDetail";
import { PageHeader, Panel } from "@/components/Surface";
import { useToast } from "@/components/Toaster";
import { MANUAL_DEVICE, useDeviceChoice } from "@/hooks/useDeviceChoice";
import { api, type ApiError } from "@/services/api";
import type { Order, OrderFilter, OrderSummary, RequestType } from "@/types/api";
import { cn } from "@/utils/cn";
import { formatDateTime } from "@/utils/format";
import { MODE, ORDER_STATUS, REQUEST_TYPE, deviceName } from "@/utils/labels";

const STATUS_FILTERS: Array<{ value: string; label: string }> = [
  { value: "active", label: "Activas" },
  { value: "", label: "Todas" },
  ...Object.entries(ORDER_STATUS).map(([value, s]) => ({ value, label: s.label })),
];

export function OrdersPage() {
  const toast = useToast();
  const [status, setStatus] = useState("active");
  const [query, setQuery] = useState("");
  const [list, setList] = useState<OrderSummary[] | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [order, setOrder] = useState<Order | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    try {
      const filter: OrderFilter = { status: (status || null) as OrderFilter["status"], query };
      setList(await api.orders.list(filter));
      setError(null);
    } catch (e) {
      setError(e as ApiError);
    }
  }, [status, query]);

  useEffect(() => {
    const t = window.setTimeout(() => void load(), query ? 200 : 0);
    return () => window.clearTimeout(t);
  }, [load, query]);

  useEffect(() => {
    if (selectedId === null) return setOrder(null);
    let alive = true;
    api.orders.get(selectedId).then((o) => alive && setOrder(o)).catch((e: ApiError) => alive && toast(e.message, "error"));
    return () => {
      alive = false;
    };
  }, [selectedId, toast]);

  const changed = (o: Order) => {
    setOrder(o);
    void load();
  };

  return (
    <>
      <PageHeader title="Órdenes de servicio" description="Cliente, equipo, consentimiento, diagnóstico, instrucciones, acciones ejecutadas y verificaciones de cada trabajo."
                  actions={<Button variant="primary" icon={Plus} onClick={() => setCreating(true)}>Nueva orden</Button>} />
      <div className="grid items-start gap-5 xl:grid-cols-[360px_1fr]">
        <Panel className="xl:sticky xl:top-0">
          <div className="space-y-3">
            <div className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-ink-500" aria-hidden />
              <input aria-label="Buscar órdenes" placeholder="Número, cliente o equipo" value={query} onChange={(e) => setQuery(e.target.value)}
                     className="h-9 w-full rounded-lg border border-ink-700 bg-ink-925 pr-3 pl-9 text-sm text-ink-100 placeholder:text-ink-500 hover:border-ink-600 focus:border-brand-500 focus:outline-none" />
            </div>
            <Select label="Estado" value={status} onChange={(e) => setStatus(e.target.value)} options={STATUS_FILTERS} />
          </div>
          <div className="mt-4 -mx-2">
            {error && <Notice tone="danger">{error.message}</Notice>}
            {!list && !error && <div className="px-2"><Spinner /></div>}
            {list && list.length === 0 && (
              <EmptyState icon={ClipboardList} title={query || status !== "active" ? "Sin resultados" : "No hay órdenes activas"}>
                {!query && status === "active" && "Cree una orden al recibir un equipo."}
              </EmptyState>
            )}
            {list && list.length > 0 && (
              <ul className="max-h-[calc(100vh-330px)] space-y-0.5 overflow-y-auto">
                {list.map((o) => (
                  <li key={o.id}>
                    <button type="button" onClick={() => setSelectedId(o.id)} aria-current={o.id === selectedId || undefined}
                            className={cn("w-full rounded-lg px-3 py-2.5 text-left transition-colors duration-150",
                              o.id === selectedId ? "bg-brand-500/10 ring-1 ring-brand-500/40" : "hover:bg-ink-850")}>
                      <span className="flex items-center justify-between gap-2">
                        <span className="text-[13px] font-medium tabular-nums text-ink-100">{o.number}</span>
                        <Badge tone={ORDER_STATUS[o.status].tone}>{ORDER_STATUS[o.status].label}</Badge>
                      </span>
                      <span className="mt-0.5 block truncate text-[13px] text-ink-300">{o.customer} · {o.device}</span>
                      <span className="mt-0.5 flex items-center justify-between gap-2 text-xs text-ink-400">
                        <span className="truncate">{REQUEST_TYPE[o.request_type]} · {formatDateTime(o.received_at)}</span>
                        {o.pending_verification > 0 && <span className="shrink-0 text-warn">{o.pending_verification} pend.</span>}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </Panel>
        <div className="min-w-0">
          {order ? (
            <OrderDetail order={order} onChange={changed} />
          ) : (
            <Panel>
              <EmptyState icon={ClipboardList} title="Seleccione una orden">
                El detalle muestra el consentimiento, el registro del servicio y el reporte PDF.
              </EmptyState>
            </Panel>
          )}
        </div>
      </div>
      <NewOrderDialog open={creating} onClose={() => setCreating(false)}
                      onCreated={(o) => { setCreating(false); setStatus("active"); setQuery(""); setSelectedId(o.id); setOrder(o); void load(); toast(`Orden ${o.number} creada`, "ok"); }} />
    </>
  );
}

function NewOrderDialog({ open, onClose, onCreated }: { open: boolean; onClose: () => void; onCreated: (o: Order) => void }) {
  const toast = useToast();
  const { devices, selected, setChoice, device, manualLabel, setManualLabel } = useDeviceChoice();
  const [customer, setCustomer] = useState("");
  const [contact, setContact] = useState("");
  const [type, setType] = useState<RequestType>("diagnose");
  const [problem, setProblem] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setCustomer("");
    setContact("");
    setType("diagnose");
    setProblem("");
  }, [open]);

  const label = device ? "" : manualLabel.trim();
  const ready = customer.trim() !== "" && (device !== null || label !== "");

  async function create() {
    setBusy(true);
    try {
      onCreated(await api.orders.create(device?.id ?? null, {
        customer_name: customer, customer_contact: contact, device_label: label, request_type: type, reported_problem: problem,
      }));
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose} title="Nueva orden de servicio" width="max-w-xl"
            description="El consentimiento del propietario se registra en la orden antes de empezar el trabajo."
            footer={<><Button variant="ghost" onClick={onClose}>Cancelar</Button><Button variant="primary" loading={busy} disabled={!ready} onClick={create}>Crear orden</Button></>}>
      <div className="space-y-3">
        <div className="grid gap-3 sm:grid-cols-2">
          <TextInput label="Cliente" value={customer} onChange={(e) => setCustomer(e.target.value)} maxLength={120} />
          <TextInput label="Contacto (opcional)" hint="Teléfono o correo del cliente para avisarle." value={contact} onChange={(e) => setContact(e.target.value)} maxLength={120} />
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <Select label="Equipo" value={selected} onChange={(e) => setChoice(e.target.value)}
                  options={[...devices.map((d) => ({ value: d.id, label: `${deviceName(d)} · ${MODE[d.mode].label}` })), { value: MANUAL_DEVICE, label: "Equipo no conectado (escribir marca y modelo)" }]} />
          {device ? (
            <Select label="Tipo de servicio" value={type} onChange={(e) => setType(e.target.value as RequestType)}
                    options={Object.entries(REQUEST_TYPE).map(([value, l]) => ({ value, label: l }))} />
          ) : (
            <TextInput label="Marca y modelo" placeholder="Ej. Samsung Galaxy A14" value={manualLabel} onChange={(e) => setManualLabel(e.target.value)} maxLength={120} />
          )}
        </div>
        {!device && (
          <Select label="Tipo de servicio" value={type} onChange={(e) => setType(e.target.value as RequestType)}
                  options={Object.entries(REQUEST_TYPE).map(([value, l]) => ({ value, label: l }))} />
        )}
        {device && device.mode !== "adb" && (
          <Notice tone="warn">El equipo no está en modo ADB autorizado: si todavía no se identificó, escriba su marca y modelo.</Notice>
        )}
        <TextArea label="Problema o solicitud del cliente" value={problem} onChange={(e) => setProblem(e.target.value)} rows={3}
                  hint="No anote contraseñas, PIN ni códigos de verificación." />
      </div>
    </Dialog>
  );
}
