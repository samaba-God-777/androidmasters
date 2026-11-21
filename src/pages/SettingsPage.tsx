import { useState } from "react";
import { DatabaseBackup, FolderCog, KeyRound, UserPlus, Users } from "lucide-react";
import { Button } from "@/components/Button";
import { ConfirmDialog, Dialog } from "@/components/Dialog";
import { PasswordInput, Select, TextInput } from "@/components/Field";
import { Badge, EmptyState, Notice, Spinner } from "@/components/Feedback";
import { PageHeader, Panel } from "@/components/Surface";
import { Table } from "@/components/Table";
import { useToast } from "@/components/Toaster";
import { useAsync } from "@/hooks/useAsync";
import { useCurrentUser } from "@/hooks/useSession";
import { api, type ApiError } from "@/services/api";
import type { Role, User } from "@/types/api";
import { formatDateTime } from "@/utils/format";
import { ROLE_LABEL } from "@/utils/labels";
import { MIN_PASSWORD } from "@/pages/AccessPage";

function ChangePassword() {
  const toast = useToast();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function save() {
    if (next.length < MIN_PASSWORD) return setError(`Mínimo ${MIN_PASSWORD} caracteres.`);
    setBusy(true);
    try {
      await api.changePassword(current, next);
      setCurrent("");
      setNext("");
      setError(null);
      toast("Contraseña actualizada", "ok");
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel title="Mi cuenta" description="Cambie su contraseña de acceso a AndroidMasters.">
      <div className="grid max-w-xl gap-4 sm:grid-cols-2">
        <PasswordInput label="Contraseña actual" autoComplete="current-password" value={current} onChange={(e) => setCurrent(e.target.value)} />
        <PasswordInput label="Contraseña nueva" autoComplete="new-password" error={error} value={next} onChange={(e) => setNext(e.target.value)} />
      </div>
      <Button className="mt-4" icon={KeyRound} loading={busy} disabled={!current || !next} onClick={save}>
        Cambiar contraseña
      </Button>
    </Panel>
  );
}

function CreateUserDialog({ open, onClose, onCreated }: { open: boolean; onClose: () => void; onCreated: () => void }) {
  const toast = useToast();
  const [form, setForm] = useState({ username: "", name: "", password: "", role: "technician" as Role });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  async function create() {
    setBusy(true);
    try {
      await api.users.create(form.username, form.name, form.password, form.role);
      toast(`Cuenta ${form.username} creada`, "ok");
      setForm({ username: "", name: "", password: "", role: "technician" });
      setError(null);
      onCreated();
      onClose();
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog open={open} onClose={onClose} title="Nueva cuenta"
            description="Entregue la contraseña inicial a la persona en persona; podrá cambiarla en Configuración."
            footer={<><Button variant="ghost" onClick={onClose}>Cancelar</Button><Button variant="primary" loading={busy} onClick={create}>Crear cuenta</Button></>}>
      <div className="space-y-4">
        <TextInput label="Usuario" autoCapitalize="none" spellCheck={false} value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} />
        <TextInput label="Nombre completo" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
        <PasswordInput label="Contraseña inicial" hint={`Mínimo ${MIN_PASSWORD} caracteres.`} autoComplete="new-password" value={form.password}
                       onChange={(e) => setForm({ ...form, password: e.target.value })} />
        <Select label="Rol" value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value as Role })}
                options={[{ value: "technician", label: "Técnico — trabajo de servicio" }, { value: "admin", label: "Administrador — también gestiona el laboratorio" }]} />
        {error && <Notice tone="danger">{error}</Notice>}
      </div>
    </Dialog>
  );
}

function ResetPasswordDialog({ user, onClose }: { user: User | null; onClose: () => void }) {
  const toast = useToast();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  async function save() {
    if (!user) return;
    setBusy(true);
    try {
      await api.users.resetPassword(user.id, password);
      toast(`Contraseña de ${user.username} restablecida`, "ok");
      setPassword("");
      onClose();
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog open={user !== null} onClose={onClose} title={`Restablecer contraseña de ${user?.display_name ?? ""}`}
            footer={<><Button variant="ghost" onClick={onClose}>Cancelar</Button><Button variant="primary" loading={busy} onClick={save}>Guardar</Button></>}>
      <PasswordInput label="Contraseña nueva" hint={`Mínimo ${MIN_PASSWORD} caracteres.`} error={error} autoComplete="new-password"
                     value={password} onChange={(e) => setPassword(e.target.value)} />
    </Dialog>
  );
}

function UsersPanel({ me }: { me: User }) {
  const toast = useToast();
  const { data, error, loading, reload } = useAsync(api.users.list);
  const [creating, setCreating] = useState(false);
  const [resetting, setResetting] = useState<User | null>(null);
  const [toggling, setToggling] = useState<User | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(action: () => Promise<unknown>, message: string) {
    setBusy(true);
    try {
      await action();
      toast(message, "ok");
      await reload();
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
      setToggling(null);
    }
  }

  return (
    <Panel title="Usuarios y roles" description="Los técnicos realizan el trabajo de servicio; los administradores también gestionan el laboratorio."
           actions={<Button size="sm" variant="primary" icon={UserPlus} onClick={() => setCreating(true)}>Nueva cuenta</Button>}>
      {loading && !data && <Spinner />}
      {error && <Notice tone="danger">{error.message}</Notice>}
      {data && (
        <Table<User>
          rows={data}
          rowKey={(u) => u.id}
          empty={<EmptyState icon={Users} title="Sin cuentas" />}
          columns={[
            { key: "name", header: "Persona", render: (u) => (<div><p className="text-ink-100">{u.display_name}</p><p className="text-xs text-ink-500">@{u.username}</p></div>) },
            { key: "role", header: "Rol", render: (u) => (
              <select aria-label={`Rol de ${u.display_name}`} value={u.role} disabled={busy}
                      onChange={(e) => run(() => api.users.setRole(u.id, e.target.value as Role), `Rol de ${u.username} actualizado`)}
                      className="h-8 rounded-md border border-ink-700 bg-ink-925 px-2 text-[13px] text-ink-100">
                <option value="technician">{ROLE_LABEL.technician}</option>
                <option value="admin">{ROLE_LABEL.admin}</option>
              </select>) },
            { key: "status", header: "Estado", render: (u) => (u.active ? <Badge tone="ok">Activa</Badge> : <Badge>Desactivada</Badge>) },
            { key: "last", header: "Último acceso", render: (u) => <span className="text-ink-400">{formatDateTime(u.last_login)}</span> },
            { key: "actions", header: "", className: "text-right", render: (u) => (
              <div className="flex justify-end gap-1.5">
                <Button size="sm" variant="ghost" onClick={() => setResetting(u)}>Contraseña</Button>
                {u.id !== me.id && (
                  <Button size="sm" variant={u.active ? "danger" : "secondary"} onClick={() => (u.active ? setToggling(u) : run(() => api.users.setActive(u.id, true), `Cuenta ${u.username} activada`))}>
                    {u.active ? "Desactivar" : "Activar"}
                  </Button>
                )}
              </div>) },
          ]}
        />
      )}
      <CreateUserDialog open={creating} onClose={() => setCreating(false)} onCreated={() => void reload()} />
      <ResetPasswordDialog user={resetting} onClose={() => setResetting(null)} />
      <ConfirmDialog open={toggling !== null} onClose={() => setToggling(null)} busy={busy} destructive confirmLabel="Desactivar cuenta"
                     title={`Desactivar a ${toggling?.display_name ?? ""}`}
                     onConfirm={() => toggling && run(() => api.users.setActive(toggling.id, false), `Cuenta ${toggling.username} desactivada`)}>
        <p>La persona no podrá iniciar sesión. Su historial se conserva y puede volver a activarse.</p>
      </ConfirmDialog>
    </Panel>
  );
}

function BackupPanel() {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const [last, setLast] = useState<string | null>(null);
  async function backup() {
    setBusy(true);
    try {
      const result = await api.createBackup();
      setLast(result.path);
      toast("Copia de seguridad creada y verificada", "ok");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel title="Copias de seguridad" description="Copia consistente de la base de datos, verificada con una comprobación de integridad.">
      <Button icon={DatabaseBackup} loading={busy} onClick={backup}>Crear copia ahora</Button>
      {last && <p className="mt-3 text-xs text-ink-400" data-selectable>Guardada en {last}</p>}
      <Notice tone="warn" className="mt-4">
        Guarde también el archivo <code className="font-mono">secret.key</code> de la carpeta de datos en un lugar seguro: sin él no se pueden leer los números de serie cifrados.
      </Notice>
    </Panel>
  );
}

function ToolsPanel() {
  const toast = useToast();
  const { data, error, reload } = useAsync(api.tools.get);
  const [dir, setDir] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const value = dir ?? data?.platform_tools_dir ?? "";
  async function save(next: string | null) {
    setBusy(true);
    try {
      await api.tools.set(next);
      setDir(null);
      await reload();
      toast(next ? "Carpeta de Platform-Tools guardada" : "Se usará la detección automática", "ok");
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel title="Android SDK Platform-Tools" description="Carpeta que contiene adb y fastboot. Vacía = detección automática (SDK, Homebrew, winget o PATH).">
      {error && <Notice tone="danger">{error.message}</Notice>}
      <div className="flex max-w-2xl items-end gap-2">
        <div className="flex-1">
          <TextInput label="Carpeta platform-tools" placeholder="Detección automática" spellCheck={false} value={value} onChange={(e) => setDir(e.target.value)} />
        </div>
        <Button icon={FolderCog} loading={busy} disabled={!value.trim()} onClick={() => save(value)}>Guardar</Button>
        {data?.platform_tools_dir && <Button variant="ghost" disabled={busy} onClick={() => save(null)}>Automática</Button>}
      </div>
      {data && (
        <dl className="mt-4 grid grid-cols-[90px_1fr] gap-x-6 gap-y-1.5 text-[13px]">
          <dt className="text-ink-400">adb</dt>
          <dd data-selectable>{data.adb_path ?? <span className="text-danger">No encontrado</span>}</dd>
          <dt className="text-ink-400">fastboot</dt>
          <dd data-selectable>{data.fastboot_path ?? <span className="text-ink-500">No encontrado</span>}</dd>
        </dl>
      )}
    </Panel>
  );
}

function SystemPanel() {
  const { data, error } = useAsync(api.appInfo);
  return (
    <Panel title="Sistema">
      {error && <Notice tone="danger">{error.message}</Notice>}
      {data ? (
        <dl className="grid max-w-md grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-[13px]">
          <dt className="text-ink-400">Versión</dt><dd>{data.version}</dd>
          <dt className="text-ink-400">Plataforma</dt><dd>{data.platform} · {data.arch}</dd>
          <dt className="text-ink-400">Base de datos</dt><dd>esquema v{data.schema_version} · {data.database_ok ? "íntegra" : "con errores"}</dd>
        </dl>
      ) : !error && <Spinner />}
    </Panel>
  );
}

export function SettingsPage() {
  const me = useCurrentUser();
  const admin = me.role === "admin";
  return (
    <>
      <PageHeader title="Configuración" description={admin ? "Cuenta, usuarios, copias de seguridad y sistema." : "Su cuenta y la información del sistema."} />
      <div className="space-y-5">
        <ChangePassword />
        {admin && <UsersPanel me={me} />}
        {admin && <ToolsPanel />}
        {admin && <BackupPanel />}
        <SystemPanel />
      </div>
    </>
  );
}
