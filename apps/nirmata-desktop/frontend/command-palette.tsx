import { Command } from "cmdk";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";
import type { SearchResult } from "./types.js";
import { chipStyles } from "./ui-styles.js";

type PaletteAction = {
  id: string;
  label: string;
  group: "Navegar" | "Trabajar" | "Aplicación";
  keywords?: string[];
  disabled?: boolean;
  run: () => void;
};

export function CommandPalette({
  open,
  onOpenChange,
  actions,
  returnFocus,
  query,
  onQueryChange,
  results,
  searching,
  searchError,
  onSelectResult,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: PaletteAction[];
  returnFocus: HTMLElement | null;
  query: string;
  onQueryChange: (query: string) => void;
  results: SearchResult[];
  searching: boolean;
  searchError: boolean;
  onSelectResult: (result: SearchResult) => Promise<boolean>;
}) {
  const wasOpen = useRef(false);

  useEffect(() => {
    if (isTauri()) return;
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        onOpenChange(!open);
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onOpenChange, open]);

  useEffect(() => {
    if (wasOpen.current && !open) returnFocus?.focus();
    wasOpen.current = open;
  }, [open, returnFocus]);

  function select(action: PaletteAction) {
    if (action.disabled) return;
    onOpenChange(false);
    action.run();
  }

  async function selectResult(result: SearchResult) {
    onOpenChange(false);
    await onSelectResult(result);
  }

  const hasWorldQuery = query.trim().length >= 2;

  return (
    <Command.Dialog
      open={open}
      onOpenChange={onOpenChange}
      label="Buscar objetos y acciones"
      className="command-dialog fixed left-1/2 top-[15dvh] z-50 max-h-[70dvh] w-[min(42rem,calc(100vw-2rem))] -translate-x-1/2 translate-y-0 overflow-auto rounded-2xl border border-line bg-raised p-0 shadow-overlay outline-none [&>*+*]:mt-4"
      overlayClassName="dialog-overlay command-overlay fixed inset-0 z-40 bg-overlay"
    >
      <Command.Input
        autoFocus
        value={query}
        onValueChange={onQueryChange}
        placeholder="Busca objetos o escribe una acción…"
        aria-label="Buscar objetos y acciones"
        className="rounded-none border-0 border-b border-line px-5 py-4 text-lg shadow-none"
      />
      <Command.List className="max-h-[52dvh] overflow-auto p-2">
        <Command.Empty>{hasWorldQuery && !searching ? "No hay objetos ni acciones con ese nombre." : "No hay acciones con ese nombre."}</Command.Empty>
        {hasWorldQuery && (
          <Command.Group heading="Objetos" forceMount>
            {searching && <Command.Loading>Buscando en la versión observada…</Command.Loading>}
            {searchError && <div className="command-search-state p-5 text-sm text-muted" role="alert">No se pudo buscar. El mundo no cambió.</div>}
            {!searching && !searchError && results.length === 0 && (
              <div className="command-search-state p-5 text-sm text-muted">Sin evidencia con estos términos en la versión observada.</div>
            )}
            {results.map((result) => (
              <Command.Item
                key={result.uri}
                value={`${result.snippet} ${result.object_type} ${result.classification}`}
                forceMount
                onSelect={() => void selectResult(result)}
                className="cursor-pointer rounded-xl px-3 py-3 data-[selected=true]:bg-accent-soft data-[selected=true]:text-accent"
              >
                <span className="command-result-copy grid">
                  <strong>{cleanSnippet(result.snippet)}</strong>
                  <small>{resultLabel(result.object_type)} · {classificationLabel(result.classification)}</small>
                </span>
                <span className={chipStyles({ tone: result.authority === "canonical" ? "success" : "perspective" })}>
                  {result.authority === "canonical" ? "Canon" : "Perspectiva"}
                </span>
              </Command.Item>
            ))}
          </Command.Group>
        )}
        {(["Navegar", "Trabajar", "Aplicación"] as const).map((group) => (
          <Command.Group key={group} heading={group}>
            {actions.filter((action) => action.group === group).map((action) => (
              <Command.Item
                key={action.id}
                value={`${action.label} ${(action.keywords ?? []).join(" ")}`}
                disabled={action.disabled}
                onSelect={() => select(action)}
                className="cursor-pointer rounded-xl px-3 py-3 data-[selected=true]:bg-accent-soft data-[selected=true]:text-accent"
              >
                <span>{action.label}</span>
                {action.disabled && <small>Solo lectura</small>}
              </Command.Item>
            ))}
          </Command.Group>
        ))}
      </Command.List>
    </Command.Dialog>
  );
}

function cleanSnippet(value: string): string {
  return value.replace(/[\[\]]/g, "").trim() || "Objeto sin título";
}

function resultLabel(value: SearchResult["object_type"]): string {
  const labels: Record<SearchResult["object_type"], string> = {
    world: "Mundo",
    entity: "Entidad",
    relation: "Relación",
    event: "Evento",
    claim: "Afirmación",
    rule: "Regla",
    goal: "Meta",
    document: "Documento",
  };
  return labels[value];
}

function classificationLabel(value: SearchResult["classification"]): string {
  const labels: Record<SearchResult["classification"], string> = {
    fact: "Hecho",
    perspective: "Perspectiva",
    inference: "Inferencia",
    no_evidence: "Sin evidencia",
    unspecified: "No especificado",
  };
  return labels[value];
}

export type { PaletteAction };
