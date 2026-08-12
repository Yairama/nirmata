import { buildCreateEditor } from "./editor-create.js";
import "./assistant.js";
import "./lore-import.js";
import "./variant-ui.js";
import { currentWorldUri } from "./editor-model.js";
import {
  clearError,
  setStatus,
  showError,
} from "./helpers.js";
import {
  bottomPanelSize,
  chooseCreatePath,
  closeButton,
  createForm,
  dialog,
  editWorldButton,
  epochInput,
  filter,
  invoke,
  leftPanelSize,
  nameInput,
  openButton,
  pathInput,
  premiseInput,
  resultsList,
  rightPanelSize,
  searchForm,
  searchInput,
  state,
  toggleContextButton,
  toggleNavigationButton,
  togglePendingButton,
  uriForm,
  uriInput,
} from "./state.js";
import type { WorldSession } from "./types.js";
import {
  applyCommandStateError,
  applyLayoutState,
  closeSession,
  confirmDiscardPending,
  hasPendingWork,
  openSession,
  refreshNavigation,
  renderWorkspace,
  saveCurrentDraft,
  selectUri,
} from "./workspace.js";

chooseCreatePath.addEventListener("click", async () => {
  try {
    clearError();
    const selected = await dialog.save({
      defaultPath: `${nameInput.value.trim() || "mi-mundo"}.nirmata`,
      filters: filter,
    });
    if (selected !== null) {
      pathInput.value = selected.toLowerCase().endsWith(".nirmata") ? selected : `${selected}.nirmata`;
    }
  } catch (value) {
    showError(value);
  }
});

createForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!pathInput.value) {
    showError("Elige primero dónde guardar el archivo .nirmata.");
    return;
  }

  clearError();
  setStatus("Creando mundo…");
  try {
    const session = await invoke<WorldSession>("create_world", {
      input: {
        path: pathInput.value,
        name: nameInput.value,
        premise_md: premiseInput.value,
        epoch_label: epochInput.value,
      },
    });
    openSession(session);
  } catch (value) {
    showError(value);
    setStatus("");
  }
});

openButton.addEventListener("click", async () => {
  try {
    clearError();
    const selected = await dialog.open({
      multiple: false,
      directory: false,
      filters: filter,
    });
    if (selected === null) {
      return;
    }

    setStatus("Abriendo mundo…");
    openSession(await invoke<WorldSession>("open_world", { path: selected }));
  } catch (value) {
    showError(value);
    setStatus("");
  }
});

closeButton.addEventListener("click", async () => {
  if (!confirmDiscardPending()) {
    return;
  }
  clearError();
  setStatus("Cerrando mundo…");
  try {
    await invoke("close_world");
    closeSession();
  } catch (value) {
    applyCommandStateError(value, "");
  }
});

editWorldButton.addEventListener("click", () => {
  const uri = currentWorldUri();
  if (!uri) {
    return;
  }
  void selectUri(uri);
});

toggleNavigationButton.addEventListener("click", () => {
  state.panels.leftCollapsed = !state.panels.leftCollapsed;
  applyLayoutState();
});

toggleContextButton.addEventListener("click", () => {
  state.panels.rightCollapsed = !state.panels.rightCollapsed;
  applyLayoutState();
});

togglePendingButton.addEventListener("click", () => {
  state.panels.bottomCollapsed = !state.panels.bottomCollapsed;
  applyLayoutState();
});

leftPanelSize.addEventListener("input", () => {
  state.panels.leftWidth = Number(leftPanelSize.value);
  applyLayoutState();
});

rightPanelSize.addEventListener("input", () => {
  state.panels.rightWidth = Number(rightPanelSize.value);
  applyLayoutState();
});

bottomPanelSize.addEventListener("input", () => {
  state.panels.bottomHeight = Number(bottomPanelSize.value);
  applyLayoutState();
});

searchInput.addEventListener("input", () => {
  state.queryText = searchInput.value;
});

searchForm.addEventListener("submit", (event) => {
  event.preventDefault();
  void refreshNavigation();
});

uriForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const uri = uriInput.value.trim();
  if (!uri) {
    return;
  }
  void selectUri(uri);
});

resultsList.addEventListener("keydown", (event) => {
  const options = Array.from(resultsList.querySelectorAll<HTMLButtonElement>('[role="option"]'));
  if (options.length === 0) {
    return;
  }
  const currentIndex = options.findIndex((option) => option === document.activeElement);
  const activeIndex =
    currentIndex >= 0
      ? currentIndex
      : options.findIndex((option) => option.getAttribute("aria-current") === "true");
  let nextIndex = activeIndex >= 0 ? activeIndex : 0;
  switch (event.key) {
    case "ArrowDown":
      nextIndex = Math.min(options.length - 1, nextIndex + 1);
      break;
    case "ArrowUp":
      nextIndex = Math.max(0, nextIndex - 1);
      break;
    case "Home":
      nextIndex = 0;
      break;
    case "End":
      nextIndex = options.length - 1;
      break;
    default:
      return;
  }
  event.preventDefault();
  options[nextIndex]?.focus();
  options[nextIndex]?.click();
});

window.addEventListener("beforeunload", (event) => {
  if (!hasPendingWork()) {
    return;
  }
  event.preventDefault();
  event.returnValue = "";
});

renderWorkspace();

void invoke<WorldSession | null>("get_current_world")
  .then((session) => {
    if (session !== null) {
      openSession(session);
      return;
    }
    setStatus("Crea o abre un mundo para comenzar.");
  })
  .catch(showError);
