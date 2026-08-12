# Regresion de revision profunda

## Alcance

La suite offline de Fase 8 verifica que el perfil profundo agrega contexto y
desacuerdos sin crear otra autoridad de escritura. No usa red ni credenciales
reales.

## Contratos y presupuestos

- `SpecialistReport` y `DeepSynthesis` rechazan campos desconocidos en todos sus
  DTOs propios.
- Cada hallazgo exige fuente, objeto afectado y evidencia citada dentro del
  snapshot inmutable.
- Un especialista no puede devolver `operations` ni recibe tools del proveedor.
- El plan admite entre uno y cuatro roles confirmados, cuatro llamadas como
  maximo, dos expansiones, seis nombres de tools de lectura, cero delegaciones,
  2.048 tokens por informe, 4.096 para sintesis y 30 segundos por llamada.
- Los limites viven en Rust y se incluyen en el payload para auditoria; el modelo
  no puede ampliarlos.

## Matriz offline

| Caso | Resultado exigido |
|---|---|
| Crisis de recursos/comercio | selecciona economista |
| Guerra de sucesion/poder | selecciona historiador y politologo |
| Cambio geografico/ruta comercial | selecciona geografo y economista |
| Auditoria | selecciona auditores temporal, reglas, causal y perspectivas |
| Timeout parcial | conserva informes exitosos y registra el timeout |
| Todos fallan | no llama al sintetizador y no crea propuesta |
| Cancelacion previa | no inicia llamadas nuevas |
| Fuente fuera del snapshot | invalida el informe o la sintesis |
| Finding/operacion duplicado o ausente | invalida la sintesis |
| Dos posiciones incompatibles | exige `DecisionPoint` con ambos findings |
| Sintesis valida | crea un run estandar `AwaitingReview` |
| Commit inmediato | NIR-047 lo rechaza hasta accion humana y critica final fresca |

Las auditorias consolidan findings como `ValidationReport` advisory y nunca
producen un draft. Los informes completos y sus errores siguen visibles en el
run; no se rellena un fallo con contenido sintetico.

## Frontera de escritorio

`Consultar`, `Proponer`, `Revision profunda` y `Auditoria` son modos explicitos.
Los dos ultimos primero muestran roles y presupuesto. Una segunda accion del
usuario confirma los roles e inicia el run. Progreso, fallos, informes,
evidencia y desacuerdos se muestran fuera del transcript canonico. Solo una
sintesis completa obtiene un `standardRunId` y entra al panel de cambios
pendientes existente.

Todo texto de proveedor se asigna con `textContent`. Cancelar usa el mismo token
de red que consulta/propuesta y nunca adjunta un draft parcial.

## Comandos

```powershell
cargo test -p nirmata-ai contracts
cargo test -p nirmata-ai deep_capabilities
cargo test -p nirmata-app deep_review
cargo test -p nirmata-desktop --bin nirmata-desktop
npm run build --prefix apps\nirmata-desktop\frontend
node --test apps\nirmata-desktop\frontend\safety-check.test.mjs
```

La aceptacion de fase exige ademas `cargo nextest run --workspace` y
`cargo build -p nirmata-desktop`.
