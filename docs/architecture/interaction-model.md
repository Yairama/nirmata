# Modelo de interaccion con IA

**Estado:** recomendacion consolidada.

## Decision

La GUI debe tener **un panel de asistente con dos modos explicitos**, no dos
chatbots distintos:

1. `Consultar`: solo lectura.
2. `Proponer cambio`: produce un `ChangeSet`.

Ambos comparten historial visible y seleccion actual, pero tienen contratos de
salida diferentes.

Las solicitudes complejas pueden activar `Revision profunda`, que ejecuta
especialistas de solo lectura antes de construir la propuesta. No es otro
chatbot ni otro lugar donde editar.

## Por que el modo debe ser explicito

Clasificar automaticamente la intencion puede convertir una pregunta ambigua
en una escritura. El usuario debe saber antes de enviar si esta consultando o
preparando una modificacion.

`Consultar` es el modo predeterminado.

Si el usuario escribe "haz que la ciudad sea independiente" en modo consulta,
Nirmata responde con el impacto posible y ofrece una accion para convertir la
solicitud en propuesta. No cambia de modo silenciosamente.

## Superficies de la GUI

```text
+----------------+---------------------------+----------------------+
| Navegacion     | Editor                    | Asistente            |
| entidades      | entidad/documento/evento  | Consultar | Proponer |
| busqueda       | timeline                  | contexto y respuesta |
+----------------+---------------------------+----------------------+
| Cambios pendientes: resumen, diff, validacion, aceptar/rechazar   |
+-------------------------------------------------------------------+
```

El area de cambios pendientes es distinta del transcript conversacional. Una
respuesta no debe perderse entre mensajes ni aplicarse desde un boton ambiguo.
La cola se recupera desde SQLite al abrir el mundo o cambiar de variante. Una
tarjeta conserva el mismo origen, editor y acciones tras reiniciar; stale sigue
visible pero nunca se vuelve aplicable por el mero hecho de reabrir.

`PendingReviews` y `ReviewDrawer` son el único owner visual React de esa cola.
TanStack Query llama `list_pending_reviews` y `read_manual_review` con claves de
mundo, variante y revisión; los productores solo persisten mediante su caso de
uso e invalidan esa consulta. No existe un mapa frontend paralelo ni un host DOM
imperativo. El asistente invalida la query después de que Rust ya persistió la
revisión y nunca recibe ni transporta capacidad de commit.

## Modo consultar

Debe responder con:

- respuesta directa;
- objetos utilizados;
- citas navegables;
- separacion entre hecho, inferencia y ausencia de evidencia;
- opcion de abrir cada fuente;
- opcion de iniciar una propuesta basada en la respuesta.

Ejemplo:

```text
Pregunta: Que facciones se beneficiarian del colapso de la mina?

Respuesta:
- Liga de Contrabandistas: inferencia basada en ...
- Reino del Sur: hecho, ya controla una ruta alternativa ...

Fuentes:
- nirmata://entity/...
- nirmata://event/...
```

## Modo proponer cambio

Si la solicitud es ambigua o amplia, la aplicacion presenta antes un
`IntentBrief` editable con objetivo, alcance, entidades y restricciones.
Solicitudes pequenas no agregan este paso.

El modo Proponer ofrece exactamente seis plantillas reutilizables: faccion,
ciudad, personaje, conflicto, cronologia y consecuencias. No existe una
plantilla de novela. Preparar una plantilla es determinista y local: crea un
run `IntentBriefReady`, captura la revision y el contexto y no requiere
credencial. Si no hay seleccion, el mundo es su ancla y fuente explicita.

La escala pequena admite como maximo tres operaciones y la mediana seis. Rust
aplica el limite tanto a la primera salida como a una reparacion, exige fuentes
no vacias y rechaza cualquier fuente fuera del contexto capturado. Continuar el
brief reutiliza el mismo run, revision y contexto; no consulta la seleccion
actual de la interfaz ni crea una ejecucion huerfana.

Debe producir:

- objetivo interpretado;
- objetos afectados;
- operaciones antes/despues;
- nuevas entidades o relaciones;
- consecuencias y supuestos;
- advertencias;
- fuentes utilizadas;
- validacion.
- reporte del critico semantico.
- decisiones pendientes entre alternativas.

La prosa explicativa acompana al `ChangeSet`; no lo reemplaza.

## Revision

Cada operacion puede seleccionarse o descartarse. Antes de confirmar:

1. Las operaciones elegidas forman un nuevo conjunto.
2. El conjunto completo vuelve a validarse.
3. Se muestran dependencias rotas.
4. Se muestran conflictos semanticos y excepciones solicitadas.
5. Se resuelven los `DecisionPoint`s pendientes.
6. Solo un conjunto valido puede confirmarse.

La vista muestra inicialmente dos o tres alternativas o `DecisionPoint`s. Los
hallazgos aparecen resumidos por severidad; evidencia y citas se expanden sin
ocultar errores duros.

Para reemplazos de canon, conflictos duros y cambios de impacto amplio, el
usuario registra primero su lectura. La resolucion sugerida por la IA aparece
despues. Esta friccion selectiva reduce aceptacion automatica sin castigar
ediciones rutinarias.

Si cambia la revision del mundo, la propuesta se marca obsoleta y se deshabilita
la confirmacion hasta ejecutar nuevamente validadores y critico semantico.

La confirmacion aplica una unica transaccion.

Nirmata registra localmente aceptacion, edicion, rechazo y tiempo de decision
por operacion. No envia estas metricas sin consentimiento.

La revision profunda no es un nivel superior de autoridad. Solo aumenta
contexto, evidencia y especialistas.

## Acciones contextuales

La conversacion no debe ser la unica entrada. Acciones directas reducen
ambiguedad:

- "Explicar impacto" sobre un evento.
- "Buscar contradicciones" sobre una entidad.
- "Proponer consecuencias" sobre una crisis.
- "Crear documento desde este punto de vista".
- "Comparar versiones" sobre un `Claim`.

Todas usan el mismo workflow interno.

## Historial

El historial conversacional ayuda al usuario, pero no es canon.

- Los mensajes pueden citar objetos del mundo.
- Una afirmacion del asistente no se vuelve verdadera por estar en el chat.
- Una propuesta rechazada permanece como historial, no como estado.
- Las conversaciones pueden eliminarse sin afectar el mundo.

La barra editorial muestra por separado la variante activa de escritura y la
revision observada. Una revision historica se rotula `Solo lectura`, deshabilita
edicion y permite volver explicitamente a la cabeza. Busqueda, URI, contexto,
timeline, VFS y exportacion muestran el mismo scope observado.

Comparar dos scopes lista diferencias por ID con acciones para abrir cada lado.
Preparar merge siempre toma otra variante como fuente y la cabeza activa como
destino. Operaciones independientes y `DecisionPoint`s aparecen en el panel de
cambios normal; no existe un boton que aplique directamente la comparacion.

El area Versiones es el unico owner visual del linaje y del historial editorial.
La cola global Cambios contiene propuestas pendientes, incluido un merge
preparado, pero no duplica versiones historicas. Deshacer solo actua sobre el
ultimo cambio logico de la variante activa y crea una nueva version inversa.

## Edicion manual estructurada

El area Mundo monta un unico `StructuredEditor` React para World, Entity,
Relation, Event, Claim, Rule, Goal y Document. React Hook Form conserva dirty,
reset y errores por campo; las listas compuestas usan field arrays y el
`ObjectPicker` resuelve referencias por nombre. UUID, URI, JSON y unidades
temporales permanecen disponibles solo en detalles tecnicos.

El formulario no valida el dominio por segunda vez ni escribe canon. Convierte
controles humanos al `ManualDraftRequest` existente y `preview_manual_draft`
sigue siendo la autoridad para construir y validar la propuesta. Crear, editar y
`begin_manual_review_edit`/`apply_manual_review_edit` recorren el mismo editor;
el resultado vuelve siempre a Cambios para revision estandar.

## Streaming

El streaming es adecuado para respuestas de consulta. En edicion, puede mostrar
progreso, pero la GUI solo habilita revision cuando el objeto estructurado ha
sido recibido y validado completamente.
