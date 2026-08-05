# Modelo de dominio inicial

**Estado:** propuesta inicial.

## Objetos principales

| Objeto | Responsabilidad |
|---|---|
| `World` | Premisa, tono, reglas y configuracion del universo |
| `Entity` | Persona, lugar, faccion, cultura, recurso o concepto |
| `Relation` | Conexion tipada entre entidades |
| `Event` | Cambio situado en la linea temporal |
| `Goal` | Estado deseado por una persona, faccion o institucion |
| `Rule` | Ley canonica, restriccion o principio del universo |
| `Claim` | Afirmacion atribuida a una perspectiva o fuente |
| `Document` | Carta, tratado, noticia u otro artefacto interno |
| `ChangeSet` | Modificaciones propuestas y pendientes de aprobacion |
| `DecisionPoint` | Alternativas incompatibles que requieren eleccion humana |

## Canon y perspectivas

Una misma situacion puede tener varias afirmaciones simultaneas:

- Hecho canonico: el emperador murio envenenado.
- Version oficial: murio por una enfermedad.
- Rumor: fue asesinado por su hija.
- Creencia religiosa: ascendio al cielo.

`Claim` existe para representar informacion incierta, disputada o dependiente
de una perspectiva. Los datos ordinarios deben permanecer como campos normales;
no todo necesita convertirse en una tripleta generica.

Un `Claim` debe indicar:

- contenido o proposicion;
- cuando sea validable: `predicate_key` y objeto entidad o escalar normalizados;
- polaridad positiva o negativa;
- autenticacion: `canonical`, `attributed` o `disputed`;
- holder: quien afirma, cree o supone;
- modalidad: `assertion`, `belief`, `hypothesis` o `counterfactual`;
- registro opcional: `official`, `rumor`, `myth`, `testimony` u otro;
- fuente;
- base epistemica;
- periodo de validez;
- confianza declarada por el holder.
- documento fuente o claim del que se deriva, cuando aplique;
- revision que lo registro y, si corresponde, la que lo sustituyo.

Canon y mundo privado no compiten por una unica fila verdadera. Pueden coexistir
y relacionarse.

Reglas:

- `canonical` no lleva modalidad perspectival ni holder.
- `attributed` necesita holder y modalidad.
- la confianza describe seguridad del holder, no probabilidad de verdad.
- confianza del extractor o del LLM vive en la propuesta, no en el canon.
- deseos se modelan como `Goal`; obligaciones como `Rule`.
- ausencia de claim significa desconocido, no falso.

Ejemplos:

- hecho canonico: dato estructurado o claim `canonical`;
- version oficial: `attributed` + `assertion` + registro `official`;
- rumor: `attributed` + `belief` + registro `rumor`;
- doctrina: `attributed` + `belief` + registro `myth`.

## Planos narrativos

Nirmata separa:

1. **Mundo:** entidades, reglas y estados.
2. **Historia:** eventos, causalidad, objetivos y cambios.
3. **Discurso:** documentos, escenas y versiones narradas.
4. **Diseno autoral:** tono, temas, limites y objetivos del creador.

Una regla social del mundo no es una preferencia autoral, y una frase de un
documento no se autentica automaticamente como hecho.

## Causalidad

Los acontecimientos importantes deben poder relacionarse con:

- causas inmediatas;
- causas estructurales;
- actores;
- afectados;
- consecuencias directas;
- consecuencias posteriores;
- interpretaciones publicas.

Las causas y consecuencias deben almacenarse como relaciones consultables, no
solo como parrafos generados.

Tipos iniciales de enlace causal:

- `enables`;
- `causes`;
- `motivates`;
- `prevents`;
- `terminates`;
- `reveals`.

La aplicacion puede derivar centralidad causal para recuperacion y auditoria;
no es una medida automatica de calidad.

## Objetivos e intencion

Un `Goal` contiene:

- holder;
- estado deseado;
- prioridad;
- estado: activo, logrado, abandonado o frustrado;
- periodo;
- visibilidad publica o secreta;
- fuente.

Una accion puede ser estructuralmente posible y aun no tener una motivacion
plausible. El critico revisa si los eventos de actores se conectan con objetivos
o declaran que la motivacion es desconocida.

Un deseo persistente se guarda como `Goal`, no como `Claim`. Una obligacion se
guarda como `Rule` institucional y puede originar goals o conflictos.

## Reglas

Una `Rule` contiene:

- declaracion narrativa;
- alcance;
- autoridad;
- severidad;
- excepciones;
- si es derrotable y su prioridad declarada;
- validador codificado opcional.

Categorias:

- constitutiva: que puede existir u ocurrir;
- generativa: que consecuencias produce una condicion;
- institucional: ley, costumbre o norma que puede ser violada;
- autoral: tono, tema o limite creativo fuera de la diegesis.

Solo una regla asociada a un validador Rust puede producir automaticamente un
error duro. Las reglas semanticas producen conflictos para critica y revision
humana.

Una prioridad solo se compara entre reglas codificadas del mismo alcance.
Nirmata no infiere jerarquias desde prosa.

Una regla institucional incumplida normalmente genera consecuencias; no hace
imposible el evento. Una directiva autoral tampoco debe confundirse con una ley
fisica.

## Referencias de contenido

La prosa puede mencionar objetos sin convertirlos en participantes o relaciones.
Cada cuerpo Markdown mantiene `content_references` separadas para que busqueda y
validacion incorporen esas entidades.

Cada referencia guarda objeto fuente, objeto destino y `ordinal` dentro del
contenido. El orden de discurso se deriva de ese ordinal; no requiere una
entidad `Scene` en el MVP.

En contenido generado por IA son obligatorias para toda mencion canonica
identificada. En contenido manual, el editor sugiere enlaces usando nombres y
aliases y permite resolver ambiguedades.

## Decisiones

Un `DecisionPoint` agrupa alternativas mutuamente excluyentes, sus fuentes y
consecuencias. Un `ChangeSet` con decisiones no resueltas no es aplicable.

Cuando un `replacement` cambia canon existente, el `DecisionPoint` asociado
identifica el target sustituido, registra la razon y conserva la alternativa
resuelta que habilita la confirmacion.

Cada operacion de un `ChangeSet` puede declarar:

- `additive`: completa un vacio;
- `reinterpretive`: agrega una lectura sin borrar las anteriores;
- `replacement`: sustituye canon y exige decision explicita.

## Incompletitud

Un mundo ficticio puede dejar datos sin determinar.

- `NULL`: no especificado.
- valor explicito negativo: se sabe que no existe o no ocurrio.
- claims incompatibles: conflicto o perspectivas distintas.

Los validadores no rellenan vacios. Una inferencia basada en expectativas
ordinarias se etiqueta como inferencia y requiere autenticacion humana para
convertirse en canon.

El mundo es abierto por defecto. El cierre local solo se usa para referencias,
unicidad, campos monovaluados, ciclos de vida y conjuntos declarados completos.

Claims opuestos pueden coexistir en holders, modalidades o registros distintos.
Claims canonicos opuestos en el mismo contexto y periodo producen conflicto en
el draft. El commit se bloquea si ambos seguirian activos despues de resolver
reemplazos y decisiones.

## Tiempo

La primera version usa ticks `i64` relativos al epoch del mundo y no necesita un
motor de calendarios ficticios. Un evento registra:

- tipo: desconocido, instante, intervalo u ongoing;
- inicio y final opcionales;
- precision: exacta, dia, mes, ano, era o desconocida;
- certeza: cierta, aproximada, incierta o ambas.

Las relaciones de Allen se calculan cuando los extremos son conocidos; no se
almacenan. El orden de discurso es la posicion de una referencia a evento
dentro de un documento. Un flashback no cambia el tiempo del evento.

## Dimensiones de evento

Cuando existan datos, un evento registra:

- tiempo;
- espacio;
- participantes y roles;
- enlaces causales;
- objetivos o intenciones afectados.

Una discontinuidad en una dimension puede ser deliberada. Debe explicarse o
marcarse como conflicto, no bloquearse automaticamente.

## Autoridad del canon

- La base estructurada es la fuente de verdad.
- Los documentos y textos generados son representaciones de esa verdad.
- Una propuesta de IA permanece fuera del canon hasta ser aceptada.
- Un `ChangeSet` debe aplicarse de forma atomica o no aplicarse.

## Fundamento

Ver [`../research/academic-foundations/README.md`](../research/academic-foundations/README.md).
Ver tambien [`../research/critical-fronts/README.md`](../research/critical-fronts/README.md).
