# Capas de validacion

**Estado:** recomendacion consolidada.

## Objetivo

Ningun componente individual puede garantizar la coherencia:

- el generador puede alucinar;
- reglas deterministas no comprenden toda la semantica;
- el critico LLM puede equivocarse;
- el usuario puede revisar un snapshot obsoleto;
- la base de datos solo conoce restricciones codificadas.

La seguridad nace de capas con responsabilidades distintas.

## Resumen

```text
capacidad
  -> snapshot y contexto
  -> esquema
  -> estructura
  -> tiempo y ciclo de vida
  -> reglas codificadas
  -> impacto y conflictos
  -> critico semantico
  -> reparacion acotada
  -> revision humana
  -> revalidacion por revision
  -> transaccion y constraints
  -> auditoria
```

## 0. Frontera de capacidad

**Responsable:** capa de aplicacion.

Antes de invocar el modelo se decide el modo:

- consulta: tools de lectura;
- propuesta: tools de lectura y salida `ChangeSetDraft`;
- auditoria: tools de lectura y salida `ValidationReport`.

El modelo nunca recibe una tool de commit. Esta proteccion es de codigo, no de
prompt.

## 1. Snapshot y suficiencia de contexto

**Responsables:** aplicacion y store.

Se captura:

- `world_id`;
- `base_revision`;
- objetos seleccionados;
- reglas aplicables;
- relaciones y eventos vecinos;
- claims y perspectivas;
- goals e intenciones;
- hashes/versiones.

Todo cuerpo Markdown generado por IA debe declarar referencias estructuradas a
entidades, eventos y reglas que menciona. El contexto incluye esas referencias,
no solo las relaciones principales del objeto.

El modelo puede solicitar contexto adicional dentro de su presupuesto. Toda
fuente queda citada.

Esta capa no garantiza que el contexto sea completo, pero evita razonar sobre
una mezcla de revisiones.

## 2. Esquema de salida

**Responsable:** adaptador de IA.

Comprueba que la salida sea un `ChangeSetDraft` valido:

- tipos correctos;
- operaciones conocidas;
- IDs bien formados;
- campos requeridos;
- limites de tamano;
- referencias temporales con formato valido.
- referencias de contenido para nombres canonicos mencionados en Markdown.

Texto libre nunca se interpreta como una mutacion.

Fallo: una reparacion de esquema; despues, error visible.

## 3. Integridad estructural

**Responsable:** `nirmata-core`.

Funciones Rust simples validan:

- referencias existentes o creadas en el mismo conjunto;
- tipos de origen/destino permitidos;
- cardinalidad;
- operaciones duplicadas;
- dependencias internas;
- eliminaciones que dejan huerfanos;
- versiones esperadas.

Para contenido generado, todo enlace `nirmata://...` debe resolver. Ademas, un
escaneo exacto de nombres y aliases conocidos marca menciones sin enlazar. Una
mencion ambigua se presenta para resolucion; no se inventa automaticamente una
identidad.

No se necesita un framework de reglas ni plugins en el MVP.

## 4. Tiempo y ciclo de vida

**Responsable:** `nirmata-core`.

Antes de evaluar semantica se valida la forma temporal:

- kind unknown, instant, interval u ongoing;
- inicio anterior al fin;
- precision y certeza validas;
- relacion de Allen derivada cuando ambos extremos son conocidos.

Ejemplos:

- muerte anterior al nacimiento;
- reinado fuera de la vida conocida;
- dos estados mutuamente exclusivos simultaneos;
- evento causado por otro posterior;
- viaje que termina antes de comenzar;
- actor participando despues de una muerte canonica.
- cambio espacial imposible sin transicion;
- accion incompatible con los objetivos conocidos del actor.

Estas validaciones requieren fechas normalizadas y estados tipados. Si la
informacion solo existe en prosa, el critico semantico debe revisarla.

## 5. Reglas del universo

No todas las leyes son igualmente verificables.

### Regla codificada

Tiene un `validator_kind` implementado en Rust y parametros:

```text
Rule
- statement_md
- enforcement: hard
- validator_kind: no_resurrection
- parameters
```

Puede producir un error duro.

### Regla semantica

Ejemplo: "toda magia importante exige sacrificar un recuerdo valioso".

Su significado vive en lenguaje natural. El critico LLM la compara contra la
propuesta y genera un conflicto citado. No se debe fingir que Rust comprende
esa regla si no existe un validador implementado.

### Creencia interna

Una doctrina religiosa o version oficial no es ley fisica. Una contradiccion
puede representar propaganda, ignorancia o herejia y debe modelarse como
perspectiva, no bloquear el evento.

### Vacio

La falta de una regla o propiedad no es una contradiccion. El validador produce
`unspecified`, no `false`. Una inferencia ordinaria del critico permanece
advisory hasta que el usuario la autentique.

El mundo es abierto por defecto. El cierre local solo se usa para referencias,
unicidad, campos monovaluados, ciclos de vida y conjuntos declarados
exhaustivos.

## 6. Impacto y conflictos

**Responsables:** core y store.

Se calcula el subgrafo afectado:

- objetos escritos;
- referencias entrantes;
- eventos dependientes;
- reglas enlazadas;
- documentos que afirman hechos afectados.
- objetos mencionados mediante `content_references`.

Se detectan:

- dos operaciones sobre el mismo campo;
- relaciones incompatibles;
- claims canonicos opuestos;
- cambios que invalidan documentos;
- consecuencias obligatorias no incluidas cuando estan codificadas.
- eventos causalmente aislados;
- acciones sin intencion o motivacion conocida.

Esta validacion es incremental. Una auditoria completa del mundo es un workflow
separado.

Dos claims canonicos normalizados sobre el mismo sujeto, predicado y periodo no
pueden quedar activos con polaridades opuestas. Durante la revision es un
`conflict`; se vuelve `error` si el estado final a confirmar conserva ambos.
Cuando la proposicion solo existe como prosa y no puede normalizarse, el critico
semantico detecta el conflicto.

Un cambio `replacement` debe identificar el canon sustituido y producir un
`DecisionPoint`. Un cambio `reinterpretive` no borra claims anteriores.

## 7. Critico semantico

**Responsable:** adaptador de IA, en modo de solo lectura.

Se ejecuta para todo `ChangeSet` generado por IA despues de los validadores
deterministas.

Recibe:

- draft;
- reporte determinista;
- reglas semanticas relevantes;
- snapshot del subgrafo afectado;
- fuentes citadas.

Busca contradicciones narrativas y consecuencias omitidas. Devuelve
`CritiqueReport`; no modifica la propuesta.

La critica cubre cinco ejes:

- continuidad temporal;
- continuidad espacial;
- continuidad de entidades;
- causalidad local y global;
- intenciones, objetivos y acceso epistemico.

Una discontinuidad explicada es valida. Una dimension no especificada es un
vacio. Solo compromisos incompatibles forman una contradiccion.

La validacion epistemica comprueba que un claim atribuido tenga holder y que su
base de acceso sea compatible con el momento narrativo. No convierte una
creencia falsa en error de canon.

Cuando aplica, el reporte distingue:

- `rebuts`: contradice una conclusion;
- `undercuts`: cuestiona fuente, evidencia o acceso.

## 8. Reparacion acotada

**Responsable:** capa de aplicacion.

Si existen errores reparables:

- se realiza una unica reparacion;
- se crea una nueva version del draft;
- se repiten capas 2 a 7;
- un segundo fallo termina el bucle.

No se aplica la reparacion automaticamente.

## 9. Revision humana

**Responsables:** GUI y usuario.

Muestra:

- antes y despues;
- fuentes;
- errores, conflictos y warnings;
- supuestos;
- consecuencias;
- especialistas que fallaron;
- excepciones solicitadas.
- vacios relevantes e inferencias propuestas.

El usuario puede:

- aceptar;
- rechazar;
- editar;
- seleccionar operaciones;
- registrar una excepcion intencional.

El subconjunto elegido forma otro `ChangeSet` y vuelve a validarse.

En reemplazos, conflictos duros y cambios de impacto amplio, la resolucion
sugerida por la IA se muestra despues del juicio inicial del usuario.

## 10. Revalidacion por revision

**Responsables:** aplicacion y store.

Antes del commit se compara `base_revision` con la revision actual.

Si cambio:

- no se aplica;
- se reconstruye el snapshot;
- se invalidan el reporte determinista y el `CritiqueReport`;
- se recalculan todas las validaciones;
- se ejecuta una nueva critica semantica;
- se muestra el nuevo conflicto.

Se permite un refresco automatico. Si la revision vuelve a cambiar durante ese
refresco, la propuesta queda obsoleta y el usuario debe reiniciarla. Una critica
contra una revision anterior nunca autoriza un commit.

Esto evita confirmar una propuesta correcta para un mundo que ya no existe.

## 11. Transaccion SQLite

**Responsable:** store.

Dentro de una transaccion:

- verifica claves foraneas y constraints;
- aplica todas las operaciones;
- registra valores anteriores y posteriores;
- incrementa la revision;
- actualiza indices derivados;
- confirma todo o hace rollback.

Disco lleno, lock o constraint fallido dejan el canon intacto.

## 12. Auditoria

Se conserva:

- draft y version final;
- reportes;
- excepciones;
- fuentes;
- modelo y version del prompt;
- revision base y revision resultante.

Una auditoria profunda puede revisar periodicamente todo el mundo, pero nunca
repara automaticamente.

## Ejemplo: resurreccion contradictoria

Estado existente:

- la reina murio en el ano 430;
- una regla codificada prohibe resurreccion;
- una regla semantica indica que cruzar la muerte destruye recuerdos ajenos.

Propuesta:

> En el ano 480 la reina regresa sin consecuencias.

Flujo:

1. El parser crea un evento y una transicion de estado.
2. Integridad confirma que la reina existe.
3. Tiempo detecta que estaba muerta.
4. `no_resurrection` produce un error duro.
5. El critico senala ademas el coste magico ausente.
6. El generador tiene una oportunidad para reparar.
7. Puede proponer que no era la reina, modificar la regla mediante una excepcion
   explicita o agregar un mecanismo compatible.
8. Todo vuelve a validarse.
9. El usuario decide. Sin resolucion, SQLite nunca recibe la operacion.

## Verificacion doble real

El doble check no es "el mismo agente pensando dos veces". Es:

1. Validacion determinista sobre datos y reglas codificadas.
2. Critica semantica independiente sobre lenguaje y consecuencias.
3. Decision humana.
4. Constraints y transaccion en la fuente de verdad.

Cada capa cubre fallos que las otras no pueden detectar.

## Ejemplo: contradiccion escondida en prosa

Un documento generado dice:

> La embajadora recuerda haber hablado con la reina en el ano 450.

La reina murio en 430, pero no fue agregada como participante del documento.

Para evitar que la contradiccion quede invisible:

1. La salida generada incluye una `content_reference` a la reina.
2. El escaneo de aliases detecta su nombre si el modelo omitio el enlace.
3. El contexto del critico incorpora la vida y muerte de la reina.
4. El critico marca el encuentro como conflicto temporal.
5. El usuario puede corregir la fecha, convertirlo en rumor o explicar una
   identidad equivocada.
