import { useEffect, useState } from "react";
import { CircleCheck, FileDown, FolderOpen, Link2, Plus, ShieldCheck, ShieldX } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { Badge, Notice } from "@/components/Feedback";
import { Select, TextArea, TextInput } from "@/components/Field";
import { Panel } from "@/components/Surface";
import { useToast } from "@/components/Toaster";
import { useCurrentUser } from "@/hooks/useSession";
import { api, type ApiError } from "@/services/api";
import type { EntryKind, EntryStatus, LinkCandidate, Order, OrderEntry, OrderStatus, OwnershipMethod, SavedFile } from "@/types/api";
import { cn } from "@/utils/cn";
import { formatDateTime } from "@/utils/format";
import { CONSENT, ENTRY_KIND, ENTRY_STATUS, ENTRY_STATUSES, ORDER_STATUS, OWNERSHIP, REQUEST_TYPE } from "@/utils/labels";

/** Button text for each requested transition (depends on where the order comes from). */
function transitionLabel(from: OrderStatus, to: OrderStatus): string {
  switch (to) {
    case "in_progress":
      return from === "open" ? "Empezar trabajo" : "Volver a «En curso»";
    case "pending_verification":
      return "Marcar pendiente de verificación";
    case "completed":
      return "Completar";
    case "delivered":
      return "Entregar al cliente";
    case "cancelled":
      return "Cancelar orden…";
    default:
      return ORDER_STATUS[to].label;
  }
}

function deviceName(order: Order): string {
  const d = order.device;
  const name = d ? `${d.manufacturer} ${d.model}`.trim() : "";
  return name || order.device_label || "—";
}

export function OrderDetail({ order, onChange }: { order: Order; onChange: (order: Order) => void }) {
  const toast = useToast();
  const me = useCurrentUser();
  const [busy, setBusy] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [entryFor, setEntryFor] = useState<{ verifies: OrderEntry | null } | null>(null);
  const [saved, setSaved] = useState<SavedFile | null>(null);
  const closed = order.status === "delivered" || order.status === "cancelled";
  const pending = order.entries.filter((e) => e.status === "pending_verification" && e.verified_by_entry_id === null);

  useEffect(() => setSaved(null), [order.id]);

  async function run<T>(key: string, fn: () => Promise<T>, done?: (value: T) => void) {
    setBusy(key);
    try {
      done?.(await fn());
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  const move = (to: OrderStatus, note = "") =>
    run(`status-${to}`, () => api.orders.setStatus(order.id, to, note), (o) => {
      onChange(o);
      toast(`Orden ${ORDER_STATUS[o.status].label.toLowerCase()}`, "ok");
    });

  return (
    <div className="space-y-5">
      <Panel>
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="text-lg font-semibold tabular-nums text-ink-100" data-selectable>{order.number}</h2>
              <Badge tone={ORDER_STATUS[order.status].tone}>{ORDER_STATUS[order.status].label}</Badge>
              <Badge tone={CONSENT[order.consent_status].tone}>{CONSENT[order.consent_status].label}</Badge>
            </div>
            <p className="mt-1 text-[13px] text-ink-300">
              {order.customer} · {deviceName(order)} · {REQUEST_TYPE[order.request_type]}
            </p>
            <p className="mt-0.5 text-xs text-ink-400">
              Recibida {formatDateTime(order.received_at)} · {order.technician ?? "—"}
              {order.delivered_at && <> · Entregada {formatDateTime(order.delivered_at)}</>}
            </p>
          </div>
          <Button icon={FileDown} loading={busy === "report"} onClick={() => run("report", () => api.orders.report(order.id), async (file) => {
            setSaved(file);
            onChange(await api.orders.get(order.id));
          })}>
            Generar reporte PDF
          </Button>
        </div>
        {saved && (
          <Notice tone="ok" className="mt-4" title="Reporte guardado">
            <span data-selectable>{saved.name}</span> en {saved.folder}.{" "}
            <button type="button" className="font-medium text-brand-300 underline-offset-2 hover:underline"
                    onClick={() => api.reports.reveal(saved.path).catch((e: ApiError) => toast(e.message, "error"))}>
              <FolderOpen className="mr-1 inline size-3.5" aria-hidden />Mostrar en la carpeta
            </button>
          </Notice>
        )}
        {order.next_statuses.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2 border-t border-ink-800 pt-4">
            {order.next_statuses.filter((s) => s !== "cancelled" || me?.role === "admin").map((to) => (
              <Button key={to} size="sm" variant={(to === "in_progress" && order.status === "open") || (to === "completed" && order.status !== "open") ? "primary" : to === "cancelled" ? "ghost" : "secondary"}
                      loading={busy === `status-${to}`} disabled={busy !== null}
                      onClick={() => (to === "cancelled" ? setCancelling(true) : void move(to))}>
                {transitionLabel(order.status, to)}
              </Button>
            ))}
          </div>
        )}
        {pending.length > 0 && !closed && (
          <Notice tone="warn" className="mt-4" title={`${pending.length} ${pending.length === 1 ? "entrada pendiente" : "entradas pendientes"} de verificación`}>
            La orden no se puede completar hasta registrar su verificación (correcta o fallida).
          </Notice>
        )}
      </Panel>

      <div className="grid gap-5 2xl:grid-cols-2">
        <Panel title="Cliente y equipo">
          <dl className="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 text-[13px]">
            <dt className="text-ink-400">Cliente</dt><dd className="text-ink-100" data-selectable>{order.customer}</dd>
            <dt className="text-ink-400">Contacto</dt><dd className="text-ink-100" data-selectable>{order.customer_contact || "—"}</dd>
            <dt className="text-ink-400">Equipo</dt><dd className="text-ink-100">{deviceName(order)}</dd>
            {order.device ? (
              <>
                <dt className="text-ink-400">Serie</dt><dd className="font-mono text-ink-100">{order.device.serial_masked}</dd>
                <dt className="text-ink-400">Android</dt>
                <dd className="text-ink-100">{order.device.android_version || "—"}{order.device.api_level ? ` (API ${order.device.api_level})` : ""} · parche {order.device.security_patch || "—"}</dd>
              </>
            ) : (
              <><dt className="text-ink-400">Identificación</dt><dd className="text-ink-300">Indicada por el técnico (equipo no conectado)</dd></>
            )}
            <dt className="text-ink-400">Problema</dt><dd className="whitespace-pre-wrap text-ink-100">{order.reported_problem || "—"}</dd>
          </dl>
        </Panel>
        <ConsentPanel order={order} onChange={onChange} closed={closed} />
      </div>

      <Panel title="Registro del servicio" description="Cada entrada indica si es diagnóstico, instrucción, acción ejecutada o verificación."
             actions={!closed && <Button size="sm" icon={Plus} onClick={() => setEntryFor({ verifies: null })}>Añadir entrada</Button>}>
        <ol className="space-y-2.5">
          {order.entries.map((e) => (
            <EntryRow key={e.id} entry={e} all={order.entries} canVerify={!closed} onVerify={() => setEntryFor({ verifies: e })} />
          ))}
        </ol>
      </Panel>

      {!closed && <LinkPanel order={order} onChange={onChange} />}
      <NotesPanel order={order} onChange={onChange} closed={closed} />

      <EntryDialog order={order} state={entryFor} onClose={() => setEntryFor(null)} onSaved={(o) => { setEntryFor(null); onChange(o); toast("Entrada registrada", "ok"); }} />
      <CancelDialog open={cancelling} busy={busy === "status-cancelled"} onClose={() => setCancelling(false)}
                    onConfirm={(reason) => { setCancelling(false); void move("cancelled", reason); }} />
    </div>
  );
}

function EntryRow({ entry, all, canVerify, onVerify }: { entry: OrderEntry; all: OrderEntry[]; canVerify: boolean; onVerify: () => void }) {
  const kind = ENTRY_KIND[entry.kind];
  const waiting = entry.status === "pending_verification" && entry.verified_by_entry_id === null;
  const verifiedBy = entry.verified_by_entry_id ? all.find((x) => x.id === entry.verified_by_entry_id) : null;
  const verifies = entry.verifies_entry_id ? all.find((x) => x.id === entry.verifies_entry_id) : null;
  return (
    <li className={cn("rounded-lg border px-3.5 py-3", waiting ? "border-warn/40 bg-warn/5" : "border-ink-800")}>
      <div className="flex flex-wrap items-center gap-2">
        <Badge tone={kind.tone}>{kind.label}</Badge>
        {entry.status !== "info" && !verifiedBy && <Badge tone={ENTRY_STATUS[entry.status].tone}>{ENTRY_STATUS[entry.status].label}</Badge>}
        {verifiedBy && (
          <Badge tone={verifiedBy.status === "ok" ? "ok" : "danger"}>
            {verifiedBy.status === "ok" ? "Verificado" : "Verificación fallida"}
          </Badge>
        )}
        <span className="ml-auto text-xs text-ink-400 tabular-nums">{formatDateTime(entry.created_at)} · {entry.author ?? "Sistema"}</span>
      </div>
      <p className="mt-1.5 text-[13px] font-medium text-ink-100">{entry.title}</p>
      {entry.detail && <p className="mt-0.5 whitespace-pre-wrap text-[13px] text-ink-300" data-selectable>{entry.detail}</p>}
      {verifies && <p className="mt-1 text-xs text-ink-400">Verifica: {verifies.title}</p>}
      {waiting && canVerify && (
        <Button size="sm" icon={CircleCheck} className="mt-2.5" onClick={onVerify}>Registrar verificación</Button>
      )}
    </li>
  );
}

function ConsentPanel({ order, onChange, closed }: { order: Order; onChange: (o: Order) => void; closed: boolean }) {
  const toast = useToast();
  const [name, setName] = useState(order.customer);
  const [method, setMethod] = useState<OwnershipMethod | "">("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    setName(order.customer);
    setMethod("");
    setNote("");
  }, [order.id, order.customer]);

  async function decide(decision: "granted" | "denied" | "revoked") {
    setBusy(decision);
    try {
      onChange(await api.orders.consent(order.id, decision, name, method || null, note));
      toast(decision === "granted" ? "Consentimiento registrado" : decision === "denied" ? "Servicio detenido: sin autorización" : "Consentimiento revocado", decision === "granted" ? "ok" : "info");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  return (
    <Panel title="Consentimiento del propietario">
      {order.consent_status === "granted" && (
        <div className="space-y-3 text-[13px]">
          <p className="flex items-center gap-2 text-ink-100"><ShieldCheck className="size-4 text-ok" aria-hidden />Autorizado por {order.consent_by}</p>
          <p className="text-ink-300">{order.ownership_method ? OWNERSHIP[order.ownership_method] : ""} · {formatDateTime(order.consent_at)}</p>
          {!closed && (
            <div className="flex flex-wrap items-end gap-2 border-t border-ink-800 pt-3">
              <div className="min-w-0 flex-1"><TextInput label="Motivo (si el cliente retira el consentimiento)" value={note} onChange={(e) => setNote(e.target.value)} /></div>
              <Button size="sm" variant="ghost" loading={busy === "revoked"} disabled={!note.trim()} onClick={() => decide("revoked")}>Registrar revocación</Button>
            </div>
          )}
        </div>
      )}
      {(order.consent_status === "denied" || order.consent_status === "revoked") && (
        <Notice tone="danger" title={CONSENT[order.consent_status].label}>
          No se realizarán operaciones sobre cuentas ni borrados. Oriente al cliente hacia el propietario anterior, el vendedor, el fabricante o un servicio autorizado.
        </Notice>
      )}
      {order.consent_status === "pending" && !closed && (
        <div className="space-y-3">
          <p className="text-[13px] text-ink-300">Compruebe que el cliente es el propietario o está autorizado antes de empezar. No guarde contraseñas ni códigos.</p>
          <TextInput label="Nombre de quien autoriza" value={name} onChange={(e) => setName(e.target.value)} />
          <Select label="Cómo se comprobó" value={method} onChange={(e) => setMethod(e.target.value as OwnershipMethod | "")}
                  options={[{ value: "", label: "Seleccione…" }, ...Object.entries(OWNERSHIP).map(([value, label]) => ({ value, label }))]} />
          <TextInput label="Observaciones (opcional)" value={note} onChange={(e) => setNote(e.target.value)} />
          <div className="flex flex-wrap gap-2">
            <Button variant="primary" icon={ShieldCheck} disabled={!name.trim() || !method || busy !== null} loading={busy === "granted"} onClick={() => decide("granted")}>
              Registrar consentimiento
            </Button>
            <Button variant="ghost" icon={ShieldX} disabled={busy !== null} loading={busy === "denied"} onClick={() => decide("denied")}>
              No puede demostrar la autorización
            </Button>
          </div>
        </div>
      )}
      {order.consent_status === "pending" && closed && <p className="text-[13px] text-ink-400">Sin decisión registrada.</p>}
    </Panel>
  );
}

const MANUAL_KINDS: EntryKind[] = ["diagnosis", "instruction", "verification", "incident", "note"];

function EntryDialog({ order, state, onClose, onSaved }: {
  order: Order;
  state: { verifies: OrderEntry | null } | null;
  onClose: () => void;
  onSaved: (o: Order) => void;
}) {
  const toast = useToast();
  const verifies = state?.verifies ?? null;
  const [kind, setKind] = useState<EntryKind>("diagnosis");
  const [status, setStatus] = useState<EntryStatus>("info");
  const [title, setTitle] = useState("");
  const [detail, setDetail] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!state) return;
    setKind(verifies ? "verification" : "diagnosis");
    setStatus(verifies ? "ok" : "info");
    setTitle(verifies ? `Verificación: ${verifies.title}` : "");
    setDetail("");
  }, [state, verifies]);

  const statuses = ENTRY_STATUSES[kind] ?? ["info"];
  const chooseKind = (k: EntryKind) => {
    setKind(k);
    setStatus(ENTRY_STATUSES[k]?.[0] ?? "info");
  };

  async function save() {
    setBusy(true);
    try {
      onSaved(await api.orders.addEntry(order.id, { kind, title, detail, status, verifies_entry_id: verifies?.id ?? null }));
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={state !== null} onClose={onClose} width="max-w-lg"
            title={verifies ? "Registrar verificación" : "Añadir entrada"}
            description={verifies ? <>Evidencia de: <span className="text-ink-200">{verifies.title}</span></> : "Las acciones ejecutadas las registra la aplicación; aquí se anota lo observado o lo indicado al propietario."}
            footer={<><Button variant="ghost" onClick={onClose}>Cancelar</Button><Button variant="primary" loading={busy} disabled={!title.trim()} onClick={save}>Guardar</Button></>}>
      <div className="space-y-3">
        {!verifies && (
          <div className="grid gap-3 sm:grid-cols-2">
            <Select label="Tipo" value={kind} onChange={(e) => chooseKind(e.target.value as EntryKind)}
                    options={MANUAL_KINDS.map((k) => ({ value: k, label: ENTRY_KIND[k].label }))} />
            <Select label="Estado" value={status} onChange={(e) => setStatus(e.target.value as EntryStatus)}
                    options={statuses.map((s) => ({ value: s, label: ENTRY_STATUS[s].label }))} />
          </div>
        )}
        {verifies && (
          <Select label="Resultado" value={status} onChange={(e) => setStatus(e.target.value as EntryStatus)}
                  options={[{ value: "ok", label: "Correcto: se comprobó que funcionó" }, { value: "failed", label: "Fallido: no se pudo comprobar o no funcionó" }]} />
        )}
        <p className="text-xs text-ink-400">{ENTRY_KIND[kind].hint}</p>
        <TextInput label="Título" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} />
        <TextArea label="Detalle" value={detail} onChange={(e) => setDetail(e.target.value)} rows={4}
                  hint="No escriba contraseñas, códigos de verificación ni correos de cuentas." />
      </div>
    </Dialog>
  );
}

function CancelDialog({ open, busy, onClose, onConfirm }: { open: boolean; busy: boolean; onClose: () => void; onConfirm: (reason: string) => void }) {
  const [reason, setReason] = useState("");
  useEffect(() => {
    if (open) setReason("");
  }, [open]);
  return (
    <Dialog open={open} onClose={onClose} title="Cancelar la orden" description="La orden quedará cerrada y no admitirá cambios. El registro se conserva."
            footer={<><Button variant="ghost" onClick={onClose}>Volver</Button><Button variant="danger" loading={busy} disabled={!reason.trim()} onClick={() => onConfirm(reason)}>Cancelar orden</Button></>}>
      <TextInput label="Motivo" value={reason} onChange={(e) => setReason(e.target.value)} />
    </Dialog>
  );
}

function LinkPanel({ order, onChange }: { order: Order; onChange: (o: Order) => void }) {
  const toast = useToast();
  const [items, setItems] = useState<LinkCandidate[] | null>(null);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    api.orders.candidates(order.id).then((c) => {
      if (!alive) return;
      setItems(c);
      setChosen(new Set(c.filter((x) => !x.linked).map((x) => x.source)));
    }).catch((e: ApiError) => alive && toast(e.message, "error"));
    return () => {
      alive = false;
    };
  }, [order.id, order.updated_at, toast]);

  const fresh = items?.filter((c) => !c.linked) ?? [];
  const toggle = (source: string) =>
    setChosen((s) => {
      const next = new Set(s);
      if (next.has(source)) next.delete(source);
      else next.add(source);
      return next;
    });

  async function link() {
    setBusy(true);
    try {
      onChange(await api.orders.link(order.id, [...chosen]));
      toast("Trabajo añadido al registro", "ok");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel title="Trabajo registrado para este equipo"
           description="Diagnósticos FRP, comprobaciones de cuentas, listas de preparación y restablecimientos del mismo equipo.">
      {items === null && <p className="text-[13px] text-ink-400">Buscando…</p>}
      {items !== null && fresh.length === 0 && (
        <p className="text-[13px] text-ink-400">{items.length === 0 ? "No hay trabajo registrado para este equipo en otros módulos." : "Todo el trabajo registrado ya está en la orden."}</p>
      )}
      {fresh.length > 0 && (
        <>
          <ul className="space-y-1.5">
            {fresh.map((c) => (
              <li key={c.source}>
                <label className="flex cursor-pointer items-start gap-3 rounded-lg px-2 py-2 hover:bg-ink-850/60">
                  <input type="checkbox" className="mt-0.5 size-4 accent-[var(--color-brand-500)]" checked={chosen.has(c.source)} onChange={() => toggle(c.source)} />
                  <span className="min-w-0 flex-1">
                    <span className="flex flex-wrap items-center gap-2">
                      <Badge tone={ENTRY_KIND[c.kind].tone}>{ENTRY_KIND[c.kind].label}</Badge>
                      <span className="text-[13px] text-ink-100">{c.title}</span>
                    </span>
                    {c.detail && <span className="mt-0.5 block text-xs text-ink-400">{c.detail}</span>}
                  </span>
                  <span className="text-xs text-ink-400 tabular-nums">{formatDateTime(c.created_at)}</span>
                </label>
              </li>
            ))}
          </ul>
          <Button icon={Link2} className="mt-3" loading={busy} disabled={chosen.size === 0} onClick={link}>
            Añadir {chosen.size === 1 ? "1 registro" : `${chosen.size} registros`} a la orden
          </Button>
        </>
      )}
    </Panel>
  );
}

function NotesPanel({ order, onChange, closed }: { order: Order; onChange: (o: Order) => void; closed: boolean }) {
  const toast = useToast();
  const [notes, setNotes] = useState(order.notes);
  const [busy, setBusy] = useState(false);
  useEffect(() => setNotes(order.notes), [order.id, order.notes]);
  if (closed && !order.notes) return null;
  return (
    <Panel title="Observaciones">
      {closed ? (
        <p className="whitespace-pre-wrap text-[13px] text-ink-200">{order.notes}</p>
      ) : (
        <>
          <TextArea label="Observaciones de la orden" value={notes} onChange={(e) => setNotes(e.target.value)} rows={3} />
          <Button size="sm" className="mt-2" loading={busy} disabled={notes === order.notes} onClick={async () => {
            setBusy(true);
            try {
              onChange(await api.orders.setNotes(order.id, notes));
              toast("Observaciones guardadas", "ok");
            } catch (e) {
              toast((e as ApiError).message, "error");
            } finally {
              setBusy(false);
            }
          }}>Guardar observaciones</Button>
        </>
      )}
    </Panel>
  );
}
