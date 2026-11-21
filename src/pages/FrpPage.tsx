import { useEffect, useState } from "react";
import { ClipboardCheck, History, ScanSearch, ShieldAlert } from "lucide-react";
import { CopyLink } from "@/components/CopyLink";
import { Button } from "@/components/Button";
import { CheckList } from "@/components/CheckList";
import { YesNo } from "@/components/Choice";
import { Select, TextInput } from "@/components/Field";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { PageHeader, Panel } from "@/components/Surface";
import { Table } from "@/components/Table";
import { useToast } from "@/components/Toaster";
import { useAsync } from "@/hooks/useAsync";
import { useDevices } from "@/hooks/useDevices";
import { api, type ApiError } from "@/services/api";
import type { FrpAssessment, FrpIndicators, FrpObservation, FrpRecord, ResetOrigin } from "@/types/api";
import { EMPTY_OBSERVATION, FRP_SUMMARY, OFFICIAL_LINKS, OUTCOME, QUESTIONS, RESET_ORIGINS } from "@/modules/frp/content";
import { formatDateTime } from "@/utils/format";
import { MODE, deviceName } from "@/utils/labels";

const MANUAL = "__manual__";

function StepList({ title, kind, items }: { title: string; kind: "diagnosis" | "instruction"; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <section className="mt-4">
      <h3 className="mb-1.5 flex items-center gap-2 text-[13px] font-medium text-ink-200">
        {title}
        <Badge tone={kind === "diagnosis" ? "info" : "brand"}>{kind === "diagnosis" ? "Diagnóstico" : "Instrucción para el propietario"}</Badge>
      </h3>
      <ol className="list-decimal space-y-1 pl-5 text-[13px] text-ink-300 marker:text-ink-500">
        {items.map((s) => <li key={s}>{s}</li>)}
      </ol>
    </section>
  );
}

function Result({ assessment }: { assessment: FrpAssessment | null }) {
  if (!assessment) return <Spinner label="Evaluando…" />;
  const meta = OUTCOME[assessment.outcome];
  return (
    <div>
      <Badge tone={meta.tone}>{meta.label}</Badge>
      <p className="mt-2 text-[15px] font-medium text-ink-100">{assessment.headline}</p>
      {assessment.stop && (
        <Notice tone="danger" className="mt-3" title="No continúe con este equipo">
          AndroidMasters no ofrece ni ofrecerá formas de saltarse FRP. Sin prueba de propiedad o autorización, el servicio se detiene aquí.
        </Notice>
      )}
      {assessment.pending_questions.length > 0 && (
        <ul className="mt-3 list-disc space-y-1 pl-5 text-[13px] text-ink-400">
          {assessment.pending_questions.map((q) => <li key={q}>{q}</li>)}
        </ul>
      )}
      <StepList title="Causas posibles" kind="diagnosis" items={assessment.possible_causes} />
      <StepList title="Siguientes pasos" kind="instruction" items={assessment.next_steps} />
      {assessment.documentation.length > 0 && (
        <section className="mt-4">
          <h3 className="mb-1.5 text-[13px] font-medium text-ink-200">Documentación que puede pedir el fabricante</h3>
          <ul className="list-disc space-y-1 pl-5 text-[13px] text-ink-300">{assessment.documentation.map((d) => <li key={d}>{d}</li>)}</ul>
        </section>
      )}
      {(assessment.outcome === "account_recovery" || assessment.outcome === "wait_security_period" || assessment.outcome === "owner_signs_in") && (
        <section className="mt-4 space-y-1.5">
          <h3 className="text-[13px] font-medium text-ink-200">Recursos oficiales de Google</h3>
          {OFFICIAL_LINKS.map((l) => <CopyLink key={l.url} label={l.label} url={l.url} />)}
        </section>
      )}
    </div>
  );
}

export function FrpPage() {
  const toast = useToast();
  const { snapshot } = useDevices();
  const devices = snapshot?.devices ?? [];
  const history = useAsync(api.frp.history);
  const [deviceId, setDeviceId] = useState<string>("");
  const [manualLabel, setManualLabel] = useState("");
  const [indicators, setIndicators] = useState<FrpIndicators | null>(null);
  const [observation, setObservation] = useState<FrpObservation>(EMPTY_OBSERVATION);
  const [assessment, setAssessment] = useState<FrpAssessment | null>(null);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState<"read" | "save" | null>(null);

  const connected = devices.find((d) => d.id === deviceId) ?? null;
  const selection = connected ? connected.id : deviceId === "" && devices.length > 0 ? devices[0]!.id : MANUAL;
  const device = devices.find((d) => d.id === selection) ?? null;

  useEffect(() => setIndicators(null), [selection]);
  useEffect(() => {
    let current = true;
    api.frp.assess(observation).then((a) => current && setAssessment(a)).catch((e: ApiError) => toast(e.message, "error"));
    return () => {
      current = false;
    };
  }, [observation, toast]);

  const set = <K extends keyof FrpObservation>(key: K, value: FrpObservation[K]) => setObservation((o) => ({ ...o, [key]: value }));

  async function readIndicators() {
    if (!device) return;
    setBusy("read");
    try {
      setIndicators(await api.frp.indicators(device.id));
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  async function save() {
    setBusy("save");
    try {
      await api.frp.record(device?.id ?? null, device ? deviceName(device) : manualLabel, observation, notes);
      toast("Diagnóstico FRP registrado", "ok");
      setObservation(EMPTY_OBSERVATION);
      setNotes("");
      await history.reload();
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(null);
    }
  }

  const canSave = assessment !== null && assessment.outcome !== "incomplete" && (device !== null || manualLabel.trim() !== "");

  return (
    <>
      <PageHeader title="Diagnóstico FRP" description="Registro de indicios de Factory Reset Protection y orientación hacia la recuperación oficial." />
      <Notice tone="info" className="mb-5" title="Qué es FRP">
        {FRP_SUMMARY} AndroidMasters no incluye bypass, exploits ni herramientas para evadirla.
      </Notice>
      <div className="grid gap-5 xl:grid-cols-[1fr_440px]">
        <div className="space-y-5">
          <Panel title="1 · Equipo">
            <div className="grid gap-4 sm:grid-cols-2">
              <Select label="Dispositivo" value={selection} onChange={(e) => setDeviceId(e.target.value === MANUAL ? MANUAL : e.target.value)}
                      options={[...devices.map((d) => ({ value: d.id, label: `${deviceName(d)} · ${MODE[d.mode].label}` })), { value: MANUAL, label: "Equipo no conectado (escribir marca y modelo)" }]} />
              {!device && <TextInput label="Marca y modelo" placeholder="Ej. Samsung Galaxy A14" value={manualLabel} onChange={(e) => setManualLabel(e.target.value)} />}
            </div>
            {device && (
              <div className="mt-4">
                <Button icon={ScanSearch} loading={busy === "read"} onClick={readIndicators}>Leer indicios del equipo</Button>
                {device.mode !== "adb" && <p className="mt-2 text-xs text-ink-400">Con el equipo en «{MODE[device.mode].label}» solo se puede leer información limitada.</p>}
                {indicators && (
                  <div className="mt-4">
                    <p className="mb-1 flex items-center gap-2 text-xs text-ink-400">Detectado en el equipo <Badge tone="info">Diagnóstico</Badge></p>
                    <CheckList items={indicators.items} />
                  </div>
                )}
              </div>
            )}
          </Panel>
          <Panel title="2 · Observaciones del técnico y respuestas del propietario">
            <div className="divide-y divide-ink-800">
              {QUESTIONS.filter((q) => !q.when || q.when(observation)).map((q) => (
                <YesNo key={q.key} label={q.text} hint={q.hint} value={observation[q.key]} onChange={(v) => set(q.key, v)} />
              ))}
              <div className="py-3">
                <Select label="¿Cómo se restableció el equipo?" value={observation.reset_origin ?? ""}
                        onChange={(e) => set("reset_origin", (e.target.value || null) as ResetOrigin | null)}
                        options={[{ value: "", label: "Sin respuesta" }, ...RESET_ORIGINS]} />
              </div>
            </div>
          </Panel>
        </div>
        <div className="space-y-5">
          <Panel title="3 · Resultado">
            <Result assessment={assessment} />
            <div className="mt-5 border-t border-ink-800 pt-4">
              <TextInput label="Observaciones (opcional)" placeholder="Lo que se vio en pantalla, documentos presentados…" value={notes} onChange={(e) => setNotes(e.target.value)} />
              <Button variant="primary" icon={ClipboardCheck} className="mt-3 w-full" disabled={!canSave} loading={busy === "save"} onClick={save}>
                Registrar diagnóstico
              </Button>
              {!canSave && <p className="mt-2 text-xs text-ink-500">Responda las preguntas obligatorias e indique el equipo para registrar.</p>}
            </div>
          </Panel>
        </div>
      </div>
      <Panel title="Historial de diagnósticos FRP" className="mt-5">
        {history.error && <Notice tone="danger">{history.error.message}</Notice>}
        {history.data && (
          <Table<FrpRecord>
            rows={history.data}
            rowKey={(r) => r.id}
            empty={<EmptyState icon={History} title="Todavía no hay diagnósticos registrados" />}
            columns={[
              { key: "date", header: "Fecha", render: (r) => formatDateTime(r.created_at) },
              { key: "device", header: "Equipo", render: (r) => r.device },
              { key: "outcome", header: "Resultado", render: (r) => <Badge tone={OUTCOME[r.outcome]?.tone ?? "neutral"}>{OUTCOME[r.outcome]?.label ?? r.outcome}</Badge> },
              { key: "author", header: "Técnico", render: (r) => r.author ?? "—" },
            ]}
          />
        )}
      </Panel>
      <p className="mt-4 flex items-center gap-1.5 text-xs text-ink-500"><ShieldAlert className="size-3.5" aria-hidden /> Cada diagnóstico queda en la auditoría con el técnico responsable.</p>
    </>
  );
}
