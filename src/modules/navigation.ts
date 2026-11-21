import {
  Building2, ClipboardList, FileText, LayoutDashboard, RotateCcw, Settings, ShieldAlert, Smartphone, Usb, UserRoundCheck,
  type LucideIcon,
} from "lucide-react";

export interface RouteDef {
  id: string;
  label: string;
  icon: LucideIcon;
  group: "Laboratorio" | "Diagnóstico" | "Gestión" | "Sistema";
  /** Development phase in which the module becomes available (docs/PHASES.md). */
  phase: number;
  summary: string;
}

export const ROUTES = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, group: "Laboratorio", phase: 2,
    summary: "Estado del laboratorio con datos reales." },
  { id: "devices", label: "Dispositivos", icon: Smartphone, group: "Laboratorio", phase: 3,
    summary: "Detección por USB e identificación: fabricante, modelo, versión de Android, nivel de API, parche de seguridad y estado ADB." },
  { id: "usb", label: "Conexión USB", icon: Usb, group: "Diagnóstico", phase: 3,
    summary: "Dispositivos ADB autorizados, estado de depuración USB, desconexiones, errores y diagnóstico técnico de la conexión." },
  { id: "frp", label: "Diagnóstico FRP", icon: ShieldAlert, group: "Diagnóstico", phase: 4,
    summary: "Registro de indicios de Factory Reset Protection, causas posibles y orientación hacia la recuperación oficial de Google. Sin bypass." },
  { id: "google", label: "Cuentas Google", icon: UserRoundCheck, group: "Gestión", phase: 5,
    summary: "Guías oficiales para retirar la cuenta desde Ajustes, recuperar el acceso y preparar la transferencia, con verificación posterior." },
  { id: "brands", label: "Fabricantes", icon: Building2, group: "Gestión", phase: 5,
    summary: "Procedimientos oficiales versionados para Samsung, Xiaomi, Motorola, Huawei, OPPO, Realme, Vivo, Tecno, Infinix y Google Pixel." },
  { id: "reset", label: "Restablecimiento", icon: RotateCcw, group: "Gestión", phase: 6,
    summary: "Restablecimiento autorizado: autorización, retiro de cuentas, confirmación explícita y verificación posterior." },
  { id: "orders", label: "Órdenes de servicio", icon: ClipboardList, group: "Gestión", phase: 7,
    summary: "Órdenes de trabajo con cliente, equipo, problema, diagnóstico, técnico, estado, fechas y observaciones." },
  { id: "reports", label: "Reportes", icon: FileText, group: "Gestión", phase: 7,
    summary: "Resumen del periodo, reportes PDF por orden y por periodo, y exportación de órdenes a CSV." },
  { id: "settings", label: "Configuración", icon: Settings, group: "Sistema", phase: 2,
    summary: "Cuenta, usuarios y roles, copias de seguridad e información del sistema." },
] as const satisfies readonly RouteDef[];

export type RouteId = (typeof ROUTES)[number]["id"];
export const DEFAULT_ROUTE: RouteId = "dashboard";
export const CURRENT_PHASE = 8;

export function isRouteId(value: string): value is RouteId {
  return ROUTES.some((r) => r.id === value);
}

export function routeById(id: RouteId): RouteDef {
  return ROUTES.find((r) => r.id === id)!;
}
