# Flujo de IA

**Estado:** resumen consolidado.

La IA propone cambios; nunca escribe directamente en la base de datos.

## Pipeline

```text
modo explicito y accion del usuario
-> seleccion de contexto relevante
-> instruccion especializada
-> respuesta estructurada
-> validacion determinista
-> critica semantica independiente
-> vista previa
-> aprobacion del usuario
-> transaccion
```

## Contexto

El modelo no debe recibir el mundo completo. El contexto se construye usando:

- entidades seleccionadas;
- relaciones directas;
- eventos cercanos;
- afirmaciones relevantes;
- resultados de busqueda textual.

La recuperacion explicita, relaciones SQL y FTS5 son suficientes inicialmente.
Los embeddings se agregan solo si una evaluacion demuestra fallos de recuerdo
semantico.

## Roles iniciales

1. **Arquitecto:** ayuda a definir premisa, temas y leyes.
2. **Historiador:** propone causas, consecuencias y cambios temporales.
3. **Guardian del canon:** busca contradicciones y problemas de continuidad.

Estos roles son configuraciones sobre el mismo pipeline, no procesos autonomos
ni crates separados. La evolucion posterior recomendada es fan-out/fan-in, no
una red conversacional ciclica.

## Validacion

Antes de solicitar una revision semantica al modelo, Nirmata debe comprobar de
forma determinista:

- referencias inexistentes;
- fechas incompatibles;
- relaciones duplicadas;
- violaciones de reglas con validador Rust;
- modificaciones que dejan datos huerfanos.

Todo cambio generado por IA recibe una critica semantica separada. El critico
revisa reglas narrativas y consecuencias; no puede editar ni ser la unica
autoridad.

## Propuestas

Cada respuesta que pueda cambiar el mundo debe convertirse en un `ChangeSet`
tipado, mostrar sus fuentes de contexto y permitir aceptar o rechazar cambios
individuales antes de aplicar una transaccion.

Una consulta nunca genera escritura. El usuario selecciona explicitamente
`Consultar` o `Proponer cambio`.

## Detalle

- [`agent-graph.md`](agent-graph.md): estados, nodos y evolucion multiagente.
- [`reasoning-policy.md`](reasoning-policy.md): presupuestos y reparacion acotada.
- [`validation-pipeline.md`](validation-pipeline.md): responsables y orden de checks.
- [`interaction-model.md`](interaction-model.md): contrato de la GUI.
- [`retrieval.md`](retrieval.md): construccion del contexto.
