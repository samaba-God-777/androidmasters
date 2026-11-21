import { useEffect, useState } from "react";
import { CircleCheck, Circle, ListChecks } from "lucide-react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { TextInput } from "@/components/Field";
import { Badge, Notice, Spinner } from "@/components/Feedback";
import { Panel } from "@/components/Surface";
import { useToast } from "@/components/Toaster";
import { api, type ApiError } from "@/services/api";
import type { Checklist, ChecklistItem } from "@/types/api";
import { formatDateTime } from "@/utils/format";

/** Preparation for a new owner. Account items require evidence (account check or written on-screen verification). */
export function TransferChecklist({ deviceId, deviceLabel, refreshKey }: { deviceId: string | null; deviceLabel: string; refreshKey?: number }) {
  const toast = useToast();
  const [list, setList] = useState<Checklist | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<ChecklistItem | null>(null);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  // A connected device opens its list automatically. A typed label only opens one when the technician
  // asks for it, so partial text never creates lists ("S", "Sa", "Sam"…).
  const [requestedLabel, setRequestedLabel] = useState<string | null>(null);
  const label = deviceLabel.trim();
  const target = deviceId ?? requestedLabel;

  useEffect(() => {
    setRequestedLabel(null);
  }, [deviceId]);

  useEffect(() => {
    setList(null);
    setError(null);
    if (!target) return;
    api.checklist.open(deviceId, deviceId ? "" : target).then(setList).catch((e: ApiError) => setError(e.message));
  }, [deviceId, target, refreshKey]);

  async function setItem(item: ChecklistItem, done: boolean, withNote = "") {
    if (!list) return;
    setBusy(true);
    try {
      setList(await api.checklist.set(list.id, item.key, done, withNote));
      setPending(null);
      setNote("");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  const done = list?.items.filter((i) => i.state.done).length ?? 0;
  return (
    <Panel title="Preparación para nuevo propietario" description="Lista previa al restablecimiento y a la entrega."
           actions={list && <Badge tone={list.status === "complete" ? "ok" : "neutral"}>{done}/{list.items.length}</Badge>}>
      {!deviceId && label && label !== requestedLabel && (
        <Button className="mb-3" icon={ListChecks} onClick={() => setRequestedLabel(label)}>Abrir lista para «{label}»</Button>
      )}
      {!target && !label && <p className="text-[13px] text-ink-400">Seleccione el equipo o escriba su marca y modelo para abrir la lista.</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {target && !list && !error && <Spinner />}
      {list && (
        <>
          {list.status === "complete" && <Notice tone="ok" className="mb-3" title="Lista completa">El equipo está listo para el restablecimiento autorizado.</Notice>}
          <ul className="space-y-1">
            {list.items.map((item) => (
              <li key={item.key}>
                <button type="button" disabled={busy || list.status === "complete"}
                        onClick={() => (item.state.done ? setItem(item, false) : item.needs_account_evidence && !deviceId ? setPending(item) : setItem(item, true))}
                        className="flex w-full items-start gap-2.5 rounded-lg px-2 py-2 text-left text-[13px] hover:bg-ink-850 disabled:cursor-default disabled:hover:bg-transparent"
                        aria-pressed={item.state.done}>
                  {item.state.done ? <CircleCheck className="mt-0.5 size-4 shrink-0 text-ok" aria-hidden /> : <Circle className="mt-0.5 size-4 shrink-0 text-ink-500" aria-hidden />}
                  <span className="min-w-0">
                    <span className={item.state.done ? "text-ink-100" : "text-ink-300"}>{item.label}</span>
                    {item.needs_account_evidence && !item.state.done && <span className="block text-xs text-ink-500">Requiere comprobación de cuentas{deviceId ? "" : " o verificación escrita"}</span>}
                    {item.state.evidence && <span className="block text-xs text-ink-400">{item.state.evidence}</span>}
                    {item.state.done && item.state.at && <span className="block text-[11px] text-ink-500">{formatDateTime(item.state.at)}</span>}
                  </span>
                </button>
              </li>
            ))}
          </ul>
          <p className="mt-3 flex items-center gap-1.5 text-xs text-ink-500"><ListChecks className="size-3.5" aria-hidden /> Cada marca queda registrada con el técnico y la hora.</p>
        </>
      )}
      <Dialog open={pending !== null} onClose={() => setPending(null)} title="Verificación en pantalla"
              description="El equipo no está conectado: describa qué comprobó en la pantalla del teléfono."
              footer={<><Button variant="ghost" onClick={() => setPending(null)}>Cancelar</Button><Button variant="primary" loading={busy} onClick={() => pending && setItem(pending, true, note)}>Registrar</Button></>}>
        <TextInput label="Qué verificó" placeholder="Ej. Ajustes › Cuentas sin cuentas, revisado con el cliente" value={note} onChange={(e) => setNote(e.target.value)} />
      </Dialog>
    </Panel>
  );
}
