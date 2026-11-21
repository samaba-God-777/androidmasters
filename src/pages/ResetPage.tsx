import { useCallback, useEffect, useState } from "react";
import { History, Power, RefreshCw, RotateCcw, Settings2, ShieldCheck, TriangleAlert } from "lucide-react";
import { Button } from "@/components/Button";
import { CheckList } from "@/components/CheckList";
import { YesNo } from "@/components/Choice";
import { Dialog } from "@/components/Dialog";
import { Select, TextInput } from "@/components/Field";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { PageHeader, Panel } from "@/components/Surface";
import { Table } from "@/components/Table";
import { useToast } from "@/components/Toaster";
import { useAsync } from "@/hooks/useAsync";
import { MANUAL_DEVICE, useDeviceChoice } from "@/hooks/useDeviceChoice";
import { api, type ApiError } from "@/services/api";
import type { DeviceView, ResetChallenge, ResetMethod, ResetPreflight, ResetRecord } from "@/types/api";
import { formatDateTime } from "@/utils/format";
import { MODE, RESET_STATUS, deviceName } from "@/utils/labels";
import { cn } from "@/utils/cn";

const METHODS: Array<{ value: ResetMethod; title: string; text: string }> = [
  { value: "settings", title: "Desde Ajustes (recomendado)", text: "El propietario borra el equipo desde Ajustes con su bloqueo de pantalla." },
  { value: "recovery", title: "Desde Recovery", text: "Para equipos que no arrancan o cuyo propietario no recuerda el bloqueo de pantalla." },
];

function ConfirmResetDialog({ record, device, onClose, onConfirmed }: {
  record: ResetRecord | null;
  device: DeviceView | null;
  onClose: () => void;
  onConfirmed: (r: ResetRecord) => void;
}) {
  const toast = useToast();
  const [challenge, setChallenge] = useState<ResetChallenge | null>(null);
  const [typed, setTyped] = useState("");
  const [owner, setOwner] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setChallenge(null);
    setTyped("");
    setOwner("");
    if (record) api.reset.challenge(record.id).then(setChallenge).catch((e: ApiError) => toast(e.message, "error"));
  }, [record, toast]);

  async function confirm() {
    if (!record || !challenge) return;
    setBusy(true);
    try {
      onConfirmed(await api.reset.confirm(record.id, challenge.token, typed, owner));
    } catch (e) {
      toast((e as ApiError).message, "error");
      // A failed attempt consumes the challenge: ask for a new one.
      api.reset.challenge(record.id).then(setChallenge).catch(() => setChallenge(null));
      setTyped("");
    } finally {
      setBusy(false);
    }
  }

  const ready = challenge !== null && typed.trim() === challenge.phrase && owner.trim().length >= 3;
  return (
    <Dialog open={record !== null} onClose={onClose} title="Confirmar el borrado de todos los datos" width="max-w-lg"
            footer={<><Button variant="ghost" onClick={onClose}>Cancelar</Button><Button variant="danger" disabled={!ready} loading={busy} onClick={confirm}>Confirmar borrado</Button></>}>
      <Notice tone="danger" title="Esta acción no se puede deshacer">
        Se borrarán fotos, mensajes, aplicaciones y cuentas del equipo. Compruebe que el equipo es el correcto.
      </Notice>
      <dl className="mt-4 grid grid-cols-[110px_1fr] gap-y-1.5 text-[13px]">
        <dt className="text-ink-400">Equipo</dt>
        <dd className="text-ink-100">{device ? deviceName(device) : record?.device_label}</dd>
        {device && <><dt className="text-ink-400">Serie</dt><dd className="font-mono text-ink-100">{device.serial_masked}</dd></>}
        <dt className="text-ink-400">Método</dt>
        <dd className="text-ink-100">{record?.method === "recovery" ? "Recovery" : "Ajustes del teléfono"}</dd>
      </dl>
      <div className="mt-4 space-y-4">
        <TextInput label="Nombre de quien autoriza el borrado" placeholder="Propietario o representante autorizado" value={owner} onChange={(e) => setOwner(e.target.value)} />
        {challenge ? (
          <TextInput label={`Escriba ${challenge.phrase} para confirmar`} spellCheck={false} autoComplete="off" value={typed}
                     hint={`La confirmación vale ${Math.round(challenge.expires_in_seconds / 60)} minutos y solo una vez.`}
                     onChange={(e) => setTyped(e.target.value)} className="font-mono" />
        ) : <Spinner label="Generando confirmación…" />}
      </div>
    </Dialog>
  );
}

function ActiveReset({ record, device, onChange, onNavigate }: {
  record: ResetRecord;
  device: DeviceView | null;
  onChange: (r: ResetRecord) => void;
  onNavigate: (route: "frp" | "google") => void;
}) {
  const toast = useToast();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [wizard, setWizard] = useState<boolean | null>(null);
  const [asks, setAsks] = useState<boolean | null>(null);
  const [note, setNote] = useState("");
  const sameDevice = device !== null && record.device_id !== null;
  const canAssist = sameDevice && device?.mode === "adb";

  async function run(name: string, fn: () => Promise<ResetRecord>, message?: string) {
    setBusy(name);
    try {
      onChange(await fn());
      if (message) toast(message, "ok");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  const status = RESET_STATUS[record.status];
  return (
    <Panel title={`Restablecimiento #${record.id}`} description={<>{record.device_label || (device ? deviceName(device) : "")} · <Badge tone={status.tone}>{status.label}</Badge></>}
           actions={(record.status === "prepared" || record.status === "confirmed") && (
             <Button size="sm" variant="ghost" disabled={busy !== null} onClick={() => run("cancel", () => api.reset.cancel(record.id), "Restablecimiento cancelado")}>Cancelar</Button>
           )}>
      {record.status === "prepared" && (
        <div>
          <Notice tone="warn" title="Falta la confirmación explícita">Antes de borrar, confirme con el propietario y escriba el código de confirmación.</Notice>
          <Button variant="danger" icon={TriangleAlert} className="mt-4" onClick={() => setConfirming(true)}>Confirmar borrado…</Button>
        </div>
      )}
      {record.status === "confirmed" && (
        <div className="space-y-4">
          <p className="text-[13px] text-ink-300">Confirmado por {record.confirmed_by} el {formatDateTime(record.confirmed_at)} · autoriza {record.owner_name}.</p>
          <section>
            <h3 className="mb-1.5 flex items-center gap-2 text-[13px] font-medium text-ink-200">Pasos en el teléfono <Badge tone="brand">Instrucción</Badge></h3>
            <ol className="list-decimal space-y-1 pl-5 text-[13px] text-ink-300 marker:text-ink-500">{record.instructions.map((s) => <li key={s}>{s}</li>)}</ol>
          </section>
          <div className="flex flex-wrap gap-2">
            {canAssist && record.method === "settings" && (
              <Button variant="primary" icon={Settings2} loading={busy === "assist"} onClick={() => run("assist", () => api.reset.assist(record.id, device!.id, "open_settings"), "Ajustes abierto en el teléfono")}>
                Abrir Ajustes en el teléfono
              </Button>
            )}
            {canAssist && record.method === "recovery" && (
              <Button variant="primary" icon={Power} loading={busy === "assist"} onClick={() => run("assist", () => api.reset.assist(record.id, device!.id, "reboot_recovery"), "Reiniciando en Recovery")}>
                Reiniciar en Recovery
              </Button>
            )}
            <Button loading={busy === "manual"} onClick={() => run("manual", () => api.reset.manualStart(record.id))}>El borrado se inició en el teléfono</Button>
          </div>
          {!canAssist && <p className="text-xs text-ink-500">La ayuda desde la aplicación requiere el mismo equipo conectado con ADB autorizado.</p>}
        </div>
      )}
      {record.status === "pending_verification" && (
        <div>
          <p className="text-[13px] text-ink-300">Cuando el teléfono termine y se reinicie, verifique lo que muestra la pantalla.</p>
          <div className="mt-2 divide-y divide-ink-800">
            <YesNo label="¿El teléfono muestra el asistente de configuración (pantalla de bienvenida)?" value={wizard} onChange={setWizard} />
            <YesNo label="¿Pide una cuenta Google o del fabricante usada antes?" value={asks} onChange={setAsks} />
          </div>
          <TextInput label="Observaciones (opcional)" value={note} onChange={(e) => setNote(e.target.value)} />
          <Button variant="primary" icon={ShieldCheck} className="mt-3" disabled={wizard === null || asks === null} loading={busy === "verify"}
                  onClick={() => run("verify", () => api.reset.verify(record.id, device?.id ?? null, wizard!, asks!, note))}>
            Registrar verificación
          </Button>
        </div>
      )}
      {record.verification && (
        <div className="space-y-3">
          <Notice tone={record.verification.verified ? "ok" : "danger"} title={record.verification.verified ? "Restablecimiento verificado" : "Restablecimiento no verificado"}>
            {record.verification.message}
            {record.verification.disconnect_observed && <> AndroidMasters vio el equipo desconectarse el {formatDateTime(record.verification.disconnect_observed)}.</>}
          </Notice>
          {record.verification.asks_previous_account && <Button icon={ShieldCheck} onClick={() => onNavigate("frp")}>Ir a Diagnóstico FRP</Button>}
        </div>
      )}
      <ConfirmResetDialog record={confirming ? record : null} device={sameDevice ? device : null} onClose={() => setConfirming(false)}
                          onConfirmed={(r) => { setConfirming(false); onChange(r); toast("Borrado confirmado", "ok"); }} />
    </Panel>
  );
}

export function ResetPage({ onNavigate }: { onNavigate: (route: "frp" | "google") => void }) {
  const toast = useToast();
  const { devices, selected, setChoice, device, manualLabel, setManualLabel } = useDeviceChoice();
  const history = useAsync(api.reset.list);
  const [preflight, setPreflight] = useState<ResetPreflight | null>(null);
  const [method, setMethod] = useState<ResetMethod>("settings");
  const [active, setActive] = useState<ResetRecord | null>(null);
  const [busy, setBusy] = useState(false);
  const label = device ? "" : manualLabel.trim();
  const hasTarget = device !== null || label !== "";

  const check = useCallback(async () => {
    if (!hasTarget) return setPreflight(null);
    try {
      setPreflight(await api.reset.preflight(device?.id ?? null, label));
    } catch (e) {
      toast((e as ApiError).message, "error");
    }
  }, [device?.id, label, hasTarget, toast]);

  useEffect(() => {
    setActive(null);
    setPreflight(null);
  }, [device?.id, label]);

  async function prepare() {
    setBusy(true);
    try {
      setActive(await api.reset.prepare(device?.id ?? null, label, method));
      await history.reload();
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <PageHeader title="Restablecimiento" description="Restablecimiento de fábrica autorizado: comprobaciones, confirmación explícita, pasos oficiales en el teléfono y verificación posterior." />
      <Notice tone="info" className="mb-5" title="AndroidMasters no borra el teléfono por sí mismo">
        El borrado lo confirma el propietario o el técnico en el propio equipo, por el camino oficial. La aplicación comprueba los requisitos, registra la confirmación y verifica el resultado.
      </Notice>
      <div className="mb-5 grid max-w-3xl gap-4 sm:grid-cols-2">
        <Select label="Equipo" value={selected} onChange={(e) => setChoice(e.target.value)}
                options={[...devices.map((d) => ({ value: d.id, label: `${deviceName(d)} · ${MODE[d.mode].label}` })), { value: MANUAL_DEVICE, label: "Equipo no conectado (escribir marca y modelo)" }]} />
        {!device && <TextInput label="Marca y modelo" placeholder="Como figura en su lista de preparación" value={manualLabel} onChange={(e) => setManualLabel(e.target.value)} />}
      </div>
      <div className="grid gap-5 xl:grid-cols-[1fr_440px]">
        <div className="space-y-5">
          <Panel title="1 · Comprobaciones previas" actions={<Button size="sm" icon={RefreshCw} disabled={!hasTarget} onClick={check}>Comprobar</Button>}>
            {!hasTarget && <p className="text-[13px] text-ink-400">Seleccione el equipo.</p>}
            {hasTarget && !preflight && <p className="text-[13px] text-ink-400">Pulse «Comprobar» para revisar autorización, lista de preparación y cuentas.</p>}
            {preflight && (
              <>
                <CheckList items={preflight.items} />
                {!preflight.ready && (
                  <div className="mt-3 flex items-center gap-3">
                    <p className="text-[13px] text-ink-300">Resuelva los puntos en rojo para continuar.</p>
                    <Button size="sm" onClick={() => onNavigate("google")}>Abrir Cuentas Google y lista de preparación</Button>
                  </div>
                )}
              </>
            )}
          </Panel>
          <Panel title="2 · Método oficial">
            <div role="radiogroup" aria-label="Método" className="grid gap-2 sm:grid-cols-2">
              {METHODS.map((m) => (
                <button key={m.value} type="button" role="radio" aria-checked={method === m.value} onClick={() => setMethod(m.value)}
                        className={cn("pressable rounded-lg border px-4 py-3 text-left", method === m.value ? "border-brand-500/60 bg-brand-500/10" : "border-ink-700 hover:border-ink-600")}>
                  <p className="text-[13px] font-medium text-ink-100">{m.title}</p>
                  <p className="mt-0.5 text-xs text-ink-400">{m.text}</p>
                </button>
              ))}
            </div>
            <Button variant="primary" icon={RotateCcw} className="mt-4" disabled={!preflight?.ready || active !== null} loading={busy} onClick={prepare}>
              Preparar restablecimiento
            </Button>
          </Panel>
        </div>
        <div>
          {active ? (
            <ActiveReset record={active} device={device} onChange={(r) => { setActive(r); void history.reload(); }} onNavigate={onNavigate} />
          ) : (
            <Panel title="3 · Confirmación y verificación">
              <p className="text-[13px] text-ink-400">Aparecerá aquí cuando prepare el restablecimiento.</p>
            </Panel>
          )}
        </div>
      </div>
      <Panel title="Historial de restablecimientos" className="mt-5">
        {history.error && <Notice tone="danger">{history.error.message}</Notice>}
        {history.data && (
          <Table<ResetRecord>
            rows={history.data}
            rowKey={(r) => r.id}
            empty={<EmptyState icon={History} title="Todavía no hay restablecimientos" />}
            columns={[
              { key: "id", header: "#", render: (r) => r.id },
              { key: "date", header: "Fecha", render: (r) => formatDateTime(r.created_at) },
              { key: "device", header: "Equipo", render: (r) => r.device_label || "Equipo conectado" },
              { key: "method", header: "Método", render: (r) => (r.method === "recovery" ? "Recovery" : "Ajustes") },
              { key: "status", header: "Estado", render: (r) => <Badge tone={RESET_STATUS[r.status].tone}>{RESET_STATUS[r.status].label}</Badge> },
              { key: "open", header: "", className: "text-right", render: (r) => (
                ["prepared", "confirmed", "pending_verification"].includes(r.status) && (r.id !== active?.id)
                  ? <Button size="sm" variant="ghost" onClick={() => setActive(r)}>Continuar</Button> : null) },
            ]}
          />
        )}
      </Panel>
    </>
  );
}
