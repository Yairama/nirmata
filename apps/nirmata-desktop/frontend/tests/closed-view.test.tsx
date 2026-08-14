import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { ClosedView } from "../closed-view.js";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  save: vi.fn(),
  openSession: vi.fn(),
  startProposal: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn().mockResolvedValue("0.1.0") }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: mocks.save }));
vi.mock("../session-provider.js", () => ({ useSession: () => null }));
vi.mock("../workspace.js", () => ({ openSession: mocks.openSession }));

describe("ClosedView", () => {
  beforeEach(() => {
    mocks.invoke.mockReset().mockImplementation((command: string) => {
      if (command === "list_recent_projects" || command === "remember_recent_project") return Promise.resolve([]);
      if (command === "get_ai_provider_status") {
        return Promise.resolve({
          state: "connection_unchecked",
          message: "Configuración local completa.",
          canCheckConnection: true,
          connected: false,
          credential: {
            configured: true,
            source: "session_memory",
            persistence: "session",
            secureStoreAvailable: true,
            limitation: null,
          },
          baseUrl: "https://example.services.ai.azure.com",
          model: "deployment-name",
        });
      }
      return Promise.resolve({ world_id: "world", path: "C:/Mundos/aurora.nirmata", world: { name: "Aurora" } });
    });
    mocks.save.mockReset();
    mocks.openSession.mockReset();
    mocks.startProposal.mockReset();
  });

  afterEach(cleanup);

  function renderClosedView(desktopAction: { id: number; action: "project.new" | "settings.open" } | null = null) {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(<QueryClientProvider client={client}><ClosedView desktopAction={desktopAction} onStartProposal={mocks.startProposal} onStartImport={vi.fn()} /></QueryClientProvider>);
  }

  test("routes native project actions through the existing React owner", async () => {
    renderClosedView({ id: 1, action: "project.new" });
    await screen.findByRole("heading", { name: "Empezar manualmente" });
  });

  test("navigates the three creation paths and restores focus on back", async () => {
    const user = userEvent.setup();
    renderClosedView();

    const manual = screen.getByRole("button", { name: /Empezar manualmente/u });
    expect(screen.getByRole("button", { name: /Crear una base del mundo con IA/u })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Estructurar material existente/u })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: /Crear una base del mundo con IA/u }));
    expect(screen.getByRole("heading", { name: "Crear una base del mundo con IA" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Cancelar" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Empezar manualmente/u })).toBe(document.activeElement);
    });
    expect(mocks.invoke).not.toHaveBeenCalledWith("create_world", expect.anything());
  });

  test("creates once and hands the AI brief to the shared proposal workflow", async () => {
    const user = userEvent.setup();
    const session = { world_id: "world" };
    mocks.save.mockResolvedValue("C:/Mundos/aurora.nirmata");
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_recent_projects" || command === "remember_recent_project") return Promise.resolve([]);
      if (command === "get_ai_provider_status") {
        return Promise.resolve({ state: "credential_missing", message: "Falta la credencial.", canCheckConnection: false, connected: false, credential: { configured: false, source: "missing", persistence: "none", secureStoreAvailable: true, limitation: null } });
      }
      if (command === "create_world") return Promise.resolve(session);
      return Promise.resolve(null);
    });
    renderClosedView();

    await user.click(screen.getByRole("button", { name: /Crear una base del mundo con IA/u }));
    await user.type(screen.getByRole("textbox", { name: "Nombre" }), "Aurora");
    await user.type(screen.getByRole("textbox", { name: "Premisa" }), "La memoria vive en minerales.");
    await user.click(screen.getByRole("button", { name: "Elegir…" }));
    await user.click(screen.getByRole("button", { name: "Continuar" }));
    await user.type(screen.getByRole("textbox", { name: "Género" }), "Fantasía");
    await user.click(screen.getByRole("button", { name: "Revisar creación" }));
    await user.click(screen.getByRole("button", { name: "Crear mundo" }));

    expect(mocks.invoke).toHaveBeenCalledWith("create_world", {
      input: expect.objectContaining({
        path: "C:/Mundos/aurora.nirmata",
        name: "Aurora",
      }),
    });
    expect(mocks.openSession).toHaveBeenCalledWith(session);
    expect(mocks.startProposal).toHaveBeenCalledOnce();
  });

  test("opens Settings and About without a world", async () => {
    const user = userEvent.setup();
    renderClosedView();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Apariencia" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Cerrar Settings" }));
    await user.click(screen.getByRole("button", { name: "Acerca de" }));
    expect(screen.getByRole("dialog", { name: "Acerca de Nirmata" })).toBeTruthy();
    expect(await screen.findByText("0.1.0")).toBeTruthy();
  });

  test("edits the Foundry endpoint and model as local settings", async () => {
    const user = userEvent.setup();
    mocks.invoke.mockImplementation((command: string, payload?: { input?: { baseUrl: string; model: string } }) => {
      if (command === "list_recent_projects") return Promise.resolve([]);
      if (command === "get_ai_provider_status" || command === "set_ai_provider_settings") {
        return Promise.resolve({
          state: "credential_missing",
          message: "Falta la credencial.",
          canCheckConnection: false,
          connected: false,
          credential: { configured: false, source: "missing", persistence: "none", secureStoreAvailable: true, limitation: null },
          baseUrl: payload?.input?.baseUrl ?? "",
          model: payload?.input?.model ?? "",
        });
      }
      return Promise.resolve(null);
    });
    renderClosedView();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("tab", { name: "IA" }));
    const endpoint = await screen.findByRole("textbox", { name: "Endpoint de Microsoft Foundry" });
    const model = screen.getByRole("textbox", { name: "Modelo o deployment" });
    expect(endpoint.getAttribute("placeholder")).toBe("Ej.: https://mi-recurso.services.ai.azure.com");
    expect(model.getAttribute("placeholder")).toBe("Ej.: gpt-5.6-sol");

    await user.type(endpoint, "https://example.services.ai.azure.com");
    await user.type(model, "deployment-name");
    await user.click(screen.getByRole("button", { name: "Guardar endpoint y modelo" }));

    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("set_ai_provider_settings", {
      input: {
        baseUrl: "https://example.services.ai.azure.com",
        model: "deployment-name",
      },
    }));
  });

  test("reopens a recent project with one action", async () => {
    const user = userEvent.setup();
    const recent = {
      path: "C:/Mundos/aurora.nirmata",
      name: "Aurora",
      worldId: "world",
      lastOpenedMs: 1_700_000_000_000,
    };
    const session = { world_id: "world", path: recent.path, world: { name: recent.name } };
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_recent_projects" || command === "remember_recent_project") return Promise.resolve([recent]);
      if (command === "open_world") return Promise.resolve(session);
      if (command === "get_ai_provider_status") return Promise.resolve({ state: "credential_missing", message: "Falta la credencial.", canCheckConnection: false, connected: false, credential: { configured: false, source: "missing", persistence: "none", secureStoreAvailable: true, limitation: null } });
      return Promise.resolve(null);
    });
    renderClosedView();

    await user.click(await screen.findByRole("button", { name: "Abrir Aurora" }));

    expect(mocks.invoke).toHaveBeenCalledWith("open_world", { path: recent.path });
    expect(mocks.openSession).toHaveBeenCalledWith(session);
  });
});
