import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import * as Tabs from "@radix-ui/react-tabs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { applyAppearanceTheme, readAppearanceTheme } from "./appearance.js";
import type { AppearanceTheme } from "./appearance.js";
import { useSession } from "./session-provider.js";
import type { AiProviderDiagnosticStatus } from "./types.js";

type SoftwareDialog = "settings" | "help" | "about" | null;

const providerStatusKey = ["desktop", "provider-status"] as const;

function providerSource(status: AiProviderDiagnosticStatus): string {
  switch (status.credential.source) {
    case "system_secure_store": return "Almacén seguro del sistema";
    case "session_environment": return "Entorno de esta sesión";
    case "session_memory": return "Memoria de esta sesión";
    default: return "Sin credencial";
  }
}

function SettingsContent() {
  const session = useSession();
  const queryClient = useQueryClient();
  const [theme, setTheme] = useState<AppearanceTheme>(readAppearanceTheme);
  const [credentialError, setCredentialError] = useState("");
  const provider = useQuery({
    queryKey: providerStatusKey,
    queryFn: () => invoke<AiProviderDiagnosticStatus>("get_ai_provider_status"),
    retry: false,
  });
  const diagnose = useMutation({
    mutationFn: () => invoke<AiProviderDiagnosticStatus>("diagnose_ai_provider", {
      input: { requestId: crypto.randomUUID() },
    }),
    onSuccess: (status) => queryClient.setQueryData(providerStatusKey, status),
  });
  const saveCredential = useMutation({
    mutationFn: (apiKey: string) => invoke("set_provider_api_key", { apiKey }),
    onSuccess: async () => {
      setCredentialError("");
      await queryClient.invalidateQueries({ queryKey: providerStatusKey });
    },
  });
  const clearCredential = useMutation({
    mutationFn: () => invoke("clear_provider_api_key"),
    onSuccess: async () => {
      setCredentialError("");
      await queryClient.invalidateQueries({ queryKey: providerStatusKey });
    },
  });

  useEffect(() => applyAppearanceTheme(theme), [theme]);

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

  const providerError = provider.error || diagnose.error || saveCredential.error || clearCredential.error;
  return (
    <Tabs.Root className="settings-tabs" defaultValue="general" orientation="vertical">
      <Tabs.List className="settings-tab-list" aria-label="Secciones de Settings">
        <Tabs.Trigger value="general">General</Tabs.Trigger>
        <Tabs.Trigger value="appearance">Apariencia</Tabs.Trigger>
        <Tabs.Trigger value="ai">IA</Tabs.Trigger>
        <Tabs.Trigger value="project">Proyecto</Tabs.Trigger>
        <Tabs.Trigger value="accessibility">Accesibilidad</Tabs.Trigger>
        <Tabs.Trigger value="advanced">Avanzado</Tabs.Trigger>
      </Tabs.List>
      <div className="settings-tab-content">
        <Tabs.Content value="general">
          <h3>General</h3>
          <p>Nirmata guarda mundos como archivos locales <code>.nirmata</code>. No hay sincronización, telemetría ni guardado en nube.</p>
          <p className="muted">No hay preferencias generales configurables en esta versión.</p>
        </Tabs.Content>
        <Tabs.Content value="appearance">
          <h3>Apariencia</h3>
          <label>Tema
            <select value={theme} onChange={(event) => setTheme(event.target.value as AppearanceTheme)}>
              <option value="system">Usar el del sistema</option>
              <option value="light">Claro</option>
              <option value="dark">Oscuro</option>
              <option value="high-contrast">Alto contraste</option>
            </select>
          </label>
          <p className="muted">La preferencia se guarda únicamente en este equipo.</p>
        </Tabs.Content>
        <Tabs.Content value="ai">
          <h3>Microsoft Foundry</h3>
          {provider.isPending && <p role="status">Comprobando configuración…</p>}
          {provider.data && (
            <div className="settings-section-stack">
              <p className={`status-chip ${provider.data.connected ? "success" : "info"}`}>
                {provider.data.message}
              </p>
              <dl className="settings-facts">
                <div><dt>Credencial</dt><dd>{providerSource(provider.data)}</dd></div>
                <div><dt>Persistencia</dt><dd>{provider.data.credential.persistence === "system_secure_store" ? "Persistente" : "Solo esta sesión"}</dd></div>
              </dl>
              <button type="button" className="secondary" disabled={!provider.data.canCheckConnection || diagnose.isPending} onClick={() => diagnose.mutate()}>
                {diagnose.isPending ? "Probando conexión…" : "Probar conexión"}
              </button>
            </div>
          )}
          <form className="settings-credential-form" onSubmit={submitCredential}>
            <label>Reemplazar credencial
              <input name="api-key" type="password" autoComplete="off" />
            </label>
            <div className="dialog-actions">
              <button type="submit" disabled={saveCredential.isPending}>Guardar</button>
              <button type="button" className="ghost" disabled={clearCredential.isPending} onClick={() => clearCredential.mutate()}>Borrar credencial</button>
            </div>
          </form>
          {credentialError && <p role="alert" className="creation-error">{credentialError}</p>}
          {providerError && <p role="alert" className="creation-error">No se pudo actualizar la configuración de IA.</p>}
          <p className="muted">La credencial nunca se devuelve a la interfaz. Nirmata envía únicamente el contexto elegido y solicita al proveedor no almacenar la respuesta.</p>
        </Tabs.Content>
        <Tabs.Content value="project">
          <h3>Proyecto</h3>
          {session ? (
            <dl className="settings-facts">
              <div><dt>Nombre</dt><dd>{session.world.name}</dd></div>
              <div><dt>Archivo</dt><dd className="path">{session.path}</dd></div>
              <div><dt>Escribiendo en</dt><dd>{session.active_variant.name}</dd></div>
              <div><dt>Vista</dt><dd>{session.read_only ? "Versión anterior, solo lectura" : "Versión actual"}</dd></div>
            </dl>
          ) : <p>Abre un mundo para ver sus datos de proyecto.</p>}
        </Tabs.Content>
        <Tabs.Content value="accessibility">
          <h3>Accesibilidad</h3>
          <p>Nirmata respeta el movimiento reducido y el contraste forzado del sistema. El tema de alto contraste puede activarse en Apariencia.</p>
        </Tabs.Content>
        <Tabs.Content value="advanced">
          <h3>Avanzado</h3>
          {session ? (
            <dl className="settings-facts technical-facts">
              <div><dt>Mundo</dt><dd>{session.world_id}</dd></div>
              <div><dt>Revisión</dt><dd>{session.current_revision}</dd></div>
              <div><dt>Variante</dt><dd>{session.active_variant.id}</dd></div>
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
    <div className="about-content">
      <p className="eyebrow">Nirmata</p>
      <h3>Software editorial para mundos coherentes</h3>
      <dl className="settings-facts">
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

function HelpContent({ onAbout }: { onAbout: () => void }) {
  return (
    <div className="help-content">
      <nav className="help-index" aria-label="Temas de ayuda">
        <a href="#help-create">Crear</a>
        <a href="#help-ai">Preguntar y proponer</a>
        <a href="#help-versions">Versiones</a>
        <a href="#help-import">Importar</a>
        <a href="#help-shortcuts">Atajos</a>
        <a href="#help-glossary">Glosario</a>
      </nav>
      <section id="help-create">
        <h3>Crear un mundo</h3>
        <p><strong>Manual</strong> crea un proyecto vacío. <strong>Base con IA</strong> prepara contenido revisable. <strong>Material existente</strong> copia texto de forma inerte para extraer candidatos.</p>
      </section>
      <button type="button" className="secondary" onClick={() => window.dispatchEvent(new CustomEvent("nirmata:show-onboarding"))}>Volver a mostrar la guía del mundo</button>
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
        <p>Un archivo de lore es una fuente, no canon. Nirmata conserva procedencia, presenta candidatos y exige revisión. Un snapshot es una copia estructurada distinta de Markdown o texto.</p>
      </section>
      <section id="help-shortcuts">
        <h3>Atajos</h3>
        <dl className="settings-facts">
          <div><dt>Ctrl/Cmd + K</dt><dd>Abrir búsqueda y acciones</dd></div>
          <div><dt>Escape</dt><dd>Cerrar diálogo o palette y devolver el foco</dd></div>
          <div><dt>Tab / Shift + Tab</dt><dd>Recorrer controles</dd></div>
        </dl>
      </section>
      <section id="help-glossary">
        <h3>Glosario</h3>
        <dl className="settings-facts">
          <div><dt>Canon</dt><dd>Información aceptada como verdadera en esa versión del mundo.</dd></div>
          <div><dt>Propuesta</dt><dd>Conjunto de cambios todavía fuera del canon.</dd></div>
          <div><dt>Variante</dt><dd>Línea canónica nombrada que puede evolucionar de forma independiente.</dd></div>
          <div><dt>Fuente</dt><dd>Objeto o fragmento que justifica una consulta o cambio.</dd></div>
        </dl>
      </section>
      <button type="button" className="secondary" onClick={onAbout}>Acerca de Nirmata</button>
    </div>
  );
}

export function SoftwareDialogs({ active, onActiveChange, returnFocus }: {
  active: SoftwareDialog;
  onActiveChange: (dialog: SoftwareDialog) => void;
  returnFocus?: HTMLElement | null;
}) {
  const title = active === "settings" ? "Settings" : active === "help" ? "Centro de ayuda" : "Acerca de Nirmata";
  return (
    <Dialog.Root open={active !== null} onOpenChange={(open) => !open && onActiveChange(null)}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="software-dialog"
          aria-describedby="software-dialog-description"
          onCloseAutoFocus={(event) => {
            if (!returnFocus) return;
            event.preventDefault();
            returnFocus.focus();
          }}
        >
          <div className="dialog-heading">
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
            <Dialog.Close asChild><button type="button" className="ghost" aria-label={`Cerrar ${title}`}>Cerrar</button></Dialog.Close>
          </div>
          {active === "settings"
            ? <SettingsContent />
            : active === "help"
              ? <HelpContent onAbout={() => onActiveChange("about")} />
              : <AboutContent />}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export type { SoftwareDialog };
