# Journeys de creacion e IA

**Estado:** especificacion aprobada; prototipo navegable pendiente.

## Nuevo mundo

`Inicio > Nuevo mundo` presenta tres caminos equivalentes.

### Empezar manualmente

1. Nombre y ubicacion local.
2. Premisa y origen de calendario opcional.
3. Resumen: se creara un proyecto local vacio.
4. Crear y llegar al Inicio del mundo con una siguiente accion util.

Cancelar antes del resumen no crea archivo parcial.

### Crear base del mundo con IA

1. Nombre y ubicacion.
2. Genero, premisa, temas, tono, escala y restricciones.
3. Diagnostico del proveedor sin perder el brief.
4. Confirmacion de alcance: base acotada, no una obra completa.
5. Plan y conjunto de cambios.
6. Revision directa sin buscar el asistente.
7. `Aplicar al mundo` como unica escritura.

Fallo o cancelacion puede dejar un proyecto raiz vacio, nunca canon parcial.

### Estructurar material existente

1. Nombre y ubicacion.
2. Seleccionar `.md`, `.markdown` o `.txt` UTF-8.
3. Explicar que el original queda intacto y la copia es inerte.
4. Extraer, editar, seleccionar o rechazar elementos.
5. Resolver identidad y autoridad.
6. Abrir revision y aplicar explicitamente.

La seleccion multiarchivo no se promete hasta UX-033/UX-058.

## Preguntar y modificar

### Editar manualmente

`Mundo > Crear` y abrir un objeto canónico usan el mismo editor estructurado.
Relaciones, participantes, causalidad, metas afectadas y referencias documentales
se eligen por nombre y se pueden añadir, quitar o reordenar. Calendario y fechas
usan controles humanos; los formatos de transporte y las unidades canónicas
quedan en detalles técnicos.

`Preparar cambios` conserva objetivo, fuentes y supuestos y llama al preview
autoritativo de Rust. Un fallo mantiene el formulario. Un resultado válido abre
la revisión estándar; editar una operación pendiente vuelve por el mismo camino
sin aplicar canon.

### Preguntar

`Objeto > Preguntar` abre contexto, respuesta clasificada y fuentes navegables.
No crea una revision ni cambia canon.

### Convertir en propuesta

Una respuesta puede ofrecer `Convertir en propuesta`. La aplicacion confirma
que preparara cambios, hereda solicitud, seleccion, fuentes y version observada,
y solo entonces cambia al workflow de propuesta. Nunca cambia de modo en
silencio.

### Proponer directamente

`Objeto > Pedir un cambio` y `Asistente > Proponer cambios` usan el mismo
workflow. Alcance amplio muestra un resumen de intencion; todo resultado termina
en la cola global de revision.

Dentro de `Proponer cambios`, las plantillas Faccion, Ciudad, Personaje,
Conflicto, Cronologia y Consecuencias preparan un brief editable antes de llamar
al proveedor. Objetivo, alcance, entidades elegidas por nombre, restricciones y
escala pequena (hasta 3 operaciones) o mediana (hasta 6) permanecen visibles.
El brief explica por que aparece y que su autoridad es solo de propuesta. No hay
accion para generar una novela.

## Primer mundo vacio

Inicio ofrece acciones directas para editar el mundo, crear regla, entidad y
evento, consultar y proponer. La guia puede ocultarse por mundo, pero errores,
solo lectura y ausencia factual nunca se ocultan con esa preferencia. El ultimo
paso se completa unicamente cuando el historial editorial contiene el primer
conjunto de cambios aplicado; no depende de marcar una casilla local.

Los ejemplos del asistente solo rellenan una solicitud o abren el modo
correspondiente. Nunca ejecutan IA. Un mundo realmente vacio se distingue de una
busqueda o filtro sin coincidencias; Cronologia vacia ofrece crear evento o abrir
la plantilla Cronologia y Cambios vacio enlaza edicion manual, Proponer e
Importar sin crear revisiones por anticipado.

## Comprension de versiones

Cinco pruebas internas deben confirmar que la persona explica correctamente:

1. Una version anterior es de solo lectura.
2. Aplicar crea una revision nueva y no reemplaza la anterior.
3. Traer de B hacia A modifica A, no B.
4. Deshacer crea una revision inversa.
5. Crear una variante desde historia no mueve el canon principal.
