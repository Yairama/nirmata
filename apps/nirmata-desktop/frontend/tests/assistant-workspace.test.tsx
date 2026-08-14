import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { AiRunSnapshot, WorldSession } from "../types.js";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  unlisten: [vi.fn(), vi.fn(), vi.fn()],
  beginAiActivity: vi.fn(),
  endAiActivity: vi.fn(),
  session: null as WorldSession | null,
  state: { aiProviderReady: false },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../session-provider.js", () => ({ useSession: () => mocks.session }));
vi.mock("../state.js", () => ({
  useAppState: () => ({ selectedUri: null }),
  beginAiActivity: mocks.beginAiActivity,
  endAiActivity: mocks.endAiActivity,
  listen: mocks.listen,
}));
vi.mock("../helpers.js", () => ({
  clearError: vi.fn(),
  commandCode: (value: { code?: string }) => value?.code ?? null,
  humanize: (value: string) => value.split("_").map((part) => part.slice(0, 1).toUpperCase() + part.slice(1)).join(" "),
  labelForUri: () => "Objeto seleccionado",
  showError: vi.fn(),
}));
vi.mock("../pending-reviews.js", () => ({ pendingReviewsQueryKey: (session: WorldSession) => ["world", session.world_id, "pending-reviews"] }));
vi.mock("../object-picker.js", () => ({ useObjectPicker: () => vi.fn() }));
vi.mock("../workspace.js", () => ({ selectUri: vi.fn(), selectUriInScope: vi.fn(() => Promise.resolve(true)) }));

import { AssistantWorkspace } from "../assistant-workspace.js";

const session = {
  world_id: "world-1",
  current_revision: "revision-1",
  read_only: false,
  active_variant: { id: "variant-1", name: "Principal" },
  read_scope: { variantId: "variant-1", revisionId: null },
  world: { id: "world-1", name: "Mundo", premise_md: "", epoch_label: "", current_revision: "revision-1" },
} as WorldSession;

const provider = {
  state: "connected",
  message: "Conexión verificada.",
  canCheckConnection: true,
  connected: true,
  credential: { configured: true, source: "system_secure_store", persistence: "system_secure_store", secureStoreAvailable: true, limitation: null },
};

function runWithBrief(): AiRunSnapshot {
  return {
    id: "run-template",
    baseRevision: "revision-1",
    request: "Expandir ciudad",
    status: "intent_brief_ready",
    context: { context: { canon: [{ uri: "nirmata://world/world-1" }], perspectives: [], desires: [], obligations: [], searchEvidence: [] } },
    draft: null,
    validationReport: null,
    critiqueReport: null,
    repairCount: 0,
    reviewKey: null,
    intentBrief: {
      userRequest: "Expandir ciudad",
      objective: "Crear una ciudad",
      scope: "Ciudad y rutas",
      entities: [],
      restrictions: ["Máximo tres operaciones"],
      reason: "Delimita la expansión",
      authority: "Solo prepara una propuesta; no es canon.",
      template: "city",
      scale: "small",
    },
    error: null,
  };
}

function renderAssistant(intent: { id: number; mode: "query" | "propose"; template?: "city" } | null = null) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><AssistantWorkspace active intent={intent} onClose={vi.fn()} /></QueryClientProvider>);
}

describe("AssistantWorkspace", () => {
  beforeEach(() => {
    localStorage.clear();
    mocks.session = session;
    mocks.invoke.mockReset().mockImplementation((command: string, args?: { input?: { request?: string } }) => {
      if (command === "get_ai_provider_status") return Promise.resolve(provider);
      if (command === "list_pending_reviews") return Promise.resolve([]);
      if (command === "execute_ai_query") return Promise.resolve({
        request: args?.input?.request ?? "",
        snapshot: { worldId: "world-1", baseRevision: "revision-1", readScope: { variantId: "variant-1", revisionId: null } },
        items: [{ itemId: crypto.randomUUID(), classification: "inference", markdown: "Respuesta segura <img onerror=alert(1)>", contentReferences: [], citations: [] }],
        proposalAction: null,
      });
      throw new Error(command);
    });
    mocks.unlisten.forEach((value) => value.mockReset());
    mocks.listen.mockReset().mockImplementation((_event: string) => Promise.resolve(mocks.unlisten[mocks.listen.mock.calls.length - 1]));
    mocks.beginAiActivity.mockReset();
    mocks.endAiActivity.mockReset();
  });

  afterEach(cleanup);

  test("switches query, proposal, deep review and audit modes without executing them", async () => {
    const user = userEvent.setup();
    renderAssistant();
    await screen.findByText("Conexión verificada.");

    await user.click(screen.getByRole("button", { name: "Proponer cambios" }));
    expect(screen.getByRole("button", { name: "Proponer cambios" }).getAttribute("aria-pressed")).toBe("true");
    await user.click(screen.getByText("Perfiles avanzados"));
    await user.click(screen.getByRole("button", { name: /Revisión profunda/u }));
    expect(screen.getByRole("button", { name: /Revisión profunda/u }).getAttribute("aria-pressed")).toBe("true");
    await user.click(screen.getByRole("button", { name: /Auditoría del canon/u }));
    expect(screen.getByRole("button", { name: /Auditoría del canon/u }).getAttribute("aria-pressed")).toBe("true");
    expect(mocks.invoke.mock.calls.some(([command]) => command === "execute_deep_review")).toBe(false);
  });

  test("keeps local turns and sends only prior bounded conversation history", async () => {
    const user = userEvent.setup();
    renderAssistant();
    await screen.findByText("Conexión verificada.");
    const request = screen.getByRole("textbox", { name: "Solicitud" });

    await user.type(request, "Primera pregunta");
    await user.click(screen.getAllByRole("button", { name: "Consultar" }).at(-1)!);
    await screen.findByText("Respuesta segura <img onerror=alert(1)>");
    expect(document.getElementsByTagName("img")).toHaveLength(0);
    await user.clear(request);
    await user.type(request, "Segunda pregunta");
    await user.click(screen.getAllByRole("button", { name: "Consultar" }).at(-1)!);
    await waitFor(() => expect(mocks.invoke.mock.calls.filter(([command]) => command === "execute_ai_query")).toHaveLength(2));

    const second = mocks.invoke.mock.calls.filter(([command]) => command === "execute_ai_query")[1][1];
    expect(second.input.history).toHaveLength(1);
    expect(second.input.history[0]).toMatchObject({ userRequest: "Primera pregunta", assistantResponse: "Respuesta segura <img onerror=alert(1)>" });
    expect(JSON.parse(localStorage.getItem("nirmata.assistant.conversations.world-1") ?? "[]")[0].turns).toHaveLength(2);
  });

  test("edits a template brief and continues the same run", async () => {
    const user = userEvent.setup();
    mocks.invoke.mockImplementation((command: string, args?: { input?: Record<string, unknown> }) => {
      if (command === "get_ai_provider_status") return Promise.resolve(provider);
      if (command === "prepare_ai_proposal_template") return Promise.resolve(runWithBrief());
      if (command === "execute_ai_proposal_from_brief") return Promise.resolve({ ...runWithBrief(), id: args?.input?.runId, status: "awaiting_review", intentBrief: null, reviewKey: "review-1", draft: { objective: args?.input?.objective, assumptions: [], operations: [], decisions: [] } });
      throw new Error(command);
    });
    renderAssistant({ id: 1, mode: "propose", template: "city" });

    const objective = await screen.findByRole("textbox", { name: "Objetivo" });
    await user.clear(objective);
    await user.type(objective, "Ciudad autónoma");
    await user.selectOptions(screen.getAllByRole("combobox", { name: "Escala" }).at(-1)!, "medium");
    await user.click(screen.getByRole("button", { name: "Continuar al proveedor" }));

    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("execute_ai_proposal_from_brief", { input: expect.objectContaining({ runId: "run-template", objective: "Ciudad autónoma", scale: "medium" }) }));
  });

  test("cancels the active request through its concrete command", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    const pending = new Promise((resolve) => { finish = resolve; });
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "get_ai_provider_status") return Promise.resolve(provider);
      if (command === "execute_ai_query") return pending;
      if (command === "cancel_ai_request") return Promise.resolve(null);
      throw new Error(command);
    });
    renderAssistant();
    await screen.findByText("Conexión verificada.");
    await user.type(screen.getByRole("textbox", { name: "Solicitud" }), "Consulta larga");
    await user.click(screen.getAllByRole("button", { name: "Consultar" }).at(-1)!);
    await user.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_ai_request", { requestId: expect.any(String) });
    finish({ request: "", snapshot: { worldId: "world-1", baseRevision: "revision-1", readScope: session.read_scope }, items: [], proposalAction: null });
  });

  test("unregisters all Tauri progress listeners on unmount", async () => {
    const view = renderAssistant();
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(3));
    await waitFor(() => expect(mocks.unlisten.every((unlisten) => unlisten.mock.calls.length === 0)).toBe(true));
    view.unmount();
    await waitFor(() => mocks.unlisten.forEach((unlisten) => expect(unlisten).toHaveBeenCalledOnce()));
  });
});
