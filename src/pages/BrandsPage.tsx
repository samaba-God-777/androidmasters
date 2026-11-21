import { useEffect, useMemo, useState } from "react";
import { Search, Smartphone } from "lucide-react";
import { CopyLink } from "@/components/CopyLink";
import { Badge, Notice, Spinner } from "@/components/Feedback";
import { ProcedureCard } from "@/components/ProcedureCard";
import { PageHeader, Panel } from "@/components/Surface";
import { useAsync } from "@/hooks/useAsync";
import { useDeviceChoice } from "@/hooks/useDeviceChoice";
import { useCurrentUser } from "@/hooks/useSession";
import { api } from "@/services/api";
import type { BrandModule } from "@/types/api";
import { deviceName } from "@/utils/labels";
import { cn } from "@/utils/cn";

const BRAND_ORDER = ["samsung", "xiaomi", "motorola", "huawei", "oppo", "realme", "vivo", "tecno", "infinix", "pixel"];

export function BrandsPage() {
  const me = useCurrentUser();
  const { device } = useDeviceChoice();
  const catalog = useAsync(() => api.procedures.catalog(device?.id ?? null), [device?.id]);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);

  const brands = useMemo(
    () => (catalog.data?.brands ?? []).filter((b) => b.brand !== "google").sort((a, b) => BRAND_ORDER.indexOf(a.brand) - BRAND_ORDER.indexOf(b.brand)),
    [catalog.data],
  );
  const deviceBrand = catalog.data?.device_brand ?? null;
  useEffect(() => {
    if (!selected && deviceBrand) setSelected(deviceBrand);
  }, [deviceBrand, selected]);
  const q = query.trim().toLowerCase();
  const visible = brands.filter((b) => !q || b.name.toLowerCase().includes(q) || b.brand.includes(q));
  const current: BrandModule | undefined = brands.find((b) => b.brand === (selected ?? deviceBrand)) ?? visible[0];
  const pendingReview = (b: BrandModule) => b.procedures.filter((p) => !p.review?.current).length;

  return (
    <>
      <PageHeader title="Fabricantes" description="Procedimientos oficiales y versionados de cada marca para cuentas, búsqueda del dispositivo y transferencia." />
      {catalog.error && <Notice tone="danger">{catalog.error.message}</Notice>}
      {!catalog.data && !catalog.error && <Spinner />}
      {catalog.data && (
        <div className="grid gap-5 lg:grid-cols-[260px_1fr]">
          <nav aria-label="Fabricantes" className="space-y-2">
            <label className="relative block">
              <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-ink-500" aria-hidden />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar marca…" aria-label="Buscar marca"
                     className="h-9 w-full rounded-lg border border-ink-700 bg-ink-925 pr-3 pl-9 text-sm text-ink-100 placeholder:text-ink-500 focus:border-brand-500 focus:outline-none" />
            </label>
            <ul className="space-y-0.5">
              {visible.map((b) => (
                <li key={b.brand}>
                  <button type="button" onClick={() => setSelected(b.brand)} aria-current={current?.brand === b.brand ? "true" : undefined}
                          className={cn("flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left text-[13px]",
                                        current?.brand === b.brand ? "bg-ink-800 text-ink-100" : "text-ink-300 hover:bg-ink-850 hover:text-ink-100")}>
                    <span className="flex items-center gap-2">
                      {b.name}
                      {b.brand === deviceBrand && <Smartphone className="size-3.5 text-brand-400" aria-label="Equipo conectado" />}
                    </span>
                    {pendingReview(b) > 0 && <span className="text-[11px] text-warn">{pendingReview(b)} sin revisar</span>}
                  </button>
                </li>
              ))}
              {visible.length === 0 && <li className="px-3 py-2 text-[13px] text-ink-500">Sin resultados</li>}
            </ul>
          </nav>
          {current && (
            <div className="space-y-4">
              <Panel title={current.name} description={<>Biblioteca actualizada el {current.updated}{device && current.brand === deviceBrand && <> · <Badge tone="brand">{deviceName(device)} conectado</Badge></>}</>}>
                <div className="grid gap-2 sm:grid-cols-2">
                  {current.support_links.map((l) => <CopyLink key={l.url} label={l.label} url={l.url} />)}
                </div>
              </Panel>
              {current.procedures.map((p, i) => (
                <ProcedureCard key={p.id} procedure={p} androidVersion={current.brand === deviceBrand ? device?.android_version : null}
                               canReview={me.role === "admin"} onReviewed={() => void catalog.reload()} defaultOpen={i === 1} />
              ))}
            </div>
          )}
        </div>
      )}
    </>
  );
}
