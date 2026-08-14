import { afterEach, describe, expect, test, vi } from "vitest";
import { aiTimeoutDetail } from "../feedback.js";

describe("AI timeout diagnostics", () => {
  afterEach(() => vi.useRealTimers());

  test("explains that silence cannot distinguish reasoning from a stalled provider", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:30Z"));

    const detail = aiTimeoutDetail({
      startedAtMs: new Date("2026-08-14T12:00:00Z").getTime(),
      phase: "calling_model",
      receivedCharacters: 0,
    });

    expect(detail).toContain("30 s esperando la respuesta del modelo");
    expect(detail).toContain("ningún contenido recibido");
    expect(detail).toContain("no se puede distinguir entre razonamiento interno y un proveedor bloqueado");
  });

  test("identifies a response that started and then became incomplete", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:30Z"));

    const detail = aiTimeoutDetail({
      startedAtMs: new Date("2026-08-14T12:00:00Z").getTime(),
      phase: "streaming_delta",
      receivedCharacters: 640,
    });

    expect(detail).toContain("640 caracteres recibidos antes de detenerse");
    expect(detail).toContain("La respuesta comenzó, pero quedó incompleta");
  });
});
