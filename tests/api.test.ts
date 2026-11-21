import { describe, expect, it } from "vitest";
import { ApiError, api, inTauri, toApiError } from "@/services/api";

describe("api error mapping", () => {
  it("keeps backend error payloads", () => {
    const err = toApiError({ code: "consent_required", message: "Registre el consentimiento" });
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe("consent_required");
    expect(err.message).toBe("Registre el consentimiento");
  });

  it("wraps strings and unknown values", () => {
    expect(toApiError("boom").code).toBe("internal");
    expect(toApiError(42).message).toMatch(/inesperado/);
  });

  it("refuses to fake data outside the desktop app", async () => {
    expect(inTauri()).toBe(false);
    await expect(api.appInfo()).rejects.toMatchObject({ code: "unavailable" });
  });
});
