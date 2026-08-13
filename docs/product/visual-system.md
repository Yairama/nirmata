# Sistema visual Nirmata

**Estado:** especificacion aprobada.

## Direccion

Nirmata es una mesa editorial de mundo, no un dashboard. El contenido domina;
divisores y ritmo sustituyen cajas anidadas. Las tarjetas se reservan para
propuestas, decisiones, conflictos y documentos. El color expresa autoridad,
estado o severidad, no el subsistema.

## Tokens base

| Token | Light | Dark |
|---|---|---|
| `--n-color-canvas` | `#F5F2EA` | `#151412` |
| `--n-color-surface` | `#FFFCF5` | `#1D1B18` |
| `--n-color-raised` | `#FFFFFF` | `#26231F` |
| `--n-color-subtle` | `#ECE7DC` | `#302C27` |
| `--n-color-text` | `#24211C` | `#F1ECE2` |
| `--n-color-text-muted` | `#625D54` | `#BDB5A8` |
| `--n-color-border` | `#898175` | `#756E64` |
| `--n-color-border-strong` | `#625B52` | `#AAA194` |
| `--n-color-accent` | `#1F5C78` | `#8BC6DC` |
| `--n-color-accent-soft` | `#DCEAF0` | `#20343D` |
| `--n-color-action` | `#1F5C78` | `#2E6D89` |
| `--n-color-action-hover` | `#17475D` | `#255A72` |
| `--n-color-on-action` | `#FFFFFF` | `#FFFFFF` |
| `--n-color-focus` | `#006A87` | `#8DD3ED` |

El contraste texto/canvas supera `14:1`; texto de accion/action supera `7:1`
en light y `5.7:1` en dark; borde/superficie supera `3:1`.

## Autoridad y severidad

- Canon: sello/check, texto `Canon`, verde apagado.
- Perspectiva: cita/persona y titular explicito, violeta.
- Inferencia: enlace discontinuo y texto `Inferencia`, ambar.
- Solo lectura: candado y franja persistente, neutral; nunca rojo.
- Sin evidencia: neutral y borde discontinuo; no es warning.
- Fuera del canon/propuesta: borde discontinuo y etiqueta textual.

`error`, `conflict`, `warning` e `info` usan tokens distintos y siempre icono,
nombre y texto. `success` describe una accion completada, no una severidad.

| Estado | Light | Dark |
|---|---|---|
| Danger | `#A12622` / `#F5DEDA` | `#FFB4AB` / `#49201E` |
| Warning | `#7A4B00` / `#F3E3C3` | `#F0C36A` / `#453516` |
| Conflict | `#7A3E00` / `#F4DFC9` | `#E7A45A` / `#442B18` |
| Info | `#1F5C78` / `#DCEAF0` | `#8BC6DC` / `#20343D` |
| Success | `#2F6B4F` / `#DCE9DF` | `#91D5A8` / `#1E3829` |
| Canon | `#376A47` / `#DDE9DF` | `#A4D5AF` / `#203526` |
| Perspectiva | `#684B8A` / `#E9E0F2` | `#D0B6F4` / `#382A49` |
| Inferencia | `#7A4B00` / `#F3E3C3` | `#F0C36A` / `#453516` |

## Tipografia y densidad

- UI: `system-ui, -apple-system, "Segoe UI", sans-serif`.
- Editorial: `ui-serif, Charter, Georgia, Cambria, serif`.
- Tecnica: `ui-monospace, "Cascadia Mono", Consolas, monospace`.
- Controles normales `36px`, compactos `32px`, estrechos/tactiles `44px`.
- Escala espacial `4, 8, 12, 16, 24, 32px`.
- Serif para mundo, objetos y prosa; mono solo en detalles tecnicos.
- Sombras solo en overlays; paneles acoplados usan bordes.

## Shell responsive

- `>=1200px`: topbar, navegacion `216-256px`, editor flexible y panel derecho
  `304-352px`; cola inferior colapsable.
- `800-1199px`: navegacion y editor; contexto/asistente en drawer derecho.
- `<800px`: una sola region principal; navegacion, asistente, contexto y
  revision como sheets/tabs, nunca toda la app apilada.
- Cada region tiene scroll propio dentro de `100dvh`.

## Temas y movimiento

Light/dark consumen las mismas variables semanticas. High contrast aplana
superficies, usa bordes de `2px` y en `forced-colors` adopta colores del sistema.
Reduced motion elimina smooth scroll y transiciones; busy conserva texto o
progreso estatico.

## Gate

1. Hex/rgb solo en bloques de tokens.
2. Texto normal `>=4.5:1`; foco, bordes y texto grande `>=3:1`.
3. Fixtures light/dark/high contrast para landing, workspace, read-only, error y revision.
4. Estados epistemicos y severidad no dependen solo del color.
5. `720x520`, `960x680`, `1440x900` y `390x844` no tienen scroll horizontal.
6. Workspace aparece en el primer viewport; laboratorios no lo preceden.
7. En estrecho solo una region principal esta abierta.
8. Forced colors y reduced motion conservan foco, seleccion y significado.
9. Todo icon-only tiene nombre accesible.
10. No se agrega una dependencia para colores, tipografia, iconos o motion.
