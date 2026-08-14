import { afterEach, describe, expect, test, vi } from "vitest";
import { aiResponseErrorDetail, aiTimeoutDetail, commandErrorCopy } from "../feedback.js";

describe("AI timeout diagnostics", () => {
  afterEach(() => vi.useRealTimers());

  test("explains that silence cannot distinguish reasoning from a stalled provider", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:30Z"));

    const detail = aiTimeoutDetail(
      { code: "provider_timeout", message: "Azure Foundry request timed out after 45s" },
      {
        startedAtMs: new Date("2026-08-14T12:00:00Z").getTime(),
        phase: "calling_model",
        receivedCharacters: 0,
      },
    );

    expect(detail).toContain("30 s esperando la respuesta del modelo");
    expect(detail).toContain("ningún contenido recibido");
    expect(detail).toContain("no se puede distinguir entre razonamiento interno y un proveedor bloqueado");
    expect(detail).toContain("Detalle del sistema: Azure Foundry request timed out after 45s");
  });

  test("identifies a response that started and then became incomplete", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:30Z"));

    const detail = aiTimeoutDetail(
      null,
      {
        startedAtMs: new Date("2026-08-14T12:00:00Z").getTime(),
        phase: "streaming_delta",
        receivedCharacters: 640,
      },
    );

    expect(detail).toContain("640 caracteres recibidos antes de detenerse");
    expect(detail).toContain("La respuesta comenzó, pero quedó incompleta");
  });
});

describe("AI response diagnostics", () => {
  test("identifies the failed stage and preserves the safe backend diagnostic", () => {
    const detail = aiResponseErrorDetail(
      {
        code: "provider_response_error",
        message: "Azure Foundry returned an invalid response:\n critique_report is missing field `issues`",
      },
      { startedAtMs: 0, phase: "calling_critic", receivedCharacters: 0 },
    );

    expect(detail).toContain("Etapa: esperando la revisión del crítico");
    expect(detail).toContain("Diagnóstico: Azure Foundry returned an invalid response: critique_report is missing field `issues`");
    expect(detail).toContain("Ningún cambio se aplicó al canon");
  });

  test("bounds provider diagnostics before presenting them", () => {
    const detail = aiResponseErrorDetail(
      { code: "provider_response_error", message: "x".repeat(1_200) },
      { startedAtMs: 0, phase: "parsing_response", receivedCharacters: 0 },
    );

    expect(detail).toContain(`${"x".repeat(997)}...`);
    expect(detail).not.toContain("x".repeat(1_001));
  });

  test("uses dedicated copy for proposals rejected by deterministic validation", () => {
    expect(commandErrorCopy({ code: "invalid_ai_proposal" })).toEqual({
      kind: "warning",
      title: "La propuesta de IA no pasó la validación",
      detail: "Ningún cambio se aplicó. Revisa el diagnóstico y vuelve a generar la propuesta.",
    });
  });

  test("adds exact sanitized system details to known command errors", () => {
    expect(commandErrorCopy({
      code: "provider_transport_error",
      message: "Azure Foundry request failed: dns error: no such host",
    }).detail).toContain("Detalle del sistema: Azure Foundry request failed: dns error: no such host");
  });

  test("does not expose arbitrary messages for unknown error codes", () => {
    expect(commandErrorCopy({ code: "unknown", message: "UNTRUSTED_PAYLOAD" }).detail)
      .not.toContain("UNTRUSTED_PAYLOAD");
  });
});
