# Versionado del canon

**Estado:** modelo minimo recomendado.

## Problema

Nirmata necesita corregir, reinterpretar y reemplazar canon sin perder
trazabilidad. Eso no implica convertir cada lectura en un replay de eventos ni
construir Git dentro de la aplicacion.

## Modelo elegido

### Revisiones lineales

Cada `ChangeSet` confirmado crea una revision inmutable:

```text
Revision
- id
- parent_revision_id
- created_at
- author
- summary
- change_set_id
```

En el fundamento inicial:

- una revision tiene cero o un padre;
- el proyecto tiene una sola cabeza;
- no hay ramas ni merges;
- `parent_revision_id` permite detectar bases obsoletas y deja abierta una
  evolucion futura sin disenarla ahora.

SQLite conserva el estado actual y un registro reversible de operaciones. No se
reconstruye el mundo completo reproduciendo eventos.

### Variantes e historia materializada

NIR-071–NIR-075 extienden ese modelo sin convertirlo en un DAG general. Cada
`Variant` tiene ID estable, nombre unico por mundo y exactamente una cabeza. La
variante `main` recibe toda cadena anterior durante la migracion. Una revision
normal sigue teniendo un solo padre y pertenece a la variante que la creo; una
revision de merge solo registra la revision fuente como procedencia adicional.

`ReadScope { variant_id, revision_id? }` separa lectura de escritura:

- sin `revision_id`, lee la cabeza nombrada;
- con `revision_id`, exige que la revision sea ancestro de esa cabeza y siempre
  es de solo lectura;
- la escritura solo usa la cabeza de la variante activa, aunque la GUI observe
  otra revision;
- cambiar variante rematerializa atomicamente el snapshot de su cabeza, no crea
  una revision y no mueve ninguna otra cabeza.

SQLite conserva una proyeccion canonica materializada para la variante activa y
un snapshot inmutable completo por revision. La migracion reconstruye una vez
los snapshots historicos desde los `before` auditados; las lecturas normales no
reproducen eventos. IDs, tombstones por ausencia y valores completos permiten
comparar y navegar historia sin depender de slug o ruta.

Crear una variante apunta a cualquier revision existente. Renombrar no cambia
su ID. Archivar la activa se rechaza; descendientes o lotes asociados requieren
confirmacion explicita. Drafts, snapshots y lotes de importacion conservan
variante y revision base, por lo que coincidir solo en revision no autoriza una
aplicacion cruzada. Undo considera unicamente commits creados por la variante
activa.

La comparacion usa `ObjectRef`/ID. Clasifica alta, baja, renombre, edicion y
divergencia de relacion, y cada lado conserva scope y URI de revision. El merge
limitado calcula cambios desde el ancestro comun: solo preselecciona objetos
independientes o dependencias satisfechas. Una doble escritura, delete/update o
dependencia ausente produce un `DecisionPoint` citado y pendiente. Confirmar
crea un `ChangeSet` normal en destino; la fuente no se mueve ni se reescribe.

### Taxonomia de retcon

La taxonomia es una decision de producto, no un estandar academico:

| Tipo | Significado | Tratamiento |
|---|---|---|
| `additive` | Completa un vacio sin cambiar lo autentificado | Revision normal |
| `reinterpretive` | Agrega una perspectiva o nueva lectura | Conserva las afirmaciones previas |
| `replacement` | Invalida o sustituye canon vigente | Conflicto explicito y decision humana |

Un reemplazo debe citar los objetos afectados y explicar por que el cambio es
intencional. No se borra silenciosamente el historial.

### Procedencia relacional

Nirmata adopta el vocabulario conceptual de PROV:

- entidad: afirmacion, documento, evento o regla;
- actividad: generacion, importacion, validacion o aprobacion;
- agente: usuario, modelo o proceso.

Se implementa con foreign keys y metadatos tipados, no con RDF.

### Dos tiempos distintos

- **Tiempo valido:** cuando algo ocurre o aplica dentro del mundo.
- **Tiempo de transaccion:** en que revision entro, cambio o salio del canon.

El fundamento registra ambos conceptos. NIR-071 agrega consultas historicas
explicitas mediante snapshots inmutables; nunca cambia tiempo valido ni mueve
una cabeza para observar el pasado.

## Alternativas rechazadas

### Event sourcing completo

Complica migraciones, consultas, recuperacion y depuracion. El historial de
`ChangeSet` ya cubre auditoria y undo.

### CRDT

Resuelve edicion concurrente distribuida. El MVP es local y de un usuario.

### Merge semantico automatico

Las variantes narrativas ya existen como cabezas nombradas y lineales. Se sigue
rechazando un merge semantico automatico: solo operaciones independientes y no
solapadas se preseleccionan; toda interpretacion queda en decision humana.

### TEI para todo el canon

TEI es valioso para variantes textuales y ediciones criticas de documentos, no
como modelo principal de entidades, reglas y eventos.

## Claim y atribucion

Separar una proposicion abstracta de cada acto de atribucion evitaria
duplicacion, pero agrega joins, identidad semantica y normalizacion. El MVP
mantiene una fila `Claim` por afirmacion contextualizada. Se reconsidera solo si
la duplicacion real dificulta la procedencia o la edicion.

## Fuentes seleccionadas

- W3C, *PROV-DM: The PROV Data Model*:
  <https://www.w3.org/TR/prov-dm/>
- Moreau y Missier, *PROV-DM: The PROV Data Model* (2013):
  <https://www.w3.org/TR/2013/REC-prov-dm-20130430/>
- Jensen et al., *The Consensus Glossary of Temporal Database Concepts*:
  <https://doi.org/10.1145/140979.140996>
- Jensen y Snodgrass, *Temporal Data Management*:
  <https://doi.org/10.1109/69.755613>
- Noy y Musen, *The PROMPT Suite*:
  <https://doi.org/10.1007/s10115-003-0137-2>
- Vrandecic y Krotzsch, *Wikidata*:
  <https://doi.org/10.1145/2629489>
- Mimram y Di Giusto, *A Categorical Theory of Patches*:
  <https://doi.org/10.1016/j.entcs.2013.09.018>
- TEI Consortium, *Critical Apparatus*:
  <https://tei-c.org/release/doc/tei-p5-doc/en/html/TC.html>

## Limite de la evidencia

La literatura respalda procedencia, temporalidad de datos y manejo de cambios,
pero no ofrece una taxonomia universal de retcons para storyworlds. Esa parte
debe validarse con autores y casos reales de Nirmata.
