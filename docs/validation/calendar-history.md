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
- weekdays y meses se editan como filas con nombre/días y controles de
  agregar, quitar y reordenar; el formato interno no se expone;
- año, mes, día y unidad se editan por controles separados; el frontend solo
  serializa `ManualDraftRequest` y Rust convierte la fecha, por lo que una fecha
  inválida no produce draft;
- `unknown`, `instant`, `interval` y `ongoing` conservan sus invariantes sin
  exigir un tick visible;
- abrir un evento y editar una revisión pendiente reciben una proyección Rust
  estructurada para hidratar la fecha sin conversión TypeScript;
- timeline conserva el tick y añade etiquetas del calendario observado;
- timeline sin calendario presenta `Tiempo conocido sin calendario de
  presentación` en vez de exponer el tick;
- contexto y citas muestran tick y etiqueta sin persistir la proyección;
- revisiones históricas y variantes muestran calendarios distintos para el mismo
  evento e identidad;
- snapshot formato 1/schema 10 incluye calendario en metadata de `World`;
- snapshot formato 1/schema 9 sin calendario sigue siendo importable;
- editar calendario en snapshot produce un `UpdateWorld` revisable y reversible;
- configurar calendario desde Cronología o la paleta abre el editor del `World`
  actual; una versión histórica mantiene los accesos en solo lectura;
- revisión de `World` presenta nombres humanos de días y meses, con epoch y
  unidades bajo detalles técnicos;
- publicación de snapshot reintenta bloqueos transitorios de Windows sin ocultar
  errores persistentes.

## Resultado vigente

El 12 de agosto de 2026, `cargo nextest run --workspace --no-fail-fast` ejecutó
236 pruebas offline: 236 pasaron y 1 smoke test de red quedó omitido. Frontend
build, 7 checks de seguridad, formato y desktop build también pasaron.

El 13 de agosto de 2026, UX-065 pasó 5 unit frontend, 22 safety y 39 E2E con axe
y capturas `1280x900`/`390x844`. Rust pasó 7 pruebas de calendario, 11 de
formularios manuales, 15 de variantes y 7 de búsqueda/proyección. El build Vite
midió 166,51 KiB JS gzip y 11,62 KiB CSS gzip; continúa el warning conocido por
el chunk único mayor de 500 kB sin comprimir.

## Comandos

```powershell
cargo test -p nirmata-core
cargo test -p nirmata-store
cargo test -p nirmata-app --test manual_forms
cargo test -p nirmata-app --test phase10_variants
cargo test -p nirmata-app --test search_use_cases
cargo test -p nirmata-app --test snapshot_import
cargo nextest run --workspace --no-fail-fast
node apps\nirmata-desktop\frontend\safety-check.test.mjs
npm test --prefix apps\nirmata-desktop\frontend
npm run test:e2e --prefix apps\nirmata-desktop\frontend
npm run build --prefix apps\nirmata-desktop\frontend
cargo build -p nirmata-desktop
```
