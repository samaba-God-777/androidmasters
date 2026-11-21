import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/Button";

/** Native modal <dialog>: focus trap, Escape to close and inert background come from the platform. */
export function Dialog({ open, onClose, title, description, children, footer, width = "max-w-md" }: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (open && !el.open) {
      el.showModal?.();
      // Start in the first field (not on the close button) so the user can type right away.
      el.querySelector<HTMLElement>("[autofocus], input:not([type=hidden]), select, textarea")?.focus();
    }
    if (!open && el.open) el.close?.();
  }, [open]);
  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      aria-labelledby={titleId}
      className={`am-dialog m-auto w-[calc(100%-2rem)] ${width} rounded-xl border border-ink-700 bg-ink-900 p-0 text-ink-100 shadow-2xl shadow-black/50`}
    >
      {open && (
        <>
          <header className="flex items-start justify-between gap-4 px-5 pt-5">
            <div>
              <h2 id={titleId} className="text-base font-semibold">
                {title}
              </h2>
              {description && <div className="mt-1 text-[13px] text-ink-400">{description}</div>}
            </div>
            <button type="button" onClick={onClose} aria-label="Cerrar" className="rounded-md p-1 text-ink-400 hover:bg-ink-800 hover:text-ink-100">
              <X className="size-4" aria-hidden />
            </button>
          </header>
          {children && <div className="px-5 pt-4">{children}</div>}
          {footer && <footer className="mt-5 flex justify-end gap-2 border-t border-ink-800 px-5 py-3.5">{footer}</footer>}
        </>
      )}
    </dialog>
  );
}

/** Confirmation for important or destructive actions. With `phrase`, the user must type it exactly. */
export function ConfirmDialog({ open, onClose, onConfirm, title, children, confirmLabel = "Confirmar", destructive, phrase, busy }: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  children?: ReactNode;
  confirmLabel?: string;
  destructive?: boolean;
  phrase?: string;
  busy?: boolean;
}) {
  const [typed, setTyped] = useState("");
  useEffect(() => {
    if (!open) setTyped("");
  }, [open]);
  const ready = !phrase || typed.trim() === phrase;
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button variant={destructive ? "danger" : "primary"} disabled={!ready} loading={busy} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-sm text-ink-300">
        {children}
        {phrase && (
          <label className="block">
            <span className="text-[13px] text-ink-300">
              Escriba <strong className="font-mono text-ink-100">{phrase}</strong> para confirmar
            </span>
            <input
              autoFocus
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              className="mt-1.5 h-9 w-full rounded-lg border border-ink-700 bg-ink-925 px-3 font-mono text-sm text-ink-100 focus:border-brand-500 focus:outline-none"
            />
          </label>
        )}
      </div>
    </Dialog>
  );
}
