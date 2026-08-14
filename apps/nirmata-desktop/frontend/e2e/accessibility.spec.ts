import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import type { Locator, Page } from "@playwright/test";

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
  path: "C:/Fixtures/accesibilidad.nirmata",
  world_id: variant.worldId,
  current_revision: variant.headRevisionId,
  world: {
    id: variant.worldId,
    name: "Archivo accesible",
    premise_md: "Una ciudad conserva su memoria en minerales.",
    epoch_label: "Fundación",
    current_revision: variant.headRevisionId,
    created_at_ms: 1725000000000,
    updated_at_ms: 1725000000000,
    calendar: null,
  },
  active_variant: variant,
  read_scope: { variantId: variant.id, revisionId: null },
  read_only: false,
};

async function installFixture(page: Page, options: { closed?: boolean; readOnly?: boolean; searchError?: boolean } = {}) {
  await page.addInitScript(({ sessionValue, variantValue, fixtureOptions }) => {
    let callbackId = 1;
    const emptyReport = { errors: [], conflicts: [], warnings: [], info: [] };
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        transformCallback(callback: (...args: unknown[]) => void) {
          const id = callbackId++;
          Object.assign(window, { [`_${id}`]: callback });
          return id;
        },
        unregisterCallback(id: number) {
          delete (window as unknown as Record<string, unknown>)[`_${id}`];
        },
        async invoke(command: string) {
          if (command.startsWith("plugin:event|")) return 1;
          if (command.startsWith("plugin:dialog|")) return null;
          if (command === "plugin:app|version") return "0.1.0";
          if (command === "get_current_world") {
            if (fixtureOptions.closed) return null;
            const current = structuredClone(sessionValue);
            if (fixtureOptions.readOnly) {
              return {
                ...current,
                read_only: true,
                read_scope: { ...current.read_scope, revisionId: current.current_revision },
              };
            }
            return current;
          }
          if (command === "get_ai_activity") return { busy: false, requestIds: [] };
          if (command === "list_recent_projects") return [];
          if (command === "get_ai_provider_status") return {
            state: "connection_unchecked", message: "Configuración local completa.", canCheckConnection: true, connected: false,
            credential: { configured: true, source: "session_memory", persistence: "session", secureStoreAvailable: true, limitation: null },
          };
          if (command === "search_world") {
            if (fixtureOptions.searchError) throw { code: "storage_error", message: "raw storage failure" };
            return { hits: [], absence: { classification: "no_evidence", provenance: "search_world" } };
          }
          if (command === "read_logical_vfs") return { name: "/", children: [] };
          if (command === "list_timeline_events") return { known: [], unknown: [], calendarName: null };
          if (command === "list_revision_history") return { currentHeadRevisionId: variantValue.headRevisionId, undoTargetRevisionId: null, revisions: [] };
          if (command === "list_variants") return [structuredClone(variantValue)];
          if (command === "list_variant_summaries") return [{ variant: structuredClone(variantValue), originVariantName: null, originSummary: "Versión inicial", originCreatedAtMs: 1725000000000, latestSummary: "Versión inicial", latestCreatedAtMs: 1725000000000 }];
          if (command === "list_lore_imports") return [];
          if (command === "list_simulation_scenarios") return [];
          if (command === "get_project_diagnostics") return { schemaVersion: 10, integrity: "ok" };
          if (command === "preview_manual_draft") return {
            draft: null,
            review: null,
            fieldIssues: [{ field: "name", code: "required", message: "Escribe un nombre." }],
          };
          if (command === "get_related_context") return { canon: [], perspectives: [], desires: [], obligations: [], search_evidence: [], usage: { max_objects: 24, max_chars: 4000, used_objects: 0, used_chars: 0 }, absence: null };
          if (command === "read_manual_review") return { reviewKey: "none", objective: "", sources: [], assumptions: [], baseRevision: variantValue.headRevisionId, operations: [], validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: false, freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" } };
          return null;
        },
      },
    });
  }, { sessionValue: session, variantValue: variant, fixtureOptions: options });
}

async function expectAxeClean(page: Page, root: string | Locator) {
  const builder = new AxeBuilder({ page });
  if (typeof root === "string") builder.include(root);
  else builder.include(await root.evaluate((element) => {
    if (!element.id) element.id = `axe-${crypto.randomUUID()}`;
    return `#${CSS.escape(element.id)}`;
  }));
  const result = await builder.analyze();
  const blocking = result.violations.filter((violation) =>
    violation.id === "color-contrast" || violation.impact === "serious" || violation.impact === "critical"
  );
  expect(blocking).toEqual([]);
}

async function expectDomAuditClean(page: Page) {
  const issues = await page.evaluate(() => {
    const visible = (element: HTMLElement) => !element.hidden && element.getClientRects().length > 0;
    const controls = Array.from(document.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input, select, textarea"))
      .filter((control) => control.type !== "hidden");
    const findings: string[] = [];
    for (const control of controls) {
      const labelledBy = control.getAttribute("aria-labelledby")?.split(/\s+/).filter(Boolean) ?? [];
      const hasName = Boolean(
        control.getAttribute("aria-label")?.trim()
        || control.labels?.length
        || labelledBy.some((id) => document.getElementById(id)?.textContent?.trim()),
      );
      if (visible(control) && !hasName) findings.push(`sin nombre accesible: ${control.outerHTML.slice(0, 160)}`);
      if (control.closest("form") && !control.name.trim()) findings.push(`sin name dentro de form: ${control.outerHTML.slice(0, 160)}`);
      if (control instanceof HTMLInputElement && control.type === "password" && !control.hasAttribute("autocomplete")) {
        findings.push(`password sin autocomplete: ${control.outerHTML.slice(0, 160)}`);
      }
    }
    const ids = Array.from(document.querySelectorAll<HTMLElement>("[id]")).map((element) => element.id);
    for (const id of new Set(ids)) if (ids.filter((value) => value === id).length > 1) findings.push(`id duplicado: ${id}`);
    for (const element of Array.from(document.querySelectorAll<HTMLElement>("[aria-controls], [aria-describedby], [aria-labelledby], [aria-errormessage]"))) {
      for (const attribute of ["aria-controls", "aria-describedby", "aria-labelledby", "aria-errormessage"]) {
        for (const id of element.getAttribute(attribute)?.split(/\s+/).filter(Boolean) ?? []) {
          if (!document.getElementById(id)) findings.push(`${attribute} inválido: ${id}`);
        }
      }
    }
    return findings;
  });
  expect(issues).toEqual([]);
}

test("WCAG gate covers landing, wizard and Ajustes", async ({ page }) => {
  await installFixture(page, { closed: true });
  await page.goto("/");
  await expectAxeClean(page, "#closed-view");
  await expectDomAuditClean(page);

  await page.getByRole("button", { name: /Empezar manualmente/u }).click();
  await expectAxeClean(page, "#closed-view");
  await expectDomAuditClean(page);
  await page.getByRole("button", { name: "Cancelar" }).click();

  await page.getByRole("button", { name: "Ajustes" }).click();
  await expectAxeClean(page, ".software-dialog");
  await page.getByRole("tab", { name: "IA", exact: true }).click();
  await expectDomAuditClean(page);
});

test("WCAG gate covers workspace, overlays and advanced areas", async ({ page }) => {
  test.setTimeout(90_000);
  await installFixture(page);
  await page.goto("/");
  const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
  await expectAxeClean(page, "main");

  await navigation.getByRole("button", { name: "Mundo", exact: true }).click();
  await expectAxeClean(page, "#world-view");
  await expectDomAuditClean(page);

  await page.keyboard.press("Control+k");
  await expectAxeClean(page, ".command-dialog");
  await page.keyboard.press("Escape");

  await page.locator("#navigation-panel").getByLabel("Nuevo").selectOption("relation");
  await page.locator("#navigation-panel").getByRole("button", { name: "Crear", exact: true }).click();
  await page.getByText("Entidad destino", { exact: true }).locator("..").getByRole("button", { name: "Elegir por nombre" }).click();
  await expectAxeClean(page, ".object-picker-dialog");
  await page.keyboard.press("Escape");

  await navigation.getByRole("button", { name: "Cronología", exact: true }).click();
  await expectAxeClean(page, ".world-timeline");
  await page.getByRole("button", { name: "Configurar calendario", exact: true }).click();
  await expectAxeClean(page, "#editor-panel");

  for (const area of ["Estudio narrativo", "Simulación", "Importaciones", "Versiones"]) {
    await navigation.getByRole("button", { name: area, exact: true }).click();
    const root = area === "Estudio narrativo" ? ".narrative-workspace"
      : area === "Simulación" ? "#simulation-panel"
        : area === "Importaciones" ? "#imports-panel" : ".versions-workspace";
    await expect(page.locator(root)).toBeVisible();
    await expectAxeClean(page, root);
    await expectDomAuditClean(page);
    if (area === "Importaciones") {
      await page.getByRole("tab", { name: "Copias de seguridad" }).click();
      await expectAxeClean(page, "#imports-panel");
      await page.getByRole("tab", { name: "Textos" }).click();
    }
  }

  const assistantTrigger = navigation.getByRole("button", { name: "Asistente", exact: true });
  await assistantTrigger.click();
  await expect(page.locator("#assistant-panel")).toBeVisible();
  await expectAxeClean(page, "#assistant-panel");
  await expectDomAuditClean(page);
  await page.keyboard.press("Escape");
  await expect(assistantTrigger).toBeFocused();

  const changesTrigger = page.getByRole("button", { name: /Cambios 0/u }).first();
  await changesTrigger.click();
  await expect(page.locator("#pending-panel")).toBeVisible();
  await expectAxeClean(page, "#pending-panel");
  await page.keyboard.press("Escape");
  await expect(changesTrigger).toBeFocused();
});

test("field issues expose and focus the invalid editor control", async ({ page }) => {
  await installFixture(page);
  await page.setViewportSize({ width: 720, height: 520 });
  await page.goto("/");
  const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
  const world = navigation.getByRole("button", { name: "Mundo", exact: true });
  await world.focus();
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Crear entidad", exact: true }).first().press("Enter");
  await page.getByRole("button", { name: "Preparar cambios", exact: true }).press("Enter");
  const name = page.getByRole("textbox", { name: "Nombre", exact: true });
  await expect(name).toBeFocused();
  await expect(name).toHaveAttribute("aria-invalid", "true");
  const describedBy = await name.getAttribute("aria-describedby");
  expect(describedBy).toBeTruthy();
  await expect(page.locator(`#${describedBy}`)).toContainText("Escribe un nombre.");
  await expectAxeClean(page, "#editor-panel");
});

test("read-only and error states remain accessible", async ({ page }) => {
  await installFixture(page, { readOnly: true, searchError: true });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await expect(page.getByText("Solo lectura", { exact: true })).toBeVisible();
  await expect(page.getByRole("alert").filter({ hasText: "No se pudo buscar" })).toBeVisible();
  await expectAxeClean(page, "main");
});
