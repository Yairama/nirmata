# Recuperacion de contexto y RAG

**Estado:** recomendacion consolidada.

## Decision

El MVP usa recuperacion hibrida determinista:

```text
seleccion actual
  + SQL estructurado
  + relaciones cercanas
  + ventana temporal
  + objetivos e intenciones
  + perspectiva epistemica
  + FTS5
  + WordNet local
  -> deduplicar
  -> priorizar
  -> ajustar al presupuesto
  -> contexto citado
```

Esto ya es RAG. RAG no implica obligatoriamente embeddings ni una base
vectorial.

Desde NIR-071 toda etapa recibe un `ReadScope`. La cabeza de la variante activa
usa la proyeccion materializada; otra cabeza o una revision explicita lee su
snapshot inmutable. Anclas, SQL equivalente sobre snapshot, texto, URI,
contexto y VFS nunca mezclan scopes. La revision historica permanece de solo
lectura y sus resultados conservan variante y revision para navegacion.

## Orden de recuperacion

### 1. Anclas explicitas

Primero se incluyen los objetos que el usuario tiene abiertos, seleccionados o
mencionados por ID. Tienen mas autoridad que una coincidencia semantica.

### 2. Datos estructurados

Se consultan:

- campos de las entidades;
- relaciones directas;
- eventos asociados;
- claims relevantes;
- goals relevantes;
- leyes del mundo aplicables.

### 3. Vecindad del grafo

Las relaciones forman un grafo de adyacencia dentro de SQLite. Consultas SQL y
CTE recursivos cubren recorridos de uno o pocos saltos.

Debe existir un limite de profundidad y cantidad. Recuperar todo el componente
conectado produciria ruido.

### 4. Tiempo

Para un evento se agregan antecedentes y consecuencias cercanas dentro de una
ventana ajustada al tipo de pregunta.

### 5. Modelo de situacion

El paquete intenta cubrir, cuando existan datos:

- tiempo;
- espacio;
- protagonistas o entidades;
- causalidad;
- intencionalidad.

Una dimension ausente permanece como vacio. No se completa silenciosamente.

El sistema opera con mundo abierto por defecto. Solo responde como conjunto
exhaustivo cuando el dominio consultado tiene una regla de completitud
codificada.

### 6. Texto completo

FTS5 busca nombres, frases y documentos. Sirve para:

- referencias no enlazadas;
- documentos largos;
- sinonimos controlados;
- notas e historias.

En store vive dentro del mismo archivo SQLite como un indice derivado y
reconstruible desde las tablas canonicas. Sus escrituras viajan en la misma
transaccion que cambia canon.

### 7. Ensamblaje

El contexto final:

- elimina duplicados;
- prioriza canon sobre rumores salvo que la pregunta pida perspectivas;
- conserva por separado los mundos de conocimiento, creencia, deseo y
  obligacion;
- favorece eventos causalmente conectados sobre coincidencias aisladas;
- conserva procedencia;
- limita tokens por categoria;
- incluye resumen y fragmentos, no documentos completos por defecto.

## Planificador

El MVP no necesita que otro LLM decida como buscar. Reglas simples por tipo de
solicitud son suficientes:

| Solicitud | Recuperacion |
|---|---|
| Pregunta sobre entidad | entidad, vecinos, claims, FTS |
| Impacto de evento | participantes, causas, recursos, ventana temporal |
| Contradicciones | objeto, reglas, claims opuestos, goals, eventos solapados |
| Crear documento | perspectiva, acceso epistemico, hechos conocidos, estilo y periodo |

Un planificador LLM se justifica solo si las reglas se vuelven inmanejables.

## Embeddings

No se agregan al inicio. Deben incorporarse cuando una evaluacion muestre
preguntas relevantes que FTS no recupera por diferencias de vocabulario.

NIR-054 probo primero una representacion semantica mas pequena: expansion
lexica offline con WordNet, lematizacion acotada y chunks en memoria de hasta
800 caracteres. Lee las filas canonicas del mundo abierto, calcula un score
entero explicable y conserva el `ObjectRef` y fragmento originales. No crea una
tabla, no descarga un modelo, no llama a un proveedor y no participa en ninguna
escritura canonica. El vocabulario proviene del recurso general de WordNet, no
del corpus de evaluacion.

Sobre `nir-053-v1` recupero 3 de 12 parafrasis, frente a 0 de 12 de la linea
base, con 100 % de precision citada y p95 agregado de 2,340 ms. La mejora de 25
puntos supera DR-002, por lo que el prototipo se conserva. Sigue siendo una ruta
explicita. NIR-055 la integra despues de las etapas autoritativas y FTS5 sin
cambiar su autoridad ni permitirle ocultar contradicciones.

La opcion minima es `sqlite-vec` dentro del mismo archivo:

```text
embeddings
- object_type
- object_id
- chunk_id
- content_hash
- model_id
- vector
```

Los embeddings son un indice derivado. Cambiar texto invalida sus filas.

## Base vectorial separada

LanceDB o Qdrant solo se justifican con:

- millones de fragmentos;
- busqueda multimodal importante;
- indexacion distribuida;
- sincronizacion entre dispositivos;
- limites medidos de SQLite.

Nirmata no tiene esos requisitos en su primera escala.

## GraphRAG

GraphRAG extrae entidades y relaciones desde texto no estructurado para crear
un grafo consultable. Nirmata ya captura esas relaciones de forma estructurada,
por lo que ejecutar GraphRAG sobre su propio canon reconstruiria con coste y
errores un grafo que ya existe.

Su uso razonable aparece en una futura importacion:

```text
novela existente
  -> segmentacion
  -> extraccion GraphRAG
  -> candidatos de entidad/relacion
  -> revision humana
  -> canon
```

La salida sigue siendo una propuesta, nunca una importacion automatica.

NIR-067 materializa ese unico uso. Por cada chunk foco, un CTE recursivo
acotado carga el ordinal anterior, el foco y el siguiente de la misma fuente y
hash. El proveedor concreto estandar recibe esos DTOs inertes mediante el
contrato cerrado `import_extraction_v1`; no recibe tools, SQL, rutas de commit ni
contratos de especialistas. Cada candidato Entity, Relation, Event, Claim o Rule
debe citar `chunk_id`, `source_id`, hash y excerpt literal. App vuelve a comprobar
que el excerpt pertenece al chunk vigente antes de guardar staging.

La resolucion graph-aware se limita al lote: nombres y aliases candidatos forman
un indice local para enlazar extremos de relaciones entre chunks. Claves de
contradiccion agrupan evidencia opuesta sin fusionarla. El canon estructurado no
se reprocesa, no hay tabla de grafo, embedding, servicio externo ni dependencia
de Revision profunda. La comparacion con canon ocurre despues como resolucion de
identidad explicita y solo las selecciones humanas producen operaciones tipadas.

## Base de grafos

No se recomienda una base de grafos separada.

SQLite es suficiente mientras:

- la mayoria de consultas recorra pocos saltos;
- el volumen sea local;
- no se ejecuten algoritmos masivos de centralidad o comunidades;
- las transacciones de canon sean mas importantes que analitica especializada.

Se reevalua cuando consultas reales no puedan expresarse o rendir
razonablemente con SQL. No antes.

## Calidad

La recuperacion debe evaluarse con un conjunto pequeno de preguntas esperadas:

- respuesta correcta;
- fuentes necesarias recuperadas;
- fuentes irrelevantes incluidas;
- contradicciones omitidas;
- tokens utilizados.

Sin esta evaluacion no existe evidencia para agregar vectores, rerankers o un
framework RAG.

La linea base reproducible de NIR-053 se registra en
[`../validation/retrieval-benchmark.md`](../validation/retrieval-benchmark.md).
El objetivo acordado es 90 % de recall de fuentes necesarias por consulta. Diez
o mas consultas de parafrasis por debajo de ese objetivo abren NIR-054 solo para
construir y medir un prototipo local. Activarlo en producto exige ademas mejorar
el recall al menos 10 puntos porcentuales, perder como maximo 5 puntos de
precision citada y mantener p95 local no mayor de 250 ms. Si falla cualquiera
de esas condiciones, el prototipo se elimina y FTS5/SQL sigue siendo la unica
ruta activa.

La medicion `nir-053-v1` encontro 12 de 12 parafrasis afectadas (0 % de recall),
frente a 100 % en las 22 consultas estructuradas o de vocabulario exacto. No
hubo citas irrelevantes ni contradicciones omitidas y el p95 local agregado fue
1,838 ms. Por tanto NIR-054 queda justificado como prototipo; esta conclusion no
incorpora todavia embeddings al producto.

La comparacion de NIR-054 sobre el mismo corpus mejoro el recall de parafrasis a
25 % (3/12), mantuvo 100 % de precision citada y midio 2,340 ms de p95 local
agregado. La ruta WordNet cumple el gate para permanecer, sin tablas semanticas
ni integracion silenciosa en la tuberia vigente.

NIR-055 integra esa ruta en `search_structured`, `search_world` y el ensamblaje
de contexto. La fusion es determinista y no usa un reranker opaco:

1. anclas, alias, relaciones, tiempo, goals y perspectivas se agregan primero;
2. FTS5 aporta coincidencias exactas con prioridad inferior;
3. WordNet aporta como maximo ocho coincidencias semanticas por consulta;
4. un `ObjectRef` ya recuperado por una etapa autoritativa no puede ser
   reemplazado por evidencia lexical;
5. score entero, rank, etapa, URI, fragmento, procedencia y explicacion viajan
   con cada resultado.

Las bandas de score solo expresan este orden, no probabilidad: seleccion
`100000`, contexto autoritativo `90000..60000`, FTS5 `30000` y WordNet
`10000 + matched_bps`. Los empates se resuelven por tipo e ID. Por ello una
coincidencia semantica no desplaza anclas ni relatos contradictorios ya
recuperados. `search_structured_fts` conserva la ruta SQL/FTS determinista para
medir la linea base y como degradacion explicita.

El modelo semantico estable es `wordnet-en-offline` version `1`. No existe un
indice semantico persistido ni cache de contenido: cada consulta lee las filas
canonicas actuales y vuelve a calcular tokens y chunks. Una actualizacion o
eliminacion se refleja en la consulta siguiente sin invalidacion diferida. El
rebuild completo reconstruye FTS5 desde canon; la parte semantica no tiene
estado que reconstruir y se recalcula en la siguiente consulta. Cambiar el ID o
version compilada crea un proceso con el nuevo recurso y, al no existir estado
semantico persistido, no puede reutilizar derivados de la version anterior.

Si WordNet o su lectura derivada falla, la fusion descarta solo esa rama y
devuelve el mismo resultado SQL/FTS. El error nunca cambia canon. Persistir un
cache futuro exigiria `content_hash` por objeto, invalidacion solo del objeto
modificado y limpieza completa al cambiar modelo; NIR-055 no agrega ese cache
porque no hay trabajo reutilizado que justifique su estado.

La integracion activa conserva sobre `nir-053-v1` el mismo 25 % de recall de
parafrasis, 100 % de recall no-parafrasis, 100 % de precision citada y 2/2
contradicciones. La ejecucion verificada midio 3,517 ms de p95 local agregado,
bajo el limite de 250 ms.

## Contrato de respuesta

Toda respuesta debe distinguir:

- `hecho`: existe evidencia canonica;
- `perspectiva`: una fuente interna lo afirma;
- `inferencia`: el modelo deduce a partir de hechos;
- `sin evidencia`: el canon no permite responder.
- `no especificado`: el mundo dejo ese aspecto abierto.

Esta distincion es mas importante que el algoritmo de recuperacion.

Una inferencia por partida minima puede ayudar a contestar, pero debe quedar
etiquetada y nunca guardarse como canon automaticamente.

## Mapeo de respuestas

| Respuesta | Fuente |
|---|---|
| `hecho` | dato estructurado o `Claim.authentication=canonical` |
| `perspectiva` | `Claim` attributed o disputed |
| `inferencia` | conclusion producida para la consulta, no persistida |
| `sin evidencia` | la recuperacion no aporta soporte suficiente |
| `no especificado` | el modelo del mundo conserva explicitamente un vacio |

`sin evidencia` describe una limitacion de la consulta; `no especificado`
describe una indeterminacion del storyworld.
