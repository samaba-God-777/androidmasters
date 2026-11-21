import { LogOut, Smartphone } from "lucide-react";
import { useDevices } from "@/hooks/useDevices";
import { Button } from "@/components/Button";
import { Badge } from "@/components/Feedback";
import type { User } from "@/types/api";
import { ROLE_LABEL } from "@/utils/labels";

function LiveDevices({ onOpen }: { onOpen: () => void }) {
  const { snapshot } = useDevices();
  if (!snapshot) return null;
  const n = snapshot.devices.length;
  const broken = Boolean(snapshot.tools.last_error);
  return (
    <button type="button" onClick={onOpen} className="mr-auto flex items-center gap-2 rounded-lg px-2 py-1 text-[13px] text-ink-300 hover:bg-ink-850 hover:text-ink-100">
      <span className={`size-2 rounded-full ${broken ? "bg-danger" : n > 0 ? "bg-ok" : "bg-ink-600"}`} aria-hidden />
      <Smartphone className="size-4" aria-hidden />
      {broken ? "Detección detenida" : n === 0 ? "Sin dispositivos" : `${n} dispositivo${n === 1 ? "" : "s"}`}
    </button>
  );
}

export function Topbar({ user, onSignOut, onOpenDevices }: { user: User; onSignOut: () => void; onOpenDevices: () => void }) {
  return (
    <header className="flex h-14 shrink-0 items-center justify-end gap-3 border-b border-ink-800 bg-ink-950/80 px-6">
      <LiveDevices onOpen={onOpenDevices} />
      <div className="text-right leading-tight">
        <p className="text-[13px] font-medium text-ink-100">{user.display_name}</p>
        <p className="text-[11px] text-ink-400">@{user.username}</p>
      </div>
      <Badge tone={user.role === "admin" ? "brand" : "neutral"}>{ROLE_LABEL[user.role]}</Badge>
      <Button variant="ghost" size="sm" icon={LogOut} onClick={onSignOut}>
        Salir
      </Button>
    </header>
  );
}
