# Narrativa computacional

## Hallazgos

### Generacion local no garantiza coherencia global

TALE-SPIN genero historias como efecto de personajes persiguiendo objetivos.
Sus resultados absurdos mostraron temprano que acciones localmente permitidas
pueden producir narrativas globalmente incoherentes.

### Mundo persistente y trama son problemas distintos

UNIVERSE separo el mantenimiento de personajes y continuidad serial de la
planificacion de tramas. Nirmata sigue esa separacion: el canon persiste; cada
historia o propuesta opera sobre una revision.

### Causalidad consultable

Lehnert represento unidades de trama mediante estados y enlaces causales.
Trabasso y van den Broek encontraron empiricamente que la centralidad causal
predice importancia y recuerdo. Los eventos aislados son una señal util para el
critico, aunque no necesariamente un error.

### Intencion del personaje

IPOCL extiende planificacion causal para exigir una explicacion intencional de
las acciones. Una accion puede ser posible para el mundo y aun ser inverosimil
para el personaje. De aqui surge `Goal` como objeto distinto de `Event`.

### Reparacion acotada

Fabulist y trabajos posteriores reparan planes cuando una intervencion invalida
precondiciones. Este precedente informa la eleccion de reparar un draft sobre
el nuevo estado y volver a validarlo, en lugar de ejecutar bucles abiertos.

### Engagement y reflexion

MEXICA alterna generacion y reflexion. Confirma el valor de separar ambos modos,
pero tambien muestra que una reflexion con la misma representacion puede
compartir los sesgos del generador. Nirmata conserva un critico separado y mide
sus fallos correlacionados.

### Agentes emergentes

Versu y la narrativa emergente muestran que agentes sociales autonomos pueden
producir variedad, pero no garantizan arco, ritmo ni intencion autoral. El perfil
profundo de Nirmata produce informes de solo lectura; un sintetizador y un
humano conservan control editorial.

### Evaluacion

Las metricas automaticas de calidad narrativa son proxies imperfectos. La suite
de regresion necesita casos con respuestas esperadas y debe complementarse con
evaluacion humana de comprension e intencion.

## Fuentes seleccionadas

1. Meehan, James R. “TALE-SPIN, An Interactive Program that Writes Stories.”
   *Proceedings of IJCAI-77* (1977): 91-98.
   [PDF](https://www.ijcai.org/Proceedings/77-1/Papers/013.pdf).
2. Lebowitz, Michael. “Story-Telling as Planning and Learning.” *Poetics* 14.6
   (1985): 483-502.
   [DOI](https://doi.org/10.1016/0304-422X%2885%2990015-4).
3. Lehnert, Wendy G. “Plot Units and Narrative Summarization.” *Cognitive
   Science* 5.4 (1981): 293-331.
   [DOI](https://doi.org/10.1207/s15516709cog0504_1).
4. Trabasso, Tom, y Paul van den Broek. “Causal Thinking and the Representation
   of Narrative Events.” *Journal of Memory and Language* 24.5 (1985): 612-630.
   [DOI](https://doi.org/10.1016/0749-596X%2885%2990049-X).
5. Riedl, Mark O., y R. Michael Young. “Narrative Planning: Balancing Plot and
   Character.” *Journal of Artificial Intelligence Research* 39 (2010):
   217-268. [DOI](https://doi.org/10.1613/jair.2989).
6. Riedl, Mark O., C. J. Saretto y R. Michael Young. “Managing Interaction
   between Users and Agents in a Multi-Agent Storytelling Environment.”
   *AAMAS 2003*: 741-748. [DOI](https://doi.org/10.1145/860575.860694).
7. Perez y Perez, Rafael, y Mike Sharples. “MEXICA: A Computer Model of a
   Cognitive Account of Creative Writing.” *Journal of Experimental &
   Theoretical Artificial Intelligence* 13.2 (2001): 119-139.
   [DOI](https://doi.org/10.1080/09528130010029820).
8. Cavazza, Marc, Fred Charles y Steven J. Mead. “Character-Based Interactive
   Storytelling.” *IEEE Intelligent Systems* 17.4 (2002): 17-24.
   [DOI](https://doi.org/10.1109/MIS.2002.1024747).
9. Evans, Richard, y Emily Short. “Versu—A Simulationist Storytelling System.”
   *IEEE Transactions on Computational Intelligence and AI in Games* 6.2
   (2014): 113-130. [DOI](https://doi.org/10.1109/TCIAIG.2013.2287297).
10. Aylett, Ruth et al. “Unscripted Narrative for Affectively Driven
    Characters.” *IEEE Computer Graphics and Applications* 26.3 (2006): 42-52.
    [DOI](https://doi.org/10.1109/MCG.2006.71).
11. Kybartas, Ben, y Rafael Bidarra. “A Survey on Story Generation Techniques
    for Authoring Computational Narratives.” *IEEE Transactions on
    Computational Intelligence and AI in Games* 9.3 (2017): 239-253.
    [DOI](https://doi.org/10.1109/TCIAIG.2016.2546063).
12. Purdy, Christopher et al. “Predicting Generated Story Quality with
    Quantitative Measures.” *AIIDE 2018*: 95-101.
    [DOI](https://doi.org/10.1609/aiide.v14i1.13021).

## Consecuencias

- Nirmata es una herramienta de soporte narrativo, no un generador autonomo.
- La causalidad y la intencion deben ser datos consultables.
- Especialistas no deben escribir estado compartido.
- La simulacion futura necesita mediacion y checkpoints, no commit automatico.
- Las metricas automaticas no reemplazan revision ni estudios con usuarios.
