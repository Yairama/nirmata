import { useEffect, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { appActions } from "./state.js";
import { Icon } from "./icons.js";
import { buttonStyles, cn } from "./ui-styles.js";

type FeedbackAction = {
  label: string;
  run: () => void | Promise<void>;
};

type FeedbackItem = {
  id: number;
  presentation: "toast" | "banner";
  kind: "success" | "info" | "warning" | "error";
  title: string;
  detail: string;
  action?: FeedbackAction;
};

type ErrorCopy = Pick<FeedbackItem, "kind" | "title" | "detail">;

export type AiFailureObservation = {
  startedAtMs: number;
  phase: string;
  receivedCharacters: number;
};

const errorCopy: Record<string, ErrorCopy> = {
  app_busy: { kind: "warning", title: "Hay una solicitud en curso", detail: "Espera a que termine o cancélala antes de cambiar el mundo." },
  no_world_open: { kind: "warning", title: "La sesión no está disponible", detail: "Tu trabajo local se conserva. Comprueba la sesión antes de volver al inicio." },
  read_only_scope: { kind: "warning", title: "Esta versión es de solo lectura", detail: "Vuelve a la versión actual para preparar o aplicar cambios." },
  object_not_found: { kind: "warning", title: "El objeto ya no existe", detail: "Actualiza la vista o elige otro objeto. Los borradores locales se conservan." },
  manual_review_stale: { kind: "warning", title: "La propuesta está desactualizada", detail: "El mundo avanzó desde que se preparó. Vuelve a comprobarla antes de aplicarla." },
  manual_review_not_ready: { kind: "warning", title: "La propuesta todavía no está lista", detail: "Resuelve los bloqueos indicados en la revisión antes de aplicarla." },
  manual_review_revalidation_failed: { kind: "warning", title: "No se pudo actualizar la propuesta", detail: "La propuesta se conserva para que puedas corregirla o volver a intentarlo." },
  validation_error: { kind: "warning", title: "Hay datos que requieren atención", detail: "Revisa los campos y decisiones señalados. Ningún cambio se aplicó." },
  constraint_error: { kind: "error", title: "El cambio fue rechazado", detail: "El canon no cambió y la propuesta sigue disponible para corregirla." },
  storage_error: { kind: "error", title: "No se pudo completar la transacción", detail: "El canon no cambió. Tu propuesta se conserva para volver a intentarlo." },
  project_locked: { kind: "warning", title: "El archivo está en uso", detail: "Cierra la otra aplicación o proceso que lo usa y vuelve a intentarlo." },
  file_not_found: { kind: "warning", title: "El archivo fue movido o eliminado", detail: "Localiza el proyecto en su nueva ubicación o elige otro archivo." },
  file_error: { kind: "error", title: "No se pudo acceder al archivo", detail: "Comprueba la ubicación y los permisos antes de volver a intentarlo." },
  invalid_project_path: { kind: "warning", title: "La ruta no es válida", detail: "Elige un archivo local con extensión .nirmata." },
  invalid_project_format: { kind: "error", title: "El archivo no es un proyecto Nirmata", detail: "Elige otro archivo .nirmata o restaura una copia válida." },
  incompatible_schema: { kind: "error", title: "El proyecto usa otra versión", detail: "Esta versión de Nirmata no puede abrirlo de forma segura." },
  corrupt_project: { kind: "error", title: "El proyecto está dañado", detail: "No se realizaron cambios. Restaura un backup o elige otro archivo." },
  provider_key_missing: { kind: "warning", title: "Falta configurar la credencial de IA", detail: "Abre Ajustes de IA y guarda una credencial antes de continuar." },
  provider_config_missing: { kind: "warning", title: "La IA no está configurada", detail: "Completa la configuración de Microsoft Foundry en Ajustes." },
  invalid_provider_base_url: { kind: "warning", title: "La dirección del proveedor no es válida", detail: "Corrige la configuración de Microsoft Foundry y vuelve a probar." },
  provider_timeout: { kind: "warning", title: "La IA tardó demasiado", detail: "La solicitud terminó sin modificar el canon. Puedes volver a intentarlo." },
  provider_cancelled: { kind: "info", title: "Solicitud cancelada", detail: "El trabajo preparado se conserva y puedes retomarlo cuando quieras." },
  provider_transport_error: { kind: "error", title: "No se pudo conectar con la IA", detail: "Comprueba la conexión y vuelve a intentarlo." },
  provider_http_error: { kind: "error", title: "El proveedor rechazó la solicitud", detail: "Revisa la configuración o inténtalo de nuevo más tarde." },
  provider_response_error: { kind: "error", title: "La respuesta de IA no era válida", detail: "No se aplicó ningún cambio. Puedes volver a generar la respuesta." },
  invalid_ai_proposal: { kind: "warning", title: "La propuesta de IA no pasó la validación", detail: "Ningún cambio se aplicó. Revisa el diagnóstico y vuelve a generar la propuesta." },
  ai_context_stale: { kind: "warning", title: "El contexto quedó desactualizado", detail: "La versión cambió. Actualiza el contexto antes de continuar." },
  invalid_lore_import: { kind: "warning", title: "No se pudo usar este material", detail: "El lote se conserva. Reemplaza la fuente indicada o elige otros archivos." },
  lore_import_not_found: { kind: "warning", title: "El lote ya no está disponible", detail: "Actualiza la lista de lotes o crea una importación nueva." },
  invalid_snapshot_parent: { kind: "warning", title: "La carpeta de destino no es válida", detail: "Elige otra carpeta para guardar el backup." },
  invalid_snapshot_name: { kind: "warning", title: "El nombre del backup no es válido", detail: "Usa letras, números, guion o guion bajo, sin espacios y con un máximo de 80 caracteres." },
  snapshot_destination_occupied: { kind: "warning", title: "Ya existe un backup con ese nombre", detail: "Escribe otro nombre o elige otra carpeta." },
  snapshot_io_error: { kind: "error", title: "No se pudo guardar el backup", detail: "Comprueba el espacio, la carpeta y sus permisos antes de reintentar." },
  invalid_snapshot_import: { kind: "warning", title: "La copia no corresponde a esta vista", detail: "Elige un backup del mismo mundo y variante, preparado desde una versión compatible." },
  snapshot_has_no_changes: { kind: "info", title: "La copia no contiene cambios", detail: "El backup ya coincide con la versión actual; no se creó una propuesta." },
  invalid_object_uri: { kind: "warning", title: "La referencia no es válida", detail: "Selecciona nuevamente el objeto desde el explorador." },
  invalid_simulation_scenario: { kind: "warning", title: "El escenario necesita correcciones", detail: "Revisa facciones, recursos, existencias y reglas. El escenario permanece fuera del canon." },
  simulation_scenario_not_found: { kind: "warning", title: "El escenario ya no está disponible", detail: "Actualiza la lista o crea otro escenario para esta sesión." },
  invalid_simulation_promotion: { kind: "warning", title: "La selección no puede promoverse", detail: "Revisa las transiciones elegidas. Ningún cambio se aplicó al canon." },
  simulation_scenario_stale: { kind: "warning", title: "El escenario usa otra versión", detail: "Crea o actualiza el escenario desde la versión actual antes de preparar Cambios." },
  invalid_revision_id: { kind: "warning", title: "La versión no es válida", detail: "Actualiza el historial y selecciona otra versión." },
  undo_target_invalid: { kind: "warning", title: "No se puede deshacer esa versión", detail: "El historial indica cuál es el último cambio reversible." },
  undo_conflict: { kind: "warning", title: "Deshacer produciría un conflicto", detail: "Revisa los cambios posteriores antes de crear una versión inversa." },
};

let current: FeedbackItem | null = null;
let sequence = 0;
const listeners = new Set<() => void>();

function emit(item: Omit<FeedbackItem, "id"> | null) {
  current = item ? { ...item, id: ++sequence } : null;
  for (const listener of listeners) listener();
}

function commandCode(value: unknown): string | null {
  if (typeof value === "object" && value !== null && "code" in value) {
    return String((value as { code: unknown }).code);
  }
  return null;
}

function commandMessage(value: unknown): string | null {
  if (typeof value !== "object" || value === null || !("message" in value)) return null;
  const message = String((value as { message: unknown }).message).replace(/\s+/g, " ").trim();
  if (!message) return null;
  return message.length <= 1_000 ? message : `${message.slice(0, 997)}...`;
}

export function commandErrorCopy(value: unknown): ErrorCopy {
  const code = commandCode(value);
  if (!code || !errorCopy[code]) {
    return { kind: "error", title: "No se pudo completar la acción", detail: "Tu trabajo se conserva. Vuelve a intentarlo o elige otra acción." };
  }
  const copy = errorCopy[code];
  const message = commandMessage(value);
  return message
    ? { ...copy, detail: `${copy.detail} Detalle del sistema: ${message}` }
    : copy;
}

export function showCommandError(value: unknown, action?: FeedbackAction) {
  emit({ presentation: "banner", ...commandErrorCopy(value), action });
}

function aiPhaseLabel(phase: string): string {
  const labels: Record<string, string> = {
    preparing: "preparando el contexto",
    preparing_context: "preparando el contexto",
    extracting: "esperando la extracción del modelo",
    calling_model: "esperando la respuesta del modelo",
    streaming_delta: "recibiendo la respuesta",
    parsing_response: "interpretando la respuesta recibida",
    validating: "validando el resultado",
    calling_critic: "esperando la revisión del crítico",
    repairing: "esperando una reparación del modelo",
    synthesizing: "esperando la síntesis",
    validating_synthesis: "validando la síntesis",
    handing_to_standard_review: "preparando la revisión estándar",
    checking_connection: "comprobando la conexión",
  };
  return labels[phase] ?? "esperando al proveedor";
}

export function aiResponseErrorDetail(value: unknown, observation: AiFailureObservation): string {
  const message = commandMessage(value);
  const diagnostic = message ? ` Diagnóstico: ${message}` : "";
  return `Etapa: ${aiPhaseLabel(observation.phase)}.${diagnostic} Ningún cambio se aplicó al canon.`;
}

export function aiTimeoutDetail(value: unknown, observation: AiFailureObservation): string {
  const elapsedSeconds = Math.max(1, Math.round((Date.now() - observation.startedAtMs) / 1_000));
  const activity = observation.receivedCharacters > 0
    ? `${observation.receivedCharacters.toLocaleString("es")} caracteres recibidos antes de detenerse`
    : "ningún contenido recibido";
  const uncertainty = observation.receivedCharacters > 0
    ? "La respuesta comenzó, pero quedó incompleta."
    : "Sin contenido no se puede distinguir entre razonamiento interno y un proveedor bloqueado.";
  const message = commandMessage(value);
  const diagnostic = message ? ` Detalle del sistema: ${message}` : "";
  return `Actividad observable: ${elapsedSeconds} s ${aiPhaseLabel(observation.phase)}; ${activity}. ${uncertainty} El canon no cambió. Puedes volver a intentarlo.${diagnostic}`;
}

export function showAiCommandError(
  value: unknown,
  observation: AiFailureObservation,
  action?: FeedbackAction,
) {
  const code = commandCode(value);
  if (code === "provider_response_error" || code === "invalid_ai_proposal") {
    const fallback = commandErrorCopy(value);
    emit({
      presentation: "banner",
      ...fallback,
      title: observation.phase === "calling_critic"
        ? "La revisión de IA no era válida"
        : fallback.title,
      detail: aiResponseErrorDetail(value, observation),
      action,
    });
    return;
  }
  if (code !== "provider_timeout") {
    showCommandError(value, action);
    return;
  }
  emit({
    presentation: "banner",
    kind: "warning",
    title: "La IA tardó demasiado",
    detail: aiTimeoutDetail(value, observation),
    action,
  });
}

export function showSuccess(title: string, detail: string) {
  appActions.setStatus(`${title}. ${detail}`);
}

export function clearFeedback() {
  emit(null);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function snapshot() {
  return current;
}

export function FeedbackHost() {
  const item = useSyncExternalStore(subscribe, snapshot, snapshot);
  useEffect(() => {
    if (!item || item.presentation !== "toast") return;
    const timeout = window.setTimeout(() => {
      if (current?.id === item.id) clearFeedback();
    }, 4_000);
    return () => window.clearTimeout(timeout);
  }, [item]);

  if (!item) return null;
  return createPortal(
    <aside
      className={cn(
        "global-feedback pointer-events-auto fixed right-4 top-20 z-[120] grid w-[min(27rem,calc(100vw-2rem))] grid-cols-[1fr_auto] gap-4 rounded-2xl border border-line bg-raised p-4 text-sm shadow-overlay max-mobile:bottom-3 max-mobile:left-3 max-mobile:right-3 max-mobile:top-auto max-mobile:w-auto",
        item.presentation,
        item.kind === "warning" && "warning border-warning",
        item.kind === "error" && "error border-danger",
        item.kind === "info" && "info border-info",
        item.kind === "success" && "success border-success",
      )}
      role={item.presentation === "toast" ? "status" : "alert"}
      aria-live={item.presentation === "toast" ? "polite" : "assertive"}
    >
      <div className="grid gap-1">
        <strong>{item.title}</strong>
        <p>{item.detail}</p>
      </div>
      <div className="global-feedback-actions flex items-start gap-2">
        {item.action && (
          <button
            type="button"
            className={buttonStyles()}
            onClick={() => {
              const result = item.action?.run();
              if (result instanceof Promise) void result.catch((error) => showCommandError(error, item.action));
            }}
          >
            {item.action.label}
          </button>
        )}
        <button type="button" className={buttonStyles({ variant: "icon" })} onClick={clearFeedback} aria-label="Cerrar aviso" title="Cerrar aviso"><Icon name="x" /></button>
      </div>
    </aside>,
    document.body,
  );
}
