import type { ButtonHTMLAttributes, ReactNode } from "react";
import { LoaderCircle, type LucideIcon } from "lucide-react";
import { cn } from "@/utils/cn";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-brand-500 text-ink-950 hover:bg-brand-400 border-transparent font-semibold",
  secondary: "bg-ink-800 text-ink-100 border-ink-700 hover:bg-ink-750 hover:border-ink-600",
  ghost: "bg-transparent text-ink-300 border-transparent hover:bg-ink-800 hover:text-ink-100",
  danger: "bg-danger/10 text-danger border-danger/40 hover:bg-danger/20 font-semibold",
};
const SIZES: Record<Size, string> = { sm: "h-8 px-3 text-[13px] gap-1.5", md: "h-9 px-4 text-sm gap-2" };

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: LucideIcon;
  loading?: boolean;
  children?: ReactNode;
}

export function Button({ variant = "secondary", size = "md", icon: Icon, loading, disabled, className, children, type = "button", ...rest }: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cn(
        "pressable inline-flex shrink-0 items-center justify-center rounded-lg border whitespace-nowrap",
        "disabled:cursor-not-allowed disabled:opacity-45",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      {loading ? <LoaderCircle className="size-4 animate-spin" aria-hidden /> : Icon && <Icon className="size-4" aria-hidden />}
      {children}
    </button>
  );
}
