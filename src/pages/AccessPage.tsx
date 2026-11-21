import { useState, type FormEvent } from "react";
import { ShieldCheck } from "lucide-react";
import { Button } from "@/components/Button";
import { PasswordInput, TextInput } from "@/components/Field";
import { Notice } from "@/components/Feedback";
import { useSession } from "@/hooks/useSession";
import type { ApiError } from "@/services/api";

export const MIN_PASSWORD = 10;

/** Client-side checks mirror the backend rules; the backend validates again. */
export function validateSetup(username: string, displayName: string, password: string, confirm: string): string | null {
  if (!/^[a-z0-9._-]{3,32}$/.test(username.trim().toLowerCase())) {
    return "Usuario: 3 a 32 caracteres en minúscula, números, punto, guion o guion bajo.";
  }
  if (!displayName.trim()) return "Escriba su nombre.";
  if (password.length < MIN_PASSWORD) return `La contraseña debe tener al menos ${MIN_PASSWORD} caracteres.`;
  if (/^\d+$/.test(password)) return "La contraseña no puede ser solo números.";
  if (password !== confirm) return "Las contraseñas no coinciden.";
  return null;
}

export function AccessPage({ mode, notice }: { mode: "setup" | "login"; notice?: string }) {
  const session = useSession();
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const problem = mode === "setup" ? validateSetup(username, displayName, password, confirm) : null;
    if (problem) return setError(problem);
    setBusy(true);
    setError(null);
    try {
      if (mode === "setup") await session.createFirstAdmin(username, displayName, password);
      else await session.signIn(username, password);
    } catch (e) {
      setError((e as ApiError).message);
      setPassword("");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="flex h-full items-center justify-center p-6">
      <form onSubmit={submit} noValidate className="w-full max-w-sm">
        <div className="mb-8 flex items-center gap-3">
          <span className="flex size-10 items-center justify-center rounded-xl bg-brand-500/15">
            <ShieldCheck className="size-5 text-brand-400" aria-hidden />
          </span>
          <div>
            <h1 className="text-lg font-semibold tracking-tight">AndroidMasters</h1>
            <p className="text-[13px] text-ink-400">{mode === "setup" ? "Configuración inicial" : "Iniciar sesión"}</p>
          </div>
        </div>
        {mode === "login" && notice && <Notice tone="info" className="mb-5">{notice}</Notice>}
        {mode === "setup" && (
          <Notice tone="info" className="mb-5" title="Cree la cuenta de administrador">
            Es la primera vez que se abre AndroidMasters en este equipo. El administrador gestiona usuarios, copias de
            seguridad y la revisión de procedimientos.
          </Notice>
        )}
        <div className="space-y-4">
          <TextInput label="Usuario" autoFocus autoComplete="username" autoCapitalize="none" spellCheck={false}
                     value={username} onChange={(e) => setUsername(e.target.value)} />
          {mode === "setup" && (
            <TextInput label="Nombre completo" autoComplete="name" value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
          )}
          <PasswordInput label="Contraseña" autoComplete={mode === "setup" ? "new-password" : "current-password"}
                         hint={mode === "setup" ? `Mínimo ${MIN_PASSWORD} caracteres.` : undefined}
                         value={password} onChange={(e) => setPassword(e.target.value)} />
          {mode === "setup" && (
            <PasswordInput label="Repita la contraseña" autoComplete="new-password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
          )}
        </div>
        {error && (
          <p role="alert" className="mt-4 text-[13px] text-danger">
            {error}
          </p>
        )}
        <Button type="submit" variant="primary" className="mt-6 w-full" loading={busy}>
          {mode === "setup" ? "Crear administrador y entrar" : "Entrar"}
        </Button>
      </form>
    </main>
  );
}
