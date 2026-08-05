# Suite de regresion para IA

**Estado:** requisito previo a implementar edicion asistida.

## Objetivo

Medir si generador, recuperacion y critico detectan contradicciones conocidas.
Sin esta suite no se puede afirmar que una segunda llamada LLM mejora la
seguridad.

## Casos minimos

1. Referencia inexistente.
2. Actor participando despues de morir.
3. Evento causado por un hecho posterior.
4. Regla codificada violada.
5. Regla semantica ignorada.
6. Creencia interna confundida con ley fisica.
7. Personaje con conocimiento secreto imposible.
8. Mencion contradictoria escondida en Markdown.
9. Regla con excepcion valida.
10. Texto libre que intenta interpretarse como mutacion sin `ChangeSet`.
11. `ChangeSet` validado contra una revision obsoleta.
12. Falso positivo: rumor contradictorio que debe coexistir con el canon.
13. Dato no especificado que no debe marcarse como falso.
14. Cambio espacial sin transicion suficiente.
15. Accion posible pero incompatible con el objetivo del actor.
16. Evento causalmente aislado que debe producir warning, no siempre error.
17. Deseo de un personaje confundido con conocimiento.
18. Discontinuidad deliberada correctamente explicada.
19. Negacion explicita confundida con dato desconocido.
20. Claims opuestos de holders distintos que deben coexistir.
21. Claims canonicos opuestos en el mismo periodo que deben entrar en conflicto.
22. Critica que rebate una conclusion frente a critica que solo cuestiona la
    fuente.
23. Retcon aditivo que no debe tratarse como reemplazo.
24. Retcon reinterpretativo que conserva la perspectiva anterior.
25. Retcon de reemplazo sin `DecisionPoint`.
26. Intervalo con fin anterior al inicio.
27. Evento ongoing tratado incorrectamente como terminado.
28. Fecha aproximada tratada como exacta.
29. Flashback que altera por error el tiempo del evento.
30. Relacion temporal derivada contradictoria con los endpoints.
31. Procedencia que cita un documento o claim inexistente.
32. Regla derrotable con excepcion mas especifica.
33. Regla de mundo cerrado aplicada a un dominio no declarado completo.
34. Excepcion intencional que conserva una decision humana trazable.

## Casos posteriores al MVP

- Dos especialistas proponiendo consecuencias incompatibles.
- Especialista que excede su presupuesto o intenta escribir.
- Sintetizador que oculta desacuerdo entre especialistas.

## Resultado esperado

Cada caso define:

- snapshot inicial;
- solicitud;
- contexto esperado;
- draft de prueba;
- issues esperados;
- severidad;
- operaciones que deben bloquearse;
- excepciones permitidas.

## Criterios

- 100% de contradicciones criticas conocidas detectadas.
- 100% de errores estructurales bloqueados.
- Ningun commit permitido con `base_revision` obsoleta.
- Rumores, mitos y propaganda no deben bloquearse como hechos.
- Las fuentes citadas deben incluir el objeto que demuestra el conflicto.
- Los vacios no deben convertirse en hechos ni contradicciones.
- Las acciones de personajes deben evaluarse contra goals y acceso epistemico.
- Los contextos epistemicos incompatibles no deben fusionarse.
- Los retcons de reemplazo deben exigir una decision humana.
- Story time y discourse order no deben confundirse.

Los warnings admiten precision imperfecta; los errores conocidos no.

## Promocion de modelos y prompts

La suite se ejecuta al cambiar:

- modelo generador;
- modelo critico;
- prompt;
- esquema de salida;
- recuperacion;
- reglas de chunking o contexto.

Un fallo critico bloquea promover la configuracion. Dos incidentes reales de la
misma categoria obligan a probar un critico de otro modelo o proveedor.

## Alcance

La suite no intenta demostrar que el sistema descubre toda contradiccion
posible. Evita regresiones conocidas y permite decidir con evidencia cuando
separar criticos, ampliar contexto o agregar validadores Rust.
