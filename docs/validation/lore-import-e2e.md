# Regresion E2E de importacion de lore

**Estado:** verificada para NIR-065–NIR-070.

## Alcance

La prueba offline `nir_070_offline_multipage_import_commits_only_reviewed_provenance_and_undoes_after_reopen`
usa [`../../crates/nirmata-app/tests/fixtures/lore_import/chronicle.md`](../../crates/nirmata-app/tests/fixtures/lore_import/chronicle.md)
y [`../../crates/nirmata-app/tests/fixtures/lore_import/orders.txt`](../../crates/nirmata-app/tests/fixtures/lore_import/orders.txt).
Incluyen encabezados multipagina, alias, afirmaciones opuestas, prompt injection,
HTML, enlace `file://` y macro hostil. El proveedor es un fake del contrato
estandar; la prueba no usa red, Revision profunda, base de grafos ni servicio
externo.

## Garantias ejecutadas

- un lote mixto con binario falla antes de staging y deja canon identico;
- reemplazar una fuente cambia hash/chunks y elimina la generacion vieja;
- cerrar antes de revisar, reabrir el `.nirmata` y listar lotes recupera las
  fuentes copiadas sin tocar canon;
- cancelacion previa consume cero salidas y no deja candidatos parciales;
- cada candidato conserva cita literal a source/hash/chunk;
- alias entre paginas resuelven identidad, mientras claims opuestos permanecen
  separados;
- solo Entity y Claim seleccionados generan las dos operaciones del ChangeSet;
- regla derivada de script, relacion derivada de prompt injection y claim
  rechazado no aparecen en canon;
- un commit intermedio vuelve stale la revision y la critica final revalida la
  cabeza vigente antes de confirmar;
- el Claim confirmado conserva `import://` y el ChangeSet conserva dos trazas
  candidato-operacion-chunk con audits `lore_import`;
- borrar staging no borra canon confirmado;
- fuentes seleccionadas y fixtures permanecen byte a byte iguales;
- cerrar/reabrir conserva el commit y undo; un segundo reopen confirma que undo
  retiro solo las operaciones importadas.
- Playwright verifica reanudacion, campos completos por tipo, citas por archivo
  y lineas, hash bajo detalles tecnicos, teclado/axe y `390x844` sin overflow.

## Comandos

```powershell
cargo test -p nirmata-ai contracts
cargo test -p nirmata-app lore_import
cargo test -p nirmata-app nir_070
npm run build --prefix apps\nirmata-desktop\frontend
node --test apps\nirmata-desktop\frontend\safety-check.test.mjs
cargo test -p nirmata-desktop
```

El gate final de fase exige ademas `cargo nextest run --workspace` y
`cargo build -p nirmata-desktop`.
