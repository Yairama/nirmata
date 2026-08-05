# Revision de `deep-agents-from-scratch`

**Repositorio:** [langchain-ai/deep-agents-from-scratch](https://github.com/langchain-ai/deep-agents-from-scratch)

**Revision analizada:** `2edfe194c097178d4eed3e0cef7b06a96459db5e`.

## Veredicto

Es un curso valioso sobre **context engineering**, no una arquitectura lista
para proteger el canon de Nirmata.

Ensena tres patrones utiles:

- planificacion mediante TODO;
- descarga de contexto a un VFS;
- aislamiento mediante subagentes.

No implementa:

- validacion de dominio;
- aprobacion humana en el grafo;
- checkpointing;
- persistencia durable;
- merge semantico;
- limites duros de presupuestos;
- tests automatizados.

## Arquitectura real

Los notebooks construyen un unico agente ReAct mediante
`langchain.agents.create_agent`. El modelo llama herramientas hasta responder o
alcanzar el limite del runtime.

No existe un `StateGraph` de dominio escrito explicitamente. LangGraph funciona
como runtime del agente prefabricado.

## Estado

[`state.py`](https://github.com/langchain-ai/deep-agents-from-scratch/blob/main/src/deep_agents_from_scratch/state.py)
extiende `AgentState` con:

- una lista `todos`;
- un diccionario `files`.

`TypedDict` aporta typing estatico, pero no valida invariantes en runtime.

## VFS

[`file_tools.py`](https://github.com/langchain-ai/deep-agents-from-scratch/blob/main/src/deep_agents_from_scratch/file_tools.py)
implementa:

- `ls`;
- `read_file`;
- `write_file`.

Los archivos son strings dentro del estado del agente. No son persistencia,
canon, transacciones ni control de versiones.

El README y `CLAUDE.md` mencionan `edit_file`, pero esa funcion no existe en el
codigo revisado.

## TODO

[`todo_tools.py`](https://github.com/langchain-ai/deep-agents-from-scratch/blob/main/src/deep_agents_from_scratch/todo_tools.py)
reemplaza la lista completa de tareas.

Reglas como "solo una tarea in progress" viven en el prompt, no en codigo. El
modelo puede incumplirlas y ninguna funcion lo impide.

Para Nirmata, el concepto se adapta como `RunPlan` tipado; un TODO generado por
LLM solo sirve para mostrar progreso.

## Subagentes

[`task_tool.py`](https://github.com/langchain-ai/deep-agents-from-scratch/blob/main/src/deep_agents_from_scratch/task_tool.py)
aporta el patron mas reutilizable:

```text
padre
  -> tarea cerrada
  -> subagente con historial limpio
  -> resultado resumido
  -> padre
```

Aspectos positivos:

- aislamiento de contexto;
- herramientas distintas por especialista;
- posibilidad de ejecutar tareas independientes.

Limitaciones:

- devuelve texto libre;
- fusiona archivos por ultima escritura;
- no tiene merge de dominio;
- no captura errores alrededor de `sub_agent.invoke`;
- presupuestos y concurrencia se expresan en prompts;
- los subagentes comparten la estructura de estado completa.

Nirmata adopta aislamiento, pero exige `SpecialistReport` tipado, timeout,
presupuesto Rust y herramientas de solo lectura.

## `think_tool`

[`research_tools.py`](https://github.com/langchain-ai/deep-agents-from-scratch/blob/main/src/deep_agents_from_scratch/research_tools.py)
implementa la supuesta reflexion como:

```python
return f"Reflection recorded: {reflection}"
```

No verifica la calidad de la reflexion ni cambia el flujo. Es una pausa
solicitada al modelo, no una autocritica independiente.

Nirmata puede registrar una traza de decision, pero su doble check debe ser un
critico separado y validadores de dominio.

## Errores

La funcion de resumen usa un `except Exception` y devuelve contenido de relleno
con forma exitosa cuando falla.

Esto es tolerable para un resumen web descartable. Es inaceptable para canon:
un error de esquema, red o dominio debe ser visible y tipado.

## Bucles y limites

Los limites de:

- busquedas;
- delegaciones;
- iteraciones;
- concurrencia;
- criterio para detenerse;

estan principalmente en texto de prompts. El agente completo no implementa
contadores propios.

Nirmata debe aplicar cada presupuesto en Rust.

## Documentacion desincronizada

En la revision realizada:

- `CLAUDE.md` menciona `react_agent.py`;
- menciona `studio_react_agent.py`;
- menciona `langgraph.json`;
- README y `CLAUDE.md` mencionan `edit_file`;

pero estos elementos no existen en el arbol actual. Tampoco existe una suite de
tests.

Esto confirma que debe tratarse como material educativo, no como dependencia o
referencia de produccion completa.

## Que adoptar

| Patron | Decision para Nirmata |
|---|---|
| Contexto aislado para especialistas | Adoptar |
| TODO como recitacion | Adaptar a estado tipado |
| VFS para descargar contexto | Adaptar al VFS logico sobre SQLite |
| Salida estructurada | Adoptar y hacer obligatoria |
| ReAct libre como motor global | Rechazar |
| `think_tool` como validacion | Rechazar |
| Limites escritos solo en prompts | Rechazar |
| Subagentes escribiendo archivos compartidos | Rechazar |
| Fallback silencioso ante errores | Rechazar |

## Diferencia esencial

`deep-agents-from-scratch` optimiza una investigacion desechable y de horizonte
largo. Nirmata modifica una fuente de verdad persistente.

Por eso Nirmata necesita controles ausentes en el curso:

- tipos de dominio;
- snapshot y version;
- validadores deterministas;
- critico semantico independiente;
- revision humana por operacion;
- transaccion atomica;
- historial y excepciones.

## Uso futuro posible

LangGraph o `deepagents` pueden ser razonables para un proceso externo de:

- importacion masiva de novelas;
- auditoria profunda que dura horas;
- investigacion web para enriquecer referencias;
- workflows que deban reanudarse tras reinicios.

Ese proceso debe ser de solo lectura sobre el canon y devolver candidatos
tipados. No reemplaza al motor Rust de validacion y commit.
