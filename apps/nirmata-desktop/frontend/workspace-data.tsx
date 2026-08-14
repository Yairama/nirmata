import { invoke } from "@tauri-apps/api/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseQueryResult } from "@tanstack/react-query";
import { createContext, useContext, useEffect, useMemo, useRef } from "react";
import type { ReactNode } from "react";
import { useSession } from "./session-provider.js";
import { appActions, useAppState } from "./state.js";
import type {
  OpenUriResponse,
  RelatedContextResponse,
  RevisionHistorySnapshot,
  TimelineOverview,
  WorldSession,
} from "./types.js";

export function observedScopeQueryKey(session: WorldSession) {
  return [
    "world",
    session.world_id,
    session.read_scope.variantId,
    session.read_scope.revisionId ?? "head",
  ] as const;
}

export function openUriQuery(session: WorldSession | null, uri: string | null) {
  const scopeKey = session
    ? observedScopeQueryKey(session)
    : ["world", "closed", "closed", "closed"] as const;
  return {
    queryKey: [...scopeKey, "object", uri] as const,
    queryFn: () => invoke<OpenUriResponse>("open_uri", { uri: uri! }),
    staleTime: 30_000,
  };
}

type WorkspaceData = {
  selectedUri: string | null;
  selectedObject: UseQueryResult<OpenUriResponse>;
  relatedContext: UseQueryResult<RelatedContextResponse>;
  timeline: UseQueryResult<TimelineOverview>;
  revisionHistory: UseQueryResult<RevisionHistorySnapshot>;
};

const WorkspaceDataContext = createContext<WorkspaceData | null>(null);

export function WorkspaceDataProvider({ children }: { children: ReactNode }) {
  const session = useSession();
  const state = useAppState();
  const queryClient = useQueryClient();
  const selectedUri = state.selectedUri;
  const previousWorldId = useRef(session?.world_id ?? null);
  const changingWorld = Boolean(previousWorldId.current && previousWorldId.current !== session?.world_id);
  const currentSelectedUri = changingWorld ? null : selectedUri;
  const scopeKey = session
    ? observedScopeQueryKey(session)
    : ["world", "closed", "closed", "closed"] as const;
  const selectedObject = useQuery({
    ...openUriQuery(session, currentSelectedUri),
    enabled: Boolean(session && currentSelectedUri),
    retry: false,
  });
  const relatedContext = useQuery({
    queryKey: [...scopeKey, "related-context", currentSelectedUri],
    queryFn: () => invoke<RelatedContextResponse>("get_related_context", { input: { uri: currentSelectedUri! } }),
    enabled: Boolean(session && currentSelectedUri),
    retry: false,
    staleTime: 30_000,
  });
  const timeline = useQuery({
    queryKey: [...scopeKey, "timeline"],
    queryFn: () => invoke<TimelineOverview>("list_timeline_events"),
    enabled: Boolean(session),
    retry: false,
  });
  const revisionHistory = useQuery({
    queryKey: [...scopeKey, "revision-history"],
    queryFn: () => invoke<RevisionHistorySnapshot>("list_revision_history"),
    enabled: Boolean(session),
    retry: false,
  });

  useEffect(() => {
    const worldId = session?.world_id ?? null;
    if (previousWorldId.current && previousWorldId.current !== worldId) {
      appActions.setSelectedUri(null);
      appActions.setSelectedLogicalPath(null);
      appActions.setStructuredEditor(null);
    }
    previousWorldId.current = worldId;
  }, [session?.world_id]);

  useEffect(() => {
    if (!session) return;
    const previousScopeKey = observedScopeQueryKey(session);
    return () => {
      void queryClient.cancelQueries({ queryKey: previousScopeKey }).then(() => {
        queryClient.removeQueries({ queryKey: previousScopeKey });
      });
    };
  }, [queryClient, session?.world_id, session?.read_scope.variantId, session?.read_scope.revisionId]);

  const value = useMemo(() => ({
    selectedUri: currentSelectedUri,
    selectedObject,
    relatedContext,
    timeline,
    revisionHistory,
  }), [currentSelectedUri, relatedContext, revisionHistory, selectedObject, timeline]);
  return <WorkspaceDataContext.Provider value={value}>{changingWorld ? null : children}</WorkspaceDataContext.Provider>;
}

export function useWorkspaceData(): WorkspaceData {
  const value = useContext(WorkspaceDataContext);
  if (!value) throw new Error("WorkspaceDataProvider is missing.");
  return value;
}
