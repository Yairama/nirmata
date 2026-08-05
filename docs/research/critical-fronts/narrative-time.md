# Tiempo narrativo

**Estado:** representacion minima recomendada.

## Dos ejes

Nirmata debe separar:

- **story time:** cuando sucede un evento en el mundo;
- **discourse order:** cuando y en que orden un documento lo presenta.

Un flashback cambia el segundo eje, no el primero. El orden de discurso se
representa por el `ordinal` de referencias a eventos dentro de un documento; no
requiere duplicar eventos ni crear una entidad `Scene` en el MVP.

## Tiempo interno

Cada mundo define un epoch y usa ticks `i64` sobre un eje monotono. El tick es
interno y no presupone calendario gregoriano.

```text
EventTime
- kind: unknown | instant | interval | ongoing
- start_tick?
- end_tick?
- precision: exact | day | month | year | era | unknown
- certainty: certain | approximate | uncertain | approximate_uncertain
```

Reglas:

- `instant` tiene un solo tick;
- `interval` requiere inicio y fin ordenados;
- `ongoing` puede no tener fin;
- `unknown` no inventa una posicion;
- precision y certeza describen el dato, no alteran el orden interno.

## Intervalos

Las relaciones de Allen se implementan como una funcion pura de Rust para
intervalos con extremos conocidos:

- before / after;
- meets / met-by;
- overlaps / overlapped-by;
- starts / started-by;
- during / contains;
- finishes / finished-by;
- equals.

No se guardan relaciones derivables. Una relacion relativa sin fechas solo se
persistira cuando aparezca un caso de uso que no pueda expresarse con ticks.

## Fluents

Nirmata no necesita un Event Calculus general. Los validadores compilan solo
estados concretos que ya necesita el dominio:

- vivo o muerto;
- rol vigente;
- posesion vigente;
- relacion activa.

Cada estado tiene reglas explicitas de inicio y terminacion.

## Tiempo del canon

El tiempo del mundo no se mezcla con el historial editorial:

- ticks e intervalos: tiempo valido;
- revisiones: tiempo de transaccion del canon.

Un retcon puede cambiar la representacion actual de un evento sin afirmar que
el evento ocurrio en la fecha de edicion.

## Calendarios ficticios

La conversion `tick <-> fecha mostrada` es una capa futura. El MVP puede mostrar
ticks o etiquetas autorales y conservar precision/certidumbre. No debe incluir
un motor de calendarios antes de que un mundo real lo necesite.

## Alternativas rechazadas

- constraint network temporal completa;
- Event Calculus general;
- CTL o logica temporal modal;
- branching time formal;
- SQL:2011 temporal;
- distribuciones probabilisticas de fechas;
- TimeML como formato interno.

## Fuentes seleccionadas

- Allen, *Maintaining Knowledge about Temporal Intervals*:
  <https://doi.org/10.1145/182.358434>
- Kowalski y Sergot, *A Logic-Based Calculus of Events*:
  <https://doi.org/10.1007/BF03037383>
- Jensen et al., *The Consensus Glossary of Temporal Database Concepts*:
  <https://doi.org/10.1145/140979.140996>
- Jensen y Snodgrass, *Temporal Data Management*:
  <https://doi.org/10.1109/69.755613>
- Library of Congress, *Extended Date/Time Format*:
  <https://www.loc.gov/standards/datetime/>
- Dershowitz y Reingold, *Calendrical Calculations*:
  <https://doi.org/10.1017/CBO9781107051119>
- Sternberg, *Telling in Time*:
  <https://doi.org/10.2307/468572>

## Limite de la evidencia

La teoria temporal ofrece representaciones mucho mas expresivas. La seleccion
anterior cubre cronologia, incertidumbre basica, duracion y anacronias sin
convertir el MVP en un solver temporal.
