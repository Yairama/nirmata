import assert from "node:assert/strict";
import test from "node:test";
import { readdir, readFile } from "node:fs/promises";

const frontendDirectory = new URL("./", import.meta.url);
const sourceNames = await readdir(frontendDirectory);
const frontendSource = (
  await Promise.all(
    sourceNames
      .filter((name) => name.endsWith(".ts") || name.endsWith(".tsx"))
      .map((name) => readFile(new URL(name, frontendDirectory), "utf8")),
  )
).join("\n");
const tauriSource = await readFile(new URL("../src-tauri/src/main.rs", import.meta.url), "utf8");
const simulationSource = await readFile(new URL("simulation-workspace.tsx", frontendDirectory), "utf8");
const narrativeSource = await readFile(new URL("narrative-workspace.tsx", frontendDirectory), "utf8");
const loreSource = await readFile(new URL("lore-import-workspace.tsx", frontendDirectory), "utf8");
const assistantSource = await readFile(new URL("assistant-workspace.tsx", frontendDirectory), "utf8");
const variantSource = await readFile(new URL("versions-workspace.tsx", frontendDirectory), "utf8");
const workspaceSource = await readFile(new URL("workspace.ts", frontendDirectory), "utf8");
const pendingSource = await readFile(new URL("pending-reviews.tsx", frontendDirectory), "utf8");
const editorModelSource = await readFile(new URL("editor-model.ts", frontendDirectory), "utf8");
const editorSource = await readFile(new URL("structured-editor.tsx", frontendDirectory), "utf8");
const htmlSource = await readFile(new URL("index.html", frontendDirectory), "utf8");
const cssSource = await readFile(new URL("styles.css", frontendDirectory), "utf8");
const closedViewSource = await readFile(new URL("closed-view.tsx", frontendDirectory), "utf8");
const mainSource = await readFile(new URL("main.tsx", frontendDirectory), "utf8");
const worldShellSource = await readFile(new URL("world-shell.tsx", frontendDirectory), "utf8");
const workspaceDataSource = await readFile(new URL("workspace-data.tsx", frontendDirectory), "utf8");

test("hidden application views stay out of layout and accessibility flow", () => {
  assert.match(cssSource, /\[hidden\]\s*\{\s*display:\s*none\s*!important;/u);
  assert.match(htmlSource, /<div id="root"><\/div>/u);
  assert.doesNotMatch(htmlSource, /id="(?:world-view|closed-view|workspace-shell|status|error)"/u);
});

test("React owns workspace splitters without hidden legacy layout controls", () => {
  assert.doesNotMatch(htmlSource, /left-panel-size|right-panel-size|bottom-panel-size|toggle-navigation|toggle-context|toggle-pending/u);
  assert.doesNotMatch(frontendSource, /state\.panels|applyLayoutState/u);
  assert.equal((worldShellSource.match(/role="separator"/gu) ?? []).length, 2);
  assert.match(worldShellSource, /requestAnimationFrame/u);
  assert.match(worldShellSource, /nirmata\.workspace\.layout\./u);
  assert.match(worldShellSource, /aria-valuetext/u);
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
  assert.match(assistantSource, /selectUriInScope\(uri, scope\)/u);
  assert.match(worldShellSource, /selectUriInScope\(uri, scope\)/u);
  assert.match(variantSource, /compare_variant_scopes/u);
  assert.doesNotMatch(assistantSource, /invoke<WorldSession>\("set_read_scope"/u);
  assert.doesNotMatch(narrativeSource, /invoke<WorldSession>\("set_read_scope"/u);
});

test("Query owns shared scoped workspace reads without AppState snapshots", () => {
  assert.match(workspaceDataSource, /session\.read_scope\.variantId/u);
  assert.match(workspaceDataSource, /session\.read_scope\.revisionId \?\? "head"/u);
  assert.match(workspaceDataSource, /"open_uri"/u);
  assert.match(workspaceDataSource, /"get_related_context"/u);
  assert.match(workspaceDataSource, /"list_timeline_events"/u);
  assert.match(workspaceDataSource, /"list_revision_history"/u);
  assert.match(workspaceDataSource, /cancelQueries/u);
  assert.match(workspaceDataSource, /removeQueries/u);
  assert.doesNotMatch(workspaceSource, /open_uri|get_related_context|list_timeline_events|list_revision_history|refreshNavigation|loadSelection/u);
  assert.doesNotMatch(frontendSource, /state\.(?:selectedObject|context|timeline|revisionHistory)/u);
});

test("ephemeral feature work participates in close and variant guards", () => {
  assert.match(workspaceSource, /Object\.keys\(state\.ephemeralWork\)/u);
  assert.match(workspaceSource, /trabajo de sesión que se perderá/u);
  assert.match(simulationSource, /setEphemeralWork/u);
  assert.match(narrativeSource, /setEphemeralWork/u);
  assert.match(frontendSource, /discardRevision/u);
  assert.match(simulationSource, /solo durante esta sesión/u);
});

test("pending reviews have one edit route and are discarded in their backend owner", () => {
  assert.match(pendingSource, /"discard_manual_review"/u);
  assert.match(pendingSource, /"discard_ai_run"/u);
  assert.match(pendingSource, /Editar cambio/u);
  assert.match(pendingSource, /Ver objeto actual/u);
  assert.match(pendingSource, /Aplicar al mundo/u);
  assert.doesNotMatch(pendingSource, /Abrir formulario|openPendingDraft/u);
  assert.doesNotMatch(editorModelSource, /restorePendingValues/u);
  assert.match(workspaceSource, /"begin_manual_review_edit"/u);
  assert.match(workspaceSource, /"apply_manual_review_edit"/u);
  assert.match(pendingSource, /useQuery\(/u);
  assert.match(pendingSource, /"list_pending_reviews"/u);
  assert.match(pendingSource, /"read_manual_review"/u);
  assert.doesNotMatch(frontendSource, /pendingDrafts/u);
  assert.doesNotMatch(htmlSource, /id="pending-panel"/u);
  assert.equal((frontendSource.match(/"confirm_manual_review"/gu) ?? []).length, 1);
});

test("React exclusively owns the structured editor and RHF owns composed lists", async () => {
  assert.ok(!sourceNames.includes("render-editor.ts"));
  assert.match(worldShellSource, /<StructuredEditor onPendingReviewsChanged=/u);
  assert.match(editorSource, /useForm<EditorForm>/u);
  assert.ok((editorSource.match(/useFieldArray\(/gu) ?? []).length >= 7);
  assert.doesNotMatch(htmlSource, /id="editor-(?:title|subtitle|empty|content)"/u);
  assert.doesNotMatch(frontendSource, /EditorMode|editorMode/u);
});

test("hostile Markdown stays in plain text mode", () => {
  assert.match(worldShellSource, /id="world-premise">\{session\.world\.premise_md/u);
  assert.match(assistantSource, /\{item\.markdown\}/u);
  assert.match(assistantSource, /<pre>\{streamedText\}<\/pre>/u);
  assert.doesNotMatch(frontendSource, /failure\.textContent = specialist\.error/u);
  assert.match(assistantSource, /title=\{evidence\.excerptMd\}/u);
  assert.match(frontendSource, /<pre className="lore-preview">\{source\.preview\}<\/pre>/u);
  assert.match(frontendSource, /setOpenedExcerpt\(location\.chunk\.content\)/u);
  assert.match(assistantSource, /\{humanize\(issue\.severity\)\}: \{issue\.summary\.markdown\}/u);
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
  assert.match(mainSource, /onStartProposal=\{startProposal\}/u);
  assert.match(mainSource, /onStartImport=\{startImport\}/u);
});

test("React owns the closed-project surface without an imperative renderer", () => {
  assert.match(mainSource, /createRoot\(document\.querySelector\("#root"\)!\)/u);
  assert.match(mainSource, /<RootErrorBoundary>/u);
  assert.match(mainSource, /<SessionProvider>/u);
  assert.doesNotMatch(htmlSource, /id="closed-view"|id="create-form"/u);
  assert.doesNotMatch(frontendSource, /document\.querySelector<[^>]+>\("#(?:create-form|creation-|name|premise|epoch-label|create-path|open-button)"\)/u);
  assert.doesNotMatch(frontendSource, /dangerouslySetInnerHTML/u);
  assert.match(closedViewSource, /if \(session === null\)[\s\S]{0,240}setBusy\(false\)/u);
});

test("one React root owns every application host without legacy bridges", () => {
  assert.equal((mainSource.match(/createRoot\(/gu) ?? []).length, 1);
  assert.match(mainSource, /get_current_world/u);
  assert.match(mainSource, /get_ai_activity/u);
  assert.match(mainSource, /session === null \? \(/u);
  assert.match(worldShellSource, /id="world-view"/u);
  assert.match(worldShellSource, /id="workspace-shell"/u);
  assert.match(worldShellSource, /id="navigation-panel"/u);
  assert.match(worldShellSource, /id="editor-panel"/u);
  assert.match(worldShellSource, /id="context-panel"/u);
  assert.doesNotMatch(frontendSource, /MutationObserver|renderWorkspace|useLegacyModal/u);
  assert.doesNotMatch(`${mainSource}\n${worldShellSource}`, /createPortal/u);
  assert.doesNotMatch(frontendSource, /new CustomEvent|nirmata:(?:pick-object|selection-changed|context-changed|scope-changed|editor-opened|open-area|open-reviews|show-onboarding|discard-ephemeral-work|pending-reviews-changed|start-lore-import)/u);
  assert.doesNotMatch(frontendSource, /export const \w+\s*=\s*document\.(?:querySelector|getElementById)/u);
  assert.doesNotMatch(frontendSource, /export const state\b/u);
  assert.match(frontendSource, /useSyncExternalStore/u);
  assert.match(frontendSource, /ObjectPickerProvider/u);
  assert.match(frontendSource, /useObjectPicker/u);
});

test("queries and selected objects enter the same explicit proposal workflow", () => {
  assert.match(assistantSource, /turn\.response\.proposalAction\?\.action === "start_proposal"/u);
  assert.match(assistantSource, /Convertir en propuesta/u);
  assert.match(assistantSource, /Confirmar paso a propuesta/u);
  assert.match(assistantSource, /originIsCurrent/u);
  assert.match(assistantSource, /runId: run\.id/u);
  assert.doesNotMatch(assistantSource, /execute_ai_proposal_from_brief[\s\S]{0,500}anchorUri/u);
  assert.doesNotMatch(assistantSource, /window\.confirm\(/u);
  assert.match(frontendSource, /Pedir un cambio sobre la selección/u);
  assert.equal((assistantSource.match(/"execute_ai_proposal"/gu) ?? []).length, 1);
});

test("browser prompts and confirms are replaced by accessible owned forms", () => {
  assert.doesNotMatch(frontendSource, /window\.(?:prompt|confirm)\s*\(/u);
  assert.doesNotMatch(frontendSource, /(?:^|[^\w.])(?:prompt|confirm)\s*\(/mu);
  assert.match(worldShellSource, /<Dialog\.Content className="confirmation-dialog"/u);
  assert.match(pendingSource, /inline-review-form/u);
});

test("proposal templates are closed, bounded and reuse standard review", () => {
  const cards = assistantSource.match(/\{ id: "(?:faction|city|character|conflict|chronology|consequences)"/gu) ?? [];
  assert.equal(cards.length, 6);
  assert.doesNotMatch(assistantSource, /data-template="novel"|Generar novela/iu);
  assert.match(assistantSource, /Pequeña · máximo 3 operaciones/u);
  assert.match(assistantSource, /Mediana · máximo 6 operaciones/u);
  assert.match(assistantSource, /"prepare_ai_proposal_template"/u);
  assert.match(assistantSource, /"execute_ai_proposal_from_brief"/u);
  assert.match(assistantSource, /requestObjectPicker/u);
  assert.match(assistantSource, /Todavía no se llamó al proveedor/u);
});

test("empty states offer actions without auto-running editors or AI", () => {
  assert.doesNotMatch(frontendSource, /else startCreatingObject\("entity"\)/u);
  assert.match(frontendSource, /El mundo todavía no tiene objetos/u);
  assert.match(frontendSource, /La cronología todavía no tiene acontecimientos/u);
  assert.match(frontendSource, /Crear cambio manual/u);
  assert.match(frontendSource, /Ejemplos para empezar/u);
});

test("advanced AI profiles keep authority and costs explicit", () => {
  assert.match(assistantSource, /Perfiles avanzados/u);
  assert.match(assistantSource, /Auditoría del canon/u);
  assert.match(assistantSource, /solo lectura y no crea propuestas/u);
  assert.match(assistantSource, /Resultado orientativo · solo lectura\. No se creó ninguna propuesta\./u);
  assert.match(assistantSource, /todavía requiere revisión y «Aplicar al mundo»/u);
  assert.doesNotMatch(assistantSource, /revisión base \$\{run\.baseRevision\}/u);
  assert.match(assistantSource, /specialistRoleLabel/u);
});

test("assistant conversations stay local and pass structured bounded history", () => {
  assert.match(assistantSource, /Historial guardado en este equipo · no es canon/u);
  assert.match(assistantSource, /nirmata\.assistant\.conversations\./u);
  assert.match(assistantSource, /conversation\.turns\.slice\(-8\)/u);
  assert.match(assistantSource, /history: conversationHistory\(conversation\)/u);
  assert.match(assistantSource, /Conversación eliminada\. El mundo y sus propuestas no cambiaron\./u);
  assert.match(tauriSource, /input\.history\.len\(\) > 8/u);
});

test("AI busy state is global, cancelable, and blocks world controls", () => {
  assert.match(tauriSource, /fn get_ai_activity/u);
  assert.match(tauriSource, /current_ai_activity/u);
  assert.match(frontendSource, /beginAiActivity/u);
  assert.match(frontendSource, /endAiActivity/u);
  assert.match(frontendSource, /"app_busy"/u);
  assert.match(worldShellSource, /id="ai-busy-cancel"/u);
  assert.match(worldShellSource, /disabled=\{Boolean\(aiActivity\)\}/u);
  assert.doesNotMatch(frontendSource, /aiBusyDisabled|control\.disabled\s*=/u);
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

test("React exclusively owns the assistant without imperative DOM or a compatibility shim", () => {
  assert.ok(!sourceNames.includes("assistant.ts"));
  assert.match(worldShellSource, /<AssistantWorkspace active=\{area === "assistant"\}/u);
  assert.doesNotMatch(htmlSource, /id="assistant-panel"|id="assistant-input"/u);
  assert.doesNotMatch(mainSource, /assistant\.js/u);
  assert.doesNotMatch(assistantSource, /createElement|querySelector|innerHTML|dangerouslySetInnerHTML|\bprompt\s*\(|\bconfirm\s*\(/u);
  assert.doesNotMatch(frontendSource, /nirmata:start-template|nirmata:ai-review-attached/u);
  assert.doesNotMatch(frontendSource, /nirmata:start-proposal|nirmata:ai-provider-changed/u);
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
    "prepare_ai_proposal_template",
    "execute_ai_proposal_from_brief",
    "prepare_deep_review",
    "execute_deep_review",
    "create_lore_import",
    "list_lore_imports",
    "extract_lore_import",
    "replace_lore_source",
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
    "list_variant_summaries",
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
    "get_project_diagnostics",
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

test("versions workspace separates writing, viewing, comparison and history without visible IDs", () => {
  assert.match(variantSource, /Escribiendo en/u);
  assert.match(variantSource, /Viendo/u);
  assert.match(variantSource, /Volver a la versión actual/u);
  assert.match(variantSource, /Traer cambios de \{selectedVariant\.variant\.name\} hacia/u);
  assert.match(variantSource, /Deshacer creando otra versión/u);
  assert.match(variantSource, /list_variant_summaries/u);
  assert.doesNotMatch(variantSource, /shortId/u);
  assert.match(frontendSource, /decisionOperationIds/u);
  assert.match(frontendSource, /descendants or import references/u);
  assert.match(frontendSource, /affectedReferences/u);
  assert.match(frontendSource, /auditSource/u);
  assert.match(pendingSource, /Conservar lo que ya existe en/u);
  assert.match(pendingSource, /Traer la versión de/u);
  assert.match(tauriSource, /AppError::ReadOnlyScope => "read_only_scope"/u);
});

test("lore import uses standard AI and explicit review without deep-review coupling", () => {
  const importModule = loreSource;
  assert.match(importModule, /"prepare_lore_import_review"/u);
  assert.doesNotMatch(importModule, /"read_manual_review"/u);
  assert.doesNotMatch(importModule, /prepare_deep_review|execute_deep_review/u);
  assert.match(importModule, /archivos originales permanecen intactos/u);
  assert.match(importModule, /"lore-import-progress"/u);
  assert.match(importModule, /setDecisionPoints\(prepared\.decisionPoints\)/u);
  assert.match(importModule, /pendingReviewsQueryKey/u);
  assert.match(importModule, /mark_canonical/u);
});

test("calendar UI delegates conversion to Rust and preserves canonical ticks", () => {
  assert.match(frontendSource, /calendar_mode/u);
  assert.match(frontendSource, /calendar_months/u);
  assert.match(frontendSource, /start_calendar_date/u);
  assert.match(editorSource, /singular="día"/u);
  assert.match(editorSource, /singular="mes"/u);
  assert.match(editorSource, /Año de \{suffix\}/u);
  assert.match(editorSource, /Unidad del día de \{suffix\}/u);
  assert.doesNotMatch(editorSource, /año\|mes\|día\|sub-tick|nombre\|días|Weekdays/u);
  assert.doesNotMatch(frontendSource, /function\s+(tickToDate|dateToTick)/u);
  assert.doesNotMatch(frontendSource, /save_calendar/u);
  assert.doesNotMatch(htmlSource, /edit-world-button/u);
});

test("simulation stays one-shot, outside canon, and enters only standard review", () => {
  assert.match(simulationSource, /fuera del canon/iu);
  assert.match(simulationSource, /session\.world_id/u);
  assert.match(simulationSource, /session\.read_scope\.variantId/u);
  assert.match(simulationSource, /session\.world\.current_revision/u);
  assert.match(simulationSource, /session\.read_only/u);
  assert.match(simulationSource, /pendingReviewsQueryKey/u);
  assert.match(simulationSource, /<dt>Antes<\/dt>/u);
  assert.match(simulationSource, /<dt>Después<\/dt>/u);
  assert.match(simulationSource, /Existencias finales/u);
  assert.match(simulationSource, /requestObjectPicker/u);
  assert.doesNotMatch(htmlSource, /simulation-(factions|resources|stocks|rules)/u);
  assert.doesNotMatch(simulationSource, /confirm_manual_review|setInterval|autoplay/u);
  assert.match(tauriSource, /struct CreateSimulationScenarioCommand/u);
  assert.match(tauriSource, /fn parse_simulation_scenario_id/u);
  assert.match(tauriSource, /deny_unknown_fields\)\]\s*struct PrepareSimulationReviewCommand/u);
});

test("narrative derivation stays cited, scoped, review-only, and shallow by default", () => {
  assert.match(narrativeSource, /Estudio narrativo/u);
  assert.match(narrativeSource, /Cronología/u);
  assert.match(narrativeSource, /Causalidad/u);
  assert.match(narrativeSource, /Cabos abiertos/u);
  assert.match(narrativeSource, /Documentos/u);
  assert.match(narrativeSource, /storyTime/u);
  assert.match(narrativeSource, /discourseOrder/u);
  assert.match(narrativeSource, /evidenceUris/u);
  assert.match(narrativeSource, /requestObjectPicker/u);
  assert.match(narrativeSource, /useWorkspaceData/u);
  assert.match(workspaceDataSource, /"list_timeline_events"/u);
  assert.match(narrativeSource, /session\.read_only/u);
  assert.match(narrativeSource, /onPendingReviewsChanged\(\)/u);
  assert.match(narrativeSource, /Alternativas de continuidad listas; todavía no se llamó a IA/u);
  assert.match(narrativeSource, /<pre className="narrative-document-body">\{value\.document\.body_md\}<\/pre>/u);
  assert.match(narrativeSource, /Abrir revisión en Cambios/u);
  assert.match(narrativeSource, /beginAiActivity/u);
  assert.match(narrativeSource, /endAiActivity/u);
  assert.match(narrativeSource, /unlisten\(\)/u);
  assert.doesNotMatch(htmlSource, /id="narrative-panel"/u);
  assert.doesNotMatch(mainSource, /narrative\.js/u);
  assert.doesNotMatch(narrativeSource, /confirm_manual_review|prepare_deep_review|execute_deep_review/u);
  assert.doesNotMatch(narrativeSource, /apiKey|PROVIDER_API_KEY|innerHTML/u);
  assert.doesNotMatch(htmlSource, />\s*Generar novela\s*</u);
  assert.match(tauriSource, /deny_unknown_fields\)\]\s*struct GenerateInternalDocumentCommand/u);
  assert.match(tauriSource, /deny_unknown_fields\)\]\s*struct ProposeNarrativeContinuityCommand/u);
  assert.doesNotMatch(tauriSource, /GenerateInternalDocumentCommand[\s\S]{0,500}(?:api_key|apiKey)/u);
});
