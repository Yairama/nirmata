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

Cada `revision` tiene `parent_revision_id`. En el MVP existe una sola cabeza:
la cadena padre-hijo sirve para trazabilidad y control de drafts obsoletos, no
para exponer ramas.

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

Cuando los usuarios necesiten Git o editores externos, se puede materializar el
VFS:

```text
my-world/
|-- manifest.json
|-- entities/.../*.md
|-- events/.../*.md
`-- documents/.../*.md
```

Los archivos exportados pueden incluir frontmatter con IDs y metadatos. La
primera version debe tratar esto como exportacion/importacion, no como escritura
bidireccional en vivo; sincronizar DB y archivos introduce conflictos que el
MVP no necesita.

## Historial y deshacer

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
revisiones. El MVP conserva estado actual, auditoria y undo, pero no promete
consultas arbitrarias "como estaba el canon en la revision X".

Las relaciones temporales derivables se calculan en Rust y no se guardan. Un
calendario ficticio sera una futura capa de conversion `tick <-> etiqueta`.

## Indices derivados

FTS y futuros embeddings son caches reconstruibles:

- si fallan, el canon sigue intacto;
- se invalidan cuando cambia el contenido fuente;
- almacenan el hash del contenido y version del modelo;
- nunca participan como autoridad de escritura.

## Archivos grandes

Mapas, audio e imagenes no forman parte del MVP. Cuando aparezcan, deben vivir
en un directorio de recursos asociado al proyecto y ser referenciados por hash.
No es necesario inflar el archivo SQLite con binarios antes de esa necesidad.
