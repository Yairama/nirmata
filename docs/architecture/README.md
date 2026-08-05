# Arquitectura tecnica

**Estado:** recomendacion consolidada antes del desarrollo.

## Decision resumida

| Area | Recomendacion |
|---|---|
| Estilo | Monolito modular local-first |
| Orquestacion | Perfil estandar en MVP; perfil profundo posterior |
| Canon | SQLite en un archivo `.nirmata` |
| Prosa | Markdown almacenado como `TEXT` |
| VFS | Espacio de nombres logico y exportable |
| Busqueda inicial | SQL + FTS5 |
| Relaciones | Tablas relacionales y CTE recursivos |
| Vectores | Solo despues de medir fallos semanticos |
| GraphRAG | Solo para importar prosa no estructurada |
| Base de grafos | No necesaria en el alcance conocido |
| Backend | Rust |
| GUI recomendada | Tauri 2 con frontend web pequeno |
| Python | Herramienta externa opcional, nunca requisito del MVP |
| Escrituras de IA | Generador + Rust + critico + humano + transaccion |

## Principio central

Nirmata no debe delegar la autoridad del mundo a un LLM. El modelo propone;
el dominio valida; el usuario decide; SQLite confirma.

```text
usuario
  -> caso de uso
  -> recuperacion de contexto
  -> modelo
  -> salida tipada
  -> validacion
  -> revision humana
  -> transaccion
```

## Documentos

- [`system-overview.md`](system-overview.md): procesos, capas y dependencias.
- [`workspace.md`](workspace.md): paquetes y estructura del repositorio.
- [`agent-graph.md`](agent-graph.md): topologia del workflow.
- [`reasoning-policy.md`](reasoning-policy.md): bucles, presupuestos y autocritica.
- [`validation-pipeline.md`](validation-pipeline.md): doble check y capas de validacion.
- [`interaction-model.md`](interaction-model.md): UX conversacional y de edicion.
- [`storage.md`](storage.md): persistencia, Markdown y VFS.
- [`retrieval.md`](retrieval.md): construccion de contexto y evolucion de RAG.
- [`language-runtime.md`](language-runtime.md): seleccion de lenguajes y GUI.

## Lo que se evita deliberadamente

- Microservicios.
- Un proceso por agente.
- Un ReAct loop libre como motor del producto.
- Escritura libre del LLM sobre archivos o tablas.
- Python embebido en la aplicacion.
- Una base vectorial separada.
- Una base de grafos separada.
- Event sourcing como fuente primaria.
- Sincronizacion multiusuario.

Cada elemento agrega fallos operativos sin resolver una necesidad del primer
producto.
