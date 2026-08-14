import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen as listenTauri } from "@tauri-apps/api/event";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useSyncExternalStore } from "react";
import type {
  AiActivity,
  AppState,
  LogicalVfsDirectory,
  SearchKind,
  SearchObjectKind,
  StructuredEditorState,
  WorkspaceNotice,
  WorldSession,
} from "./types.js";

export { invoke };
export function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  return isTauri() ? listenTauri(event, handler) : Promise.resolve(() => undefined);
}
export const dialog = { open, save };
export const loreFilter = [{ name: "Lore UTF-8", extensions: ["md", "markdown", "txt"] }];
export const kinds: Array<{ value: SearchKind; label: string }> = [
  { value: "all", label: "Todo" },
  { value: "entity", label: "Entidades" },
  { value: "relation", label: "Relaciones" },
  { value: "event", label: "Eventos" },
  { value: "claim", label: "Afirmaciones" },
  { value: "rule", label: "Reglas" },
  { value: "goal", label: "Metas" },
  { value: "document", label: "Documentos" },
];
export const createKinds = kinds.filter((kind) => kind.value !== "all") as Array<{
  value: SearchObjectKind;
  label: string;
}>;

const emptyState: AppState = {
  session: null,
  logicalTree: null,
  selectedUri: null,
  selectedLogicalPath: null,
  structuredEditor: null,
  recentUris: [],
  ephemeralWork: {},
  workspaceNotice: null,
  aiActivity: null,
  status: "",
  discardRevision: 0,
};

let snapshot: AppState = Object.freeze(emptyState);
const listeners = new Set<() => void>();

function publish(patch: Partial<AppState>): void {
  const entries = Object.entries(patch) as Array<[keyof AppState, AppState[keyof AppState]]>;
  if (entries.every(([key, value]) => Object.is(snapshot[key], value))) return;
  snapshot = Object.freeze({ ...snapshot, ...patch });
  for (const listener of listeners) listener();
}

export function getAppState(): AppState {
  return snapshot;
}

export function subscribeAppState(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useAppState(): AppState {
  return useSyncExternalStore(subscribeAppState, getAppState, getAppState);
}

export const appActions = {
  setSession(session: WorldSession | null) {
    publish({ session });
  },
  setSelectedUri(selectedUri: string | null) {
    publish({ selectedUri });
  },
  selectUri(selectedUri: string) {
    publish({ selectedUri, selectedLogicalPath: null, structuredEditor: null });
  },
  setLogicalTree(logicalTree: LogicalVfsDirectory | null, selectedLogicalPath = snapshot.selectedLogicalPath) {
    publish({ logicalTree, selectedLogicalPath });
  },
  setSelectedLogicalPath(selectedLogicalPath: string | null) {
    publish({ selectedLogicalPath });
  },
  setStructuredEditor(structuredEditor: StructuredEditorState | null) {
    publish({ structuredEditor });
  },
  updateStructuredEditor(update: (editor: StructuredEditorState) => StructuredEditorState) {
    if (snapshot.structuredEditor) publish({ structuredEditor: update(snapshot.structuredEditor) });
  },
  recordRecentUri(uri: string) {
    if (snapshot.recentUris[0] === uri) return;
    publish({ recentUris: [uri, ...snapshot.recentUris.filter((item) => item !== uri)].slice(0, 8) });
  },
  setWorkspaceNotice(workspaceNotice: WorkspaceNotice | null) {
    publish({ workspaceNotice });
  },
  setAiActivity(aiActivity: AiActivity | null) {
    publish({ aiActivity });
  },
  endAiActivity(requestId: string) {
    if (snapshot.aiActivity?.requestId === requestId) publish({ aiActivity: null });
  },
  setEphemeralWork(key: string, label: string, present: boolean) {
    const ephemeralWork = { ...snapshot.ephemeralWork };
    if (present) ephemeralWork[key] = label;
    else delete ephemeralWork[key];
    publish({ ephemeralWork });
  },
  discardEphemeralWork() {
    publish({ ephemeralWork: {}, discardRevision: snapshot.discardRevision + 1 });
  },
  setStatus(status: string) {
    publish({ status });
  },
  resetWorkspace(session: WorldSession | null, status: string) {
    publish({
      session,
      logicalTree: null,
      selectedUri: null,
      selectedLogicalPath: null,
      structuredEditor: null,
      recentUris: [],
      ephemeralWork: {},
      workspaceNotice: null,
      status,
      discardRevision: snapshot.discardRevision + 1,
    });
  },
};

export function beginAiActivity(activity: AiActivity): void {
  appActions.setAiActivity(activity);
}

export function endAiActivity(requestId: string): void {
  appActions.endAiActivity(requestId);
}

export function setEphemeralWork(key: string, label: string, present: boolean): void {
  appActions.setEphemeralWork(key, label, present);
}
