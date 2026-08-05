# Fases de construccion

**Estado:** propuesta inicial.

## Fase 1: canon manual

Entidades, relaciones, eventos, busqueda y persistencia. Esta fase valida el
modelo sin depender de IA. Incluye revisiones lineales, auditoria y undo.

## Fase 2: IA controlada

Contexto acotado y pipeline estandar completo:

```text
generador
  -> validadores Rust
  -> critico semantico
  -> revision humana
  -> revalidacion
  -> transaccion
```

La fase incluye la suite de regresion del perfil estandar. Vista previa y
aprobacion sin estas capas no cuentan como edicion segura.

## Fase 3: causalidad

Causas, consecuencias, intervalos, impacto temporal y deteccion de
contradicciones contextuales.

## Fase 4: especialistas

Economista, geografo, antropologo y otros roles reutilizan el pipeline
existente. Solo se agregan los que resuelvan casos concretos.

## Fase 5: simulacion limitada

Se simula un dominio acotado, por ejemplo facciones y recursos. No se intenta
crear un simulador universal.

## Fase 6: narrativa

Extraccion de historias emergentes y generacion de documentos internos a partir
del canon existente.

## Despues del MVP, solo con evidencia

Ramas, merge, calendarios ficticios y consultas historicas completas son
evoluciones independientes. Ninguna se implementa como requisito oculto de las
fases anteriores.

## Criterio de avance

Cada fase debe demostrar utilidad por si sola. Una fase posterior no justifica
complejidad anticipada en la anterior.
