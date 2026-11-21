import { useEffect, useState } from "react";
import { ScanSearch, UserRoundCheck } from "lucide-react";
import { Button } from "@/components/Button";
import { Badge, EmptyState, Notice } from "@/components/Feedback";
import { Panel } from "@/components/Surface";
import { useToast } from "@/components/Toaster";
import { api, type ApiError } from "@/services/api";
import type { AccountCheck, AccountCheckResult, DeviceView } from "@/types/api";
import { formatDateTime } from "@/utils/format";

/** Account types and counts read from the device (never names), with before/after verification. */
export function AccountCheckPanel({ device, onChecked }: { device: DeviceView | null; onChecked?: () => void }) {
  const toast = useToast();
  const [result, setResult] = useState<AccountCheckResult | null>(null);
  const [history, setHistory] = useState<AccountCheck[]>([]);
  const [busy, setBusy] = useState(false);
  const ready = device?.mode === "adb" && device.identified;

  useEffect(() => {
    setResult(null);
    setHistory([]);
    if (device && ready) api.accounts.history(device.id).then(setHistory).catch(() => setHistory([]));
  }, [device?.id, ready]);

  async function check() {
    if (!device) return;
    setBusy(true);
    try {
      const r = await api.accounts.check(device.id);
      setResult(r);
      setHistory(await api.accounts.history(device.id));
      onChecked?.();
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel title="Cuentas en el equipo" description="Solo tipos y cantidades: AndroidMasters nunca lee ni guarda correos ni contraseñas."
           actions={<Button size="sm" variant="primary" icon={ScanSearch} loading={busy} disabled={!ready} onClick={check}>Comprobar cuentas en el equipo</Button>}>
      {!device && <p className="text-[13px] text-ink-400">Conecte el teléfono por USB para comprobar sus cuentas.</p>}
      {device && !ready && <p className="text-[13px] text-ink-400">Requiere el equipo encendido en Android con la depuración USB autorizada.</p>}
      {result && (
        <div className="space-y-3">
          {result.removal_verified && (
            <Notice tone="ok" title="Retiro de cuentas verificado">
              La comprobación anterior encontró cuentas con protección; esta ya no encuentra ninguna.
            </Notice>
          )}
          {result.check.summary.types.length === 0 ? (
            <EmptyState icon={UserRoundCheck} title="No hay cuentas con sesión iniciada" />
          ) : (
            <ul className="divide-y divide-ink-800">
              {result.check.summary.types.map((t) => (
                <li key={t.label} className="flex items-center justify-between py-2 text-[13px]">
                  <span className="text-ink-100">{t.label}</span>
                  <span className="flex items-center gap-2">
                    {t.protects && <Badge tone="warn">Protección antirrobo</Badge>}
                    <span className="tabular text-ink-300">{t.count}</span>
                  </span>
                </li>
              ))}
            </ul>
          )}
          <p className="flex items-center gap-2 text-xs text-ink-400">Detectado en el equipo <Badge tone="info">Diagnóstico</Badge></p>
        </div>
      )}
      {history.length > 0 && (
        <div className="mt-4 border-t border-ink-800 pt-3">
          <p className="mb-1.5 text-xs text-ink-400">Comprobaciones anteriores</p>
          <ul className="space-y-1 text-xs text-ink-300">
            {history.slice(0, 5).map((h) => (
              <li key={h.id} className="flex justify-between gap-3">
                <span>{formatDateTime(h.created_at)}{h.author ? ` · ${h.author}` : ""}</span>
                <span className="tabular">Google {h.google_count} · con protección {h.protected_count}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Panel>
  );
}
