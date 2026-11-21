import { useState } from "react";
import { BadgeCheck, ChevronDown } from "lucide-react";
import { Button } from "@/components/Button";
import { CopyLink } from "@/components/CopyLink";
import { Dialog } from "@/components/Dialog";
import { TextInput } from "@/components/Field";
import { Badge, Notice } from "@/components/Feedback";
import { useToast } from "@/components/Toaster";
import { api, type ApiError } from "@/services/api";
import type { Procedure } from "@/types/api";
import { formatDateTime } from "@/utils/format";

export function androidWarning(p: Pick<Procedure, "android_min" | "android_max">, androidVersion: string | null | undefined): string | null {
  if (!androidVersion) return null;
  const major = Number.parseInt(androidVersion, 10);
  if (Number.isNaN(major)) return null;
  if (p.android_min && major < p.android_min) return `Escrito para Android ${p.android_min} o posterior; este equipo tiene Android ${androidVersion}. Los menús pueden ser distintos.`;
  if (p.android_max && major > p.android_max) return `Escrito hasta Android ${p.android_max}; este equipo tiene Android ${androidVersion}. Revíselo antes de usarlo.`;
  return null;
}

function ReviewBadge({ procedure }: { procedure: Procedure }) {
  const r = procedure.review;
  if (r?.current) return <Badge tone="ok"><BadgeCheck className="size-3" aria-hidden /> Revisado por {r.reviewed_by}</Badge>;
  if (r) return <Badge tone="warn">Revisión desactualizada (v{r.version})</Badge>;
  return <Badge tone="warn">Pendiente de revisión del taller</Badge>;
}

/** An official procedure: always guidance for the owner, never an executed action. */
export function ProcedureCard({ procedure, androidVersion, canReview, onReviewed, defaultOpen = false }: {
  procedure: Procedure;
  androidVersion?: string | null;
  canReview: boolean;
  onReviewed: () => void;
  defaultOpen?: boolean;
}) {
  const toast = useToast();
  const [open, setOpen] = useState(defaultOpen);
  const [reviewing, setReviewing] = useState(false);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const warning = androidWarning(procedure, androidVersion);

  async function review() {
    setBusy(true);
    try {
      await api.procedures.review(procedure.id, note);
      toast("Procedimiento marcado como revisado", "ok");
      setReviewing(false);
      setNote("");
      onReviewed();
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className="rounded-xl border border-ink-800 bg-ink-900">
      <button type="button" onClick={() => setOpen((o) => !o)} aria-expanded={open}
              className="flex w-full items-start justify-between gap-4 px-5 py-4 text-left hover:bg-ink-850/50">
        <div className="min-w-0">
          <p className="text-xs text-ink-400">{procedure.service} · v{procedure.version}</p>
          <h3 className="mt-0.5 text-[15px] font-medium text-ink-100">{procedure.title}</h3>
          <div className="mt-2 flex flex-wrap gap-1.5">
            <Badge tone="brand">Instrucción para el propietario</Badge>
            <ReviewBadge procedure={procedure} />
          </div>
        </div>
        <ChevronDown className={`mt-1 size-4 shrink-0 text-ink-400 transition-transform duration-150 ${open ? "rotate-180" : ""}`} aria-hidden />
      </button>
      {open && (
        <div className="border-t border-ink-800 px-5 py-4 text-[13px]">
          <p className="text-ink-300">{procedure.summary}</p>
          {warning && <Notice tone="warn" className="mt-3">{warning}</Notice>}
          <h4 className="mt-4 mb-1.5 font-medium text-ink-200">Pasos</h4>
          <ol className="list-decimal space-y-1 pl-5 text-ink-300 marker:text-ink-500">{procedure.steps.map((s) => <li key={s}>{s}</li>)}</ol>
          <h4 className="mt-4 mb-1.5 font-medium text-ink-200">Cómo verificarlo</h4>
          <ul className="list-disc space-y-1 pl-5 text-ink-300">{procedure.verification.map((s) => <li key={s}>{s}</li>)}</ul>
          {procedure.notes.length > 0 && <ul className="mt-3 space-y-1 text-xs text-ink-400">{procedure.notes.map((n) => <li key={n}>{n}</li>)}</ul>}
          {procedure.links.length > 0 && (
            <div className="mt-4 space-y-1.5">
              {procedure.links.map((l) => <CopyLink key={l.url} label={l.label} url={l.url} />)}
            </div>
          )}
          <div className="mt-4 flex items-center justify-between gap-3 text-xs text-ink-500">
            <span>
              {procedure.review?.current
                ? `Revisado el ${formatDateTime(procedure.review.reviewed_at)}${procedure.review.note ? ` · ${procedure.review.note}` : ""}`
                : "Revise los pasos en un equipo real de este fabricante antes de usarlos con clientes."}
            </span>
            {canReview && !procedure.review?.current && (
              <Button size="sm" icon={BadgeCheck} onClick={() => setReviewing(true)}>Marcar como revisado</Button>
            )}
          </div>
        </div>
      )}
      <Dialog open={reviewing} onClose={() => setReviewing(false)} title="Marcar como revisado"
              description={`${procedure.title} · versión ${procedure.version}`}
              footer={<><Button variant="ghost" onClick={() => setReviewing(false)}>Cancelar</Button><Button variant="primary" loading={busy} onClick={review}>Confirmar revisión</Button></>}>
        <TextInput label="Nota de la revisión" placeholder="Ej. comprobado en Galaxy A14 con One UI 6" value={note} onChange={(e) => setNote(e.target.value)} />
        <p className="mt-3 text-xs text-ink-400">Confirma que los pasos coinciden con los menús reales. Si el procedimiento cambia de versión, la revisión deja de valer.</p>
      </Dialog>
    </article>
  );
}
