# Fundamentos academicos de Nirmata

**Revision:** 2026-08-04.

## Alcance

Se revisaron y verificaron trabajos de:

- teoria literaria de mundos posibles y storyworlds;
- narratologia cognitiva y comprension narrativa;
- narrativa computacional, planificacion y simulacion.

La seleccion privilegia articulos revisados por pares, actas academicas,
editoriales universitarias y enlaces DOI. No se incorporaron blogs como
fundamento de arquitectura.

## Conclusion consolidada

Nirmata no debe modelar una coleccion de textos. Debe modelar un **storyworld**
incompleto, versionado y autenticado, del cual los textos son representaciones.

La literatura converge en ocho principios:

1. Mundo, historia y discurso son niveles diferentes.
2. Una afirmacion necesita autoridad para convertirse en verdad ficcional.
3. Los mundos ficticios son necesariamente incompletos.
4. Personajes y facciones mantienen mundos privados de conocimiento, creencia,
   deseo y obligacion.
5. La coherencia se actualiza en tiempo, espacio, causalidad, intencion y
   continuidad de entidades.
6. Un evento narrativamente valido necesita conexiones causales e intenciones
   plausibles, no solo fechas correctas.
7. Generacion y evaluacion deben estar separadas, pero ninguna autocritica
   sustituye reglas ni revision humana.
8. La simulacion emergente gana riqueza a costa de control narrativo; por eso no
   debe escribir canon automaticamente.

## Cambios derivados

La revision modifica o confirma el diseño:

- `Claim` incorpora modalidad y acceso epistemico.
- `Goal` pasa a ser objeto de dominio.
- `Event` explicita las cinco dimensiones del event-indexing model.
- Las relaciones causales tienen tipos narrativos.
- `NULL` significa no especificado, no falso.
- Los validadores distinguen vacio, contradiccion y excepcion.
- La recuperacion reconstruye una situacion, no solo coincidencias textuales.
- El critico revisa coherencia local y global.
- El perfil multiagente sigue siendo analitico y de solo lectura.

## Documentos

- [`literary-worlds.md`](literary-worlds.md): ontologia, autenticacion, reglas y
  mundos posibles.
- [`cognitive-narratology.md`](cognitive-narratology.md): comprension,
  perspectivas, inferencia y coherencia.
- [`computational-narrative.md`](computational-narrative.md): planificacion,
  causalidad, agentes y evaluacion.
- [`design-consequences.md`](design-consequences.md): trazabilidad desde papers
  hasta decisiones de Nirmata.
- [`../critical-fronts/README.md`](../critical-fronts/README.md): cocreacion,
  versionado, conocimiento incierto y tiempo narrativo.

## Limites

Estas teorias no entregan automaticamente un esquema de base de datos ni
demuestran que un LLM detectara toda contradiccion. Sirven para:

- elegir categorias de dominio no arbitrarias;
- diferenciar controles deterministas y semanticos;
- formular casos de evaluacion;
- evitar repetir fallos conocidos de sistemas narrativos.

La eficacia real de Nirmata se medira con la suite de regresion y pruebas con
usuarios, no por cantidad de referencias.
