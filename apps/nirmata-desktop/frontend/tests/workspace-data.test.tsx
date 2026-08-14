import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { setConfirmationHandler } from "../confirmation.js";
import { buildCreateEditor } from "../editor-create.js";
import { appActions, getAppState } from "../state.js";
import { StructuredEditor } from "../structured-editor.js";
import type { OpenUriResponse, RelatedContextResponse, SearchResult, WorldSession } from "../types.js";
import { selectUri } from "../workspace.js";
import { WorkspaceDataProvider, useWorkspaceData } from "../workspace-data.js";
import { WorldContext } from "../world-context.js";
import { WorldTimeline } from "../world-timeline.js";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
let currentSession: WorldSession;

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../session-provider.js", () => ({ useSession: () => currentSession }));
vi.mock("../object-picker.js", () => ({ useObjectPicker: () => vi.fn() }));
vi.mock("../helpers.js", async (importOriginal) => {
  const original = await importOriginal<typeof import("../helpers.js")>();
  return { ...original, setStatus: vi.fn() };
});

const uriA = "nirmata://entity/entity-a";
const uriB = "nirmata://entity/entity-b";

function session(worldId = "world-1", variantId = "variant-1", revisionId: string | null = null): WorldSession {
  return {
    world_id: worldId,
    current_revision: "head-1",
    read_only: revisionId !== null,
    active_variant: { id: "variant-1", name: "Principal" },
    read_scope: { variantId, revisionId },
    world: {
      id: worldId,
      name: `Mundo ${worldId}`,
      premise_md: "",
      epoch_label: "",
      current_revision: "head-1",
      calendar: null,
      created_at_ms: 1,
      updated_at_ms: 1,
    },
  } as WorldSession;
}

function result(uri: string, snippet: string): SearchResult {
  const id = uri.split("/").at(-1)!;
  return {
    object_ref: { entity: id },
    object_type: "entity",
    object_id: id,
    uri,
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

function opened(uri: string, name: string): OpenUriResponse {
  const id = uri.split("/").at(-1)!;
  return {
    result: result(uri, name),
    object: {
      entity: {
        id,
        world_id: currentSession.world_id,
        kind: "person",
        name,
        slug: name.toLocaleLowerCase("es"),
        aliases: [],
        summary: name,
        body_md: "",
        attributes_json: "{}",
        version: 1,
        created_at_ms: 1,
        updated_at_ms: 1,
      },
    },
    eventCalendar: null,
  };
}

function context(uri: string, label: string): RelatedContextResponse {
  return {
    canon: [{ result: result(uri, label), stage: "selection" }],
    perspectives: [],
    desires: [],
    obligations: [],
    search_evidence: [],
    usage: { max_objects: 24, max_chars: 4_000, used_objects: 1, used_chars: label.length },
    absence: null,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

function renderWorkspace(children: React.ReactNode, client = new QueryClient({ defaultOptions: { queries: { retry: false } } })) {
  const view = render(
    <QueryClientProvider client={client}>
      <WorkspaceDataProvider>{children}</WorkspaceDataProvider>
    </QueryClientProvider>,
  );
  return { ...view, client };
}

describe("WorkspaceDataProvider", () => {
  beforeEach(() => {
    currentSession = session();
    appActions.resetWorkspace(currentSession, "");
    setConfirmationHandler(null);
    mocks.invoke.mockReset();
  });

  afterEach(() => {
    cleanup();
    setConfirmationHandler(null);
    appActions.setSelectedUri(null);
    appActions.setStructuredEditor(null);
  });

  test("does not hydrate a late A response after selecting B", async () => {
    const objectA = deferred<OpenUriResponse>();
    const contextA = deferred<RelatedContextResponse>();
    const objectB = deferred<OpenUriResponse>();
    const contextB = deferred<RelatedContextResponse>();
    mocks.invoke.mockImplementation((command: string, args?: { uri?: string; input?: { uri?: string } }) => {
      if (command === "list_timeline_events") return Promise.resolve({ known: [], unknown: [], calendarName: null });
      if (command === "list_revision_history") return Promise.resolve({ currentHeadRevisionId: "head-1", undoTargetRevisionId: null, revisions: [] });
      const uri = args?.uri ?? args?.input?.uri;
      if (command === "open_uri") return uri === uriA ? objectA.promise : objectB.promise;
      if (command === "get_related_context") return uri === uriA ? contextA.promise : contextB.promise;
      throw new Error(command);
    });
    appActions.setSelectedUri(uriA);
    renderWorkspace(<StructuredEditor />);

    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("open_uri", { uri: uriA }));
    act(() => {
      appActions.setStructuredEditor(null);
      appActions.setSelectedUri(uriB);
    });
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("open_uri", { uri: uriB }));
    await act(async () => {
      objectB.resolve(opened(uriB, "B actual"));
      contextB.resolve(context(uriB, "Contexto B"));
    });
    expect(await screen.findByRole("heading", { name: "B actual" })).not.toBeNull();

    await act(async () => {
      objectA.resolve(opened(uriA, "A tardía"));
      contextA.resolve(context(uriA, "Contexto A tardío"));
    });
    expect(screen.queryByRole("heading", { name: "A tardía" })).toBeNull();
    expect(screen.getByRole("heading", { name: "B actual" })).not.toBeNull();
  });

  test("clears selection before querying a different world", async () => {
    mocks.invoke.mockImplementation((command: string, args?: { uri?: string; input?: { uri?: string } }) => {
      if (command === "list_timeline_events") return Promise.resolve({ known: [], unknown: [], calendarName: null });
      if (command === "list_revision_history") return Promise.resolve({ currentHeadRevisionId: "head-1", undoTargetRevisionId: null, revisions: [] });
      const uri = args?.uri ?? args?.input?.uri ?? uriA;
      if (command === "open_uri") return Promise.resolve(opened(uri, "Objeto del primer mundo"));
      if (command === "get_related_context") return Promise.resolve(context(uri, "Contexto primero"));
      throw new Error(command);
    });
    appActions.setSelectedUri(uriA);
    const view = renderWorkspace(<StructuredEditor />);
    expect(await screen.findByRole("heading", { name: "Objeto del primer mundo" })).not.toBeNull();
    const openCalls = mocks.invoke.mock.calls.filter(([command]) => command === "open_uri").length;

    currentSession = session("world-2");
    appActions.setSession(currentSession);
    view.rerender(
      <QueryClientProvider client={view.client}>
        <WorkspaceDataProvider><StructuredEditor /></WorkspaceDataProvider>
      </QueryClientProvider>,
    );

    await waitFor(() => expect(getAppState().selectedUri).toBeNull());
    expect(screen.queryByRole("heading", { name: "Objeto del primer mundo" })).toBeNull();
    expect(mocks.invoke.mock.calls.filter(([command]) => command === "open_uri")).toHaveLength(openCalls);
  });

  test("does not reuse object data across observed variant or revision", async () => {
    mocks.invoke.mockImplementation((command: string, args?: { uri?: string; input?: { uri?: string } }) => {
      if (command === "list_timeline_events") return Promise.resolve({ known: [], unknown: [], calendarName: null });
      if (command === "list_revision_history") return Promise.resolve({ currentHeadRevisionId: "head-1", undoTargetRevisionId: null, revisions: [] });
      const uri = args?.uri ?? args?.input?.uri ?? uriA;
      if (command === "open_uri") return Promise.resolve(opened(uri, currentSession.read_scope.variantId === "variant-1" ? "Versión uno" : "Versión dos"));
      if (command === "get_related_context") return Promise.resolve(context(uri, currentSession.read_scope.variantId));
      throw new Error(command);
    });
    appActions.setSelectedUri(uriA);
    const view = renderWorkspace(<StructuredEditor />);
    expect(await screen.findByRole("heading", { name: "Versión uno" })).not.toBeNull();

    currentSession = session("world-1", "variant-2", "revision-2");
    appActions.setSession(currentSession);
    appActions.setStructuredEditor(null);
    view.rerender(
      <QueryClientProvider client={view.client}>
        <WorkspaceDataProvider><StructuredEditor /></WorkspaceDataProvider>
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "Versión dos" })).not.toBeNull();
    expect(screen.queryByRole("heading", { name: "Versión uno" })).toBeNull();
    expect(mocks.invoke.mock.calls.filter(([command]) => command === "open_uri")).toHaveLength(2);
  });

  test("shows context only for the current selection", async () => {
    const objectA = deferred<OpenUriResponse>();
    const contextA = deferred<RelatedContextResponse>();
    mocks.invoke.mockImplementation((command: string, args?: { uri?: string; input?: { uri?: string } }) => {
      if (command === "list_timeline_events") return Promise.resolve({ known: [], unknown: [], calendarName: null });
      if (command === "list_revision_history") return Promise.resolve({ currentHeadRevisionId: "head-1", undoTargetRevisionId: null, revisions: [] });
      const uri = args?.uri ?? args?.input?.uri;
      if (command === "open_uri") return uri === uriA ? objectA.promise : Promise.resolve(opened(uriB, "Objeto B"));
      if (command === "get_related_context") return uri === uriA ? contextA.promise : Promise.resolve(context(uriB, "Canon B"));
      throw new Error(command);
    });
    appActions.setSelectedUri(uriA);
    renderWorkspace(<><StructuredEditor /><WorldContext /></>);
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("get_related_context", { input: { uri: uriA } }));
    act(() => {
      appActions.setStructuredEditor(null);
      appActions.setSelectedUri(uriB);
    });
    expect(await screen.findByText("Canon B")).not.toBeNull();

    await act(async () => {
      objectA.resolve(opened(uriA, "Objeto A"));
      contextA.resolve(context(uriA, "Canon A tardío"));
    });
    expect(screen.queryByText("Canon A tardío")).toBeNull();
    expect(screen.getByText("Canon B")).not.toBeNull();
  });

  test("dirty cancellation changes no intent and invokes no query or backend", async () => {
    mocks.invoke.mockImplementation((command: string, args?: { uri?: string; input?: { uri?: string } }) => {
      if (command === "list_timeline_events") return Promise.resolve({ known: [], unknown: [], calendarName: null });
      if (command === "list_revision_history") return Promise.resolve({ currentHeadRevisionId: "head-1", undoTargetRevisionId: null, revisions: [] });
      const uri = args?.uri ?? args?.input?.uri ?? uriA;
      if (command === "open_uri") return Promise.resolve(opened(uri, "Objeto A"));
      if (command === "get_related_context") return Promise.resolve(context(uri, "Canon A"));
      throw new Error(command);
    });
    appActions.setSelectedUri(uriA);
    renderWorkspace(<StructuredEditor />);
    await screen.findByRole("heading", { name: "Objeto A" });
    const editor = buildCreateEditor("entity");
    editor.values.name = "Trabajo sin guardar";
    appActions.setStructuredEditor(editor);
    setConfirmationHandler(() => Promise.resolve(false));
    mocks.invoke.mockClear();

    await expect(selectUri(uriB)).resolves.toBe(false);
    expect(getAppState().selectedUri).toBe(uriA);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });

  test("timeline and history have one shared query owner", async () => {
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_timeline_events") return Promise.resolve({ known: [], unknown: [], calendarName: null });
      if (command === "list_revision_history") return Promise.resolve({ currentHeadRevisionId: "head-1", undoTargetRevisionId: null, revisions: [] });
      throw new Error(command);
    });
    function Probe() {
      const { timeline, revisionHistory } = useWorkspaceData();
      return <span>{timeline.data ? "timeline" : ""}{revisionHistory.data ? "history" : ""}</span>;
    }
    renderWorkspace(<><Probe /><Probe /><WorldContext /><WorldTimeline onOpen={vi.fn()} onConfigureCalendar={vi.fn()} onCreateEvent={vi.fn()} onUseTemplate={vi.fn()} /></>);

    await screen.findAllByText("timelinehistory");
    expect(mocks.invoke.mock.calls.filter(([command]) => command === "list_timeline_events")).toHaveLength(1);
    expect(mocks.invoke.mock.calls.filter(([command]) => command === "list_revision_history")).toHaveLength(1);
  });
});
