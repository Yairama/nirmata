import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { buildCreateEditor } from "../editor-create.js";
import { appActions, getAppState } from "../state.js";
import { StructuredEditor } from "../structured-editor.js";
import { WorkspaceDataProvider } from "../workspace-data.js";
import type { SearchResult, WorldSession } from "../types.js";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  saveCurrentDraft: vi.fn(),
  resetCurrentEditor: vi.fn(),
  selectUri: vi.fn(),
  requestObjectPicker: vi.fn(),
}));

const session = {
  world_id: "10000000-0000-4000-8000-000000000001",
  current_revision: "20000000-0000-4000-8000-000000000001",
  read_only: false,
  active_variant: { id: "30000000-0000-4000-8000-000000000001" },
  read_scope: { variantId: "30000000-0000-4000-8000-000000000001", revisionId: null },
  world: { calendar: null },
} as WorldSession;

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../session-provider.js", () => ({ useSession: () => session }));
vi.mock("../workspace.js", () => ({
  saveCurrentDraft: mocks.saveCurrentDraft,
  resetCurrentEditor: mocks.resetCurrentEditor,
  selectUri: mocks.selectUri,
}));
vi.mock("../object-picker.js", () => ({ useObjectPicker: () => mocks.requestObjectPicker }));

function result(kind: SearchResult["object_type"], id: string, snippet: string): SearchResult {
  return {
    object_ref: { [kind]: id } as SearchResult["object_ref"],
    object_type: kind,
    object_id: id,
    uri: `nirmata://${kind}/${id}`,
    snippet,
    authority: "canonical",
    classification: "fact",
    provenance: "test",
    stage: "search",
    score: 1,
    rank: 1,
    score_explanation: "test",
  };
}

function renderEditor() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <WorkspaceDataProvider><StructuredEditor /></WorkspaceDataProvider>
    </QueryClientProvider>,
  );
}

async function applyNextPicker(user: ReturnType<typeof userEvent.setup>, buttonName: string, selected: SearchResult[], index = 0) {
  mocks.requestObjectPicker.mockClear();
  await user.click(screen.getAllByRole("button", { name: buttonName })[index]);
  const picker = mocks.requestObjectPicker.mock.calls[0][0] as { apply: (results: SearchResult[]) => void };
  act(() => picker.apply(selected));
}

describe("StructuredEditor", () => {
  beforeEach(() => {
    appActions.setSession(session);
    mocks.invoke.mockReset().mockImplementation((_command: string, args: { uri?: string }) => Promise.resolve({ result: { snippet: args.uri?.endsWith("0001") ? "Mara" : "Vale" } }));
    mocks.saveCurrentDraft.mockReset().mockResolvedValue(undefined);
    mocks.resetCurrentEditor.mockReset();
    appActions.setStructuredEditor(null);
  });

  afterEach(() => {
    cleanup();
    appActions.setStructuredEditor(null);
  });

  test("renders claim fields conditionally and focuses the first Rust issue", async () => {
    const editor = buildCreateEditor("claim");
    editor.issues = [{ field: "subject_entity", message: "Elige una entidad sujeto." }];
    appActions.setStructuredEditor(editor);
    const user = userEvent.setup();
    renderEditor();

    const technicalSubject = screen.getByRole("textbox", { name: "Entidad sujeto, valor técnico" });
    await waitFor(() => expect(technicalSubject.getAttribute("aria-invalid")).toBe("true"));
    expect(technicalSubject.getAttribute("aria-invalid")).toBe("true");
    expect(screen.queryByText("Quien sostiene la afirmación")).toBeNull();

    await user.selectOptions(screen.getByLabelText("Autenticación"), "attributed");
    expect(screen.getByText("Quien sostiene la afirmación")).not.toBeNull();
    expect(screen.getByLabelText("Modalidad")).not.toBeNull();
  });

  test("uses field arrays for participant add, reorder and transport", async () => {
    const editor = buildCreateEditor("event");
    appActions.setStructuredEditor(editor);
    const user = userEvent.setup();
    renderEditor();
    const mara = result("entity", "40000000-0000-4000-8000-000000000001", "Mara");
    const vale = result("entity", "40000000-0000-4000-8000-000000000002", "Vale");

    await applyNextPicker(user, "Agregar participante", [mara]);
    await applyNextPicker(user, "Agregar participante", [vale]);
    await user.type(screen.getAllByLabelText("Rol")[0], "testigo");
    await user.type(screen.getAllByLabelText("Rol")[1], "guía");
    await user.click(screen.getByRole("button", { name: "Subir participante 2" }));

    await user.click(screen.getByRole("button", { name: "Preparar cambios" }));
    await waitFor(() => expect(mocks.saveCurrentDraft.mock.calls[0][0].values.participants).toBe(`${vale.uri}|guía|0\n${mara.uri}|testigo|1`));
    expect(screen.getAllByText(/Mara|Vale/u)).toHaveLength(2);
  });

  test("serializes document references in the reordered field-array order", async () => {
    const editor = buildCreateEditor("document");
    appActions.setStructuredEditor(editor);
    const user = userEvent.setup();
    renderEditor();
    const first = result("event", "60000000-0000-4000-8000-000000000001", "Fundación");
    const second = result("claim", "70000000-0000-4000-8000-000000000001", "Juramento");

    await applyNextPicker(user, "Agregar en Referencias de contenido por nombre", [first, second]);
    await user.click(screen.getByRole("button", { name: "Subir referencia 2" }));

    await user.click(screen.getByRole("button", { name: "Preparar cambios" }));
    await waitFor(() => expect(mocks.saveCurrentDraft.mock.calls[0][0].values.content_references).toBe(`${second.uri}|0\n${first.uri}|1`));
  });
});
