# Consecuencias para el diseño

Los trabajos orientan estas elecciones, pero no las determinan. Cada decision
debe validarse como ingenieria y producto.

| Hallazgo academico | Eleccion informada de Nirmata |
|---|---|
| Storyworld distinto del discurso | SQLite es canon; Markdown representa |
| Autenticacion ficcional | Solo un `ChangeSet` aceptado entra al canon |
| Mundos necesariamente incompletos | `NULL` significa no especificado |
| Mundos privados de personajes | `Claim` tiene holder, modalidad y acceso |
| Event-indexing model | Eventos indexan tiempo, espacio, entidad, causa e intencion |
| Coherencia local/global | Validacion Rust local + critico contra contexto amplio |
| Causal centrality | Recuperacion prioriza eventos causalmente conectados |
| IPOCL | `Goal` y motivacion son primera clase |
| MEXICA | Generador y critico usan contratos/contextos separados |
| Fabulist/Glaive | Reparacion y revalidacion acotadas por revision |
| Versu/emergent narrative | Multiagente de solo lectura antes de sintesis humana |
| Evaluacion narrativa | Suite de casos + evaluacion humana, no score unico |

## Lo que no debe inferirse de los papers

- La partida minima no autoriza copiar el mundo real dentro del canon.
- Un alto grado causal no significa automaticamente calidad narrativa.
- Una accion con motivacion explicable no es necesariamente interesante.
- La coherencia no exige completar todos los vacios.
- Un sistema multiagente no es mejor por producir mas contenido.
- Una ontologia academica no reemplaza decisiones de UX.

## Nuevos invariantes

1. Ausencia de dato y negacion explicita son estados diferentes.
2. Todo `Claim` atribuido tiene modalidad y sujeto epistemico.
3. Toda accion significativa de un actor puede enlazarse a un `Goal` o quedar
   marcada como motivacion desconocida.
4. Todo evento puede describirse en las cinco dimensiones; algunas pueden
   permanecer sin especificar.
5. La critica distingue discontinuidad explicada de contradiccion.
6. Ninguna inferencia producida por partida minima se autentica sin usuario.
