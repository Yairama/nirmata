import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useForm, useWatch } from "react-hook-form";
import { SoftwareDialogs } from "./software-dialogs.js";
import type { SoftwareDialog } from "./software-dialogs.js";
import type { DesktopActionRequest } from "./desktop-actions.js";
import { commandErrorCopy } from "./feedback.js";
import { useSession } from "./session-provider.js";
import type { AiProviderDiagnosticStatus, RecentProject, WorldSession } from "./types.js";
import { buttonStyles, cn, noticeStyles } from "./ui-styles.js";
import { openSession } from "./workspace.js";

type CreationPath = "manual" | "ai" | "import";

type CreationValues = {
  name: string;
  premise: string;
  epochLabel: string;
  genre: string;
  themes: string;
  tone: string;
  scale: "small" | "medium";
  restrictions: string;
};

const projectFilter = [{ name: "Proyecto Nirmata", extensions: ["nirmata"] }];
const recentProjectsKey = ["desktop", "recent-projects"] as const;
const providerStatusKey = ["desktop", "provider-status"] as const;

function errorMessage(value: unknown): string {
  return commandErrorCopy(value).detail;
}

function commandCode(value: unknown): string {
  if (typeof value === "object" && value !== null && "code" in value) {
    return String((value as { code: unknown }).code);
  }
  return "";
}

function pathTitle(path: CreationPath): string {
  if (path === "manual") return "Empezar manualmente";
  if (path === "ai") return "Crear una base del mundo con IA";
  return "Estructurar material existente";
}

function pathDescription(path: CreationPath): string {
  if (path === "manual") return "Crea un proyecto local vacío para construirlo paso a paso.";
  if (path === "ai") return "Define una base pequeña o mediana. La IA preparará una propuesta que revisarás antes de aplicarla.";
  return "Crea el proyecto y continúa en Importaciones con una copia inerte de tu material.";
}

function formatRecentDate(timestamp: number): string {
  return new Intl.DateTimeFormat("es", { dateStyle: "medium" }).format(new Date(timestamp));
}

export function ClosedView({ desktopAction, onStartProposal, onStartImport }: { desktopAction?: DesktopActionRequest | null; onStartProposal: (request: string) => void; onStartImport: () => void }) {
  const session = useSession();
  const queryClient = useQueryClient();
  const [creationPath, setCreationPath] = useState<CreationPath | null>(null);
  const [step, setStep] = useState(1);
  const [projectPath, setProjectPath] = useState("");
  const [status, setStatus] = useState("Crea o abre un mundo para comenzar.");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [missingRecent, setMissingRecent] = useState<RecentProject | null>(null);
  const [dialog, setDialog] = useState<SoftwareDialog>(null);
  const [dialogTrigger, setDialogTrigger] = useState<HTMLElement | null>(null);
  const manualButton = useRef<HTMLButtonElement>(null);
  const nameInput = useRef<HTMLInputElement | null>(null);
  const form = useForm<CreationValues>({
    defaultValues: {
      name: "",
      premise: "",
      epochLabel: "",
      genre: "",
      themes: "",
      tone: "",
      scale: "small",
      restrictions: "",
    },
    shouldFocusError: true,
  });
  const values = useWatch({ control: form.control });
  const recents = useQuery({
    queryKey: recentProjectsKey,
    queryFn: () => invoke<RecentProject[]>("list_recent_projects"),
    enabled: session === null,
    retry: false,
  });
  const provider = useQuery({
    queryKey: providerStatusKey,
    queryFn: () => invoke<AiProviderDiagnosticStatus>("get_ai_provider_status"),
    enabled: session === null,
    retry: false,
  });
  const removeRecent = useMutation({
    mutationFn: (path: string) => invoke<RecentProject[]>("remove_recent_project", { input: { path } }),
    onSuccess: (projects) => queryClient.setQueryData(recentProjectsKey, projects),
  });

  useEffect(() => {
    if (session === null) {
      setCreationPath(null);
      setStep(1);
      setProjectPath("");
      setStatus("Crea o abre un mundo para comenzar.");
      setError("");
      setBusy(false);
      setMissingRecent(null);
      form.reset();
    }
  }, [form, session]);

  useEffect(() => {
    if (creationPath && step === 1) nameInput.current?.focus();
    if (!creationPath) manualButton.current?.focus();
  }, [creationPath, step]);

  useEffect(() => {
    if (!desktopAction || session) return;
    if (desktopAction.action === "project.new") startCreation("manual");
    if (desktopAction.action === "project.open") void openWorld();
    if (desktopAction.action === "settings.open") setDialog("settings");
    if (desktopAction.action === "help.open") setDialog("help");
    if (desktopAction.action === "help.about") setDialog("about");
  }, [desktopAction?.id, session]);

  if (session) return null;

  function openSoftwareDialog(nextDialog: Exclude<SoftwareDialog, null>, trigger: HTMLElement) {
    setDialogTrigger(trigger);
    setDialog(nextDialog);
  }

  function startCreation(path: CreationPath) {
    setCreationPath(path);
    setStep(1);
    setProjectPath("");
    setError("");
    form.reset();
  }

  async function rememberProject(nextSession: WorldSession) {
    const projects = await invoke<RecentProject[]>("remember_recent_project", {
      input: {
        path: nextSession.path,
        name: nextSession.world.name,
        worldId: nextSession.world_id,
      },
    });
    queryClient.setQueryData(recentProjectsKey, projects);
  }

  async function activateSession(nextSession: WorldSession) {
    try {
      await rememberProject(nextSession);
    } catch {
      // A settings write must not prevent access to an already opened world.
    }
    openSession(nextSession);
  }

  async function chooseProjectPath() {
    setError("");
    try {
      const selected = await save({
        defaultPath: `${form.getValues("name").trim() || "mi-mundo"}.nirmata`,
        filters: projectFilter,
      });
      if (selected !== null) {
        setProjectPath(selected.toLowerCase().endsWith(".nirmata") ? selected : `${selected}.nirmata`);
      }
    } catch (value) {
      setError(errorMessage(value));
    }
  }

  async function advanceFromProject() {
    const valid = await form.trigger(["name"]);
    if (!valid) return;
    if (!projectPath) {
      setError("Elige dónde guardar el archivo .nirmata.");
      return;
    }
    setError("");
    setStep(creationPath === "manual" ? 3 : 2);
  }

  function nextStep() {
    setStep(3);
  }

  async function createWorld(data: CreationValues) {
    if (!creationPath || step !== 3 || !projectPath) return;
    setBusy(true);
    setError("");
    setStatus("Creando mundo…");
    try {
      const nextSession = await invoke<WorldSession>("create_world", {
        input: {
          path: projectPath,
          name: data.name,
          premise_md: data.premise,
          epoch_label: data.epochLabel,
        },
      });
      await activateSession(nextSession);
      if (creationPath === "ai") {
        const scale = data.scale === "small" ? "pequeña" : "mediana";
        const request = [
          `Prepara una base ${scale} y revisable para el mundo «${data.name.trim()}».`,
          data.premise.trim() ? `Premisa: ${data.premise.trim()}.` : null,
          data.genre.trim() ? `Género: ${data.genre.trim()}.` : null,
          data.themes.trim() ? `Temas: ${data.themes.trim()}.` : null,
          data.tone.trim() ? `Tono: ${data.tone.trim()}.` : null,
          data.restrictions.trim() ? `Restricciones: ${data.restrictions.trim()}.` : null,
          "Limita la propuesta a premisa, reglas fundamentales y pocos lugares, facciones o personajes iniciales.",
        ].filter(Boolean).join("\n");
        onStartProposal(request);
      } else if (creationPath === "import") {
        onStartImport();
      }
    } catch (value) {
      setError(errorMessage(value));
      setStatus("");
      setBusy(false);
    }
  }

  async function openWorld(path?: string) {
    setError("");
    try {
      const selected = path ?? await open({ multiple: false, directory: false, filters: projectFilter });
      if (selected === null) return;
      setBusy(true);
      setStatus("Abriendo mundo…");
      await activateSession(await invoke<WorldSession>("open_world", { path: selected }));
    } catch (value) {
      setBusy(false);
      setStatus("");
      if (path && commandCode(value) === "file_not_found") {
        setMissingRecent(recents.data?.find((project) => project.path === path) ?? null);
        setError("No encontramos ese archivo. Puedes localizarlo de nuevo o quitarlo de recientes.");
      } else {
        setError(errorMessage(value));
      }
    }
  }

  async function relocateRecent() {
    const previous = missingRecent;
    if (!previous) return;
    const selected = await open({ multiple: false, directory: false, filters: projectFilter });
    if (selected === null) return;
    try {
      setBusy(true);
      const nextSession = await invoke<WorldSession>("open_world", { path: selected });
      await removeRecent.mutateAsync(previous.path);
      await activateSession(nextSession);
    } catch (value) {
      setBusy(false);
      setError(errorMessage(value));
    }
  }

  const totalSteps = creationPath === "manual" ? 2 : 3;
  const visibleStep = step === 3 ? totalSteps : step;
  const nameRegistration = form.register("name", { required: "Escribe un nombre para el mundo." });
  return (
    <>
      <header className="home-header flex items-center justify-between gap-6 border-b border-line px-6 py-4 max-mobile:items-start max-mobile:px-4">
        <div className="grid gap-1">
          <p className="eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Nirmata</p>
          <h1>Tu mundo, guardado localmente</h1>
          <p>Construye canon, consulta consecuencias y revisa cada cambio antes de aplicarlo.</p>
        </div>
        <nav className="home-utilities flex flex-wrap justify-end gap-2 max-compact:gap-1" aria-label="Aplicación">
          <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={(event) => openSoftwareDialog("settings", event.currentTarget)}>Ajustes</button>
          <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={(event) => openSoftwareDialog("help", event.currentTarget)}>Ayuda</button>
          <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={(event) => openSoftwareDialog("about", event.currentTarget)}>Acerca de</button>
        </nav>
      </header>
      <section id="closed-view" className="grid min-h-[calc(100dvh-4rem)] grid-rows-[auto_1fr_auto] overflow-hidden rounded-3xl border border-line bg-surface shadow-sm max-mobile:min-h-dvh max-mobile:rounded-none max-mobile:border-0" aria-labelledby="create-title">
        {creationPath === null ? (
          <div className="home-grid grid min-h-0 grid-cols-[minmax(0,1.6fr)_minmax(19rem,0.65fr)] max-workspace:grid-cols-1 max-mobile:overflow-auto">
            <section className="creation-paths min-w-0 px-6 py-8 lg:px-10 max-mobile:px-4 max-mobile:py-6" aria-labelledby="create-title">
              <p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Inicio</p>
              <h2 id="create-title">Nuevo mundo</h2>
              <p className="mt-4 max-w-3xl text-lg text-muted">Elige un punto de partida. Ningún camino incorpora cambios al canon sin revisión.</p>
              <div className="creation-path-grid mt-8 grid grid-cols-3 gap-3 max-workspace:grid-cols-2 max-mobile:grid-cols-1">
                <button ref={manualButton} type="button" className="creation-path-card flex min-h-44 flex-col items-start justify-between gap-4 rounded-2xl border border-line bg-raised p-5 text-left text-ink shadow-sm enabled:hover:-translate-y-0.5 enabled:hover:border-accent enabled:hover:bg-accent-soft" onClick={() => startCreation("manual")}>
                  <strong className="font-serif text-xl">Empezar manualmente</strong><span className="font-normal leading-relaxed text-muted">Un proyecto vacío para construir paso a paso.</span>
                </button>
                <button type="button" className="creation-path-card flex min-h-44 flex-col items-start justify-between gap-4 rounded-2xl border border-line bg-raised p-5 text-left text-ink shadow-sm enabled:hover:-translate-y-0.5 enabled:hover:border-accent enabled:hover:bg-accent-soft" onClick={() => startCreation("ai")}>
                  <strong className="font-serif text-xl">Crear una base del mundo con IA</strong><span className="font-normal leading-relaxed text-muted">Una propuesta pequeña y editable que revisarás antes de aplicar.</span>
                </button>
                <button type="button" className="creation-path-card flex min-h-44 flex-col items-start justify-between gap-4 rounded-2xl border border-line bg-raised p-5 text-left text-ink shadow-sm enabled:hover:-translate-y-0.5 enabled:hover:border-accent enabled:hover:bg-accent-soft" onClick={() => startCreation("import")}>
                  <strong className="font-serif text-xl">Estructurar material existente</strong><span className="font-normal leading-relaxed text-muted">Una copia inerte de Markdown o texto para convertir en candidatos.</span>
                </button>
              </div>
              <button type="button" className="open-project-action mt-4" onClick={() => void openWorld()} disabled={busy}>Abrir otro mundo…</button>
            </section>
            <aside className="home-side min-w-0 border-l border-line bg-canvas p-6 max-workspace:border-l-0 max-workspace:border-t max-mobile:p-4" aria-label="Estado y proyectos recientes">
              <section className="provider-card grid gap-4 border-b border-line py-5">
                <p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">IA opcional</p>
                <h2>Microsoft Foundry</h2>
                <p>{provider.isPending ? "Comprobando configuración…" : provider.data?.message ?? "No se pudo comprobar la configuración."}</p>
                <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={(event) => openSoftwareDialog("settings", event.currentTarget)}>Configurar IA</button>
              </section>
              <section className="recent-projects grid gap-4 border-b border-line py-5" aria-labelledby="recents-heading">
                <h2 id="recents-heading">Recientes</h2>
                {recents.isPending && <p role="status">Cargando proyectos…</p>}
                {recents.isError && <p>No se pudo leer la lista. Aún puedes abrir un archivo.</p>}
                {recents.data?.length === 0 && <p className="muted text-muted">Los mundos que abras aparecerán aquí.</p>}
                <ul>
                  {recents.data?.map((project) => (
                    <li key={project.path}>
                      <button type="button" className="recent-project grid grid-cols-[1fr_auto] gap-2 border-b border-line py-3" aria-label={`Abrir ${project.name}`} disabled={busy} onClick={() => void openWorld(project.path)}>
                        <strong>{project.name}</strong>
                        <span>{formatRecentDate(project.lastOpenedMs)}</span>
                        <small className="path font-mono">{project.path}</small>
                      </button>
                      <button type="button" className={cn(buttonStyles({ variant: "ghost", size: "compact" }), "recent-remove")} aria-label={`Quitar ${project.name} de recientes`} onClick={() => removeRecent.mutate(project.path)}>Quitar</button>
                    </li>
                  ))}
                </ul>
              </section>
            </aside>
          </div>
        ) : (
          <form className="creation-wizard mt-6 grid gap-6" onSubmit={form.handleSubmit(createWorld)} noValidate>
            <div className="creation-step-heading flex items-start justify-between gap-4">
              <div>
                <p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Nuevo mundo · Paso {visibleStep} de {totalSteps}</p>
                <h2 id="create-title">{pathTitle(creationPath)}</h2>
                <p>{pathDescription(creationPath)}</p>
              </div>
              <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => { setCreationPath(null); setError(""); }}>Cancelar</button>
            </div>
            <ol className="wizard-progress flex gap-2" aria-label="Progreso de creación">
              <li aria-current={step === 1 ? "step" : undefined}>Proyecto</li>
              {creationPath !== "manual" && <li aria-current={step === 2 ? "step" : undefined}>{creationPath === "ai" ? "Intención" : "Importación"}</li>}
              <li aria-current={step === 3 ? "step" : undefined}>Revisar</li>
            </ol>
            {step === 1 && (
              <div className="wizard-fields grid max-w-3xl gap-4">
                <label>Nombre
                  <input {...nameRegistration} ref={(element) => { nameRegistration.ref(element); nameInput.current = element; }} maxLength={200} autoComplete="off" aria-invalid={Boolean(form.formState.errors.name)} />
                  {form.formState.errors.name && <span className="field-error text-sm text-danger" role="alert">{form.formState.errors.name.message}</span>}
                </label>
                <label>Premisa <textarea {...form.register("premise")} rows={4} maxLength={100000} /></label>
                <label>Origen del calendario <input {...form.register("epochLabel")} maxLength={200} /></label>
                <label>Archivo
                  <span className="path-row grid grid-cols-[1fr_auto] gap-2">
                    <input name="project-path" value={projectPath} readOnly placeholder="Selecciona dónde guardarlo" />
                    <button type="button" onClick={() => void chooseProjectPath()}>Elegir…</button>
                  </span>
                </label>
                <div className="wizard-actions flex flex-wrap items-center gap-2"><button type="button" onClick={() => void advanceFromProject()}>Continuar</button></div>
              </div>
            )}
            {step === 2 && creationPath === "ai" && (
              <fieldset className="creation-brief wizard-fields grid max-w-3xl gap-4">
                <legend>Resumen de intención</legend>
                <label>Género <input {...form.register("genre")} maxLength={120} /></label>
                <label>Temas <input {...form.register("themes")} maxLength={300} /></label>
                <label>Tono <input {...form.register("tone")} maxLength={120} /></label>
                <label>Escala
                  <select {...form.register("scale")}><option value="small">Pequeña</option><option value="medium">Mediana</option></select>
                </label>
                <label>Restricciones <textarea {...form.register("restrictions")} rows={3} /></label>
                <div className="wizard-actions flex flex-wrap items-center gap-2"><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setStep(1)}>Atrás</button><button type="button" onClick={nextStep}>Revisar creación</button></div>
              </fieldset>
            )}
            {step === 2 && creationPath === "import" && (
              <div className="wizard-fields grid max-w-3xl gap-4">
                <div className={noticeStyles({ tone: "info" })}>
                  <h3>El material original permanece intacto</h3>
                  <p>Después de crear el proyecto podrás seleccionar Markdown o texto. Nirmata lo copiará a un lote local, extraerá candidatos y te pedirá decidir qué revisar.</p>
                </div>
                <div className="wizard-actions flex flex-wrap items-center gap-2"><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setStep(1)}>Atrás</button><button type="button" onClick={nextStep}>Revisar creación</button></div>
              </div>
            )}
            {step === 3 && (
              <div className="creation-summary grid gap-3 rounded-2xl border border-line bg-raised p-5">
                <h3>Comprueba antes de crear</h3>
                <dl className="settings-facts grid gap-0 [&>div]:grid [&>div]:grid-cols-[minmax(7rem,0.4fr)_minmax(0,1fr)] [&>div]:gap-4 [&>div]:border-b [&>div]:border-line [&>div]:py-2 [&>div]:text-sm [&_dd]:min-w-0 [&_dd]:break-words [&_dt]:text-muted">
                  <div><dt>Mundo</dt><dd>{values.name}</dd></div>
                  <div><dt>Archivo</dt><dd className="path font-mono">{projectPath}</dd></div>
                  <div><dt>Inicio</dt><dd>{pathTitle(creationPath)}</dd></div>
                  {creationPath === "ai" && <div><dt>Alcance IA</dt><dd>Base {values.scale === "small" ? "pequeña" : "mediana"}, siempre revisable</dd></div>}
                </dl>
                <p className={noticeStyles({ tone: "info" })}>Se creará un único archivo local. {creationPath === "manual" ? "No se agregará contenido automáticamente." : "El siguiente paso preparará material para revisión; no lo aplicará al canon."}</p>
                <div className="wizard-actions flex flex-wrap items-center gap-2"><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setStep(creationPath === "manual" ? 1 : 2)}>Atrás</button><button type="submit" disabled={busy}>{busy ? "Creando…" : "Crear mundo"}</button></div>
              </div>
            )}
          </form>
        )}
      </section>
      {missingRecent && (
        <section className={cn(noticeStyles({ tone: "warning" }), "missing-recent flex items-start justify-between gap-4 max-mobile:flex-col")} role="alert">
          <div><strong>Archivo movido</strong><p>Localiza «{missingRecent.name}» en su nueva ubicación o quítalo de la lista.</p></div>
          <div className="dialog-actions flex flex-wrap items-center gap-2"><button type="button" onClick={() => void relocateRecent()}>Localizar…</button><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => { removeRecent.mutate(missingRecent.path); setMissingRecent(null); setError(""); }}>Quitar</button></div>
        </section>
      )}
      <p className="creation-status border-t border-line px-6 py-3 text-sm text-muted" role="status" aria-live="polite">{status}</p>
      {error && <p className="creation-error text-sm text-danger" role="alert">{error}</p>}
      <SoftwareDialogs active={dialog} onActiveChange={setDialog} returnFocus={dialogTrigger} onOpenBackups={() => undefined} onShowOnboarding={() => undefined} />
    </>
  );
}
