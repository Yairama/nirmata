# Vista general del sistema

**Estado:** recomendacion consolidada.

## Estilo arquitectonico

Nirmata debe comenzar como un **monolito modular local-first**:

- una aplicacion de escritorio;
- un proceso principal;
- un archivo de proyecto SQLite;
- llamadas remotas opcionales a un proveedor de IA;
- modulos Rust con dependencias dirigidas;
- ningun servicio local que el usuario deba administrar.

El monolito modular conserva transacciones simples y distribucion sencilla sin
impedir extraer componentes si aparece una necesidad real.

## Capas

```text
+----------------------------------------------------------+
| GUI: navegacion, editor, asistente, diff, timeline       |
+-----------------------------+----------------------------+
                              |
+-----------------------------v----------------------------+
| Aplicacion: casos de uso, permisos, workflow, contexto   |
+------------------+---------------------+-----------------+
                   |                     |
+------------------v--------+  +---------v-----------------+
| Persistencia SQLite       |  | Adaptador de IA           |
| SQL, FTS, transacciones   |  | HTTP, prompts, streaming  |
+------------------+--------+  +---------+-----------------+
                   |                     |
+------------------v---------------------v-----------------+
| Dominio: World, Entity, Event, Goal, Claim, Rule, ChangeSet |
+----------------------------------------------------------+
```

## Limites

### Dominio

Contiene tipos y reglas que siguen siendo correctos sin GUI, SQLite o IA:

- identidad y tipos de entidades;
- relaciones permitidas;
- tiempo y causalidad;
- estados del canon;
- operaciones de cambio;
- validacion determinista.

No hace I/O ni depende de un runtime asincrono.

### Aplicacion

Implementa los casos de uso:

- consultar el mundo;
- proponer una edicion;
- validar una propuesta;
- aceptar un subconjunto de operaciones;
- confirmar o deshacer un cambio;
- construir el contexto para el modelo.

Esta capa es necesaria porque consulta y edicion comparten reglas y no deben
duplicarse dentro de la GUI.

### Persistencia

Expone operaciones orientadas al dominio, no SQL arbitrario a la GUI. Mantiene:

- esquema y migraciones;
- consultas;
- FTS5;
- transacciones;
- historial de cambios;
- indices derivados.

### IA

Se limita a capacidades no deterministas:

- enviar contexto e instrucciones;
- recibir streaming;
- convertir salida estructurada;
- reportar uso y errores.

No puede iniciar transacciones ni escribir canon.

### GUI

Presenta estado y dispara casos de uso. No contiene reglas de continuidad,
prompts ni SQL.

## Procesos y concurrencia

El MVP no necesita workers persistentes ni colas externas.

- La GUI permanece responsiva mediante tareas asincronas.
- Las llamadas de red usan el runtime asincrono de la aplicacion.
- Las operaciones SQLite son sincronas y cortas.
- Una operacion pesada puede ejecutarse en el pool bloqueante existente.
- Solo se permite un escritor por proyecto.

No se necesita un actor dedicado para SQLite hasta que las mediciones muestren
contencion. Un mutex o acceso serializado dentro del adaptador de persistencia
es suficiente para un usuario.

## Unidad de consistencia

La unidad de escritura es un `ChangeSet`.

1. El usuario selecciona operaciones.
2. El dominio vuelve a validar el conjunto resultante.
3. Persistencia abre una transaccion.
4. Se aplican todas las operaciones.
5. Se registra el antes y despues.
6. Se actualizan indices derivados.
7. Se confirma o revierte todo.

No se aplica una operacion parcial fuera de esta frontera.

## Evolucion permitida

La arquitectura puede crecer sin reescribir el dominio:

- otra GUI consume la capa de aplicacion;
- un CLI reutiliza los mismos casos de uso;
- un proveedor local reemplaza al remoto;
- embeddings se agregan como indice derivado;
- un importador Python funciona como proceso externo;
- colaboracion futura reemplaza la persistencia, no las reglas del mundo.
