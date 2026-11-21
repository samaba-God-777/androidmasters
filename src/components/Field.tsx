import { useId, useState, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes, type TextareaHTMLAttributes } from "react";
import { Eye, EyeOff } from "lucide-react";
import { cn } from "@/utils/cn";

const CONTROL =
  "h-9 w-full rounded-lg border border-ink-700 bg-ink-925 px-3 text-sm text-ink-100 placeholder:text-ink-500 " +
  "transition-[border-color] duration-150 hover:border-ink-600 focus:border-brand-500 focus:outline-none " +
  "aria-[invalid=true]:border-danger/70 disabled:opacity-50";

interface FieldProps {
  label: string;
  hint?: ReactNode;
  error?: string | null;
  children: (id: string, describedBy: string | undefined) => ReactNode;
}

export function Field({ label, hint, error, children }: FieldProps) {
  const id = useId();
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [hintId, errorId].filter(Boolean).join(" ") || undefined;
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-[13px] font-medium text-ink-200">
        {label}
      </label>
      {children(id, describedBy)}
      {hint && !error && (
        <p id={hintId} className="text-xs text-ink-400">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} role="alert" className="text-xs text-danger">
          {error}
        </p>
      )}
    </div>
  );
}

export interface TextInputProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  hint?: ReactNode;
  error?: string | null;
}

export function TextInput({ label, hint, error, className, ...rest }: TextInputProps) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(id, describedBy) => (
        <input id={id} aria-describedby={describedBy} aria-invalid={Boolean(error) || undefined} className={cn(CONTROL, className)} {...rest} />
      )}
    </Field>
  );
}

export function PasswordInput({ label, hint, error, className, ...rest }: TextInputProps) {
  const [visible, setVisible] = useState(false);
  return (
    <Field label={label} hint={hint} error={error}>
      {(id, describedBy) => (
        <div className="relative">
          <input
            id={id}
            type={visible ? "text" : "password"}
            aria-describedby={describedBy}
            aria-invalid={Boolean(error) || undefined}
            className={cn(CONTROL, "pr-10", className)}
            {...rest}
          />
          <button
            type="button"
            onClick={() => setVisible((v) => !v)}
            aria-label={visible ? "Ocultar contraseña" : "Mostrar contraseña"}
            className="absolute inset-y-0 right-0 flex w-10 items-center justify-center text-ink-400 hover:text-ink-100"
          >
            {visible ? <EyeOff className="size-4" aria-hidden /> : <Eye className="size-4" aria-hidden />}
          </button>
        </div>
      )}
    </Field>
  );
}

export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label: string;
  hint?: ReactNode;
  options: Array<{ value: string; label: string }>;
}

export function Select({ label, hint, options, className, ...rest }: SelectProps) {
  return (
    <Field label={label} hint={hint}>
      {(id, describedBy) => (
        <select id={id} aria-describedby={describedBy} className={cn(CONTROL, "pr-8", className)} {...rest}>
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      )}
    </Field>
  );
}

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label: string;
  hint?: ReactNode;
  error?: string | null;
}

export function TextArea({ label, hint, error, className, rows = 3, ...rest }: TextAreaProps) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(id, describedBy) => (
        <textarea id={id} rows={rows} aria-describedby={describedBy} aria-invalid={Boolean(error) || undefined}
                  className={cn(CONTROL, "h-auto resize-y py-2 leading-relaxed", className)} {...rest} />
      )}
    </Field>
  );
}
