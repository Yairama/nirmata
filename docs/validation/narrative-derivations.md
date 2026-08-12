# Validación de derivaciones narrativas

## Alcance

NIR-083 deriva estructuras de solo lectura desde un `ReadScope`. No crea canon,
documentos ni inferencias persistidas.

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

## Resultado vigente

El 12 de agosto de 2026 la suite narrativa pasó 1/1 y
`cargo nextest run --workspace --no-fail-fast` pasó 245/245 pruebas offline, con
1 smoke test de red omitido.
