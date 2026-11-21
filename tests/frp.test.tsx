import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { FrpAssessment, FrpObservation } from "@/types/api";

const backend = vi.hoisted(() => ({ assess: vi.fn(), record: vi.fn(), history: vi.fn(), indicators: vi.fn() }));

vi.mock("@/hooks/useDevices", () => ({
  useDevices: () => ({ snapshot: { devices: [], tools: { adb_path: "/x/adb", adb_version: null, fastboot_path: null, last_scan: null, last_error: null }, events: [] }, error: null }),
}));
vi.mock("@/services/api", async (importOriginal) => {
  const real = await importOriginal<typeof import("@/services/api")>();
  return { ...real, api: { ...real.api, frp: { assess: backend.assess, record: backend.record, history: backend.history, indicators: backend.indicators } } };
});

import { FrpPage } from "@/pages/FrpPage";
import { Toaster } from "@/components/Toaster";

const base: FrpAssessment = { outcome: "incomplete", headline: "Faltan datos para recomendar un procedimiento.", possible_causes: [], next_steps: [], documentation: [], stop: false, pending_questions: ["¿La persona es el propietario?"] };

function renderPage() {
  return render(<Toaster><FrpPage /></Toaster>);
}

beforeEach(() => {
  vi.clearAllMocks();
  backend.history.mockResolvedValue([]);
  backend.assess.mockImplementation(async (o: FrpObservation): Promise<FrpAssessment> => {
    if (o.owner_or_authorized === false) {
      return { ...base, outcome: "stop_no_authorization", headline: "Detener: no se ha demostrado la propiedad ni la autorización.", stop: true, pending_questions: [], next_steps: ["No continúe con el servicio de este equipo."], documentation: ["Comprobante de compra"] };
    }
    if (o.knows_credentials === false) {
      return { ...base, outcome: "account_recovery", headline: "El propietario recupera su cuenta Google por el canal oficial.", pending_questions: [],
               possible_causes: ["Equipo de segunda mano"], next_steps: ["El propietario abre la recuperación oficial."] };
    }
    return base;
  });
});

describe("FRP diagnostic page", () => {
  it("explains FRP and refuses bypass up front", async () => {
    renderPage();
    expect(screen.getByText(/protección antirrobo de Android/)).toBeInTheDocument();
    expect(screen.getByText(/no incluye bypass/)).toBeInTheDocument();
    await screen.findByText("Faltan datos para recomendar un procedimiento.");
    expect(screen.getByRole("button", { name: "Registrar diagnóstico" })).toBeDisabled();
  });

  it("asks follow-up questions only when they apply", async () => {
    renderPage();
    expect(screen.queryByText(/conoce el correo y la contraseña/)).toBeNull();
    const asks = screen.getByRole("radiogroup", { name: /pide la cuenta Google/ });
    fireEvent.click(within(asks).getByRole("radio", { name: "Sí" }));
    expect(await screen.findByText(/conoce el correo y la contraseña/)).toBeInTheDocument();
  });

  it("stops the process without authorization", async () => {
    renderPage();
    fireEvent.click(within(screen.getByRole("radiogroup", { name: /propietario del equipo/ })).getByRole("radio", { name: "No" }));
    expect(await screen.findByText("No continúe con este equipo")).toBeInTheDocument();
    expect(screen.getByText("Comprobante de compra")).toBeInTheDocument();
  });

  it("labels causes as diagnosis and steps as owner instructions, then records", async () => {
    backend.record.mockResolvedValue({ id: 1, assessment: base });
    renderPage();
    fireEvent.click(within(screen.getByRole("radiogroup", { name: /pide la cuenta Google/ })).getByRole("radio", { name: "Sí" }));
    fireEvent.click(within(await screen.findByRole("radiogroup", { name: /conoce el correo/ })).getByRole("radio", { name: "No" }));
    expect(await screen.findByText("Recuperación oficial de la cuenta")).toBeInTheDocument();
    expect(screen.getByText("Diagnóstico")).toBeInTheDocument();
    expect(screen.getByText("Instrucción para el propietario")).toBeInTheDocument();
    expect(screen.getByText("https://accounts.google.com/signin/recovery")).toBeInTheDocument();
    const save = screen.getByRole("button", { name: "Registrar diagnóstico" });
    expect(save).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Marca y modelo"), { target: { value: "Samsung Galaxy A14" } });
    expect(save).toBeEnabled();
    fireEvent.click(save);
    await waitFor(() => expect(backend.record).toHaveBeenCalledWith(null, "Samsung Galaxy A14", expect.objectContaining({ knows_credentials: false }), ""));
  });
});
