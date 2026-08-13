# Tree test de arquitectura de informacion

**Estado:** protocolo aprobado; ejecucion pendiente.

## Arbol congelado

```text
Inicio
  Nuevo mundo
    Empezar manualmente
    Crear base del mundo con IA
    Estructurar material existente
  Abrir un mundo
  Mundos recientes
Mundo
  Explorar objetos
  Buscar en el mundo
  Crear objeto
  Objeto seleccionado
    Editar
    Preguntar sobre este objeto
    Pedir un cambio
Cronologia
  Orden cronologico
  Tiempo desconocido
  Crear o abrir evento
Narrativa
  Orden en que se cuenta
  Causalidad
  Cabos abiertos
  Documentos internos
Simulacion
  Escenarios
  Ejecutar y comparar
  Preparar resultados para revision
Importaciones
  Material Markdown o texto
  Snapshot de un mundo
Versiones
  Version actual y versiones anteriores
  Deshacer ultimo cambio
  Variantes
  Comparar variantes
  Traer cambios de otra variante
Settings
  IA
  Proyecto
  Detalles tecnicos
Ayuda
  About
```

## Tareas criticas

| ID | Consigna | Destino correcto |
|---|---|---|
| T1 | Preparar con IA una base pequena y revisable para un proyecto nuevo. | Inicio > Nuevo mundo > Crear base del mundo con IA |
| T2 | Pedir a la IA que cambie el objeto abierto. | Mundo > Objeto seleccionado > Pedir un cambio |
| T3 | Revisar el ultimo cambio y revertirlo sin borrar auditoria. | Versiones > Deshacer ultimo cambio |
| T4 | Extraer elementos de un Markdown antes de incorporarlos. | Importaciones > Material Markdown o texto |
| T5 | Configurar la credencial y comprobar la conexion de IA. | Settings > IA |

Tareas diagnosticas cubren orden cronologico, cabos abiertos, comparacion de
escenarios, snapshot y traer cambios entre variantes.

## Gate

- Ejecutar al menos cinco pruebas internas independientes.
- Congelar consignas y respuestas antes de empezar y alternar su orden.
- No explicar terminos durante la prueba.
- Primer intento correcto exige raiz y destino correctos sin ayuda.
- Cada tarea T1-T5 debe alcanzar al menos `4/5`; no se permite agrupar tareas
  faciles para ocultar un fallo sistematico.
- Conservar respuestas literales, rutas equivocadas y terminos buscados.
