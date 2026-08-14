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
| `--n-color-surface` | `#FBFCF8` | `#171E1B` |
| `--n-color-raised` | `#FFFFFF` | `#202925` |
| `--n-color-subtle` | `#E9ECE7` | `#28322E` |
| `--n-color-text` | `#18201E` | `#EDF2EE` |
| `--n-color-text-muted` | `#5E6965` | `#AAB7B1` |
| `--n-color-border` | `#D1D7D2` | `#36433E` |
| `--n-color-border-strong` | `#9DA8A3` | `#718079` |
| `--n-color-accent` | `#136F68` | `#73D0C7` |
| `--n-color-accent-soft` | `#DCEFEB` | `#173D39` |
| `--n-color-action` | `#176F68` | `#287E76` |
| `--n-color-action-hover` | `#0F5B56` | `#33958B` |
| `--n-color-on-action` | `#FFFFFF` | `#FFFFFF` |
| `--n-color-focus` | `#087F78` | `#80DDD3` |

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
| Danger | `#B43A36` / `#F8E5E3` | `#FFB4AE` / `#4C2422` |
| Warning | `#8A5A12` / `#F8ECD2` | `#F0C879` / `#463717` |
| Conflict | `#9A4B17` / `#F8E6DA` | `#EFAA72` / `#4A2D1C` |
| Info | `#256789` / `#DEEDF5` | `#91CBEA` / `#183847` |
| Success | `#287052` / `#DCEEE4` | `#8FD4AD` / `#1B3C2B` |
| Canon | `#287052` / `#DCEEE4` | `#8FD4AD` / `#1B3C2B` |
| Perspectiva | `#6B55A0` / `#EBE5F6` | `#CFB8F3` / `#382D4B` |
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

- `>1180px`: topbar, navegacion primaria y workspace con Explorador, editor y
  Contexto visibles. Dos separadores ajustan las columnas laterales, permiten
  colapsarlas y conservan el editor como region flexible.
- El layout del workspace se guarda localmente por mundo en
  `nirmata.workspace.layout.<worldId>`; cambiar de mundo no mezcla anchos ni
  estados colapsados y recargar no pierde la preferencia.
- `<=1180px`: una sola region de Mundo visible mediante tabs; no se muestran
  separadores ni se desmonta el estado de las otras regiones.
- Asistente y Cambios son drawers modales independientes del grid. El asistente
  conserva visible el área de origen, usa un único scroll interno y ofrece volver
  dentro de su workflow sin convertir el drawer en una ruta primaria. Cambios no
  usa splitter vertical porque actualmente no es una fila acoplada permanente.
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
10. Tailwind consume únicamente la paleta semántica; los iconos son SVG locales
    y no se agrega un catálogo visual paralelo.
