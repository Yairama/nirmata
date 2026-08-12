# Corte vertical de validacion

**Estado:** implementado en
`crates/nirmata-app/tests/vertical_slice.rs`.

## Escenario

Existe un imperio, una ciudad minera, una religion y un mineral capaz de
almacenar recuerdos. La mina colapsa.

Nirmata debe:

1. Identificar las entidades relacionadas.
2. Proponer consecuencias economicas, politicas y religiosas.
3. Explicar que datos del mundo sustentan cada propuesta.
4. Permitir aceptar, editar o rechazar cada cambio.
5. Aplicar los cambios aceptados de forma atomica.
6. Actualizar la linea temporal y las relaciones causales.
7. Mantener simultaneamente el hecho canonico y las interpretaciones internas.
8. Detectar contradicciones introducidas por el cambio.
9. Relacionar decisiones de facciones con objetivos conocidos o declarar la
   motivacion como no especificada.
10. No inventar datos para completar vacios del mundo.

## Resultado esperado

El usuario puede seguir la cadena desde el colapso hasta cada consecuencia y
entender por que el mundo cambio.

Si este escenario funciona con claridad, trazabilidad y control, la tesis
principal de Nirmata queda validada. Si no funciona, agregar mas agentes o una
simulacion mayor solo producira lore incoherente con mayor rapidez.
