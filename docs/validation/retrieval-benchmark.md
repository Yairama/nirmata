# Benchmark de recuperación NIR-053

**Estado:** línea base verificada el 7 de agosto de 2026.

## Decisión

El gate de entrada de NIR-054 queda **abierto para construir y medir un
prototipo híbrido local**. La línea base determinista no recuperó ninguna de las
12 fuentes requeridas por las consultas de paráfrasis y vocabulario distante:
0 % de recall frente al objetivo acordado de 90 % por consulta. Las 12
consultas afectadas superan el mínimo de 10 de DR-002.

Esto no activa todavía recuperación semántica en producto. NIR-054 debe cerrar
el gate comparativo de DR-002 demostrando sobre este mismo corpus al menos 10
puntos porcentuales de mejora de recall, una pérdida máxima de 5 puntos de
precisión citada y p95 local no mayor de 250 ms. Si el prototipo no cumple las
tres condiciones, el gate de activación se cierra y se elimina el prototipo.
NIR-053 no agrega embeddings, tablas, traits ni dependencias semánticas.

## Corpus reproducible

La fuente versionada es
[`crates/nirmata-app/tests/fixtures/retrieval_benchmark.json`](../../crates/nirmata-app/tests/fixtures/retrieval_benchmark.json)
y el ejecutor está en
[`crates/nirmata-app/tests/retrieval_benchmark.rs`](../../crates/nirmata-app/tests/retrieval_benchmark.rs).
El corpus `nir-053-v1` crea desde cero dos archivos SQLite temporales:

- un mundo estructural conectado con 42 objetos: entidades, alias, relación,
  regla, goals, eventos causales y temporales, claims opuestos, documentos y 20
  distractores;
- un mundo léxico con 12 documentos objetivo y 36 distractores;
- 34 consultas: 10 para atribución de etapas, 12 controles FTS5 de vocabulario
  exacto y sus 12 paráfrasis emparejadas;
- 90 objetos de canon en total, sin contar las dos filas `World`.

Cada ejecución reconstruye ambos mundos y realiza una pasada de calentamiento
más nueve muestras locales por consulta. Las fuentes se comparan por
`ObjectRef`; los nombres estables de la tabla son etiquetas del corpus, no IDs
generados. El test exige resultados idénticos entre muestras.

## Umbrales

| Métrica | Umbral ejecutable |
|---|---:|
| Recall de fuentes necesarias por consulta | 90 % |
| Consultas de paráfrasis bajo el objetivo para justificar prototipo | al menos 10 |
| Precisión citada de la línea base | al menos 95 % |
| Contradicciones requeridas omitidas | 0 |
| Presupuesto local p95 por consulta y agregado | 250 ms |

`recall = fuentes requeridas recuperadas / fuentes requeridas` y
`precisión citada = citas relevantes / citas recuperadas`. Una consulta sin
citas tiene precisión `n/a`, no 100 %. Los caracteres de cita son la medida
determinista de presupuesto disponible en el contrato actual; `~tokens` usa
`ceil(caracteres / 4)` solo como proxy reproducible, no como tokenización de un
proveedor.

La latencia se comprueba con un límite holgado, no contra los valores exactos
de esta tabla. Así una regresión funcional o un p95 mayor de 250 ms falla,
pero variaciones normales del scheduler no convierten los microsegundos
medidos en golden snapshots.

## Resultado medido

Comando:

```powershell
cargo test -p nirmata-app --test retrieval_benchmark -- --nocapture
```

Entorno: Windows, perfil Cargo `test` sin optimizar, SQLite local, workspace del
7 de agosto de 2026. Los valores de latencia son la medición registrada; las
fuentes, etapas y métricas de calidad son assertions deterministas.

| Consulta | Familia | Requeridas | Recuperadas | Irrelevantes | Contradicción | Etapa/procedencia | Recall | Precisión | Caracteres (~tokens) | p50/p95/limite ms |
|---|---|---|---|---|---|---|---:|---:|---:|---:|
| `anchor-01` | ancla explícita | caldris-mine | caldris-mine | ninguna | n/a | selection/anchor | 100 % | 100 % | 38 (~10) | 0.091/0.137/250 |
| `sql-type-01` | SQL estructurado | mine-ledger-rule | mine-ledger-rule | ninguna | n/a | type/type | 100 % | 100 % | 54 (~14) | 0.039/0.055/250 |
| `sql-alias-01` | SQL estructurado | caldris-mine | caldris-mine | ninguna | n/a | alias/alias | 100 % | 100 % | 38 (~10) | 0.062/0.076/250 |
| `relations-01` | relaciones | empire-controls-mine | empire-controls-mine | ninguna | n/a | relation/relation | 100 % | 100 % | 25 (~7) | 1.852/2.131/250 |
| `relations-02` | relaciones | sabotage, rationing | sabotage, rationing | ninguna | n/a | neighbor/neighbor | 100 % | 100 % | 87 (~22) | 0.108/0.169/250 |
| `time-01` | tiempo | sabotage | sabotage | ninguna | n/a | temporal/tick | 100 % | 100 % | 38 (~10) | 0.225/0.270/250 |
| `goals-01` | goals | restore-supply-goal, collapse, rationing | restore-supply-goal, collapse, rationing | ninguna | n/a | goal/goal | 100 % | 100 % | 106 (~27) | 0.175/0.201/250 |
| `perspectives-01` | perspectivas | sera-claim, sera-journal | sera-claim, sera-journal | ninguna | n/a | perspective/perspective | 100 % | 100 % | 131 (~33) | 0.323/0.354/250 |
| `contradictions-01` | perspectivas | sera-claim, orun-claim | sera-claim, orun-claim | ninguna | preservada | relation/claim | 100 % | 100 % | 155 (~39) | 5.103/5.355/250 |
| `fts5-01` | FTS5 exacto | vaultglass, vaultglass-registry | vaultglass, vaultglass-registry | ninguna | n/a | text/fts5 | 100 % | 100 % | 33 (~9) | 0.440/0.500/250 |
| `lex-01-exact` | FTS5 exacto | lex-01.source | lex-01.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 55 (~14) | 0.554/0.587/250 |
| `lex-01-paraphrase` | paráfrasis | lex-01.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.455/0.480/250 |
| `lex-02-exact` | FTS5 exacto | lex-02.source | lex-02.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 49 (~13) | 0.546/0.568/250 |
| `lex-02-paraphrase` | paráfrasis | lex-02.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.448/0.469/250 |
| `lex-03-exact` | FTS5 exacto | lex-03.source | lex-03.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 47 (~12) | 0.572/0.606/250 |
| `lex-03-paraphrase` | paráfrasis | lex-03.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.461/0.511/250 |
| `lex-04-exact` | FTS5 exacto | lex-04.source | lex-04.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 48 (~12) | 0.580/0.614/250 |
| `lex-04-paraphrase` | paráfrasis | lex-04.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.445/0.477/250 |
| `lex-05-exact` | FTS5 exacto | lex-05.source | lex-05.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 41 (~11) | 0.565/0.593/250 |
| `lex-05-paraphrase` | paráfrasis | lex-05.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.460/0.470/250 |
| `lex-06-exact` | FTS5 exacto | lex-06.source | lex-06.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 43 (~11) | 0.566/0.583/250 |
| `lex-06-paraphrase` | paráfrasis | lex-06.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.447/0.536/250 |
| `lex-07-exact` | FTS5 exacto | lex-07.source | lex-07.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 47 (~12) | 0.572/0.608/250 |
| `lex-07-paraphrase` | paráfrasis | lex-07.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.442/0.468/250 |
| `lex-08-exact` | FTS5 exacto | lex-08.source | lex-08.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 45 (~12) | 0.559/0.582/250 |
| `lex-08-paraphrase` | paráfrasis | lex-08.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.438/0.467/250 |
| `lex-09-exact` | FTS5 exacto | lex-09.source | lex-09.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 48 (~12) | 0.563/0.592/250 |
| `lex-09-paraphrase` | paráfrasis | lex-09.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.454/0.482/250 |
| `lex-10-exact` | FTS5 exacto | lex-10.source | lex-10.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 43 (~11) | 0.568/0.583/250 |
| `lex-10-paraphrase` | paráfrasis | lex-10.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.438/0.472/250 |
| `lex-11-exact` | FTS5 exacto | lex-11.source | lex-11.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 50 (~13) | 0.563/0.589/250 |
| `lex-11-paraphrase` | paráfrasis | lex-11.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.444/0.491/250 |
| `lex-12-exact` | FTS5 exacto | lex-12.source | lex-12.source | ninguna | n/a | text/fts5 | 100 % | 100 % | 39 (~10) | 0.567/0.611/250 |
| `lex-12-paraphrase` | paráfrasis | lex-12.source | ninguna | ninguna | n/a | fts5/sin hit | 0 % | n/a | 0 (~0) | 0.443/0.467/250 |

Resumen medido:

| Métrica | Resultado |
|---|---:|
| Consultas | 34 |
| Fuentes necesarias recuperadas | 28/40 |
| Recall global | 70 % |
| Recall sin paráfrasis | 100 % (28/28) |
| Recall de paráfrasis | 0 % (0/12) |
| Paráfrasis bajo objetivo | 12/12 |
| Citas irrelevantes | 0 |
| Precisión citada | 100 % (28/28) |
| Contradicciones requeridas preservadas | 2/2 |
| Caracteres citados | 1260 (~315 tokens) |
| Latencia local agregada p50/p95 | 0.464/1.838 ms |
| Presupuesto local p95 | 250 ms |

## Assertions de regresión

El test falla si cambia cualquiera de estos hechos:

- una fuente requerida exacta o estructurada deja de recuperarse;
- una paráfrasis deja de reproducir la línea base sin que se actualice
  explícitamente la comparación de NIR-054;
- aparece una cita irrelevante o duplicada;
- se omite uno de los dos claims contradictorios;
- una fuente cambia de etapa o familia de procedencia;
- alguna de las nueve muestras de una consulta produce fuentes distintas;
- una consulta o el agregado supera 250 ms p95 local;
- el corpus deja de tener 34 consultas, 12 paráfrasis afectadas o los umbrales
  versionados.

La prueba de NIR-054 conserva esta línea base y ejecuta el prototipo contra las
mismas etiquetas en un test separado.

## Comparación NIR-054

**Estado:** gate comparativo superado el 7 de agosto de 2026.

El prototipo usa WordNet offline como recurso léxico general. Normaliza tokens,
aplica lematización inglesa acotada, divide la prosa canónica en chunks de hasta
800 caracteres y exige coincidencia semántica para al menos la mitad de los
conceptos consultados. Ordena con enteros por score, tipo e ID. No contiene una
tabla de sinónimos del fixture, no descarga modelos, no llama a proveedores y no
persiste un índice semántico.

Comandos medidos:

```powershell
cargo test -p nirmata-app --test retrieval_benchmark -- --nocapture
cargo nextest run --workspace
```

Las etapas estructuradas y exactas siguen la ruta base. Solo las 12 paráfrasis
ejecutan la ruta explícita del prototipo:

| Consulta | Fuente recuperada | Recall | Citas irrelevantes |
|---|---|---:|---:|
| `lex-01-paraphrase` | ninguna | 0 % | 0 |
| `lex-02-paraphrase` | ninguna | 0 % | 0 |
| `lex-03-paraphrase` | ninguna | 0 % | 0 |
| `lex-04-paraphrase` | ninguna | 0 % | 0 |
| `lex-05-paraphrase` | `lex-05.source` | 100 % | 0 |
| `lex-06-paraphrase` | ninguna | 0 % | 0 |
| `lex-07-paraphrase` | ninguna | 0 % | 0 |
| `lex-08-paraphrase` | ninguna | 0 % | 0 |
| `lex-09-paraphrase` | `lex-09.source` | 100 % | 0 |
| `lex-10-paraphrase` | ninguna | 0 % | 0 |
| `lex-11-paraphrase` | ninguna | 0 % | 0 |
| `lex-12-paraphrase` | `lex-12.source` | 100 % | 0 |

| Métrica | Línea base NIR-053 | Prototipo NIR-054 | Gate |
|---|---:|---:|---:|
| Recall de paráfrasis | 0 % (0/12) | 25 % (3/12) | mejora >=10 puntos: cumple |
| Recall sin paráfrasis | 100 % (28/28) | 100 % (28/28) | sin regresión |
| Recall global | 70 % (28/40) | 77,5 % (31/40) | informativo |
| Precisión citada | 100 % (28/28) | 100 % (31/31) | pérdida <=5 puntos: cumple |
| Citas irrelevantes | 0 | 0 | cumple |
| Contradicciones preservadas | 2/2 | 2/2 | cumple |
| p95 local agregado | 1,838 ms | 2,340 ms | <=250 ms: cumple |

La implementación se conserva porque cumple las tres condiciones de DR-002.
NIR-054 la mantuvo separada de `search_structured` hasta que NIR-055 verificó su
integración. Una prueba adicional demuestra resultados deterministas,
aislamiento entre archivos de mundo, ausencia de tablas semánticas, canon
intacto al borrar el índice derivado FTS5 y recuperación exacta después de
reconstruirlo.

## Integración NIR-055

**Estado:** gate de integración superado el 7 de agosto de 2026.

NIR-055 ejecuta el mismo corpus por la ruta híbrida que usa la aplicación. SQL y
las etapas de contexto mantienen bandas de prioridad superiores a FTS5 y
WordNet. Cada hit expone URI/ObjectRef, fragmento, etapa, procedencia, score
entero, rank y explicación. FTS5 gana la deduplicación cuando el mismo objeto
también coincide semánticamente; WordNet solo agrega fuentes nuevas.

Comando medido:

```powershell
cargo test -p nirmata-app --test retrieval_benchmark -- --nocapture
```

El límite de latencia wall-clock se aplica en este comando dedicado. El comando
`cargo nextest run --workspace` conserva todas las assertions funcionales y
métricas de calidad, pero no usa tiempos tomados mientras ejecuta en paralelo el
resto del workspace como gate de rendimiento local.

| Métrica | Línea base NIR-053 | Híbrido activo NIR-055 | Gate |
|---|---:|---:|---:|
| Recall de paráfrasis | 0 % (0/12) | 25 % (3/12) | mejora >=10 puntos: cumple |
| Recall sin paráfrasis | 100 % (28/28) | 100 % (28/28) | sin regresión |
| Recall global | 70 % (28/40) | 77,5 % (31/40) | informativo |
| Precisión citada | 100 % (28/28) | 100 % (31/31) | sin pérdida |
| Citas irrelevantes | 0 | 0 | cumple |
| Contradicciones preservadas | 2/2 | 2/2 | cumple |
| p95 local agregado | 3,745 ms | 3,517 ms | <=250 ms: cumple |

Las pruebas adicionales cubren update y delete visibles en la consulta
siguiente, ranking idéntico después de rebuild, aislamiento entre mundos,
degradación a FTS ante fallo semántico y ensamblaje mediante
`NirmataApp::get_related_context`. No existe tabla ni cache semántico: el modelo
estable es `wordnet-en-offline` versión `1`, y cambiarlo no deja estado derivado
de una versión anterior.

## Revalidación NIR-058

**Estado:** gate activo revalidado el 7 de agosto de 2026.

NIR-058 volvió ejecutables los conteos exactos del gate, además de los umbrales,
y unió la recuperación activa con exportación, edición externa, importación,
revisión humana selectiva, commit, exportación equivalente y undo. El escenario
completo se documenta en
[`retrieval-snapshot-e2e.md`](retrieval-snapshot-e2e.md).

Comando medido:

```powershell
cargo test -p nirmata-app --test retrieval_benchmark -- --nocapture
```

| Métrica | Línea base NIR-058 | Híbrido activo NIR-058 | Gate |
|---|---:|---:|---:|
| Recall de paráfrasis | 0 % (0/12) | 25 % (3/12) | mejora de 25 puntos: cumple |
| Recall no-paráfrasis | 100 % (28/28) | 100 % (28/28) | sin regresión |
| Precisión citada | 100 % (28/28) | 100 % (31/31) | sin pérdida |
| Citas irrelevantes | 0 | 0 | cumple |
| Contradicciones preservadas | 2/2 | 2/2 | cumple |
| p95 local agregado | 3,496 ms | 3,527 ms | <=250 ms: cumple |

Las siete familias deterministas siguen atribuidas por etapa y procedencia:
ancla, SQL estructurado, relaciones, tiempo, goals, perspectivas y FTS5. La
rama WordNet conserva URI/ObjectRef, fragmento y procedencia semántica para sus
tres fuentes adicionales. El test unido demuestra además que borrar FTS5 no
cambia canon ni impide a WordNet leerlo, y que el rebuild restaura la búsqueda
exacta.
