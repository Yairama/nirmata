# Lenguaje de producto

**Estado:** especificacion aplicada y cubierta por el gate de copy del frontend.

## Principios

- Nombrar la tarea del autor, no el DTO, indice o mecanismo interno.
- `Aplicar al mundo` es la unica accion de commit.
- Ausencia de evidencia no se presenta como falsedad ni error.
- Version activa de escritura y version observada siempre se nombran por
  separado.
- UUID, URI, ticks, JSON y DSL viven en `Detalles tecnicos`.

## Equivalencias

| Interno o actual | Producto |
|---|---|
| Guardar draft | Preparar cambios |
| Draft / ChangeSet | Propuesta / conjunto de cambios |
| Confirmar / commit | Aplicar al mundo |
| ManualReview | Revision de cambios |
| Create / Update | Crear / Modificar |
| stale | Propuesta desactualizada |
| waiver | Advertencia aceptada con motivo |
| DecisionPoint | Decision pendiente |
| head / cabeza | Version actual / ultima version |
| active variant | Escribiendo en |
| read scope | Viendo |
| merge | Traer cambios de otra variante |
| FTS | Buscar |
| VFS logico | Explorador del mundo |
| query | Preguntar |
| propose | Proponer cambios |
| IntentBrief | Resumen de intencion |
| critic | Comprobacion final |
| lore | Material del mundo |
| staging | Copia temporal de importacion |
| chunk | Fragmento de fuente |
| candidate | Elemento encontrado |
| claim | Afirmacion |
| holder | Quien la sostiene |
| goal | Meta |
| epoch | Origen del calendario |
| tick / sub-tick | Unidad temporal, solo en detalles técnicos |
| weekdays | Días de la semana |
| `nombre|días` | Filas de mes con campos Nombre y Días |
| Story time | Orden cronologico |
| discourse | Orden en que se cuenta |
| Loose ends | Cabos abiertos |

Los antiguos campos `Kind`, `Request`, `Scope`, `Factions`, `Resources`,
`Stocks`, `Rules`, `Assumptions`, `Max steps`, `Mapping`, `Before`, `After`,
`Requested`, `Applied` y `Final stocks` ya se presentan con lenguaje de producto
en sus superficies React.

## Versiones

- **Escribiendo en:** variante que recibira los proximos cambios.
- **Viendo:** version cuyos datos aparecen en busqueda, editor y contexto.
- **Version anterior:** fotografia inmutable de solo lectura.
- **Traer cambios de B hacia A:** prepara una propuesta en A; B no cambia.
- **Deshacer:** crea una nueva version inversa; no borra historia.

```text
Canon principal: r0 -> r1 -> r2 (version actual)
                         \
Variante alternativa:    r1 -> r3 -> r4

Traer alternativa hacia principal:
r4 -> propuesta -> revision -> Aplicar al mundo -> r5 en principal
```

## Gate de copy

El flujo basico no puede contener `FTS`, `VFS`, `head`, `stale`, `waiver`,
`DecisionPoint`, `Create`, `Update`, `Story time`, `Loose ends`, `Factions`,
`Resources`, `Stocks` ni `Max steps`. Una aparicion en `Detalles tecnicos` debe
estar justificada y no ser necesaria para completar la tarea.

En calendario y eventos, el flujo básico usa filas reordenables y campos
separados de año, mes, día y unidad. La cronología sin calendario dice `Tiempo
conocido sin calendario de presentación`; no inventa una fecha ni muestra el
tick canónico.
