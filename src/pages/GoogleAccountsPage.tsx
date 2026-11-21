import { useState } from "react";
import { Select, TextInput } from "@/components/Field";
import { Notice, Spinner } from "@/components/Feedback";
import { AccountCheckPanel } from "@/components/AccountCheckPanel";
import { ProcedureCard } from "@/components/ProcedureCard";
import { PageHeader } from "@/components/Surface";
import { TransferChecklist } from "@/components/TransferChecklist";
import { useAsync } from "@/hooks/useAsync";
import { MANUAL_DEVICE, useDeviceChoice } from "@/hooks/useDeviceChoice";
import { useCurrentUser } from "@/hooks/useSession";
import { api } from "@/services/api";
import { MODE, deviceName } from "@/utils/labels";

export function GoogleAccountsPage() {
  const me = useCurrentUser();
  const { devices, selected, setChoice, device, manualLabel, setManualLabel } = useDeviceChoice();
  const catalog = useAsync(() => api.procedures.catalog(null));
  const [checks, setChecks] = useState(0);
  const google = catalog.data?.brands.find((b) => b.brand === "google");

  return (
    <>
      <PageHeader title="Cuentas Google" description="Retiro de la cuenta desde Ajustes, recuperación de acceso y preparación para un nuevo propietario, con verificación posterior." />
      <div className="mb-5 grid max-w-3xl gap-4 sm:grid-cols-2">
        <Select label="Equipo" value={selected} onChange={(e) => setChoice(e.target.value)}
                options={[...devices.map((d) => ({ value: d.id, label: `${deviceName(d)} · ${MODE[d.mode].label}` })), { value: MANUAL_DEVICE, label: "Equipo no conectado (escribir marca y modelo)" }]} />
        {!device && <TextInput label="Marca y modelo" placeholder="Ej. Samsung Galaxy A14" value={manualLabel} onChange={(e) => setManualLabel(e.target.value)} />}
      </div>
      <div className="grid gap-5 xl:grid-cols-[1fr_420px]">
        <div className="space-y-4">
          <AccountCheckPanel device={device} onChecked={() => setChecks((n) => n + 1)} />
          {catalog.error && <Notice tone="danger">{catalog.error.message}</Notice>}
          {!catalog.data && !catalog.error && <Spinner />}
          {google?.procedures.map((p, i) => (
            <ProcedureCard key={p.id} procedure={p} androidVersion={device?.android_version} canReview={me.role === "admin"}
                           onReviewed={() => void catalog.reload()} defaultOpen={i === 0} />
          ))}
        </div>
        <div>
          <TransferChecklist deviceId={device?.id ?? null} deviceLabel={device ? "" : manualLabel} refreshKey={checks} />
        </div>
      </div>
    </>
  );
}
