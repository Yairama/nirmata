import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import * as Tabs from "@radix-ui/react-tabs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { applyAppearanceTheme, readAppearanceTheme } from "./appearance.js";
import { commandErrorCopy, showCommandError, showSuccess } from "./feedback.js";
import type { AppearanceTheme } from "./appearance.js";
import { useSession } from "./session-provider.js";
import type { AiProviderDiagnosticStatus, ProjectDiagnostics } from "./types.js";
import { buttonStyles, chipStyles, noticeStyles } from "./ui-styles.js";

type SoftwareDialog = "settings" | "help" | "about" | null;

const providerStatusKey = ["desktop", "provider-status"] as const;
const settingsFactsClassName = "settings-facts grid gap-0 [&>div]:grid [&>div]:grid-cols-[minmax(7rem,0.4fr)_minmax(0,1fr)] [&>div]:gap-4 [&>div]:border-b [&>div]:border-line [&>div]:py-2 [&>div]:text-sm [&_dt]:text-muted [&_dd]:min-w-0 [&_dd]:break-words";

function providerSource(status: AiProviderDiagnosticStatus): string {
  switch (status.credential.source) {
    case "system_secure_store": return "Almacén seguro del sistema";
    case "session_environment": return "Entorno de esta sesión";
    case "session_memory": return "Memoria de esta sesión";
    default: return "Sin credencial";
  }
}

function SettingsContent({ onBackups, initialTab }: { onBackups: () => void; initialTab: "general" | "ai" }) {
  const session = useSession();
  const queryClient = useQueryClient();
  const [theme, setTheme] = useState<AppearanceTheme>(readAppearanceTheme);
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [providerSettingsError, setProviderSettingsError] = useState("");
  const [credentialError, setCredentialError] = useState("");
  const provider = useQuery({
    queryKey: providerStatusKey,
    queryFn: () => invoke<AiProviderDiagnosticStatus>("get_ai_provider_status"),
    retry: false,
  });
  const project = useQuery({
    queryKey: ["project", session?.world_id, "diagnostics"],
    queryFn: () => invoke<ProjectDiagnostics>("get_project_diagnostics"),
    enabled: Boolean(session),
    retry: false,
  });
  const diagnose = useMutation({
    mutationFn: () => invoke<AiProviderDiagnosticStatus>("diagnose_ai_provider", {
      input: { requestId: crypto.randomUUID() },
    }),
    onSuccess: (status) => {
      queryClient.setQueryData(providerStatusKey, status);
      showSuccess("Conexión comprobada", status.connected ? "Microsoft Foundry está disponible." : "La configuración todavía requiere atención.");
    },
  });
  const saveProviderSettings = useMutation({
    mutationFn: (input: { baseUrl: string; model: string }) =>
      invoke<AiProviderDiagnosticStatus>("set_ai_provider_settings", { input }),
    onSuccess: (status) => {
      setProviderSettingsError("");
      setBaseUrl(status.baseUrl);
      setModel(status.model);
      queryClient.setQueryData(providerStatusKey, status);
      showSuccess("Proveedor guardado", "El endpoint y el modelo quedaron configurados en este equipo.");
    },
  });
  const saveCredential = useMutation({
    mutationFn: (apiKey: string) => invoke("set_provider_api_key", { apiKey }),
    onSuccess: async () => {
      setCredentialError("");
      await queryClient.invalidateQueries({ queryKey: providerStatusKey });
      showSuccess("Credencial guardada", "La credencial quedó protegida por el almacenamiento disponible.");
    },
  });
  const clearCredential = useMutation({
    mutationFn: () => invoke("clear_provider_api_key"),
    onSuccess: async () => {
      setCredentialError("");
      await queryClient.invalidateQueries({ queryKey: providerStatusKey });
      showSuccess("Credencial eliminada", "Nirmata ya no usará esa credencial.");
    },
  });

  useEffect(() => applyAppearanceTheme(theme), [theme]);
  useEffect(() => {
    if (!provider.data) return;
    setBaseUrl(provider.data.baseUrl ?? "");
    setModel(provider.data.model ?? "");
  }, [provider.data]);

  function submitProviderSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextBaseUrl = baseUrl.trim();
    const nextModel = model.trim();
    if (!nextBaseUrl || !nextModel) {
      setProviderSettingsError("Escribe el endpoint HTTPS y el nombre del modelo o deployment.");
      return;
    }
    saveProviderSettings.mutate({ baseUrl: nextBaseUrl, model: nextModel });
  }

  function submitCredential(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const apiKey = String(new FormData(form).get("api-key") ?? "").trim();
    if (!apiKey) {
      setCredentialError("Escribe una credencial antes de guardarla.");
      return;
    }
    saveCredential.mutate(apiKey, { onSuccess: () => form.reset() });
  }

  const providerError = provider.error || diagnose.error || saveProviderSettings.error || saveCredential.error || clearCredential.error;

  async function copyValue(label: string, value: string) {
    try {
      await navigator.clipboard.writeText(value);
      showSuccess(`${label} copiado`, "Ya puedes pegarlo donde lo necesites.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: () => copyValue(label, value) });
    }
  }
  return (
    <Tabs.Root className="settings-tabs grid min-h-96 grid-cols-[11rem_minmax(0,1fr)] max-workspace:grid-cols-1" defaultValue={initialTab} orientation="vertical">
      <Tabs.List className="settings-tab-list flex flex-col gap-1 overflow-visible border-r border-line pb-0 pr-4 max-workspace:flex-row max-workspace:overflow-x-auto max-workspace:border-r-0 max-workspace:border-b max-workspace:pb-3 max-workspace:pr-0" aria-label="Secciones de Ajustes">
        <Tabs.Trigger className="min-h-8 shrink-0 rounded-full px-3 py-1 text-xs data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent" value="general">General</Tabs.Trigger>
        <Tabs.Trigger className="min-h-8 shrink-0 rounded-full px-3 py-1 text-xs data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent" value="appearance">Apariencia</Tabs.Trigger>
        <Tabs.Trigger className="min-h-8 shrink-0 rounded-full px-3 py-1 text-xs data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent" value="ai">IA</Tabs.Trigger>
        <Tabs.Trigger className="min-h-8 shrink-0 rounded-full px-3 py-1 text-xs data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent" value="project">Proyecto</Tabs.Trigger>
        <Tabs.Trigger className="min-h-8 shrink-0 rounded-full px-3 py-1 text-xs data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent" value="accessibility">Accesibilidad</Tabs.Trigger>
        <Tabs.Trigger className="min-h-8 shrink-0 rounded-full px-3 py-1 text-xs data-[state=active]:border-accent data-[state=active]:bg-accent-soft data-[state=active]:text-accent" value="advanced">Avanzado</Tabs.Trigger>
      </Tabs.List>
      <div className="settings-tab-content mt-0 pl-4 max-workspace:mt-4 max-workspace:pl-0">
        <Tabs.Content value="general">
          <h3>General</h3>
          <p>Nirmata guarda mundos como archivos locales <code>.nirmata</code>. No hay sincronización, telemetría ni guardado en nube.</p>
          <p className="muted text-muted">No hay preferencias generales configurables en esta versión.</p>
        </Tabs.Content>
        <Tabs.Content value="appearance">
          <h3>Apariencia</h3>
          <label>Tema
            <select name="appearance-theme" value={theme} onChange={(event) => setTheme(event.target.value as AppearanceTheme)}>
              <option value="system">Usar el del sistema</option>
              <option value="light">Claro</option>
              <option value="dark">Oscuro</option>
              <option value="high-contrast">Alto contraste</option>
            </select>
          </label>
          <p className="muted text-muted">La preferencia se guarda únicamente en este equipo.</p>
        </Tabs.Content>
        <Tabs.Content value="ai">
          <h3>Microsoft Foundry</h3>
          {provider.isPending && <p role="status">Comprobando configuración…</p>}
          {provider.data && (
            <div className="settings-section-stack grid gap-5">
              <p className={chipStyles({ kind: "status", tone: provider.data.connected ? "success" : "info" })}>
                {provider.data.message}
              </p>
              <dl className={settingsFactsClassName}>
                <div><dt>Credencial</dt><dd>{providerSource(provider.data)}</dd></div>
                <div><dt>Persistencia</dt><dd>{provider.data.credential.persistence === "system_secure_store" ? "Persistente" : "Solo esta sesión"}</dd></div>
              </dl>
              <form className="settings-provider-form grid gap-4" onSubmit={submitProviderSettings}>
                <label>Endpoint de Microsoft Foundry
                  <input
                    name="base-url"
                    type="url"
                    value={baseUrl}
                    onChange={(event) => setBaseUrl(event.target.value)}
                    placeholder="Ej.: https://mi-recurso.services.ai.azure.com"
                    autoCapitalize="none"
                    autoComplete="url"
                    spellCheck={false}
                  />
                </label>
                <label>Modelo o deployment
                  <input
                    name="model"
                    value={model}
                    onChange={(event) => setModel(event.target.value)}
                    placeholder="Ej.: gpt-5.6-sol"
                    autoCapitalize="none"
                    autoComplete="off"
                    spellCheck={false}
                  />
                </label>
                <button type="submit" className={buttonStyles()} disabled={saveProviderSettings.isPending}>
                  {saveProviderSettings.isPending ? "Guardando…" : "Guardar endpoint y modelo"}
                </button>
              </form>
              {providerSettingsError && <p role="alert" className="creation-error text-sm text-danger">{providerSettingsError}</p>}
              <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={!provider.data.canCheckConnection || diagnose.isPending} onClick={() => diagnose.mutate()}>
                {diagnose.isPending ? "Comprobando acceso…" : "Comprobar acceso ahora"}
              </button>
              <p className="muted text-muted">No necesitas comprobarlo antes de cada consulta. Úsalo solo al cambiar el endpoint, el modelo o la credencial.</p>
            </div>
          )}
          <form className="settings-credential-form grid gap-4" onSubmit={submitCredential}>
            <label>Clave API
              <input name="api-key" type="password" autoComplete="off" placeholder="Ej.: 0123456789abcdef…" />
            </label>
            <div className="dialog-actions flex flex-wrap items-center gap-2">
              <button type="submit" className={buttonStyles()} disabled={saveCredential.isPending}>Guardar</button>
              <button type="button" className={buttonStyles({ variant: "ghost" })} disabled={clearCredential.isPending} onClick={() => clearCredential.mutate()}>Borrar credencial</button>
            </div>
          </form>
          {credentialError && <p role="alert" className="creation-error text-sm text-danger">{credentialError}</p>}
          {providerError && <p role="alert" className="creation-error text-sm text-danger">No se pudo actualizar la configuración de IA.</p>}
          <p className="muted text-muted">La credencial nunca se devuelve a la interfaz. Nirmata envía únicamente el contexto elegido y solicita al proveedor no almacenar la respuesta.</p>
        </Tabs.Content>
        <Tabs.Content value="project">
          <h3>Proyecto</h3>
          {session ? (
            <div className="settings-section-stack grid gap-5">
              <dl className={settingsFactsClassName}>
                <div><dt>Nombre</dt><dd>{session.world.name}</dd></div>
                <div><dt>Archivo</dt><dd className="copyable-fact grid grid-cols-[1fr_auto] items-center gap-2"><span className="path font-mono">{session.path}</span><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => copyValue("Ruta", session.path)}>Copiar ruta</button></dd></div>
                <div><dt>Escribiendo en</dt><dd>{session.active_variant.name}</dd></div>
                <div><dt>Vista</dt><dd>{session.read_only ? "Versión anterior, solo lectura" : "Versión actual"}</dd></div>
                <div><dt>Schema</dt><dd>{project.data ? `Versión ${project.data.schemaVersion}` : project.isPending ? "Comprobando…" : "No disponible"}</dd></div>
                <div><dt>Integridad</dt><dd>{project.data?.integrity === "ok" ? <span className={chipStyles({ kind: "status", tone: "success" })}>Correcta</span> : "No comprobada"}</dd></div>
              </dl>
              {project.error && (
                <div className={noticeStyles({ tone: "warning" })} role="alert">
                  <strong>{commandErrorCopy(project.error).title}</strong>
                  <p>{commandErrorCopy(project.error).detail}</p>
                  <button type="button" className={buttonStyles({ variant: "secondary" })} onClick={() => project.refetch()}>Reintentar diagnóstico</button>
                </div>
              )}
              <button type="button" className={buttonStyles({ variant: "secondary" })} onClick={onBackups}>Abrir backups</button>
              <p className="muted text-muted">El diagnóstico comprueba la estructura e integridad sin exponer consultas SQL.</p>
            </div>
          ) : <p>Abre un mundo para ver sus datos de proyecto.</p>}
        </Tabs.Content>
        <Tabs.Content value="accessibility">
          <h3>Accesibilidad</h3>
          <p>Nirmata respeta el movimiento reducido y el contraste forzado del sistema. El tema de alto contraste puede activarse en Apariencia.</p>
        </Tabs.Content>
        <Tabs.Content value="advanced">
          <h3>Avanzado</h3>
          {session ? (
            <dl className={`${settingsFactsClassName} technical-facts`}>
              <div><dt>Mundo</dt><dd className="copyable-fact grid grid-cols-[1fr_auto] items-center gap-2"><span>{session.world_id}</span><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => copyValue("ID del mundo", session.world_id)}>Copiar</button></dd></div>
              <div><dt>Revisión</dt><dd className="copyable-fact grid grid-cols-[1fr_auto] items-center gap-2"><span>{session.current_revision}</span><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => copyValue("ID de revisión", session.current_revision)}>Copiar</button></dd></div>
              <div><dt>Variante</dt><dd className="copyable-fact grid grid-cols-[1fr_auto] items-center gap-2"><span>{session.active_variant.id}</span><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => copyValue("ID de variante", session.active_variant.id)}>Copiar</button></dd></div>
            </dl>
          ) : <p>Los identificadores técnicos estarán disponibles al abrir un mundo.</p>}
        </Tabs.Content>
      </div>
    </Tabs.Root>
  );
}

function AboutContent() {
  const version = useQuery({ queryKey: ["desktop", "version"], queryFn: getVersion, retry: false });
  return (
    <div className="about-content grid gap-5">
      <p className="eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Nirmata</p>
      <h3>Software editorial para mundos coherentes</h3>
      <dl className={settingsFactsClassName}>
        <div><dt>Versión</dt><dd>{version.data ?? "No disponible"}</dd></div>
        <div><dt>Identificador</dt><dd>com.nirmata.desktop</dd></div>
        <div><dt>Licencia declarada</dt><dd>MIT</dd></div>
      </dl>
      <p>Aplicación de escritorio local-first. El canon vive en tu archivo <code>.nirmata</code> y la IA es opcional.</p>
      <p>Las propuestas de IA nunca se aplican directamente: se revisan como conjuntos de cambios antes de entrar al mundo.</p>
      <h4>Ayuda rápida</h4>
      <ul>
        <li><strong>Preguntar</strong> consulta el canon sin modificarlo.</li>
        <li><strong>Proponer cambios</strong> prepara una revisión que tú decides aplicar o descartar.</li>
        <li><strong>Versión actual</strong> es la vista editable; las versiones anteriores son de solo lectura.</li>
      </ul>
    </div>
  );
}

function HelpContent({ onAbout, onShowOnboarding }: { onAbout: () => void; onShowOnboarding: () => void }) {
  return (
    <div className="help-content grid gap-5">
      <nav className="help-index flex flex-wrap gap-2" aria-label="Temas de ayuda">
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-create">Crear</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-ai">Preguntar y proponer</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-versions">Versiones</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-import">Importar</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-changes">Cambios</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-tools">Herramientas</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-privacy">Privacidad</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-shortcuts">Atajos</a>
        <a className="rounded-full border border-line px-2.5 py-1 text-ink no-underline hover:bg-subtle" href="#help-glossary">Glosario</a>
      </nav>
      <section id="help-create">
        <h3>Crear un mundo</h3>
        <p><strong>Manual</strong> crea un proyecto vacío. <strong>Base con IA</strong> prepara contenido revisable. <strong>Material existente</strong> copia texto de forma inerte para extraer candidatos.</p>
      </section>
      <button type="button" className={buttonStyles({ variant: "secondary" })} onClick={onShowOnboarding}>Volver a mostrar la guía del mundo</button>
      <section id="help-ai">
        <h3>Preguntar no es modificar</h3>
        <p>Preguntar consulta el canon y muestra fuentes. Proponer cambios prepara un conjunto que puedes revisar, editar, aplicar o descartar. La IA nunca aplica canon por sí sola.</p>
      </section>
      <section id="help-versions">
        <h3>Entender las versiones</h3>
        <p><strong>Escribiendo en</strong> indica el canon que recibirá cambios. <strong>Viendo</strong> indica la versión observada. Una versión anterior siempre es de solo lectura.</p>
      </section>
      <section id="help-import">
        <h3>Importar con seguridad</h3>
        <p>Un texto externo es una fuente, no canon. Nirmata conserva procedencia, presenta elementos y exige revisión. Una copia de seguridad estructurada es distinta de Markdown o texto.</p>
      </section>
      <section id="help-changes">
        <h3>Cambios y recuperación</h3>
        <p>Antes y Después explican cada operación. Puedes editar, rechazar, aceptar advertencias con motivo, volver a comprobar y descartar. <strong>Aplicar al mundo</strong> es la única escritura de canon y ocurre en una transacción.</p>
        <p>Las revisiones pendientes se guardan dentro del proyecto y reaparecen al abrir su variante.</p>
      </section>
      <section id="help-tools">
        <h3>Simulación, narrativa y calendario</h3>
        <p>Simulación permanece fuera del canon hasta seleccionar resultados. Estudio narrativo deriva historia y prepara documentos revisables. Calendario presenta fechas humanas sin sustituir la unidad temporal autoritativa.</p>
      </section>
      <section id="help-privacy">
        <h3>Privacidad</h3>
        <p>El canon y las conversaciones locales permanecen en este equipo. La IA es opcional; recibe contexto acotado con solicitud de no almacenamiento, y la credencial nunca vuelve a la interfaz.</p>
      </section>
      <section id="help-shortcuts">
        <h3>Atajos</h3>
        <dl className={settingsFactsClassName}>
          <div><dt>Ctrl/Cmd + K</dt><dd>Abrir búsqueda y acciones</dd></div>
          <div><dt>Ctrl/Cmd + N / O</dt><dd>Crear o abrir un mundo desde el menú</dd></div>
          <div><dt>F1</dt><dd>Abrir este Centro de ayuda</dd></div>
          <div><dt>Escape</dt><dd>Cerrar diálogo o palette y devolver el foco</dd></div>
          <div><dt>Tab / Shift + Tab</dt><dd>Recorrer controles</dd></div>
        </dl>
      </section>
      <section id="help-glossary">
        <h3>Glosario</h3>
        <dl className={settingsFactsClassName}>
          <div><dt>Canon</dt><dd>Información aceptada como verdadera en esa versión del mundo.</dd></div>
          <div><dt>Propuesta</dt><dd>Conjunto de cambios todavía fuera del canon.</dd></div>
          <div><dt>Variante</dt><dd>Línea canónica nombrada que puede evolucionar de forma independiente.</dd></div>
          <div><dt>Fuente</dt><dd>Objeto o fragmento que justifica una consulta o cambio.</dd></div>
        </dl>
      </section>
      <button type="button" className={buttonStyles({ variant: "secondary" })} onClick={onAbout}>Acerca de Nirmata</button>
    </div>
  );
}

export function SoftwareDialogs({ active, onActiveChange, returnFocus, onOpenBackups, onShowOnboarding, settingsInitialTab = "general" }: {
  active: SoftwareDialog;
  onActiveChange: (dialog: SoftwareDialog) => void;
  returnFocus?: HTMLElement | null;
  onOpenBackups: () => void;
  onShowOnboarding: () => void;
  settingsInitialTab?: "general" | "ai";
}) {
  const title = active === "settings" ? "Ajustes" : active === "help" ? "Centro de ayuda" : "Acerca de Nirmata";
  return (
    <Dialog.Root open={active !== null} onOpenChange={(open) => !open && onActiveChange(null)}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content
          className="software-dialog fixed left-1/2 top-1/2 z-50 max-h-[calc(100dvh-2rem)] w-[min(42rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-2xl border border-line bg-raised p-6 shadow-overlay outline-none [&>*+*]:mt-4"
          aria-describedby="software-dialog-description"
          onCloseAutoFocus={(event) => {
            if (!returnFocus) return;
            event.preventDefault();
            returnFocus.focus();
          }}
        >
          <div className="dialog-heading flex items-start justify-between gap-4 border-b border-line pb-4">
            <div>
              <Dialog.Title>{title}</Dialog.Title>
              <Dialog.Description id="software-dialog-description">
                {active === "settings"
                  ? "Configura la aplicación sin modificar el canon."
                  : active === "help"
                    ? "Creación, cambios, versiones, importación y atajos."
                    : "Información, privacidad y arquitectura local-first."}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild><button type="button" className={buttonStyles({ variant: "ghost" })} aria-label={`Cerrar ${title}`}>Cerrar</button></Dialog.Close>
          </div>
          {active === "settings"
            ? <SettingsContent initialTab={settingsInitialTab} onBackups={() => { onActiveChange(null); onOpenBackups(); }} />
            : active === "help"
              ? <HelpContent onAbout={() => onActiveChange("about")} onShowOnboarding={onShowOnboarding} />
              : <AboutContent />}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export type { SoftwareDialog };
