# Nirmata

Nirmata es una aplicacion de escritorio local-first para construir y mantener
universos ficticios coherentes. Funciona como un IDE de storyworlds: modela
canon, entidades, relaciones, acontecimientos, perspectivas y cambios asistidos
por IA.

La IA puede consultar el mundo y proponer `ChangeSet`s. Nunca modifica el canon
directamente; cada cambio se valida, revisa y confirma mediante una transaccion
SQLite.

## Estado

El primer corte vertical local esta implementado. La aplicacion Tauri crea,
cierra y reabre un mundo desde un archivo SQLite `.nirmata`, sin IA.

El corte incluye:

- workspace Rust modular con `core`, `store`, `app` y `desktop`;
- identidad y validacion minima de `World`;
- esquema SQLite versionado con revision inicial;
- casos de uso `create_world`, `open_world` y `close_world`;
- frontend TypeScript, HTML y CSS nativos.

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

Las tareas habituales estan disponibles desde la raiz mediante
[`just`](https://just.systems/):

```powershell
just setup     # instala dependencias frontend exactas
just dev       # inicia Vite
just build     # frontend + ejecutable debug
just test      # unit, E2E, safety y workspace Rust
just release   # frontend + ejecutable Rust optimizado
```

`just release` genera el ejecutable en `target\release`; no crea un instalador
mientras `bundle.active` permanezca deshabilitado en Tauri. Usa `just --list`
para ver las recetas separadas de check y test.

Validacion del corte actual:

```powershell
npm ci --prefix apps\nirmata-desktop\frontend
npm run build --prefix apps\nirmata-desktop\frontend
cargo nextest run --workspace
cargo build -p nirmata-desktop
```

Para probar la frontera Tauri manualmente:

```powershell
cargo run -p nirmata-desktop
```

## Compilacion de produccion

Desde la raiz del repositorio, instala las dependencias exactas del frontend,
genera los assets y compila el ejecutable optimizado:

```powershell
npm ci --prefix apps\nirmata-desktop\frontend
npm run build --prefix apps\nirmata-desktop\frontend
cargo build --release -p nirmata-desktop
```

El ejecutable resultante queda en:

```text
target\release\nirmata-desktop.exe
```

`cargo build` no ejecuta el `beforeBuildCommand` de Tauri CLI, por eso el build
del frontend es un paso explicito. El empaquetado de instaladores no esta
habilitado actualmente (`bundle.active` es `false` en `tauri.conf.json`).

En la ventana, crea un `.nirmata`, cierralo y abre el mismo archivo. La revision
mostrada debe conservarse.

Para desarrollo con Azure AI Foundry, configura en el proceso `BASE_URL`,
`PROVIDER_API_KEY` y `AZURE_FOUNDRY_MODEL` (el test también acepta
`GPT-5.6-SOL`). El repositorio ignora `.env`, pero no lo carga automáticamente.
El cliente usa `POST /openai/v1/responses`, autenticación Bearer y `store: false`;
el nombre del modelo/deployment se envía en cada solicitud.

Principio rector:

> Implementar la solucion mas pequena que preserve el canon y resuelva una
> necesidad demostrada.
