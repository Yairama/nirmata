import * as Tabs from "@radix-ui/react-tabs";
import { useEffect, useState } from "react";
import { exportVfsSnapshotButton, importVfsSnapshotButton } from "./state.js";
import { useSession } from "./session-provider.js";

export function ImportCenter({ active }: { active: boolean }) {
  const session = useSession();
  const [tab, setTab] = useState("lore");
  useEffect(() => {
    const panel = document.querySelector<HTMLElement>("#lore-import-panel");
    if (panel) panel.hidden = !active || tab !== "lore";
  }, [active, tab]);
  if (!session) return null;
  return (
    <div className="import-center">
      <header className="import-center-heading">
        <div><p className="panel-eyebrow">Fuentes y copias estructuradas</p><h1 id="imports-page-title">Importaciones</h1><p>Elige según el material: texto para extraer candidatos, snapshot para revisar una copia estructurada de Nirmata.</p></div>
      </header>
      <Tabs.Root className="import-tabs" value={tab} onValueChange={setTab}>
        <Tabs.List aria-label="Tipos de importación">
          <Tabs.Trigger value="lore">Lore</Tabs.Trigger>
          <Tabs.Trigger value="snapshot">Snapshot</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="lore" className="import-tab-content">
          <p className="muted">Selecciona material debajo y continúa el lote hasta abrir su revisión estándar.</p>
        </Tabs.Content>
        <Tabs.Content value="snapshot" className="import-tab-content">
          <section className="snapshot-workspace">
            <p className="panel-eyebrow">Backup y traslado explícito</p>
            <h2>Snapshot estructurado</h2>
            <p>Un snapshot conserva objetos, relaciones, variante y revisión. A diferencia de un archivo de lore, no extrae candidatos desde prosa.</p>
            <div className="snapshot-comparison">
              <article><h3>Lore</h3><p>Markdown o texto no confiable. Produce candidatos citados para decidir.</p></article>
              <article><h3>Snapshot</h3><p>Copia estructurada. Antes de aplicar muestra altas, cambios y eliminaciones en revisión.</p></article>
            </div>
            {session.read_only && <p className="notice warning">Estás viendo una versión anterior. Vuelve a la versión actual para importar un snapshot.</p>}
            <div className="dialog-actions">
              <button type="button" className="secondary" onClick={() => exportVfsSnapshotButton.click()}>Exportar backup…</button>
              <button type="button" disabled={session.read_only} onClick={() => importVfsSnapshotButton.click()}>Importar y revisar…</button>
            </div>
            <p className="muted">Importar nunca escribe directamente: prepara un conjunto de cambios para la cola global.</p>
          </section>
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}
