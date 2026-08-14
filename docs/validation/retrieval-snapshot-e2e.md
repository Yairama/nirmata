# Validación end-to-end de recuperación y snapshots

**Estado:** aceptada el 7 de agosto de 2026.

## Frontera ejecutable

La prueba
`nir_058_hybrid_retrieval_and_snapshot_round_trip_preserve_authority_and_human_selection`
une en un solo mundo la ruta híbrida activa y el ciclo completo de snapshot:
exportar, editar externamente, importar, revisar, rechazar una operación,
confirmar la operación restante, volver a exportar y deshacer.

El escenario agrega una fuente que FTS5 recupera por vocabulario exacto y
WordNet recupera por paráfrasis. También agrega dos claims atribuidos y opuestos
sobre el mismo predicado. Cada resultado comprueba `ObjectRef`, URI estable,
etapa, procedencia, score, rank y explicación; ambos claims contradictorios
permanecen visibles antes y después del round-trip.

## Autoridad derivada

La prueba borra todas las filas de `canon_fts` y comprueba que:

- la búsqueda FTS aislada queda vacía;
- WordNet sigue recuperando la fuente desde las tablas canónicas;
- el snapshot lógico del canon no cambia;
- `rebuild_canon_text_index` restaura el resultado exacto desde canon.

No existe tabla ni cache semántico persistido. La representación WordNet se
calcula de nuevo desde el canon vigente, por lo que ni FTS5 ni la rama semántica
pueden convertirse en autoridad.

## Revisión humana

El snapshot externo renombra una entidad y edita su Markdown, pero el humano
rechaza esa operación. También edita un documento con Markdown hostil y texto
recuperable; esa operación permanece aceptada. El commit resultante registra
una sola operación auditada, conserva nombre, slug, cuerpo y URI de la entidad,
y aplica únicamente el documento aprobado.

La exportación posterior se importa inmediatamente como `SnapshotHasNoChanges`,
lo que comprueba su equivalencia lógica con el canon resultante. El undo crea su
revisión normal y restaura exactamente el contenido lógico anterior, ignorando
solo campos editoriales esperados como revisión, versión y timestamps.

Antes del import válido, una segunda exportación con `logical_hash` alterado se
rechaza sin cambiar canon. Las pruebas enfocadas existentes completan la matriz
sin duplicarla en el escenario unido: base stale no confirmable ni rebasable,
hashes, rutas, IDs, referencias, tipos, binarios y entradas extra manipuladas.

La superficie React usa un nombre controlado, presenta mundo, variante,
revision, conteos y hash antes de entregar el resultado a Cambios. Playwright
comprueba que el diff aparece antes de abrir la revision y que nunca se invoca
`confirm_manual_review` durante la importacion.

## Métricas

El benchmark `nir-053-v1` se volvió a ejecutar por la ruta activa:

| Métrica | Resultado NIR-058 | Gate |
|---|---:|---:|
| Recall de paráfrasis base | 0 % (0/12) | referencia |
| Recall de paráfrasis híbrido | 25 % (3/12) | mejora >= 10 puntos |
| Recall no-paráfrasis | 100 % (28/28) | sin regresión |
| Precisión citada | 100 % (31/31) | pérdida <= 5 puntos |
| Citas irrelevantes | 0 | 0 |
| Contradicciones preservadas | 2/2 | 2/2 |
| p95 local híbrido | 3,527 ms | <= 250 ms |

Estas métricas mantienen justificadas todas las etapas activas: las etapas
estructuradas y FTS5 conservan recall completo; WordNet aporta tres fuentes que
la línea base no recupera sin perder precisión, citas ni contradicciones.
La latencia se exige en el comando dedicado del benchmark; nextest ejecuta en
paralelo y conserva los gates funcionales, no un wall-clock contaminado por las
demás pruebas del workspace.

## Comandos verificados

Ejecutados desde la raíz del repositorio:

```powershell
cargo test -p nirmata-app --test retrieval_benchmark -- --nocapture
cargo test -p nirmata-app --test snapshot_import -- --nocapture
npm ci --prefix apps\nirmata-desktop\frontend
npm run build --prefix apps\nirmata-desktop\frontend
node --test apps\nirmata-desktop\frontend\safety-check.test.mjs
cargo nextest run --workspace
cargo build -p nirmata-desktop
```

Resultados:

- benchmark: 6 aprobadas, 0 fallidas;
- snapshot/import E2E: 5 aprobadas, 0 fallidas;
- frontend: instalación y build completados, 0 vulnerabilidades;
- seguridad frontend: 4 aprobadas, 0 fallidas;
- workspace: 184 aprobadas, 1 smoke test de red omitido;
- desktop: build completado.

No se realizaron llamadas de red.
