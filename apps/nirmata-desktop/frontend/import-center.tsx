import * as Tabs from "@radix-ui/react-tabs";
import { useEffect, useState } from "react";
import { useSession } from "./session-provider.js";
import { LoreImportWorkspace } from "./lore-import-workspace.js";
import { SnapshotWorkspace } from "./snapshot-workspace.js";

export function ImportCenter({ active, intent, onOpenReviews }: { active: boolean; intent: { id: number; tab: "lore" | "snapshot" } | null; onOpenReviews: () => void }) {
  const session = useSession();
  const [tab, setTab] = useState("lore");
  useEffect(() => {
    if (intent) setTab(intent.tab);
  }, [intent?.id]);
  if (!session) return null;
  return (
    <div className="import-center">
      <header className="import-center-heading">
        <div><p className="panel-eyebrow">Fuentes y copias estructuradas</p><h1 id="imports-page-title">Importaciones</h1><p>Convierte textos en elementos revisables o restaura una copia estructurada.</p></div>
      </header>
      <Tabs.Root className="import-tabs" value={tab} onValueChange={setTab}>
        <Tabs.List aria-label="Tipos de importación">
          <Tabs.Trigger value="lore">Textos</Tabs.Trigger>
          <Tabs.Trigger value="snapshot">Copias de seguridad</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="lore" className="import-tab-content">
          <LoreImportWorkspace launchId={intent?.tab === "lore" ? intent.id : null} />
        </Tabs.Content>
        <Tabs.Content value="snapshot" className="import-tab-content">
          <SnapshotWorkspace onOpenReviews={onOpenReviews} />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}
