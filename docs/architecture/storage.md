# Almacenamiento del universo

**Estado:** recomendacion consolidada.

## Decision

El canon vive en un archivo SQLite con extension `.nirmata`.

No todo el lore debe almacenarse como Markdown:

- los datos que necesitan reglas y consultas son estructurados;
- la prosa larga se almacena como Markdown;
- las rutas VFS son una proyeccion navegable, no la identidad real;
- busqueda, embeddings y vistas son indices derivados.

## Por que SQLite

- transacciones atomicas;
- archivo local portable;
- copia de seguridad simple;
- relaciones y restricciones;
- FTS5 integrado;
- madurez y disponibilidad a largo plazo;
- no requiere servidor.

## Datos estructurados

Esquema conceptual inicial:

```text
worlds
world_rules
entities
relations
events
event_participants
goals
content_references
claims
documents
change_sets
change_operations
decision_points
change_set_waivers
conversations
messages
revisions
```

Los `change_sets` guardan `base_revision`, reportes de validacion y critica. Las
excepciones intencionales se registran en `change_set_waivers`; no se eliminan
advertencias para aparentar coherencia.

`content_references` enlaza fragmentos Markdown con objetos canonicos. Permite
validar una mencion narrativa aunque no sea una relacion principal.

Campos minimos:

- `source_type` y `source_id`;
- `target_type` y `target_id`;
- `ordinal` dentro del contenido fuente.

El `ordinal` se recalcula en la misma transaccion que modifica el Markdown.

`decision_points` guarda alternativas incompatibles propuestas por
especialistas. No forma parte del commit hasta que el usuario elige una opcion.

Cada `revision` tiene `parent_revision_id`. Desde NIR-071 tambien registra la
variante que creo la revision y una revision fuente opcional de merge. Cada fila
de `variants` contiene una unica cabeza no nula; el nombre es unico por mundo.
`revision_snapshots` conserva el estado completo e inmutable necesario para
lectura historica. No es event sourcing: el canon activo sigue materializado y
cada commit escribe el snapshot resultante en su misma transaccion.

`pending_reviews` es la unica cola durable de cambios aun no confirmados. Su
clave estable es `(variant_id, review_key)` y cada fila conserva mundo, variante,
revision base, origen cerrado, tiempos y un payload JSON tipado version 1. El
payload guarda draft original y editado, seleccion por operacion, juicios,
decisiones, waivers, reglas de revalidacion, procedencia de import/merge y, para
IA, solo el resumen necesario para volver a la critica final. No guarda
credenciales, razonamiento privado ni capacidad de commit.

Crear, editar o revalidar una revision hace upsert de esa misma fila. Confirmar
aplica el `ChangeSet`, auditoria, snapshot y borrado pendiente en una sola
transaccion; cualquier fallo conserva tanto canon como revision pendiente.
Descartar borra primero la fila y solo entonces retira el estado en memoria. Al
abrir un proyecto se deserializan y revalidan todas las filas de la variante
activa; datos tipados corruptos abortan la apertura sin activar parcialmente el
mundo.

### `entities`

Campos comunes:

- `id`;
- `world_id`;
- `kind`;
- `name`;
- `slug`;
- `summary`;
- `body_md`;
- `attributes_json`;
- `created_at`;
- `updated_at`;
- `version`.

`attributes_json` permite atributos particulares sin crear una tabla por cada
tipo durante el MVP. Los campos consultados frecuentemente deben promoverse a
columnas o tablas normales cuando aparezcan casos reales.

### `relations`

- origen;
- destino;
- tipo;
- direccion;
- validez temporal opcional;
- certeza;
- fuente;
- metadatos.

Los identificadores, no los nombres ni rutas, forman las referencias.

### `events`

- tipo temporal: unknown, instant, interval u ongoing;
- ticks `i64` relativos al epoch del mundo;
- etiqueta de fecha;
- precision y certeza;
- inicio y final opcionales;
- ubicaciones;
- participantes y roles;
- objetivos afectados;
- resumen;
- cuerpo Markdown;
- causas y consecuencias mediante relaciones tipadas.

### `goals`

- holder;
- estado deseado;
- prioridad;
- estado;
- periodo de validez;
- visibilidad;
- fuente.

### `claims`

Representa una afirmacion y su perspectiva:

- sujeto;
- contenido Markdown opcional;
- forma normalizada opcional: `predicate_key` y objeto entidad o escalar;
- polaridad positiva o negativa;
- autenticacion: canonical, attributed o disputed;
- holder;
- modalidad: assertion, belief, hypothesis o counterfactual;
- registro opcional: official, rumor, myth o testimony;
- base epistemica;
- fuente;
- documento fuente y claim derivado opcionales;
- confianza declarada por el holder;
- periodo de validez.
- revision que lo registro y revision que lo sustituyo, si aplica.

No intenta convertir cada frase del mundo en una tripleta.

Solo la forma normalizada participa en conflictos deterministas. Los claims de
prosa permanecen bajo revision del critico semantico.

La confianza tecnica de una extraccion o del modelo se guarda en el
`ChangeSetDraft` o `ValidationReport`, nunca como propiedad epistemica del mundo.

## Semantica de ausencia

SQLite `NULL` representa un dato no especificado. No representa falsedad.

Cuando sea importante afirmar ausencia, se guarda un valor explicito o un
`Claim`. Esta distincion evita que Nirmata complete automaticamente los vacios
constitutivos de un mundo ficticio.

El cierre local solo se aplica a dominios conocidos como completos por el
validador: referencias, unicidad, campos monovaluados y ciclos de vida.

### `documents`

Contiene artefactos largos:

- titulo;
- tipo;
- autor o perspectiva interna;
- estado canonico;
- cuerpo Markdown;
- referencias a entidades y eventos.

Cartas, tratados, cronicas e historias viven aqui.

## Markdown

Markdown es apropiado para:

- descripciones largas;
- documentos internos del mundo;
- notas del autor;
- escenas e historias;
- resumen narrativo de eventos.

No es apropiado como unica fuente para:

- identidad;
- relaciones;
- fechas ordenables;
- restricciones;
- perspectivas;
- permisos de escritura;
- operaciones atomicas.

Guardar todo como archivos `.md` obligaria a parsear texto para reconstruir
informacion que la aplicacion ya conoce.

## VFS logico

Nirmata puede presentar el mundo como un arbol sin almacenarlo fisicamente de
esa forma:

```text
/
|-- world.md
|-- entities/
|   |-- people/
|   |-- places/
|   `-- factions/
|-- events/
|-- claims/
|-- documents/
|   |-- chronicles/
|   |-- letters/
|   `-- stories/
`-- proposals/
```

Cada entrada es una vista generada desde SQLite. Para enlaces estables se usa
un URI basado en ID:

```text
nirmata://entity/<uuid>
nirmata://event/<uuid>
nirmata://document/<uuid>
```

El nombre visible puede cambiar sin romper referencias.

No hace falta implementar una interfaz completa de sistema de archivos. Basta
un resolvedor de URI y consultas que produzcan el arbol.

## Exportacion fisica

NIR-056 materializa el VFS mediante el caso de uso explicito
`export_vfs_snapshot`. El usuario elige un directorio padre existente y un
nombre nuevo; la aplicacion no observa ni sincroniza el resultado. SQLite sigue
siendo la unica autoridad.

```text
my-world/
|-- manifest.json
|-- worlds/<world-id>.md
|-- entities/<entity-id>.md
|-- relations/<relation-id>.md
|-- events/<event-id>.md
|-- claims/<claim-id>.md
|-- rules/<rule-id>.md
|-- goals/<goal-id>.md
`-- documents/<document-id>.md
```

Los nombres visibles y slugs nunca forman rutas: cada archivo usa el UUID
estable y declara su URI `nirmata://`. `manifest.json` version 1 registra mundo,
variante `main`, revision base, version del esquema SQLite, algoritmo de hash,
metadata estructurada sin duplicar la prosa, referencias de contenido por URI,
hash SHA-256 de cada Markdown/metadata y un hash logico del conjunto. El hash
logico excluye ruta de destino y tiempo de exportacion, por lo que dos
exportaciones de la misma revision y contenido son equivalentes.

Desde NIR-071 el manifest conserva tanto nombre como ID de variante. Un
manifest version 1 anterior sin ID se interpreta explicitamente como `main`;
ningun snapshot puede generar o confirmar operaciones sobre otra variante.

La lectura de canon ocurre dentro de una transaccion de lectura SQLite. La
escritura usa solo directorios de tipo fijos y UUIDs, crea archivos con
`create_new` dentro de un staging oculto y hermano del destino, sincroniza cada
archivo y publica con un unico rename en el mismo padre. Un destino existente,
un nombre no seguro o un padre inexistente/symlink se rechaza; cualquier fallo
previo al rename elimina staging y no deja un directorio presentado como
snapshot completo.

La prosa Markdown se conserva como datos UTF-8, sin ejecutar HTML, enlaces ni
scripts. NIR-057 completa el ciclo mediante `import_vfs_snapshot`: recibe un
directorio elegido explicitamente y genera una `ManualReviewSession` almacenada,
nunca una escritura directa. Compara por ID el snapshot editado con la
representacion canonica vigente y produce operaciones tipadas de alta, cambio y
baja. Los cambios de un `Document` incluyen su lista ordenada de
`ContentReference`, no solo la cantidad de enlaces.

El importador trata todo el arbol como no confiable. Exige exactamente
`manifest.json`, los ocho directorios fijos y los archivos declarados; rechaza
entradas extra o ausentes, duplicados, symlinks, subdirectorios, rutas no
canonicas, traversal, UUID/URI/tipo inconsistentes, otro mundo o variante, una
revision base inexistente y otra version de formato o esquema. Manifest y cada
Markdown tienen limites de tamano. Los Markdown deben ser UTF-8 sin NUL y
conservar el header generado; HTML, scripts y enlaces permanecen texto y nunca
se abren ni ejecutan.

Los hashes SHA-256 describen el estado editado completo. Por eso una herramienta
externa que cambie prosa, metadata, referencias, altas o bajas debe actualizar
`content_hash`, `metadata_hash` y finalmente `logical_hash`; una edicion parcial
o un hash manipulado se rechaza. La metadata se deserializa al tipo de dominio,
se vuelve a serializar para detectar campos desconocidos y se valida con los
constructores de core. IDs y `world_id` son inmutables; versiones y timestamps
editoriales no provienen del archivo, sino que se normalizan como en una edicion
manual.

Una base distinta de la cabeza actual se presenta como `stale`, no puede
confirmarse ni rebasarse automaticamente: el usuario debe exportar e importar
un snapshot nuevo. Rechazar operaciones o descartar la sesion no cambia SQLite.
Confirmar usa la revalidacion y transaccion atomica existentes, deja auditoria
before/after y puede deshacerse con el undo lineal normal. No existe watcher,
montaje ni sincronizacion bidireccional viva.

Las revisiones pendientes se cargan solo para su variante. Cambiar de variante
no las borra: quedan ocultas hasta volver a la variante correspondiente. Si la
cabeza de esa variante avanzo, reaparecen como `stale`; snapshot mantiene su
prohibicion de rebase y los demas origenes usan la revalidacion normal. Una
revision IA recuperada vuelve deliberadamente a `AwaitingFinalCritique`, aunque
antes del cierre estuviera lista, para que ningun reporte contra otro proceso o
cabeza autorice el commit.

## Historial, variantes y deshacer

No se recomienda event sourcing completo.

Cada commit guarda:

- `ChangeSet` aceptado;
- operaciones aplicadas;
- valor anterior;
- valor posterior;
- usuario;
- fecha;
- version esperada.

Esto permite auditoria y deshacer sin reconstruir todo el mundo reproduciendo
eventos desde el origen.

El tiempo valido pertenece al mundo; el tiempo editorial se expresa mediante
revisiones. `ReadScope` resuelve una cabeza de variante o un snapshot historico
ancestro. Busqueda, URI, contexto, timeline y VFS usan ese mismo scope. Una vista
historica no puede crear drafts, importar ni confirmar. El undo crea un
ChangeSet inverso sobre la variante activa y no considera commits heredados de
otra variante.

Las relaciones temporales derivables se calculan en Rust y no se guardan. Un
calendario ficticio sera una futura capa de conversion `tick <-> etiqueta`.

## Staging de importacion de lore

NIR-065–NIR-070 agregan cuatro tablas no canonicas dentro del mismo archivo
SQLite: `import_batches`, `import_sources`, `import_chunks` e
`import_candidates`. No son una segunda ruta de persistencia del mundo. Un lote
solo conserva material externo copiado como UTF-8 inerte, hashes, rangos y
candidatos; entrar al canon sigue requiriendo el `ChangeSet` y la transaccion de
NIR-047.

Los lotes se consultan por mundo y fecha mediante una operacion de dominio
acotada. Por eso cerrar y reabrir el proyecto permite reanudar fuentes,
candidatos y decisiones desde SQLite; la interfaz no conserva un segundo estado
volatil ni clasifica el staging persistido como trabajo efimero.

La seleccion recibe una raiz absoluta elegida por el usuario y archivos
absolutos confinados debajo de ella. Se rechazan symlinks, traversal, archivos
no regulares, extensiones distintas de `.md`, `.markdown` y `.txt`, UTF-8
invalido, controles binarios y fuentes mayores de 1 MiB. Todas las fuentes se
leen y validan antes de insertar el lote. El contenido copiado y el SHA-256 son
la evidencia estable aunque el original desaparezca; la interfaz informa por
separado si el archivo actual todavia coincide.

Los chunks cubren rangos contiguos de bytes originales, sin normalizar la cita.
Conservan ordinal, lineas y encabezado Markdown, y sus IDs dependen de
`source_id`, hash, ordinal y rango. Reemplazar una fuente borra en la misma
transaccion sus chunks y candidatos antes de insertar la nueva generacion. Una
constraint de la escritura candidata exige el hash actual de la fuente, por lo
que no pueden mezclarse generaciones.

La traza candidato-operacion-chunks se persiste junto al ChangeSet confirmado y
los audits usan fuente `lore_import`. Campos de dominio con procedencia propia,
como `Claim.source`, `Rule.source` y `Relation.source_reference`, reciben ademas
un URI `import://<batch>/<source>/<chunk>?hash=<sha256>`. Borrar el staging no
borra la auditoria ni el canon confirmado. Descartar o borrar antes de confirmar
elimina la revision pendiente y no toca originales ni tablas canonicas.

## Indices derivados

FTS y futuros embeddings son caches reconstruibles:

- si fallan, el canon sigue intacto;
- se invalidan cuando cambia el contenido fuente;
- almacenan el hash del contenido y version del modelo;
- nunca participan como autoridad de escritura.

El prototipo semantico de NIR-054 no necesita persistencia: proyecta texto
canonico en chunks acotados y los compara en memoria con un vocabulario WordNet
offline. Por ello no cambia la version del esquema ni agrega tablas. Borrar las
filas de FTS5 o perder cualquier estado en memoria no altera canon; FTS5 se
reconstruye desde las tablas fuente y la representacion semantica se recalcula
en la siguiente consulta.

NIR-055 mantiene deliberadamente esa representacion sin cache de contenido. El
modelo compilado tiene ID `wordnet-en-offline` y version `1`; cada busqueda lee
el texto canonico vigente, por lo que update y delete se observan de inmediato.
No se almacena un `content_hash` ficticio cuando no existe un derivado semantico
persistido que invalidar. Un rebuild completo reconstruye `canon_fts`; la ruta
WordNet se recalcula en la proxima consulta y un cambio de modelo no puede
heredar estado de otra version. Si esa ruta falla, SQL y FTS5 siguen disponibles
y ninguna transaccion canonica depende del resultado semantico.

## Archivos grandes

Mapas, audio e imagenes no forman parte del MVP. Cuando aparezcan, deben vivir
en un directorio de recursos asociado al proyecto y ser referenciados por hash.
No es necesario inflar el archivo SQLite con binarios antes de esa necesidad.
