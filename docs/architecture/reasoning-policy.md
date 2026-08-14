# Politica de reasoning, bucles y autocritica

**Estado:** recomendacion consolidada.

## Principio

El reasoning ayuda a explorar; no concede autoridad.

Nirmata no necesita almacenar cadenas privadas de pensamiento. Necesita guardar
evidencia verificable:

- que contexto se consulto;
- que herramientas se usaron;
- que supuestos se declararon;
- que validadores fallaron;
- que reparaciones se intentaron;
- que decision tomo el usuario.

## Planificacion

Para solicitudes largas, el sistema puede crear un plan tipado:

```text
RunPlan
- goal
- steps
- selected_specialists
- retrieval_budget
- completion_conditions
```

El plan lo produce la aplicacion con reglas simples y, si hace falta, una
propuesta del modelo. Rust valida presupuestos y capacidades.

Una lista TODO escrita por el modelo puede mostrarse como progreso, pero no
controla el workflow.

## Micro-bucle de recuperacion

El generador puede descubrir que falta contexto:

```text
contexto inicial
  -> modelo solicita lectura
  -> aplicacion valida la consulta
  -> recupera datos
  -> modelo solicita otra lectura o produce salida
```

Restricciones:

- herramientas solo de lectura;
- consultas parametrizadas;
- profundidad y cantidad limitadas;
- maximo dos expansiones;
- maximo seis tool calls;
- cancelacion y timeout;
- sin subagentes dentro de subagentes.

El modelo no elige SQL ni rutas fisicas. Solicita operaciones de dominio como
`get_entity`, `get_related_events` o `search_claims`.

## Autocritica del generador

Pedir al mismo generador "revisa tu respuesta" dentro del mismo turno tiene
valor limitado: conserva los mismos sesgos y contexto.

Puede exigirse que la propuesta declare:

- supuestos;
- incertidumbres;
- consecuencias no resueltas;
- evidencia usada.

Esto mejora explicabilidad, pero no cuenta como doble check.

## Critico independiente

El doble check real es otra invocacion con un contrato distinto:

```text
CritiqueReport
- issues
- affected_operations
- evidence
- severity
- category
- attack_type?
- target_claim_id?
- confidence
- suggested_resolution
- recommend_deep_review
- suggested_specialists
```

Categorias:

- contradiccion de canon;
- ley del universo;
- conflicto temporal;
- ciclo causal;
- conocimiento imposible para una perspectiva;
- consecuencia faltante;
- ambiguedad o evidencia insuficiente.

Cuando aplica, `attack_type` distingue:

- `rebuts`: contradice la conclusion;
- `undercuts`: cuestiona evidencia, fuente o acceso.

El critico:

- recibe el snapshot, reglas relevantes y draft;
- busca fallos, no intenta ser creativo;
- no edita el draft;
- no decide que entra al canon;
- debe citar IDs y operaciones.

En cambios de alto riesgo, `suggested_resolution` y `confidence` se muestran
despues de que el usuario registra su juicio inicial. La evidencia que demuestra
un error duro nunca se oculta.

## Reparacion

Si hay problemas reparables:

1. La aplicacion entrega al generador solo el reporte estructurado.
2. El generador produce una nueva version completa del draft.
3. Se repiten todos los validadores.
4. Se ejecuta una segunda y ultima critica.

Una reparacion parcial no se mezcla a ciegas con el draft anterior.

## Severidad

| Severidad | Efecto |
|---|---|
| `error` | No puede confirmarse |
| `conflict` | Requiere resolver o registrar una excepcion explicita |
| `warning` | Puede confirmarse tras reconocimiento |
| `info` | Explicacion, inferencia o mejora opcional |

Un hallazgo LLM no debe ser un `error` no anulable por si solo. Los errores
duros provienen de esquema, invariantes implementados o restricciones SQLite.

## Excepciones intencionales

Una contradiccion puede ser una revelacion, retcon o ruptura deliberada de una
ley. El sistema debe permitirla sin fingir que no existe:

- el usuario marca la excepcion como intencional;
- explica la razon;
- modifica la regla o registra una excepcion;
- el `ChangeSet` incluye ambas operaciones;
- la decision queda en el historial.

## Trazabilidad

Se guarda una traza resumida:

```text
DecisionTrace
- run_id
- model_and_prompt_version
- context_object_ids
- tool_calls
- validation_reports
- critique_reports
- retry_counts
- user_resolution
- operation_decision_metrics
```

No se guarda reasoning oculto del proveedor ni texto innecesario que pueda
contener datos sensibles.

La interfaz puede mostrar una traza operativa resumida sin presentarla como
cadena de pensamiento: fase del workflow, tiempo transcurrido, fragmentos
recibidos, validaciones y reintentos. Antes del primer fragmento no puede
distinguir si el proveedor sigue razonando o quedo bloqueado; debe declarar esa
incertidumbre en lugar de inventar una explicacion.

Las metricas locales distinguen aceptar, editar y rechazar; sirven para detectar
automatizacion excesiva y fatiga de revision.

## Evaluacion

La calidad de la autocritica debe medirse con casos conocidos:

- contradiccion obvia;
- contradiccion semantica;
- falso positivo;
- regla con excepcion;
- evento temporal imposible;
- perspectiva que conoce informacion secreta;
- consecuencia indirecta omitida.

Si un solo critico falla repetidamente en una categoria, recien entonces se
separa en criticos especializados o se prueba otro modelo.

Reglas operativas:

- cada cambio de modelo o prompt ejecuta la suite de regresion;
- todos los casos criticos conocidos deben ser detectados;
- un fallo critico bloquea promover esa configuracion;
- dos fallos reales de la misma categoria obligan a evaluar un modelo de
  critico distinto al generador.

Ver [`../validation/ai-regression-suite.md`](../validation/ai-regression-suite.md).
