# Workspace inicial

**Estado:** recomendacion consolidada.

## Estructura

```text
nirmata/
|-- Cargo.toml
|-- crates/
|   |-- nirmata-core/
|   |-- nirmata-store/
|   |-- nirmata-ai/
|   `-- nirmata-app/
`-- apps/
    `-- nirmata-desktop/
        |-- src-tauri/
        `-- frontend/
```

## Responsabilidades

| Paquete | Responsabilidad |
|---|---|
| `nirmata-core` | Modelo de dominio, validaciones y reglas del canon |
| `nirmata-store` | SQLite, migraciones, consultas y busqueda |
| `nirmata-ai` | Proveedor, prompts, streaming y salida estructurada |
| `nirmata-app` | Casos de uso, contexto y workflow |
| `nirmata-desktop` | Tauri, GUI y adaptadores de presentacion |

## Dependencias

```text
             nirmata-core
              ^    ^   ^
              |    |   |
          store   ai   app
              ^    ^   ^
               \   |  /
                desktop
```

`nirmata-core` debe ser sincrono y no conocer GUI, SQLite, HTTP ni proveedores
de IA.

`nirmata-app` evita que la GUI coordine directamente almacenamiento e IA. Es la
unica adicion respecto al esquema minimo que ya tiene una responsabilidad real:
consulta y edicion comparten los mismos casos de uso.

No se necesitan crates separados para agentes, simulacion, economia, mapas,
linguistica o RAG. Esas divisiones solo deben aparecer cuando exista codigo
real que no pueda mantenerse cohesionado.

## Stack recomendado

- Rust estable, edicion 2024.
- Tauri 2 para escritorio.
- SQLite con `rusqlite`.
- FTS5 para busqueda textual.
- `serde` para estructuras y serializacion.
- `reqwest` para comunicacion con el proveedor de IA.
- `tokio` limitado a aplicacion, IA y frontera Tauri.
- `tracing` para diagnostico.

SQLite evita operar servidores y cubre transacciones, busqueda, JSON y archivos
de proyecto portables. No se justifica inicialmente una base de grafos,
PostgreSQL ni una base vectorial.

## GUI

- Panel izquierdo: navegacion, entidades, filtros y busqueda.
- Panel central: editor de entidad, documento o linea temporal.
- Panel derecho: relaciones, contexto, advertencias y propuestas.
- Panel inferior: cambios pendientes y validacion.

Tauri se recomienda sobre una GUI completamente Rust porque el producto depende
de edicion Markdown, streaming y diffs. `egui` permanece como alternativa para
un prototipo estricto all-Rust.

Los mapas visuales y grafos interactivos quedan para una necesidad posterior.

Ver [`language-runtime.md`](language-runtime.md) para la comparacion.
