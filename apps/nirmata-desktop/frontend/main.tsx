import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./assistant.js";
import "./lore-import.js";
import "./narrative.js";
import "./simulation.js";
import "./variant-ui.js";
import { ClosedView } from "./closed-view.js";
import { applyAppearanceTheme, readAppearanceTheme } from "./appearance.js";
import { SessionProvider } from "./session-provider.js";
import { WorldShell } from "./world-shell.js";
import { currentWorldUri } from "./editor-model.js";
import { clearError, setStatus, showError } from "./helpers.js";
import {
  aiBusyBanner,
  aiBusyCancel,
  aiBusyMessage,
  bottomPanelSize,
  closeButton,
  editWorldButton,
  invoke,
  leftPanelSize,
  rightPanelSize,
  state,
  toggleContextButton,
  toggleNavigationButton,
  togglePendingButton,
} from "./state.js";
import type { AiActivitySnapshot, WorldSession } from "./types.js";
import {
  applyCommandStateError,
  applyLayoutState,
  closeSession,
  confirmDiscardPending,
  hasPendingWork,
  openSession,
  refreshNavigation,
  renderWorkspace,
  selectUri,
} from "./workspace.js";

class RootErrorBoundary extends Component<{ children: ReactNode }, { error: string | null }> {
  state = { error: null as string | null };

  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error.message : String(error) };
  }

  componentDidCatch(_error: unknown, _info: ErrorInfo) {
    // Rendering the recoverable message is sufficient; no private state is logged.
  }

  render() {
    if (this.state.error) {
      return (
        <section className="notice warning" role="alert">
          <h2>No se pudo mostrar el inicio</h2>
          <p>{this.state.error}</p>
          <button type="button" onClick={() => window.location.reload()}>Reintentar</button>
        </section>
      );
    }
    return this.props.children;
  }
}

const localOnlyControls = new Set([
  "toggle-navigation",
  "toggle-context",
  "toggle-pending",
  "left-panel-size",
  "right-panel-size",
  "bottom-panel-size",
]);

function syncAiBusyUi(): void {
  const activity = state.aiActivity;
  const busy = activity !== null;
  const worldView = document.querySelector<HTMLElement>("#world-view")!;
  worldView.setAttribute("aria-busy", String(busy));
  aiBusyBanner.hidden = !busy;
  const message = activity
    ? `${activity.label} Las acciones que cambian o recargan el mundo están pausadas.`
    : "";
  if (aiBusyMessage.textContent !== message) {
    aiBusyMessage.textContent = message;
  }
  aiBusyCancel.disabled = !busy;

  Array.from(document.querySelectorAll<HTMLElement>("#world-view, #pending-panel"))
    .flatMap((owner) => Array.from(owner.querySelectorAll<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement | HTMLButtonElement>("input, textarea, select, button")))
    .forEach((control) => {
      const allowed = control === aiBusyCancel || localOnlyControls.has(control.id);
      if (busy) {
        if (control.dataset.aiBusyDisabled === undefined) {
          control.dataset.aiBusyDisabled = String(control.disabled);
        }
        control.disabled = !allowed;
        return;
      }
      const previous = control.dataset.aiBusyDisabled;
      if (previous !== undefined) {
        control.disabled = previous === "true";
        delete control.dataset.aiBusyDisabled;
      }
    });
}

const busyControlObserver = new MutationObserver(() => {
  if (state.aiActivity) syncAiBusyUi();
});
busyControlObserver.observe(document.querySelector("#world-view")!, { childList: true, subtree: true });
window.addEventListener("nirmata:ai-activity-changed", syncAiBusyUi);

aiBusyCancel.addEventListener("click", async () => {
  const requestId = state.aiActivity?.requestId;
  if (!requestId) return;
  aiBusyCancel.disabled = true;
  aiBusyMessage.textContent = "Cancelando la solicitud activa…";
  try {
    await invoke("cancel_ai_request", { requestId });
  } catch (value) {
    applyCommandStateError(value, "La solicitud puede haber terminado antes de cancelarse.");
  }
});

closeButton.addEventListener("click", async () => {
  if (!confirmDiscardPending()) return;
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
  if (uri) void selectUri(uri);
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

window.addEventListener("beforeunload", (event) => {
  if (!hasPendingWork()) return;
  event.preventDefault();
  event.returnValue = "";
});

const root = createRoot(document.querySelector("#closed-root")!);
const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 30_000 } },
});
applyAppearanceTheme(readAppearanceTheme());
root.render(
  <RootErrorBoundary>
    <QueryClientProvider client={queryClient}>
      <SessionProvider>
        <ClosedView />
        <WorldShell />
      </SessionProvider>
    </QueryClientProvider>
  </RootErrorBoundary>,
);

renderWorkspace();
syncAiBusyUi();

void Promise.all([
  invoke<WorldSession | null>("get_current_world"),
  invoke<AiActivitySnapshot>("get_ai_activity"),
])
  .then(([session, activity]) => {
    if (activity.busy && activity.requestIds[0]) {
      state.aiActivity = {
        requestId: activity.requestIds[0],
        source: "unknown",
        label: "Hay una solicitud anterior todavía activa.",
      };
      syncAiBusyUi();
    }
    if (session !== null) openSession(session);
  })
  .catch(showError);
