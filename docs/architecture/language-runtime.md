# Rust, Python y GUI

**Estado:** recomendacion consolidada.

## Decision

El producto distribuido debe usar Rust para:

- dominio;
- casos de uso;
- SQLite;
- validacion;
- integracion con proveedores de IA;
- comandos del escritorio.

No se debe incluir Python en el runtime del MVP.

## Por que Rust es adecuado

- binario distribuible sin entorno adicional;
- tipos fuertes para `ChangeSet` y reglas del canon;
- buen control de concurrencia;
- integracion madura con SQLite y HTTP;
- rendimiento suficiente para busqueda y recorridos locales;
- una sola implementacion del dominio.

La orquestacion inicial es un workflow pequeno. No necesita el ecosistema de
frameworks multiagente de Python.

## Capa de IA en Rust

El flujo minimo requiere:

- HTTP y streaming;
- serializacion con `serde`;
- esquema de salida;
- validacion;
- cancelacion;
- telemetria.

Puede implementarse directamente con `reqwest` y tipos propios. No se necesita
una abstraccion generica de proveedores mientras exista uno solo.

Rig es una opcion posterior cuando haya una necesidad comprobada de:

- varios proveedores;
- tool calling uniforme;
- tracing especializado;
- workflows agenticos reutilizables.

Adoptarlo desde el primer dia agregaria una API inestable que el flujo lineal
no necesita.

## Cuando usar Python

Python gana valor en trabajos opcionales y pesados:

- importar novelas mediante GraphRAG;
- NLP experimental;
- entrenamiento o evaluacion de embeddings;
- procesamiento por lotes;
- prototipos de investigacion.

La integracion recomendada es un proceso externo con un contrato de archivos o
JSON. El proyecto principal no embebe interprete, `venv` ni dependencias Python.

## GUI: recomendacion

Para Nirmata, la GUI es principalmente:

- editor Markdown;
- arboles y formularios;
- chat con streaming;
- diffs;
- timeline;
- grafo visual futuro.

La opcion recomendada es **Tauri 2 con un frontend web pequeno** y los crates
Rust como backend.

### Por que Tauri

- reutiliza editores Markdown y componentes de diff maduros;
- facilita streaming, paneles redimensionables y accesibilidad;
- el webview es nativo del sistema;
- conserva dominio, almacenamiento y seguridad en Rust;
- evita escribir widgets complejos de texto desde cero.

El framework frontend concreto no es una decision arquitectonica todavia. Debe
elegirse por experiencia del equipo y disponibilidad de los pocos componentes
necesarios.

### egui

`egui` sigue siendo una buena opcion para:

- un prototipo completamente Rust;
- herramientas internas;
- formularios y visualizacion inmediata;
- una primera prueba del dominio.

No es la recomendacion principal para el producto porque la edicion rica de
documentos, los diffs y la accesibilidad pueden exigir mas codigo propio.

### GPUI

Es prometedor para aplicaciones tipo IDE, pero sigue ligado a un ecosistema
mas joven y con mayor cambio de API. No es necesario asumir ese riesgo.

## Frontera Tauri

Los comandos expuestos al frontend deben corresponder a casos de uso:

```text
open_world
search_world
get_entity
save_manual_changes
ask_world
propose_changes
validate_changeset
commit_changeset
undo_commit
```

No se expone SQL, acceso libre a archivos ni una funcion generica de ejecutar
herramientas.

## Seguridad

El contenido del mundo y la salida del modelo son datos no confiables:

- Markdown se renderiza con HTML deshabilitado o sanitizado;
- las claves de proveedor se guardan en el almacen seguro del sistema;
- los comandos Tauri validan IDs y rutas;
- el modelo no recibe herramientas de escritura;
- importar archivos nunca ejecuta contenido;
- logs y telemetria no incluyen lore por defecto.

## Decision de distribucion

```text
Desktop Tauri
  -> frontend web local
  -> comandos Rust
  -> nirmata-app
  -> core/store/ai
  -> SQLite y proveedor remoto
```

No hay servidor web publico, daemon de Python ni contenedores.
