# Cocreacion humano-IA

**Estado:** evidencia aplicada al diseno.

## Pregunta

Como puede Nirmata ayudar a crear sin convertir al usuario en aprobador pasivo
de texto generado?

## Hallazgos

### Modos explicitos

La iniciativa mixta funciona mejor cuando el usuario entiende quien actua, con
que alcance y cuando puede intervenir. La separacion entre `Consultar` y
`Proponer` es correcta:

- `Consultar` explora sin preparar escritura.
- `Proponer` interpreta una intencion y genera un `ChangeSetDraft`.
- `Revision profunda` aumenta evidencia y especialistas, no autoridad.

No hay evidencia suficiente para convertir la revision profunda en el modo por
defecto.

### Evitar anclaje y aceptacion automatica

Las explicaciones de una IA pueden aumentar la aceptacion incluso cuando la
respuesta es incorrecta. Para cambios de alto riesgo conviene separar:

1. lectura humana del cambio;
2. juicio inicial;
3. recomendacion o resolucion propuesta por la IA.

Esta friccion no debe imponerse en cada edicion. Se reserva para reemplazos de
canon, conflictos duros y cambios de impacto amplio.

### Control estructurado antes que prosa cerrada

Los sistemas de coescritura mas controlables permiten dirigir estructura,
alcance o fragmentos antes de producir una pieza completa. En Nirmata esto no
requiere nuevos sliders ni un editor especial: basta un resumen editable de
intencion para solicitudes amplias:

```text
IntentBrief
- objetivo
- alcance
- entidades implicadas
- restricciones que preservar
```

Solicitudes pequenas pueden pasar directamente al draft.

### Revision progresiva

Mas alternativas favorecen ideacion, pero perjudican eficiencia. La interfaz
debe:

- mostrar dos o tres alternativas o `DecisionPoint`s inicialmente;
- resumir cada hallazgo por severidad y una linea;
- dejar evidencia y citas disponibles al expandir;
- no ocultar errores duros ni referencias que explican un bloqueo.

### Ownership y medicion

Editar una propuesta antes de aceptarla aumenta la sensacion de autoria. Nirmata
debe medir localmente, por operacion:

- aceptada sin cambios;
- editada y aceptada;
- rechazada;
- tiempo hasta la decision.

Estas metricas evaluan el producto; no entrenan al modelo ni salen del equipo
sin consentimiento.

Para estudios con usuarios se recomienda reutilizar el Creativity Support Index
o un subconjunto validado, no inventar una encuesta propia.

## Consecuencias para Nirmata

- Mantener modos explicitos.
- Usar `IntentBrief` solo cuando el alcance sea ambiguo o amplio.
- Aplicar juicio-antes-de-recomendacion solo a cambios riesgosos.
- Limitar alternativas visibles y hacer progresiva la evidencia.
- Preservar la edicion granular de cada operacion.

## Lo que no se implementa

- Autonomia adaptativa que decide cuando interrumpir al usuario.
- Personalizacion automatica de friccion.
- Generacion continua mientras el usuario escribe.
- Telemetria remota por defecto.

## Fuentes seleccionadas

- Horvitz, *Principles of Mixed-Initiative User Interfaces* (1999):
  <https://doi.org/10.1145/302979.303030>
- Amershi et al., *Guidelines for Human-AI Interaction* (2019):
  <https://doi.org/10.1145/3290605.3300233>
- Bucinca et al., *To Trust or to Think* (2021):
  <https://doi.org/10.1145/3449287>
- Bansal et al., *Does the Whole Exceed its Parts?* (2021):
  <https://doi.org/10.1145/3411764.3445717>
- Wu et al., *AI Chains* (2022):
  <https://doi.org/10.1145/3491102.3517582>
- Lee et al., *CoAuthor* (2022):
  <https://doi.org/10.1145/3491102.3502030>
- Chung et al., *TaleBrush* (2022):
  <https://doi.org/10.1145/3491102.3501819>
- Kreminski et al., *Loose Ends* (2022):
  <https://doi.org/10.1609/aiide.v18i1.21955>
- Mirowski et al., *Dramatron* (2023):
  <https://doi.org/10.1145/3544548.3581225>
- Cherry y Latulipe, *Creativity Support Index* (2014):
  <https://doi.org/10.1145/2617588>

## Limite de la evidencia

No se encontro un estudio controlado sobre una herramienta LLM, persistente y
multi-sesion dedicada a canon de mundos ficticios. Las reglas anteriores
combinan evidencia directa de coescritura con analogias declaradas de decision
asistida, codigo y otras herramientas creativas.

