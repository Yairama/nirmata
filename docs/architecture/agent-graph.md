# Grafo de ejecucion y agentes

**Estado:** recomendacion revisada tras auditar `deep-agents-from-scratch`.

## Decision

Nirmata debe tener dos perfiles:

1. **Estandar:** un generador, validadores deterministas, un critico semantico y
   revision humana.
2. **Profundo:** especialistas aislados en paralelo, sintesis, las mismas
   validaciones y revision humana.

El perfil profundo es multiagente. No es el motor predeterminado de cada
pregunta o edicion.

El MVP implementa solamente el perfil estandar. El perfil profundo se agrega
cuando la suite y el flujo de revision simple ya sean fiables.

## Por que la decision cambio

La propuesta anterior dejaba el critico LLM como opcional para cambios de alto
impacto. Eso exige clasificar correctamente el impacto antes de revisar y puede
dejar pasar contradicciones semanticas en cambios aparentemente pequenos.

La regla revisada es mas simple:

> Todo `ChangeSet` generado por IA recibe una segunda lectura semantica antes de
> mostrarse como aplicable.

Las ediciones manuales usan validacion determinista y pueden solicitar una
revision semantica bajo demanda.

## Perfil estandar: consulta

```text
solicitud
  -> snapshot del mundo
  -> recuperar contexto
  -> responder con citas
  -> fin
```

- Solo dispone de herramientas de lectura.
- No crea ni aplica operaciones.
- Puede ampliar contexto dentro de un presupuesto acotado.
- No necesita un critico separado porque no modifica el canon.

## Perfil estandar: edicion

```text
solicitud
  -> snapshot del mundo
  -> recuperar contexto
  -> generador
  -> parsear ChangeSetDraft
  -> validadores Rust
  -> critico semantico
       sin conflictos -> revision humana
       con conflictos -> una reparacion -> validar y criticar otra vez
  -> revision humana
  -> revalidar contra revision actual
  -> transaccion SQLite
  -> fin
```

El generador y el critico pueden usar el mismo modelo inicialmente, pero:

- reciben prompts distintos;
- usan contextos separados;
- el critico no recibe razonamiento privado del generador;
- el critico no puede modificar el `ChangeSet`;
- el critico devuelve un `CritiqueReport` tipado.

Un segundo modelo solo se justifica si una evaluacion demuestra errores
correlacionados que el mismo modelo no detecta.

## Bucle de reparacion

Solo existe un bucle autonomo:

```text
draft -> validar -> criticar -> reparar -> validar -> criticar
```

Limites:

- una reparacion de esquema;
- una reparacion de dominio/semantica;
- dos revisiones semanticas como maximo;
- despues del segundo fallo se muestra el conflicto al usuario.

Los contadores viven en Rust. No son instrucciones dentro de un prompt.

## Perfil profundo: impacto multiagente

Se utiliza cuando el usuario pide analizar consecuencias amplias, simular una
crisis o explorar varias disciplinas.

```text
                            -> historiador ----\
snapshot -> contexto base  -> economista ------> informes -> sintesis
                            -> politologo ------/              |
                                                              v
                    revision humana <- critico <- validar <- ChangeSetDraft
```

### Especialistas

Cada especialista:

- recibe una tarea cerrada;
- obtiene solo el contexto relevante;
- tiene herramientas de lectura;
- no puede delegar en otros agentes;
- dispone de un presupuesto fijo;
- devuelve un `SpecialistReport` tipado.

No devuelve directamente escrituras al canon.

```text
SpecialistReport
- specialist
- findings
- affected_object_ids
- candidate_consequences
- assumptions
- evidence
- confidence
- unresolved_questions
```

### Por que informes y no fragmentos de `ChangeSet`

Permitir que varios agentes escriban operaciones independientes crea IDs
duplicados, escrituras al mismo campo y dependencias dificiles de fusionar.

Un unico sintetizador convierte los informes en un `ChangeSetDraft`. Debe citar
que hallazgos originan cada operacion y conservar desacuerdos como alternativas,
no resolverlos silenciosamente.

Los desacuerdos se representan como `DecisionPoint`s tipados. Una propuesta con
decisiones pendientes no puede confirmarse hasta que el usuario seleccione una
alternativa o descarte el cambio relacionado.

### Seleccion de especialistas

La primera version usa seleccion explicita del usuario o reglas del caso de uso.
No necesita un agente coordinador libre.

Ejemplos:

- recurso o comercio: economista;
- guerra o sucesion: historiador y politologo;
- ritual o tabues: antropologo y teologo;
- cambio geografico: geografo y economista.

Maximo inicial: cuatro especialistas. El limite se aplica en codigo.

## Perfil profundo: auditoria

El mejor uso del multiagente en Nirmata es una auditoria de solo lectura:

```text
canon
  -> auditor temporal
  -> auditor de reglas
  -> auditor causal
  -> auditor de perspectivas
  -> consolidar ValidationReport
  -> usuario
```

Los auditores buscan problemas; no generan canon. El usuario puede convertir un
hallazgo en una propuesta separada.

Esto aprovecha el paralelismo sin permitir que varios agentes compitan por
escribir la fuente de verdad.

El critico del perfil estandar puede recomendar una revision profunda cuando
detecte varias disciplinas afectadas, contexto insuficiente o conflictos de
alta incertidumbre. La aplicacion no la ejecuta silenciosamente: muestra coste,
motivo y especialistas sugeridos.

## Estado de una ejecucion

```text
Run
- id
- world_id
- base_revision
- mode: query | edit | deep_impact | audit
- request
- selected_objects
- context_bundle
- specialist_reports
- draft
- validation_report
- critique_report
- schema_repair_count
- semantic_repair_count
- decision_points
- status
- error
```

Los estados y transiciones son tipos Rust. Una ejecucion no avanza si falta el
artefacto requerido por el siguiente nodo.

## Fallo de especialistas

- Cada especialista tiene timeout.
- Un fallo queda registrado y visible.
- Si uno falla, los demas pueden continuar.
- Si todos fallan, la ejecucion termina sin propuesta.
- No se reemplaza un fallo con contenido de relleno que parezca exitoso.

## Reasoning y herramientas

Un nodo puede ejecutar un micro-bucle ReAct de lectura para pedir contexto
adicional. No puede escribir ni continuar indefinidamente.

Presupuesto inicial:

- dos expansiones de recuperacion;
- seis llamadas de herramienta;
- cero delegaciones anidadas;
- una salida final tipada.

Ver [`reasoning-policy.md`](reasoning-policy.md).

## Framework

No se adopta LangGraph ni `deepagents` dentro del nucleo:

- Nirmata ya necesita dominio, validadores y transacciones en Rust;
- separar la orquestacion en Python duplicaria tipos y manejo de errores;
- el grafo interactivo tiene pocos estados y limites conocidos.

Un proceso LangGraph externo podria justificarse para una futura importacion
masiva o auditoria que deba sobrevivir reinicios. Incluso entonces solo tendria
acceso de lectura y devolveria propuestas tipadas.

## Condicion de terminacion

Toda ruta termina por una de estas causas:

- respuesta completada;
- propuesta lista para revision;
- cambio confirmado;
- usuario cancelo;
- presupuesto agotado;
- error visible;
- conflicto no resuelto.

No existe el estado "seguir pensando hasta que parezca correcto".

El presupuesto de reparacion no crece con la cantidad de especialistas. En un
draft grande, los conflictos restantes se convierten en decisiones humanas en
vez de disparar mas bucles LLM.
