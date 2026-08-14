import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { showCommandError, showSuccess } from "./feedback.js";
import { pendingReviewsQueryKey } from "./pending-reviews.js";
import { dialog, invoke } from "./state.js";
import type { ExportSnapshotResult, ImportSnapshotResult } from "./types.js";
import { useSession } from "./session-provider.js";

function shortRevision(revision: string) {
  return revision.slice(0, 8);
}

export function SnapshotWorkspace({ onOpenReviews }: { onOpenReviews: () => void }) {
  const session = useSession();
  const queryClient = useQueryClient();
  const revision = session?.read_scope.revisionId ?? session?.active_variant.headRevisionId ?? "version";
  const [snapshotName, setSnapshotName] = useState(`backup-${shortRevision(revision)}`);
  const [parentDirectory, setParentDirectory] = useState("");
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exported, setExported] = useState<ExportSnapshotResult | null>(null);
  const [imported, setImported] = useState<ImportSnapshotResult | null>(null);
  if (!session) return null;

  const nameValid = /^[A-Za-z0-9_-]{1,80}$/.test(snapshotName);

  async function chooseExportDirectory() {
    try {
      const selected = await dialog.open({ multiple: false, directory: true });
      if (typeof selected === "string") setParentDirectory(selected);
    } catch (error) {
      showCommandError(error, { label: "Elegir carpeta", run: chooseExportDirectory });
    }
  }

  async function exportSnapshot() {
    if (!parentDirectory || !nameValid || exporting) return;
    setExporting(true);
    try {
      const result = await invoke<ExportSnapshotResult>("export_vfs_snapshot", {
        input: { parentDirectory, snapshotName },
      });
      setExported(result);
      showSuccess("Backup exportado", `${result.objectCount} objetos guardados en ${result.variant}.`);
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: exportSnapshot });
    } finally {
      setExporting(false);
    }
  }

  async function importFrom(snapshotDirectory: string) {
    if (importing) return;
    setImporting(true);
    try {
      const result = await invoke<ImportSnapshotResult>("import_vfs_snapshot", {
        input: { snapshotDirectory },
      });
      await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session!) });
      setImported(result);
      showSuccess("Snapshot preparado", "Revisa el resumen y abre Cambios para decidir cada operación.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: () => importFrom(snapshotDirectory) });
    } finally {
      setImporting(false);
    }
  }

  async function chooseImportDirectory() {
    try {
      const selected = await dialog.open({ multiple: false, directory: true });
      if (typeof selected === "string") await importFrom(selected);
    } catch (error) {
      showCommandError(error, { label: "Elegir snapshot", run: chooseImportDirectory });
    }
  }

  return (
    <section className="snapshot-workspace" aria-labelledby="snapshot-title">
      <p className="panel-eyebrow">Backup y traslado explícito</p>
      <h2 id="snapshot-title">Snapshot estructurado</h2>
      <p>Conserva objetos, relaciones, variante y revisión. A diferencia del lore, no extrae candidatos desde prosa.</p>
      <div className="snapshot-comparison">
        <article><h3>Lore</h3><p>Markdown o texto no confiable. Produce candidatos citados para decidir.</p></article>
        <article><h3>Snapshot</h3><p>Copia estructurada. Antes de aplicar muestra altas, cambios y eliminaciones.</p></article>
      </div>

      <div className="snapshot-actions-grid">
        <section className="snapshot-action-card" aria-labelledby="snapshot-export-title">
          <h3 id="snapshot-export-title">Crear backup</h3>
          <label>Nombre de la carpeta
            <input value={snapshotName} maxLength={80} aria-invalid={!nameValid} onChange={(event) => setSnapshotName(event.target.value)} />
          </label>
          {!nameValid && <p className="creation-error" role="alert">Usa letras, números, guion o guion bajo, sin espacios.</p>}
          <button type="button" className="secondary" onClick={chooseExportDirectory}>Elegir carpeta…</button>
          {parentDirectory && <p className="path snapshot-path">Destino: {parentDirectory}</p>}
          <button type="button" disabled={!parentDirectory || !nameValid || exporting} onClick={exportSnapshot}>
            {exporting ? "Exportando…" : "Exportar backup"}
          </button>
          {exported && (
            <article className="snapshot-result" aria-label="Resumen del backup exportado">
              <strong>Backup listo</strong>
              <dl className="settings-facts">
                <div><dt>Mundo</dt><dd>{session.world.name}</dd></div>
                <div><dt>Variante</dt><dd>{exported.variant}</dd></div>
                <div><dt>Contenido</dt><dd>{exported.objectCount} objetos</dd></div>
              </dl>
              <details><summary>Detalles técnicos</summary><p className="path">{exported.path}</p><dl className="technical-facts"><div><dt>Revisión</dt><dd>{exported.baseRevision}</dd></div><div><dt>Hash</dt><dd>{exported.logicalHash}</dd></div></dl></details>
            </article>
          )}
        </section>

        <section className="snapshot-action-card" aria-labelledby="snapshot-import-title">
          <h3 id="snapshot-import-title">Importar y revisar</h3>
          <p>Valida la identidad y calcula el diff. No modifica el canon hasta que uses <strong>Aplicar al mundo</strong>.</p>
          {session.read_only && <p className="notice warning">Estás viendo una versión anterior. Vuelve a la versión actual para importar.</p>}
          <button type="button" disabled={session.read_only || importing} onClick={chooseImportDirectory}>
            {importing ? "Validando snapshot…" : "Elegir snapshot…"}
          </button>
          {imported && (
            <article className="snapshot-result" aria-label="Resumen del snapshot importado">
              <strong>Comparación preparada</strong>
              <dl className="settings-facts">
                <div><dt>Mundo</dt><dd>{session.world.name}</dd></div>
                <div><dt>Variante</dt><dd>{imported.variant}</dd></div>
                <div><dt>Diff</dt><dd>{imported.createdCount} altas, {imported.updatedCount} cambios, {imported.deletedCount} bajas</dd></div>
              </dl>
              <ul className="snapshot-diff-preview">
                {imported.review.operations.slice(0, 6).map((operation) => (
                  <li key={operation.operationId}>
                    <span className="badge">{operation.before ? operation.after ? "Cambio" : "Baja" : "Alta"}</span>
                    {operation.after?.title ?? operation.before?.title ?? "Objeto"}
                  </li>
                ))}
              </ul>
              {imported.review.operations.length > 6 && <p className="muted">Y {imported.review.operations.length - 6} operaciones más.</p>}
              <button type="button" onClick={onOpenReviews}>Abrir revisión</button>
              <details><summary>Detalles técnicos</summary><p className="path">{imported.path}</p><dl className="technical-facts"><div><dt>Revisión base</dt><dd>{imported.baseRevision}</dd></div><div><dt>Hash</dt><dd>{imported.logicalHash}</dd></div></dl></details>
            </article>
          )}
        </section>
      </div>
    </section>
  );
}
