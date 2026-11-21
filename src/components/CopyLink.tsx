import { Copy } from "lucide-react";
import { Button } from "@/components/Button";
import { useToast } from "@/components/Toaster";

/** Official link shown for the owner to open in their own browser, with a copy button. */
export function CopyLink({ label, url }: { label: string; url: string }) {
  const toast = useToast();
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg bg-ink-850 px-3 py-2 text-[13px]">
      <span className="min-w-0">
        <span className="block text-ink-200">{label}</span>
        <span className="block truncate font-mono text-xs text-ink-400" data-selectable>{url}</span>
      </span>
      <Button size="sm" variant="ghost" icon={Copy} aria-label={`Copiar ${label}`}
              onClick={() => navigator.clipboard.writeText(url).then(() => toast("Enlace copiado", "ok"), () => toast("No se pudo copiar", "error"))}>
        Copiar
      </Button>
    </div>
  );
}
