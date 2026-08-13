import assert from "node:assert/strict";
import test from "node:test";
import { readdir, readFile } from "node:fs/promises";

const frontendDirectory = new URL("./", import.meta.url);
const frontendSource = (
  await Promise.all(
    (await readdir(frontendDirectory))
      .filter((name) => name.endsWith(".ts") || name.endsWith(".tsx"))
      .map((name) => readFile(new URL(name, frontendDirectory), "utf8")),
  )
).join("\n");
const tauriSource = await readFile(new URL("../src-tauri/src/main.rs", import.meta.url), "utf8");
const simulationSource = await readFile(new URL("simulation.ts", frontendDirectory), "utf8");
const narrativeSource = await readFile(new URL("narrative.ts", frontendDirectory), "utf8");
const assistantSource = await readFile(new URL("assistant.ts", frontendDirectory), "utf8");
const variantSource = await readFile(new URL("variant-ui.ts", frontendDirectory), "utf8");
const workspaceSource = await readFile(new URL("workspace.ts", frontendDirectory), "utf8");
const pendingSource = await readFile(new URL("render-pending.ts", frontendDirectory), "utf8");
const editorModelSource = await readFile(new URL("editor-model.ts", frontendDirectory), "utf8");
const htmlSource = await readFile(new URL("index.html", frontendDirectory), "utf8");
const cssSource = await readFile(new URL("styles.css", frontendDirectory), "utf8");
const closedViewSource = await readFile(new URL("closed-view.tsx", frontendDirectory), "utf8");
const mainSource = await readFile(new URL("main.tsx", frontendDirectory), "utf8");

test("hidden application views stay out of layout and accessibility flow", () => {
  assert.match(cssSource, /\[hidden\]\s*\{\s*display:\s*none\s*!important;/u);
  assert.match(htmlSource, /id="world-view" hidden/u);
});

test("component colors consume semantic tokens instead of literals", () => {
  const componentCss = cssSource
    .split(/\r?\n/u)
    .filter((line) => !line.includes("--n-color-"))
    .join("\n");
  assert.doesNotMatch(componentCss, /#[0-9a-f]{3,8}\b|\brgb\(|\bcolor-mix\(/iu);
  assert.doesNotMatch(cssSource, /var\(--(?:border|surface|accent|mono|danger)\)/u);
});

test("dirty navigation is guarded before selection or scope changes", () => {
  assert.match(workspaceSource, /confirmDiscardPending\("editor"\)/u);
  assert.match(workspaceSource, /export async function selectUriInScope/u);
  assert.match(assistantSource, /selectUriInScope\(citation\.source\.uri/u);
  assert.match(narrativeSource, /selectUriInScope\(uri, scope\)/u);
  assert.match(variantSource, /selectUriInScope\(uri, scope\)/u);
  assert.doesNotMatch(assistantSource, /invoke<WorldSession>\("set_read_scope"/u);
  assert.doesNotMatch(narrativeSource, /invoke<WorldSession>\("set_read_scope"/u);
});

test("ephemeral feature work participates in close and variant guards", () => {
  assert.match(workspaceSource, /state\.ephemeralWork\.size/u);
  assert.match(workspaceSource, /trabajo de sesión que se perderá/u);
  assert.match(simulationSource, /setEphemeralWork/u);
  assert.match(narrativeSource, /setEphemeralWork/u);
  assert.match(frontendSource, /"nirmata:discard-ephemeral-work"/u);
  assert.match(htmlSource, /se borra al cerrar el mundo/u);
});

test("pending reviews have one edit route and are discarded in their backend owner", () => {
  assert.match(pendingSource, /"discard_manual_review"/u);
  assert.match(pendingSource, /"discard_ai_run"/u);
  assert.match(pendingSource, /"Editar cambio"/u);
  assert.match(pendingSource, /"Ver objeto actual"/u);
  assert.match(pendingSource, /"Aplicar al mundo"/u);
  assert.doesNotMatch(pendingSource, /Abrir formulario|openPendingDraft/u);
  assert.doesNotMatch(editorModelSource, /restorePendingValues/u);
  assert.match(workspaceSource, /"begin_manual_review_edit"/u);
  assert.match(workspaceSource, /"apply_manual_review_edit"/u);
});

test("hostile Markdown stays in plain text mode", () => {
  assert.match(frontendSource, /setMarkdownText\(worldPremise,/u);
  assert.match(frontendSource, /answer\.textContent = item\.markdown/u);
  assert.match(frontendSource, /streamElement\.textContent = streamedText/u);
  assert.match(frontendSource, /failure\.textContent = specialist\.error/u);
  assert.match(frontendSource, /source\.title = evidence\.excerptMd/u);
  assert.match(frontendSource, /preview\.textContent = source\.preview/u);
  assert.match(frontendSource, /preview\.textContent = location\.chunk\.content/u);
  assert.match(frontendSource, /summary\.textContent = `\$\{humanize\(issue\.severity\)\}: \$\{issue\.summary\.markdown\}`/u);
  assert.doesNotMatch(frontendSource, /\binnerHTML\b/u);
  assert.doesNotMatch(frontendSource, /\bdangerouslySetInnerHTML\b/u);
  assert.doesNotMatch(frontendSource, /\binsertAdjacentHTML\b/u);
});

test("desktop sources avoid default console or stdout logging", () => {
  assert.doesNotMatch(frontendSource, /\bconsole\.(?:log|warn|error|info|debug)\b/u);
  assert.doesNotMatch(tauriSource, /\b(?:println!|eprintln!|dbg!)\b/u);
});

test("Tauri IPC uses tree-shakeable package imports without a window global", () => {
  assert.match(frontendSource, /from "@tauri-apps\/api\/core"/u);
  assert.match(frontendSource, /from "@tauri-apps\/api\/event"/u);
  assert.match(frontendSource, /from "@tauri-apps\/plugin-dialog"/u);
  assert.doesNotMatch(frontendSource, /window\.__TAURI__|TauriApi/u);
  assert.match(tauriSource, /tauri_plugin_dialog::init\(\)/u);
});

test("failure states preserve recovery paths", () => {
  assert.match(frontendSource, /La propuesta se conservó para corregirla o reintentarlo\./u);
  assert.match(frontendSource, /El canon no cambió\. La propuesta se conservó y puedes reintentar\./u);
  assert.match(frontendSource, /La ejecución terminó sin modificar el canon\. Puedes reintentar\./u);
  assert.match(frontendSource, /Actualizar y volver a comprobar/u);
});

test("creation journey exposes three paths through one project form", () => {
  assert.match(closedViewSource, /Empezar manualmente/u);
  assert.match(closedViewSource, /Crear una base del mundo con IA/u);
  assert.match(closedViewSource, /Estructurar material existente/u);
  assert.equal((closedViewSource.match(/<form/gu) ?? []).length, 1);
  assert.equal((frontendSource.match(/"create_world"/gu) ?? []).length, 1);
  assert.match(frontendSource, /"nirmata:start-proposal"/u);
  assert.match(frontendSource, /"nirmata:start-lore-import"/u);
});

test("React owns the closed-project surface without an imperative renderer", () => {
  assert.match(mainSource, /createRoot\(document\.querySelector\("#closed-root"\)!\)/u);
  assert.match(mainSource, /<RootErrorBoundary>/u);
  assert.match(mainSource, /<SessionProvider>/u);
  assert.doesNotMatch(htmlSource, /id="closed-view"|id="create-form"/u);
  assert.doesNotMatch(frontendSource, /document\.querySelector<[^>]+>\("#(?:create-form|creation-|name|premise|epoch-label|create-path|open-button)"\)/u);
  assert.doesNotMatch(frontendSource, /dangerouslySetInnerHTML/u);
  assert.match(closedViewSource, /if \(session === null\)[\s\S]{0,240}setBusy\(false\)/u);
});

test("queries and selected objects enter the same explicit proposal workflow", () => {
  assert.match(assistantSource, /response\.proposalAction\?\.action === "start_proposal"/u);
  assert.match(assistantSource, /Convertir en propuesta/u);
  assert.match(assistantSource, /window\.confirm/u);
  assert.match(frontendSource, /Pedir un cambio sobre la selección/u);
  assert.equal((assistantSource.match(/"execute_ai_proposal"/gu) ?? []).length, 1);
});

test("AI busy state is global, cancelable, and blocks world controls", () => {
  assert.match(tauriSource, /fn get_ai_activity/u);
  assert.match(tauriSource, /current_ai_activity/u);
  assert.match(frontendSource, /beginAiActivity/u);
  assert.match(frontendSource, /endAiActivity/u);
  assert.match(frontendSource, /"app_busy"/u);
  assert.match(htmlSource, /id="ai-busy-cancel"/u);
  assert.match(frontendSource, /control\.disabled = !allowed/u);
  assert.match(frontendSource, /aiBusyMessage\.textContent !== message/u);
});

test("provider diagnostics distinguish local configuration from connectivity", () => {
  assert.match(tauriSource, /credential_missing/u);
  assert.match(tauriSource, /endpoint_missing/u);
  assert.match(tauriSource, /model_missing/u);
  assert.match(tauriSource, /connection_unchecked/u);
  assert.match(tauriSource, /fn diagnose_ai_provider/u);
  assert.match(assistantSource, /"provider_transport_error"/u);
  assert.match(assistantSource, /No se creó ninguna propuesta/u);
});

test("foundation workflows are wired to specific Tauri commands", () => {
  const commands = [
    "create_world",
    "open_world",
    "search_world",
    "open_uri",
    "get_related_context",
    "get_ai_activity",
    "get_ai_provider_status",
    "diagnose_ai_provider",
    "execute_ai_query",
    "execute_ai_proposal",
    "prepare_deep_review",
    "execute_deep_review",
    "create_lore_import",
    "extract_lore_import",
    "decide_lore_candidate",
    "edit_lore_candidate",
    "open_lore_chunk",
    "prepare_lore_import_review",
    "delete_lore_import",
    "apply_manual_review_action",
    "apply_manual_review_edit",
    "discard_manual_review",
    "revalidate_manual_review",
    "revalidate_ai_run",
    "confirm_manual_review",
    "list_revision_history",
    "list_variants",
    "create_variant",
    "rename_variant",
    "switch_variant",
    "archive_variant",
    "set_read_scope",
    "view_active_head",
    "compare_variant_scopes",
    "prepare_variant_merge",
    "export_vfs_snapshot",
    "import_vfs_snapshot",
    "undo_revision",
    "create_simulation_scenario",
    "update_simulation_scenario",
    "delete_simulation_scenario",
    "list_simulation_scenarios",
    "run_simulation_scenario",
    "prepare_simulation_review",
    "derive_narrative_timeline",
    "derive_causal_threads",
    "derive_loose_ends",
    "generate_internal_document",
    "explore_narrative_continuity",
    "propose_narrative_continuity",
  ];
  for (const command of commands) {
    assert.ok(frontendSource.includes(`"${command}"`), `frontend invokes ${command}`);
    assert.match(tauriSource, new RegExp(`\\b${command},`, "u"), `Tauri registers ${command}`);
  }
});

test("variant UI separates writing and viewed versions without visible IDs", () => {
  assert.match(frontendSource, /state\.session\?\.read_only/u);
  assert.match(variantSource, /Escribiendo en:/u);
  assert.match(variantSource, /Viendo:/u);
  assert.match(htmlSource, /Volver a la versión actual/u);
  assert.doesNotMatch(variantSource, /option\.textContent = `\$\{variant\.name\} · \$\{shortId/u);
  assert.match(frontendSource, /decisionOperationIds/u);
  assert.match(frontendSource, /descendants or import references/u);
  assert.match(frontendSource, /affectedReferences/u);
  assert.match(frontendSource, /auditSource/u);
  assert.match(tauriSource, /AppError::ReadOnlyScope => "read_only_scope"/u);
});

test("lore import uses standard AI and explicit review without deep-review coupling", () => {
  const importModule = frontendSource.slice(frontendSource.indexOf("function candidateText"));
  assert.match(importModule, /"prepare_lore_import_review"/u);
  assert.match(importModule, /"read_manual_review"/u);
  assert.doesNotMatch(importModule, /prepare_deep_review|execute_deep_review/u);
  assert.match(importModule, /original permanecen intactos/u);
  assert.match(importModule, /"lore-import-progress"/u);
  assert.match(importModule, /decisionPoints = prepared\.decisionPoints/u);
  assert.match(importModule, /state\.pendingDrafts\.delete\(batchReviewKey\)/u);
  assert.match(importModule, /mark_canonical/u);
});

test("calendar UI delegates conversion to Rust and preserves canonical ticks", () => {
  assert.match(frontendSource, /calendar_mode/u);
  assert.match(frontendSource, /calendar_months/u);
  assert.match(frontendSource, /start_calendar_date/u);
  assert.match(frontendSource, /año\|mes\|día\|sub-tick/u);
  assert.doesNotMatch(frontendSource, /function\s+(tickToDate|dateToTick)/u);
});

test("simulation stays one-shot, outside canon, and enters only standard review", () => {
  assert.match(htmlSource, /Fuera del canon/u);
  assert.match(simulationSource, /session!\.world_id/u);
  assert.match(simulationSource, /session!\.read_scope\.variantId/u);
  assert.match(simulationSource, /session!\.world\.current_revision/u);
  assert.match(simulationSource, /session\.read_only/u);
  assert.match(simulationSource, /state\.pendingDrafts\.set\(review\.reviewKey/u);
  assert.match(simulationSource, /Antes:/u);
  assert.match(simulationSource, /Después:/u);
  assert.match(simulationSource, /Solicitado:/u);
  assert.match(simulationSource, /Existencias finales:/u);
  assert.doesNotMatch(simulationSource, /confirm_manual_review|setInterval|autoplay/u);
  assert.match(tauriSource, /struct CreateSimulationScenarioCommand/u);
  assert.match(tauriSource, /fn parse_simulation_scenario_id/u);
  assert.match(tauriSource, /deny_unknown_fields\)\]\s*struct PrepareSimulationReviewCommand/u);
});

test("narrative derivation stays cited, scoped, review-only, and shallow by default", () => {
  assert.match(htmlSource, /Derivación narrativa/u);
  assert.match(narrativeSource, /storyTime/u);
  assert.match(narrativeSource, /discourseOrder/u);
  assert.match(narrativeSource, /finding\.code/u);
  assert.match(narrativeSource, /evidenceUris/u);
  assert.match(narrativeSource, /\.textContent =/u);
  assert.match(narrativeSource, /state\.session\?\.read_only/u);
  assert.match(narrativeSource, /attachAiReview\(proposal\.run/u);
  assert.match(narrativeSource, /attachAiReview\(run/u);
  assert.match(narrativeSource, /Alternativas de continuidad listas; todavía no se llamó a IA/u);
  assert.doesNotMatch(narrativeSource, /confirm_manual_review|prepare_deep_review|execute_deep_review/u);
  assert.doesNotMatch(narrativeSource, /apiKey|PROVIDER_API_KEY|innerHTML/u);
  assert.doesNotMatch(htmlSource, />\s*Generar novela\s*</u);
  assert.match(tauriSource, /deny_unknown_fields\)\]\s*struct GenerateInternalDocumentCommand/u);
  assert.match(tauriSource, /deny_unknown_fields\)\]\s*struct ProposeNarrativeContinuityCommand/u);
  assert.doesNotMatch(tauriSource, /GenerateInternalDocumentCommand[\s\S]{0,500}(?:api_key|apiKey)/u);
});
