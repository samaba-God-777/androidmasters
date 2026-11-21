
import { Sidebar } from "@/components/Sidebar";
import { Topbar } from "@/components/Topbar";
import { Notice, Spinner } from "@/components/Feedback";
import { Toaster, useToast } from "@/components/Toaster";
import { SessionProvider, useSession } from "@/hooks/useSession";
import { DevicesProvider } from "@/hooks/useDevices";
import { useHashRoute } from "@/hooks/useHashRoute";
import { CURRENT_PHASE, routeById, type RouteId } from "@/modules/navigation";
import { AccessPage } from "@/pages/AccessPage";
import { DashboardPage } from "@/pages/DashboardPage";
import { DevicesPage } from "@/pages/DevicesPage";
import { UsbPage } from "@/pages/UsbPage";
import { FrpPage } from "@/pages/FrpPage";
import { GoogleAccountsPage } from "@/pages/GoogleAccountsPage";
import { BrandsPage } from "@/pages/BrandsPage";
import { ResetPage } from "@/pages/ResetPage";
import { OrdersPage } from "@/pages/OrdersPage";
import { ReportsPage } from "@/pages/ReportsPage";
import { ModulePendingPage } from "@/pages/ModulePendingPage";
import { SettingsPage } from "@/pages/SettingsPage";
import type { ApiError } from "@/services/api";

function Page({ id, navigate }: { id: RouteId; navigate: (id: RouteId) => void }) {
  const route = routeById(id);
  if (route.phase > CURRENT_PHASE) return <ModulePendingPage route={route} />;
  switch (id) {
    case "dashboard":
      return <DashboardPage />;
    case "devices":
      return <DevicesPage onNavigate={navigate} />;
    case "usb":
      return <UsbPage />;
    case "frp":
      return <FrpPage />;
    case "google":
      return <GoogleAccountsPage />;
    case "brands":
      return <BrandsPage />;
    case "reset":
      return <ResetPage onNavigate={navigate} />;
    case "orders":
      return <OrdersPage />;
    case "reports":
      return <ReportsPage />;
    case "settings":
      return <SettingsPage />;
    default:
      return <ModulePendingPage route={route} />;
  }
}

function Shell() {
  const { state, signOut } = useSession();
  const toast = useToast();
  const [route, navigate] = useHashRoute();

  switch (state.status) {
    case "loading":
      return <div className="flex h-full items-center justify-center"><Spinner label="Iniciando…" /></div>;
    case "unavailable":
      return (
        <main className="flex h-full items-center justify-center p-6">
          <div className="max-w-md">
            <Notice tone="warn" title="Backend no disponible">{state.error.message}</Notice>
          </div>
        </main>
      );
    case "setup":
      return <AccessPage mode="setup" />;
    case "signed_out":
      return <AccessPage mode="login" notice={state.notice} />;
    case "signed_in":
      return (
        <DevicesProvider>
          <div className="flex h-full">
            <Sidebar current={route} onNavigate={navigate} />
            <div className="flex min-w-0 flex-1 flex-col">
              <Topbar user={state.user} onOpenDevices={() => navigate("devices")}
                      onSignOut={() => signOut().catch((e: ApiError) => toast(e.message, "error"))} />
              <main className="flex-1 overflow-y-auto px-8 py-7">
                <div className="mx-auto max-w-[1280px]">
                  <Page id={route} navigate={navigate} />
                </div>
              </main>
            </div>
          </div>
        </DevicesProvider>
      );
  }
}

export default function App() {
  return (
    <Toaster>
      <SessionProvider>
        <Shell />
      </SessionProvider>
    </Toaster>
  );
}

