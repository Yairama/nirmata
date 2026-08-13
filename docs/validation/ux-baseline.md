# Baseline interna de usabilidad

**Estado:** protocolo aprobado; medicion pendiente.

## Objetivo

Medir tareas humanas sobre la interfaz anterior al rediseño sin sustituir
comprension por tests de codigo. El resultado servira como baseline de UX-009 y
se repetira en UX-076.

## Entorno

- Build Tauri vigente y proyecto temporal descartable.
- Viewport principal `960x680`; repetir bloqueos en `720x520`.
- Sin telemetria remota.
- Registrar pantalla o un log cronologico por intento.
- Un fallo del fixture se excluye y se publica; abandono o bloqueo del producto
  cuenta como fallo.

## Tareas

| ID | Consigna | Exito observable | Bloqueo conocido |
|---|---|---|---|
| B1 | Crear un mundo con nombre, premisa y epoca; cerrarlo y reabrirlo. | El archivo reabierto conserva nombre y premisa. | El boton final queda bajo el fold; `epoch` no se explica. |
| B2 | Encontrar una entidad nombrada y abrir su ficha. | Editor, contexto y recientes muestran la misma entidad. | El workspace aparece despues de laboratorios; `FTS` y `VFS` son tecnicos. |
| B3 | Cambiar su resumen sin aplicarlo. | La revision muestra antes/despues y canon conserva el valor anterior. | `draft` puede confundirse con guardado canonico. |
| B4 | Revisar y aplicar el cambio. | La revision desaparece, nace una revision y reapertura conserva el cambio. | Bloqueos y estados mezclan ingles, IDs y conceptos internos. |
| B5 | Deshacer el ultimo cambio sin borrar historia. | Nace una revision inversa y ambas siguen auditables. | Historial y cambios pendientes compiten en un panel. |

## Registro

```text
session_id:
build_commit:
fecha:
viewport:
task_id:
inicio_segundos:
fin_segundos:
completada: si | no
primera_superficie_elegida:
desvios:
errores_de_entrada:
bloqueos_del_producto:
intervenciones:
confirmaciones_de_descarte:
lenguaje_literal_usado:
observaciones:
```

## Metricas

Publicar por tarea el numerador y denominador de exito, mediana y rango de
tiempo, primer intento correcto, desvios, errores, bloqueos e intervenciones.
No agregar los resultados en una metrica de vanidad.

## Evidencia disponible

La auditoria visual registrada en [`../../backlog.md`](../../backlog.md)
confirma cualitativamente los bloqueos de B1 y B2 en `960x680` y `390x844`.
Los tiempos y tasas permanecen desconocidos hasta ejecutar sesiones moderadas;
no se imputan desde tests automatizados.
