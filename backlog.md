# Backlog funcional end-to-end de Nirmata

## Propósito

Este documento es el plan ejecutable y autosuficiente para construir la
solución funcional general de Nirmata. Consolida producto, dominio,
arquitectura, persistencia, interacción, IA, validación y pruebas para que la
implementación pueda continuar aunque se pierda el resto de la documentación.

El criterio rector es YAGNI: construir la solución más pequeña que permita a un
autor mantener un mundo ficticio coherente, consultarlo y aceptar cambios
asistidos por IA sin ceder al modelo autoridad de escritura. Las tareas
NIR-001–NIR-052 forman el fundamento funcional; son un hito utilizable, no el
límite del producto. Las fases posteriores añaden únicamente capacidades
documentadas y acotadas, reutilizando el mismo canon, revisión y transacción.

## Estado actual

- NIR-001–NIR-046 están completadas (46 de 89; 51,7 % del backlog general y
  88,5 % del fundamento funcional).
- El workspace Rust incluye `nirmata-core`, `nirmata-store`, `nirmata-ai`,
  `nirmata-app` y la aplicación Tauri.
- El producto manual permite crear, editar, buscar, revisar, confirmar, auditar
  y deshacer cambios del canon en un archivo `.nirmata`.
- La recuperación determinista combina anclas, SQLite, relaciones, tiempo,
  perspectivas y FTS5 con fuentes navegables.
- La frontera de IA incluye credenciales seguras, streaming, contratos
  estrictos, modo `Consultar` y generación validada de propuestas.
- La siguiente tarea funcional es NIR-047, la integración del run de IA con
  revisión, stale check y commit.
- La aceptación de NIR-052 certifica el fundamento funcional. La solución
  general queda completa únicamente al satisfacer la Definition of Done final.

## Bloqueos

- No hay bloqueos activos. El 5 de agosto de 2026 se verificaron solicitudes
  reales normal y streaming con `gpt-5.6-sol` mediante
  `POST /openai/v1/responses`, autenticación Bearer y `store: false`. Este
  deployment rechaza `temperature`, por lo que el cliente no envía ese
  parámetro.

## Resumen autónomo del producto y la arquitectura

Nirmata es una aplicación de escritorio local-first y de un solo usuario para
construir storyworlds. El canon vive en un único archivo SQLite con extensión
`.nirmata`. Markdown se usa únicamente para prosa almacenada en columnas
`TEXT`; el árbol parecido a archivos es una proyección lógica de SQLite y no una
segunda fuente de verdad.

La arquitectura final es un monolito modular en un workspace Rust 2024:

| Módulo | Responsabilidad y límites |
|---|---|
| `crates/nirmata-core` | Tipos de dominio, invariantes y validadores deterministas puros. No depende de GUI, SQLite, HTTP, proveedor de IA ni runtime asíncrono. |
| `crates/nirmata-store` | Apertura del archivo, migraciones, SQLite, FTS5, consultas, transacciones, auditoría y undo. No expone SQL a la GUI. |
| `crates/nirmata-ai` | Cliente HTTP concreto para un proveedor inicial, prompts, streaming y parsing estructurado. Solo lee contexto y devuelve respuestas o drafts; nunca recibe capacidad de commit. Una segunda implementación local o remota solo aparece tras una necesidad medida y reemplaza directamente el acoplamiento concreto. |
| `crates/nirmata-app` | Casos de uso, construcción de contexto, estados del workflow, revisión, revalidación y coordinación entre core, store e IA. |
| `apps/nirmata-desktop` | Tauri 2, comandos de transporte y frontend web pequeño. Presenta estado; no contiene SQL, prompts ni reglas del canon. |

La unidad de escritura es `ChangeSet`: contiene `world_id`, `base_revision`,
operaciones tipadas, versiones esperadas, fuentes y decisiones. Un draft pasa
por esquema, integridad estructural, tiempo/ciclo de vida, reglas codificadas,
subgrafo afectado, crítico semántico independiente, revisión humana,
revalidación contra la revisión vigente y una transacción SQLite atómica. Existe
como máximo un intento de reparación. Una consulta nunca produce operaciones.

El dominio mínimo incluye `World`, `Entity`, `Relation`, `Event`, `Goal`,
`Rule`, `Claim`, `Document`, `ChangeSet` y `DecisionPoint`. Los IDs son estables
y no dependen de nombres o rutas. Cada fila editable tiene `version`; cada
commit crea inicialmente una revisión lineal con un solo padre. Hay auditoría y
undo. Después de estabilizar esta base se incorporan variantes con cabezas
explícitas, lectura histórica y merge seguro limitado; no se usa event sourcing
completo, CRDT ni colaboración concurrente.

Los claims separan:

- `authentication`: `canonical`, `attributed`, `disputed`;
- `modality`: `assertion`, `belief`, `hypothesis`, `counterfactual`;
- `holder`, `register`, `polarity`, fuente y periodo de validez;
- forma normalizada opcional (`predicate_key` y objeto entidad o escalar);
- procedencia mediante documento/claim/revisión.

La ausencia significa desconocido. `NULL` nunca significa falsedad. Claims
opuestos pueden coexistir si pertenecen a holders, modalidades o registros
distintos; dos claims canónicos normalizados, activos y opuestos en el mismo
contexto y periodo bloquean el estado final.

El tiempo narrativo usa ticks `i64` relativos al epoch del mundo. `EventTime`
tiene `kind` (`unknown`, `instant`, `interval`, `ongoing`), inicio/fin
opcionales, precisión y certeza. Las relaciones de intervalos de Allen se
calculan en Rust y no se persisten. El orden de discurso se deriva del
`ordinal` de `content_reference`; un flashback no cambia el tiempo del evento.

La recuperación comienza por selección explícita, continúa con relaciones SQL y
CTE, ventana temporal, goals y perspectivas, y termina con FTS5. Esa tubería ya
es RAG determinista. El resultado conserva procedencia y distingue `hecho`,
`perspectiva`, `inferencia`, `sin_evidencia` y `no_especificado`. Solo si el
benchmark demuestra huecos de paráfrasis o vocabulario se añade recuperación
semántica como índice derivado dentro del mismo proyecto SQLite y se combina con
ranking híbrido citado. Una base vectorial o de grafos separada no es parte de
la arquitectura predeterminada.

La GUI ofrece modos explícitos `Consultar` y `Proponer`, además de navegación,
editor, contexto y cambios pendientes. La revisión permite aceptar, editar o
rechazar cada operación. Los reemplazos y conflictos de alto riesgo exigen que
el usuario registre su juicio antes de revelar la resolución sugerida por IA.

Las capacidades posteriores conservan las mismas fronteras:

- `Revisión profunda` es un modo explícito, acotado y de solo lectura para
  especialistas seleccionados por relevancia; un sintetizador produce
  `DecisionPoint`s y un `ChangeSetDraft`, nunca un commit.
- La importación de lore trata Markdown, texto y documentos como material no
  confiable, conserva fragmentos y procedencia, y usa extracción graph-aware
  únicamente para proponer entidades, relaciones, eventos, claims y reglas.
- Las variantes mantienen cabezas nombradas y conflictos manuales; el merge
  automático se limita a operaciones no solapadas o conmutativas.
- El tick sigue siendo el tiempo canónico. Un calendario fijo por mundo es solo
  una capa de conversión y visualización.
- La simulación se limita a facciones y recursos, corre fuera del canon y solo
  entra mediante un `ChangeSet` revisado.
- La extracción narrativa deriva timelines, hilos causales, cabos sueltos y
  documentos desde el canon; no promete generar novelas.
- El VFS lógico se exporta e importa mediante snapshots explícitos para backup o
  edición externa, nunca mediante sincronización bidireccional viva.

## Alcance

### Fundamento funcional — NIR-001–NIR-052

- Crear, guardar, cerrar y reabrir un mundo local.
- Editar premisa, reglas, entidades, relaciones, goals, eventos, claims y
  documentos.
- Mantener referencias de contenido, causalidad, participantes, perspectivas,
  tiempo incierto e intervalos.
- Buscar mediante SQL y FTS5; navegar fuentes por URI lógico estable.
- Aplicar ediciones manuales y asistidas como `ChangeSet`s atómicos.
- Clasificar retcons como `additive`, `reinterpretive` o `replacement`.
- Revisar operaciones individualmente, resolver decisiones y reconocer
  warnings o excepciones intencionales.
- Revisiones lineales, auditoría local y undo.
- Modos explícitos de consulta y propuesta con contexto visible.
- Pipeline estándar de IA: generador → validadores Rust → crítico semántico
  independiente → humano → transacción, con un solo intento de reparación.
- Pruebas automatizadas de comportamiento no trivial en dominio,
  almacenamiento, recuperación y workflow.

### Capacidades de la solución general — NIR-053–NIR-089

- Evolución de RAG mediante benchmark, embeddings condicionales en SQLite,
  ranking híbrido citado, invalidación y reconstrucción de índices.
- Revisión multiagente profunda bajo solicitud explícita, con especialistas de
  solo lectura, presupuestos estrictos, informes tipados, desacuerdos visibles y
  síntesis revisable.
- Importación de lore existente como propuestas con fragmentos, procedencia y
  extracción graph-aware limitada a la ingestión.
- Variantes nombradas, cabezas explícitas, comparación, lectura histórica y
  merge seguro limitado.
- Calendario ficticio fijo y deliberadamente pequeño como capa de presentación.
- Simulación determinista y acotada de facciones y recursos en escenarios fuera
  del canon.
- Extracción narrativa y generación de documentos internos dependientes de
  perspectiva, siempre revisables.
- Segundo proveedor o proveedor local solo ante una necesidad funcional medida,
  mediante refactor directo y sin capa de compatibilidad.
- Exportación e importación del VFS lógico como snapshots explícitos.

### No objetivos explícitos

- CI/CD, automatización de empaquetado o release, gobierno, proyectos genéricos
  de observabilidad o calidad de código sin comportamiento de producto.
- Estudios con usuarios, telemetría remota o personalización automática.
- Colaboración multiusuario, CRDT, servidor de sincronización o edición
  concurrente.
- Agentes autónomos o continuos, subagentes anidados, coordinadores libres o
  frameworks generales de planificación.
- Base de grafos o vectorial obligatoria. SQLite, FTS5, CTE y un índice
  semántico in-process son la primera opción; cualquier servicio externo exige
  superar un gate medido y justificar migración y operación.
- GraphRAG sobre el canon estructurado nativo; solo se permite extracción
  graph-aware durante importación de material no estructurado.
- Solvers generales de teoremas, OWL, Datalog, AGM, BDI, Dung, redes temporales
  completas o Event Calculus general.
- Plugins, marketplace de proveedores, interfaces de extensión especulativas o
  compatibility shims.
- Mapas procedurales, generación procedural, binarios multimedia administrados,
  simulador universal, economía matemática, combate o galaxias.
- DSL astronómico o motor universal de calendarios.
- Generación de novelas completas.
- Sincronización Markdown bidireccional viva, Python embebido o servicios
  locales obligatorios administrados por el usuario.

## Reglas de ingeniería

Estas reglas son obligatorias durante la ejecución:

1. Leer el flujo afectado de extremo a extremo y buscar implementaciones
   existentes antes de crear código.
2. Preservar cambios ajenos y limitar cada cambio a la necesidad actual.
3. YAGNI primero; DRY solo después de duplicación real.
4. No crear capas `service` o `repository`, factories, barrels, wrappers,
   aliases, re-exports, compatibility layers ni interfaces con una sola
   implementación.
5. Preferir Rust, SQLite, HTML y controles nativos antes de agregar
   dependencias. Incorporar cada dependencia solo cuando una tarea ya la use.
6. Mantener una única ruta evidente para persistir, validar, llamar IA,
   manejar errores y aplicar estilos.
7. Core permanece síncrono y libre de I/O. SQL vive en store; prompts y HTTP en
   ai; workflow en app; transporte y presentación en desktop.
8. La IA no recibe herramientas ni handles de escritura. Texto libre nunca se
   interpreta como mutación.
9. Todo `ChangeSet` se confirma completo o se revierte completo.
10. Los errores duros provienen de esquema, invariantes implementadas o
    constraints SQLite; un hallazgo LLM es evidencia revisable.
11. No registrar chain-of-thought ni lore en logs por defecto; guardar solo
    fuentes, operaciones, reportes, contadores y decisiones necesarias.
12. La lógica no trivial debe dejar al menos un check ejecutable. Usar las
    pruebas nativas de Cargo y constraints SQLite; no agregar frameworks sin
    necesidad.
13. Ejecutar la comprobación más pequeña que cubra el cambio y documentar
    comandos oficiales únicamente después de verificarlos.
14. No declarar una tarea completada con errores conocidos o comportamiento no
    comprobado.

## Leyenda de estados

| Estado | Significado |
|---|---|
| `Pendiente` | No iniciada. Estado inicial de toda tarea de implementación. |
| `En progreso` | Trabajo activo con alcance y dependencias satisfechas. |
| `Bloqueado` | No puede continuar; debe registrarse la causa concreta. |
| `Completado` | Entregable implementado y criterio de aceptación verificado. |

## Secuencia recomendada

Las fases son secuenciales. Dentro de una fase solo pueden adelantarse tareas
sin dependencias pendientes; no se asume paralelismo artificial. Cada fase deja
un producto más utilizable que la anterior.

## Fase 0 — Primer corte vertical local

**Resultado:** una aplicación Tauri mínima crea un mundo, lo guarda en un
`.nirmata`, lo cierra y lo reabre sin IA.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-001 | Completado | — | Crear el workspace Rust 2024 mínimo. | Añadir el `Cargo.toml` raíz y únicamente `crates/nirmata-core`, `crates/nirmata-store`, `crates/nirmata-app` y `apps/nirmata-desktop`. Configurar dependencias dirigidas: store y app dependen de core; desktop depende de app. No crear todavía `nirmata-ai`, crates auxiliares, barrels ni módulos vacíos “para después”. | El workspace compila con los cuatro miembros iniciales; `nirmata-core` no contiene dependencias de SQLite, HTTP, GUI ni runtime async. | [Workspace](docs/architecture/workspace.md), [AGENTS](AGENTS.md) |
| NIR-002 | Completado | NIR-001 | Definir primitivas mínimas de core. | Crear newtypes serializables para `WorldId` y `RevisionId`, un `World` con `id`, `name`, `premise_md`, `epoch_label`, `current_revision`, `created_at_ms` y `updated_at_ms`, y errores de dominio explícitos. Usar UUID para identidad y `i64` UTC milisegundos para tiempo editorial; no mezclarlo con ticks narrativos. Validar nombre no vacío y límites razonables de texto. | Pruebas unitarias demuestran creación válida, rechazo de nombre vacío y round-trip de serialización; core sigue síncrono y sin I/O. | [Modelo](docs/domain/model.md), [Sistema](docs/architecture/system-overview.md) |
| NIR-003 | Completado | NIR-001, NIR-002 | Abrir y crear el archivo SQLite canónico. | En store, abrir rutas con extensión `.nirmata`, habilitar `foreign_keys`, usar transacciones para migraciones y crear tablas mínimas `schema_migrations`, `worlds` y `revisions`. La revisión inicial no tiene padre y representa la creación del mundo. Rechazar archivos con versión de esquema más nueva y reportar errores de ruta, lock o corrupción sin sobrescribir. | Una prueba temporal crea un archivo, verifica esquema y revisión inicial, lo reabre y obtiene el mismo `World`; un fallo de apertura no crea un mundo parcial. | [Almacenamiento](docs/architecture/storage.md), [Versionado](docs/research/critical-fronts/canon-versioning.md) |
| NIR-004 | Completado | NIR-002, NIR-003 | Implementar casos de uso `create_world`, `open_world` y `close_world`. | En app, aceptar DTOs simples, validar en core y delegar persistencia a store. Mantener una sesión con ruta, `world_id` y revisión vigente. Impedir que dos mundos estén activos en la misma sesión y devolver errores tipados para archivo inexistente, formato inválido y esquema incompatible. | Pruebas de app crean, cierran y reabren un mundo conservando nombre, premisa y revisión; ningún caso de uso expone una conexión o SQL. | [Sistema](docs/architecture/system-overview.md), [MVP](docs/product/mvp.md) |
| NIR-005 | Completado | NIR-001, NIR-004 | Crear la carcasa Tauri 2 y transporte mínimo. | Añadir comandos Tauri específicos `create_world`, `open_world`, `get_current_world` y `close_world`. Usar un frontend web pequeño con TypeScript, HTML y CSS nativos, sin librería de componentes. Mostrar selector de archivo, formulario de nombre/premisa, estado de carga y errores accionables. No exponer acceso libre a archivos ni un comando genérico. | Desde la GUI se crea un `.nirmata`, se muestra el mundo activo, se cierra y se reabre; cancelar el selector o fallar la ruta no bloquea la interfaz. | [Runtime](docs/architecture/language-runtime.md), [Interacción](docs/architecture/interaction-model.md) |
| NIR-006 | Completado | NIR-003, NIR-004, NIR-005 | Probar el primer corte vertical completo. | Añadir la prueba de integración más pequeña que atraviese core, store y app; complementar con una verificación manual de la frontera Tauri sin introducir un framework E2E. Comprobar persistencia real en disco, reapertura y mensajes de error. | El corte crea y reabre un mundo en un proceso nuevo, conserva la revisión y deja el archivo válido tras cerrar; todas las pruebas nativas existentes pasan. | [Corte vertical](docs/validation/vertical-slice.md), [AGENTS](AGENTS.md) |

## Fase 1 — Canon manual estructurado

**Resultado:** el backend puede representar y persistir todo el dominio mínimo
del canon sin depender de IA.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-007 | Completado | NIR-002 | Completar `World` y definir `Rule`. | Mantener `World` con premisa y epoch. Definir `Rule` con `id`, `world_id`, `kind` (`constitutive`, `generative`, `institutional`, `authorial`), `statement_md`, `scope`, `severity`, fuente opcional, `validator_kind` opcional, parámetros JSON, versión y timestamps. Solo reglas con un `validator_kind` implementado podrán producir error duro. Prioridades, reglas derrotables y excepciones estructuradas se agregan únicamente cuando un caso real las consuma. | Core rechaza kind/severidad desconocidos y parámetros incompatibles con un validador conocido; reglas puramente semánticas siguen permitidas y llegan a revisión sin fingir validación Rust. | [Modelo](docs/domain/model.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-008 | Completado | NIR-002 | Definir `Entity` y sus invariantes. | Campos: `id`, `world_id`, `kind`, `name`, `slug`, `summary`, `body_md`, `attributes_json`, aliases, versión y timestamps. El slug es único por mundo pero no es identidad; aliases no pueden ser vacíos ni duplicarse por entidad. Mantener `attributes_json` como escape local, sin tabla por tipo ni esquema genérico. | Pruebas cubren nombre/slug, aliases normalizados, JSON válido y renombrado sin cambiar el ID. | [Almacenamiento](docs/architecture/storage.md), [Modelo](docs/domain/model.md) |
| NIR-009 | Completado | NIR-008 | Definir `Relation`. | Campos: `id`, `world_id`, `source_entity_id`, `target_entity_id`, `kind`, `direction`, `valid_from_tick`, `valid_to_tick`, `certainty`, `source_reference`, `metadata_json`, versión. Prohibir extremos inexistentes, intervalo invertido y duplicado exacto; permitir auto-relación solo para kinds declarados explícitamente. No crear una ontología extensible ni base de grafos. | Validadores identifican duplicados, extremos inválidos e intervalo invertido, y aceptan relaciones dirigidas y no dirigidas válidas. | [Modelo](docs/domain/model.md), [Recuperación](docs/architecture/retrieval.md) |
| NIR-010 | Completado | NIR-008 | Definir `Goal`. | Campos: `id`, `world_id`, `holder_entity_id`, `desired_state_md`, `priority`, `status` (`active`, `achieved`, `abandoned`, `frustrated`), periodo opcional, `visibility` (`public`, `secret`), fuente, versión. Un deseo persistente es Goal, no Claim; no inferir motivación cuando falte. | Core exige holder existente, estado deseado no vacío y periodo ordenado; una acción sin Goal puede quedar como motivación desconocida sin error duro. | [Modelo](docs/domain/model.md), [Fundamentos](docs/research/academic-foundations/design-consequences.md) |
| NIR-011 | Completado | NIR-002 | Implementar `EventTime` y comparaciones temporales necesarias. | Definir `kind` (`unknown`, `instant`, `interval`, `ongoing`), `start_tick`, `end_tick`, `precision` (`exact`, `day`, `month`, `year`, `era`, `unknown`) y `certainty` (`certain`, `approximate`, `uncertain`, `approximate_uncertain`). Validar combinaciones por kind. Implementar como funciones puras únicamente `before`, `after`, `overlaps`, `during`, `contains` y `equals`, que cubren causalidad, solapamiento de claims y lifecycle del MVP; no persistir resultados derivados. | Pruebas de tabla cubren las seis comparaciones, límites iguales, intervalos inválidos, `ongoing` sin fin y `unknown` sin posición inventada. Las demás relaciones de Allen no existen hasta que una regla concreta las necesite. | [Tiempo](docs/research/critical-fronts/narrative-time.md), [Modelo](docs/domain/model.md) |
| NIR-012 | Completado | NIR-008, NIR-010, NIR-011 | Definir `Event`, participantes y causalidad. | `Event` incluye `id`, `world_id`, `kind`, `summary`, `body_md`, `EventTime`, ubicación opcional, versión y timestamps. Participantes guardan entidad, rol y ordinal. Enlaces entre eventos usan `enables`, `causes`, `motivates`, `prevents`, `terminates` o `reveals`; goals afectados se enlazan explícitamente. No almacenar relaciones temporales derivables. | Core rechaza participante/evento inexistente, ordinal duplicado y causalidad hacia el mismo evento; una causa posterior conocida se reporta como conflicto temporal. | [Modelo](docs/domain/model.md), [Narrativa computacional](docs/research/academic-foundations/computational-narrative.md) |
| NIR-013 | Completado | NIR-008, NIR-011 | Definir `Claim` con contexto epistemológico. | Campos: `id`, `world_id`, sujeto, `content_md`, `predicate_key` opcional, objeto entidad o escalar opcional, `polarity`, `authentication`, holder opcional, modalidad opcional, register opcional, base epistemológica, fuente, documento/claim de procedencia opcionales, confianza declarada opcional, periodo, revisión de alta/sustitución y versión. `canonical` no lleva holder ni modalidad; `attributed` exige ambos. La confianza pertenece al holder, no al modelo. | Pruebas cubren canon, rumor, creencia, negación explícita, dato desconocido y procedencia inválida; contextos distintos pueden contener claims opuestos. | [Conocimiento incierto](docs/research/critical-fronts/uncertain-knowledge.md), [Modelo](docs/domain/model.md) |
| NIR-014 | Completado | NIR-008, NIR-012, NIR-013 | Definir `Document` y `ContentReference`. | `Document` contiene `id`, `world_id`, `title`, `kind`, autor/perspectiva opcional, estado canónico, `body_md`, versión y timestamps. `ContentReference` enlaza un source tipado con un target tipado por ID y `ordinal`; el ordinal es único por contenido y define orden de discurso. Proveer parser de URI `nirmata://<kind>/<uuid>` sin acceso físico a archivos. | Pruebas demuestran orden estable de menciones, resolución de URI, rechazo de targets inexistentes y que un flashback solo cambia ordinal, no `EventTime`. | [Almacenamiento](docs/architecture/storage.md), [Tiempo](docs/research/critical-fronts/narrative-time.md) |
| NIR-015 | Completado | NIR-007–NIR-014 | Consolidar validadores deterministas de core. | Crear funciones explícitas, no un framework de reglas, para validar referencias, cardinalidad, unicidad, versiones, claims, periodos, causalidad y estados de ciclo de vida mínimos (`alive/dead`, rol, posesión, relación activa cuando estén codificados). Devolver `ValidationIssue` con código estable, severidad, objetos y mensaje; ausencia de datos produce `unspecified`, no `false`. | Una suite unitaria cubre referencias rotas, muerte antes de nacimiento, actor después de muerte, causalidad posterior, claims canónicos opuestos y casos válidos con vacíos. | [Validación](docs/architecture/validation-pipeline.md), [AGENTS](AGENTS.md) |
| NIR-016 | Completado | NIR-003, NIR-007–NIR-014 | Migrar el esquema completo del canon manual. | Añadir tablas para rules, entities, entity_aliases, relations, events, event_participants, event_links, event_goals, goals, claims, documents y content_references. Usar foreign keys, `CHECK`, índices por `world_id`, IDs y tiempo, y unicidad donde sea cerrada. Markdown permanece `TEXT`; JSON permanece texto validado en el borde. | Migraciones nuevas y desde el esquema inicial se ejecutan atómicamente; constraints rechazan FK rota, slug duplicado e intervalo imposible sin dejar filas parciales. | [Almacenamiento](docs/architecture/storage.md), [Sistema](docs/architecture/system-overview.md) |
| NIR-017 | Completado | NIR-015, NIR-016 | Implementar operaciones de lectura y escritura manual en store. | Añadir funciones orientadas al dominio para insertar, obtener, listar y actualizar cada agregado; usar SQL parametrizado y conversiones de fila locales al módulo correspondiente. Toda actualización comprueba `version` y la incrementa. Las escrituras relacionadas, como evento con participantes o documento con referencias, ocurren en una transacción. | Pruebas de store hacen round-trip de cada tipo, detectan versión obsoleta, prueban rollback de agregado incompleto y no exponen SQL o `rusqlite::Connection` fuera de store. | [Almacenamiento](docs/architecture/storage.md), [AGENTS](AGENTS.md) |

## Fase 2 — Cambios seguros, revisiones y undo

**Resultado:** toda edición manual pasa por revisión estructurada, validación y
commit atómico con historial reversible.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-018 | Completado | NIR-007–NIR-015 | Definir `ChangeSetDraft`, `ChangeSet` y operaciones tipadas. | Incluir `id`, `world_id`, `base_revision`, objetivo, fuentes, supuestos, operaciones y decisiones. Cada operación tiene `operation_id`, tipo concreto, payload antes/después, IDs afectados, `expected_version` y retcon (`additive`, `reinterpretive`, `replacement`). Cubrir crear/actualizar/eliminar los agregados del MVP y actualizar listas relacionadas sin una operación genérica de SQL/JSON Patch. | Serialización round-trip conserva todos los tipos; payload desconocido o campo requerido ausente falla de forma explícita; ninguna operación puede referir otro mundo. | [Versionado](docs/research/critical-fronts/canon-versioning.md), [Modelo](docs/domain/model.md) |
| NIR-019 | Completado | NIR-015, NIR-018 | Validar esquema lógico e integridad estructural del ChangeSet. | Validar tamaño, IDs, operaciones duplicadas, referencias existentes o creadas en el mismo set, orden de dependencias, cardinalidad, expected versions y eliminaciones huérfanas. Generar un `ValidationReport` determinista con errores, conflictos, warnings e info. No interpretar texto libre como operación. | Tests bloquean ID inexistente, doble escritura del mismo campo, delete huérfano y dependencia ausente; aceptan crear una entidad y referirla después dentro del mismo set. | [Validación](docs/architecture/validation-pipeline.md), [Flujo IA](docs/architecture/ai-flow.md) |
| NIR-020 | Completado | NIR-007, NIR-011, NIR-012, NIR-019 | Validar tiempo, ciclo de vida y reglas codificadas. | Evaluar el estado resultante, no solo cada operación aislada. Comprobar forma temporal, vida/muerte, estados mutuamente exclusivos, causalidad, participantes y los `validator_kind` realmente implementados. Empezar solo con validadores necesarios por fixtures del MVP, incluido `no_resurrection`; reglas semánticas generan material para crítica, no error Rust fingido. | Tests prueban que una resurrección prohibida y una causa posterior bloquean; una regla institucional violable o una dimensión no especificada no se convierte automáticamente en error duro. | [Validación](docs/architecture/validation-pipeline.md), [Tiempo](docs/research/critical-fronts/narrative-time.md) |
| NIR-021 | Completado | NIR-013, NIR-014, NIR-019, NIR-020 | Calcular subgrafo afectado, conflictos y `DecisionPoint`. | Desde store obtener objetos escritos, referencias entrantes, eventos dependientes, reglas, documents y content references. Detectar claims canónicos normalizados opuestos con periodos solapados y efectos codificados omitidos. `replacement` exige target sustituido, razón y un `DecisionPoint` resuelto; `reinterpretive` conserva lo anterior. | Un reemplazo sin decisión bloquea; un retcon aditivo no borra datos; claims opuestos de holders distintos coexisten; claims canónicos opuestos activos bloquean el resultado final. | [Validación](docs/architecture/validation-pipeline.md), [Conocimiento incierto](docs/research/critical-fronts/uncertain-knowledge.md) |
| NIR-022 | Completado | NIR-016, NIR-018 | Persistir revisiones, drafts, operaciones y auditoría. | Añadir `change_sets`, `change_operations`, `decision_points`, `change_set_waivers`, `revisions` completas y registros de auditoría con valores antes/después, decisión por operación, fuente, timestamp, base/result revision y resumen. Guardar reportes deterministas; no guardar chain-of-thought. La cadena de revisiones tiene una sola cabeza. | Foreign keys unen commit, operaciones y revisión; no puede existir una segunda cabeza ni una operación auditada sin ChangeSet; los reportes se recuperan tras reabrir. | [Almacenamiento](docs/architecture/storage.md), [Versionado](docs/research/critical-fronts/canon-versioning.md) |
| NIR-023 | Completado | NIR-018–NIR-022 | Implementar workflow manual de propuesta y revisión en app. | Convertir ediciones de formularios en draft, validar, permitir `accept`, `edit` o `reject` por operación y reconstruir un nuevo conjunto solo con operaciones elegidas. Registrar excepciones intencionales con razón; una excepción no puede ocultar constraints ni errores duros. Revalidar después de cada edición o cambio de selección. | Pruebas demuestran que rechazar una operación dependida muestra la dependencia rota, editar cambia el reporte y solo un conjunto válido queda listo para confirmar. | [Interacción](docs/architecture/interaction-model.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-024 | Completado | NIR-017, NIR-021–NIR-023 | Confirmar ChangeSets con revalidación y transacción atómica. | Comparar `base_revision` con la cabeza actual, recargar versiones, ejecutar nuevamente validadores y abrir una única transacción. Aplicar operaciones, guardar antes/después, actualizar content references e índices derivados, crear revisión hija e incrementar la cabeza. Ante lock, disco lleno o constraint, hacer rollback y conservar el draft revisable. | Tests fuerzan constraint y versión obsoleta: no cambia ninguna tabla de canon ni revisión. Un commit válido aplica todas las operaciones y produce exactamente una nueva revisión. | [Sistema](docs/architecture/system-overview.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-025 | Completado | NIR-022, NIR-024 | Implementar undo lineal como ChangeSet inverso. | Generar desde auditoría las operaciones inversas del último commit no deshecho, validar contra la cabeza vigente y confirmarlas como una nueva revisión; no mover silenciosamente el puntero ni reconstruir por event sourcing. Bloquear undo si no puede restaurar constraints y mostrar el conflicto. | Crear, modificar y eliminar objetos puede deshacerse conservando auditoría; cerrar y reabrir mantiene el estado; no se puede deshacer una revisión que no sea ancestro inmediato lógico del estado actual. | [Almacenamiento](docs/architecture/storage.md), [Versionado](docs/research/critical-fronts/canon-versioning.md) |
| NIR-026 | Completado | NIR-018–NIR-025 | Probar el workflow manual completo. | Añadir pruebas integradas para draft, selección parcial, edición, decisión, stale revision, commit, rollback, auditoría y undo. Usar fixtures pequeñas que incluyan entidad, evento, claim y documento; no crear un framework de fixtures genérico. | Las pruebas reproducen al menos un cambio válido, uno estructuralmente inválido, uno obsoleto, un replacement y un undo; todos verifican estado y auditoría en SQLite. | [Suite IA](docs/validation/ai-regression-suite.md), [AGENTS](AGENTS.md) |

## Fase 3 — Recuperación, búsqueda y proyección lógica

**Resultado:** el usuario puede encontrar, abrir y contextualizar el canon con
fuentes navegables, todavía sin IA.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-027 | Completado | NIR-016, NIR-024 | Añadir FTS5 como índice derivado. | Crear una tabla virtual FTS5 para nombre/título, summary y Markdown de rules, entities, events, claims y documents. Mantenerla explícitamente dentro de la misma transacción que cambia contenido y ofrecer reconstrucción completa desde tablas canónicas. FTS nunca es autoridad ni bloquea recuperar el canon. | Búsquedas encuentran texto actualizado, eliminaciones desaparecen del índice y una reconstrucción produce los mismos resultados; fallo del índice no corrompe tablas canónicas. | [Recuperación](docs/architecture/retrieval.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-028 | Completado | NIR-009–NIR-014, NIR-017 | Implementar recuperación estructurada por anclas. | Dado un conjunto explícito de IDs, cargar objetos, relaciones directas con límite, eventos asociados, participantes, claims, goals y reglas aplicables. Usar consultas parametrizadas y límites codificados; no recuperar todo el componente conectado ni introducir un planificador LLM. | Tests verifican prioridad de anclas, límite de vecinos, procedencia y aislamiento por mundo; objetos no relacionados no aparecen por coincidencia accidental. | [Recuperación](docs/architecture/retrieval.md), [Sistema](docs/architecture/system-overview.md) |
| NIR-029 | Completado | NIR-027, NIR-028 | Construir `ContextBundle` determinista. | Combinar selección, relaciones, ventana temporal, goals/intenciones, perspectiva epistemológica y finalmente FTS5. Deduplicar por ID, separar canon/perspectivas/deseos/obligaciones, conservar citas y aplicar presupuesto por cantidad de objetos/caracteres. Una dimensión ausente permanece vacía. | Para consultas de entidad, impacto, contradicción y documento, pruebas verifican fuentes esperadas, ausencia de duplicados, límites y que canon tenga prioridad salvo petición de perspectivas. | [Recuperación](docs/architecture/retrieval.md), [Narratología](docs/research/academic-foundations/cognitive-narratology.md) |
| NIR-030 | Completado | NIR-014, NIR-017 | Implementar URI estable y VFS lógico de lectura. | Resolver `nirmata://entity/<uuid>`, `event`, `claim`, `rule`, `goal` y `document`; producir un árbol lógico agrupado por tipo desde consultas SQLite. Los nombres visibles pueden cambiar sin alterar URI. No implementar filesystem virtual real, exportación física ni sincronización Markdown. | Renombrar un objeto conserva enlaces; URI inválida o de otro mundo devuelve error tipado; el árbol se regenera únicamente desde SQLite. | [Almacenamiento](docs/architecture/storage.md), [MVP](docs/product/mvp.md) |
| NIR-031 | Completado | NIR-027–NIR-030 | Exponer casos de uso de búsqueda, navegación y contexto. | En app implementar `search_world`, `open_uri`, `get_related_context`, filtros por tipo y ventana temporal. Definir resultados con snippet, tipo, ID, autoridad y fuente. Preparar el contrato de clasificación `fact`, `perspective`, `inference`, `no_evidence`, `unspecified`, aunque las inferencias de IA aún no existan. | Una búsqueda permite abrir la fuente exacta; filtros y ventanas son estables; “sin coincidencias” no se presenta como negación del canon. | [Recuperación](docs/architecture/retrieval.md), [Interacción](docs/architecture/interaction-model.md) |
| NIR-032 | Completado | NIR-027–NIR-031 | Evaluar recuperación con preguntas conocidas. | Crear un conjunto pequeño de fixtures y preguntas que midan fuentes necesarias, irrelevantes, contradicciones omitidas y presupuesto. Incluir vocabulario exacto para FTS, perspectiva, evento causal y dato no especificado. No agregar embeddings, rerankers ni servicios. | Todas las fuentes marcadas como necesarias aparecen dentro del límite; no se mezclan holders incompatibles; el resultado documenta cualquier fallo real antes de considerar otra tecnología. | [Recuperación](docs/architecture/retrieval.md), [Corte vertical](docs/validation/vertical-slice.md) |

## Fase 4 — Producto de escritorio manual

**Resultado:** una persona puede construir y revisar un mundo completo desde la
GUI sin usar IA.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-033 | Completado | NIR-005, NIR-023, NIR-031 | Construir la disposición principal de la GUI. | Implementar panel izquierdo de navegación/búsqueda, panel central de editor, panel derecho de contexto/advertencias y panel inferior de cambios pendientes. Usar estado explícito del frontend y comandos Tauri específicos; evitar store global genérico, framework visual y abstracciones de componentes sin reutilización real. | Los paneles se pueden redimensionar o colapsar, el foco y teclado son utilizables y seleccionar un objeto actualiza editor y contexto sin recargar el mundo. | [Interacción](docs/architecture/interaction-model.md), [Workspace](docs/architecture/workspace.md) |
| NIR-034 | Completado | NIR-030, NIR-031, NIR-033 | Completar apertura, navegación, filtros y búsqueda. | Mostrar árbol VFS lógico, recientes de la sesión, filtros por tipo y resultados FTS con snippet. Abrir por URI y conservar selección al renombrar. Manejar archivo movido, mundo cerrado y objeto eliminado con estados vacíos claros, no panics. | El usuario crea/reabre un mundo, busca una frase, abre la fuente y navega a relaciones; errores no dejan selección fantasma ni comandos activos sobre un mundo cerrado. | [Interacción](docs/architecture/interaction-model.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-035 | Completado | NIR-007–NIR-014, NIR-023, NIR-033 | Implementar formularios de edición manual como drafts. | Proveer formularios concretos para World/Rule/Entity/Relation/Goal/Event/Claim/Document, incluyendo enums, periodos, aliases, participantes, causalidad y procedencia. Guardar genera un `ChangeSetDraft`; nunca escribe directamente. Usar textarea para Markdown y controles HTML nativos. | Cada tipo puede crearse y editarse; validaciones aparecen junto al campo y en cambios pendientes; cerrar con cambios no revisados solicita descartar o continuar. | [Modelo](docs/domain/model.md), [Interacción](docs/architecture/interaction-model.md) |
| NIR-036 | Completado | NIR-011, NIR-012, NIR-014, NIR-035 | Añadir timeline y edición de documentos/referencias. | Mostrar eventos ordenados por ticks conocidos y agrupar unknown aparte; distinguir instant, interval y ongoing, precisión y certeza. En documentos permitir añadir/reordenar `ContentReference` y abrir targets; el ordinal expresa discurso sin alterar story time. No crear calendario, Scene ni mapa. | Un intervalo, evento ongoing, fecha aproximada y flashback se muestran correctamente; reordenar referencias solo cambia ordinal. | [Tiempo](docs/research/critical-fronts/narrative-time.md), [Interacción](docs/architecture/interaction-model.md) |
| NIR-037 | Completado | NIR-023, NIR-024, NIR-033 | Implementar revisión por operación. | En el panel inferior mostrar objetivo, fuentes, before/after, severidad, dependencias y decisiones. Permitir aceptar, editar o rechazar cada operación, reconocer warnings y registrar waiver con razón. Deshabilitar confirmación mientras haya error, conflicto no resuelto o dependencia rota. | Seleccionar un subconjunto dispara revalidación y actualiza el diff; confirmar aplica una sola transacción; el estado visual coincide con el ChangeSet persistido. | [Interacción](docs/architecture/interaction-model.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-038 | Completado | NIR-021, NIR-024, NIR-037 | Aplicar fricción de alto riesgo y manejar obsolescencia. | Para `replacement`, conflicto duro o cambio de impacto amplio, pedir primero un juicio breve del usuario y solo después mostrar resolución sugerida disponible. Si cambia la revisión, marcar el draft obsoleto, deshabilitar commit y ofrecer una revalidación; un segundo cambio durante el refresco obliga a reiniciar. | Una propuesta obsoleta nunca se confirma; la sugerencia de resolución permanece oculta hasta registrar juicio; la evidencia de un error duro nunca se oculta. | [Cocreación](docs/research/critical-fronts/human-ai-cocreation.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-039 | Completado | NIR-025, NIR-030, NIR-033–NIR-038 | Completar seguridad, auditoría y recuperación de errores en GUI. | Renderizar Markdown como texto seguro o con HTML deshabilitado; validar IDs/rutas en Tauri. Añadir vistas locales de revisión, operaciones, waivers y undo. Mostrar fallos de lock, constraint, archivo y validación sin perder el draft. No registrar lore en consola por defecto. | Contenido HTML/script no se ejecuta; undo se realiza desde una revisión visible; un fallo de commit conserva canon y propuesta; auditoría permite seguir antes/después y decisión humana. | [Runtime](docs/architecture/language-runtime.md), [AGENTS](AGENTS.md) |

## Fase 5 — IA controlada estándar

**Resultado:** la aplicación consulta el canon y propone cambios mediante un
único proveedor, siempre con contexto, validación, crítica y revisión humana.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-040 | Completado | NIR-032, NIR-039 | Crear `nirmata-ai`, el cliente del proveedor inicial y el flujo de credenciales. | Añadir el crate al workspace solo ahora. Incorporar únicamente dependencias usadas para HTTP, async, streaming, serialización y almacén seguro. Implementar un cliente concreto, sin trait de proveedor ni factory. App expone casos de uso para guardar, consultar solo el estado (`configured`/`missing`) y borrar la clave; el valor nunca regresa al frontend. Guardarla en el almacén seguro del sistema o, si falla, mantenerla solo en memoria durante la sesión y explicar esa limitación. Nunca escribirla en `.nirmata`, prompts persistidos o logs. Incluir timeout y cancelación. | Pruebas con transporte y credenciales simulados cubren missing key, set/status/clear, éxito, timeout, cancelación, HTTP inválido y stream interrumpido; ningún tipo de store/commit es accesible desde ai y la clave no aparece en errores o trazas. | [Runtime](docs/architecture/language-runtime.md), [Fuentes técnicas](docs/research/technical-sources.md) |
| NIR-041 | Completado | NIR-018, NIR-040 | Definir contratos estructurados y parsing estricto. | Crear DTOs de IA para respuesta consultiva, `ChangeSetDraft` y `CritiqueReport` con límites de tamaño, enums cerrados, IDs, fuentes y operaciones conocidas. Rechazar JSON parcial, texto libre disfrazado de mutación, URI inválida y referencias de contenido faltantes en prosa generada. El error conserva respuesta diagnóstica mínima sin guardar razonamiento privado. | Fixtures válidas parsean; campos/operaciones desconocidos y salida truncada fallan; ninguna salida fallida llega al workflow de commit. | [Validación](docs/architecture/validation-pipeline.md), [Flujo IA](docs/architecture/ai-flow.md) |
| NIR-042 | Completado | NIR-029, NIR-040, NIR-041 | Implementar frontera de capacidades y prompts mínimos. | App decide antes de llamar: `query` solo acepta respuesta con citas; `propose` acepta draft; `critic` solo devuelve reporte. Pasar snapshots y `ContextBundle`, no conexión, SQL, rutas físicas ni función de commit. Usar prompts distintos para generador y crítico, aunque compartan modelo, y registrar modelo/prompt version, IDs de contexto y uso técnico sin lore por defecto. | Tests demuestran que un modo query no puede deserializar operaciones y que ai no recibe capacidades de escritura; todas las fuentes enviadas pertenecen a la revisión base. | [Grafo estándar](docs/architecture/agent-graph.md), [Reasoning](docs/architecture/reasoning-policy.md) |
| NIR-043 | Completado | NIR-031, NIR-040–NIR-042 | Implementar modo `Consultar` con streaming y citas. | Recuperar contexto determinista, transmitir progreso/respuesta y producir items clasificados como hecho, perspectiva, inferencia, sin evidencia o no especificado. Cada afirmación factual/perspectival incluye URI navegable; inferencias no se persisten. Si el usuario pide una escritura, responder con impacto y acción explícita para iniciar propuesta, sin cambiar de modo. | La consulta abre sus fuentes, separa rumor de canon y no inventa respuesta para un vacío; cancelar detiene red y deja la GUI utilizable; no se crea ChangeSet. | [Interacción](docs/architecture/interaction-model.md), [Recuperación](docs/architecture/retrieval.md) |
| NIR-044 | Completado | NIR-019–NIR-021, NIR-040–NIR-042 | Implementar generador de propuestas. | Para solicitudes pequeñas generar directamente un draft; para solicitudes amplias o ambiguas presentar un `IntentBrief` editable con objetivo, alcance, entidades y restricciones. El draft declara objetos afectados, before/after, consecuencias, supuestos, fuentes, content references y retcon. Parsear y ejecutar todos los validadores Rust antes de cualquier crítica. | Una propuesta válida llega a revisión; una salida estructuralmente inválida no se presenta como aplicable; el usuario puede corregir IntentBrief sin que cambie el canon. | [Cocreación](docs/research/critical-fronts/human-ai-cocreation.md), [Flujo IA](docs/architecture/ai-flow.md) |
| NIR-045 | Completado | NIR-021, NIR-041, NIR-044 | Implementar crítico semántico independiente. | Enviar draft, reporte determinista, reglas semánticas y snapshot del subgrafo afectado a una segunda invocación con prompt/contexto separado. `CritiqueReport` incluye issue, operaciones afectadas, evidencia, severidad, categoría, `rebuts`/`undercuts`, confidence y resolución sugerida. El crítico no edita el draft ni puede producir error duro no anulable por sí solo. | Fixtures detectan regla semántica ignorada, conocimiento secreto imposible y consecuencia omitida; el reporte cita IDs y no altera operaciones. | [Reasoning](docs/architecture/reasoning-policy.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-046 | Completado | NIR-041, NIR-044, NIR-045 | Orquestar un único intento de reparación acotado. | Si parsing, validación o crítica produce un problema reparable, app entrega al generador solo el reporte estructurado y solicita un draft completo nuevo. Repetir parsing, validadores y crítica una vez. Contadores viven en Rust; el segundo fallo termina con conflicto visible. No mezclar parches parciales ni abrir un loop ReAct. | Pruebas verifican cero o una reparación, nunca dos; una reparación válida reemplaza íntegramente el draft y una fallida queda revisable pero no confirmable. | [Grafo estándar](docs/architecture/agent-graph.md), [Reasoning](docs/architecture/reasoning-policy.md) |
| NIR-047 | Pendiente | NIR-023, NIR-024, NIR-042–NIR-046 | Integrar el run de IA con revisión, stale check y commit. | Definir estado tipado de ejecución con id, world, base revision, mode, request, context, draft, reportes, repair count, status y error. Después de revisión humana, revalidar revisión y crítica antes de commit. Persistir traza resumida, decisiones y versión de modelo/prompt; no persistir chain-of-thought. | Transiciones inválidas son imposibles o devuelven error; cancelar/fallar red no cambia canon; un draft criticado contra revisión antigua nunca autoriza commit. | [Grafo estándar](docs/architecture/agent-graph.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-048 | Pendiente | NIR-033, NIR-037–NIR-040, NIR-043–NIR-047 | Añadir el asistente `Consultar` / `Proponer` a la GUI. | Un solo panel con modo explícito, `Consultar` predeterminado, selección/contexto visibles, streaming para consulta y progreso para propuesta. Incluir una configuración mínima para ingresar, reemplazar y borrar la clave, mostrar únicamente si está configurada y deshabilitar acciones IA con explicación cuando falte. Mantener transcript separado del panel de cambios. Mostrar reportes por severidad, fuentes expandibles y dos o tres DecisionPoints como máximo; aplicar juicio previo en alto riesgo. | Cambiar de modo requiere acción explícita; una respuesta de chat no se confunde con canon; ninguna llamada sale sin credencial configurada y solo un draft completamente recibido, validado y criticado habilita revisión. | [Interacción](docs/architecture/interaction-model.md), [Cocreación](docs/research/critical-fronts/human-ai-cocreation.md) |
| NIR-049 | Pendiente | NIR-041–NIR-048 | Implementar la suite automatizada de regresión de IA del MVP. | Crear snapshots, solicitudes, contexto esperado, drafts e issues esperados para los casos críticos: referencia inexistente, actor muerto, causa posterior, regla codificada/semántica, rumor válido, vacío, negación, claims canónicos opuestos, replacement sin decisión, intervalo inválido, ongoing, fecha aproximada, flashback, procedencia rota, stale revision y excepción trazable. Separar tests deterministas de tests de contrato con respuestas grabadas; no depender de red para la suite normal. | 100% de errores estructurales conocidos bloquean, ningún stale commit pasa, rumores válidos no bloquean y cada conflicto cita la fuente necesaria. Los tests fallan ante regresión de parser, prompt fixture o recuperación. | [Suite IA](docs/validation/ai-regression-suite.md), [Reasoning](docs/architecture/reasoning-policy.md) |

## Fase 6 — Hito del fundamento funcional

**Resultado:** el fundamento funciona de extremo a extremo y resiste los fallos
que podrían corromper o confundir el canon; las fases generales pueden apoyarse
en él sin redefinir persistencia, revisión ni autoridad.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-050 | Pendiente | NIR-026, NIR-032, NIR-049 | Automatizar el escenario vertical de la mina. | Crear un fixture con imperio, ciudad minera, religión, mineral de recuerdos, relaciones, goals, reglas, eventos y claims. Ejecutar la caída de la mina como propuesta con consecuencias económicas, políticas y religiosas, fuentes, causalidad y perspectivas. Seleccionar, editar y rechazar operaciones antes del commit. | El test conserva hecho e interpretaciones internas, no completa vacíos, enlaza cada consecuencia a evidencia/goal o marca motivación desconocida, aplica atomically y permite undo. | [Corte vertical](docs/validation/vertical-slice.md), [Visión](docs/product/vision.md) |
| NIR-051 | Pendiente | NIR-024–NIR-025, NIR-039, NIR-047 | Verificar durabilidad y fallos de frontera. | Probar rollback ante constraint, lock y fallo simulado de índice; reapertura tras commit/undo; rechazo de esquema futuro; stale revision; stream cancelado; salida IA truncada; URI/ruta inválida y Markdown hostil. Verificar que logs no contienen cuerpos de lore ni claves. | En todos los fallos el archivo sigue abriendo, la cabeza y auditoría son coherentes, no hay aplicación parcial y la GUI ofrece recuperación o reintento explícito. | [Validación](docs/architecture/validation-pipeline.md), [Runtime](docs/architecture/language-runtime.md) |
| NIR-052 | Pendiente | NIR-050, NIR-051 | Ejecutar aceptación completa del fundamento funcional. | Recorrer desde GUI: crear/reabrir mundo; editar todos los tipos; buscar y navegar; configurar credencial; consultar con citas; proponer; validar; criticar; aceptar/editar/rechazar por operación; resolver replacement; revalidar stale; confirmar; auditar y deshacer. Ejecutar todas las pruebas Cargo existentes y registrar comandos oficiales solo después de éxito. | Se cumple la Definition of Done del fundamento y pasan NIR-049, NIR-050 y NIR-051; ninguno de los fallos enumerados allí reproduce corrupción, escritura parcial o commit autorizado por IA. El hito funciona sin embeddings, multiagentes ni servicios auxiliares, que pertenecen a capacidades posteriores y medidas. | [MVP](docs/product/mvp.md), [AGENTS](AGENTS.md) |

## Definition of Done del fundamento funcional

El fundamento funcional está terminado únicamente cuando:

1. La aplicación Tauri crea y reabre un `.nirmata` portable; SQLite es la única
   fuente canónica y las migraciones son atómicas.
2. Se pueden crear, editar y navegar World, Rule, Entity, Relation, Goal,
   Event, Claim y Document con sus campos, referencias e invariantes descritos.
3. Story time, incertidumbre, intervalos, causalidad, goals, perspectivas,
   negación explícita y datos desconocidos se conservan sin confundirse.
4. Toda escritura manual o de IA se representa como operaciones tipadas,
   revisadas y confirmadas en una única transacción.
5. Existen revisiones lineales, auditoría antes/después, waivers trazables y
   undo verificado después de reabrir.
6. Búsqueda SQL/FTS5, contexto determinista, URI estable y VFS lógico permiten
   llegar desde una respuesta a sus fuentes.
7. `Consultar` nunca prepara escritura; `Proponer` ejecuta generador,
   validadores Rust, crítico independiente, revisión humana y stale
   revalidation antes del commit.
8. La revisión acepta, edita o rechaza cada operación y bloquea dependencias,
   errores, decisiones pendientes y revisiones obsoletas.
9. Los replacements aplican juicio humano previo; los retcons aditivos y
   reinterpretativos no borran canon incorrectamente.
10. Existe como máximo un intento de reparación y ningún componente de IA
    recibe capacidad de commit.
11. Constraints y pruebas automatizadas demuestran rollback, conflictos
    críticos, recuperación, auditoría y el escenario de la mina.
12. Claves, lore y chain-of-thought no se filtran en almacenamiento o logs no
    destinados a ello.

## Decisiones y gates obligatorios

Estas decisiones evitan convertir evoluciones útiles en infraestructura
predeterminada. Un gate se registra con corpus, métricas, fecha y conclusión;
no deja interfaces, servicios o tablas preventivas cuando la conclusión es
“todavía no”.

| Registro | Decisión vigente | Gate para cambiarla |
|---|---|---|
| DR-001 — RAG determinista | RAG es obligatorio desde NIR-029: anclas, SQL, relaciones, tiempo, goals, perspectivas y FTS5 producen contexto citado. Embeddings no definen si existe RAG. | Solo cambia el ranking o los índices derivados; la autoridad, procedencia y contrato de respuesta permanecen iguales. |
| DR-002 — Semántica condicional | No se añaden embeddings al fundamento. Se prefiere un índice in-process dentro del mismo proyecto/SQLite; no un servicio vectorial. | Un corpus representativo debe mostrar huecos repetibles de paráfrasis o vocabulario: recall de fuentes necesarias por debajo del objetivo acordado en al menos diez consultas, y un prototipo híbrido debe mejorar recall al menos 10 puntos porcentuales sin perder más de 5 puntos de precisión citada ni superar el presupuesto local de latencia. Un servicio externo requiere además demostrar límites medidos de tamaño, memoria o latencia del índice local. |
| DR-003 — Grafo en SQLite | Relaciones, CTE recursivos y límites de saltos en SQLite son la implementación primaria. No hay tarea obligatoria de base de grafos. | Reevaluar solo con mundos reales de tamaño objetivo cuando consultas necesarias de hasta tres saltos no puedan expresarse o mantengan p95 superior a 250 ms después de índices y optimización, y cuando el beneficio supere el coste de migración, transacciones y distribución. |
| DR-004 — Multiagente explícito | El perfil estándar sigue siendo el predeterminado. Multiagente solo sirve para `Revisión profunda` solicitada por el usuario, con especialistas de lectura, máximo cuatro, presupuestos estrictos y cero delegaciones anidadas. | Ampliar roles o presupuestos únicamente si la regresión demuestra cobertura adicional y no aumenta conflictos, coste o latencia fuera de los límites visibles al usuario. |
| DR-005 — Graph-aware solo al importar | El canon nativo ya tiene grafo estructurado y no se reprocesa con GraphRAG. La extracción graph-aware se limita a conectar fragmentos y candidatos durante la ingestión de lore no estructurado. | Extenderla exige un caso de importación medido; nunca autoriza canonización automática ni exige una base de grafos. |
| DR-006 — Proveedores por necesidad | Se mantiene un proveedor concreto mientras cubra consultas, salida estructurada, crítica y streaming. No existe marketplace ni abstracción anticipada. | Añadir un proveedor local o segundo proveedor solo ante una necesidad comprobada, como privacidad/offline o una capacidad estructurada ausente; entonces se reemplaza el acoplamiento concreto directamente y se actualizan todos los callers, sin shim. |

## Fase 7 — Recuperación evolutiva y snapshots del VFS

**Resultado:** la recuperación puede cerrar huecos semánticos medidos sin perder
determinismo ni citas, y el mundo puede salir y volver como snapshot explícito.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-053 | Pendiente | NIR-032, NIR-052 | Ampliar el benchmark de recuperación y ejecutar el gate semántico. | Añadir mundos de tamaño representativo y consultas de coincidencia exacta, alias, paráfrasis, vocabulario distante, perspectiva, causalidad y tiempo. Medir recall de fuentes necesarias, precisión citada, contradicciones omitidas, tokens y latencia p50/p95 para la tubería determinista. Registrar corpus, umbrales y decisión de DR-002; no añadir embeddings durante la medición base. | El benchmark es reproducible, identifica por consulta qué etapa recuperó cada fuente y concluye con evidencia si FTS5/SQL basta o si existe el hueco requerido para NIR-054. | [Recuperación](docs/architecture/retrieval.md), [Suite IA](docs/validation/ai-regression-suite.md) |
| NIR-054 | Pendiente | NIR-053 | Implementar recuperación semántica local solo si el gate la exige. | Si DR-002 se abre, almacenar chunks y embeddings como índice derivado asociado a `object_type`, `object_id`, `chunk_id`, `content_hash`, `model_id` y vector usando una extensión SQLite/in-process viable. Chunkear prosa sin separar identidad ni procedencia; no enviar lore a un servicio adicional ni crear una base vectorial separada. Si el gate no se abre, cerrar la tarea con la decisión medida y sin código preventivo. | Con gate abierto, consultas de paráfrasis recuperan las fuentes objetivo dentro del presupuesto y borrar el índice no afecta el canon; con gate cerrado, no existen tablas, traits ni dependencias semánticas no usadas. | [Recuperación](docs/architecture/retrieval.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-055 | Pendiente | NIR-054 | Integrar ranking híbrido citado e invalidación reconstruible. | Combinar anclas y resultados estructurados autoritativos con FTS5 y, cuando exista, similitud semántica mediante una fusión determinista con límites por categoría. Cada resultado conserva URI, fragmento, etapa y puntuación explicable. Invalidar por `content_hash` en la transacción que cambia contenido y ofrecer rebuild completo por modelo; un índice fallido se degrada a RAG determinista. | El benchmark de NIR-053 mejora según el gate sin ocultar contradicciones ni perder citas; cambiar texto invalida solo chunks afectados, cambiar modelo reconstruye el índice y una reconstrucción produce ranking estable dentro de tolerancia definida. | [Recuperación](docs/architecture/retrieval.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-056 | Pendiente | NIR-030, NIR-052 | Exportar el VFS lógico como snapshot portable. | Materializar en un directorio elegido `manifest.json` y Markdown agrupado por tipo con IDs, variante/revisión base, esquema, hashes y metadatos mínimos. Escribir primero un staging dentro del destino y publicar el snapshot completo o no publicar nada. No observar archivos, montar un filesystem ni crear sincronización viva. | Un mundo exportado contiene todos los objetos y referencias, conserva URI/IDs tras renombres y dos exportaciones de la misma revisión son lógicamente equivalentes; un fallo no deja un snapshot presentado como completo. | [Almacenamiento](docs/architecture/storage.md), [Runtime](docs/architecture/language-runtime.md) |
| NIR-057 | Pendiente | NIR-024, NIR-056 | Importar un snapshot editado como ChangeSet revisable. | Validar manifest, esquema, IDs, rutas confinadas, hashes y revisión base; comparar el snapshot con la base exportada y convertir altas, cambios y eliminaciones en operaciones tipadas. Tratar Markdown/frontmatter como no confiable, bloquear path traversal y referencias rotas, y nunca sobrescribir canon directamente. Una base obsoleta produce conflicto visible. | Editar externamente una entidad y un documento genera un draft con diff y fuentes; archivos desconocidos o manipulados no ejecutan contenido; confirmar usa la revisión normal y rechazar deja el canon intacto. | [Almacenamiento](docs/architecture/storage.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-058 | Pendiente | NIR-053–NIR-057 | Validar recuperación evolutiva y snapshots de extremo a extremo. | Ejecutar corpus exacto/semántico, invalidación, rebuild, degradación sin índice, exportación, edición externa, importación, stale check y rollback. Incluir un snapshot con Markdown hostil, manifest alterado y referencias renombradas. | Las métricas sustentan la tecnología activa, todas las respuestas siguen citadas, ningún índice derivado se vuelve autoridad y un round-trip de snapshot solo cambia las operaciones aprobadas. | [Recuperación](docs/architecture/retrieval.md), [Almacenamiento](docs/architecture/storage.md) |

## Fase 8 — Revisión profunda multiagente acotada

**Resultado:** el usuario puede solicitar una revisión interdisciplinaria de
solo lectura; los desacuerdos llegan a una única propuesta humana, nunca a
escrituras competidoras.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-059 | Pendiente | NIR-041, NIR-047, NIR-052 | Definir contratos del perfil profundo. | Añadir `DeepReviewRun`, `SpecialistReport` y resultados de auditoría con especialista, findings, IDs afectados, consecuencias candidatas, supuestos, evidencia, confianza y preguntas abiertas. Extender `Run.mode` con `deep_impact` y `audit`; los tipos no contienen operaciones de commit ni razonamiento privado. | JSON válido hace round-trip y salidas incompletas, sin evidencia o con operaciones directas fallan antes de síntesis; el perfil estándar no cambia de contrato. | [Grafo de agentes](docs/architecture/agent-graph.md), [Reasoning](docs/architecture/reasoning-policy.md) |
| NIR-060 | Pendiente | NIR-029, NIR-059 | Seleccionar especialistas y aplicar capacidades/presupuestos. | Elegir roles por selección explícita o reglas de dominio, por ejemplo economía para recursos, historia/política para sucesión y geografía para cambios territoriales. Limitar a cuatro especialistas, dos expansiones de contexto, seis herramientas de lectura, timeout y una salida tipada por rol. Prohibir escritura, delegación, agentes anidados y coordinador libre. | Tests demuestran selección relevante, rechazo del quinto rol y de herramientas no permitidas, y terminación por presupuesto; ninguna solicitud estándar activa revisión profunda silenciosamente. | [Grafo de agentes](docs/architecture/agent-graph.md), [Interacción](docs/architecture/interaction-model.md) |
| NIR-061 | Pendiente | NIR-040, NIR-055, NIR-059, NIR-060 | Orquestar especialistas aislados y fallos parciales. | Ejecutar especialistas en paralelo sobre el mismo `base_revision` con contextos mínimos separados. Registrar estado, coste, timeout y fuentes sin compartir respuestas entre especialistas. Permitir continuar si falla alguno; si fallan todos, terminar sin propuesta y con error visible. | Una ejecución con un timeout conserva los demás informes, cancelación detiene nuevas llamadas y ningún informe usa una revisión o fuente fuera de su snapshot. | [Grafo de agentes](docs/architecture/agent-graph.md), [Reasoning](docs/architecture/reasoning-policy.md) |
| NIR-062 | Pendiente | NIR-021, NIR-044–NIR-047, NIR-061 | Sintetizar informes sin borrar desacuerdos. | Un único sintetizador recibe informes tipados y produce `DecisionPoint`s y, para impacto, un `ChangeSetDraft` cuyas operaciones citan findings. Alternativas incompatibles permanecen separadas; una decisión pendiente bloquea confirmación. Auditorías producen `ValidationReport` y permiten iniciar una propuesta aparte. El sintetizador no aplica cambios. | Fixtures con especialistas en desacuerdo conservan ambas alternativas y evidencia; no aparecen IDs duplicados ni operaciones sin origen, y el draft atraviesa validadores, crítico y revisión estándar. | [Grafo de agentes](docs/architecture/agent-graph.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-063 | Pendiente | NIR-048, NIR-060–NIR-062 | Añadir `Revisión profunda` a la GUI. | Ofrecer acción explícita desde consulta/propuesta y auditoría, mostrar motivo, roles, presupuesto y coste estimado antes de iniciar, progreso/fallos por especialista, informes, desacuerdos y fuentes. Mantener transcript, informes y cambios pendientes diferenciados; cancelar no genera draft parcial aplicable. | El usuario elige o confirma roles, puede cancelar y resolver DecisionPoints; el botón de commit solo aparece en el panel estándar tras síntesis, validación y revisión completa. | [Interacción](docs/architecture/interaction-model.md), [Cocreación](docs/research/critical-fronts/human-ai-cocreation.md) |
| NIR-064 | Pendiente | NIR-059–NIR-063 | Crear regresión del perfil profundo. | Añadir casos de crisis de recursos, sucesión, cambio geográfico y auditorías temporal, causal, de reglas y perspectivas. Probar presupuestos, roles irrelevantes, desacuerdo, evidencia insuficiente, timeout parcial, todos fallidos, cancelación y stale revision. | Cero runs exceden límites o delegan; los desacuerdos nunca se resuelven silenciosamente, un fallo total no produce propuesta y ningún resultado profundo puede saltar el workflow NIR-047. | [Grafo de agentes](docs/architecture/agent-graph.md), [Suite IA](docs/validation/ai-regression-suite.md) |

## Fase 9 — Importación revisada de lore existente

**Resultado:** material previo se convierte en candidatos trazables y
ChangeSets revisables sin asumir que el texto importado es canon.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-065 | Pendiente | NIR-052, NIR-056 | Ingerir fuentes locales no confiables en un lote de importación. | Crear `ImportBatch` y `ImportSource` con archivo, hash, formato, tamaño, estado y revisión objetivo. Aceptar inicialmente Markdown y texto UTF-8; otros formatos fallan como no soportados hasta que un caso real justifique un extractor concreto. Nunca ejecutar HTML, enlaces, scripts ni rutas embebidas. Copiar o referenciar contenido por hash según el contrato del lote. | Archivos válidos se previsualizan con metadatos y hash; binarios, formatos no soportados, límites excedidos, path traversal y contenido ilegible fallan sin modificar canon ni abrir recursos externos. | [Runtime](docs/architecture/language-runtime.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-066 | Pendiente | NIR-065 | Segmentar fuentes con procedencia estable. | Crear chunks por estructura documental y límites de tamaño, conservando `source_id`, hash, rango de bytes/páginas, encabezados y orden. Normalizar texto solo para búsqueda; mantener el fragmento original para cita. Reprocesar una fuente invalida únicamente sus chunks y candidatos. | Cada chunk abre su ubicación original, la concatenación ordenada permite auditar la extracción y cambiar un archivo no mezcla candidatos viejos con contenido nuevo. | [Recuperación](docs/architecture/retrieval.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-067 | Pendiente | NIR-041, NIR-055, NIR-066 | Extraer candidatos con contexto graph-aware limitado a importación. | Definir DTOs propios del lote y reutilizar el parsing estructurado estándar, sin contratos de especialistas. Recuperar chunks vecinos y menciones enlazadas para proponer candidatos de Entity, Relation, Event, Claim y Rule con spans, fuentes, confianza técnica y posibles co-referencias. Usar tablas de staging y adyacencia/CTE SQLite; no escribir tablas canónicas, no reprocesar el canon con GraphRAG y no exigir base de grafos. | Un fixture multipágina enlaza aliases y relaciones con citas correctas, conserva candidatos contradictorios separados y ninguna salida del modelo aparece como objeto canónico; la importación funciona aunque `Revisión profunda` esté deshabilitada. | [Recuperación](docs/architecture/retrieval.md), [Conocimiento incierto](docs/research/critical-fronts/uncertain-knowledge.md) |
| NIR-068 | Pendiente | NIR-018–NIR-024, NIR-067 | Resolver identidad y convertir candidatos en ChangeSets. | Comparar candidatos con IDs, aliases, tiempo y relaciones existentes; presentar crear, enlazar, fusionar como candidato o descartar. Una fusión de identidad requiere decisión humana. Convertir solo selecciones confirmadas en operaciones tipadas con procedencia al chunk y pasar validadores, crítico y stale check normales. | Aceptar dos candidatos y rechazar uno produce exactamente esas operaciones; una identidad ambigua o claim canónico opuesto queda como DecisionPoint/conflicto y nunca se canoniza automáticamente. | [Validación](docs/architecture/validation-pipeline.md), [Versionado](docs/research/critical-fronts/canon-versioning.md) |
| NIR-069 | Pendiente | NIR-039, NIR-065–NIR-068 | Construir la experiencia de importación y revisión. | Mostrar lotes, fuentes, chunks, candidatos agrupados, confianza técnica separada de confianza diegética, comparación con canon, citas y acciones por lote/candidato. Permitir pausar, reanudar, cancelar y volver a extraer; el paso final abre el panel estándar de ChangeSet. No reutilizar la UI ni el orquestador multiagente: importación es un workflow independiente. | El usuario puede rastrear cada operación hasta archivo y fragmento, editar candidatos antes del draft y cancelar/eliminar el lote sin afectar canon ni documentos originales; funciona con el perfil estándar como único modo de IA activo. | [Interacción](docs/architecture/interaction-model.md), [Cocreación](docs/research/critical-fronts/human-ai-cocreation.md) |
| NIR-070 | Pendiente | NIR-065–NIR-069 | Validar importación de extremo a extremo. | Probar crónica Markdown, texto con aliases, documento paginado, fuentes contradictorias, archivo reemplazado, prompt injection, HTML hostil, macro/enlace, identidad ambigua, stale revision y rollback. Verificar limpieza de staging y reconstrucción por hash. | Solo operaciones revisadas llegan al canon, toda afirmación conserva procedencia, los ataques permanecen datos inertes y el flujo no instala ni requiere una base de grafos o servicio externo. | [Runtime](docs/architecture/language-runtime.md), [Recuperación](docs/architecture/retrieval.md) |

## Fase 10 — Variantes, comparación e historia

**Resultado:** un mundo puede mantener líneas canónicas nombradas y consultar su
historia sin introducir colaboración concurrente ni merge semántico automático.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-071 | Pendiente | NIR-022–NIR-025, NIR-052, NIR-057, NIR-070 | Extender persistencia, lecturas y workflows externos a variantes. | Migrar la cadena existente a variante `main`; añadir `variants`, cabezas explícitas y pertenencia de revisión. Mantener estado actual materializado por variante y versiones inmutables/tombstones suficientes para leer una revisión sin mover la cabeza. Introducir `ReadScope { variant_id, revision_id? }` en búsqueda, URI, contexto y consultas derivadas: sin revisión lee la cabeza; con revisión siempre es read-only. Añadir `variant_id` y cabeza base a manifests de snapshot, `ImportBatch`, drafts y sesiones; artefactos anteriores migran explícitamente a `main`. Reimportar o confirmar se permite solo sobre la misma variante/cabeza, salvo que el usuario desvíe el cambio al merge de NIR-074. Una revisión normal conserva un padre; un merge registra además la revisión fuente como procedencia. | Migración conserva IDs e historial, cada variante tiene exactamente una cabeza, todas las lecturas respetan `ReadScope`, abrir una revisión histórica no altera estado actual y ningún snapshot/import batch puede aplicarse silenciosamente sobre otra variante. | [Versionado](docs/research/critical-fronts/canon-versioning.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-072 | Pendiente | NIR-071 | Implementar ciclo de vida y commits por variante. | Crear variante desde cualquier revisión, nombrar, renombrar, cambiar variante activa y archivar; commits, stale checks y undo operan contra su cabeza. Prohibir borrar una variante con descendientes o referencias sin una decisión explícita; no permitir dos escritores concurrentes ni sincronización remota. | Dos variantes divergen sin contaminarse, reabrir conserva sus cabezas y un draft basado en otra cabeza queda obsoleto; undo en una variante no cambia la otra. | [Versionado](docs/research/critical-fronts/canon-versioning.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-073 | Pendiente | NIR-030, NIR-071, NIR-072 | Comparar variantes y revisiones por identidad estable. | Calcular altas, bajas y cambios de campos/relaciones por ID entre dos cabezas o revisiones, con before/after, retcon, procedencia y referencias afectadas. Distinguir mismo objeto modificado de objetos diferentes con igual nombre; permitir abrir ambos lados en modo lectura. | Comparaciones detectan renombre, edición, eliminación y relaciones divergentes sin falsos matches por slug; cada diferencia navega a su revisión y fuente. | [Versionado](docs/research/critical-fronts/canon-versioning.md), [Almacenamiento](docs/architecture/storage.md) |
| NIR-074 | Pendiente | NIR-019–NIR-024, NIR-073 | Implementar merge seguro limitado y resolución manual. | Traducir diferencias de la fuente a un ChangeSet sobre la cabeza destino. Auto-seleccionar solo operaciones sobre objetos/campos no solapados o conmutativas demostradas, como altas con IDs distintos o miembros independientes de un conjunto. Cualquier doble escritura, delete/update, conflicto temporal, claim opuesto o dependencia dudosa crea DecisionPoint manual; no usar CRDT ni “merge semántico” LLM automático. | Fixtures aplican automáticamente cambios independientes, bloquean conflictos solapados y registran fuente/decisiones; confirmar produce una revisión destino normal y nunca mueve ni reescribe la variante fuente. | [Versionado](docs/research/critical-fronts/canon-versioning.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-075 | Pendiente | NIR-071–NIR-074 | Añadir UI y regresión de variantes/historia. | Incorporar selector y cabeza visible, crear desde revisión, timeline editorial, vista histórica read-only, comparación y resolución de merge en el panel de cambios. Toda búsqueda, URI, contexto, timeline, snapshot e importación muestran el `ReadScope` activo; intentar editar o importar desde otra variante exige cambiar scope o abrir merge. Probar reapertura, branching, divergencia, stale, undo, merge parcial, conflicto manual y navegación histórica. | La GUI nunca confunde variante activa con revisión observada, no edita una vista histórica, no aplica artefactos a otra variante y todos los escenarios conservan cabezas, auditoría y aislamiento después de reabrir. | [Interacción](docs/architecture/interaction-model.md), [Versionado](docs/research/critical-fronts/canon-versioning.md) |

## Fase 11 — Calendario ficticio fijo de presentación

**Resultado:** cada mundo puede mostrar sus ticks con un calendario simple sin
cambiar el orden temporal canónico ni crear un lenguaje calendárico universal.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-076 | Pendiente | NIR-011, NIR-052 | Definir un modelo de calendario fijo por mundo. | Añadir opcionalmente nombre, tick de epoch, ticks por día, días por semana, nombres de weekdays y una secuencia anual de meses con longitudes fijas. Implementar conversión pura tick a fecha/fecha a tick para valores exactos, incluidos ticks negativos. No incluir leap rules, astronomía, zonas horarias, calendarios múltiples ni DSL. | Pruebas de frontera cubren cambio de día/mes/año, epoch, ticks negativos, configuración inválida y round-trip exacto; quitar el calendario no modifica eventos ni ticks. | [Tiempo narrativo](docs/research/critical-fronts/narrative-time.md), [Modelo](docs/domain/model.md) |
| NIR-077 | Pendiente | NIR-036, NIR-076 | Integrar fechas ficticias en edición, timeline y citas. | Permitir configurar el calendario como ChangeSet, mostrar tick y etiqueta convertida, ingresar una fecha exacta convertida a tick y conservar precisión/certidumbre. Eventos `unknown`, aproximados u ongoing muestran la etiqueta posible sin inventar extremos. Exportar snapshots incluye configuración y ticks canónicos. | Cambiar nombres de meses solo cambia presentación, ordenar timeline sigue usando ticks y una fecha ambigua o inválida no persiste un tick inventado. | [Tiempo narrativo](docs/research/critical-fronts/narrative-time.md), [Interacción](docs/architecture/interaction-model.md) |
| NIR-078 | Pendiente | NIR-076, NIR-077 | Validar calendario y compatibilidad histórica. | Probar mundos sin calendario, configuración posterior, variantes con calendarios distintos, revisiones históricas, export/import snapshot y fechas en respuestas citadas. | El mismo evento conserva tick e identidad en todos los casos, cada vista usa la configuración de su variante/revisión y ningún cambio de display altera validación causal. | [Tiempo narrativo](docs/research/critical-fronts/narrative-time.md), [Versionado](docs/research/critical-fronts/canon-versioning.md) |

## Fase 12 — Simulación limitada de facciones y recursos

**Resultado:** escenarios deterministas exploran consecuencias acotadas fuera
del canon y solo los resultados elegidos se convierten en propuestas.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-079 | Pendiente | NIR-010, NIR-012, NIR-052, NIR-072 | Definir escenarios y estado simulado de facciones/recursos. | Crear tipos para escenario, revisión/variante base, facciones participantes, recursos discretos, existencias, capacidad, transferencias, producción/consumo fijos, pasos y supuestos. El estado es una copia externa al canon; cantidades y reglas usan enteros/unidades declaradas. No modelar precios, mercados, combate, población o economía universal. | Un escenario serializa y valida referencias, unidades, cantidades no negativas y base; crearlo, editarlo o borrarlo no genera revisión canónica. | [Visión](docs/product/vision.md), [Modelo](docs/domain/model.md) |
| NIR-080 | Pendiente | NIR-015, NIR-079 | Implementar transiciones deterministas y ejecución inspeccionable. | Aplicar por paso reglas explícitas en orden estable para producción, consumo, transferencia y escasez; registrar before/after, regla, fuente y eventos disparados. Sin aleatoriedad, LLM en el motor, agentes continuos ni ejecución en background después de cerrar el escenario. Detener en límite de pasos o condición declarada. | La misma entrada produce byte-for-byte el mismo resultado lógico, una regla inválida detiene sin resultado parcial presentado como completo y cada delta explica la transición que lo causó. | [Validación](docs/architecture/validation-pipeline.md), [Fases](docs/roadmap/phases.md) |
| NIR-081 | Pendiente | NIR-018–NIR-024, NIR-080 | Inspeccionar resultados y promover selecciones a ChangeSet. | Mostrar serie por paso, recursos agotados, transferencias, supuestos y consecuencias candidatas. Permitir seleccionar resultados y mapearlos a eventos, relaciones, goals o claims mediante operaciones tipadas; exigir fuente al escenario y decisión humana para cualquier interpretación no mecánica. Revalidar contra cabeza vigente. | Ejecutar no cambia canon; seleccionar dos deltas produce solo sus operaciones revisables, un escenario stale exige rebase/re-ejecución y rechazar el draft conserva únicamente el escenario. | [Interacción](docs/architecture/interaction-model.md), [Validación](docs/architecture/validation-pipeline.md) |
| NIR-082 | Pendiente | NIR-079–NIR-081 | Añadir UI y regresión de simulación acotada. | Incorporar editor de escenario, ejecución paso a paso, comparación de escenarios y promoción al panel de cambios. Probar escasez, transferencia, límite de capacidad, orden estable, stale base, variante, cancelación y round-trip de resultados. | La GUI etiqueta siempre “fuera del canon”, no ofrece modo continuo y todos los casos verifican determinismo, trazabilidad y ausencia de escritura antes de confirmación estándar. | [Interacción](docs/architecture/interaction-model.md), [Fases](docs/roadmap/phases.md) |

## Fase 13 — Extracción narrativa y documentos derivados

**Resultado:** Nirmata descubre estructuras narrativas del canon y genera
artefactos acotados por perspectiva, sin confundir prosa con verdad ni prometer
una novela completa.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-083 | Pendiente | NIR-012–NIR-014, NIR-055, NIR-075 | Derivar timelines, hilos causales y cabos sueltos. | Construir consultas de solo lectura que agrupen eventos por tiempo, sigan enlaces causales con profundidad/límite y detecten heurísticas explícitas: goals activos sin resolución, eventos ongoing, claims disputados sin cierre, causas sin consecuencia registrada y referencias narrativas pendientes. Conservar variante, revisión y citas; no persistir inferencias como canon. | Fixtures producen hilos reproducibles, separan orden de discurso/story time y etiquetan cada cabo con regla heurística y evidencia, sin afirmar que la ausencia sea falsedad. | [Visión](docs/product/vision.md), [Tiempo narrativo](docs/research/critical-fronts/narrative-time.md) |
| NIR-084 | Pendiente | NIR-043, NIR-055, NIR-083 | Generar documentos internos dependientes de perspectiva. | Permitir crear crónica, carta, informe, mito o historia corta desde objetos/hilos seleccionados. Recuperar solo conocimiento accesible al autor/perspectiva y tick elegido; distinguir hechos, rumores e inferencias en las fuentes. La salida es un `Document` draft con `ContentReference`s, nunca una novela ni canon automático. | Un narrador sin acceso a un secreto no lo presenta como conocido, todas las menciones importantes tienen referencias y cancelar o fallar parsing no crea documento. | [Interacción](docs/architecture/interaction-model.md), [Conocimiento incierto](docs/research/critical-fronts/uncertain-knowledge.md) |
| NIR-085 | Pendiente | NIR-044–NIR-048, NIR-083, NIR-084 | Proponer continuidad narrativa como cambios revisables. | Desde un hilo o cabo suelto ofrecer preguntas, alternativas y consecuencias candidatas; si el usuario elige desarrollar una, producir `IntentBrief` y `ChangeSetDraft` para eventos, goals, claims o documentos. Mantener alternativas como DecisionPoints y usar revisión profunda solo por petición explícita. | Ninguna sugerencia modifica canon, las alternativas incompatibles permanecen visibles y una propuesta aceptada atraviesa validadores, crítico, revisión y commit normal con fuentes al hilo original. | [Cocreación](docs/research/critical-fronts/human-ai-cocreation.md), [Grafo de agentes](docs/architecture/agent-graph.md) |
| NIR-086 | Pendiente | NIR-083–NIR-085 | Añadir UI y regresión narrativa. | Crear vistas de timeline derivada, hilo causal y cabos sueltos con filtros por perspectiva/variante, acciones de generar documento o proponer continuidad y panel de fuentes. Probar flashback, rumor, secreto, causalidad parcial, goal resuelto, revisión histórica y salida hostil. | La UI distingue derivación, inferencia, documento y ChangeSet; todas las salidas abren fuentes, respetan perspectiva y no ofrecen una acción de “generar novela”. | [Interacción](docs/architecture/interaction-model.md), [Visión](docs/product/vision.md) |

## Fase 14 — Evolución opcional de proveedor y aceptación general

**Resultado:** una necesidad real puede habilitar otro proveedor sin convertir
la integración en plataforma, y toda la solución queda validada de extremo a
extremo.

| Código | Estado | Dependencias | Entregable / Descripción | Detalle técnico | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| NIR-087 | Pendiente | NIR-064, NIR-070, NIR-086 | Ejecutar el gate de proveedor y refactorizar solo si se abre. | Medir el proveedor inicial contra consultas, JSON tipado, crítica, contexto, streaming, privacidad/offline y capacidades requeridas por importación/narrativa. Si existe una carencia funcional aprobada, elegir un proveedor local o segundo proveedor concreto y reemplazar el acoplamiento directo por la mínima selección usada por producto, actualizando callers y eliminando la ruta anterior que quede obsoleta. No crear marketplace, plugins, factory genérica, compatibility shim ni proveedor ficticio. | El registro demuestra la necesidad o cierra el gate sin código nuevo. Si se abre, cada modo selecciona únicamente implementaciones reales, credenciales/configuración permanecen aisladas y no existen dos APIs internas equivalentes para la misma llamada. | [Runtime](docs/architecture/language-runtime.md), [AGENTS](AGENTS.md) |
| NIR-088 | Pendiente | NIR-087 | Verificar contratos y seguridad de los proveedores activos. | Ejecutar fixtures grabadas equivalentes para query, propose, critic, specialist y documento; probar timeout, cancelación, salida truncada, capability missing, selección, almacenamiento de credenciales y ausencia de lore/secrets en logs. Si solo queda un proveedor, verificar que el cliente concreto siga sin abstracción vacía. | Todos los proveedores activos respetan los mismos DTOs y fronteras de capacidad, ninguno puede escribir canon y una capacidad ausente produce error accionable sin fallback silencioso. | [Flujo IA](docs/architecture/ai-flow.md), [Suite IA](docs/validation/ai-regression-suite.md) |
| NIR-089 | Pendiente | NIR-058, NIR-064, NIR-070, NIR-075, NIR-078, NIR-082, NIR-086, NIR-088 | Ejecutar aceptación funcional end-to-end de la solución general. | En un mundo representativo recorrer fundamento, recuperación activa, snapshots, importación de lore, revisión profunda, variantes/merge, vista histórica, calendario, escenario de facciones/recursos, extracción narrativa, documento por perspectiva y propuesta final. Reabrir el archivo entre hitos y verificar auditoría, fuentes, decisiones, rollback y ausencia de escrituras automáticas. | Se cumple la Definition of Done general; las 89 tareas tienen estado verificable, cada capacidad mantiene SQLite como canon y ningún no-objetivo se introdujo como dependencia oculta. | [Visión](docs/product/vision.md), [Fases](docs/roadmap/phases.md), [AGENTS](AGENTS.md) |

## Definition of Done de la solución general

Nirmata está funcionalmente completo para este backlog únicamente cuando:

1. Se cumple íntegramente la Definition of Done del fundamento funcional.
2. La recuperación SQL/relaciones/tiempo/FTS5 se reconoce y mide como RAG
   determinista; cualquier etapa semántica existe solo por DR-002, conserva
   citas, se invalida/reconstruye y permanece derivada dentro del proyecto.
3. No se necesita una base de grafos ni vectorial separada para los tamaños y
   consultas aceptados; cualquier reevaluación futura conserva los gates
   medibles y no forma parte de estas tareas.
4. `Revisión profunda` solo se ejecuta por acción explícita, usa especialistas
   relevantes de solo lectura con límites codificados, preserva fallos y
   desacuerdos, y entrega DecisionPoints/ChangeSetDraft sin capacidad de commit.
5. Markdown, texto y documentos existentes se importan como fuentes no
   confiables con chunks y procedencia; la extracción graph-aware produce
   candidatos y todo ingreso al canon pasa por ChangeSet y revisión humana.
6. Existen variantes nombradas con cabezas explícitas, comparación, vistas
   históricas de solo lectura y merge automático únicamente para operaciones
   no solapadas o conmutativas; los demás conflictos se resuelven manualmente.
7. El tick continúa siendo la autoridad temporal y el calendario fijo por mundo
   funciona solo como conversión/presentación, incluso en variantes e historia.
8. La simulación de facciones/recursos es determinista, limitada, inspeccionable
   y externa al canon; solo resultados seleccionados generan propuestas.
9. Timelines, hilos causales, cabos sueltos y documentos por perspectiva se
   derivan con fuentes; documentos y continuidades siguen siendo artefactos o
   ChangeSets revisables y no se ofrece generación de novelas.
10. El VFS lógico se exporta e importa como snapshot explícito y seguro; no
    existe sincronización bidireccional viva.
11. Un segundo proveedor/local solo existe si el gate funcional lo justificó y
    la integración directa no dejó marketplace, shims ni abstracciones vacías.
12. Reapertura, stale checks, cancelación, fallos parciales, rollback,
    auditoría, procedencia y seguridad se verifican de extremo a extremo en
    NIR-089, sin CI/CD, estudios, release automation u otros trabajos higiénicos
    usados para declarar funcionalidad.
