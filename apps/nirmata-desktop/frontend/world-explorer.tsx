import { invoke } from "@tauri-apps/api/core";
import * as Tabs from "@radix-ui/react-tabs";
import { useQuery } from "@tanstack/react-query";
import { useDeferredValue, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { firstUriFromTree, pathForUri } from "./helpers.js";
import { useSession } from "./session-provider.js";
import { appActions, getAppState, useAppState } from "./state.js";
import type { LogicalVfsDirectory, LogicalVfsNode, SearchKind, SearchObjectKind, SearchResult, SearchWorldResponse } from "./types.js";
import { selectUri, startCreatingObject } from "./workspace.js";
import { observedScopeQueryKey } from "./workspace-data.js";

const filters: Array<{ value: SearchKind; label: string }> = [
  { value: "all", label: "Todo" },
  { value: "entity", label: "Entidades" },
  { value: "relation", label: "Relaciones" },
  { value: "event", label: "Eventos" },
  { value: "claim", label: "Afirmaciones" },
  { value: "rule", label: "Reglas" },
  { value: "goal", label: "Metas" },
  { value: "document", label: "Documentos" },
];

const createOptions = filters.slice(1) as Array<{ value: SearchObjectKind; label: string }>;

export function WorldExplorer({ onStartProposal, onEditorOpened }: { onStartProposal: () => void; onEditorOpened: () => void }) {
  const session = useSession();
  const state = useAppState();
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<SearchKind>("all");
  const [createKind, setCreateKind] = useState<SearchObjectKind>("entity");
  const [directUri, setDirectUri] = useState("");
  const deferredQuery = useDeferredValue(query.trim());
  const initialized = useRef(false);
  const scopeKey = session
    ? observedScopeQueryKey(session)
    : ["world", "closed", "closed", "closed"] as const;
  const tree = useQuery({
    queryKey: [...scopeKey, "vfs"],
    queryFn: () => invoke<LogicalVfsDirectory>("read_logical_vfs"),
    enabled: Boolean(session),
    retry: false,
  });
  const search = useQuery({
    queryKey: [...scopeKey, "search", deferredQuery, kind],
    queryFn: () => invoke<SearchWorldResponse>("search_world", {
      input: { queryText: deferredQuery, kind, limit: 200 },
    }),
    enabled: Boolean(session),
    retry: false,
    placeholderData: (previous) => previous,
  });

  useEffect(() => {
    if (!tree.data) return;
    const current = getAppState();
    const previousPath = current.selectedLogicalPath;
    if (current.selectedUri) {
      const nextPath = pathForUri(tree.data, current.selectedUri);
      appActions.setLogicalTree(tree.data, nextPath);
      if (nextPath && previousPath && nextPath !== previousPath) {
        appActions.setWorkspaceNotice({
          kind: "info",
          title: "Objeto movido en el explorador",
          detail: `La selección conserva su identidad y ahora vive en ${nextPath}.`,
        });
      }
    } else appActions.setLogicalTree(tree.data);
  }, [tree.data]);

  useEffect(() => {
    const current = getAppState();
    if (initialized.current || !tree.data || !search.data || current.selectedUri || current.structuredEditor) return;
    const firstUri = search.data.hits[0]?.uri ?? firstUriFromTree(tree.data);
    if (firstUri) {
      initialized.current = true;
      void selectUri(firstUri);
    }
  }, [search.data, tree.data]);

  if (!session) return null;

  async function openObject(uri: string) {
    await selectUri(uri);
  }

  function createObject() {
    void startCreatingObject(createKind).then((opened) => { if (opened) onEditorOpened(); });
  }

  function onResultKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const options = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button[data-result]"));
    if (options.length === 0) return;
    const current = options.indexOf(document.activeElement as HTMLButtonElement);
    let next = current < 0 ? 0 : current;
    if (event.key === "ArrowDown") next = Math.min(options.length - 1, next + 1);
    if (event.key === "ArrowUp") next = Math.max(0, next - 1);
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = options.length - 1;
    event.preventDefault();
    options[next]?.focus();
  }

  const hits = search.data?.hits ?? [];
  const worldIsEmpty = tree.data?.children.length === 0;
  const searchIsFiltered = Boolean(deferredQuery) || kind !== "all";
  return (
    <div className="world-explorer">
      <div className="panel-header explorer-heading">
        <div><p className="panel-eyebrow">Exploración</p><h3 id="explorer-title">Mundo</h3></div>
        <p className="panel-summary">{hits.length} resultado{hits.length === 1 ? "" : "s"}</p>
      </div>
      <div className="explorer-controls">
        <label className="search-field">Buscar
          <input
            className="world-explorer-search"
            name="world-search"
            type="search"
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            placeholder="Nombre o contenido"
            autoComplete="off"
          />
        </label>
        <div className="kind-filters" aria-label="Filtros por tipo">
          {filters.map((filter) => (
            <button key={filter.value} type="button" className="kind-filter secondary" aria-pressed={kind === filter.value} onClick={() => setKind(filter.value)}>{filter.label}</button>
          ))}
        </div>
        <div className="explorer-create">
          <label>Nuevo
            <select name="new-object-kind" value={createKind} onChange={(event) => setCreateKind(event.currentTarget.value as SearchObjectKind)} disabled={session.read_only}>
              {createOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
            </select>
          </label>
          <button type="button" onClick={createObject} disabled={session.read_only}>Crear</button>
        </div>
      </div>
      <Tabs.Root className="explorer-tabs" defaultValue="results">
        <Tabs.List aria-label="Vistas del explorador">
          <Tabs.Trigger value="results">Resultados</Tabs.Trigger>
          <Tabs.Trigger value="tree">Estructura</Tabs.Trigger>
          <Tabs.Trigger value="recent">Recientes</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="results" className="explorer-scroll">
          {search.isFetching && <p role="status" className="muted">Buscando en la versión observada…</p>}
          {search.isError && <p role="alert" className="notice warning">No se pudo buscar. El mundo no cambió.</p>}
          {!search.isFetching && !search.isError && hits.length === 0 && worldIsEmpty && !searchIsFiltered && <EmptyWorldActions readOnly={session.read_only} onStartProposal={onStartProposal} onEditorOpened={onEditorOpened} />}
          {!search.isFetching && !search.isError && hits.length === 0 && (!worldIsEmpty || searchIsFiltered) && <p className="empty-state">No hay coincidencias con estos términos y filtros. El contenido del mundo no se ocultó ni cambió.</p>}
          <div className="explorer-results" role="region" aria-label="Resultados de búsqueda" onKeyDown={onResultKeyDown}>
            {hits.map((result) => <ResultButton key={result.uri} result={result} selected={result.uri === state.selectedUri} onOpen={openObject} />)}
          </div>
        </Tabs.Content>
        <Tabs.Content value="tree" className="explorer-scroll">
          {tree.isPending && <p role="status" className="muted">Cargando estructura…</p>}
          {tree.isError && <p role="alert" className="notice warning">No se pudo cargar la estructura.</p>}
          {tree.data?.children.length === 0 && <EmptyWorldActions readOnly={session.read_only} onStartProposal={onStartProposal} onEditorOpened={onEditorOpened} />}
          {tree.data && <TreeNodes nodes={tree.data.children} selectedUri={state.selectedUri} onOpen={openObject} />}
        </Tabs.Content>
        <Tabs.Content value="recent" className="explorer-scroll">
          {state.recentUris.length === 0 && <p className="empty-state">Todavía no abriste objetos en esta sesión.</p>}
          <div className="explorer-results">
            {state.recentUris.map((uri) => (
              <button key={uri} type="button" className="explorer-object" aria-current={uri === state.selectedUri ? "true" : undefined} onClick={() => void openObject(uri)}>
                <strong>{nameForUri(tree.data ?? null, uri)}</strong>
                <small>{kindFromUri(uri)}</small>
              </button>
            ))}
          </div>
        </Tabs.Content>
      </Tabs.Root>
      <details className="technical-details explorer-advanced">
        <summary>Abrir identificador técnico</summary>
        <form onSubmit={(event) => { event.preventDefault(); if (directUri.trim()) void openObject(directUri.trim()); }}>
          <label>URI <input name="direct-uri" value={directUri} onChange={(event) => setDirectUri(event.currentTarget.value)} placeholder="nirmata://…" autoComplete="off" /></label>
          <button type="submit">Abrir</button>
        </form>
      </details>
    </div>
  );
}

function EmptyWorldActions({ readOnly, onStartProposal, onEditorOpened }: { readOnly: boolean; onStartProposal: () => void; onEditorOpened: () => void }) {
  function create(kind: SearchObjectKind) {
    void startCreatingObject(kind).then((opened) => { if (opened) onEditorOpened(); });
  }
  return (
    <section className="empty-state contextual-empty" aria-label="Mundo sin objetos">
      <h4>El mundo todavía no tiene objetos</h4>
      <p>Crea una pieza canónica manualmente o abre una propuesta revisable. Nada se genera automáticamente.</p>
      <div className="pending-actions">
        <button type="button" disabled={readOnly} onClick={() => create("entity")}>Crear entidad</button>
        <button type="button" className="secondary" disabled={readOnly} onClick={() => create("rule")}>Crear regla</button>
        <button type="button" className="secondary" disabled={readOnly} onClick={() => create("event")}>Crear evento</button>
        <button type="button" className="ghost" disabled={readOnly} onClick={onStartProposal}>Proponer con IA</button>
      </div>
    </section>
  );
}

function ResultButton({ result, selected, onOpen }: { result: SearchResult; selected: boolean; onOpen: (uri: string) => Promise<void> }) {
  return (
    <article className="explorer-object">
      <button type="button" data-result aria-current={selected ? "true" : undefined} onClick={() => void onOpen(result.uri)}>
        <strong>{cleanSnippet(result.snippet)}</strong>
        <span className="badge-row">
          <span className="badge kind">{kindLabel(result.object_type)}</span>
          <span className={`badge ${result.classification === "no_evidence" ? "warning" : "info"}`}>{classificationLabel(result.classification)}</span>
          <span className={`badge ${result.authority === "canonical" ? "ready" : "context"}`}>{result.authority === "canonical" ? "Canon" : "Perspectiva"}</span>
        </span>
      </button>
      <details className="technical-details">
        <summary>Detalles de coincidencia</summary>
        <small>Posición {result.rank} · puntuación {result.score.toFixed(3)} · {result.score_explanation}</small>
      </details>
    </article>
  );
}

function TreeNodes({ nodes, selectedUri, onOpen }: { nodes: LogicalVfsNode[]; selectedUri: string | null; onOpen: (uri: string) => Promise<void> }) {
  return (
    <ul className="explorer-tree">
      {nodes.map((node) => node.type === "directory" ? (
        <li key={`directory-${node.name}`}>
          <details open><summary>{node.name}</summary><TreeNodes nodes={node.children} selectedUri={selectedUri} onOpen={onOpen} /></details>
        </li>
      ) : (
        <li key={node.uri} className="explorer-tree-object">
          <button type="button" aria-current={node.uri === selectedUri ? "true" : undefined} onClick={() => void onOpen(node.uri)}>{node.name}</button>
          <details className="technical-details"><summary>Detalles</summary><code>{node.uri}</code></details>
        </li>
      ))}
    </ul>
  );
}

function nameForUri(tree: LogicalVfsDirectory | null, uri: string): string {
  function find(nodes: LogicalVfsNode[]): string | null {
    for (const node of nodes) {
      if (node.type === "object" && node.uri === uri) return node.name;
      if (node.type === "directory") {
        const match = find(node.children);
        if (match) return match;
      }
    }
    return null;
  }
  return tree ? find(tree.children) ?? "Objeto reciente" : "Objeto reciente";
}

function kindFromUri(uri: string): string {
  return kindLabel((uri.split("/")[2] ?? "entity") as SearchResult["object_type"]);
}

function cleanSnippet(value: string): string {
  return value.replace(/[\[\]]/g, "").trim() || "Objeto sin título";
}

function kindLabel(value: SearchResult["object_type"]): string {
  return ({ world: "Mundo", entity: "Entidad", relation: "Relación", event: "Evento", claim: "Afirmación", rule: "Regla", goal: "Meta", document: "Documento" })[value];
}

function classificationLabel(value: SearchResult["classification"]): string {
  return ({ fact: "Hecho", perspective: "Perspectiva", inference: "Inferencia", no_evidence: "Sin evidencia", unspecified: "No especificado" })[value];
}
