# Frentes criticos

**Estado:** investigacion consolidada.

Estos documentos cubren cuatro decisiones que debian resolverse antes de
implementar Nirmata:

- [Cocreacion humano-IA](human-ai-cocreation.md)
- [Versionado del canon](canon-versioning.md)
- [Conocimiento incierto](uncertain-knowledge.md)
- [Tiempo narrativo](narrative-time.md)

## Resultado comun

La evidencia no justifica un sistema mas grande. Justifica limites mas claros:

1. La IA propone y el humano decide; la friccion aumenta solo con el riesgo.
2. El canon tiene revisiones lineales y retcons tipados, no event sourcing.
3. La ausencia de un dato significa desconocido salvo dominios cerrados
   explicitos.
4. El tiempo del mundo y el orden del relato son ejes distintos.
5. Las relaciones temporales derivables se calculan; no se almacenan.
6. No se necesitan CRDT, motor paraconsistente, grafo temporal ni calendario
   ficticio en el MVP.

## Decisiones de diseno

- Mantener `Claim` como unidad autocontenida en el MVP. Separar proposicion y
  atribucion solo si la duplicacion medida lo exige.
- Usar IDs de revision para la historia del canon. No duplicar esa historia con
  timestamps semanticos ambiguos.
- Preparar una cadena padre-hijo de revisiones, pero no exponer ramas ni merges.
- Persistir restricciones temporales relativas solo cuando el usuario las
  declara y no pueden derivarse de intervalos.

