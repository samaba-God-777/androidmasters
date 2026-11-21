import type { ReactNode } from "react";
import { CircleAlert, CircleCheck, Info, LoaderCircle, TriangleAlert, type LucideIcon } from "lucide-react";
import { cn } from "@/utils/cn";

export type Tone = "neutral" | "info" | "ok" | "warn" | "danger" | "brand";

const BADGE: Record<Tone, string> = {
  neutral: "bg-ink-800 text-ink-300 border-ink-700",
  info: "bg-info/10 text-info border-info/30",
  ok: "bg-ok/10 text-ok border-ok/30",
  warn: "bg-warn/10 text-warn border-warn/30",
  danger: "bg-danger/10 text-danger border-danger/30",
  brand: "bg-brand-500/10 text-brand-300 border-brand-500/30",
};

export function Badge({ tone = "neutral", children, className }: { tone?: Tone; children: ReactNode; className?: string }) {
  return (
    <span className={cn("inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-xs font-medium whitespace-nowrap", BADGE[tone], className)}>
      {children}
    </span>
  );
}

const NOTICE: Record<Exclude<Tone, "neutral" | "brand">, { icon: LucideIcon; cls: string }> = {
  info: { icon: Info, cls: "border-info/30 bg-info/5 text-ink-200 [&_svg]:text-info" },
  ok: { icon: CircleCheck, cls: "border-ok/30 bg-ok/5 text-ink-200 [&_svg]:text-ok" },
  warn: { icon: TriangleAlert, cls: "border-warn/35 bg-warn/5 text-ink-200 [&_svg]:text-warn" },
  danger: { icon: CircleAlert, cls: "border-danger/40 bg-danger/5 text-ink-200 [&_svg]:text-danger" },
};

export function Notice({ tone = "info", title, children, className }: {
  tone?: keyof typeof NOTICE;
  title?: string;
  children?: ReactNode;
  className?: string;
}) {
  const { icon: Icon, cls } = NOTICE[tone];
  return (
    <div role={tone === "danger" ? "alert" : "status"} className={cn("flex gap-3 rounded-lg border px-3.5 py-3 text-[13px]", cls, className)}>
      <Icon className="mt-0.5 size-4 shrink-0" aria-hidden />
      <div className="min-w-0">
        {title && <p className="font-semibold text-ink-100">{title}</p>}
        {children && <div className={cn(title && "mt-0.5", "text-ink-300")}>{children}</div>}
      </div>
    </div>
  );
}

export function Spinner({ label = "Cargando…" }: { label?: string }) {
  return (
    <div role="status" className="flex items-center gap-2 text-sm text-ink-400">
      <LoaderCircle className="size-4 animate-spin" aria-hidden />
      {label}
    </div>
  );
}

export function EmptyState({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children?: ReactNode }) {
  return (
    <div className="flex flex-col items-center px-6 py-10 text-center">
      <Icon className="mb-3 size-6 text-ink-500" aria-hidden />
      <p className="text-sm font-medium text-ink-200">{title}</p>
      {children && <div className="mt-1 max-w-sm text-[13px] text-ink-400">{children}</div>}
    </div>
  );
}
