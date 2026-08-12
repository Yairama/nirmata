# Regresion de variantes e historia

## Alcance

NIR-071–NIR-075 se verifican sin event sourcing, CRDT ni DAG general. La suite
principal vive en `crates/nirmata-app/tests/phase10_variants.rs`; la migracion
vive junto a las pruebas de `WorldStore`.

## Casos ejecutables

- esquema 7 migra toda identidad e historia a una unica variante `main` y crea
  snapshots historicos navegables;
- dos variantes divergen, conservan cabezas aisladas, reabren en la variante
  activa y rematerializan solo al cambiar explicitamente;
- `ReadScope` historico afecta URI, busqueda, contexto, timeline y VFS sin
  cambiar cabeza ni canon, y bloquea edicion;
- un draft nacido en otra variante se rechaza incluso cuando ambas variantes
  todavia comparten el mismo ID de revision;
- undo solo revierte commits propios de la variante activa;
- snapshots y lotes conservan variante/revision y no pueden aplicarse sobre
  otra variante; snapshots stale del mismo linaje siguen visibles pero no
  confirmables, preservando NIR-057;
- comparacion por `ObjectRef` distingue alta, baja, renombre, edicion y relacion
  divergente sin unir objetos por slug;
- merge independiente produce un ChangeSet destino normal y deja la cabeza
  fuente intacta;
- renombres solapados del mismo ID y dependencias ausentes generan
  `DecisionPoint`s pendientes con revision fuente, nunca una resolucion
  silenciosa;
- la GUI rotula cabeza activa frente a scope observado, abre ambos lados de un
  diff y entrega merge al panel de revision existente.

## Comandos

```powershell
cargo test -p nirmata-store
cargo test -p nirmata-app --test phase10_variants
cargo test -p nirmata-app --test snapshot_import
node apps\nirmata-desktop\frontend\safety-check.test.mjs
npm run build --prefix apps\nirmata-desktop\frontend
```
