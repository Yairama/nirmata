import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import { useQuery } from "@tanstack/react-query";
import { createContext, useContext, useDeferredValue, useState } from "react";
import type { ReactNode } from "react";
import { useSession } from "./session-provider.js";
import type { SearchObjectKind, SearchResult, SearchWorldResponse } from "./types.js";
import { buttonStyles, cn, noticeStyles } from "./ui-styles.js";
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
        <Dialog.Overlay className="dialog-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="object-picker-dialog fixed left-1/2 top-1/2 z-50 max-h-[calc(100dvh-2rem)] w-[min(42rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-2xl border border-line bg-raised p-6 shadow-overlay outline-none [&>*+*]:mt-4" aria-describedby="object-picker-description">
          <div className="dialog-heading flex items-start justify-between gap-4 border-b border-line pb-4">
            <div>
              <Dialog.Title>{request?.title ?? "Elegir objeto"}</Dialog.Title>
              <Dialog.Description id="object-picker-description">Busca por nombre o contenido en la versión observada.</Dialog.Description>
            </div>
            <Dialog.Close asChild><button type="button" className={buttonStyles({ variant: "ghost" })}>Cerrar</button></Dialog.Close>
          </div>
          <label>Buscar
            <input autoFocus name="object-picker-search" autoComplete="off" type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Escribe para buscar…" />
          </label>
          <div className="object-picker-results grid max-h-[45dvh] gap-1 overflow-auto" role="region" aria-label="Objetos disponibles">
            {results.isFetching && <p role="status" className="muted text-muted">Buscando…</p>}
            {results.isError && <p role="alert" className={noticeStyles({ tone: "warning" })}>No se pudo buscar. El formulario se conservó.</p>}
            {!results.isFetching && deferredQuery && allowed.length === 0 && <p className="empty-state grid gap-4 rounded-xl border border-dashed border-line bg-surface p-5 text-sm text-muted">No hay objetos compatibles.</p>}
            {allowed.map((result) => {
              const active = selected.some((item) => item.uri === result.uri);
              return (
                <button key={result.uri} type="button" className={cn("object-picker-result grid w-full justify-stretch gap-1 rounded-xl border border-transparent bg-transparent p-3 text-left text-ink hover:border-line hover:bg-subtle", active && "border-accent bg-accent-soft text-accent")} aria-pressed={request?.multiple ? active : undefined} onClick={() => choose(result)}>
                  <strong>{result.snippet.replace(/[\[\]]/g, "")}</strong>
                  <small>{kindLabel(result.object_type as SearchObjectKind)} · {result.authority === "canonical" ? "Canon" : "Perspectiva"}</small>
                </button>
              );
            })}
          </div>
          {request?.multiple && (
            <div className="dialog-actions object-picker-actions flex flex-wrap items-center justify-between gap-2">
              <span>{selected.length} seleccionado{selected.length === 1 ? "" : "s"}</span>
              <button type="button" className={buttonStyles()} disabled={selected.length === 0} onClick={() => { request.apply(selected); close(); }}>Usar selección</button>
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
