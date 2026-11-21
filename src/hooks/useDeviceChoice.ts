import { useState } from "react";
import { useDevices } from "@/hooks/useDevices";
import type { DeviceView } from "@/types/api";

export const MANUAL_DEVICE = "__manual__";

/** Connected-device selector with a manual fallback (brand and model typed by the technician). */
export function useDeviceChoice() {
  const { snapshot } = useDevices();
  const devices = snapshot?.devices ?? [];
  const [choice, setChoice] = useState<string>("");
  const [manualLabel, setManualLabel] = useState("");
  const selected = choice === MANUAL_DEVICE ? MANUAL_DEVICE : devices.find((d) => d.id === choice)?.id ?? devices[0]?.id ?? MANUAL_DEVICE;
  const device: DeviceView | null = devices.find((d) => d.id === selected) ?? null;
  return { devices, selected, setChoice, device, manualLabel, setManualLabel };
}
