import { useEffect, useState } from "react";
import { DEFAULT_ROUTE, isRouteId, type RouteId } from "@/modules/navigation";

function read(): RouteId {
  const id = window.location.hash.replace(/^#\/?/, "");
  return isRouteId(id) ? id : DEFAULT_ROUTE;
}

/** Minimal hash router: the current page survives reloads and works with the back button. */
export function useHashRoute(): [RouteId, (id: RouteId) => void] {
  const [route, setRoute] = useState<RouteId>(read);
  useEffect(() => {
    const onChange = () => setRoute(read());
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);
  return [route, (id) => (window.location.hash = `/${id}`)];
}
