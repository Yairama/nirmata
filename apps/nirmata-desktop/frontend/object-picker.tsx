import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import { useQuery } from "@tanstack/react-query";
import { createContext, useContext, useDeferredValue, useState } from "react";
import type { ReactNode } from "react";
import { useSession } from "./session-provider.js";
import type { SearchObjectKind, SearchResult, SearchWorldResponse } from "./types.js";
import { observedScopeQueryKey } from "./workspace-data.js";

export type ObjectPickerRequest = {
  title: string;
  kinds: SearchObjectKind[];
  multiple: boolean;
  returnFocus: HTMLElement | null;
  apply: (results: SearchResult[]) => void;
  allowedUris?: string[];
};

const ObjectPickerContext = createContext<((request: ObjectPickerRequest) => void) | null>(null);

export function useObjectPicker() {
  const request = useContext(ObjectPickerContext);
  if (!request) throw new Error("ObjectPickerProvider is missing.");
  return request;
}

export function ObjectPickerProvider({ children }: { children: ReactNode }) {
  const session = useSession();
  const [request, setRequest] = useState<ObjectPickerRequest | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<SearchResult[]>([]);
  const deferredQuery = useDeferredValue(query.trim());
  const kind = request?.kinds.length === 1 ? request.kinds[0] : "all";
  const scopeKey = session
    ? observedScopeQueryKey(session)
    : ["world", "closed", "closed", "closed"] as const;
  const results = useQuery({
    queryKey: [...scopeKey, "object-picker", kind, deferredQuery],
    queryFn: () => invoke<SearchWorldResponse>("search_world", {
      input: { queryText: deferredQuery, kind, limit: 50 },
    }),
    enabled: Boolean(request && session && deferredQuery.length >= 1),
    retry: false,
    placeholderData: (previous) => previous,
  });

  function openPicker(next: ObjectPickerRequest) {
    setRequest(next);
    setQuery("");
    setSelected([]);
  }

  function close() {
    const focus = request?.returnFocus;
    setRequest(null);
    window.setTimeout(() => focus?.focus());
  }

  function choose(result: SearchResult) {
    if (!request) return;
    if (!request.multiple) {
      request.apply([result]);
      close();
      return;
    }
    setSelected((current) => current.some((item) => item.uri === result.uri)
      ? current.filter((item) => item.uri !== result.uri)
      : [...current, result]);
  }

  const allowed = (results.data?.hits ?? []).filter((result) => request?.kinds.includes(result.object_type as SearchObjectKind)
    && (!request.allowedUris || request.allowedUris.includes(result.uri)));
  return (
    <ObjectPickerContext.Provider value={openPicker}>
      {children}
      <Dialog.Root open={request !== null} onOpenChange={(open) => !open && close()}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="object-picker-dialog" aria-describedby="object-picker-description">
          <div className="dialog-heading">
            <div>
              <Dialog.Title>{request?.title ?? "Elegir objeto"}</Dialog.Title>
              <Dialog.Description id="object-picker-description">Busca por nombre o contenido en la versión observada.</Dialog.Description>
            </div>
            <Dialog.Close asChild><button type="button" className="ghost">Cerrar</button></Dialog.Close>
          </div>
          <label>Buscar
            <input autoFocus name="object-picker-search" autoComplete="off" type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Escribe para buscar…" />
          </label>
          <div className="object-picker-results" role="region" aria-label="Objetos disponibles">
            {results.isFetching && <p role="status" className="muted">Buscando…</p>}
            {results.isError && <p role="alert" className="notice warning">No se pudo buscar. El formulario se conservó.</p>}
            {!results.isFetching && deferredQuery && allowed.length === 0 && <p className="empty-state">No hay objetos compatibles.</p>}
            {allowed.map((result) => {
              const active = selected.some((item) => item.uri === result.uri);
              return (
                <button key={result.uri} type="button" className="object-picker-result" aria-pressed={request?.multiple ? active : undefined} onClick={() => choose(result)}>
                  <strong>{result.snippet.replace(/[\[\]]/g, "")}</strong>
                  <small>{kindLabel(result.object_type as SearchObjectKind)} · {result.authority === "canonical" ? "Canon" : "Perspectiva"}</small>
                </button>
              );
            })}
          </div>
          {request?.multiple && (
            <div className="dialog-actions object-picker-actions">
              <span>{selected.length} seleccionado{selected.length === 1 ? "" : "s"}</span>
              <button type="button" disabled={selected.length === 0} onClick={() => { request.apply(selected); close(); }}>Usar selección</button>
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
      </Dialog.Root>
    </ObjectPickerContext.Provider>
  );
}

function kindLabel(kind: SearchObjectKind): string {
  return ({ entity: "Entidad", relation: "Relación", event: "Evento", claim: "Afirmación", rule: "Regla", goal: "Meta", document: "Documento" })[kind];
}
