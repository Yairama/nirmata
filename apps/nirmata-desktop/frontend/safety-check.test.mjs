import assert from "node:assert/strict";
import test from "node:test";
import { readdir, readFile } from "node:fs/promises";

const frontendDirectory = new URL("./", import.meta.url);
const frontendSource = (
  await Promise.all(
    (await readdir(frontendDirectory))
      .filter((name) => name.endsWith(".ts"))
      .map((name) => readFile(new URL(name, frontendDirectory), "utf8")),
  )
).join("\n");
const tauriSource = await readFile(new URL("../src-tauri/src/main.rs", import.meta.url), "utf8");

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

test("failure states preserve recovery paths", () => {
  assert.match(frontendSource, /El draft se conservó para corregirlo o reintentarlo\./u);
  assert.match(frontendSource, /El canon no cambió\. El draft se conservó y puedes reintentar\./u);
  assert.match(frontendSource, /La ejecución terminó sin modificar el canon\. Puedes reintentar\./u);
  assert.match(frontendSource, /Revalidar contra la cabeza vigente/u);
});

test("foundation workflows are wired to specific Tauri commands", () => {
  const commands = [
    "create_world",
    "open_world",
    "search_world",
    "open_uri",
    "get_related_context",
    "get_provider_credential_status",
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
  ];
  for (const command of commands) {
    assert.ok(frontendSource.includes(`"${command}"`), `frontend invokes ${command}`);
    assert.match(tauriSource, new RegExp(`\\b${command},`, "u"), `Tauri registers ${command}`);
  }
});

test("variant UI separates viewed scope from the active write head", () => {
  assert.match(frontendSource, /state\.session\?\.read_only/u);
  assert.match(frontendSource, /Solo lectura:/u);
  assert.match(frontendSource, /return to the active variant head|Vuelve a la cabeza activa/u);
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
});
