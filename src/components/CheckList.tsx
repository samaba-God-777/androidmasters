import { CircleAlert, CircleCheck, Info, TriangleAlert } from "lucide-react";
import type { CheckItem } from "@/types/api";
import { cn } from "@/utils/cn";

const ICON = { pass: CircleCheck, info: Info, warn: TriangleAlert, fail: CircleAlert } as const;
const COLOR = { pass: "text-ok", info: "text-info", warn: "text-warn", fail: "text-danger" } as const;

/** Results of a diagnosis: what was detected, and what to do about it. */
export function CheckList({ items }: { items: CheckItem[] }) {
  return (
    <ul className="divide-y divide-ink-800">
      {items.map((item) => {
        const Icon = ICON[item.status];
        return (
          <li key={item.key} className="flex gap-3 py-3">
            <Icon className={cn("mt-0.5 size-4 shrink-0", COLOR[item.status])} aria-label={item.status} />
            <div className="min-w-0 text-[13px]">
              <p>
                <span className="font-medium text-ink-100">{item.title}</span>
                <span className="text-ink-400"> · </span>
                <span className="text-ink-200" data-selectable>{item.detail}</span>
              </p>
              {item.recommendation && <p className="mt-1 text-ink-400">{item.recommendation}</p>}
            </div>
          </li>
        );
      })}
    </ul>
  );
}
