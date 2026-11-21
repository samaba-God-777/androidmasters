import { useEffect, useState } from "react";
import { Eye, RefreshCw, Settings, Smartphone, Stethoscope } from "lucide-react";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/Dialog";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { PageHeader, Panel } from "@/components/Surface";
import { Table } from "@/components/Table";
import { useToast } from "@/components/Toaster";
import { useDevices } from "@/hooks/useDevices";
import { useCurrentUser } from "@/hooks/useSession";
import { api, type ApiError } from "@/services/api";
import type { DeviceView } from "@/types/api";
import { formatDateTime } from "@/utils/format";
import { MODE, deviceName, formatBytes } from "@/utils/labels";

const NOT_DETECTED = <span className="text-ink-500">No detectado</span>;

function value(v: string | number | null | undefined) {
  return v === null || v === undefined || v === "" ? NOT_DETECTED : String(v);
}

function DeviceDetail({ device, onDiagnose }: { device: DeviceView; onDiagnose: () => void }) {
  const toast = useToast();
  const me = useCurrentUser();
  const [busy, setBusy] = useState(false);
  const [askReveal, setAskReveal] = useState(false);
  const [serial, setSerial] = useState<string | null>(null);
  useEffect(() => setSerial(null), [device.id]);

  async function refresh() {
    setBusy(true);
    try {
      await api.devices.refresh(device.id);
      toast("Lectura actualizada", "ok");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }
  async function reveal() {
    try {
      setSerial(await api.devices.revealSerial(device.id));
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setAskReveal(false);
    }
  }

  const rows: Array<[string, React.ReactNode]> = [
    ["Fabricante", value(device.manufacturer)],
    ["Marca", value(device.brand)],
    ["Modelo", value(device.model)],
    ["Nombre del producto", value(device.product)],
    ["Código del dispositivo", value(device.codename)],
    ["Versión de Android", value(device.android_version)],
    ["Nivel de API", value(device.api_level)],
    ["Parche de seguridad", value(device.security_patch)],
    ["Compilación", value(device.build_id)],
    ["Bootloader", device.bootloader_locked === null ? NOT_DETECTED : device.bootloader_locked ? "Bloqueado" : "Desbloqueado"],
    ["Número de serie", serial ? <span data-selectable className="font-mono">{serial}</span> : <span className="font-mono">{device.serial_masked}</span>],
    ["Conexión", device.network ? "Red (TCP/IP)" : `USB${device.usb_path ? ` · puerto ${device.usb_path}` : ""}`],
    ["Batería", device.battery?.level != null ? `${device.battery.level}% · ${device.battery.status ?? "estado desconocido"} · ${device.battery.source ?? ""}` : NOT_DETECTED],
    ["Almacenamiento (/data)", device.storage ? `${formatBytes(device.storage.available_bytes)} libres de ${formatBytes(device.storage.total_bytes)}` : NOT_DETECTED],
    ["Detectado desde", formatDateTime(device.first_seen)],
  ];

  return (
    <Panel
      title={deviceName(device)}
      description={<Badge tone={MODE[device.mode].tone}>{MODE[device.mode].label}</Badge>}
      actions={
        <>
          <Button size="sm" icon={Stethoscope} onClick={onDiagnose}>Diagnosticar conexión</Button>
          <Button size="sm" icon={RefreshCw} loading={busy} disabled={device.mode !== "adb"} onClick={refresh}
                  title={device.mode !== "adb" ? "Requiere ADB autorizado" : undefined}>
            Actualizar lectura
          </Button>
        </>
      }
    >
      {device.mode === "unauthorized" && (
        <Notice tone="warn" className="mb-4" title="Depuración USB sin autorizar">
          Desbloquee el teléfono y acepte «¿Permitir depuración USB?». Hasta entonces Android no permite leer los datos del equipo.
        </Notice>
      )}
      {device.identify_error && <Notice tone="danger" className="mb-4">{device.identify_error}</Notice>}
      <dl className="grid grid-cols-[180px_1fr] gap-x-6 gap-y-2 text-[13px]">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-ink-400">{k}</dt>
            <dd className="text-ink-100">{v}</dd>
          </div>
        ))}
      </dl>
      {device.missing.length > 0 && device.mode === "adb" && (
        <p className="mt-4 text-xs text-ink-400">Este equipo no informa: {device.missing.join(", ")}. Se registrará manualmente en la orden de servicio.</p>
      )}
      {me.role === "admin" && !serial && (
        <Button size="sm" variant="ghost" icon={Eye} className="mt-4" onClick={() => setAskReveal(true)}>Mostrar número de serie completo</Button>
      )}
      <ConfirmDialog open={askReveal} onClose={() => setAskReveal(false)} onConfirm={reveal} title="Mostrar el número de serie" confirmLabel="Mostrar">
        <p>El número de serie es un identificador del equipo. Se mostrará solo en esta pantalla y el acceso quedará registrado en la auditoría.</p>
      </ConfirmDialog>
    </Panel>
  );
}

export function DevicesPage({ onNavigate }: { onNavigate: (route: "usb" | "settings") => void }) {
  const { snapshot, error } = useDevices();
  const [selected, setSelected] = useState<string | null>(null);
  const devices = snapshot?.devices ?? [];
  const current = devices.find((d) => d.id === selected) ?? devices[0] ?? null;

  return (
    <>
      <PageHeader title="Dispositivos" description="Teléfonos y tabletas detectados por USB en tiempo real. Solo se muestran datos que el equipo informa." />
      {!snapshot && !error && <Spinner />}
      {error && <Notice tone="danger">{error.message}</Notice>}
      {snapshot?.tools.last_error && (
        <Notice tone="warn" className="mb-5" title="La detección no está funcionando">
          {snapshot.tools.last_error}{" "}
          <button type="button" className="text-brand-300 underline-offset-2 hover:underline" onClick={() => onNavigate("settings")}>
            Configurar Platform-Tools
          </button>
        </Notice>
      )}
      {snapshot && devices.length === 0 && !snapshot.tools.last_error && (
        <Panel>
          <EmptyState icon={Smartphone} title="No hay dispositivos conectados">
            Conecte el teléfono con un cable USB de datos, active <strong>Depuración USB</strong> en Opciones de desarrollador y acepte el aviso de
            autorización en la pantalla del teléfono. La lista se actualiza sola.
          </EmptyState>
        </Panel>
      )}
      {devices.length > 0 && (
        <div className="grid gap-5 xl:grid-cols-[1fr_minmax(420px,1fr)]">
          <Panel title={`${devices.length} conectado${devices.length === 1 ? "" : "s"}`}>
            <Table<DeviceView>
              rows={devices}
              rowKey={(d) => d.id}
              columns={[
                { key: "name", header: "Dispositivo", render: (d) => (
                  <button type="button" onClick={() => setSelected(d.id)} aria-pressed={current?.id === d.id}
                          className={`text-left ${current?.id === d.id ? "text-brand-300" : "text-ink-100 hover:text-brand-300"}`}>
                    {deviceName(d)}
                    <span className="block font-mono text-xs text-ink-500">{d.serial_masked}</span>
                  </button>) },
                { key: "mode", header: "Estado", render: (d) => <Badge tone={MODE[d.mode].tone}>{MODE[d.mode].label}</Badge> },
                { key: "android", header: "Android", render: (d) => (d.android_version ? `${d.android_version} · API ${d.api_level ?? "—"}` : <span className="text-ink-500">—</span>) },
                { key: "battery", header: "Batería", className: "tabular", render: (d) => (d.battery?.level != null ? `${d.battery.level}%` : <span className="text-ink-500">—</span>) },
              ]}
            />
          </Panel>
          {current && <DeviceDetail device={current} onDiagnose={() => onNavigate("usb")} />}
        </div>
      )}
      {snapshot?.tools.adb_path && (
        <p className="mt-5 flex items-center gap-1.5 text-xs text-ink-500">
          <Settings className="size-3.5" aria-hidden /> adb {snapshot.tools.adb_version ?? ""} · {snapshot.tools.adb_path} · última revisión{" "}
          {snapshot.tools.last_scan ? formatDateTime(snapshot.tools.last_scan) : "—"}
        </p>
      )}
    </>
  );
}
