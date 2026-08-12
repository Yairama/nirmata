# Validación de calendario e historia

## Alcance

NIR-076–NIR-078 mantienen el tick `i64` como única autoridad temporal. El
calendario fijo es metadata opcional de `World` y solo convierte ticks a una
etiqueta de presentación.

## Casos ejecutables

- epoch, día, mes, año, ticks negativos, sub-tick y round-trip exacto;
- serde rechaza campos desconocidos y configuraciones que evaden constructores;
- schema 9 migra a schema 10 con `calendar_json = NULL`;
- configuración y eliminación usan `UpdateWorld`, revisión, auditoría y undo;
- entrada `año|mes|día|sub-tick` se convierte en Rust y una fecha inválida no
  produce draft;
- timeline conserva el tick y añade etiquetas del calendario observado;
- contexto y citas muestran tick y etiqueta sin persistir la proyección;
- revisiones históricas y variantes muestran calendarios distintos para el mismo
  evento e identidad;
- snapshot formato 1/schema 10 incluye calendario en metadata de `World`;
- snapshot formato 1/schema 9 sin calendario sigue siendo importable;
- editar calendario en snapshot produce un `UpdateWorld` revisable y reversible;
- publicación de snapshot reintenta bloqueos transitorios de Windows sin ocultar
  errores persistentes.

## Resultado vigente

El 12 de agosto de 2026, `cargo nextest run --workspace --no-fail-fast` ejecutó
236 pruebas offline: 236 pasaron y 1 smoke test de red quedó omitido. Frontend
build, 7 checks de seguridad, formato y desktop build también pasaron.

## Comandos

```powershell
cargo test -p nirmata-core
cargo test -p nirmata-store
cargo test -p nirmata-app --test manual_forms
cargo test -p nirmata-app --test phase10_variants
cargo test -p nirmata-app --test snapshot_import
cargo nextest run --workspace --no-fail-fast
node apps\nirmata-desktop\frontend\safety-check.test.mjs
npm run build --prefix apps\nirmata-desktop\frontend
cargo build -p nirmata-desktop
```
