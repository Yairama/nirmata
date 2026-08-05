# Conocimiento incierto y contradictorio

**Estado:** politica de consistencia consolidada.

## Problema

Un mundo ficticio puede contener una verdad canonica, rumores incompatibles,
mentiras, hipotesis y conocimiento parcial. Tratar todo como hechos produce
falsos conflictos; permitir cualquier contradiccion destruye el canon.

## Regimen de consistencia

### Mundo abierto por defecto

Si Nirmata no contiene una afirmacion, su valor es desconocido. `NULL` y la
ausencia nunca significan falso.

### Cierre local y explicito

Solo se usa razonamiento de mundo cerrado en dominios que el validador conoce
como completos:

- existencia de una fila referenciada;
- unicidad de identificadores;
- campos definidos como monovaluados;
- estados de ciclo de vida incompatibles;
- conjuntos declarados exhaustivos por una regla codificada.

No se agrega un motor generico de completitud en el MVP.

### Contextos separados

Una afirmacion se interpreta junto a:

- autenticacion;
- modalidad;
- holder o fuente;
- registro o comunidad;
- periodo de validez.

Dos afirmaciones opuestas en contextos distintos pueden coexistir. Dos
afirmaciones canonicas activas y opuestas sobre el mismo sujeto, predicado y
periodo constituyen un conflicto durante la revision. El commit es un error si
la resolucion final conserva ambas.

## Negacion explicita

`Claim.polarity` distingue afirmacion positiva y negativa. Esto evita confundir:

- "no se sabe si A";
- "se sabe que no A".

## Procedencia minima

Cada claim puede referir:

- documento fuente;
- claim del que se deriva;
- revision que lo registro;
- revision que lo sustituyo.

La confianza describe la seguridad de la fuente o del holder, no convierte una
afirmacion en probabilidad matematica.

## Conflictos semanticos

El critico clasifica dos ataques:

- `rebuts`: una afirmacion contradice la conclusion de otra;
- `undercuts`: cuestiona su fuente, acceso o justificacion sin afirmar lo
  opuesto.

Esta distincion mejora la explicacion. No se implementa un sistema completo de
argumentacion.

## Reglas revisables

Una regla puede ser:

- dura: no admite excepciones sin modificar el canon;
- derrotables: admite excepciones registradas;
- priorizada: una regla mas especifica puede prevalecer en un alcance concreto.

La primera implementacion solo evalua prioridades declaradas entre reglas
codificadas. No intenta inferir una jerarquia por texto.

## Alternativas rechazadas

- OWL o razonador RDF.
- Datalog general.
- Revision de creencias AGM completa.
- Logica paraconsistente como motor del producto.
- Modelo BDI completo.
- Framework de argumentacion de Dung completo.
- Nested knowledge y common knowledge.

El modelo relacional contextualizado cubre los conflictos que el MVP debe
explicar.

## Fuentes seleccionadas

- Reiter, *On Closed World Data Bases* (1978):
  <https://doi.org/10.1007/978-1-4684-3384-5_3>
- Reiter, *A Logic for Default Reasoning* (1980):
  <https://doi.org/10.1016/0004-3702%2880%2990014-4>
- Alchourron, Gardenfors y Makinson, *On the Logic of Theory Change*:
  <https://doi.org/10.2307/2274239>
- Belnap, *A Useful Four-Valued Logic*:
  <https://doi.org/10.1007/978-94-010-1161-7_2>
- Dung, *On the Acceptability of Arguments*:
  <https://doi.org/10.1016/0004-3702%2894%2900041-X>
- Pollock, *Defeasible Reasoning*:
  <https://doi.org/10.1207/s15516709cog1104_4>
- Carroll et al., *Named Graphs*:
  <https://doi.org/10.1145/1060745.1060835>
- W3C, *PROV-DM*:
  <https://www.w3.org/TR/prov-dm/>
- Razniewski et al., *Completeness Management for RDF Data Sources*:
  <https://doi.org/10.1145/3184558.3186235>

## Limite de la evidencia

Estas teorias formalizan problemas mas amplios que el producto. Nirmata toma
sus distinciones utiles y evita incorporar los motores completos hasta que
casos de usuario demuestren que las reglas tipadas no bastan.
