# Validación del motor de simulación

## Alcance

NIR-079–NIR-080 modelan escenarios efímeros y externos al canon. Cada escenario
fija mundo, variante y revisión base; usa facciones y recursos existentes, stocks
enteros, capacidad y reglas ordenadas de producción, consumo y transferencia.

## Casos ejecutables

- serde estricto rechaza campos desconocidos;
- facciones deben ser `EntityKind::Faction` y recursos `EntityKind::Resource`;
- IDs, unidades, stocks, cantidades, capacidad y límite de pasos se validan;
- crear, editar, ejecutar y borrar no cambian la revisión canónica;
- la misma entrada produce JSON byte-idéntico;
- cada transición conserva regla, índice, before/after, requested, applied y
  shortage;
- producción respeta capacidad, consumo respeta existencia y transferencia
  respeta origen y capacidad destino;
- no existe azar, IA, loop continuo ni tarea de background.

## Resultado vigente

El 12 de agosto de 2026 la suite específica pasó 3/3 y
`cargo nextest run --workspace --no-fail-fast` pasó 239/239 pruebas offline, con
1 smoke test de red omitido.
