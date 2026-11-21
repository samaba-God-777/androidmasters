const dateTime = new Intl.DateTimeFormat("es", { dateStyle: "medium", timeStyle: "short" });
const shortDay = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric" });
const relative = new Intl.RelativeTimeFormat("es", { numeric: "auto" });

/** Backend timestamps are local time "YYYY-MM-DDTHH:MM:SS". */
export function parseLocal(value: string): Date {
  return new Date(value.length === 10 ? `${value}T00:00:00` : value);
}

export function formatDateTime(value: string | null | undefined): string {
  return value ? dateTime.format(parseLocal(value)) : "—";
}

export function formatDay(value: string): string {
  return shortDay.format(parseLocal(value));
}

export function formatRelative(value: string, now: Date = new Date()): string {
  const seconds = Math.round((parseLocal(value).getTime() - now.getTime()) / 1000);
  const abs = Math.abs(seconds);
  if (abs < 45) return "ahora";
  if (abs < 3600) return relative.format(Math.round(seconds / 60), "minute");
  if (abs < 86400) return relative.format(Math.round(seconds / 3600), "hour");
  if (abs < 86400 * 7) return relative.format(Math.round(seconds / 86400), "day");
  return formatDateTime(value);
}
