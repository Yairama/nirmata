import { buildSelectionEditor } from "./editor-model.js";
import { buildCreateEditor } from "./editor-create.js";
import {
  badge,
  block,
  button,
  formatReferenceLabel,
  humanize,
  normalizeContentReferences,
  objectKindFromUri,
  parseContentReferences,
  serializeContentReferences,
} from "./helpers.js";
import {
  createKinds,
  editorContent,
  editorEmpty,
  editorSubtitle,
  editorTitle,
  state,
} from "./state.js";
import type {
  EditableContentReference,
} from "./helpers.js";
import type {
  EditorField,
  EditorMode,
  ManualFieldIssue,
  ObjectRef,
  SearchObjectKind,
  WorkspaceNotice,
} from "./types.js";
import {
  renderWorkspace,
  resetCurrentEditor,
  saveCurrentDraft,
  selectUri,
} from "./workspace.js";

function issueMap(issues: ManualFieldIssue[]): Map<string, string[]> {
  const grouped = new Map<string, string[]>();
  for (const issue of issues) {
    const list = grouped.get(issue.field) ?? [];
    list.push(issue.message);
    grouped.set(issue.field, list);
  }

  return grouped;
}

export function renderNotice(notice: WorkspaceNotice): HTMLDivElement {
  const card = block(`notice ${notice.kind}`);
  const title = document.createElement("h4");
  title.textContent = notice.title;
  const detail = document.createElement("p");
  detail.textContent = notice.detail;
  card.append(title, detail);
  return card;
}

function renderDocumentReferenceField(
  mode: EditorMode,
  field: EditorField,
  wrapper: HTMLDivElement,
  messages: string[],
): HTMLDivElement {
  const container = block("document-reference-editor");
  const references = normalizeContentReferences(parseContentReferences(mode.values[field.key] ?? ""));
  const list = block("document-reference-list");

  const sync = (nextReferences: EditableContentReference[]): void => {
    mode.values[field.key] = serializeContentReferences(normalizeContentReferences(nextReferences));
    mode.issues = [];
    state.workspaceNotice = null;
    wrapper.classList.toggle("dirty", mode.values[field.key] !== mode.baselineValues[field.key]);
    renderWorkspace();
  };

  if (references.length === 0) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = "Sin referencias. Añade objetivos tipados para mantener el orden del discurso.";
    list.append(empty);
  } else {
    for (const [index, reference] of references.entries()) {
      const card = block("document-reference-card");
      const heading = block("document-reference-heading");
      const title = document.createElement("strong");
      title.textContent = objectKindFromUri(reference.targetUri)
        ? formatReferenceLabel(parseObjectRefFromUri(reference.targetUri)!)
        : reference.targetUri;
      const meta = block("badge-row");
      meta.append(
        badge(`Ordinal ${reference.ordinal}`, "context"),
        badge(objectKindFromUri(reference.targetUri) ? humanize(objectKindFromUri(reference.targetUri)!) : "URI", "kind"),
      );
      heading.append(title, meta);
      const hint = document.createElement("p");
      hint.className = "muted";
      hint.textContent = reference.targetUri;
      const actions = block("inline-actions");
      const openButton = button("Abrir target", "ghost");
      openButton.disabled = objectKindFromUri(reference.targetUri) === null;
      openButton.addEventListener("click", () => {
        void selectUri(reference.targetUri);
      });
      const upButton = button("↑", "secondary");
      upButton.disabled = index === 0;
      upButton.addEventListener("click", () => {
        const next = [...references];
        [next[index - 1], next[index]] = [next[index], next[index - 1]];
        sync(next);
      });
      const downButton = button("↓", "secondary");
      downButton.disabled = index === references.length - 1;
      downButton.addEventListener("click", () => {
        const next = [...references];
        [next[index], next[index + 1]] = [next[index + 1], next[index]];
        sync(next);
      });
      const removeButton = button("Eliminar", "secondary");
      removeButton.addEventListener("click", () => {
        sync(references.filter((_, itemIndex) => itemIndex !== index));
      });
      actions.append(openButton, upButton, downButton, removeButton);
      card.append(heading, hint, actions);
      list.append(card);
    }
  }

  const addSection = block("document-reference-add");
  const addLabel = document.createElement("label");
  addLabel.textContent = "Nueva referencia";
  const addInput = document.createElement("input");
  addInput.type = "text";
  addInput.placeholder = "nirmata://event/...";
  addLabel.append(addInput);
  const addButton = button("Añadir referencia", "secondary");
  addButton.addEventListener("click", () => {
    const targetUri = addInput.value.trim();
    if (!targetUri) {
      return;
    }
    sync([...references, { targetUri, ordinal: references.length }]);
  });
  addSection.append(addLabel, addButton);

  container.append(list, addSection);
  if (field.help) {
    const help = document.createElement("p");
    help.className = "field-help";
    help.textContent = field.help;
    container.append(help);
  }
  if (messages.length > 0) {
    const errorList = document.createElement("ul");
    errorList.className = "field-error-list";
    for (const message of messages) {
      const item = document.createElement("li");
      item.textContent = message;
      errorList.append(item);
    }
    container.append(errorList);
  }
  return container;
}

function parseObjectRefFromUri(uri: string): ObjectRef | null {
  const [, kind, id] = /^nirmata:\/\/(world|entity|relation|event|claim|rule|goal|document)\/(.+)$/u.exec(
    uri,
  ) ?? [null, null, null];
  if (!kind || !id) {
    return null;
  }
  return { [kind]: id } as ObjectRef;
}

export function renderEditor(): void {
  if (!state.editorMode) {
    editorTitle.textContent = "Selecciona o crea un objeto";
    editorSubtitle.textContent = "";
    editorEmpty.hidden = false;
    editorContent.hidden = true;
    editorContent.replaceChildren();
    return;
  }


  const mode = state.editorMode;
  editorTitle.textContent = mode.title;
  editorSubtitle.textContent = `${mode.mode === "create" ? "Nuevo" : "Editar"} · ${humanize(mode.objectType)} · ${mode.subtitle}`;
  const layout = block("editor-layout");
  if (state.workspaceNotice) {
    layout.append(renderNotice(state.workspaceNotice));
  }

  const toolbar = block("editor-section");
  const toolbarTitle = document.createElement("h4");
  toolbarTitle.textContent = "Modo";
  const actions = block("editor-toolbar");
  if (state.selectedObject) {
    const editSelection = button("Editar selección", "secondary");
    editSelection.disabled = mode.mode === "update" && mode.existingUri === state.selectedObject.result.uri;
    editSelection.addEventListener("click", () => {
      state.editorMode = buildSelectionEditor(state.selectedObject!);
      renderWorkspace();
    });
    actions.append(editSelection);
  }
  const kindSelect = document.createElement("select");
  for (const option of createKinds) {
    const item = document.createElement("option");
    item.value = option.value;
    item.textContent = option.label;
    if (option.value === mode.objectType && mode.mode === "create") {
      item.selected = true;
    }
    kindSelect.append(item);
  }
  const newButton = button("Nuevo formulario", "secondary");
  newButton.addEventListener("click", () => {
    state.workspaceNotice = null;
    state.editorMode = buildCreateEditor(kindSelect.value as SearchObjectKind);
    renderWorkspace();
  });
  actions.append(kindSelect, newButton);
  toolbar.append(toolbarTitle, actions);
  layout.append(toolbar);

  const summary = block("editor-section");
  const summaryTitle = document.createElement("h4");
  summaryTitle.textContent = "Lectura activa";
  const summaryText = document.createElement("p");
  summaryText.textContent = mode.description;
  summary.append(summaryTitle, summaryText);
  if (mode.logicalPath) {
    const path = document.createElement("p");
    path.className = "path muted";
    path.textContent = mode.logicalPath;
    summary.append(path);
  }
  layout.append(summary);

  const fieldIssues = issueMap(mode.issues);
  const formSection = block("editor-section");
  const formTitle = document.createElement("h4");
  formTitle.textContent = "Formulario";
  const fieldsWrapper = block("editor-fields");
  for (const field of mode.fields) {
    const wrapper = block("editor-field");
    if (mode.values[field.key] !== mode.baselineValues[field.key]) {
      wrapper.classList.add("dirty");
    }
    const messages = fieldIssues.get(field.key) ?? [];
    if (mode.objectType === "document" && field.key === "content_references") {
      const label = document.createElement("label");
      label.textContent = field.label;
      wrapper.append(label, renderDocumentReferenceField(mode, field, wrapper, messages));
      fieldsWrapper.append(wrapper);
      continue;
    }
    const label = document.createElement("label");
    label.textContent = field.label;
    let control: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;
    switch (field.control) {
      case "textarea": {
        const textarea = document.createElement("textarea");
        textarea.rows = field.rows ?? 5;
        control = textarea;
        break;
      }
      case "select": {
        const select = document.createElement("select");
        for (const option of field.options ?? []) {
          const element = document.createElement("option");
          element.value = option.value;
          element.textContent = option.label;
          if (option.value === mode.values[field.key]) {
            element.selected = true;
          }
          select.append(element);
        }
        control = select;
        break;
      }
      default: {
        const input = document.createElement("input");
        input.type = field.control === "number" ? "number" : "text";
        control = input;
      }
    }
    control.name = field.key;
    control.value = mode.values[field.key] ?? "";
    if (field.placeholder && "placeholder" in control) {
      control.placeholder = field.placeholder;
    }
    if (field.required) {
      control.setAttribute("aria-required", "true");
    }
    control.addEventListener("input", () => {
      mode.values[field.key] = control.value;
      mode.issues = [];
      state.workspaceNotice = null;
      wrapper.classList.toggle("dirty", mode.values[field.key] !== mode.baselineValues[field.key]);
    });
    label.append(control);
    wrapper.append(label);
    if (field.help) {
      const help = document.createElement("p");
      help.className = "field-help";
      help.textContent = field.help;
      wrapper.append(help);
    }
    if (messages.length > 0) {
      const list = document.createElement("ul");
      list.className = "field-error-list";
      for (const message of messages) {
        const item = document.createElement("li");
        item.textContent = message;
        list.append(item);
      }
      wrapper.append(list);
    }
    fieldsWrapper.append(wrapper);
  }
  formSection.append(formTitle, fieldsWrapper);
  layout.append(formSection);

  const workflow = block("editor-section");
  const workflowTitle = document.createElement("h4");
  workflowTitle.textContent = "Workflow manual";
  const objectiveLabel = document.createElement("label");
  objectiveLabel.textContent = "Objetivo del draft";
  const objectiveInput = document.createElement("input");
  objectiveInput.type = "text";
  objectiveInput.value = mode.objective;
  objectiveInput.addEventListener("input", () => {
    mode.objective = objectiveInput.value;
    mode.issues = [];
  });
  objectiveLabel.append(objectiveInput);
  const sourceLabel = document.createElement("label");
  sourceLabel.textContent = "Fuentes (URIs, una por línea)";
  const sourceTextarea = document.createElement("textarea");
  sourceTextarea.rows = 4;
  sourceTextarea.value = mode.sourceUrisText;
  sourceTextarea.addEventListener("input", () => {
    mode.sourceUrisText = sourceTextarea.value;
    mode.issues = [];
  });
  sourceLabel.append(sourceTextarea);
  const assumptionsLabel = document.createElement("label");
  assumptionsLabel.textContent = "Supuestos (uno por línea)";
  const assumptionsTextarea = document.createElement("textarea");
  assumptionsTextarea.rows = 4;
  assumptionsTextarea.value = mode.assumptionsText;
  assumptionsTextarea.addEventListener("input", () => {
    mode.assumptionsText = assumptionsTextarea.value;
    mode.issues = [];
  });
  assumptionsLabel.append(assumptionsTextarea);
  const formActions = block("form-actions");
  const saveButton = button("Guardar draft", "primary");
  saveButton.addEventListener("click", () => {
    void saveCurrentDraft();
  });
  const resetButton = button("Revertir formulario", "secondary");
  resetButton.addEventListener("click", () => {
    resetCurrentEditor();
  });
  formActions.append(saveButton, resetButton);
  workflow.append(workflowTitle, objectiveLabel, sourceLabel, assumptionsLabel, formActions);
  layout.append(workflow);

  const metaSection = block("editor-section");
  const metaTitle = document.createElement("h4");
  metaTitle.textContent = "Metadata";
  const metaList = document.createElement("dl");
  metaList.className = "meta-list";
  for (const item of mode.metadata) {
    const row = block("meta-row");
    const term = document.createElement("dt");
    term.textContent = item.label;
    const definition = document.createElement("dd");
    if (item.uri) {
      const link = button(item.value, "meta-link");
      link.addEventListener("click", () => {
        void selectUri(item.uri!);
      });
      definition.append(link);
    } else {
      definition.textContent = item.value;
    }
    row.append(term, definition);
    metaList.append(row);
  }
  metaSection.append(metaTitle, metaList);
  layout.append(metaSection);

  editorContent.replaceChildren(layout);
  editorEmpty.hidden = true;
  editorContent.hidden = false;
}
