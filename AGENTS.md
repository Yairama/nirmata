# AGENTS.md

Estas instrucciones aplican a todo el repositorio.

## Antes de editar

1. Lee el flujo afectado de extremo a extremo.
2. Busca implementaciones y patrones existentes antes de crear algo.
3. Identifica la causa compartida; no parches sintomas en cada caller.
4. Revisa [`docs/README.md`](docs/README.md) y los documentos de arquitectura
   relacionados.
5. Conserva cambios existentes del usuario que no pertenezcan a tu tarea.

Si el codigo contradice una propuesta documental antigua, confirma el
comportamiento vigente antes de cambiarlo.

## Principios de ingenieria

- **YAGNI primero.** No agregues codigo, archivos, configuracion, capas,
  abstracciones ni extension points para necesidades hipoteticas.
- **DRY despues de duplicacion real.** Una pequena duplicacion local es mejor
  que una abstraccion prematura.
- **Elimina antes de agregar.** Borra rutas, helpers, wrappers o familias de
  componentes reemplazadas.
- **Refactoriza directamente.** No conserves compatibility layers, re-exports o
  wrappers transitorios para mantener imports antiguos.
- **Reutiliza antes de crear.** Prefiere codigo y patrones ya presentes.
- Elige codigo aburrido, explicito y facil de eliminar.

## No negociables

- Nada de boilerplate sin comportamiento.
- No barrel files por defecto; importa desde el modulo real.
- No wrappers, aliases o pass-through helpers de una linea sin valor adicional.
- Una sola forma evidente de persistir, validar, llamar IA, manejar errores y
  aplicar estilos.
- Cambios pequenos y locales.
- Agrega un archivo solo cuando el actual mezclaria responsabilidades no
  relacionadas.
- No agregues dependencias para resolver lo que cubren Rust, la plataforma o
  una dependencia ya instalada.
- No mantengas dos implementaciones durante una migracion: actualiza callers e
  imports y elimina la anterior.

## Limites arquitectonicos

La arquitectura objetivo es un monolito modular local-first:

| Modulo | Responsabilidad |
|---|---|
| `nirmata-core` | Dominio, invariantes y validacion determinista |
| `nirmata-store` | SQLite, migraciones, consultas, FTS5 y transacciones |
| `nirmata-ai` | HTTP del proveedor, prompts, streaming y parsing estructurado |
| `nirmata-app` | Casos de uso, permisos, contexto y workflow |
| `nirmata-desktop` | Tauri, GUI y adaptadores de presentacion |

Reglas de dependencia:

- `nirmata-core` no conoce GUI, SQLite, HTTP, proveedores de IA ni runtime
  asincrono.
- La GUI llama casos de uso; no contiene SQL, prompts ni reglas del canon.
- Persistencia expone operaciones de dominio, no SQL arbitrario a la GUI.
- La IA produce respuestas o `ChangeSetDraft`s; nunca recibe capacidad de
  commit.
- Un `ChangeSet` se aplica completo en una transaccion o no se aplica.
- Markdown contiene prosa; SQLite conserva identidad, relaciones, reglas y
  tiempo estructurado.

No crees capas `service`, `repository`, factories o interfaces solo para cumplir
una plantilla. Los limites anteriores ya asignan las responsabilidades.

## Ubicacion del codigo

- Mantener logica exclusiva junto a la feature que la usa.
- Promover a shared solo despues de reutilizacion real y ownership claro.
- Transport y comandos Tauri pertenecen a la frontera desktop.
- Reglas de negocio pertenecen a core o a casos de uso de app.
- SQL y conversion de filas pertenecen a store.
- Wiring visual pertenece a componentes, paginas o layouts del frontend.
- Adaptacion especifica de un proveedor pertenece a ai.

## Cambios de dominio e IA

- Ausencia de un dato significa desconocido, no falso.
- Separa canon de rumores, creencias e hipotesis.
- Separa tiempo del mundo de revisiones editoriales.
- Los errores duros deben provenir de esquema, invariantes implementadas o
  constraints SQLite.
- Un hallazgo LLM es evidencia para revision, no autoridad.
- No registres chain-of-thought; guarda fuentes, operaciones, reportes y
  decisiones.

## Validacion

- Ejecuta la comprobacion mas pequena que cubra el cambio.
- Agrega una prueba solo para comportamiento nuevo o corregido que pueda
  romperse.
- La logica no trivial debe dejar al menos un check ejecutable.
- No agregues frameworks de testing, linting o build sin necesidad demostrada.
- No declares completada una tarea con errores conocidos o comportamiento sin
  verificar.

Cuando existan comandos oficiales de build, lint y test, documentalos aqui solo
despues de ejecutarlos correctamente.

Comandos verificados para el corte actual:

```powershell
npm ci --prefix apps\nirmata-desktop\frontend
npm run build --prefix apps\nirmata-desktop\frontend
cargo nextest run --workspace
cargo build -p nirmata-desktop
```

## Documentacion

- Actualiza documentacion cuando cambie una decision que ya describe.
- Mantiene una responsabilidad por archivo en `docs/`.
- Usa enlaces relativos y rutas estables.
- No crees documentos de planificacion temporales dentro del repositorio.
- Una propuesta futura no justifica scaffolding presente.
