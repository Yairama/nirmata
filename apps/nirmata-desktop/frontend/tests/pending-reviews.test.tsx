import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { ManualReviewSnapshot, PendingReviewSnapshot, WorldSession } from "../types.js";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  applyCommandStateError: vi.fn(),
  confirmDiscardPending: vi.fn(() => Promise.resolve(true)),
  state: {
    session: null as WorldSession | null,
    structuredEditor: null,
    selectedUri: null,
    selectedLogicalPath: null,
    workspaceNotice: null,
  },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../state.js", () => ({
  getAppState: () => mocks.state,
  useAppState: () => mocks.state,
  appActions: {
    setSession: (session: WorldSession) => { mocks.state.session = session; },
    setStructuredEditor: vi.fn(),
    setWorkspaceNotice: vi.fn(),
    selectUri: vi.fn(),
  },
  beginAiActivity: vi.fn(),
  endAiActivity: vi.fn(),
}));
vi.mock("../helpers.js", () => ({
  clearError: vi.fn(),
  firstReviewIssue: (report: { errors: unknown[]; conflicts: unknown[]; warnings: unknown[]; info: unknown[] }) => report.errors[0] ?? report.conflicts[0] ?? report.warnings[0] ?? report.info[0] ?? null,
  humanize: (value: string) => value.replaceAll("_", " "),
  labelForUri: (uri: string) => uri.startsWith("nirmata://world/") ? "Mundo de prueba" : "Fuente relacionada",
  objectKindFromUri: (uri: string) => uri.split("/")[2] ?? null,
  setStatus: vi.fn(),
  validationIssueMessage: (issue: { message: string }) => issue.message,
}));
vi.mock("../workspace.js", () => ({
  applyCommandStateError: mocks.applyCommandStateError,
  confirmDiscardPending: mocks.confirmDiscardPending,
  openReviewOperationEditor: vi.fn(),
  selectUri: vi.fn(),
  startCreatingObject: vi.fn(() => Promise.resolve(true)),
}));

import { PendingReviews } from "../pending-reviews.js";

const session = {
  world_id: "world-1",
  current_revision: "revision-1",
  read_only: false,
  active_variant: { id: "variant-1", name: "Principal" },
  read_scope: { variantId: "variant-1", revisionId: null },
  world: { id: "world-1", name: "Mundo de prueba" },
} as WorldSession;

const emptyReport = { errors: [], conflicts: [], warnings: [], info: [] };

function review(reviewKey: string, options: { stale?: boolean; rich?: boolean; ready?: boolean } = {}): ManualReviewSnapshot {
  const warning = { code: "review.warning", severity: "warning" as const, objects: [], message: "Revisa esta consecuencia." };
  return {
    reviewKey,
    objective: "Cambiar el Archivo",
    sources: ["nirmata://world/world-1"],
    assumptions: ["La ruta sigue abierta"],
    baseRevision: "revision-1",
    operations: [{
      operationId: `${reviewKey}-operation`, decision: "accept", selected: true, severity: options.rich ? "warning" : "info",
      targetUri: "nirmata://entity/entity-1", dependencies: [],
      before: { title: "Archivo", objectType: "entity", targetUri: "nirmata://entity/entity-1", lines: [{ label: "Resumen", value: "Antes" }] },
      after: { title: "Archivo renovado", objectType: "entity", targetUri: "nirmata://entity/entity-1", lines: [{ label: "Resumen", value: "Después" }] },
      issues: options.rich ? { ...emptyReport, warnings: [warning] } : emptyReport,
      effectiveIssues: options.rich ? { ...emptyReport, warnings: [warning] } : emptyReport,
      waivers: [],
      decisionPoints: options.rich ? [{ decisionPointId: "decision-1", prompt: "¿Qué versión conservar?", alternatives: ["keep", "replace"], replacementTarget: null, suggestionAvailable: true, suggestionHidden: true, reason: null, resolvedAlternative: null }] : [],
      risk: options.rich ? { requiresJudgment: true, judgment: null, suggestedResolutionAvailable: true, suggestedResolutionHidden: true, triggers: [{ code: "wide", title: "Impacto amplio", detail: "Afecta varias piezas." }] } : { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] },
    }],
    validationReport: emptyReport,
    effectiveReport: emptyReport,
    readyToConfirm: options.ready ?? !options.rich,
    freshness: options.stale
      ? { status: "stale", currentRevision: "revision-2", canRevalidate: true, message: "El mundo avanzó." }
      : { status: "current", currentRevision: "revision-1", canRevalidate: false, message: "Vigente" },
  };
}

function record(value: ManualReviewSnapshot, origin: PendingReviewSnapshot["origin"], aiRunId: string | null = null): PendingReviewSnapshot {
  return {
    review: value,
    origin,
    aiRunId,
    title: origin === "ai" ? "Propuesta asistida" : "Edición del Archivo",
    merge: null,
    editorRequest: { objectType: "entity", existingUri: "nirmata://entity/entity-1", objective: value.objective, sourceUris: value.sources, assumptions: value.assumptions, values: {} },
  };
}

function renderReviews(records: PendingReviewSnapshot[], onCountChange = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  render(<QueryClientProvider client={client}><PendingReviews open onClose={vi.fn()} onCountChange={onCountChange} onEdit={vi.fn()} onStartProposal={vi.fn()} onOpenImports={vi.fn()} onOpenWorld={vi.fn()} /></QueryClientProvider>);
  return { client, onCountChange };
}

describe("PendingReviews", () => {
  beforeEach(() => {
    mocks.state.session = session;
    mocks.state.structuredEditor = null;
    mocks.invoke.mockReset();
    mocks.applyCommandStateError.mockReset();
    mocks.confirmDiscardPending.mockClear();
  });

  afterEach(() => cleanup());

  test("rehydrates manual and AI origins and reads each persisted review through Query", async () => {
    const manual = record(review("manual-review"), "manual");
    const ai = record(review("ai-review", { ready: false }), "ai", "run-1");
    mocks.invoke.mockImplementation((command: string, args?: { input?: { reviewKey?: string }; runId?: string }) => {
      if (command === "list_pending_reviews") return Promise.resolve([manual, ai]);
      if (command === "read_manual_review") return Promise.resolve(args?.input?.reviewKey === "manual-review" ? manual.review : ai.review);
      if (command === "read_ai_run") return Promise.resolve({ id: args?.runId, status: "awaiting_final_critique", critiqueReport: { issues: [] }, draft: { objective: "", assumptions: [], operations: [], decisions: [] } });
      throw new Error(command);
    });
    const { onCountChange } = renderReviews([manual, ai]);

    expect(await screen.findByText("Edición manual")).not.toBeNull();
    expect(screen.getByText("IA")).not.toBeNull();
    await waitFor(() => expect(onCountChange).toHaveBeenLastCalledWith(2));
    await waitFor(() => expect(mocks.invoke.mock.calls.filter(([command]) => command === "read_manual_review")).toHaveLength(2));
    expect(screen.getByRole("button", { name: "Revalidar crítica final" })).not.toBeNull();
  });

  test("submits judgment, decision, waiver and reject actions without browser prompts", async () => {
    let current = review("rich-review", { rich: true });
    const pending = record(current, "manual");
    mocks.invoke.mockImplementation((command: string, args?: { input?: { action?: { kind: string; judgment?: string; alternative?: string; rationale?: string } } }) => {
      if (command === "list_pending_reviews") return Promise.resolve([{ ...pending, review: current }]);
      if (command === "read_manual_review") return Promise.resolve(current);
      if (command === "apply_manual_review_action") {
        const action = args?.input?.action;
        const operation = current.operations[0];
        if (action?.kind === "record_judgment") current = { ...current, operations: [{ ...operation, risk: { ...operation.risk, judgment: action.judgment ?? null }, decisionPoints: operation.decisionPoints.map((item) => ({ ...item, suggestionHidden: false })) }] };
        if (action?.kind === "resolve_decision") current = { ...current, operations: [{ ...operation, decisionPoints: operation.decisionPoints.map((item) => ({ ...item, suggestionHidden: false, resolvedAlternative: action.alternative ?? null })) }] };
        if (action?.kind === "add_waiver") current = { ...current, operations: [{ ...operation, waivers: [{ issueCode: "review.warning", rationale: action.rationale ?? "", createdAtMs: 1 }] }] };
        if (action?.kind === "reject") current = { ...current, operations: [{ ...operation, selected: false, decision: "reject" }] };
        return Promise.resolve(current);
      }
      throw new Error(command);
    });
    const user = userEvent.setup();
    renderReviews([pending]);

    await user.click(await screen.findByRole("button", { name: "Registrar juicio" }));
    await user.type(screen.getByLabelText("Tu lectura antes de revelar la resolución sugerida"), "El impacto es aceptable.");
    await user.click(screen.getByRole("button", { name: "Guardar juicio" }));
    await waitFor(() => expect(screen.getByText(/Juicio registrado/u)).not.toBeNull());
    await user.click(screen.getByRole("button", { name: "keep" }));
    await user.click(screen.getByRole("button", { name: "Aceptar advertencia con motivo" }));
    await user.type(screen.getByLabelText("Motivo para aceptar review.warning"), "La fuente lo justifica.");
    await user.click(screen.getByRole("button", { name: "Guardar motivo" }));
    await user.click(screen.getByRole("button", { name: "Rechazar operación" }));

    await waitFor(() => expect(mocks.invoke.mock.calls.filter(([command]) => command === "apply_manual_review_action")).toHaveLength(4));
    expect(mocks.invoke.mock.calls.map(([, args]) => args?.input?.action?.kind)).toEqual(expect.arrayContaining(["record_judgment", "resolve_decision", "add_waiver", "reject"]));
  });

  test("keeps stale cards after apply/discard failures and revalidates explicitly", async () => {
    let current = review("stale-review", { stale: true, ready: true });
    const pending = record(current, "manual");
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_pending_reviews") return Promise.resolve([{ ...pending, review: current }]);
      if (command === "read_manual_review") return Promise.resolve(current);
      if (command === "revalidate_manual_review") {
        current = { ...current, freshness: { status: "current", currentRevision: "revision-2", canRevalidate: false, message: "Vigente" } };
        return Promise.resolve(current);
      }
      if (command === "confirm_manual_review" || command === "discard_manual_review") return Promise.reject({ code: "storage_error", message: "failure" });
      throw new Error(command);
    });
    const user = userEvent.setup();
    renderReviews([pending]);

    expect((await screen.findByRole("button", { name: "Aplicar al mundo" }) as HTMLButtonElement).disabled).toBe(true);
    await user.click(screen.getByRole("button", { name: "Actualizar y volver a comprobar" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Aplicar al mundo" }) as HTMLButtonElement).disabled).toBe(false));
    await user.click(screen.getByRole("button", { name: "Aplicar al mundo" }));
    await waitFor(() => expect(mocks.applyCommandStateError).toHaveBeenCalled());
    expect(screen.getByText("Edición del Archivo")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Descartar propuesta" }));
    await user.click(screen.getByRole("button", { name: "Sí, descartar propuesta" }));
    await waitFor(() => expect(mocks.applyCommandStateError).toHaveBeenCalledTimes(2));
    expect(screen.getByText("Edición del Archivo")).not.toBeNull();
  });

  test("never enables apply while the observed version is read-only", async () => {
    mocks.state.session = { ...session, read_only: true, read_scope: { variantId: "variant-1", revisionId: "revision-1" } };
    const current = review("read-only-review", { ready: true });
    const pending = record(current, "manual");
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_pending_reviews") return Promise.resolve([pending]);
      if (command === "read_manual_review") return Promise.resolve(current);
      throw new Error(command);
    });
    renderReviews([pending]);

    const apply = await screen.findByRole("button", { name: "Aplicar al mundo" }) as HTMLButtonElement;
    expect(apply.disabled).toBe(true);
    expect(apply.title).toContain("Vuelve a la versión actual");
  });
});
