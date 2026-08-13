import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

const variant = {
  id: "30000000-0000-4000-8000-000000000001",
  worldId: "10000000-0000-4000-8000-000000000001",
  name: "Canon principal",
  headRevisionId: "20000000-0000-4000-8000-000000000001",
  archived: false,
  createdFromRevisionId: "20000000-0000-4000-8000-000000000001",
  createdAtMs: 1725000000000,
};

const session = {
  path: "C:/Fixtures/mundo-vacio.nirmata",
  world_id: variant.worldId,
  current_revision: variant.headRevisionId,
  world: {
    id: variant.worldId,
    name: "Mundo vacío",
    premise_md: "",
    epoch_label: "",
    current_revision: variant.headRevisionId,
    created_at_ms: 1725000000000,
    updated_at_ms: 1725000000000,
  },
  active_variant: variant,
  read_scope: { variantId: variant.id, revisionId: null },
  read_only: false,
};

const alternateVariant = {
  ...variant,
  id: "30000000-0000-4000-8000-000000000002",
  name: "Línea alternativa",
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ sessionValue, variantValue, alternateVariantValue }) => {
    let callbackId = 1;
    let activeVariant = structuredClone(variantValue);
    const commands: Array<{ command: string; args: unknown }> = [];
    Object.assign(window, {
      __nirmataCommands: commands,
      __TAURI_INTERNALS__: {
        transformCallback(callback: (...args: unknown[]) => void) {
          const id = callbackId++;
          Object.assign(window, { [`_${id}`]: callback });
          return id;
        },
        unregisterCallback(id: number) {
          delete (window as unknown as Record<string, unknown>)[`_${id}`];
        },
        async invoke(command: string, args?: { input?: { variantId?: string } }) {
          commands.push({ command, args });
          if (command.startsWith("plugin:event|")) return 1;
          if (command === "get_current_world") {
            const current = structuredClone(sessionValue);
            current.active_variant = structuredClone(activeVariant);
            current.read_scope.variantId = activeVariant.id;
            if (localStorage.getItem("nirmata.fixture.readOnly") === "true") {
              return {
                ...current,
                read_only: true,
                read_scope: { ...current.read_scope, revisionId: current.current_revision },
              };
            }
            return current;
          }
          if (command === "get_ai_activity") return { busy: false, requestIds: [] };
          if (command === "get_ai_provider_status") return {
            state: "credential_missing",
            message: "La IA no está configurada.",
            canCheckConnection: false,
            connected: false,
            credential: { configured: false, source: "missing", persistence: "none", secureStoreAvailable: true, limitation: null },
          };
          if (command === "search_world") {
            if (localStorage.getItem("nirmata.fixture.error") === "true") {
              throw new Error("El mundo no cambió; puedes reintentar.");
            }
            if (localStorage.getItem("nirmata.fixture.paletteSearch") === "true") {
              return {
                hits: [{
                  object_ref: { entity: "40000000-0000-4000-8000-000000000001" },
                  object_type: "entity",
                  object_id: "40000000-0000-4000-8000-000000000001",
                  uri: "nirmata://entity/40000000-0000-4000-8000-000000000001",
                  snippet: "[Archivo] de la Aurora",
                  authority: "canonical",
                  classification: "fact",
                  provenance: "fts5",
                  stage: "search",
                  score: 1,
                  rank: 1,
                  score_explanation: "exact",
                }],
                absence: null,
              };
            }
            if (localStorage.getItem("nirmata.fixture.explorerLoaded") === "true") {
              return {
                hits: Array.from({ length: 200 }, (_, index) => ({
                  object_ref: { entity: `40000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}` },
                  object_type: "entity",
                  object_id: `40000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
                  uri: `nirmata://entity/40000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
                  snippet: `[Archivo ${index + 1}]`,
                  authority: "canonical",
                  classification: "fact",
                  provenance: "fts5",
                  stage: "search",
                  score: 1 - index / 1000,
                  rank: index + 1,
                  score_explanation: "coincidencia exacta",
                })),
                absence: null,
              };
            }
            return { hits: [], absence: { classification: "no_evidence", provenance: "search_world" } };
          }
          if (command === "open_uri") {
            return {
              result: {
                object_ref: { entity: "40000000-0000-4000-8000-000000000001" },
                object_type: "entity",
                object_id: "40000000-0000-4000-8000-000000000001",
                uri: "nirmata://entity/40000000-0000-4000-8000-000000000001",
                snippet: "Archivo de la Aurora",
                authority: "canonical",
                classification: "fact",
                provenance: "uri",
                stage: "selection",
                score: 1,
                rank: 1,
                score_explanation: "exact",
              },
              object: {
                entity: {
                  id: "40000000-0000-4000-8000-000000000001",
                  world_id: variantValue.worldId,
                  kind: "place",
                  name: "Archivo de la Aurora",
                  slug: "archivo-aurora",
                  aliases: [],
                  summary: "Custodia memorias minerales.",
                  body_md: "",
                  attributes_json: "{}",
                  version: 1,
                  created_at_ms: 1725000000000,
                  updated_at_ms: 1725000000000,
                },
              },
            };
          }
          if (command === "get_related_context") {
            if (localStorage.getItem("nirmata.fixture.explorerLoaded") === "true") {
              const related = (snippet: string, object_type: string, id: string, authority: string, classification: string) => ({
                result: {
                  object_ref: { [object_type]: id }, object_type, object_id: id,
                  uri: `nirmata://${object_type}/${id}`, snippet, authority, classification,
                  provenance: "context", stage: "relation", score: 1, rank: 1,
                  score_explanation: "relación estructurada",
                },
                stage: "relation",
              });
              return {
                canon: [related("Regla de memoria", "rule", "50000000-0000-4000-8000-000000000001", "canonical", "fact")],
                perspectives: [related("Rumor del archivo", "claim", "50000000-0000-4000-8000-000000000002", "perspective", "perspective")],
                desires: [related("Proteger el archivo", "goal", "50000000-0000-4000-8000-000000000003", "canonical", "fact")],
                obligations: [related("Custodiar las memorias", "rule", "50000000-0000-4000-8000-000000000004", "canonical", "fact")],
                search_evidence: [], usage: { max_objects: 24, max_chars: 4000, used_objects: 4, used_chars: 120 }, absence: null,
              };
            }
            return { canon: [], perspectives: [], desires: [], obligations: [], search_evidence: [], usage: { max_objects: 24, max_chars: 4000, used_objects: 0, used_chars: 0 }, absence: null };
          }
          if (command === "read_logical_vfs") {
            if (localStorage.getItem("nirmata.fixture.explorerLoaded") === "true") {
              return {
                name: "/",
                children: [{
                  type: "directory",
                  name: "Entidades",
                  children: [{
                    type: "object",
                    name: localStorage.getItem("nirmata.fixture.renamed") === "true" ? "Archivo renombrado" : "Archivo 1",
                    object: { entity: "40000000-0000-4000-8000-000000000001" },
                    uri: "nirmata://entity/40000000-0000-4000-8000-000000000001",
                  }],
                }],
              };
            }
            return { name: "/", children: [] };
          }
          if (command === "list_timeline_events") {
            if (localStorage.getItem("nirmata.fixture.timelineLoaded") === "true") {
              const time = (kind: string, start_tick: number | null, certainty = "certain") => ({ kind, start_tick, end_tick: null, precision: start_tick === null ? "unknown" : "exact", certainty });
              return {
                known: [
                  { uri: "nirmata://event/60000000-0000-4000-8000-000000000001", summary: "Fundación del Archivo", kind: "founding", time: time("instant", 10), startCalendar: { tick: 10, label: "Día 11", dateInput: "1|1|11|0" }, endCalendar: null },
                  { uri: "nirmata://event/60000000-0000-4000-8000-000000000002", summary: "Custodia prolongada", kind: "guardianship", time: time("ongoing", 20, "approximate"), startCalendar: { tick: 20, label: "Día 21", dateInput: "1|1|21|0" }, endCalendar: null },
                ],
                unknown: [{ uri: "nirmata://event/60000000-0000-4000-8000-000000000003", summary: "Pérdida sin fecha", kind: "loss", time: time("unknown", null), startCalendar: null, endCalendar: null }],
              };
            }
            return { known: [], unknown: [] };
          }
          if (command === "list_revision_history") return { currentHeadRevisionId: variantValue.headRevisionId, undoTargetRevisionId: null, revisions: [] };
          if (command === "list_variants") return [structuredClone(variantValue), structuredClone(alternateVariantValue)];
          if (command === "switch_variant") {
            activeVariant = args?.input?.variantId === alternateVariantValue.id
              ? structuredClone(alternateVariantValue)
              : structuredClone(variantValue);
            const current = structuredClone(sessionValue);
            current.active_variant = structuredClone(activeVariant);
            current.read_scope = { variantId: activeVariant.id, revisionId: null };
            return current;
          }
          if (command === "list_simulation_scenarios") return [];
          throw new Error(`Unhandled Tauri command: ${command}`);
        },
      },
    });
  }, { sessionValue: session, variantValue: variant, alternateVariantValue: alternateVariant });
});

test("workspace themes are distinct, accessible and screenshotable", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  for (const theme of ["light", "dark", "high-contrast"]) {
    await page.goto("/");
    await page.evaluate((value) => localStorage.setItem("nirmata.appearance.theme", value), theme);
    await page.reload();
    await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeVisible();
    await expect(page.locator(".open-shell-topbar").getByText("Escribiendo en", { exact: true })).toBeVisible();
    await expect(page.locator(".open-shell-topbar")).not.toContainText(variant.id);
    await expect(page.locator("#status")).toHaveText("Navegación actualizada.");
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);

    const accessibility = await new AxeBuilder({ page }).include("main").analyze();
    const blocking = accessibility.violations.filter((violation) =>
      violation.id === "color-contrast" || violation.impact === "serious" || violation.impact === "critical"
    );
    expect(blocking).toEqual([]);
    if (theme === "high-contrast") {
      await expect(page.locator(".open-shell-topbar")).toHaveCSS("border-bottom-width", "2px");
    }
    await page.screenshot({ path: testInfo.outputPath(`workspace-${theme}-1440x900.png`) });
  }
});

test("forced colors preserves workspace controls and focus", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await expect(page.locator("#world-view")).toBeVisible();
  const search = page.locator(".world-explorer-search");
  await search.focus();
  await expect(search).toBeFocused();
  const styles = await search.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      borderWidth: style.borderTopWidth,
      borderStyle: style.borderTopStyle,
      outlineWidth: style.outlineWidth,
      opacity: style.opacity,
    };
  });
  expect(styles.borderWidth).toBe("2px");
  expect(styles.borderStyle).not.toBe("none");
  expect(styles.outlineWidth).toBe("2px");
  expect(styles.opacity).toBe("1");
});

test("read-only and error states keep semantic contrast", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("nirmata.fixture.readOnly", "true");
    localStorage.setItem("nirmata.fixture.error", "true");
  });
  await page.goto("/");
  await expect(page.getByText("Solo lectura", { exact: true })).toBeVisible();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await expect(page.locator("#world-view")).toBeVisible();
  await expect(page.getByRole("alert", { name: "" }).filter({ hasText: "No se pudo buscar" })).toBeVisible();
  const accessibility = await new AxeBuilder({ page }).include("#world-view").analyze();
  const blocking = accessibility.violations.filter((violation) =>
    violation.id === "color-contrast" || violation.impact === "serious" || violation.impact === "critical"
  );
  expect(blocking).toEqual([]);
});

test("shell routes one area at a time and preserves inactive forms", async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 680 });
  await page.goto("/");
  const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
  await navigation.getByRole("button", { name: "Simulación", exact: true }).click();
  await expect(page.locator("#simulation-panel")).toBeVisible();
  await expect(page.locator("#workspace-shell")).toBeHidden();
  await page.locator("#simulation-assumptions").fill("El puerto sigue abierto.");

  await navigation.getByRole("button", { name: "Importaciones", exact: true }).click();
  await expect(page.locator("#lore-import-panel")).toBeVisible();
  await expect(page.locator("#simulation-panel")).toBeHidden();
  await navigation.getByRole("button", { name: "Simulación", exact: true }).click();
  await expect(page.locator("#simulation-assumptions")).toHaveValue("El puerto sigue abierto.");

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBe(0);
});

test("shell remains operable without horizontal overflow at required narrow sizes", async ({ page }, testInfo) => {
  for (const viewport of [{ width: 720, height: 520 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport);
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeVisible();
    const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
    await navigation.getByRole("button", { name: "Importaciones", exact: true }).click();
    await expect(page.locator("#lore-import-panel")).toBeVisible();
    await expect(page.locator("#variant-bar")).toBeHidden();
    await expect(page.locator("#simulation-panel")).toBeHidden();
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow).toBe(0);
    await page.screenshot({ path: testInfo.outputPath(`shell-imports-${viewport.width}x${viewport.height}.png`), fullPage: true });
  }
});

test("Import center distinguishes lore from structured snapshots", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Importaciones", exact: true }).click();
  const center = page.locator("#imports-panel");
  await expect(page.locator("#lore-import-panel")).toBeVisible();
  await center.getByRole("tab", { name: "Snapshot" }).click();
  await expect(page.locator("#lore-import-panel")).toBeHidden();
  await expect(center.getByText("Copia estructurada", { exact: false }).first()).toBeVisible();
  await expect(center.getByRole("button", { name: "Importar y revisar…" })).toBeEnabled();
  await center.getByRole("tab", { name: "Lore" }).click();
  await expect(page.locator("#lore-import-panel")).toBeVisible();
});

test("command palette opens by keyboard, filters actions and restores focus", async ({ page }) => {
  await page.goto("/");
  const trigger = page.getByRole("button", { name: /Buscar Ctrl K/u });
  await trigger.focus();
  const started = await page.evaluate(() => performance.now());
  await page.keyboard.press("Control+k");
  const input = page.getByRole("dialog", { name: "Buscar objetos y acciones" }).getByRole("combobox");
  await expect(input).toBeFocused();
  const elapsed = await page.evaluate((start) => performance.now() - start, started);
  expect(elapsed).toBeLessThan(100);

  await input.fill("simulación");
  await page.keyboard.press("Enter");
  await expect(page.locator("#simulation-panel")).toBeVisible();
  await page.keyboard.press("Control+k");
  await expect(input).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
});

test("command palette searches exact world objects and starts guarded creation", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.setItem("nirmata.fixture.paletteSearch", "true"));
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  const input = palette.getByRole("combobox");
  await input.fill("aurora");
  const result = palette.getByRole("option", { name: /Archivo de la Aurora/u });
  await expect(result).toContainText("Canon");
  await expect(result).not.toContainText("fts5");
  await result.click();
  await expect(page.locator("#editor-title")).toHaveText("Archivo de la Aurora");
  await expect(page.locator("#editor-subtitle")).not.toContainText("40000000");
  await expect(page.getByRole("combobox", { name: "Tipo" })).toHaveValue("place");
  await expect(page.getByRole("combobox", { name: "Tipo" }).locator('option:checked')).toHaveText("Lugar");
  await expect(page.getByText("Opciones avanzadas")).toBeVisible();
  await expect(page.getByText("Atributos JSON")).toBeHidden();

  await page.keyboard.press("Control+k");
  await input.fill("crear evento");
  await palette.getByRole("option", { name: /^Crear evento$/u }).click();
  await expect(page.locator("#editor-title")).toHaveText("Nuevo evento");
});

test("command palette changes the writing variant through the guarded workflow", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("línea alternativa");
  await palette.getByRole("option", { name: "Escribir en Línea alternativa" }).click();
  await expect(page.locator(".scope-summary")).toContainText("Línea alternativa");
  await expect(page.locator("#status")).toHaveText("Navegación actualizada.");
});

test("React explorer renders 200 results, hides IDs and preserves selection after rename", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.explorerLoaded", "true"));
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  const explorer = page.locator("#navigation-panel");
  await expect(explorer.locator("button[data-result]")).toHaveCount(200);
  await expect(page.locator("#editor-title")).toHaveText("Archivo de la Aurora");
  const visibleCopy = await explorer.evaluate((element) => (element as HTMLElement).innerText);
  expect(visibleCopy).not.toContain("nirmata://");
  expect(visibleCopy).not.toContain("fts5");
  const accessibility = await new AxeBuilder({ page }).include("#navigation-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("explorer-200-results-1440x900.png") });

  const paintMs = await explorer.evaluate(async (element) => {
    const filter = Array.from(element.querySelectorAll<HTMLButtonElement>("button"))
      .find((button) => button.textContent === "Eventos");
    const started = performance.now();
    filter?.click();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    return performance.now() - started;
  });
  expect(paintMs).toBeLessThan(50);

  await page.evaluate(() => localStorage.setItem("nirmata.fixture.renamed", "true"));
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("línea alternativa");
  await palette.getByRole("option", { name: "Escribir en Línea alternativa" }).click();
  await explorer.getByRole("tab", { name: "Estructura" }).click();
  const renamed = explorer.getByRole("button", { name: "Archivo renombrado" });
  await expect(renamed).toHaveAttribute("aria-current", "true");
});

test("reference picker fills relation fields by name and keeps IDs advanced", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.setItem("nirmata.fixture.paletteSearch", "true"));
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("crear relación");
  await palette.getByRole("option", { name: "Crear relación" }).click();

  const destinationField = page.getByText("Entidad destino", { exact: true }).locator("..");
  await destinationField.getByRole("button", { name: "Elegir por nombre" }).click();
  const picker = page.getByRole("dialog", { name: "Entidad destino" });
  await picker.getByRole("searchbox", { name: "Buscar" }).fill("aurora");
  await picker.getByRole("button", { name: /Archivo de la Aurora/u }).click();
  await expect(destinationField).toContainText("1 selección guardada");
  await expect(destinationField).not.toContainText("40000000-0000");

  await destinationField.getByText("Introducir UUID o URI manualmente").click();
  await expect(destinationField.getByRole("textbox", { name: "Entidad destino, valor técnico" }))
    .toHaveValue("40000000-0000-4000-8000-000000000001");
});

test("situated context separates canon, perspectives and goals without IDs", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.explorerLoaded", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  const context = page.locator("#context-panel");
  await expect(context.getByRole("button", { name: /Regla de memoria/u })).toContainText("Canon");
  await context.getByRole("tab", { name: "Perspectivas" }).click();
  await expect(context.getByRole("button", { name: /Rumor del archivo/u })).toContainText("Perspectiva");
  await context.getByRole("tab", { name: "Metas" }).click();
  await expect(context.getByRole("button", { name: /Proteger el archivo/u })).toBeVisible();
  await expect(context.getByRole("button", { name: /Custodiar las memorias/u })).toBeVisible();
  const copy = await context.evaluate((element) => (element as HTMLElement).innerText);
  expect(copy).not.toContain("50000000-0000");
  const accessibility = await new AxeBuilder({ page }).include("#context-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
});

test("chronology separates known and unknown time and opens events", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.timelineLoaded", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Cronología", exact: true }).click();
  const timeline = page.getByRole("region", { name: "Cronología" });
  await expect(timeline.getByRole("heading", { name: "Tiempo conocido" })).toBeVisible();
  await expect(timeline.getByText("Día 11")).toBeVisible();
  await expect(timeline.getByText("En curso")).toBeVisible();
  await expect(timeline.getByRole("heading", { name: "Tiempo no especificado" })).toBeVisible();
  await expect(timeline.locator(".timeline-date").getByText("Sin fecha", { exact: true })).toBeVisible();
  await timeline.getByRole("button", { name: /Fundación del Archivo/u }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string; args: { uri?: string } }> }).__nirmataCommands.some((call) => call.command === "open_uri" && call.args?.uri?.includes("60000000")))).toBe(true);
});

test("Markdown preview keeps hostile HTML inert and preserves editor text", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("crear evento");
  await palette.getByRole("option", { name: "Crear evento" }).click();
  const body = page.getByRole("textbox", { name: "Cuerpo Markdown" });
  const hostile = "## Registro\n<img src=x onerror=alert(1)>\n[Referencia](nirmata://entity/40000000-0000-4000-8000-000000000001)\n[Fuente](https://example.test)";
  await body.fill(hostile);
  await page.getByRole("button", { name: "Mostrar vista previa segura" }).click();
  const preview = page.locator('.markdown-preview[data-markdown-mode="safe-preview"]');
  await expect(preview).toContainText("<img src=x onerror=alert(1)>");
  await expect(preview.locator("img, script")).toHaveCount(0);
  await expect(preview.getByRole("button", { name: "Referencia" })).toHaveAttribute("title", "Abrir referencia interna");
  await expect(preview.getByRole("link", { name: "Fuente (enlace externo)" })).toHaveAttribute("rel", "noreferrer");
  await expect(body).toHaveValue(hostile);
});

test("onboarding progress is local, dismissible and reopens from Help", async ({ page }) => {
  await page.goto("/");
  const guide = page.getByRole("heading", { name: "Construye una base coherente" });
  await expect(guide).toBeVisible();
  await page.getByRole("checkbox", { name: "Registrar una regla fundamental" }).check();
  await expect(page.getByText("1 de 6 pasos marcados en este equipo.")).toBeVisible();
  await page.getByRole("button", { name: "Ocultar guía" }).click();
  await expect(guide).toBeHidden();

  await page.getByRole("button", { name: "Ayuda", exact: true }).click();
  await page.getByRole("button", { name: "Volver a mostrar la guía del mundo" }).click();
  await expect(guide).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "Registrar una regla fundamental" })).toBeChecked();
  await expect.poll(() => page.evaluate(() => localStorage.getItem(`nirmata.onboarding.${"10000000-0000-4000-8000-000000000001"}`))).not.toBeNull();
});

test("assistant opens as a sheet and returns focus without losing its request", async ({ page }) => {
  await page.goto("/");
  const assistant = page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true });
  await assistant.click();
  await expect(page.locator("#assistant-panel")).toBeVisible();
  await page.locator("#assistant-input").fill("¿Qué tensión falta explorar?");
  await page.getByRole("button", { name: "Cerrar asistente", exact: true }).last().click();
  await expect(page.locator("#assistant-panel")).toBeHidden();
  await expect(assistant).toBeFocused();
  await assistant.click();
  await expect(page.locator("#assistant-input")).toHaveValue("¿Qué tensión falta explorar?");
});

test("global review drawer opens over Inicio and restores focus", async ({ page }) => {
  await page.goto("/");
  const changes = page.getByRole("button", { name: /Cambios 0 cambios pendientes/u });
  await changes.click();
  await expect(page.locator("#pending-panel")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Cambios pendientes" })).toBeVisible();
  await page.getByRole("button", { name: "Cerrar cambios", exact: true }).click();
  await expect(changes).toBeFocused();
  await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeVisible();
});
