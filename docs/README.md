# Nirmata Docs

Documentacion de producto y arquitectura organizada como un arbol navegable.
Cada archivo cubre una sola responsabilidad y puede referenciarse por su ruta.

## Indice

### Producto

- [`product/vision.md`](product/vision.md): tesis, usuario y flujo principal.
- [`product/mvp.md`](product/mvp.md): alcance inicial y exclusiones.

### Dominio

- [`domain/model.md`](domain/model.md): entidades conceptuales y reglas del canon.

### Arquitectura

- [`architecture/README.md`](architecture/README.md): indice y decisiones tecnicas.
- [`architecture/system-overview.md`](architecture/system-overview.md): arquitectura general y limites.
- [`architecture/workspace.md`](architecture/workspace.md): estructura del workspace Rust.
- [`architecture/agent-graph.md`](architecture/agent-graph.md): workflow de IA y evolucion multiagente.
- [`architecture/reasoning-policy.md`](architecture/reasoning-policy.md): reasoning, bucles y autocritica.
- [`architecture/validation-pipeline.md`](architecture/validation-pipeline.md): capas de validacion.
- [`architecture/interaction-model.md`](architecture/interaction-model.md): consulta, edicion y revision.
- [`architecture/storage.md`](architecture/storage.md): canon, Markdown y VFS logico.
- [`architecture/retrieval.md`](architecture/retrieval.md): SQL, FTS, RAG y grafos.
- [`architecture/language-runtime.md`](architecture/language-runtime.md): Rust, Python y GUI.
- [`architecture/ai-flow.md`](architecture/ai-flow.md): resumen del flujo controlado de IA.

### Investigacion

- [`research/technical-sources.md`](research/technical-sources.md): fuentes y tecnologias evaluadas.
- [`research/deep-agents-from-scratch.md`](research/deep-agents-from-scratch.md): auditoria del repositorio de LangChain.
- [`research/academic-foundations/README.md`](research/academic-foundations/README.md): sintesis de teoria literaria, cognitiva y computacional.
- [`research/critical-fronts/README.md`](research/critical-fronts/README.md): cocreacion, versionado, conocimiento incierto y tiempo.

### Ejecucion

- [`roadmap/phases.md`](roadmap/phases.md): orden recomendado de construccion.
- [`validation/vertical-slice.md`](validation/vertical-slice.md): escenario que valida la tesis del producto.
- [`validation/ai-regression-suite.md`](validation/ai-regression-suite.md): casos para validar generador y critico.
- [`validation/foundation-acceptance.md`](validation/foundation-acceptance.md): aceptacion ejecutable del fundamento funcional.
- [`validation/retrieval-benchmark.md`](validation/retrieval-benchmark.md): corpus, metricas y gate semantico de recuperacion.
- [`validation/retrieval-snapshot-e2e.md`](validation/retrieval-snapshot-e2e.md): aceptacion unida de recuperacion activa y snapshots.
- [`validation/deep-review-regression.md`](validation/deep-review-regression.md): presupuestos, fallos, desacuerdos y frontera NIR-047 del perfil profundo.
- [`validation/lore-import-e2e.md`](validation/lore-import-e2e.md): ingestion hostil, procedencia, revision estandar, commit y undo offline.
- [`validation/variants-history.md`](validation/variants-history.md): migracion, aislamiento, historia, comparacion y merge limitado.

## Convencion

- Una idea principal por archivo.
- Enlaces relativos entre documentos.
- Las propuestas no son decisiones definitivas hasta indicarlo expresamente.
- La documentacion se divide solo cuando un archivo mezcla responsabilidades.
