import * as Tabs from "@radix-ui/react-tabs";
import { useEffect, useState } from "react";
import { useSession } from "./session-provider.js";
import { LoreImportWorkspace } from "./lore-import-workspace.js";
import { SnapshotWorkspace } from "./snapshot-workspace.js";
import { buttonStyles, cn } from "./ui-styles.js";

export function ImportCenter({ active, intent, onOpenReviews }: { active: boolean; intent: { id: number; tab: "lore" | "snapshot" } | null; onOpenReviews: () => void }) {
  const session = useSession();
  const [tab, setTab] = useState("lore");
  useEffect(() => {
    if (intent) setTab(intent.tab);
  }, [intent?.id]);
  if (!session) return null;
  return (
    <div className="import-center bg-canvas p-5 lg:p-7">
      <header className="import-center-heading flex items-start justify-between gap-4">
        <div><p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Fuentes y copias estructuradas</p><h1 id="imports-page-title">Importaciones</h1><p>Convierte textos en elementos revisables o restaura una copia estructurada.</p></div>
      </header>
      <Tabs.Root className="import-tabs flex min-h-0 flex-col" value={tab} onValueChange={setTab}>
        <Tabs.List className="flex gap-1 overflow-x-auto pb-1" aria-label="Tipos de importación">
          <Tabs.Trigger className={cn(buttonStyles({ size: "compact" }), "shrink-0 rounded-full px-3 data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent")} value="lore">Textos</Tabs.Trigger>
          <Tabs.Trigger className={cn(buttonStyles({ size: "compact" }), "shrink-0 rounded-full px-3 data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent")} value="snapshot">Copias de seguridad</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="lore" className="import-tab-content mt-4">
          <LoreImportWorkspace launchId={intent?.tab === "lore" ? intent.id : null} />
        </Tabs.Content>
        <Tabs.Content value="snapshot" className="import-tab-content mt-4">
          <SnapshotWorkspace onOpenReviews={onOpenReviews} />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}
