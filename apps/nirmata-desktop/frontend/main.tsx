import { isTauri } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Component, useEffect, useRef, useState } from "react";
import type { ErrorInfo, ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { applyAppearanceTheme, readAppearanceTheme } from "./appearance.js";
import { ClosedView } from "./closed-view.js";
import type { DesktopActionRequest, DesktopMenuAction } from "./desktop-actions.js";
import { FeedbackHost } from "./feedback.js";
import { clearError, setStatus, showError } from "./helpers.js";
import { ObjectPickerProvider } from "./object-picker.js";
import { SessionProvider, useSession } from "./session-provider.js";
import { appActions, invoke, listen, useAppState } from "./state.js";
import type { AiActivitySnapshot, WorldSession } from "./types.js";
import { closeSession, confirmDiscardPending, hasPendingWork } from "./workspace.js";
import { WorkspaceDataProvider } from "./workspace-data.js";
import { WorldShell } from "./world-shell.js";

class RootErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(_error: unknown, _info: ErrorInfo) {
    // Rendering the recoverable message is sufficient; no private state is logged.
  }

  render() {
    if (this.state.failed) {
      return (
        <section className="notice warning" role="alert">
          <h2>No se pudo mostrar esta vista</h2>
          <p>Tu proyecto no cambió. Intenta mostrar la interfaz otra vez sin recargar la aplicación.</p>
          <button type="button" onClick={() => this.setState({ failed: false })}>Reintentar mostrar</button>
        </section>
      );
    }
    return this.props.children;
  }
}

type HandoffIntent =
  | { id: number; kind: "proposal"; request: string }
  | { id: number; kind: "lore-import" };

function App() {
  const session = useSession();
  const state = useAppState();
  const intentId = useRef(0);
  const [handoff, setHandoff] = useState<HandoffIntent | null>(null);
  const desktopActionId = useRef(0);
  const [desktopAction, setDesktopAction] = useState<DesktopActionRequest | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: () => void = () => undefined;
    void listen<DesktopMenuAction>("desktop-menu-action", ({ payload }) => {
      desktopActionId.current += 1;
      setDesktopAction({ id: desktopActionId.current, action: payload });
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten();
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke("set_desktop_menu_state", {
      input: {
        worldOpen: session !== null,
        readOnly: session?.read_only ?? false,
        aiBusy: state.aiActivity !== null,
      },
    }).catch(showError);
  }, [session, state.aiActivity]);

  useEffect(() => {
    if (!desktopAction) return;
    if (desktopAction.action === "project.close" && session) void closeWorld();
    if (desktopAction.action === "app.quit") {
      void (async () => {
        if (!await confirmDiscardPending()) return;
        await invoke("exit_application");
      })().catch(showError);
    }
  }, [desktopAction?.id]);

  useEffect(() => {
    if (!state.status) return;
    const timeout = window.setTimeout(() => appActions.setStatus(""), 4_000);
    return () => window.clearTimeout(timeout);
  }, [state.status]);

  function startProposal(request: string) {
    intentId.current += 1;
    setHandoff({ id: intentId.current, kind: "proposal", request });
  }

  function startImport() {
    intentId.current += 1;
    setHandoff({ id: intentId.current, kind: "lore-import" });
  }

  async function closeWorld() {
    if (!await confirmDiscardPending()) return;
    clearError();
    setStatus("Cerrando mundo…");
    try {
      await invoke("close_world");
      closeSession();
    } catch (value) {
      showError(value);
    }
  }

  return (
    <main>
      {session === null ? (
        <ClosedView
          desktopAction={desktopAction}
          onStartProposal={startProposal}
          onStartImport={startImport}
        />
      ) : (
        <WorldShell
          desktopAction={desktopAction}
          handoff={handoff}
          onHandoffHandled={() => setHandoff(null)}
          aiActivity={state.aiActivity}
          onClose={() => void closeWorld()}
        />
      )}
      {state.status && <p id="status" role="status" aria-live="polite">{state.status}</p>}
      <FeedbackHost />
    </main>
  );
}

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 30_000 } },
});

applyAppearanceTheme(readAppearanceTheme());
createRoot(document.querySelector("#root")!).render(
  <RootErrorBoundary>
    <QueryClientProvider client={queryClient}>
      <SessionProvider>
        <WorkspaceDataProvider>
          <ObjectPickerProvider>
            <App />
          </ObjectPickerProvider>
        </WorkspaceDataProvider>
      </SessionProvider>
    </QueryClientProvider>
  </RootErrorBoundary>,
);

void Promise.all([
  invoke<WorldSession | null>("get_current_world"),
  invoke<AiActivitySnapshot>("get_ai_activity"),
])
  .then(([session, activity]) => {
    appActions.resetWorkspace(session, "");
    if (activity.busy && activity.requestIds[0]) {
      appActions.setAiActivity({
        requestId: activity.requestIds[0],
        source: "unknown",
        label: "Hay una solicitud anterior todavía activa.",
      });
    }
  })
  .catch(showError);

if (!isTauri()) {
  window.addEventListener("beforeunload", (event) => {
    if (!hasPendingWork()) return;
    event.preventDefault();
    event.returnValue = "";
  });
}
