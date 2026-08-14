# Manual de usuario

## Proyectos locales

Nirmata guarda cada mundo en un archivo local `.nirmata`. Desde Inicio puedes
crear un mundo vacío, preparar una base con IA o estructurar Markdown y texto
existentes. Crear con IA o importar material nunca incorpora contenido al canon
sin una revisión posterior.

Usa `Ctrl+O` desde Proyecto para abrir un archivo y `Ctrl+N` para iniciar otro
cuando no haya un mundo abierto. Cerrar el mundo no elimina el archivo.

## Mundo y objetos

El área Mundo reúne Explorador, Editor y Contexto. En ventanas estrechas aparecen
como tabs; en desktop los separadores cambian sus anchos. Elige objetos por
nombre. Los identificadores internos viven en Detalles técnicos.

`Preparar cambios` crea una propuesta revisable; todavía no modifica el mundo.

Para eliminar una entidad, selecciónala en Mundo y usa `Eliminar del canon`.
Nirmata prepara una propuesta, no borra inmediatamente. Si relaciones, eventos,
metas, afirmaciones o documentos todavía la utilizan, Cambios muestra esas
dependencias y bloquea la aplicación hasta que las elimines o redirijas. Después
registra tu juicio, confirma la eliminación y usa `Aplicar al mundo`. No existe
borrado automático en cascada y Versiones permite preparar el undo posterior.

## Preguntar y proponer

`Consultar` responde con fuentes sin escribir. `Proponer cambios` produce
operaciones estructuradas. Las plantillas Facción, Ciudad, Personaje, Conflicto,
Cronología y Consecuencias preparan un resumen de intención editable.

La revisión profunda coordina especialistas de solo lectura. La auditoría es
orientativa y no crea propuestas. Puedes cancelar una solicitud desde la franja
global.

## Cambios

Cambios persiste las revisiones en el proyecto. Permite comparar Antes y Después,
aceptar, editar o rechazar operaciones, registrar juicios, aceptar advertencias
con motivo, volver a comprobar o descartar.

`Aplicar al mundo` es la única acción que escribe canon. Aplica todo el conjunto
en una transacción o no aplica nada.

## Versiones

`Escribiendo en` nombra la variante que recibirá cambios. `Viendo` indica la
versión observada. Una versión anterior es de solo lectura. Traer de B hacia A
modifica A, no B. Deshacer crea una versión inversa y no borra historia.

## Importaciones y backups

Lore copia Markdown o texto a un lote local, conserva citas y exige decisiones
de identidad. Snapshot exporta una copia estructurada y muestra el diff antes de
preparar Cambios. Un `.nirmata` es el proyecto vivo; un snapshot es un backup;
Markdown es material de entrada.

## Simulación, narrativa y calendario

Simulación usa escenarios de sesión fuera del canon. Solo una selección explícita
se vuelve propuesta y las afirmaciones nacen disputadas.

Estudio narrativo separa orden cronológico, relato, causalidad y cabos abiertos.
Los documentos generados muestran una preview inerte y entran en Cambios.

Calendario configura días y meses. Los eventos usan año, mes, día y unidad; Rust
conserva la unidad temporal autoritativa. Renombrar cambia presentación, no orden.

## Privacidad y recuperación

Canon, conversaciones locales, revisiones y backups permanecen en el equipo. La
IA es opcional. Envía contexto acotado a Microsoft Foundry con `store: false`; la
credencial se gestiona en Settings y nunca regresa a la interfaz.

Los errores conservan formularios y propuestas siempre que sea seguro. Los
banners indican cómo reintentar.

## Atajos

| Acción | Atajo |
|---|---|
| Nuevo mundo | `Ctrl+N` |
| Abrir mundo | `Ctrl+O` |
| Cerrar mundo | `Ctrl+Shift+W` |
| Buscar acciones y objetos | `Ctrl+K` |
| Settings | `Ctrl+,` |
| Ayuda | `F1` |
| Cerrar diálogo | `Escape` |

En macOS, los aceleradores usan `Cmd` en lugar de `Ctrl`.
