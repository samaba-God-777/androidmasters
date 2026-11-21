import type { FrpObservation, FrpOutcome, ResetOrigin } from "@/types/api";
import type { Tone } from "@/components/Feedback";

/** Official Google resources (opened by the owner in their own browser). */
export const OFFICIAL_LINKS = [
  { label: "Recuperación de cuentas de Google", url: "https://accounts.google.com/signin/recovery" },
  { label: "Ayuda de Google: restablecer un dispositivo Android", url: "https://support.google.com/android/answer/6088915" },
] as const;

export const FRP_SUMMARY =
  "Factory Reset Protection (FRP) es una protección antirrobo de Android. Si un equipo se restablece mientras tiene una cuenta Google, " +
  "al configurarlo de nuevo pide esa misma cuenta. Solo el titular puede superarla; el fabricante puede ayudar al propietario verificado.";

export const EMPTY_OBSERVATION: FrpObservation = {
  asks_previous_account: null,
  owner_or_authorized: null,
  reset_origin: null,
  second_hand: null,
  password_changed_recently: null,
  knows_credentials: null,
  can_recover_account: null,
  has_purchase_proof: null,
};

type BoolKey = Exclude<keyof FrpObservation, "reset_origin">;

export interface Question {
  key: BoolKey;
  text: string;
  hint?: string;
  /** Shown only when this returns true. */
  when?: (o: FrpObservation) => boolean;
}

export const QUESTIONS: Question[] = [
  { key: "owner_or_authorized", text: "¿La persona es el propietario del equipo o está autorizada por él?", hint: "Compruebe identificación y, si es posible, comprobante de compra." },
  { key: "asks_previous_account", text: "¿El equipo pide la cuenta Google que estaba sincronizada antes?", hint: "Pantalla «Verificar tu cuenta» / «Este dispositivo se restableció» durante la configuración." },
  { key: "second_hand", text: "¿Es un equipo de segunda mano?", when: (o) => o.asks_previous_account === true },
  { key: "knows_credentials", text: "¿El propietario conoce el correo y la contraseña de esa cuenta?", when: (o) => o.asks_previous_account === true },
  { key: "password_changed_recently", text: "¿La contraseña de la cuenta cambió en los últimos días?", when: (o) => o.asks_previous_account === true },
  { key: "can_recover_account", text: "¿Puede recuperar la cuenta con su teléfono o correo de recuperación?", when: (o) => o.knows_credentials === false },
  { key: "has_purchase_proof", text: "¿Tiene comprobante de compra a su nombre?", when: (o) => o.knows_credentials === false && o.can_recover_account === false },
];

export const RESET_ORIGINS: Array<{ value: ResetOrigin; label: string }> = [
  { value: "not_reset", label: "No se ha restablecido" },
  { value: "settings", label: "Desde Ajustes del teléfono" },
  { value: "recovery_or_buttons", label: "Desde Recovery o con los botones" },
  { value: "unknown", label: "No se sabe" },
];

export const OUTCOME: Record<FrpOutcome, { label: string; tone: Tone }> = {
  incomplete: { label: "Faltan datos", tone: "neutral" },
  no_indication: { label: "Sin indicios de FRP", tone: "ok" },
  stop_no_authorization: { label: "Detenido: sin autorización", tone: "danger" },
  owner_signs_in: { label: "Inicio de sesión del propietario", tone: "brand" },
  wait_security_period: { label: "Periodo de seguridad", tone: "warn" },
  account_recovery: { label: "Recuperación oficial de la cuenta", tone: "info" },
  manufacturer_support: { label: "Soporte del fabricante", tone: "warn" },
};
