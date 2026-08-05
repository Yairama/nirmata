# Fuentes tecnicas evaluadas

**Fecha de revision:** 2026-08-04.

Este archivo registra referencias para decisiones sensibles al estado del
ecosistema. Las decisiones del producto viven en `docs/architecture/`.

La base academica narrativa vive en
[`academic-foundations/README.md`](academic-foundations/README.md).

## Orquestacion

- [Deep Agents from Scratch](https://github.com/langchain-ai/deep-agents-from-scratch):
  curso auditado sobre TODO, VFS y aislamiento de subagentes.
- [Deep Agents](https://github.com/langchain-ai/deepagents): harness de
  produccion relacionado, con capacidades adicionales.
- [LangGraph](https://github.com/langchain-ai/langgraph): grafos de agentes,
  ejecucion durable e interrupciones; Python.
- [Microsoft Agent Framework](https://github.com/microsoft/agent-framework):
  sucesor recomendado por Microsoft para nuevos proyectos.
- [AutoGen](https://github.com/microsoft/autogen): referencia historica; su
  repositorio dirige nuevos desarrollos al framework sucesor.
- [CrewAI](https://github.com/crewAIInc/crewAI): agentes y workflows en Python.
- [Pydantic AI](https://github.com/pydantic/pydantic-ai): agentes tipados y
  grafos en Python.
- [Rig](https://github.com/0xPlaygrounds/rig): integracion y agentes LLM en
  Rust; candidato posterior, no dependencia necesaria del MVP.

## Persistencia y recuperacion

- [rusqlite](https://github.com/rusqlite/rusqlite): binding SQLite para Rust.
- [sqlite-vec](https://github.com/asg017/sqlite-vec): vectores embebidos en
  SQLite; opcion posterior.
- [LanceDB](https://github.com/lancedb/lancedb): almacenamiento vectorial
  embebido y multimodal.
- [Qdrant](https://github.com/qdrant/qdrant): base vectorial Rust orientada a
  servicio y despliegues mayores.
- [Microsoft GraphRAG](https://github.com/microsoft/graphrag): extraccion de
  grafos desde texto no estructurado.
- [GraphRAG paper](https://arxiv.org/pdf/2404.16130): fundamento del enfoque.
- [Tantivy](https://github.com/quickwit-oss/tantivy): motor de busqueda Rust a
  considerar solo si FTS5 deja de ser suficiente.

## GUI y runtime

- [Tauri](https://github.com/tauri-apps/tauri): escritorio con backend Rust y
  webview.
- [egui](https://github.com/emilk/egui): GUI inmediata completamente Rust.
- [GPUI/Zed](https://github.com/zed-industries/zed): GUI Rust orientada a
  aplicaciones tipo editor.
- [Candle](https://github.com/huggingface/candle): inferencia ML en Rust para
  una posible etapa local/offline.

## Colaboracion futura

- [Automerge](https://github.com/automerge/automerge): CRDT local-first. No es
  parte del producto inicial.

## Conclusiones derivadas

- Las herramientas de grafos de agentes mas maduras se concentran en Python,
  pero Nirmata no necesita sus capacidades iniciales.
- SQLite cubre canon, transacciones, texto completo y una futura capa vectorial
  sin servicios adicionales.
- GraphRAG resuelve importacion desde texto no estructurado, no el almacenamiento
  de un canon que ya nace estructurado.
- Rust es suficiente para el workflow y evita distribuir un segundo runtime.
