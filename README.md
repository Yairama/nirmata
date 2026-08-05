# Nirmata

Nirmata es una aplicacion de escritorio local-first para construir y mantener
universos ficticios coherentes. Funciona como un IDE de storyworlds: modela
canon, entidades, relaciones, acontecimientos, perspectivas y cambios asistidos
por IA.

La IA puede consultar el mundo y proponer `ChangeSet`s. Nunca modifica el canon
directamente; cada cambio se valida, revisa y confirma mediante una transaccion
SQLite.

## Estado

El proyecto esta en fase de diseno tecnico consolidado. Todavia no existe codigo
de producto ni comandos de instalacion o ejecucion.

La documentacion actual define:

- vision y alcance del MVP;
- modelo de dominio;
- arquitectura local-first;
- almacenamiento y recuperacion;
- workflow y validacion de IA;
- fundamentos academicos;
- orden de implementacion.

Empieza por [`docs/README.md`](docs/README.md).

## MVP

La primera version debe permitir:

- crear y reabrir un mundo;
- definir reglas, entidades, relaciones, objetivos y eventos;
- distinguir canon, rumores, creencias y datos desconocidos;
- consultar el mundo con fuentes navegables;
- proponer cambios estructurados;
- detectar conflictos directos;
- revisar, aceptar, editar o rechazar cada operacion;
- conservar revisiones lineales, auditoria y undo.

No incluye multiagentes autonomos, colaboracion, ramas, base vectorial, base de
grafos, calendarios configurables ni un motor logico general.

## Arquitectura prevista

```text
nirmata-core      dominio y validacion determinista
nirmata-store     SQLite, consultas, migraciones y FTS5
nirmata-ai        proveedor, prompts y salida estructurada
nirmata-app       casos de uso y workflow
nirmata-desktop   Tauri y presentacion
```

Stack base:

- Rust estable, edicion 2024;
- Tauri 2;
- SQLite con `rusqlite`;
- `serde`, `reqwest`, `tokio` y `tracing` cuando el codigo que los necesite
  exista.

La estructura detallada esta en
[`docs/architecture/workspace.md`](docs/architecture/workspace.md).

## Desarrollo

Las reglas para agentes y contribuidores estan en [`AGENTS.md`](AGENTS.md).

Principio rector:

> Implementar la solucion mas pequena que preserve el canon y resuelva una
> necesidad demostrada.

