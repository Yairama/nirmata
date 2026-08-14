# Validación de derivaciones narrativas

## Alcance

NIR-083–NIR-086 derivan estructuras desde un `ReadScope`; documentos y
continuidad solo aparecen como drafts/revisiones estándar.

## Casos ejecutables

- story time ordena por ticks conocidos y separa tiempo desconocido;
- discourse order usa `ContentReference.ordinal` y conserva flashbacks;
- hilos causales recorren `EventLink` con profundidad máxima 3, límite 100,
  orden estable y prevención de ciclos;
- cada enlace conserva kind, source, target y evidencia navegable;
- goals explícitamente activos producen `active_goal_without_resolution`;
- eventos ongoing producen `ongoing_event`;
- claims disputed no superseded producen `disputed_claim`;
- no se emiten hallazgos basados únicamente en ausencia de datos;
- variante y revisión histórica producen resultados scoped deterministas;
- ejecutar derivaciones no cambia revisión ni tablas canónicas.
- documento interno acepta solo crónica, carta, informe, mito o historia corta;
- contexto de documento filtra por perspectiva y tick y referencias deben
  pertenecer al bundle accesible;
- fallo/cancelación no crea documento ni revisión;
- continuidad presenta pregunta, alternativas y consecuencias antes de llamar
  propuesta;
- la alternativa elegida queda como `DecisionPoint` y las fuentes originales se
  conservan en el draft estándar;
- revisión profunda no se activa implícitamente.
- la GUI permite derivar en read-only, pero bloquea documentos/propuestas fuera
  de la cabeza activa;
- fuentes, codes, story time y discourse order se renderizan como texto inerte;
- no existe acción de generar novela ni confirmación directa.

## Resultado vigente

El 12 de agosto de 2026 la regresión narrativa y AI pasó sus casos offline,
desktop pasó 14/14, safety 9/9 y
`cargo nextest run --workspace --no-fail-fast` pasó 256/256 pruebas offline, con
1 smoke test de red omitido.

El 13 de agosto de 2026, UX-063–UX-064 sustituyeron el owner imperativo por
`narrative-workspace.tsx`. Playwright verificó las cuatro tabs, scope histórico,
pickers por nombre, fecha desde acontecimientos existentes, preview inerte,
adjunto único a Cambios y cero tarjetas ante fallo/cancelación. Axe no reportó
violaciones serious/critical y las capturas finales no desbordan:

- `apps/nirmata-desktop/frontend/test-results/workspace-theme-narrative--8aa39-ws-one-safe-document-review/narrative-desktop.png`
- `apps/nirmata-desktop/frontend/test-results/workspace-theme-narrative--8aa39-ws-one-safe-document-review/narrative-narrow.png`

Frontend pasó 5/5 unit, 22/22 safety y 36/36 E2E. El build quedó en 164,67 KiB
JS gzip y 11,42 KiB CSS gzip.
