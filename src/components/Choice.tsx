import { useId } from "react";
import { cn } from "@/utils/cn";

/** Yes / No / Unknown answer as an accessible radio group. */
export function YesNo({ label, hint, value, onChange }: {
  label: string;
  hint?: string;
  value: boolean | null;
  onChange: (value: boolean | null) => void;
}) {
  const options: Array<[string, boolean | null]> = [["Sí", true], ["No", false], ["No se sabe", null]];
  const id = useId();
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 py-3">
      <div className="min-w-0 flex-1 basis-80">
        <p id={id} className="text-[13px] text-ink-100">{label}</p>
        {hint && <p className="mt-0.5 text-xs text-ink-400">{hint}</p>}
      </div>
      <div role="radiogroup" aria-labelledby={id} className="flex rounded-lg border border-ink-700 bg-ink-925 p-0.5">
        {options.map(([text, v]) => {
          const active = value === v;
          return (
            <button
              key={text}
              type="button"
              role="radio"
              aria-checked={active}
              onClick={() => onChange(v)}
              className={cn(
                "pressable rounded-md px-3 py-1 text-[13px]",
                active ? (v === null ? "bg-ink-750 text-ink-100" : "bg-brand-500/20 text-brand-300") : "text-ink-400 hover:text-ink-100",
              )}
            >
              {text}
            </button>
          );
        })}
      </div>
    </div>
  );
}
