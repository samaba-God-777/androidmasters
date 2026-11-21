import type { ReactNode } from "react";
import { cn } from "@/utils/cn";

/** A flat, bordered panel. Panels are never nested inside other panels. */
export function Panel({ title, description, actions, children, className }: {
  title?: string;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("rounded-xl border border-ink-800 bg-ink-900", className)}>
      {(title || actions) && (
        <header className="flex items-start justify-between gap-4 border-b border-ink-800 px-5 py-3.5">
          <div>
            {title && <h2 className="text-[15px] font-semibold text-ink-100">{title}</h2>}
            {description && <p className="mt-0.5 text-[13px] text-ink-400">{description}</p>}
          </div>
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </header>
      )}
      <div className="px-5 py-4">{children}</div>
    </section>
  );
}

export function PageHeader({ title, description, actions }: { title: string; description?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div className="max-w-2xl">
        <h1 className="text-[22px] font-semibold tracking-tight text-ink-100">{title}</h1>
        {description && <p className="mt-1 text-sm text-ink-400">{description}</p>}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  );
}
