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

const archivedVariant = {
  ...variant,
  id: "30000000-0000-4000-8000-000000000003",
  name: "Primer borrador",
  archived: true,
  createdAtMs: 1725100000000,
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ sessionValue, variantValue, alternateVariantValue, archivedVariantValue }) => {
    let callbackId = 1;
    let activeVariant = structuredClone(variantValue);
    let simulationScenarios: Array<Record<string, unknown>> = [];
    let configuredCalendar: Record<string, unknown> | null = null;
    let pendingManualRequest: Record<string, unknown> | null = null;
    let pendingReviews: Array<Record<string, unknown>> = [];
    const commands: Array<{ command: string; args: unknown }> = [];
    const loreBatch = {
      id: "70000000-0000-4000-8000-000000000001",
      worldId: variantValue.worldId,
      variantId: variantValue.id,
      targetRevision: variantValue.headRevisionId,
      status: "reviewing",
      createdAtMs: 1725200000000,
      sources: [{
        id: "source-chronicle",
        path: "C:/Fixtures/cronica.md",
        fileName: "cronica.md",
        format: "markdown",
        contentHash: "sha256:advanced-technical-hash",
        sizeBytes: 84,
        status: "ready",
        preview: "# Crónica\nMara custodia el Archivo de la Aurora.",
        chunks: [{ id: "chunk-1", sourceId: "source-chronicle", sourceHash: "sha256:advanced-technical-hash", ordinal: 0, byteStart: 0, byteEnd: 52, lineStart: 1, lineEnd: 2, heading: "Crónica", content: "Mara custodia el Archivo de la Aurora." }],
      }],
    };
    const emptyReport = { errors: [], conflicts: [], warnings: [], info: [] };
    function storePending(review: Record<string, unknown>, origin: string, title: string, editorRequest: Record<string, unknown>, aiRunId: string | null = null, merge: Record<string, unknown> | null = null) {
      const reviewKey = String(review.reviewKey);
      pendingReviews = [
        ...pendingReviews.filter((item) => String((item.review as Record<string, unknown>).reviewKey) !== reviewKey),
        { review: structuredClone(review), origin, title, editorRequest: structuredClone(editorRequest), aiRunId, merge },
      ];
    }
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
        async invoke(command: string, args?: { input?: Record<string, unknown> }) {
          commands.push({ command, args });
          if (command.startsWith("plugin:event|")) return 1;
          if (command.startsWith("plugin:dialog|open")) return "C:/Fixtures/backups";
          if (command === "get_current_world") {
            const current = structuredClone(sessionValue);
            current.active_variant = structuredClone(activeVariant);
            current.read_scope.variantId = activeVariant.id;
            Object.assign(current.world, { calendar: configuredCalendar ? structuredClone(configuredCalendar) : null });
            if (localStorage.getItem("nirmata.fixture.longPremise") === "true") {
              current.world.premise_md = Array.from({ length: 18 }, (_, index) => `Párrafo ${index + 1}: magia, tecnología y ciudades transforman el mundo.`).join("\n\n");
            }
            if (localStorage.getItem("nirmata.fixture.readOnly") === "true") {
              return {
                ...current,
                read_only: true,
                read_scope: { ...current.read_scope, revisionId: current.current_revision },
              };
            }
            return current;
          }
          if (command === "list_pending_reviews") {
            if (localStorage.getItem("nirmata.fixture.persistedPending") !== "true") return structuredClone(pendingReviews);
            const review = {
              reviewKey: "nirmata://entity/40000000-0000-4000-8000-000000000099",
              objective: "Recuperar la guardiana después de reiniciar",
              sources: [], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "persisted-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", dependencies: [], before: null, after: { title: "Guardiana recuperada", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", lines: [{ label: "Tipo", value: "Person" }] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: true,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" },
            };
            storePending(review, "manual", "Guardiana recuperada", { objectType: "entity", objective: review.objective, sourceUris: [], assumptions: [], values: { kind: "person", name: "Guardiana recuperada", slug: "guardiana-recuperada", aliases: "", attributes_json: "{}" } });
            return structuredClone(pendingReviews);
          }
          if (command === "discard_manual_review") {
            const reviewKey = String(args?.input?.reviewKey ?? "");
            pendingReviews = pendingReviews.filter((item) => String((item.review as Record<string, unknown>).reviewKey) !== reviewKey);
            localStorage.removeItem("nirmata.fixture.persistedPending");
            return null;
          }
          if (command === "discard_ai_run") {
            const runId = (args as { runId?: string } | undefined)?.runId;
            pendingReviews = pendingReviews.filter((item) => item.aiRunId !== runId);
            return null;
          }
          if (command === "get_ai_activity") return { busy: false, requestIds: [] };
          if (command === "list_lore_imports") {
            if (localStorage.getItem("nirmata.fixture.loreError") === "true") {
              throw { code: "provider_timeout", message: "Azure raw timeout after 60 seconds" };
            }
            return localStorage.getItem("nirmata.fixture.lore") === "true" ? [structuredClone(loreBatch)] : [];
          }
          if (command === "read_lore_candidates") return [{
            id: "candidate-mara",
            candidate: {
              kind: "entity", candidateId: "candidate-mara", contradictionKey: null,
              citations: [{ chunkId: "chunk-1", sourceId: "source-chronicle", sourceHash: "sha256:advanced-technical-hash", excerpt: "Mara custodia el Archivo de la Aurora." }],
              technicalConfidence: 0.91, name: "Mara", entityKind: "person", aliases: ["La Custodia"], summary: "Custodia el Archivo de la Aurora.",
            },
            resolvedSourceCandidateId: null, resolvedTargetCandidateId: null, status: "pending",
            identitySuggestion: "new", identityMatches: [], identityDecision: null, canonicalUri: null,
          }];
          if (command === "replace_lore_source") return structuredClone(loreBatch);
          if (command === "open_lore_chunk") return { chunk: loreBatch.sources[0].chunks[0] };
          if (command === "get_project_diagnostics") return { schemaVersion: 11, integrity: "ok" };
          if (command === "export_vfs_snapshot") return {
            path: "C:/Fixtures/backups/backup-principal", worldId: variantValue.worldId,
            baseRevision: variantValue.headRevisionId, logicalHash: "sha256:snapshot-export",
            objectCount: 12, variantId: variantValue.id, variant: variantValue.name,
          };
          if (command === "import_vfs_snapshot") {
            const result = {
              path: "C:/Fixtures/backups", worldId: variantValue.worldId, variantId: variantValue.id,
              variant: variantValue.name, baseRevision: variantValue.headRevisionId,
              logicalHash: "sha256:snapshot-import", objectCount: 12, createdCount: 1, updatedCount: 1, deletedCount: 0,
              review: {
              reviewKey: `nirmata://world/${variantValue.worldId}`, objective: "Import snapshot", sources: [], assumptions: [],
              baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "snapshot-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", dependencies: [], before: null, after: { title: "Torre restaurada", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", lines: [] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: true,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" },
              },
            };
            storePending(result.review, "snapshot", "Importación de snapshot", { objectType: "world", existingUri: result.review.reviewKey, sourceUris: [], assumptions: [], values: {} });
            return result;
          }
          if (command === "get_ai_provider_status") {
            const connected = localStorage.getItem("nirmata.fixture.aiReady") === "true";
            return {
              state: connected ? "connected" : "credential_missing",
              message: connected ? "Conexión verificada." : "La IA no está configurada.",
              canCheckConnection: connected,
              connected,
              credential: { configured: connected, source: connected ? "system_secure_store" : "missing", persistence: connected ? "system_secure_store" : "none", secureStoreAvailable: true, limitation: null },
            };
          }
          if (command === "execute_ai_query") return {
            request: "¿Cómo podría independizarse la ciudad?",
            snapshot: { worldId: variantValue.worldId, baseRevision: variantValue.headRevisionId, readScope: { variantId: variantValue.id, revisionId: null } },
            items: [{ itemId: "answer-1", classification: "inference", markdown: "La ciudad necesitaría controlar sus rutas.", contentReferences: [], citations: [] }],
            proposalAction: { action: "start_proposal", label: "Convertir en propuesta", request: "Preparar la independencia de la ciudad" },
          };
          if (command === "prepare_deep_review") {
            const mode = String(args?.input?.mode ?? "deep_impact");
            return {
              mode,
              request: String(args?.input?.request ?? ""),
              roles: mode === "audit" ? ["temporal_auditor", "rules_auditor"] : ["historian", "political_scientist"],
              selectionSource: "rule_based",
              reason: mode === "audit" ? "La solicitud requiere comprobar reglas y continuidad." : "La solicitud puede afectar historia e instituciones.",
              confirmed: false,
              budget: { maxSpecialists: 4, maxSpecialistCalls: 4, maxSynthesisCalls: 1, maxContextExpansions: 2, maxReadToolCalls: 8, maxNestedDelegations: 0, specialistMaxOutputTokens: 1200, synthesisMaxOutputTokens: 1800, specialistTimeoutMs: 30000 },
            };
          }
          if (command === "execute_deep_review") {
            const mode = String(args?.input?.mode ?? "deep_impact");
            const roles = args?.input?.roles as string[];
            const specialists = roles.map((role, index) => ({
              role, status: "completed", error: null, elapsedMs: 1200, inputTokens: 200, outputTokens: 300,
              report: { specialist: role, sources: [`nirmata://world/${variantValue.worldId}`], findings: [{ findingId: `finding-${index}`, summary: { markdown: `${role} identifica una consecuencia trazable.`, contentReferences: [] }, affectedObjectUris: [], candidateConsequences: [], assumptions: [], evidence: [], confidence: 0.9, unresolvedQuestions: [], decisionPosition: null }] },
            }));
            if (mode === "audit") return {
              id: "deep-audit-run", baseRevision: variantValue.headRevisionId, mode, request: args?.input?.request, status: "completed_audit",
              plan: { mode, request: args?.input?.request, roles, selectionSource: "explicit", reason: "Roles confirmados.", confirmed: true, budget: { maxSpecialists: 4, maxSpecialistCalls: 4, maxSynthesisCalls: 1, maxContextExpansions: 2, maxReadToolCalls: 8, maxNestedDelegations: 0, specialistMaxOutputTokens: 1200, synthesisMaxOutputTokens: 1800, specialistTimeoutMs: 30000 } },
              specialists, synthesis: null, auditResult: { validationReport: { ...emptyReport, warnings: [{ code: "audit.warning", severity: "warning", objects: [], message: "Continuidad por revisar" }] }, findingIds: specialists.map((_, index) => `finding-${index}`) }, standardRunId: null, error: null,
            };
            const review = {
              reviewKey: "deep-review", objective: "Síntesis profunda revisable", sources: [`nirmata://world/${variantValue.worldId}`], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "deep-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", dependencies: [], before: null, after: { title: "Consecuencia profunda", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", lines: [] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: false, freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: true, message: "Vigente" },
            };
            storePending(review, "ai", "Síntesis profunda revisable", { objectType: "entity", sourceUris: review.sources, assumptions: [], values: {} }, "deep-standard-run");
            return {
              id: "deep-impact-run", baseRevision: variantValue.headRevisionId, mode, request: args?.input?.request, status: "awaiting_review",
              plan: { mode, request: args?.input?.request, roles, selectionSource: "explicit", reason: "Roles confirmados.", confirmed: true, budget: { maxSpecialists: 4, maxSpecialistCalls: 4, maxSynthesisCalls: 1, maxContextExpansions: 2, maxReadToolCalls: 8, maxNestedDelegations: 0, specialistMaxOutputTokens: 1200, synthesisMaxOutputTokens: 1800, specialistTimeoutMs: 30000 } },
              specialists, synthesis: { draft: { objective: "Síntesis profunda revisable", assumptions: [], operations: [{ create_entity: {} }], decisions: [] }, operationOrigins: [{ operationId: "deep-operation", findingIds: ["finding-0"] }], decisionOrigins: [] }, auditResult: null, standardRunId: "deep-standard-run", error: null,
            };
          }
          if (command === "prepare_ai_proposal_template") {
            const template = String(args?.input?.template ?? "faction");
            const scale = String(args?.input?.scale ?? "small");
            const entityUri = "nirmata://entity/40000000-0000-4000-8000-000000000001";
            return {
              id: `template-run-${template}`,
              baseRevision: variantValue.headRevisionId,
              request: `Expandir ${template}`,
              status: "intent_brief_ready",
              context: {
                context: {
                  canon: [{ uri: `nirmata://world/${variantValue.worldId}` }, { uri: entityUri }],
                  perspectives: [], desires: [], obligations: [], searchEvidence: [],
                },
              },
              draft: null, validationReport: null, critiqueReport: null, repairCount: 0, reviewKey: null, error: null,
              intentBrief: {
                userRequest: `Expandir ${template}`,
                objective: `Objetivo ${template}`,
                scope: `Alcance ${template}`,
                entities: [],
                restrictions: [`No superar ${scale === "small" ? 3 : 6} operaciones.`],
                reason: `La plantilla ${template} delimita la expansión antes de llamar al proveedor.`,
                authority: "La IA solo preparará una propuesta para Cambios; no puede aplicarla ni convertirla en canon.",
                template,
                scale,
              },
            };
          }
          if (command === "execute_ai_proposal_from_brief") {
            const run = {
              id: args?.input?.runId,
              baseRevision: variantValue.headRevisionId,
              request: args?.input?.objective,
              status: "awaiting_review",
              context: { context: { canon: [], perspectives: [], desires: [], obligations: [], searchEvidence: [] } },
              draft: { objective: args?.input?.objective, assumptions: [], decisions: [], operations: [{ create_entity: { operation_id: "template-operation" } }] },
              validationReport: emptyReport, critiqueReport: { issues: [] }, repairCount: 0,
              reviewKey: "template-review", intentBrief: null, error: null,
            };
            const review = {
              reviewKey: "template-review", objective: "Expansión desde plantilla",
              sources: [`nirmata://world/${variantValue.worldId}`], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "template-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", dependencies: [], before: null, after: { title: "Expansión preparada", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", lines: [] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: false,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: true, message: "Vigente" },
            };
            storePending(review, "ai", "Expansión preparada", { objectType: "entity", sourceUris: review.sources, assumptions: [], values: {} }, String(run.id));
            return run;
          }
          if (command === "search_world") {
            if (localStorage.getItem("nirmata.fixture.error") === "true") {
              throw new Error("El mundo no cambió; puedes reintentar.");
            }
            if (localStorage.getItem("nirmata.fixture.editorAggregates") === "true") {
              const requestedKind = String(args?.input?.kind ?? "all");
              const query = String(args?.input?.queryText ?? "").toLocaleLowerCase();
              const kinds = ["entity", "relation", "event", "claim", "rule", "goal", "document"];
              const hits = kinds.map((objectType, index) => {
                const id = `${index + 4}0000000-0000-4000-8000-000000000001`;
                return {
                  object_ref: { [objectType]: id }, object_type: objectType, object_id: id,
                  uri: `nirmata://${objectType}/${id}`, snippet: `[Objeto ${objectType}]`, authority: "canonical",
                  classification: "fact", provenance: "fixture", stage: "search", score: 1,
                  rank: index + 1, score_explanation: "fixture",
                };
              }).filter((item) => (requestedKind === "all" || item.object_type === requestedKind)
                && (!query || item.snippet.toLocaleLowerCase().includes(query)));
              return { hits, absence: hits.length ? null : { classification: "no_evidence", provenance: "fixture" } };
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
            if (localStorage.getItem("nirmata.fixture.narrative") === "true") {
              const query = String(args?.input?.queryText ?? "").toLocaleLowerCase();
              const candidates = [{
                object_ref: { event: "60000000-0000-4000-8000-000000000001" }, object_type: "event",
                object_id: "60000000-0000-4000-8000-000000000001", uri: "nirmata://event/60000000-0000-4000-8000-000000000001",
                snippet: "[Fundación del Archivo]", authority: "canonical", classification: "fact", provenance: "fts5", stage: "search", score: 1, rank: 1, score_explanation: "exact",
              }, {
                object_ref: { entity: "40000000-0000-4000-8000-000000000010" }, object_type: "entity",
                object_id: "40000000-0000-4000-8000-000000000010", uri: "nirmata://entity/40000000-0000-4000-8000-000000000010",
                snippet: "[Mara, custodia del Archivo]", authority: "canonical", classification: "fact", provenance: "fts5", stage: "search", score: 1, rank: 1, score_explanation: "exact",
              }];
              return { hits: candidates.filter((item) => item.snippet.toLocaleLowerCase().includes(query)), absence: null };
            }
            if (localStorage.getItem("nirmata.fixture.simulation") === "true") {
              const query = String(args?.input?.queryText ?? "").toLocaleLowerCase();
              const candidates = [{
                object_ref: { entity: "40000000-0000-4000-8000-000000000010" }, object_type: "entity",
                object_id: "40000000-0000-4000-8000-000000000010", uri: "nirmata://entity/40000000-0000-4000-8000-000000000010",
                snippet: "[Mara del Puerto]", authority: "canonical", classification: "fact", provenance: "fts5", stage: "search", score: 1, rank: 1, score_explanation: "exact",
              }, {
                object_ref: { entity: "40000000-0000-4000-8000-000000000011" }, object_type: "entity",
                object_id: "40000000-0000-4000-8000-000000000011", uri: "nirmata://entity/40000000-0000-4000-8000-000000000011",
                snippet: "[Grano azul]", authority: "canonical", classification: "fact", provenance: "fts5", stage: "search", score: 1, rank: 1, score_explanation: "exact",
              }];
              return { hits: candidates.filter((item) => item.snippet.toLocaleLowerCase().includes(query)), absence: null };
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
            const requestedUri = String((args as unknown as { uri?: string })?.uri ?? "");
            if (localStorage.getItem("nirmata.fixture.editorAggregates") === "true") {
              const id = requestedUri.split("/").at(-1)!;
              const commonResult = (object_type: string, snippet: string) => ({ object_ref: { [object_type]: id }, object_type, object_id: id, uri: requestedUri, snippet, authority: "canonical", classification: "fact", provenance: "uri", stage: "selection", score: 1, rank: 1, score_explanation: "exact" });
              if (requestedUri.startsWith("nirmata://relation/")) return { result: commonResult("relation", "Alianza del Archivo"), object: { relation: { id, world_id: variantValue.worldId, source_entity_id: "40000000-0000-4000-8000-000000000001", target_entity_id: "40000000-0000-4000-8000-000000000002", kind: "alianza", direction: "directed", valid_from_tick: null, valid_to_tick: null, certainty: "certain", source_reference: null, metadata_json: "{}", version: 1 } }, eventCalendar: null };
              if (requestedUri.startsWith("nirmata://claim/")) return { result: commonResult("claim", "El Archivo recuerda"), object: { claim: { id, world_id: variantValue.worldId, subject_entity_id: "40000000-0000-4000-8000-000000000001", content_md: "El Archivo recuerda.", predicate_key: null, object: null, polarity: "positive", authentication: "canonical", holder_entity_id: null, modality: null, register: null, epistemic_basis: null, source: null, source_document_id: null, source_claim_id: null, holder_confidence: null, period: null, registered_revision_id: variantValue.headRevisionId, superseded_revision_id: null, version: 1 } }, eventCalendar: null };
              if (requestedUri.startsWith("nirmata://rule/")) return { result: commonResult("rule", "La memoria no se destruye"), object: { rule: { id, world_id: variantValue.worldId, kind: "constitutive", statement_md: "La memoria no se destruye.", scope: "world", severity: "advisory", source: null, validator_kind: null, parameters_json: "{}", version: 1, created_at_ms: 1, updated_at_ms: 1 } }, eventCalendar: null };
              if (requestedUri.startsWith("nirmata://goal/")) return { result: commonResult("goal", "Proteger el Archivo"), object: { goal: { id, world_id: variantValue.worldId, holder_entity_id: "40000000-0000-4000-8000-000000000001", desired_state_md: "Proteger el Archivo", priority: 1, status: "active", period: null, visibility: "public", source: null, version: 1 } }, eventCalendar: null };
              if (requestedUri.startsWith("nirmata://document/")) return { result: commonResult("document", "Crónica del Archivo"), object: { document: { object: { id, world_id: variantValue.worldId, title: "Crónica del Archivo", kind: "chronicle", author_entity_id: null, perspective_entity_id: null, canon_status: "canonical", body_md: "Memoria.", version: 1, created_at_ms: 1, updated_at_ms: 1 }, references: [] } }, eventCalendar: null };
            }
            if (requestedUri.startsWith("nirmata://world/")) {
              const current = structuredClone(sessionValue.world);
              Object.assign(current, { calendar: configuredCalendar ? structuredClone(configuredCalendar) : null });
              return {
                result: { object_ref: { world: variantValue.worldId }, object_type: "world", object_id: variantValue.worldId, uri: requestedUri, snippet: "Mundo vacío", authority: "canonical", classification: "fact", provenance: "uri", stage: "selection", score: 1, rank: 1, score_explanation: "exact" },
                object: { world: current }, eventCalendar: null,
              };
            }
            if (requestedUri.startsWith("nirmata://event/")) {
              return {
                result: { object_ref: { event: "60000000-0000-4000-8000-000000000010" }, object_type: "event", object_id: "60000000-0000-4000-8000-000000000010", uri: requestedUri, snippet: "Festival de Lluvia", authority: "canonical", classification: "fact", provenance: "uri", stage: "selection", score: 1, rank: 1, score_explanation: "exact" },
                object: { event: { event: { id: "60000000-0000-4000-8000-000000000010", world_id: variantValue.worldId, kind: "festival", summary: "Festival de Lluvia", body_md: "", time: { kind: "instant", start_tick: 1234, end_tick: null, precision: "day", certainty: "certain" }, location_entity_id: null, participants: [], affected_goal_ids: [], version: 1, created_at_ms: 1725000000000, updated_at_ms: 1725000000000 }, links: [] } },
                eventCalendar: { start: { tick: 1234, label: "Cenit, Lluvia 3, año 2 · unidad 4", dateInput: "2|2|3|4", year: 2, month: 2, monthName: "Lluvia", day: 3, tickInDay: 4, weekdayName: "Cenit" }, end: null },
              };
            }
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
              eventCalendar: null,
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
            if (localStorage.getItem("nirmata.fixture.timelineNoCalendar") === "true") {
              return {
                known: [{ uri: "nirmata://event/60000000-0000-4000-8000-000000000004", summary: "Tiempo sin presentación", kind: "memory", time: { kind: "instant", start_tick: 10, end_tick: null, precision: "exact", certainty: "certain" }, startCalendar: null, endCalendar: null }],
                unknown: [], calendarName: null,
              };
            }
            if (localStorage.getItem("nirmata.fixture.timelineLoaded") === "true" || localStorage.getItem("nirmata.fixture.narrative") === "true") {
              const time = (kind: string, start_tick: number | null, certainty = "certain") => ({ kind, start_tick, end_tick: null, precision: start_tick === null ? "unknown" : "exact", certainty });
              return {
                calendarName: configuredCalendar ? String(configuredCalendar.name) : "Imperial",
                known: [
                  { uri: "nirmata://event/60000000-0000-4000-8000-000000000001", summary: "Fundación del Archivo", kind: "founding", time: time("instant", 10), startCalendar: { tick: 10, label: "Día 11", dateInput: "1|1|11|0" }, endCalendar: null },
                  { uri: "nirmata://event/60000000-0000-4000-8000-000000000002", summary: "Custodia prolongada", kind: "guardianship", time: time("ongoing", 20, "approximate"), startCalendar: { tick: 20, label: "Día 21", dateInput: "1|1|21|0" }, endCalendar: null },
                ],
                unknown: [{ uri: "nirmata://event/60000000-0000-4000-8000-000000000003", summary: "Pérdida sin fecha", kind: "loss", time: time("unknown", null), startCalendar: null, endCalendar: null }],
              };
            }
            return { known: [], unknown: [], calendarName: configuredCalendar ? String(configuredCalendar.name) : null };
          }
          if (command === "list_revision_history") {
            if (localStorage.getItem("nirmata.fixture.firstApplied") === "true") {
              return {
                currentHeadRevisionId: variantValue.headRevisionId,
                undoTargetRevisionId: variantValue.headRevisionId,
                revisions: [{
                  revisionId: variantValue.headRevisionId, parentRevisionId: null,
                  changeSetId: "first-applied", author: "manual_review", summary: "Primer cambio aplicado",
                  createdAtMs: 1725300000000, undoneRevisionId: null, isCurrentHead: true, isCurrentUndoTarget: true,
                  operations: [{ operationId: "first-operation", targetUri: `nirmata://world/${variantValue.worldId}`, decision: "accept", source: "manual_review", decidedAtMs: 1725300000000, before: null, after: null, waivers: [] }],
                  waivers: [],
                }],
              };
            }
            if (localStorage.getItem("nirmata.fixture.versions") === "true") {
              const emptyReport = { errors: [], conflicts: [], warnings: [], info: [] };
              return {
                currentHeadRevisionId: variantValue.headRevisionId,
                undoTargetRevisionId: variantValue.headRevisionId,
                revisions: [{
                  revisionId: variantValue.headRevisionId,
                  parentRevisionId: "20000000-0000-4000-8000-000000000000",
                  changeSetId: "70000000-0000-4000-8000-000000000001",
                  author: "manual_review",
                  summary: "Custodia reforzada del Archivo",
                  createdAtMs: 1725300000000,
                  undoneRevisionId: null,
                  isCurrentHead: true,
                  isCurrentUndoTarget: true,
                  operations: [{
                    operationId: "80000000-0000-4000-8000-000000000001",
                    targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001",
                    decision: "accept",
                    source: "manual_review",
                    decidedAtMs: 1725300000000,
                    before: { title: "Archivo de la Aurora", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", lines: [{ label: "Resumen", value: "Custodia memorias." }] },
                    after: { title: "Archivo de la Aurora", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", lines: [{ label: "Resumen", value: "Custodia memorias minerales." }] },
                    waivers: [],
                    effectiveIssues: emptyReport,
                  }],
                  waivers: [],
                }, {
                  revisionId: "20000000-0000-4000-8000-000000000000",
                  parentRevisionId: null,
                  changeSetId: "70000000-0000-4000-8000-000000000000",
                  author: "import",
                  summary: "Fundación de la ciudad",
                  createdAtMs: 1725000000000,
                  undoneRevisionId: null,
                  isCurrentHead: false,
                  isCurrentUndoTarget: false,
                  operations: [],
                  waivers: [],
                }],
              };
            }
            return { currentHeadRevisionId: variantValue.headRevisionId, undoTargetRevisionId: null, revisions: [] };
          }
          if (command === "list_variants") return [structuredClone(variantValue), structuredClone(alternateVariantValue)];
          if (command === "list_variant_summaries") return [
            { variant: structuredClone(variantValue), originVariantName: null, originSummary: "Versión inicial", originCreatedAtMs: 1725000000000, latestSummary: "Custodia reforzada del Archivo", latestCreatedAtMs: 1725300000000 },
            { variant: structuredClone(alternateVariantValue), originVariantName: "Canon principal", originSummary: "Fundación de la ciudad", originCreatedAtMs: 1725000000000, latestSummary: "La ciudad toma otra ruta", latestCreatedAtMs: 1725400000000 },
            { variant: structuredClone(archivedVariantValue), originVariantName: "Canon principal", originSummary: "Fundación de la ciudad", originCreatedAtMs: 1725100000000, latestSummary: "Borrador detenido", latestCreatedAtMs: 1725200000000 },
          ];
          if (command === "switch_variant") {
            activeVariant = args?.input?.variantId === alternateVariantValue.id
              ? structuredClone(alternateVariantValue)
              : structuredClone(variantValue);
            const current = structuredClone(sessionValue);
            current.active_variant = structuredClone(activeVariant);
            current.read_scope = { variantId: activeVariant.id, revisionId: null };
            return current;
          }
          if (command === "set_read_scope") {
            const current = structuredClone(sessionValue);
            const scope = args?.input?.scope as { variantId: string; revisionId: string | null };
            current.active_variant = structuredClone(activeVariant);
            return {
              ...current,
              read_scope: scope,
              read_only: scope.variantId !== activeVariant.id || scope.revisionId !== null,
            };
          }
          if (command === "view_active_head") {
            const current = structuredClone(sessionValue);
            current.active_variant = structuredClone(activeVariant);
            current.read_scope = { variantId: activeVariant.id, revisionId: null };
            current.read_only = false;
            return current;
          }
          if (command === "create_variant") return { ...structuredClone(alternateVariantValue), name: args?.input?.name };
          if (command === "rename_variant") {
            activeVariant = { ...activeVariant, name: String(args?.input?.name) };
            return structuredClone(activeVariant);
          }
          if (command === "archive_variant") return null;
          if (command === "compare_variant_scopes") return {
            left: args?.input?.left,
            right: args?.input?.right,
            differences: [{
              objectRef: { entity: "40000000-0000-4000-8000-000000000001" },
              kind: "edited",
              before: { entity: { name: "Archivo de la Aurora", summary: "Custodia memorias." } },
              after: { entity: { name: "Archivo de la Aurora", summary: "Custodia memorias minerales y rutas." } },
              leftScope: args?.input?.left,
              rightScope: args?.input?.right,
              leftSource: { revisionId: variantValue.headRevisionId, changeSetId: "70000000-0000-4000-8000-000000000001", operationId: "80000000-0000-4000-8000-000000000001", retcon: "additive", auditSource: "manual_review", scope: args?.input?.left },
              rightSource: { revisionId: alternateVariantValue.headRevisionId, changeSetId: "70000000-0000-4000-8000-000000000002", operationId: "80000000-0000-4000-8000-000000000002", retcon: "reinterpretive", auditSource: "merge", scope: args?.input?.right },
              affectedReferences: [{ rule: "50000000-0000-4000-8000-000000000001" }],
            }],
          };
          if (command === "prepare_variant_merge") {
            const emptyReport = { errors: [], conflicts: [], warnings: [], info: [] };
            const result = {
              sourceScope: args?.input?.scope,
              destinationScope: { variantId: activeVariant.id, revisionId: null },
              commonAncestorRevision: "20000000-0000-4000-8000-000000000000",
              automaticOperationIds: ["80000000-0000-4000-8000-000000000001"],
              decisionOperationIds: ["80000000-0000-4000-8000-000000000002"],
              review: {
                reviewKey: `nirmata://world/${variantValue.worldId}`,
                objective: "Merge variant revision technical-id",
                sources: [],
                assumptions: [],
                baseRevision: variantValue.headRevisionId,
                operations: [],
                validationReport: emptyReport,
                effectiveReport: emptyReport,
                readyToConfirm: false,
                freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" },
              },
            };
            storePending(result.review, "versions_merge", "Traer cambios de Línea alternativa hacia Canon principal", { objectType: "world", existingUri: result.review.reviewKey, sourceUris: [], assumptions: [], values: {} }, null, { sourceName: "Línea alternativa", destinationName: "Canon principal", automaticCount: 1, decisionCount: 1 });
            return result;
          }
          if (command === "undo_revision") return structuredClone(sessionValue);
          if (command === "list_simulation_scenarios") return structuredClone(simulationScenarios);
          if (command === "set_desktop_menu_state" || command === "exit_application") return null;
          if (command === "create_simulation_scenario") {
            const created = { id: "90000000-0000-4000-8000-000000000001", ...(args?.input?.scenario ?? {}) };
            simulationScenarios = [created];
            return structuredClone(created);
          }
          if (command === "update_simulation_scenario") {
            const updated = { id: args?.input?.scenarioId, ...(args?.input?.scenario ?? {}) };
            simulationScenarios = [updated];
            return structuredClone(updated);
          }
          if (command === "delete_simulation_scenario") {
            simulationScenarios = [];
            return null;
          }
          if (command === "run_simulation_scenario") {
            const scenario = simulationScenarios.find((item) => item.id === args?.input?.scenarioId)!;
            const stocks = structuredClone(scenario.stocks as unknown[]);
            const rule = structuredClone((scenario.rules as unknown[])[0]);
            return {
              scenarioId: scenario.id, worldId: scenario.worldId, variantId: scenario.variantId, baseRevision: scenario.baseRevision,
              stepsCompleted: 1, assumptions: scenario.assumptions, finalStocks: [{ ...(stocks[0] as object), quantity: 15 }],
              transitions: [{ step: 1, ruleIndex: 0, rule, before: stocks, after: [{ ...(stocks[0] as object), quantity: 15 }], requested: 5, applied: 5, shortage: 0 }],
            };
          }
          if (command === "prepare_simulation_review") {
            const review = {
              reviewKey: "simulation-review", objective: "Promover resultados de Cosecha del puerto", sources: [], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "simulation-event", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://event/90000000-0000-4000-8000-000000000002", dependencies: [], before: null, after: { title: "La cosecha aumenta", objectType: "event", targetUri: "nirmata://event/90000000-0000-4000-8000-000000000002", lines: [{ label: "Autoridad", value: "Propuesta revisable" }] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: true,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" },
            };
            storePending(review, "simulation", "Resultados de simulación", { objectType: "event", sourceUris: [], assumptions: [], values: {} });
            return review;
          }
          if (command === "prepare_entity_deletion") {
            const targetUri = `nirmata://entity/${String(args?.input?.entityId ?? "")}`;
            const orphan = localStorage.getItem("nirmata.fixture.deleteBlocked") === "true";
            const issue = { code: "change_set.delete_orphan", severity: "error", objects: [], message: "delete operation would leave orphaned references" };
            const report = orphan ? { ...emptyReport, errors: [issue] } : emptyReport;
            const before = { title: "Archivo de la Aurora", objectType: "entity", targetUri, lines: [{ label: "Tipo", value: "Lugar" }] };
            const review = {
              reviewKey: targetUri, objective: "Eliminar Archivo de la Aurora del canon", sources: [], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "delete-operation", decision: "accept", selected: true, severity: orphan ? "error" : "info", targetUri, dependencies: orphan ? ["nirmata://event/60000000-0000-4000-8000-000000000010"] : [], before, after: null, issues: report, effectiveIssues: report, waivers: [], decisionPoints: [{ decisionPointId: "delete-decision", prompt: "Should entity be replaced?", alternatives: ["Keep current canon", "Apply replacement"], replacementTarget: targetUri, suggestionAvailable: true, suggestionHidden: true, reason: null, resolvedAlternative: null }], risk: { requiresJudgment: true, judgment: null, suggestedResolutionAvailable: true, suggestedResolutionHidden: true, triggers: [{ code: "replacement", title: "Replacement", detail: "Requiere juicio humano." }] } }],
              validationReport: report, effectiveReport: report, readyToConfirm: false,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: true, message: "Vigente" },
            };
            storePending(review, "manual", "Archivo de la Aurora", { objectType: "entity", existingUri: targetUri, sourceUris: [], assumptions: [], values: {} });
            return review;
          }
          if (command === "preview_manual_draft") {
            if (localStorage.getItem("nirmata.fixture.editorFailure") === "true") {
              throw { code: "storage_error", message: "raw backend failure" };
            }
            pendingManualRequest = structuredClone(args?.input ?? {});
            const request = pendingManualRequest as { objectType?: string; values?: Record<string, string>; objective?: string };
            const isWorld = request.objectType === "world";
            const targetUri = isWorld
              ? `nirmata://world/${variantValue.worldId}`
              : "nirmata://event/60000000-0000-4000-8000-000000000010";
            const title = isWorld ? "Mundo vacío" : String(request.values?.summary ?? "Evento");
            const lines = isWorld ? [
              { label: "Calendario", value: String(request.values?.calendar_name ?? "") },
              { label: "Días de la semana", value: String(request.values?.calendar_weekdays ?? "").replaceAll("\n", ", ") },
              { label: "Meses", value: String(request.values?.calendar_months ?? "").split("\n").map((row) => { const [name, days] = row.split("|"); return `${name} (${days} días)`; }).join(", ") },
              { label: "Detalles técnicos del calendario", value: `epoch ${request.values?.calendar_epoch_tick ?? "0"} · ${request.values?.calendar_ticks_per_day ?? "1"} unidades por día` },
            ] : [
              { label: "Tipo de tiempo", value: "Instante" },
              { label: "Detalles técnicos del tiempo", value: "proyección temporal canónica" },
            ];
            const review = {
              reviewKey: targetUri, objective: request.objective ?? "Preparar cambios", sources: [], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "manual-operation", decision: "accept", selected: true, severity: "info", targetUri, dependencies: [], before: null, after: { title, objectType: request.objectType, targetUri, lines }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: true,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" },
            };
            storePending(review, "manual", title, pendingManualRequest ?? { objectType: request.objectType, sourceUris: [], assumptions: [], values: {} });
            return { draft: { draftKey: targetUri, targetUri, objectType: request.objectType, mode: isWorld ? "update" : "create", title, objective: review.objective, sourceUris: [], assumptions: [], logicalPath: isWorld ? "/world" : "/events/festival", validationReport: emptyReport, readyToConfirm: true }, review, fieldIssues: [] };
          }
          if (command === "confirm_manual_review") {
            const reviewKey = String(args?.input?.reviewKey ?? "");
            pendingReviews = pendingReviews.filter((item) => String((item.review as Record<string, unknown>).reviewKey) !== reviewKey);
            localStorage.removeItem("nirmata.fixture.persistedPending");
            const values = (pendingManualRequest as { values?: Record<string, string> } | null)?.values;
            if (values?.calendar_mode === "fixed") {
              configuredCalendar = {
                name: values.calendar_name, epoch_tick: Number(values.calendar_epoch_tick), ticks_per_day: Number(values.calendar_ticks_per_day),
                weekday_names: values.calendar_weekdays.split("\n"),
                months: values.calendar_months.split("\n").map((row) => { const [name, days] = row.split("|"); return { name, days: Number(days) }; }),
              };
            }
            const current = structuredClone(sessionValue);
            Object.assign(current.world, { calendar: configuredCalendar ? structuredClone(configuredCalendar) : null });
            return current;
          }
          if (command === "begin_manual_review_edit") return structuredClone(pendingManualRequest);
          if (command === "apply_manual_review_edit") {
            pendingManualRequest = structuredClone((args?.input as { request?: Record<string, unknown> })?.request ?? {});
            return {
              draft: null, fieldIssues: [],
              review: { reviewKey: "nirmata://event/60000000-0000-4000-8000-000000000010", objective: "Editar evento", sources: [], assumptions: [], baseRevision: variantValue.headRevisionId, operations: [], validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: true, freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: false, message: "Vigente" } },
            };
          }
          if (command === "derive_narrative_timeline") return {
            scope: args?.input?.scope,
            storyTime: [{ event: { objectRef: { event: "60000000-0000-4000-8000-000000000001" }, uri: "nirmata://event/60000000-0000-4000-8000-000000000001" }, summary: "Fundación del Archivo", time: { kind: "instant", start_tick: 10, end_tick: null, precision: "exact", certainty: "certain" }, evidenceUris: ["nirmata://entity/40000000-0000-4000-8000-000000000010"] }],
            unknownStoryTime: [{ event: { objectRef: { event: "60000000-0000-4000-8000-000000000003" }, uri: "nirmata://event/60000000-0000-4000-8000-000000000003" }, summary: "Pérdida sin fecha", time: { kind: "unknown", start_tick: null, end_tick: null, precision: "unknown", certainty: "uncertain" }, evidenceUris: [] }],
            discourseOrder: [{ source: { objectRef: { document: "80000000-0000-4000-8000-000000000001" }, uri: "nirmata://document/80000000-0000-4000-8000-000000000001" }, events: [{ event: { objectRef: { event: "60000000-0000-4000-8000-000000000001" }, uri: "nirmata://event/60000000-0000-4000-8000-000000000001" }, ordinal: 0, evidenceUris: [] }] }],
          };
          if (command === "derive_causal_threads") return {
            scope: args?.input?.scope, maxDepth: args?.input?.maxDepth, limit: args?.input?.limit,
            threads: [{ start: { objectRef: { event: "60000000-0000-4000-8000-000000000001" }, uri: "nirmata://event/60000000-0000-4000-8000-000000000001" }, links: [{ depth: 1, kind: "causes", source: { objectRef: { event: "60000000-0000-4000-8000-000000000001" }, uri: "nirmata://event/60000000-0000-4000-8000-000000000001" }, target: { objectRef: { event: "60000000-0000-4000-8000-000000000002" }, uri: "nirmata://event/60000000-0000-4000-8000-000000000002" }, evidenceUris: ["nirmata://document/80000000-0000-4000-8000-000000000001"] }] }],
          };
          if (command === "derive_loose_ends") return {
            scope: args?.input?.scope,
            findings: [{ code: "ongoing_event", message: "Event technical-id has no end tick", objectRefs: [{ event: "60000000-0000-4000-8000-000000000002" }], evidenceUris: ["nirmata://document/80000000-0000-4000-8000-000000000001"] }],
          };
          if (command === "explore_narrative_continuity") {
            const selection = args?.input?.selection as { kind?: string; startEventId?: string; code?: string };
            return {
              scope: args?.input?.scope,
              selection: selection.kind === "causal_thread" ? { kind: "causal_thread", startEventId: selection.startEventId } : { kind: "loose_end", code: selection.code, objectRef: { event: "60000000-0000-4000-8000-000000000002" } },
              question: "Technical question with nirmata URI",
              alternatives: [{ id: "close_thread", title: "Cerrar el hilo", consequence: "Propone una resolución explícita y sus efectos inmediatos.", proposalRequest: "Resolve technical URI" }],
              sourceUris: ["nirmata://document/80000000-0000-4000-8000-000000000001"],
            };
          }
          if (command === "generate_internal_document") {
            const outcome = localStorage.getItem("nirmata.fixture.narrativeOutcome");
            if (outcome === "failure") throw { code: "provider_response_error", message: "Raw provider failure" };
            if (outcome === "cancelled") throw { code: "provider_cancelled", message: "Raw cancellation" };
            const run = {
              id: "ai-document-run", baseRevision: variantValue.headRevisionId, request: "Documento desde Mara", status: "awaiting_review",
              draft: {
                objective: "Crear Crónica del amanecer", assumptions: [], decisions: [],
                operations: [{ create_document: { operation_id: "document-operation", affected_ids: [], expected_version: 0, retcon: "additive", after: { object: { id: "80000000-0000-4000-8000-000000000002", world_id: variantValue.worldId, title: "Crónica del amanecer", kind: "chronicle", author_entity_id: "40000000-0000-4000-8000-000000000010", perspective_entity_id: "40000000-0000-4000-8000-000000000010", canon_status: "canonical", body_md: "Mara escribe: <img src=x onerror=alert(1)>\n\nEl Archivo despierta.", version: 1, created_at_ms: 1725500000000, updated_at_ms: 1725500000000 }, references: [{ source: { document: "80000000-0000-4000-8000-000000000002" }, target: { event: "60000000-0000-4000-8000-000000000001" }, ordinal: 0 }] } } }],
              },
              validationReport: emptyReport, critiqueReport: { issues: [] }, repairCount: 0, reviewKey: "narrative-review", intentBrief: null, error: null,
            };
            const review = {
              reviewKey: "narrative-review", objective: "Crear Crónica del amanecer", sources: ["nirmata://event/60000000-0000-4000-8000-000000000001"], assumptions: [], baseRevision: variantValue.headRevisionId,
              operations: [{ operationId: "document-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://document/80000000-0000-4000-8000-000000000002", dependencies: [], before: null, after: { title: "Crónica del amanecer", objectType: "document", targetUri: "nirmata://document/80000000-0000-4000-8000-000000000002", lines: [{ label: "Cuerpo", value: "Mara escribe desde el Archivo." }] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
              validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: false,
              freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: true, message: "Vigente" },
            };
            storePending(review, "ai", "Documento interno · Crónica del amanecer", { objectType: "document", sourceUris: review.sources, assumptions: [], values: {} }, String(run.id));
            return run;
          }
          if (command === "read_ai_run") {
            const runId = (args as { runId?: string } | undefined)?.runId;
            const stored = pendingReviews.find((item) => item.aiRunId === runId);
            return { id: runId, baseRevision: variantValue.headRevisionId, request: "Propuesta", status: "awaiting_final_critique", context: { context: { canon: [], perspectives: [], desires: [], obligations: [], searchEvidence: [] } }, draft: { objective: "Propuesta", assumptions: [], operations: [], decisions: [] }, validationReport: emptyReport, critiqueReport: { issues: [] }, repairCount: 0, reviewKey: (stored?.review as Record<string, unknown> | undefined)?.reviewKey ?? null, intentBrief: null, error: null };
          }
          if (command === "read_manual_review") {
            const reviewKey = String(args?.input?.reviewKey ?? "");
            const stored = pendingReviews.find((item) => String((item.review as Record<string, unknown>).reviewKey) === reviewKey);
            if (stored) return structuredClone(stored.review);
          }
          if (command === "read_manual_review" && args?.input?.reviewKey === "narrative-review") return {
            reviewKey: "narrative-review", objective: "Crear Crónica del amanecer", sources: ["nirmata://event/60000000-0000-4000-8000-000000000001"], assumptions: [], baseRevision: variantValue.headRevisionId,
            operations: [{ operationId: "document-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://document/80000000-0000-4000-8000-000000000002", dependencies: [], before: null, after: { title: "Crónica del amanecer", objectType: "document", targetUri: "nirmata://document/80000000-0000-4000-8000-000000000002", lines: [{ label: "Cuerpo", value: "Mara escribe desde el Archivo." }] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
            validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: false,
            freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: true, message: "Vigente" },
          };
          if (command === "read_manual_review" && args?.input?.reviewKey === "template-review") return {
            reviewKey: "template-review", objective: "Expansión desde plantilla",
            sources: [`nirmata://world/${variantValue.worldId}`], assumptions: [], baseRevision: variantValue.headRevisionId,
            operations: [{ operationId: "template-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", dependencies: [], before: null, after: { title: "Expansión preparada", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000099", lines: [] }, issues: emptyReport, effectiveIssues: emptyReport, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
            validationReport: emptyReport, effectiveReport: emptyReport, readyToConfirm: false,
            freshness: { status: "current", currentRevision: variantValue.headRevisionId, canRevalidate: true, message: "Vigente" },
          };
          throw new Error(`Unhandled Tauri command: ${command}`);
        },
      },
    });
  }, { sessionValue: session, variantValue: variant, alternateVariantValue: alternateVariant, archivedVariantValue: archivedVariant });
});

test("rehydrates pending reviews after reload and uses the same discard and apply workflow", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.setItem("nirmata.fixture.persistedPending", "true"));
  await page.reload();
  await page.getByRole("button", { name: /Cambios/ }).click();
  await expect(page.locator("#pending-panel")).toContainText("Guardiana recuperada");
  await expect(page.locator("#pending-panel")).toContainText("Edición manual");
  await page.locator("#pending-panel").getByRole("button", { name: "Descartar propuesta" }).click();
  await page.locator("#pending-panel").getByRole("button", { name: "Sí, descartar propuesta" }).click();
  await expect(page.locator("#pending-panel")).toContainText("No hay cambios pendientes");
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.some((entry) => entry.command === "discard_manual_review"))).toBe(true);

  await page.evaluate(() => localStorage.setItem("nirmata.fixture.persistedPending", "true"));
  await page.reload();
  await page.getByRole("button", { name: /Cambios/ }).click();
  await page.locator("#pending-panel").getByRole("button", { name: "Aplicar al mundo" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.some((entry) => entry.command === "confirm_manual_review"))).toBe(true);
  await expect(page.locator("#pending-panel")).toContainText("No hay cambios pendientes");
});

test("review drawer handles origins, judgment, waiver, decisions, stale, final critique and failure recovery", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.evaluate(() => {
    const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__;
    const original = internals.invoke.bind(internals);
    const empty = { errors: [], conflicts: [], warnings: [], info: [] };
    const warning = { code: "review.warning", severity: "warning", objects: [], message: "Revisa esta consecuencia." };
    let manualReview = {
      reviewKey: "rich-manual-review", objective: "Revisar el Archivo", sources: [], assumptions: [], baseRevision: "revision-0",
      operations: [{ operationId: "rich-operation", decision: "accept", selected: true, severity: "warning", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", dependencies: [], before: { title: "Archivo", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", lines: [{ label: "Resumen", value: "Antes" }] }, after: { title: "Archivo renovado", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000001", lines: [{ label: "Resumen", value: "Después" }] }, issues: { ...empty, warnings: [warning] }, effectiveIssues: { ...empty, warnings: [warning] }, waivers: [] as Array<{ issueCode: string; rationale: string; createdAtMs: number }>, decisionPoints: [{ decisionPointId: "decision-1", prompt: "¿Qué versión conservar?", alternatives: ["keep", "replace"], replacementTarget: null, suggestionAvailable: true, suggestionHidden: true, reason: "Impacto amplio", resolvedAlternative: null as string | null }], risk: { requiresJudgment: true, judgment: null as string | null, suggestedResolutionAvailable: true, suggestedResolutionHidden: true, triggers: [{ code: "wide", title: "Impacto amplio", detail: "Afecta varias piezas." }] } }],
      validationReport: empty, effectiveReport: empty, readyToConfirm: false,
      freshness: { status: "stale", currentRevision: "revision-1", canRevalidate: true, message: "El mundo avanzó." },
    };
    let aiReview = {
      reviewKey: "rich-ai-review", objective: "Propuesta asistida", sources: [], assumptions: [], baseRevision: "revision-1",
      operations: [{ operationId: "ai-operation", decision: "accept", selected: true, severity: "info", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000002", dependencies: [], before: null, after: { title: "Torre propuesta", objectType: "entity", targetUri: "nirmata://entity/40000000-0000-4000-8000-000000000002", lines: [] }, issues: empty, effectiveIssues: empty, waivers: [], decisionPoints: [], risk: { requiresJudgment: false, judgment: null, suggestedResolutionAvailable: false, suggestedResolutionHidden: false, triggers: [] } }],
      validationReport: empty, effectiveReport: empty, readyToConfirm: false,
      freshness: { status: "current", currentRevision: "revision-1", canRevalidate: true, message: "Vigente" },
    };
    let aiStatus = "awaiting_final_critique";
    internals.invoke = async (command: string, args?: Record<string, unknown>) => {
      const input = args?.input as Record<string, unknown> | undefined;
      if (command === "list_pending_reviews") return [
        { review: structuredClone(manualReview), origin: "manual", aiRunId: null, title: "Edición manual rica", merge: null, editorRequest: { objectType: "entity", objective: manualReview.objective, sourceUris: [], assumptions: [], values: {} } },
        { review: structuredClone(aiReview), origin: "ai", aiRunId: "rich-run", title: "Propuesta IA rica", merge: null, editorRequest: { objectType: "entity", objective: aiReview.objective, sourceUris: [], assumptions: [], values: {} } },
      ];
      if (command === "read_manual_review") return structuredClone(input?.reviewKey === manualReview.reviewKey ? manualReview : aiReview);
      if (command === "read_ai_run") return { id: "rich-run", baseRevision: "revision-1", request: "Proponer torre", status: aiStatus, context: { context: { canon: [], perspectives: [], desires: [], obligations: [], searchEvidence: [] } }, draft: { objective: "Propuesta asistida", assumptions: [], operations: [], decisions: [] }, validationReport: empty, critiqueReport: { issues: [{ issueId: "critique-1", summary: { markdown: "Revisar el alcance de la torre." }, severity: "warning", affectedOperationIds: ["ai-operation"], evidence: [] }] }, repairCount: 0, reviewKey: aiReview.reviewKey, intentBrief: null, error: null };
      if (command === "apply_manual_review_action") {
        const action = input?.action as { kind: string; judgment?: string; alternative?: string; rationale?: string };
        const operation = manualReview.operations[0];
        if (action.kind === "record_judgment") {
          operation.risk.judgment = action.judgment ?? null;
          operation.decisionPoints[0].suggestionHidden = false;
        }
        if (action.kind === "resolve_decision") operation.decisionPoints[0].resolvedAlternative = action.alternative ?? null;
        if (action.kind === "add_waiver") operation.waivers.push({ issueCode: "review.warning", rationale: action.rationale ?? "", createdAtMs: Date.now() });
        if (action.kind === "reject") { operation.selected = false; operation.decision = "reject"; }
        if (action.kind === "accept") { operation.selected = true; operation.decision = "accept"; }
        return structuredClone(manualReview);
      }
      if (command === "revalidate_manual_review") {
        manualReview = { ...manualReview, freshness: { status: "current", currentRevision: "revision-1", canRevalidate: false, message: "Vigente" } };
        return structuredClone(manualReview);
      }
      if (command === "revalidate_ai_run") {
        aiStatus = "ready_to_commit";
        aiReview = { ...aiReview, readyToConfirm: true };
        return internals.invoke("read_ai_run", { runId: "rich-run" });
      }
      if (command === "confirm_manual_review" || command === "discard_manual_review") throw { code: "storage_error", message: "fixture failure" };
      if (command === "begin_manual_review_edit") return { objectType: "entity", objective: manualReview.objective, sourceUris: [], assumptions: [], values: { kind: "person", name: "Archivo renovado", slug: "archivo-renovado" } };
      return original(command, args);
    };
    window.dispatchEvent(new Event("visibilitychange"));
  });

  await expect(page.getByRole("button", { name: /Cambios 2 cambios pendientes/u })).toBeVisible();
  await page.getByRole("button", { name: /Cambios 2 cambios pendientes/u }).click();
  const drawer = page.locator("#pending-panel");
  const manualCard = drawer.locator(".pending-card").filter({ hasText: "Edición manual rica" });
  await expect(drawer.getByText("Edición manual", { exact: true })).toBeVisible();
  await expect(drawer.getByText("IA", { exact: true })).toBeVisible();
  await expect(drawer.getByRole("button", { name: "Aplicar al mundo" }).first()).toBeDisabled();
  await drawer.getByRole("button", { name: "Actualizar y volver a comprobar" }).click();
  await drawer.getByRole("button", { name: "Registrar juicio" }).click();
  await drawer.getByLabel("Tu lectura antes de revelar la resolución sugerida").fill("El impacto es aceptable.");
  await drawer.getByRole("button", { name: "Guardar juicio" }).click();
  await drawer.getByRole("button", { name: "keep" }).click();
  await drawer.getByRole("button", { name: "Aceptar advertencia con motivo" }).click();
  await drawer.getByLabel("Motivo para aceptar review.warning").fill("La fuente lo justifica.");
  await drawer.getByRole("button", { name: "Guardar motivo" }).click();
  await manualCard.getByRole("button", { name: "Rechazar operación" }).click();
  await manualCard.getByRole("button", { name: "Aceptar operación" }).click();
  const aiCard = drawer.locator(".pending-card").filter({ hasText: "Propuesta IA rica" });
  await expect(aiCard).toContainText("Crítica final");
  await aiCard.getByRole("button", { name: "Revalidar crítica final" }).click();
  await expect(aiCard.getByRole("button", { name: "Aplicar al mundo" })).toBeEnabled();
  await aiCard.getByRole("button", { name: "Aplicar al mundo" }).click();
  await expect(aiCard).toBeVisible();
  await manualCard.getByRole("button", { name: "Descartar propuesta" }).click();
  await manualCard.getByRole("button", { name: "Sí, descartar propuesta" }).click();
  await expect(manualCard).toBeVisible();
  const failureNotice = page.getByRole("alert").filter({ hasText: "No se pudo completar la transacción" });
  if (await failureNotice.isVisible()) await failureNotice.getByRole("button", { name: "Cerrar" }).click();
  await drawer.locator(".panel-body").evaluate((element) => element.scrollTo(0, 0));
  const accessibility = await new AxeBuilder({ page }).include("#pending-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await expect(page).toHaveScreenshot("review-rich-narrow-390x844.png");
  await manualCard.getByRole("button", { name: "Editar cambio" }).click();
  await expect(drawer).toBeHidden();
  await expect(page.locator("#editor-panel")).toContainText("Edición por operación");
});

async function openNarrativeDocumentForm(page: import("@playwright/test").Page) {
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Estudio narrativo", exact: true }).click();
  const workspace = page.locator(".narrative-workspace");
  await workspace.getByRole("tab", { name: "Documentos" }).click();
  await workspace.getByLabel("Título").fill("Crónica del amanecer");
  await workspace.getByLabel("Instrucciones").fill("Cuenta cómo despierta el Archivo desde la mirada de Mara.");
  await workspace.getByRole("button", { name: "Elegir por nombre" }).click();
  await page.getByRole("dialog").getByLabel("Buscar").fill("Mara");
  await page.getByRole("dialog").getByRole("button", { name: /Mara, custodia del Archivo/u }).click();
  await workspace.getByLabel("Momento").selectOption({ label: "Día 11 · Fundación del Archivo" });
  return workspace;
}

test("workspace themes are distinct, accessible and screenshotable", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  for (const theme of ["light", "dark", "high-contrast"]) {
    await page.goto("/");
    await page.evaluate((value) => localStorage.setItem("nirmata.appearance.theme", value), theme);
    await page.reload();
    await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeVisible();
    await expect(page.locator(".open-shell-topbar").getByText("Escribiendo en", { exact: true })).toBeVisible();
    await expect(page.locator(".open-shell-topbar")).not.toContainText(variant.id);
    await expect(page.locator("#status")).toHaveCount(0);
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
    if (theme !== "high-contrast") {
      await expect(page).toHaveScreenshot(`workspace-home-${theme}-1440x900.png`);
    }
  }
});

test("forced colors preserves workspace controls and focus", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await expect(page.locator("#world-view")).toBeVisible();
  const search = page.locator('input[name="world-search"]');
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
  await expect(page).toHaveScreenshot("workspace-read-only-error-1280x720.png");
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
  await expect(page.locator(".lore-import-workspace")).toBeVisible();
  await expect(page.locator("#simulation-panel")).toBeHidden();
  await navigation.getByRole("button", { name: "Simulación", exact: true }).click();
  await expect(page.locator("#simulation-assumptions")).toHaveValue("El puerto sigue abierto.");

  const overflow = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    elements: Array.from(document.querySelectorAll<HTMLElement>("body *"))
      .filter((element) => { const rect = element.getBoundingClientRect(); return rect.right > window.innerWidth + 1 || rect.left < -1; })
      .map((element) => ({ tag: element.tagName, className: element.className, id: element.id, right: Math.round(element.getBoundingClientRect().right) })),
  }));
  expect(overflow.width, JSON.stringify(overflow.elements)).toBe(0);
});

test("Simulation builds named scenarios and sends selected results to review", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.setItem("nirmata.fixture.simulation", "true"));
  await page.reload();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Simulación", exact: true }).click();

  const panel = page.locator("#simulation-panel");
  await panel.getByLabel("Nombre del escenario").fill("Cosecha del puerto");
  await panel.getByRole("button", { name: "Añadir facciones" }).click();
  await page.getByRole("dialog").getByLabel("Buscar").fill("Mara");
  await page.getByRole("dialog").getByRole("button", { name: /Mara del Puerto/u }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Usar selección" }).click();

  const resources = panel.locator("fieldset").filter({ hasText: "Recursos" }).first();
  await resources.getByRole("button", { name: "Elegir por nombre" }).click();
  await page.getByRole("dialog").getByLabel("Buscar").fill("Grano");
  await page.getByRole("dialog").getByRole("button", { name: /Grano azul/u }).click();
  await resources.getByLabel("Unidad").fill("sacos");

  await panel.getByRole("button", { name: "Añadir existencia" }).click();
  const stocks = panel.locator("fieldset").filter({ hasText: "Existencias iniciales" });
  await stocks.getByLabel("Cantidad").fill("10");
  await stocks.getByLabel("Capacidad").fill("20");
  await panel.getByRole("button", { name: "Añadir regla" }).click();
  await panel.locator("fieldset").filter({ hasText: "Reglas por paso" }).getByLabel("Cantidad").fill("5");
  await panel.getByLabel("Supuestos, uno por línea").fill("El puerto permanece abierto.");
  await panel.getByRole("button", { name: "Guardar escenario" }).click();

  await expect(panel.locator(".simulation-selectors select").first()).toHaveValue("90000000-0000-4000-8000-000000000001");
  await panel.getByRole("button", { name: "Ejecutar una vez" }).click();
  await expect(panel.getByRole("heading", { name: /Mara del Puerto produce 5 Grano azul/u })).toBeVisible();
  await expect(panel).toContainText("15 de 20");
  await expect(panel).not.toContainText("40000000-0000-4000-8000");

  const accessibility = await new AxeBuilder({ page }).include("#simulation-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("simulation-desktop.png"), fullPage: true });

  await panel.getByLabel("Preparar este resultado para revisión").check();
  await panel.getByRole("button", { name: "Preparar selección para revisión" }).click();
  await expect(page.locator("#pending-panel")).toBeVisible();
  await expect(page.locator("#pending-panel")).toContainText("La cosecha aumenta");

  await page.getByRole("button", { name: "Cerrar cambios", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath("simulation-narrow.png"), fullPage: true });
});

test("shell remains operable without horizontal overflow at required narrow sizes", async ({ page }, testInfo) => {
  for (const viewport of [{ width: 720, height: 520 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport);
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeVisible();
    const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
    await navigation.getByRole("button", { name: "Importaciones", exact: true }).click();
    await expect(page.locator(".lore-import-workspace")).toBeVisible();
    await expect(page.locator(".versions-workspace")).toHaveCount(0);
    await expect(page.locator("#simulation-panel")).toBeHidden();
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow).toBe(0);
    await page.screenshot({ path: testInfo.outputPath(`shell-imports-${viewport.width}x${viewport.height}.png`) });
  }
});

test("workspace splitters support pointer, keyboard, collapse and persisted desktop layout", async ({ page }, testInfo) => {
  test.setTimeout(60_000);
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();

  const explorerSeparator = page.getByRole("separator", { name: "Redimensionar Explorador" });
  const contextSeparator = page.getByRole("separator", { name: "Redimensionar Contexto" });
  await expect(page.getByRole("separator")).toHaveCount(2);
  for (const separator of [explorerSeparator, contextSeparator]) {
    await expect(separator).toHaveAttribute("aria-orientation", "vertical");
    await expect(separator).toHaveAttribute("aria-valuemin", "0");
    await expect(separator).toHaveAttribute("aria-valuemax", /\d+/u);
    await expect(separator).toHaveAttribute("aria-valuenow", /\d+/u);
    await expect(separator).toHaveAttribute("aria-valuetext", /píxeles/u);
  }

  const originalExplorerWidth = Number(await explorerSeparator.getAttribute("aria-valuenow"));
  await page.locator('input[name="world-search"]').fill("filtro que debe conservarse");
  await explorerSeparator.focus();
  await page.keyboard.press("ArrowRight");
  await expect(explorerSeparator).toHaveAttribute("aria-valuenow", String(originalExplorerWidth + 16));
  await page.keyboard.press("Home");
  await expect(explorerSeparator).toHaveAttribute("aria-valuenow", "0");
  await expect(explorerSeparator).toHaveAttribute("aria-valuetext", "Explorador colapsado");
  await expect(page.locator("#navigation-panel")).toBeHidden();
  await page.getByRole("button", { name: "Restaurar Explorador" }).click();
  await expect(page.locator("#navigation-panel")).toBeVisible();
  await expect(page.locator('input[name="world-search"]')).toHaveValue("filtro que debe conservarse");
  await expect(explorerSeparator).toHaveAttribute("aria-valuenow", String(originalExplorerWidth + 16));

  await contextSeparator.focus();
  await page.keyboard.press("End");
  const maximumContextWidth = Number(await contextSeparator.getAttribute("aria-valuenow"));
  await page.keyboard.press("ArrowRight");
  await expect(contextSeparator).toHaveAttribute("aria-valuenow", String(maximumContextWidth - 16));
  await page.getByRole("button", { name: "Colapsar Contexto" }).click();
  await expect(contextSeparator).toHaveAttribute("aria-valuetext", "Contexto colapsado");
  await page.reload();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await expect(page.getByRole("separator", { name: "Redimensionar Explorador" })).toHaveAttribute("aria-valuenow", String(originalExplorerWidth + 16));
  await expect(page.getByRole("separator", { name: "Redimensionar Contexto" })).toHaveAttribute("aria-valuetext", "Contexto colapsado");
  await page.getByRole("button", { name: "Restaurar Contexto" }).click();

  const grip = page.getByRole("separator", { name: "Redimensionar Explorador" });
  const gripBox = await grip.boundingBox();
  expect(gripBox).not.toBeNull();
  const startX = gripBox!.x + gripBox!.width / 2;
  const startY = gripBox!.y + 8;
  const beforeDrag = Number(await grip.getAttribute("aria-valuenow"));
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.evaluate(() => {
    const longTasks: number[] = [];
    const observer = new PerformanceObserver((list) => {
      longTasks.push(...list.getEntries().map((entry) => entry.duration));
    });
    try {
      observer.observe({ entryTypes: ["longtask"] });
    } catch {
      // Chromium used by the E2E gate supports longtask; frame cadence remains the fallback.
    }
    const frameTimes: number[] = [];
    const resizeWindow = window as unknown as { __nirmataResizePerformance?: { frameTimes: number[]; longTasks: number[]; observer: PerformanceObserver; active: boolean } };
    resizeWindow.__nirmataResizePerformance = { frameTimes, longTasks, observer, active: true };
    function recordFrame(time: number) {
      const measurement = resizeWindow.__nirmataResizePerformance;
      if (!measurement?.active) return;
      measurement.frameTimes.push(time);
      requestAnimationFrame(recordFrame);
    }
    requestAnimationFrame(recordFrame);
  });
  await page.mouse.move(startX + 80, startY, { steps: 40 });
  await page.mouse.up();
  const resizePerformance = await page.evaluate(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    const measurement = (window as unknown as { __nirmataResizePerformance: { frameTimes: number[]; longTasks: number[]; observer: PerformanceObserver; active: boolean } }).__nirmataResizePerformance;
    measurement.active = false;
    measurement.observer.disconnect();
    const { frameTimes, longTasks } = measurement;
    const duration = frameTimes.at(-1)! - frameTimes[0];
    return {
      fps: (frameTimes.length - 1) * 1000 / duration,
      longestTask: Math.max(0, ...longTasks),
    };
  });
  await expect.poll(async () => Number(await grip.getAttribute("aria-valuenow"))).toBeGreaterThan(beforeDrag);
  expect(resizePerformance.longestTask).toBeLessThanOrEqual(50);
  expect(resizePerformance.fps).toBeGreaterThanOrEqual(55);

  await page.setViewportSize({ width: 1440, height: 960 });
  await expect(page.getByRole("separator")).toHaveCount(2);
  await expect(page.locator("#workspace-shell > .panel:visible")).toHaveCount(3);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBe(0);
  await page.setViewportSize({ width: 960, height: 680 });
  await expect(page.getByRole("separator")).toHaveCount(0);
  await expect(page.getByRole("tablist", { name: "Región de Mundo" })).toBeVisible();
  await expect(page.locator("#workspace-shell > .panel:visible")).toHaveCount(1);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBe(0);
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.screenshot({ path: testInfo.outputPath("workspace-splitters-desktop-1440x960.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole("separator")).toHaveCount(0);
  await expect(page.getByRole("tablist", { name: "Región de Mundo" })).toBeVisible();
  await expect(page.locator("#workspace-shell > .panel:visible")).toHaveCount(1);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBe(0);
  await page.screenshot({ path: testInfo.outputPath("workspace-tabs-narrow-390x844.png") });
});

test("responsive matrix keeps one narrow World region and accessible modal overlays", async ({ page }) => {
  test.setTimeout(90_000);
  const viewports = [
    { width: 390, height: 844 },
    { width: 720, height: 520 },
    { width: 960, height: 680 },
    { width: 1440, height: 900 },
  ];
  for (const viewport of viewports) {
    await page.setViewportSize(viewport);
    expect(page.viewportSize()).toEqual(viewport);
    await page.goto("/");
    const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
    await navigation.getByRole("button", { name: "Mundo", exact: true }).click();
    const regionTabs = page.getByRole("tablist", { name: "Región de Mundo" });
    const panels = page.locator("#workspace-shell > .panel:visible");
    if (viewport.width <= 1180) {
      await expect(regionTabs).toBeVisible();
      await expect(panels).toHaveCount(1);
      await page.locator('input[name="world-search"]').fill("memoria persistente");
      await regionTabs.getByRole("tab", { name: "Editar" }).click();
      await expect(page.locator("#editor-panel")).toBeVisible();
      await regionTabs.getByRole("tab", { name: "Explorar" }).click();
      await expect(page.locator('input[name="world-search"]')).toHaveValue("memoria persistente");
    } else {
      await expect(regionTabs).toBeHidden();
      await expect(panels).toHaveCount(3);
    }
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBe(0);
    await expect(page).toHaveScreenshot(`workspace-light-${viewport.width}x${viewport.height}.png`);

    const assistantTrigger = navigation.getByRole("button", { name: "Asistente", exact: true });
    await assistantTrigger.click();
    const assistant = page.getByRole("dialog", { name: "Asistente" });
    await expect(assistant).toHaveAttribute("aria-modal", "true");
    await expect(page.locator("#root")).toHaveJSProperty("inert", true);
    await expect(page).toHaveScreenshot(`assistant-light-${viewport.width}x${viewport.height}.png`);
    await page.keyboard.press("Escape");
    await expect(assistantTrigger).toBeFocused();

    const changesTrigger = page.getByRole("button", { name: /Cambios 0/u }).first();
    await changesTrigger.click();
    const changes = page.getByRole("dialog", { name: "Cambios pendientes" });
    await expect(changes).toHaveAttribute("aria-modal", "true");
    await expect(page).toHaveScreenshot(`changes-light-${viewport.width}x${viewport.height}.png`);
    await page.keyboard.press("Escape");
    await expect(changesTrigger).toBeFocused();

    for (const areaName of ["Inicio", "Cronología", "Estudio narrativo", "Simulación", "Importaciones", "Versiones"]) {
      await navigation.getByRole("button", { name: areaName, exact: true }).click();
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBe(0);
    }
  }
});

test("open shell owns the viewport and Home returns to visible text", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.longPremise", "true"));
  await page.setViewportSize({ width: 1440, height: 720 });
  await page.goto("/");

  const geometry = await page.evaluate(() => {
    const main = document.querySelector("main")!.getBoundingClientRect();
    const shell = document.querySelector(".open-shell")!.getBoundingClientRect();
    return { main: { x: main.x, y: main.y, width: main.width, height: main.height }, shell: { x: shell.x, y: shell.y, width: shell.width, height: shell.height }, viewport: { width: innerWidth, height: innerHeight } };
  });
  expect(geometry.main).toEqual({ x: 0, y: 0, width: geometry.viewport.width, height: geometry.viewport.height });
  expect(geometry.shell).toEqual({ x: 0, y: 0, width: geometry.viewport.width, height: geometry.viewport.height });

  const home = page.locator(".open-world-home");
  await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeVisible();
  await expect(home.getByText(/Párrafo 1:/u)).toBeVisible();
  await home.evaluate((element) => element.scrollTo(0, element.scrollHeight));
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Inicio", exact: true }).click();
  await expect.poll(() => home.evaluate((element) => element.scrollTop)).toBe(0);
  await expect(page.getByRole("heading", { name: "Mundo vacío", level: 1 })).toBeFocused();
  await expect(home.getByText(/Párrafo 1:/u)).toBeVisible();

  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await expect(page.getByRole("navigation", { name: "Áreas del mundo" }).locator('[aria-current="page"]')).toHaveCount(1);
  await expect(page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true })).toHaveAttribute("aria-expanded", "true");
});

test("high contrast keeps navigation borders neutral", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.appearance.theme", "high-contrast"));
  await page.goto("/");
  const colors = await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Cronología", exact: true }).evaluate((element) => {
    const style = getComputedStyle(element);
    const neutral = getComputedStyle(document.querySelector(".open-shell-sidebar")!).borderRightColor;
    const action = getComputedStyle(document.querySelector(".world-home-actions button")!).borderTopColor;
    return { actual: style.borderTopColor, neutral, action };
  });
  expect(colors.actual).toBe(colors.neutral);
  expect(colors.actual).not.toBe(colors.action);
});

test("Import center distinguishes lore from structured snapshots", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Importaciones", exact: true }).click();
  const center = page.locator("#imports-panel");
    await expect(page.locator(".lore-import-workspace")).toBeVisible();
  await center.getByRole("tab", { name: "Copias de seguridad" }).click();
  await expect(page.locator(".lore-import-workspace")).toBeHidden();
  await expect(center.getByText("Copia estructurada", { exact: false }).first()).toBeVisible();
  await expect(center.getByRole("button", { name: "Elegir copia…" })).toBeEnabled();
  await center.getByRole("tab", { name: "Textos" }).click();
  await expect(page.locator(".lore-import-workspace")).toBeVisible();
});

test("Lore resumes persisted batches with complete candidates and advanced hashes", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.lore", "true"));
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Importaciones", exact: true }).click();
  const lore = page.locator(".lore-import-workspace");
  await expect(lore.getByRole("heading", { name: "Importar material del mundo" })).toBeVisible();
  await expect(lore).toContainText("cronica.md");
  await expect(lore).toContainText("Mara");
  await expect(lore).toContainText("La Custodia");
  await expect(lore).toContainText("líneas 1-2");
  await expect(lore.getByText("sha256:advanced-technical-hash")).toBeHidden();
  await lore.getByRole("button", { name: "Volver a copiar desde el origen" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.some((call) => call.command === "replace_lore_source"))).toBe(true);
  await page.reload();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Importaciones", exact: true }).click();
  await expect(page.locator(".lore-import-workspace")).toContainText("Lote reanudado");
  const accessibility = await new AxeBuilder({ page }).include(".lore-import-workspace").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("lore-resume-1280x900.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBe(0);
  await page.screenshot({ path: testInfo.outputPath("lore-resume-390x844.png") });
});

test("Snapshot uses controlled naming and shows identity and diff before review", async ({ page }, testInfo) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Importaciones", exact: true }).click();
  await page.getByRole("tab", { name: "Copias de seguridad" }).click();
  const snapshot = page.locator(".snapshot-workspace");
  const name = snapshot.getByLabel("Nombre de la carpeta");
  await name.fill("nombre inválido");
  await expect(snapshot.getByRole("button", { name: "Exportar backup" })).toBeDisabled();
  await name.fill("backup-principal");
  await snapshot.getByRole("button", { name: "Elegir carpeta…" }).click();
  await snapshot.getByRole("button", { name: "Exportar backup" }).click();
  await expect(snapshot).toContainText("Canon principal");
  await expect(snapshot).toContainText("12 objetos");
  await expect(snapshot.getByText("sha256:snapshot-export")).toBeHidden();
  await snapshot.getByRole("button", { name: "Elegir copia…" }).click();
  await expect(snapshot).toContainText("1 altas, 1 cambios, 0 bajas");
  await expect(snapshot).toContainText("Torre restaurada");
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.some((call) => call.command === "confirm_manual_review"))).toBe(false);
  await page.screenshot({ path: testInfo.outputPath("snapshot-summary-1280x900.png") });
  await snapshot.getByRole("button", { name: "Abrir revisión" }).click();
  await expect(page.locator("#pending-panel")).toBeVisible();
});

test("Project Ajustes exposes diagnostics, copy actions and backup routing", async ({ page }) => {
  await page.goto("/");
  await page.getByLabel("Más acciones").click();
  await page.getByRole("button", { name: "Ajustes", exact: true }).click();
  const settings = page.getByRole("dialog", { name: "Ajustes" });
  await settings.getByRole("tab", { name: "Proyecto" }).click();
  await expect(settings).toContainText("Versión 11");
  await expect(settings).toContainText("Integridad");
  await expect(settings).toContainText("Correcta");
  await expect(settings).toContainText("sin exponer consultas SQL");
  await settings.getByRole("button", { name: "Abrir backups" }).click();
  await expect(page.getByRole("tab", { name: "Copias de seguridad" })).toHaveAttribute("data-state", "active");
  await expect(page.locator(".snapshot-workspace")).toBeVisible();
});

test("Global errors translate backend codes without exposing raw English", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.loreError", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Importaciones", exact: true }).click();
  const alert = page.locator(".global-feedback");
  await expect(alert).toContainText("La IA tardó demasiado");
  await expect(alert).toContainText("volver a intentarlo");
  await expect(alert).not.toContainText("Azure raw timeout");
  await expect(alert.getByRole("button", { name: "Reintentar" })).toBeVisible();
});

test("command palette opens by keyboard, filters actions and restores focus", async ({ page }) => {
  await page.goto("/");
  const trigger = page.getByRole("button", { name: /Buscar Ctrl K/u });
  await trigger.focus();
  const elapsed = await page.evaluate(() => new Promise<number>((resolve) => {
    const started = performance.now();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }));
    function measure() {
      const active = document.activeElement as HTMLElement | null;
      if (active?.getAttribute("aria-label") === "Buscar objetos y acciones") {
        resolve(performance.now() - started);
        return;
      }
      requestAnimationFrame(measure);
    }
    requestAnimationFrame(measure);
  }));
  const input = page.getByRole("dialog", { name: "Buscar objetos y acciones" }).getByRole("combobox");
  await expect(input).toBeFocused();
  expect(elapsed).toBeLessThan(150);

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
  await expect(page.locator("#editor-panel").locator(".panel-summary")).not.toContainText("40000000");
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

test("Versions explains lineages, compares inline and creates from selected history", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.versions", "true"));
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Versiones", exact: true }).click();
  const workspace = page.getByRole("region", { name: "Versiones", exact: true });
  await expect(workspace.getByRole("heading", { name: "Líneas del mundo" })).toBeVisible();
  await expect(workspace.getByText("Activa para escribir")).toBeVisible();
  await expect(workspace.getByText("Archivada")).toBeVisible();
  await expect(workspace).toContainText("Canon principal: Fundación de la ciudad");
  await workspace.getByRole("button", { name: "Mostrar diferencias" }).click();
  await expect(workspace.getByText("Custodia memorias minerales y rutas.")).toBeVisible();
  await expect(workspace).toContainText("Cambios entre variantes · Reinterpretación");
  await expect(workspace).not.toContainText("reinterpretive");
  await expect(workspace).toContainText("Referencia a regla");
  await expect(workspace).not.toContainText("nirmata://");
  await workspace.getByRole("button", { name: /Fundación de la ciudad/u }).click();
  await workspace.getByLabel("Nombre", { exact: true }).fill("Ruta del exilio");
  await workspace.getByRole("button", { name: "Crear variante" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string; args: { input?: { name?: string; fromRevisionId?: string } } }> }).__nirmataCommands.find((call) => call.command === "create_variant")?.args.input)).toEqual({ name: "Ruta del exilio", fromRevisionId: "20000000-0000-4000-8000-000000000000" });
  const accessibility = await new AxeBuilder({ page }).include(".versions-workspace").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("versions-workspace-1280x900.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(workspace.getByRole("heading", { name: "Versiones", exact: true })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBe(0);
  await page.screenshot({ path: testInfo.outputPath("versions-workspace-390x844.png") });
});

test("Versions filters object history, explains undo and sends merge to global Changes", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.versions", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Versiones", exact: true }).click();
  const workspace = page.getByRole("region", { name: "Versiones", exact: true });
  await workspace.getByRole("searchbox", { name: "Filtrar por objeto o resumen" }).fill("Archivo");
  await expect(workspace.getByRole("button", { name: /Custodia reforzada/u })).toBeVisible();
  await expect(workspace.getByRole("button", { name: /Fundación de la ciudad/u })).toHaveCount(0);
  await expect(workspace.getByText("Siempre crea una versión nueva y nunca borra historia.")).toBeVisible();
  await workspace.getByRole("button", { name: "Deshacer creando otra versión" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.some((call) => call.command === "undo_revision"))).toBe(true);
  await workspace.getByRole("button", { name: "Preparar en Cambios" }).click();
  await expect(page.getByRole("button", { name: /Cambios 1/u })).toBeVisible();
  await page.getByRole("button", { name: /Cambios 1/u }).click();
  const drawer = page.locator("#pending-panel");
  await expect(drawer).toContainText("Traer cambios de Línea alternativa hacia Canon principal");
  await expect(drawer).toContainText("1 cambios independientes");
  await expect(drawer).not.toContainText("technical-id");
  await expect(page).toHaveScreenshot("review-populated-light-1280x720.png");
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
  await expect(page).toHaveScreenshot("workspace-loaded-light-1440x900.png");

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

test("basic world flow keeps implementation vocabulary in closed technical details", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.explorerLoaded", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  const visibleCopy = await page.locator("main").innerText();
  expect(visibleCopy).not.toMatch(/nirmata:\/\/|\b[0-9a-f]{8}-[0-9a-f-]{27,}\b|\b(?:FTS|VFS|head|stale|waiver|DecisionPoint|draft)\b/iu);
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
  await expect(destinationField).toContainText("Archivo de la Aurora");
  await expect(destinationField).not.toContainText("40000000-0000");

  await destinationField.getByText("Introducir UUID o URI manualmente").click();
  await expect(destinationField.getByRole("textbox", { name: "Entidad destino, valor técnico" }))
    .toHaveValue("nirmata://entity/40000000-0000-4000-8000-000000000001");
});

test("structured editor mounts create and canonical edit forms for every aggregate", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.editorAggregates", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  const explorer = page.locator("#navigation-panel");
  const editor = page.locator("#editor-panel");
  const kinds = [
    ["entity", "Entidad"], ["relation", "Relación"], ["event", "Evento"],
    ["claim", "Afirmación"], ["rule", "Regla"], ["goal", "Meta"], ["document", "Documento"],
  ] as const;

  for (const [kind, label] of kinds) {
    await explorer.getByLabel("Nuevo").selectOption(kind);
    await explorer.getByRole("button", { name: "Crear", exact: true }).click();
    await expect(editor.locator(".panel-summary")).toContainText(`Nuevo · ${label}`);
    await expect(editor.getByRole("button", { name: "Preparar cambios", exact: true })).toBeVisible();
  }

  for (const [kind, label] of kinds) {
    await page.keyboard.press("Control+k");
    const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
    await palette.getByRole("combobox").fill(`objeto ${kind}`);
    await palette.getByRole("option", { name: new RegExp(`Objeto ${kind}`, "iu") }).click();
    await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "open_uri").length)).toBeGreaterThan(0);
    await expect(editor.locator(".panel-summary")).toContainText(`Editar · ${label}`);
  }
});

test("entity deletion is prepared for review instead of mutating canon directly", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.editorAggregates", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("objeto entity");
  await palette.getByRole("option", { name: /Objeto entity/iu }).click();
  const editor = page.locator("#editor-panel");
  await editor.getByRole("button", { name: "Eliminar del canon" }).click();
  const confirmation = page.getByRole("dialog", { name: "Preparar eliminación de Archivo de la Aurora" });
  await expect(confirmation).toContainText("El canon no cambiará todavía");
  await confirmation.getByRole("button", { name: "Preparar eliminación" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "prepare_entity_deletion").length)).toBe(1);
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "confirm_manual_review").length)).toBe(0);
  await page.getByRole("button", { name: /Cambios 1 cambios pendientes/u }).click();
  const review = page.locator("#pending-panel");
  await expect(review).toContainText("Eliminar");
  await expect(review.getByRole("button", { name: "Editar cambio" })).toHaveCount(0);
  await expect(review.getByRole("button", { name: "Aplicar al mundo" })).toBeDisabled();
});

test("composed event and document references use named pickers and ordered transport", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.editorAggregates", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  const explorer = page.locator("#navigation-panel");
  const editor = page.locator("#editor-panel");
  await explorer.getByLabel("Nuevo").selectOption("event");
  await explorer.getByRole("button", { name: "Crear", exact: true }).click();
  await editor.getByLabel("Tipo", { exact: true }).fill("fundación");
  await editor.getByLabel("Resumen").fill("Fundación del Archivo");

  await editor.getByRole("button", { name: "Agregar participante" }).click();
  let picker = page.getByRole("dialog", { name: "Agregar participante" });
  await picker.getByRole("searchbox", { name: "Buscar" }).fill("objeto entity");
  await picker.getByRole("button", { name: /Objeto entity/u }).click();
  await editor.getByLabel("Rol").fill("fundadora");

  await editor.getByRole("button", { name: "Agregar vínculo causal" }).click();
  picker = page.getByRole("dialog", { name: "Agregar vínculo causal" });
  await picker.getByRole("searchbox", { name: "Buscar" }).fill("objeto event");
  await picker.getByRole("button", { name: /Objeto event/u }).click();
  await editor.getByLabel("Tipo de vínculo").selectOption("motivates");

  await editor.getByRole("button", { name: "Agregar en Metas afectadas por nombre" }).click();
  picker = page.getByRole("dialog", { name: "Metas afectadas" });
  await picker.getByRole("searchbox", { name: "Buscar" }).fill("objeto goal");
  await picker.getByRole("button", { name: /Objeto goal/u }).click();
  await picker.getByRole("button", { name: "Usar selección" }).click();
  await expect(editor).not.toContainText("40000000-0000");
  await editor.getByRole("button", { name: "Preparar cambios", exact: true }).click();

  const eventValues = await page.evaluate(() => {
    const call = [...(window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { objectType?: string; values?: Record<string, string> } } }> }).__nirmataCommands]
      .reverse().find((entry) => entry.command === "preview_manual_draft" && entry.args?.input?.objectType === "event");
    return call?.args?.input?.values;
  });
  expect(eventValues?.participants).toMatch(/^nirmata:\/\/entity\/.+\|fundadora\|0$/u);
  expect(eventValues?.causal_links).toMatch(/^nirmata:\/\/event\/.+\|motivates$/u);
  expect(eventValues?.affected_goal_ids).toMatch(/^nirmata:\/\/goal\//u);

  await explorer.getByLabel("Nuevo").selectOption("document");
  await explorer.getByRole("button", { name: "Crear", exact: true }).click();
  await editor.getByLabel("Título").fill("Crónica del Archivo");
  await editor.getByRole("button", { name: "Agregar en Referencias de contenido por nombre" }).click();
  picker = page.getByRole("dialog", { name: "Referencias de contenido" });
  await picker.getByRole("searchbox", { name: "Buscar" }).fill("objeto event");
  await picker.getByRole("button", { name: /Objeto event/u }).click();
  await picker.getByRole("searchbox", { name: "Buscar" }).fill("objeto claim");
  await picker.getByRole("button", { name: /Objeto claim/u }).click();
  await picker.getByRole("button", { name: "Usar selección" }).click();
  await editor.getByRole("button", { name: "Subir referencia 2" }).click();
  await editor.getByRole("button", { name: "Preparar cambios", exact: true }).click();

  const documentValues = await page.evaluate(() => {
    const call = [...(window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { objectType?: string; values?: Record<string, string> } } }> }).__nirmataCommands]
      .reverse().find((entry) => entry.command === "preview_manual_draft" && entry.args?.input?.objectType === "document");
    return call?.args?.input?.values;
  });
  expect(documentValues?.content_references).toMatch(/^nirmata:\/\/claim\/.+\|0\nnirmata:\/\/event\/.+\|1$/u);
});

test("RHF reset, dirty navigation guard and backend failure preserve the structured form", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("nirmata.fixture.editorAggregates", "true");
    localStorage.setItem("nirmata.fixture.editorFailure", "true");
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Mundo", exact: true }).click();
  const explorer = page.locator("#navigation-panel");
  const editor = page.locator("#editor-panel");
  await explorer.getByLabel("Nuevo").selectOption("entity");
  await explorer.getByRole("button", { name: "Crear", exact: true }).click();
  const name = editor.getByLabel("Nombre", { exact: true });
  await name.fill("Mara");
  await editor.getByRole("button", { name: "Revertir formulario" }).click();
  await expect(name).toHaveValue("");

  await name.fill("Mara");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("objeto entity");
  await palette.getByRole("option", { name: /Objeto entity/u }).click();
  const discardDialog = page.getByRole("dialog", { name: "Descartar cambios del formulario" });
  await expect(discardDialog).toBeVisible();
  await discardDialog.getByRole("button", { name: "Cancelar" }).click();
  await expect(name).toHaveValue("Mara");
  await expect(editor.locator(".panel-summary")).toContainText("Nuevo · Entidad");

  await editor.getByRole("button", { name: "Preparar cambios", exact: true }).click();
  await expect(name).toHaveValue("Mara");
  await expect(editor).toContainText("La transacción no pudo completarse");
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

test("chronology without a presentation calendar never exposes raw ticks", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.timelineNoCalendar", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Cronología", exact: true }).click();
  const timeline = page.getByRole("region", { name: "Cronología" });
  await expect(timeline.getByText("Tiempo conocido sin calendario de presentación")).toBeVisible();
  await expect(timeline).not.toContainText("Tick 10");
});

test("calendar builders and event date fields keep conversion inside the manual Rust workflow", async ({ page }, testInfo) => {
  test.setTimeout(60_000);
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Cronología", exact: true }).click();
  await page.getByRole("button", { name: "Configurar calendario", exact: true }).click();

  const editor = page.locator("#editor-panel");
  await editor.getByLabel("Calendario").selectOption("fixed");
  await editor.getByLabel("Nombre del calendario").fill("Calendario del Archivo");
  await editor.getByText("Opciones avanzadas").click();
  await editor.getByLabel("Unidad temporal del origen").fill("100");
  await editor.getByLabel("Unidades por día").fill("10");
  await editor.getByText("Opciones avanzadas").click();

  const addDay = editor.getByRole("button", { name: "Agregar día" });
  await addDay.press("Enter");
  await expect(editor.getByLabel("Nombre del día 1")).toBeVisible();
  await addDay.press("Enter");
  await expect(editor.getByLabel("Nombre del día 2")).toBeVisible();
  await addDay.press("Enter");
  await editor.getByLabel("Nombre del día 1").fill("Alba");
  await editor.getByLabel("Nombre del día 2").fill("Cenit");
  await editor.getByLabel("Nombre del día 3").fill("Ocaso");
  await editor.getByRole("button", { name: "Subir día 3" }).click();
  await editor.getByRole("button", { name: "Bajar día 2" }).click();

  await editor.getByRole("button", { name: "Agregar mes" }).click();
  await editor.getByRole("button", { name: "Agregar mes" }).click();
  await editor.getByLabel("Nombre del mes 1").fill("Brasa");
  await editor.getByLabel("Días").nth(0).fill("2");
  await editor.getByLabel("Nombre del mes 2").fill("Lluvia");
  await editor.getByLabel("Días").nth(1).fill("3");
  await expect(editor).not.toContainText("nombre|días");
  await expect(editor).not.toContainText("año|mes|día");
  await page.screenshot({ path: testInfo.outputPath("calendar-builder-desktop.png"), fullPage: true });
  await editor.getByRole("button", { name: "Preparar cambios" }).click();

  const calendarPayload = await page.evaluate(() => {
    const call = [...(window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { objectType?: string; values?: Record<string, string> } } }> }).__nirmataCommands]
      .reverse().find((entry) => entry.command === "preview_manual_draft" && entry.args?.input?.objectType === "world");
    return call?.args?.input?.values;
  });
  expect(calendarPayload).toMatchObject({
    calendar_name: "Calendario del Archivo",
    calendar_weekdays: "Alba\nCenit\nOcaso",
    calendar_months: "Brasa|2\nLluvia|3",
  });

  await page.getByRole("button", { name: /Cambios 1/u }).click();
  const review = page.locator("#pending-panel");
  await expect(review).toContainText("Alba, Cenit, Ocaso");
  await expect(review).toContainText("Brasa (2 días), Lluvia (3 días)");
  await expect(review.getByText(/epoch 100/u)).toBeHidden();
  await review.getByRole("button", { name: "Aplicar al mundo" }).click();
  await expect(page.locator("#status")).toContainText("Cambios aplicados");
  await page.getByRole("button", { name: "Cerrar cambios", exact: true }).click();

  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("crear evento");
  await palette.getByRole("option", { name: "Crear evento" }).click();
  await editor.getByLabel("Tipo", { exact: true }).fill("festival");
  await editor.getByLabel("Resumen").fill("Festival de Lluvia");
  await editor.getByLabel("Tipo de tiempo").selectOption("instant");
  await editor.getByLabel("Año de inicio").fill("2");
  await editor.getByLabel("Mes de inicio").selectOption({ label: "Lluvia" });
  await editor.getByLabel("Día de inicio", { exact: true }).fill("3");
  await editor.getByLabel("Unidad del día de inicio").fill("4");
  await expect(editor.getByLabel("Tick inicio")).toBeHidden();
  await expect(editor).not.toContainText("año|mes|día");
  const accessibility = await new AxeBuilder({ page }).include("#editor-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await editor.getByRole("button", { name: "Preparar cambios" }).click();

  const eventPayload = await page.evaluate(() => {
    const call = [...(window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { objectType?: string; values?: Record<string, string> } } }> }).__nirmataCommands]
      .reverse().find((entry) => entry.command === "preview_manual_draft" && entry.args?.input?.objectType === "event");
    return call?.args?.input?.values;
  });
  expect(eventPayload).toMatchObject({ start_calendar_date: "2|2|3|4", start_tick: "", end_tick: "" });

  await page.getByRole("button", { name: /Cambios 1/u }).click();
  await page.locator("#pending-panel").getByRole("button", { name: "Editar cambio" }).click();
  await expect(page.locator("#pending-panel")).toBeHidden();
  await expect(editor.getByLabel("Año de inicio")).toHaveValue("2");
  await expect(editor.getByLabel("Mes de inicio")).toHaveValue("2");
  await expect(editor.getByLabel("Día de inicio", { exact: true })).toHaveValue("3");
  await expect(editor.getByLabel("Unidad del día de inicio")).toHaveValue("4");
  await editor.getByLabel("Día de inicio", { exact: true }).fill("2");
  await editor.getByRole("button", { name: "Preparar cambios" }).click();
  const editedPayload = await page.evaluate(() => {
    const call = [...(window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { request?: { values?: Record<string, string> } } } }> }).__nirmataCommands]
      .reverse().find((entry) => entry.command === "apply_manual_review_edit");
    return call?.args?.input?.request?.values;
  });
  expect(editedPayload?.start_calendar_date).toBe("2|2|2|4");

  await page.setViewportSize({ width: 390, height: 844 });
  await page.keyboard.press("Control+k");
  await palette.getByRole("combobox").fill("crear evento");
  await palette.getByRole("option", { name: "Crear evento" }).click();
  await editor.getByLabel("Tipo de tiempo").selectOption("interval");
  await expect(editor.getByLabel("Año de fin")).toBeVisible();
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath("event-date-fields-narrow.png"), fullPage: true });
});

test("historical calendar remains read-only in timeline and shell", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.readOnly", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Cronología", exact: true }).click();
  await expect(page.getByRole("button", { name: "Configurar calendario (solo lectura)" })).toBeDisabled();
  await page.getByLabel("Más acciones").click();
  await expect(page.locator(".open-shell-topbar").getByRole("button", { name: "Editar calendario" })).toBeDisabled();
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Buscar objetos y acciones" });
  await palette.getByRole("combobox").fill("configurar calendario");
  await expect(palette.getByRole("option", { name: /Configurar calendario/u })).toHaveAttribute("aria-disabled", "true");
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "preview_manual_draft").length)).toBe(0);
  const accessibility = await new AxeBuilder({ page }).include(".world-timeline").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
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
  await expect(preview.getByRole("link", { name: "Fuente (enlace externo)" })).toHaveAttribute("rel", /noreferrer/u);
  await expect(body).toHaveValue(hostile);
});

test("home offers direct actions and a compact workflow guide", async ({ page }) => {
  await page.goto("/");
  const guide = page.getByRole("heading", { name: "Tú decides qué entra al mundo" });
  await expect(guide).toBeVisible();
  await expect(page.getByRole("button", { name: "Abrir Mundo" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Crear entidad" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Crear evento" })).toBeVisible();
  await expect(page.locator(".world-home-guide")).toContainText("Aplicar al mundo");
  await page.getByRole("button", { name: "Preguntar", exact: true }).click();
  await page.getByRole("button", { name: "Cerrar asistente", exact: true }).last().click();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Inicio", exact: true }).click();
  await page.getByRole("button", { name: "Ocultar guía" }).click();
  await expect(guide).toBeHidden();

  await page.getByLabel("Más acciones").click();
  await page.getByRole("button", { name: "Ayuda", exact: true }).click();
  await page.getByRole("button", { name: "Volver a mostrar la guía del mundo" }).click();
  await expect(guide).toBeVisible();
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

test("six proposal templates prepare locally and continue the same run into review", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await page.getByRole("tab", { name: "Proponer un cambio", exact: true }).click();
  await page.getByRole("button", { name: "Usar una plantilla", exact: true }).click();
  const catalog = page.locator("#assistant-template-catalog");
  await expect(catalog).toBeVisible();
  await expect(catalog.locator("[data-template]" )).toHaveCount(6);
  await expect(catalog).not.toContainText("Novela");
  await catalog.getByRole("button", { name: /^Ciudad/u }).click();
  await expect(page.getByText(/Por qué se prepara este brief/u)).toBeVisible();
  await expect(page.getByText(/no puede aplicarla ni convertirla en canon/u)).toBeVisible();
  await expect(page.getByLabel("Escala").last()).toHaveValue("small");
  await expect(page.getByRole("button", { name: "Continuar al proveedor" })).toBeDisabled();
  const localCalls = await page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.map((entry) => entry.command));
  expect(localCalls.filter((command) => command === "prepare_ai_proposal_template")).toHaveLength(1);
  expect(localCalls.filter((command) => command === "execute_ai_proposal_from_brief")).toHaveLength(0);

  await page.evaluate(() => {
    localStorage.setItem("nirmata.fixture.aiReady", "true");
    localStorage.setItem("nirmata.fixture.paletteSearch", "true");
  });
  await page.reload();
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await page.getByRole("tab", { name: "Proponer un cambio", exact: true }).click();
  await page.getByRole("button", { name: "Usar una plantilla", exact: true }).click();
  await catalog.getByRole("button", { name: /^Ciudad/u }).click();
  const brief = page.locator(".intent-brief-form");
  await brief.getByLabel("Objetivo").fill("Convertir el puerto en una ciudad autónoma");
  await brief.getByLabel("Alcance").fill("Puerto, gobierno y rutas inmediatas");
  await brief.getByLabel("Restricciones, una por línea").fill("Conservar la autoridad existente\nNo inventar fuentes");
  await brief.getByLabel("Escala").selectOption("medium");
  await brief.getByRole("button", { name: "Elegir entidades por nombre" }).click();
  await page.getByRole("dialog").getByLabel("Buscar").fill("Archivo");
  await page.getByRole("dialog").getByRole("button", { name: /Archivo de la Aurora/u }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Usar selección" }).click();
  await brief.evaluate((element) => element.scrollIntoView({ block: "start" }));
  await page.screenshot({ path: testInfo.outputPath("template-brief-desktop.png"), fullPage: true });
  await expect(page).toHaveScreenshot("assistant-brief-light-1280x900.png");
  await brief.getByRole("button", { name: "Continuar al proveedor" }).click();
  await expect(page.getByText(/La propuesta está fuera del canon/u)).toBeVisible();
  await expect(page.getByRole("button", { name: "Abrir en Cambios" })).toBeVisible();
  const continued = await page.evaluate(() => {
    const calls = (window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: Record<string, unknown> } }> }).__nirmataCommands;
    return calls.find((entry) => entry.command === "execute_ai_proposal_from_brief")?.args?.input;
  });
  expect(continued).toMatchObject({
    runId: "template-run-city",
    objective: "Convertir el puerto en una ciudad autónoma",
    scope: "Puerto, gobierno y rutas inmediatas",
    scale: "medium",
  });
  expect(continued).not.toHaveProperty("anchorUri");
  await page.evaluate(() => { const panel = document.querySelector<HTMLElement>("#world-view"); if (panel) panel.scrollTop = 0; });
  const accessibility = await new AxeBuilder({ page }).include("#assistant-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("templates-desktop.png"), fullPage: true });
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /Cambios 1 cambios pendientes/u }).click();
  await expect(page.locator("#pending-panel")).toContainText("Expansión preparada");
  await expect(page.locator("#pending-panel").getByText(`nirmata://world/${variant.worldId}`, { exact: true })).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("templates-review.png"), fullPage: true });
  await page.getByRole("button", { name: "Cerrar cambios", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.evaluate(() => { const panel = document.querySelector<HTMLElement>("#world-view"); if (panel) panel.scrollTop = 0; });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath("templates-narrow.png"), fullPage: true });
});

test("sample prompts only fill or open a mode and never execute AI", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.aiReady", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await page.getByRole("button", { name: "Usar ejemplo" }).click();
  await expect(page.locator("#assistant-input")).toHaveValue("¿Qué tensiones ya aparecen en el canon?");
  await page.getByRole("tab", { name: "Proponer un cambio" }).click();
  await expect(page.locator("#assistant-input")).toHaveValue("");
  await expect(page.getByRole("tab", { name: "Proponer un cambio" })).toHaveAttribute("aria-selected", "true");
  const calls = await page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.map((entry) => entry.command));
  expect(calls.filter((command) => /execute_ai_query|execute_ai_proposal|prepare_ai_proposal_template/u.test(command))).toEqual([]);
});

test("empty world, chronology and changes offer contextual non-writing actions", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
  await navigation.getByRole("button", { name: "Mundo", exact: true }).click();
  await expect(page.getByRole("heading", { name: "El mundo todavía no tiene objetos" }).first()).toBeVisible();
  await expect(page.locator("#editor-content")).toBeHidden();
  await expect(page.getByText(/No hay coincidencias con estos términos/u)).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("empty-world-actions.png"), fullPage: true });
  await page.getByRole("button", { name: "Crear entidad", exact: true }).first().click();
  await expect(page.locator("#structured-editor-react-root").getByRole("button", { name: "Preparar cambios", exact: true })).toBeVisible();

  await navigation.getByRole("button", { name: "Cronología", exact: true }).click();
  await expect(page.getByRole("heading", { name: "La cronología todavía no tiene acontecimientos" })).toBeVisible();
  await page.getByRole("button", { name: "Usar plantilla Cronología" }).click();
  await expect(page.locator("#assistant-panel")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Objetivo", exact: true })).toHaveValue("Objetivo chronology");

  await page.getByRole("button", { name: "Cerrar asistente", exact: true }).last().click();
  await navigation.getByRole("button", { name: "Inicio", exact: true }).click();
  await page.getByRole("button", { name: /Cambios 0 cambios pendientes/u }).click();
  await expect(page.getByRole("button", { name: "Crear cambio manual" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Proponer con IA" }).last()).toBeVisible();
  await expect(page.getByRole("button", { name: "Importar material" }).last()).toBeVisible();
});

test("advanced assistant profiles explain authority and preserve read-only audit", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.readOnly", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();

  await expect(page.getByRole("tab", { name: "Preguntar", exact: true })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Proponer un cambio", exact: true })).toBeDisabled();
  await page.getByText("Más opciones", { exact: true }).click();

  const deep = page.getByRole("button", { name: /Revisión profunda Especialistas de solo lectura/u });
  const audit = page.getByRole("button", { name: /Auditoría del canon Busca problemas/u });
  await expect(deep).toHaveClass(/assistant-profile/u);
  await expect(audit).toHaveClass(/assistant-profile/u);
  await expect(deep).toBeDisabled();
  await expect(audit).toBeEnabled();
  await expect(audit).toContainText("solo lectura");
  await expect(audit).toContainText("no crea propuestas");

  const accessibility = await new AxeBuilder({ page }).include("#assistant-panel").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
});

test("deep review requires role confirmation and audit remains read-only", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.aiReady", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await page.getByText("Más opciones", { exact: true }).click();
  await page.getByRole("button", { name: /Revisión profunda Especialistas/u }).click();
  await page.locator("#assistant-input").fill("Analiza el impacto institucional de independizar la ciudad");
  await page.locator("#assistant-submit").click();

  await expect(page.getByRole("heading", { name: "Confirmar revisión profunda" })).toBeVisible();
  await expect(page.getByText(/Hasta 4 especialistas/u)).toBeVisible();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "execute_deep_review").length)).toBe(0);
  await page.getByRole("button", { name: "Confirmar roles e iniciar" }).click();
  await expect(page.getByRole("heading", { name: "Síntesis enviada a Cambios" })).toBeVisible();
  await expect(page.getByText(/todavía requiere revisión y «Aplicar al mundo»/u)).toBeVisible();
  await page.getByRole("button", { name: "Cerrar asistente" }).click();
  await expect(page.getByRole("button", { name: /Cambios 1 cambios pendientes/u })).toBeVisible();

  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await page.getByRole("tab", { name: "Preguntar", exact: true }).click();
  await page.getByText("Más opciones", { exact: true }).click();
  await page.getByRole("button", { name: /Auditoría del canon Busca problemas/u }).click();
  await page.locator("#assistant-input").fill("Audita reglas y continuidad temporal");
  await page.locator("#assistant-submit").click();
  await expect(page.getByRole("heading", { name: "Confirmar auditoría del canon" })).toBeVisible();
  await expect(page.getByText(/No se crearán operaciones ni propuestas/u)).toBeVisible();
  await page.getByRole("button", { name: "Confirmar roles e iniciar" }).click();
  await expect(page.getByRole("heading", { name: "Hallazgos de la auditoría" })).toBeVisible();
  await expect(page.getByText(/ninguna propuesta creada/u)).toBeVisible();
  await page.getByRole("button", { name: "Cerrar asistente" }).click();
  await expect(page.getByRole("button", { name: /Cambios 1 cambios pendientes/u })).toBeVisible();
});

test("query conversion confirms inherited request and context before changing mode", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.aiReady", "true"));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true }).click();
  await page.locator("#assistant-input").fill("¿Cómo podría independizarse la ciudad?");
  await page.locator("#assistant-submit").click();

  const convert = page.getByRole("button", { name: "Convertir en propuesta" });
  await convert.click();
  await expect(page.getByRole("heading", { name: "Confirmar paso a propuesta" })).toBeVisible();
  await expect(page.getByText("Preparar la independencia de la ciudad", { exact: true })).toBeVisible();
  await expect(page.getByText(/Contexto: contexto general · versión actual/u)).toBeVisible();
  await expect(page.getByRole("tab", { name: "Preguntar" })).toHaveAttribute("aria-selected", "true");

  await page.locator(".proposal-confirmation").getByRole("button", { name: "Cancelar", exact: true }).click();
  await expect(convert).toBeFocused();
  await convert.click();
  await page.getByRole("button", { name: "Continuar a Proponer cambios" }).click();
  await expect(page.getByRole("tab", { name: "Proponer un cambio" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#assistant-input")).toHaveValue("Preparar la independencia de la ciudad");
  await expect(page.locator("#assistant-context")).toContainText("Contexto heredado de la consulta");
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "execute_ai_proposal").length)).toBe(0);
});

test("local conversations persist, send bounded history and delete without changing canon", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("nirmata.fixture.aiReady", "true"));
  await page.goto("/");
  const assistantArea = page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Asistente", exact: true });
  await assistantArea.click();
  await expect(page.getByText("Historial de conversaciones")).toBeVisible();

  await page.locator("#assistant-input").fill("¿Qué controla la ciudad?");
  await page.locator("#assistant-submit").click();
  await page.locator("#assistant-input").fill("¿Y qué necesita para conservarlo?");
  await page.locator("#assistant-submit").click();
  await expect(page.locator(".assistant-message.user-message")).toHaveCount(2);
  await expect(page.getByText("La ciudad necesitaría controlar sus rutas.")).toHaveCount(2);

  const secondHistory = await page.evaluate(() => {
    const calls = (window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { history?: unknown[] } } }> }).__nirmataCommands
      .filter((entry) => entry.command === "execute_ai_query");
    return calls[1]?.args?.input?.history ?? [];
  });
  expect(secondHistory).toHaveLength(1);
  expect(secondHistory[0]).toMatchObject({ userRequest: "¿Qué controla la ciudad?", assistantResponse: "La ciudad necesitaría controlar sus rutas." });

  await page.reload();
  await assistantArea.click();
  await expect(page.locator(".assistant-message.user-message")).toHaveCount(2);
  await page.getByText("Historial de conversaciones").click();
  await page.getByRole("button", { name: "Nueva", exact: true }).click();
  await expect(page.locator(".assistant-message.user-message")).toHaveCount(0);
  await page.locator("#assistant-conversation-select").selectOption({ label: "¿Qué controla la ciudad?" });
  await page.getByRole("button", { name: "Eliminar", exact: true }).click();
  await page.getByRole("button", { name: "Confirmar eliminación" }).click();
  await expect(page.getByText("Conversación eliminada. El mundo y sus propuestas no cambiaron.")).toBeVisible();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => /confirm_manual_review|discard_manual_review|execute_ai_proposal/u.test(entry.command)).length)).toBe(0);

  await page.evaluate((worldId) => localStorage.setItem(`nirmata.assistant.conversations.${worldId}`, '[{"id":"damaged","turns":[{}]}]'), variant.worldId);
  await page.reload();
  await assistantArea.click();
  await expect(page.locator(".assistant-message.user-message")).toHaveCount(0);
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

test("narrative journey derives by name and previews one safe document review", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.addInitScript(() => {
    localStorage.setItem("nirmata.fixture.aiReady", "true");
    localStorage.setItem("nirmata.fixture.narrative", "true");
  });
  await page.goto("/");
  const navigation = page.getByRole("navigation", { name: "Áreas del mundo" });
  await navigation.getByRole("button", { name: "Estudio narrativo", exact: true }).click();
  const workspace = page.locator(".narrative-workspace");

  await expect(workspace.getByRole("tab")).toHaveCount(4);
  await expect(workspace).toContainText("Versión observada · actual");
  await workspace.getByRole("button", { name: "Derivar órdenes" }).click();
  await expect(workspace.getByRole("heading", { name: "Fundación del Archivo" })).toBeVisible();
  await expect(workspace).toContainText("Orden en que se cuenta");

  await workspace.getByRole("tab", { name: "Causalidad" }).click();
  await workspace.getByRole("button", { name: "Elegir acontecimientos" }).click();
  await page.getByRole("dialog").getByLabel("Buscar").fill("Fundación");
  await page.getByRole("dialog").getByRole("button", { name: /Fundación del Archivo/u }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Usar selección" }).click();
  await workspace.getByRole("button", { name: "Derivar causalidad" }).click();
  await expect(workspace).toContainText("Causa");
  await expect(workspace).toContainText("Fundación del Archivo");

  await workspace.getByRole("tab", { name: "Cabos abiertos" }).click();
  await workspace.getByRole("button", { name: "Buscar cabos" }).click();
  await expect(workspace.getByRole("heading", { name: "Acontecimiento en curso" })).toBeVisible();
  await expect(workspace).not.toContainText("technical-id");
  await expect(workspace).not.toContainText("nirmata://");

  const documentWorkspace = await openNarrativeDocumentForm(page);
  await documentWorkspace.getByRole("button", { name: "Generar borrador revisable" }).click();
  const preview = documentWorkspace.locator(".narrative-document-preview");
  await expect(preview.getByRole("heading", { name: "Crónica del amanecer" })).toBeVisible();
  await expect(preview).toContainText("<img src=x onerror=alert(1)>");
  await expect(preview.locator("img, script")).toHaveCount(0);
  await expect(page.locator(".open-shell-topbar .count-badge")).toHaveText("1");

  const accessibility = await new AxeBuilder({ page }).include(".narrative-workspace").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await workspace.evaluate((element) => element.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath("narrative-desktop.png"), fullPage: true });

  await preview.getByRole("button", { name: "Abrir revisión en Cambios" }).click();
  await expect(page.locator("#pending-panel")).toBeVisible();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "read_manual_review").length)).toBe(1);
  await expect(page.locator("#pending-panel")).toContainText("Crónica del amanecer");
  await page.getByRole("button", { name: "Cerrar cambios", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await workspace.evaluate((element) => element.scrollTo(0, 0));
  const documentsTab = workspace.getByRole("tab", { name: "Documentos" });
  await documentsTab.evaluate((element) => element.scrollIntoView({ block: "nearest" }));
  await expect(documentsTab).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath("narrative-narrow.png"), fullPage: true });
});

test("historical narrative remains derivable while document writing is blocked", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("nirmata.fixture.narrative", "true");
    localStorage.setItem("nirmata.fixture.aiReady", "true");
    localStorage.setItem("nirmata.fixture.readOnly", "true");
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Áreas del mundo" }).getByRole("button", { name: "Estudio narrativo", exact: true }).click();
  const workspace = page.locator(".narrative-workspace");
  await expect(workspace).toContainText("Versión observada · solo lectura");
  await workspace.getByRole("button", { name: "Derivar órdenes" }).click();
  await expect(workspace.getByRole("heading", { name: "Fundación del Archivo" })).toBeVisible();
  const scope = await page.evaluate(() => {
    const call = (window as unknown as { __nirmataCommands: Array<{ command: string; args?: { input?: { scope?: unknown } } }> }).__nirmataCommands.find((entry) => entry.command === "derive_narrative_timeline");
    return call?.args?.input?.scope;
  });
  expect(scope).toMatchObject({ variantId: variant.id, revisionId: variant.headRevisionId });
  await workspace.getByRole("tab", { name: "Documentos" }).click();
  await expect(workspace).toContainText("Solo lectura");
  await expect(workspace.getByRole("button", { name: "Generar borrador revisable" })).toBeDisabled();
});

test("failed and cancelled document requests never create review cards", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("nirmata.fixture.narrative", "true");
    localStorage.setItem("nirmata.fixture.aiReady", "true");
    localStorage.setItem("nirmata.fixture.narrativeOutcome", "failure");
  });
  await page.goto("/");
  const workspace = await openNarrativeDocumentForm(page);
  await workspace.getByRole("button", { name: "Generar borrador revisable" }).click();
  await expect(page.locator(".open-shell-topbar .count-badge")).toHaveText("0");
  await expect(workspace.locator(".narrative-document-preview")).toHaveCount(0);
  await page.getByRole("button", { name: "Cerrar aviso" }).click();

  await page.evaluate(() => localStorage.setItem("nirmata.fixture.narrativeOutcome", "cancelled"));
  await workspace.getByRole("button", { name: "Generar borrador revisable" }).click();
  await expect(page.locator(".open-shell-topbar .count-badge")).toHaveText("0");
  await expect.poll(() => page.evaluate(() => (window as unknown as { __nirmataCommands: Array<{ command: string }> }).__nirmataCommands.filter((entry) => entry.command === "read_manual_review").length)).toBe(0);
});
