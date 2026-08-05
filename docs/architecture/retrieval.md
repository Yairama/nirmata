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
  -> deduplicar
  -> priorizar
  -> ajustar al presupuesto
  -> contexto citado
```

Esto ya es RAG. RAG no implica obligatoriamente embeddings ni una base
vectorial.

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
