import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import { useQuery } from "@tanstack/react-query";
import { useDeferredValue, useEffect, useState } from "react";
import { useSession } from "./session-provider.js";
import type { SearchObjectKind, SearchResult, SearchWorldResponse } from "./types.js";

type ObjectPickerRequest = {
  title: string;
  kinds: SearchObjectKind[];
  multiple: boolean;
  returnFocus: HTMLElement | null;
  apply: (results: SearchResult[]) => void;
};

export function requestObjectPicker(request: ObjectPickerRequest): void {
  window.dispatchEvent(new CustomEvent("nirmata:pick-object", { detail: request }));
}

export function ObjectPicker() {
  const session = useSession();
  const [request, setRequest] = useState<ObjectPickerRequest | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<SearchResult[]>([]);
  const deferredQuery = useDeferredValue(query.trim());
  const kind = request?.kinds.length === 1 ? request.kinds[0] : "all";
  const scopeKey = session
    ? [session.world_id, session.active_variant.id, session.read_scope.revisionId ?? session.current_revision]
    : ["closed", "closed", "closed"];
  const results = useQuery({
    queryKey: ["world", ...scopeKey, "object-picker", kind, deferredQuery],
    queryFn: () => invoke<SearchWorldResponse>("search_world", {
      input: { queryText: deferredQuery, kind, limit: 50 },
    }),
    enabled: Boolean(request && session && deferredQuery.length >= 1),
    retry: false,
    placeholderData: (previous) => previous,
  });

  useEffect(() => {
    function onRequest(event: Event) {
      setRequest((event as CustomEvent<ObjectPickerRequest>).detail);
      setQuery("");
      setSelected([]);
    }
    window.addEventListener("nirmata:pick-object", onRequest);
    return () => window.removeEventListener("nirmata:pick-object", onRequest);
  }, []);

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

  const allowed = (results.data?.hits ?? []).filter((result) => request?.kinds.includes(result.object_type as SearchObjectKind));
  return (
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
            <input autoFocus type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Escribe para buscar…" />
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
  );
}

function kindLabel(kind: SearchObjectKind): string {
  return ({ entity: "Entidad", relation: "Relación", event: "Evento", claim: "Afirmación", rule: "Regla", goal: "Meta", document: "Documento" })[kind];
}
