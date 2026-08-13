import { buildCreateEditor } from "./editor-create.js";
import { cloneEditorMode } from "./editor-model.js";
import {
  badge,
  block,
  button,
  clearError,
  firstReviewIssue,
  formatTimestamp,
  humanize,
  labelForUri,
  objectKindFromUri,
  selectedRevisionEntry,
  setStatus,
  shortId,
} from "./helpers.js";
import {
  invoke,
  pendingContent,
  pendingEmpty,
  pendingSummary,
  setSession,
  state,
} from "./state.js";
import type {
  ManualReviewActionRequest,
  ManualReviewObjectSnapshot,
  ManualReviewOperationSnapshot,
  ManualReviewSnapshot,
  ObjectKind,
  PendingDraftRecord,
  RevisionAuditOperationSnapshot,
  RevisionHistoryEntrySnapshot,
  SearchObjectKind,
  AiRunSnapshot,
  ValidationIssue,
  ValidationReport,
  WorldSession,
} from "./types.js";
import {
  applyCommandStateError,
  closeSession,
  confirmDiscardPending,
  loadSelection,
  openReviewOperationEditor,
  openSession,
  refreshNavigation,
  renderWorkspace,
  selectUri,
} from "./workspace.js";

function issueEntries(report: ValidationReport): Array<[string, ValidationIssue[]]> {
  return [
    ["Errores", report.errors],
    ["Conflictos", report.conflicts],
    ["Advertencias", report.warnings],
    ["Información", report.info],
  ];
}

export {
  attachAiReview,
  readPendingDraft,
  renderPending,
  syncPendingReviewRecord,
};

function fallbackAiEditor(targetUri: string) {
  const kind = objectKindFromUri(targetUri) as ObjectKind;
  if (state.editorMode) {
    return cloneEditorMode(state.editorMode);
  }
  return buildCreateEditor((kind === "world" ? "entity" : kind) as SearchObjectKind);
}

async function attachAiReview(run: AiRunSnapshot, title = "Propuesta de IA"): Promise<void> {
  if (!run.reviewKey || !run.draft) {
    return;
  }
  const review = await invoke<ManualReviewSnapshot>("read_manual_review", {
    input: { reviewKey: run.reviewKey },
  });
  review.readyToConfirm = run.status === "ready_to_commit" && review.readyToConfirm;
  const targetUri = review.operations[0]?.targetUri ?? run.reviewKey;
  const kind = objectKindFromUri(targetUri) as ObjectKind;
  state.pendingDrafts.set(run.reviewKey, {
    preview: {
      draftKey: run.reviewKey,
      targetUri,
      objectType: kind,
      mode: state.selectedUri === targetUri ? "update" : "create",
      title,
      objective: review.objective,
      sourceUris: review.sources,
      assumptions: review.assumptions,
      logicalPath: targetUri,
      validationReport: review.validationReport,
      readyToConfirm: review.readyToConfirm,
    },
    review,
    editor: fallbackAiEditor(targetUri),
    aiRunId: run.id,
  });
  state.panels.bottomCollapsed = false;
  renderWorkspace();
  window.dispatchEvent(new CustomEvent<AiRunSnapshot>("nirmata:ai-review-attached", {
    detail: run,
  }));
}

function syncPendingReviewRecord(record: PendingDraftRecord, review: ManualReviewSnapshot): void {
  record.review = review;
  record.preview.objective = review.objective;
  record.preview.sourceUris = [...review.sources];
  record.preview.assumptions = [...review.assumptions];
  record.preview.validationReport = review.validationReport;
  record.preview.readyToConfirm = review.readyToConfirm;
}

async function updateReviewAction(
  record: PendingDraftRecord,
  action: ManualReviewActionRequest,
): Promise<void> {
  clearError();
  setStatus("Revalidando revisión manual…");
  try {
    const review = await invoke<ManualReviewSnapshot>("apply_manual_review_action", {
      input: {
        reviewKey: record.review.reviewKey,
        action,
      },
    });
    syncPendingReviewRecord(record, review);
    renderWorkspace();
    setStatus("Revisión actualizada.");
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

async function addWaiver(
  record: PendingDraftRecord,
  operation: ManualReviewOperationSnapshot,
  issue: ValidationIssue,
): Promise<void> {
  const rationale = window.prompt(`Motivo para aceptar la advertencia ${issue.code}:`, "");
  if (!rationale || !rationale.trim()) {
    return;
  }
  await updateReviewAction(record, {
    kind: "add_waiver",
    operationId: operation.operationId,
    issueCode: issue.code,
    rationale: rationale.trim(),
  });
}

async function recordJudgment(
  record: PendingDraftRecord,
  operation: ManualReviewOperationSnapshot,
): Promise<void> {
  const judgment = window.prompt(
    "Registra tu lectura breve antes de revelar la resolución sugerida:",
    operation.risk.judgment ?? "",
  );
  if (!judgment || !judgment.trim()) {
    return;
  }
  await updateReviewAction(record, {
    kind: "record_judgment",
    operationId: operation.operationId,
    judgment: judgment.trim(),
  });
}

async function readPendingDraft(record: PendingDraftRecord): Promise<void> {
  const review = await invoke<ManualReviewSnapshot>("read_manual_review", {
    input: { reviewKey: record.review.reviewKey },
  });
  syncPendingReviewRecord(record, review);
}

async function revalidatePendingDraft(record: PendingDraftRecord): Promise<void> {
  clearError();
  setStatus("Comprobando contra la versión actual…");
  try {
    const review = await invoke<ManualReviewSnapshot>("revalidate_manual_review", {
      input: { reviewKey: record.review.reviewKey },
    });
    syncPendingReviewRecord(record, review);
    renderWorkspace();
    setStatus(
      review.freshness.status === "current"
        ? "Revisión manual revalidada."
        : "La revisión volvió a cambiar durante el refresco.",
    );
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

async function confirmPendingDraft(record: PendingDraftRecord): Promise<void> {
  if (!confirmDiscardPending("editor")) {
    return;
  }
  clearError();
  setStatus("Aplicando conjunto de cambios…");
  try {
    const session = await invoke<WorldSession>("confirm_manual_review", {
      input: { reviewKey: record.review.reviewKey },
    });
    setSession(session);
    state.pendingDrafts.delete(record.preview.draftKey);
    state.workspaceNotice = {
      kind: "info",
      title: "Cambios aplicados",
      detail: `La revisión ${session.current_revision} quedó aplicada en una única transacción.`,
    };
    state.selectedUri = record.preview.targetUri;
    state.selectedObject = null;
    state.context = null;
    state.selectedLogicalPath = null;
    await refreshNavigation();
    await loadSelection(record.preview.targetUri, true);
    setStatus(`Cambios aplicados: ${record.preview.title}.`);
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function renderReviewObjectSnapshot(
  title: string,
  snapshot: ManualReviewObjectSnapshot | null,
): HTMLDivElement | null {
  if (!snapshot) {
    return null;
  }
  const section = block("review-object");
  const heading = document.createElement("h5");
  heading.textContent = title;
  const objectTitle = document.createElement("strong");
  objectTitle.textContent = snapshot.title;
  const openButton = button("Abrir", "ghost");
  openButton.addEventListener("click", () => {
    void selectUri(snapshot.targetUri);
  });
  const header = block("subsection-header");
  header.append(objectTitle, openButton);
  const list = document.createElement("dl");
  list.className = "meta-list";
  for (const line of snapshot.lines) {
    const row = block("meta-row");
    const term = document.createElement("dt");
    term.textContent = line.label;
    const definition = document.createElement("dd");
    definition.textContent = line.value;
    row.append(term, definition);
    list.append(row);
  }
  section.append(heading, header, list);
  return section;
}

function renderRevisionAuditOperation(operation: RevisionAuditOperationSnapshot): HTMLDivElement {
  const operationCard = block("review-operation-card");
  const operationTitle = document.createElement("h5");
  operationTitle.textContent = operation.after?.title ?? operation.before?.title ?? operation.targetUri;
  const operationMeta = block("badge-row");
  operationMeta.append(
    badge(humanize(operation.decision), "info"),
    badge(operation.source, "context"),
    badge(formatTimestamp(operation.decidedAtMs), "context"),
  );
  operationCard.append(operationTitle, operationMeta);

  const openTarget = button("Abrir objetivo", "ghost");
  openTarget.addEventListener("click", () => {
    void selectUri(operation.targetUri);
  });
  operationCard.append(openTarget);

  const objectGrid = block("review-object-grid");
  const before = renderReviewObjectSnapshot("Antes", operation.before);
  const after = renderReviewObjectSnapshot("Después", operation.after);
  if (before) {
    objectGrid.append(before);
  }
  if (after) {
    objectGrid.append(after);
  }
  if (objectGrid.childElementCount > 0) {
    operationCard.append(objectGrid);
  }

  if (operation.waivers.length > 0) {
    const waiverSection = block("review-issue-group");
    const waiverTitle = document.createElement("h5");
    waiverTitle.textContent = "Advertencias aceptadas con motivo";
    const waiverList = document.createElement("ul");
    waiverList.className = "review-issue-list";
    for (const waiver of operation.waivers) {
      const item = document.createElement("li");
      item.textContent = `${waiver.issueCode}: ${waiver.rationale}`;
      waiverList.append(item);
    }
    waiverSection.append(waiverTitle, waiverList);
    operationCard.append(waiverSection);
  }

  return operationCard;
}

async function undoRevisionFromHistory(entry: RevisionHistoryEntrySnapshot): Promise<void> {
  if (!confirmDiscardPending("editor")) {
    return;
  }
  clearError();
  setStatus(`Deshaciendo revisión ${shortId(entry.revisionId)}…`);
  try {
    const session = await invoke<WorldSession>("undo_revision", {
      input: { revisionId: entry.revisionId },
    });
    setSession(session);
    state.workspaceNotice = {
      kind: "info",
      title: "Cambio deshecho",
      detail: `Se creó una nueva revisión que revierte ${shortId(entry.revisionId)} sin perder la auditoría local.`,
    };
    await refreshNavigation();
    if (state.selectedUri) {
      await loadSelection(state.selectedUri, true);
    }
    setStatus("El cambio se deshizo creando una nueva versión.");
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function renderRevisionHistory(): HTMLDivElement | null {
  if (!state.revisionHistory) {
    return null;
  }

  const card = block("pending-card");
  const title = document.createElement("h4");
  title.textContent = "Historial local de revisiones";
  const detail = document.createElement("p");
  detail.className = "muted";
  detail.textContent = state.revisionHistory.revisions.length === 0
    ? "La versión actual todavía no tiene conjuntos de cambios aplicados."
    : `${state.revisionHistory.revisions.length} versión${state.revisionHistory.revisions.length === 1 ? "" : "es"} en el historial.`;
  card.append(title, detail);

  if (state.revisionHistory.revisions.length === 0) {
    return card;
  }

  const list = block("review-source-list");
  for (const entry of state.revisionHistory.revisions) {
    const item = button("", "linked-button");
    item.setAttribute("aria-current", String(entry.revisionId === state.selectedRevisionId));
    const entryTitle = document.createElement("div");
    entryTitle.className = "linked-button-title";
    entryTitle.textContent = entry.summary;
    const meta = block("badge-row");
    meta.append(
      badge(shortId(entry.revisionId), "context"),
      badge(entry.isCurrentHead ? "Versión actual" : entry.isCurrentUndoTarget ? "Se puede deshacer" : entry.undoneRevisionId ? "Deshacer" : entry.author, entry.isCurrentUndoTarget ? "ready" : "info"),
    );
    const snippet = document.createElement("p");
    snippet.className = "linked-button-snippet";
    snippet.textContent = `${formatTimestamp(entry.createdAtMs)} · ${entry.operations.length} operación${entry.operations.length === 1 ? "" : "es"} · ${entry.waivers.length} advertencia${entry.waivers.length === 1 ? "" : "s"} aceptada${entry.waivers.length === 1 ? "" : "s"}`;
    item.append(entryTitle, meta, snippet);
    item.addEventListener("click", () => {
      state.selectedRevisionId = entry.revisionId;
      renderWorkspace();
    });
    list.append(item);
  }
  card.append(list);

  const selected = selectedRevisionEntry();
  if (!selected) {
    return card;
  }

  const selectedCard = block("review-operation-card");
  const selectedTitle = document.createElement("h5");
  selectedTitle.textContent = `Revisión ${shortId(selected.revisionId)}`;
  const selectedMeta = block("badge-row");
  selectedMeta.append(
    badge(selected.author, "context"),
    badge(formatTimestamp(selected.createdAtMs), "context"),
    badge(selected.isCurrentHead ? "Versión actual" : selected.isCurrentUndoTarget ? "Se puede deshacer" : "Versión anterior", selected.isCurrentUndoTarget ? "ready" : "info"),
  );
  const summary = document.createElement("p");
  summary.textContent = selected.summary;
  const ids = document.createElement("p");
  ids.className = "muted";
  ids.textContent = `Identificador del cambio ${shortId(selected.changeSetId)} · versión anterior ${selected.parentRevisionId ? shortId(selected.parentRevisionId) : "inicial"}`;
  selectedCard.append(selectedTitle, selectedMeta, summary, ids);

  if (selected.undoneRevisionId) {
    const undoneNotice = block("notice info");
    const undoneTitle = document.createElement("h4");
    undoneTitle.textContent = "Deshacer registrado";
    const undoneDetail = document.createElement("p");
    undoneDetail.textContent = `Esta revisión revierte ${shortId(selected.undoneRevisionId)} y mantiene la auditoría before/after accesible.`;
    undoneNotice.append(undoneTitle, undoneDetail);
    selectedCard.append(undoneNotice);
  }

  if (selected.waivers.length > 0) {
    const waiverSection = block("review-issue-group");
    const waiverTitle = document.createElement("h5");
    waiverTitle.textContent = "Advertencias aceptadas con motivo";
    const waiverList = document.createElement("ul");
    waiverList.className = "review-issue-list";
    for (const waiver of selected.waivers) {
      const item = document.createElement("li");
      item.textContent = `${waiver.issueCode}: ${waiver.rationale}`;
      waiverList.append(item);
    }
    waiverSection.append(waiverTitle, waiverList);
    selectedCard.append(waiverSection);
  }

  if (selected.operations.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "La revisión no expone operaciones auditables.";
    selectedCard.append(empty);
  } else {
    for (const operation of selected.operations) {
      selectedCard.append(renderRevisionAuditOperation(operation));
    }
  }

  const actions = block("pending-actions");
  const undo = button("Deshacer esta revisión", "secondary");
  undo.disabled = !selected.isCurrentUndoTarget;
  undo.title = selected.isCurrentUndoTarget
    ? ""
    : state.revisionHistory.undoTargetRevisionId
      ? `Primero debes deshacer ${shortId(state.revisionHistory.undoTargetRevisionId)}.`
      : "No hay revisiones lógicas para deshacer.";
  undo.addEventListener("click", () => {
    void undoRevisionFromHistory(selected);
  });
  actions.append(undo);
  selectedCard.append(actions);
  card.append(selectedCard);
  return card;
}

function renderPending(): void {
  window.dispatchEvent(new CustomEvent("nirmata:pending-changed"));
  const drafts = Array.from(state.pendingDrafts.values()).sort((left, right) =>
    left.preview.title.localeCompare(right.preview.title, "es"),
  );
  const revisionCount = state.revisionHistory?.revisions.length ?? 0;
  pendingSummary.textContent = [
    drafts.length === 0
      ? null
      : `${drafts.length} propuesta${drafts.length === 1 ? "" : "s"} pendiente${drafts.length === 1 ? "" : "s"}`,
    revisionCount === 0
      ? null
      : `${revisionCount} revisión${revisionCount === 1 ? "" : "es"} local${revisionCount === 1 ? "" : "es"}`,
  ].filter(Boolean).join(" · ") || "Sin cambios pendientes.";

  const wrapper = block("pending-list");
  if (state.editorMode && state.editorMode.issues.length > 0) {
    const issuesCard = block("pending-card");
    const title = document.createElement("h4");
    title.textContent = "Último intento sin guardar";
    const detail = document.createElement("p");
    detail.className = "muted";
    detail.textContent = `${state.editorMode.issues.length} validación(es) por campo.`;
    const list = document.createElement("ul");
    for (const issue of state.editorMode.issues) {
      const item = document.createElement("li");
      item.textContent = `${issue.field}: ${issue.message}`;
      list.append(item);
    }
    issuesCard.append(title, detail, list);
    wrapper.append(issuesCard);
  }

  for (const record of drafts) {
    const card = block("pending-card");
    const title = document.createElement("h4");
    title.textContent = record.preview.title;
    const meta = block("badge-row");
    meta.append(
      badge(reviewOrigin(record), "context"),
      badge(humanize(record.preview.objectType), "kind"),
      badge(record.preview.mode === "create" ? "Crear" : "Modificar", "info"),
      badge(
        record.review.freshness.status === "current" ? "Revisión vigente" : "Propuesta desactualizada",
        record.review.freshness.status === "current" ? "ready" : "warning",
      ),
      badge(
        record.review.readyToConfirm ? "Listo para confirmar" : "Bloqueado",
        record.review.readyToConfirm ? "ready" : "warning",
      ),
    );
    const hint = document.createElement("p");
    hint.className = "muted";
    hint.textContent = `${record.preview.logicalPath} · ${record.preview.targetUri} · base ${record.review.baseRevision}`;
    const objective = document.createElement("p");
    objective.textContent = record.review.objective;
    card.append(title, meta, hint, objective);

    if (record.review.freshness.status !== "current") {
      const freshnessNotice = block("notice warning");
      const freshnessTitle = document.createElement("h4");
      freshnessTitle.textContent =
        record.review.freshness.status === "refresh_restart_required"
          ? "Revalidación interrumpida"
          : "Propuesta desactualizada";
      const freshnessDetail = document.createElement("p");
      freshnessDetail.textContent = record.review.freshness.message;
      freshnessNotice.append(freshnessTitle, freshnessDetail);
      if (record.review.freshness.canRevalidate) {
        const revalidate = button("Actualizar y volver a comprobar", "secondary");
        revalidate.addEventListener("click", () => {
          void revalidatePendingDraft(record);
        });
        freshnessNotice.append(revalidate);
      }
      card.append(freshnessNotice);
    }

    if (record.review.sources.length > 0) {
      const sources = block("review-source-list");
      for (const source of record.review.sources) {
        const sourceButton = button(labelForUri(source), "ghost");
        sourceButton.addEventListener("click", () => {
          void selectUri(source);
        });
        sources.append(sourceButton);
      }
      card.append(sources);
    }

    const reviewIssue = firstReviewIssue(record.review.effectiveReport);
    if (reviewIssue) {
      const notice = block(`notice ${reviewIssue.severity === "error" ? "warning" : "info"}`);
      const noticeTitle = document.createElement("h4");
      noticeTitle.textContent = record.review.readyToConfirm
        ? "Revisión validada"
        : "Confirmación deshabilitada";
      const detail = document.createElement("p");
      detail.textContent = reviewIssue.message;
      notice.append(noticeTitle, detail);
      card.append(notice);
    }

    for (const operation of record.review.operations) {
      const operationCard = block("review-operation-card");
      const operationTitle = document.createElement("h5");
      operationTitle.textContent = operation.after?.title ?? operation.before?.title ?? operation.targetUri;
      const operationMeta = block("badge-row");
      operationMeta.append(
        badge(humanize(operation.severity), operation.severity),
        badge(operation.selected ? "Seleccionada" : "Excluida", operation.selected ? "ready" : "warning"),
        badge(humanize(operation.decision), "info"),
      );
      operationCard.append(operationTitle, operationMeta);

      const objectGrid = block("review-object-grid");
      const before = renderReviewObjectSnapshot("Antes", operation.before);
      const after = renderReviewObjectSnapshot("Después", operation.after);
      if (before) {
        objectGrid.append(before);
      }
      if (after) {
        objectGrid.append(after);
      }
      if (objectGrid.childElementCount > 0) {
        operationCard.append(objectGrid);
      }

      if (operation.dependencies.length > 0) {
        const dependencySection = block("review-issue-group");
        const dependencyTitle = document.createElement("h5");
        dependencyTitle.textContent = "Dependencias";
        const dependencyList = block("review-source-list");
        for (const dependency of operation.dependencies) {
          const dependencyButton = button(labelForUri(dependency), "ghost");
          dependencyButton.addEventListener("click", () => {
            void selectUri(dependency);
          });
          dependencyList.append(dependencyButton);
        }
        dependencySection.append(dependencyTitle, dependencyList);
        operationCard.append(dependencySection);
      }

      if (operation.decisionPoints.length > 0) {
        const decisionSection = block("review-issue-group");
        const decisionTitle = document.createElement("h5");
        decisionTitle.textContent = "Decisiones pendientes";
        const decisionList = block("warning-list");
        for (const decision of operation.decisionPoints) {
          const decisionCard = block("warning-card");
          const decisionHeading = document.createElement("h4");
          decisionHeading.textContent = decision.prompt;
          const detail = document.createElement("p");
          detail.textContent = decision.suggestionHidden
            ? "Registra primero tu juicio para revelar la resolución sugerida."
            : [
                decision.resolvedAlternative ? `Resuelta: ${decision.resolvedAlternative}` : null,
                decision.reason ? `Razón: ${decision.reason}` : null,
              ]
                .filter(Boolean)
                .join(" · ");
          const alternatives = block("pending-actions");
          for (const alternative of decision.alternatives) {
            const alternativeButton = button(
              alternative,
              decision.resolvedAlternative === alternative ? "secondary" : "ghost",
            );
            alternativeButton.disabled = decision.suggestionHidden;
            alternativeButton.addEventListener("click", () => {
              void updateReviewAction(record, {
                kind: "resolve_decision",
                decisionPointId: decision.decisionPointId,
                alternative,
              });
            });
            alternatives.append(alternativeButton);
          }
          decisionCard.append(decisionHeading, detail, alternatives);
          decisionList.append(decisionCard);
        }
        decisionSection.append(decisionTitle, decisionList);
        operationCard.append(decisionSection);
      }

      if (operation.risk.triggers.length > 0) {
        const riskSection = block("review-issue-group");
        const riskTitle = document.createElement("h5");
        riskTitle.textContent = "Fricción de alto riesgo";
        const riskList = document.createElement("ul");
        riskList.className = "review-issue-list";
        for (const trigger of operation.risk.triggers) {
          const item = document.createElement("li");
          item.textContent = `${trigger.title}: ${trigger.detail}`;
          riskList.append(item);
        }
        riskSection.append(riskTitle, riskList);
        const judgment = document.createElement("p");
        judgment.className = "muted";
        judgment.textContent = operation.risk.judgment
          ? `Juicio registrado: ${operation.risk.judgment}`
          : operation.risk.suggestedResolutionAvailable
            ? "Debes registrar un juicio breve antes de ver la resolución sugerida."
            : "Registra un juicio breve para dejar constancia antes de confirmar este cambio.";
        riskSection.append(judgment);
        if (!operation.risk.judgment) {
          const judgmentButton = button("Registrar juicio", "secondary");
          judgmentButton.addEventListener("click", () => {
            void recordJudgment(record, operation);
          });
          riskSection.append(judgmentButton);
        }
        operationCard.append(riskSection);
      }

      for (const [label, issues] of issueEntries(operation.effectiveIssues)) {
        if (issues.length === 0) {
          continue;
        }
        const group = block("review-issue-group");
        const groupTitle = document.createElement("h5");
        groupTitle.textContent = label;
        const list = document.createElement("ul");
        list.className = "review-issue-list";
        for (const issue of issues) {
          const item = document.createElement("li");
          const message = document.createElement("span");
          message.textContent = issue.message;
          item.append(message);
          if (issue.severity !== "error") {
            const waiveButton = button("Aceptar advertencia con motivo", "ghost");
            waiveButton.addEventListener("click", () => {
              void addWaiver(record, operation, issue);
            });
            item.append(waiveButton);
          }
          list.append(item);
        }
        group.append(groupTitle, list);
        operationCard.append(group);
      }

      if (operation.waivers.length > 0) {
        const waiverSection = block("review-issue-group");
        const waiverTitle = document.createElement("h5");
        waiverTitle.textContent = "Advertencias aceptadas con motivo";
        const waiverList = document.createElement("ul");
        waiverList.className = "review-issue-list";
        for (const waiver of operation.waivers) {
          const item = document.createElement("li");
          item.textContent = `${waiver.issueCode}: ${waiver.rationale}`;
          waiverList.append(item);
        }
        waiverSection.append(waiverTitle, waiverList);
        operationCard.append(waiverSection);
      }

      const operationActions = block("pending-actions");
      const toggleSelection = button(operation.selected ? "Excluir operación" : "Incluir operación", "secondary");
      toggleSelection.addEventListener("click", () => {
        void updateReviewAction(record, {
          kind: operation.selected ? "reject" : "accept",
          operationId: operation.operationId,
        });
      });
      const editButton = button("Editar cambio", "ghost");
      editButton.addEventListener("click", () => {
        void openReviewOperationEditor(record, operation);
      });
      operationActions.append(toggleSelection, editButton);
      if (operation.before) {
        const viewCurrent = button("Ver objeto actual", "ghost");
        viewCurrent.title = "Abre el estado vigente sin aplicar esta propuesta.";
        viewCurrent.addEventListener("click", () => {
          void selectUri(operation.targetUri);
        });
        operationActions.append(viewCurrent);
      }
      operationCard.append(operationActions);
      card.append(operationCard);
    }

    const actions = block("pending-actions");
    const confirm = button("Aplicar al mundo", "primary");
    confirm.disabled = !record.review.readyToConfirm;
    confirm.title = record.review.readyToConfirm
      ? ""
      : record.review.freshness.status !== "current"
        ? record.review.freshness.message
        : firstReviewIssue(record.review.effectiveReport)?.message ?? "Hay errores, conflictos, dependencias rotas o falta registrar juicio.";
    confirm.addEventListener("click", () => {
      void confirmPendingDraft(record);
    });
    actions.append(confirm);
    const discard = button("Descartar propuesta", "secondary");
    discard.addEventListener("click", async () => {
      if (!window.confirm(`¿Descartar “${record.preview.title}”? El mundo no cambiará.`)) {
        return;
      }
      discard.disabled = true;
      discard.textContent = "Descartando propuesta…";
      try {
        if (record.aiRunId) {
          await invoke("discard_ai_run", { runId: record.aiRunId });
        } else {
          await invoke("discard_manual_review", {
            input: { reviewKey: record.review.reviewKey },
          });
        }
        state.pendingDrafts.delete(record.review.reviewKey);
        if (state.editorMode?.reviewEdit?.reviewKey === record.review.reviewKey) {
          state.editorMode = null;
        }
        renderWorkspace();
        setStatus("Propuesta descartada. El mundo no cambió.");
      } catch (value) {
        discard.disabled = false;
        discard.textContent = "Descartar propuesta";
        applyCommandStateError(value, "No se pudo descartar la propuesta.");
      }
    });
    actions.append(discard);
    card.append(actions);
    wrapper.append(card);
  }

  const history = renderRevisionHistory();
  if (history) {
    wrapper.append(history);
  }

  if (wrapper.childElementCount === 0) {
    pendingEmpty.hidden = false;
    pendingContent.hidden = true;
    pendingContent.replaceChildren();
    return;
  }

  pendingContent.replaceChildren(wrapper);
  pendingEmpty.hidden = true;
  pendingContent.hidden = false;
}

function reviewOrigin(record: PendingDraftRecord): string {
  if (record.aiRunId) return "IA";
  const title = record.preview.title.toLocaleLowerCase("es");
  if (title.includes("import") || record.review.sources.some((source) => source.startsWith("import://"))) return "Importación";
  if (title.includes("simul")) return "Simulación";
  if (title.includes("snapshot")) return "Snapshot";
  if (title.includes("traer cambios") || title.includes("merge")) return "Versiones";
  return "Edición manual";
}
