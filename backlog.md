# Backlog end-to-end de rediseño UX/UI de Nirmata

## Propósito

Este documento sucede al backlog funcional completado, archivado en
[`docs/old-backlogs/backlog.md`](docs/old-backlogs/backlog.md). Es el plan
ejecutable y autosuficiente para convertir la interfaz actual en una aplicación
de escritorio comprensible, eficiente y visualmente coherente sin alterar las
fronteras de dominio, autoridad o persistencia ya verificadas.

El objetivo no es aplicar una capa cosmética. El rediseño debe permitir que una
persona que construye mundos entienda, sin conocer SQLite, UUID, FTS, VFS,
ChangeSet, ReadScope o heads:

1. Cómo crear un proyecto y construir una base de mundo manualmente o con IA.
2. Dónde preguntar por el canon y dónde pedir modificaciones.
3. Qué cambiará antes de aplicar una propuesta.
4. En qué versión está trabajando y qué versión está observando.
5. Cómo importar, simular, derivar narrativa, revisar, deshacer y configurar la
   aplicación sin perder trabajo.

La tesis UX es:

> Nirmata debe sentirse como software editorial para mundos, no como una página
> de diagnóstico que expone todos los subsistemas a la vez.

## Estado actual

- El backlog funcional NIR-001–NIR-089 está completado y no se reabre.
- Este backlog UX contiene UX-001–UX-078: 36 completadas de 78 (46,2 %).
- El backend Rust, SQLite, Tauri y los contratos de IA siguen siendo la
  autoridad. El rediseño consume casos de uso existentes y solo solicita nuevos
  comandos cuando una interacción demostrada no pueda expresarse hoy.
- La interfaz actual usa TypeScript, HTML y CSS nativos sobre Vite, todavía sin
  framework; ya supera el tamaño razonable para rendering imperativo global.
- No hay Settings, About, navegación primaria, command palette, onboarding ni
  conversación persistente.
- Crear un `.nirmata` no genera un mundo completo: crea el proyecto raíz. Para
  generar una base con IA hoy hay que crear el proyecto, buscar el asistente,
  cambiar a `Proponer`, escribir una petición y revisar el ChangeSet.
- El panel llamado asistente no es un chat real: cada consulta reemplaza la
  respuesta anterior y no se envía historial conversacional.
- `head` significa internamente la última revisión de una variante. Esa palabra
  no debe formar parte del flujo principal del usuario.

## Auditoría visual del 12 de agosto de 2026

Se abrió el frontend compilado con un bridge Tauri simulado y se capturaron los
estados de proyecto cerrado, mundo abierto, viewport Tauri predeterminado
`960x680`, desktop `1440x1000` y mobile `390x844`.

| Evidencia | Hallazgo |
|---|---|
| Proyecto cerrado, captura completa | Se renderiza también todo `world-view` aunque tenga `hidden`. Una regla de layout gana a la semántica nativa y expone controles inválidos sin mundo. |
| Mundo abierto, `960x680` | El primer viewport sigue mostrando el formulario de crear/abrir. El mundo activo y el workspace quedan fuera del fold; la app parece no haber abierto el proyecto. |
| Mundo abierto, captura desktop completa | La pantalla apila variantes, layout, IA, narrativa, importación, simulación y finalmente el editor. El flujo diario aparece después de varios laboratorios avanzados. |
| Mobile `390x844` | Todo se convierte en una columna extremadamente larga; los controles técnicos ocupan decenas de pantallas y no existe navegación contextual. |
| Lighthouse snapshot | Accesibilidad 92/100. Fallan contraste en 17 botones, nombre accesible del selector de tipo y árbol de accesibilidad asociado. |
| Consola | Hay campos sin `name`/autocomplete y un recurso 404; los errores de extensión del navegador no pertenecen a Nirmata. |

### Autocrítica visual

- La paleta oscura es consistente, pero todas las superficies tienen peso visual
  similar; no hay jerarquía entre tarea primaria, configuración y laboratorio.
- El diseño parece un formulario administrativo: cajas, bordes y textareas, no
  un entorno editorial orientado a objetos, relaciones e historias.
- Se usan colores distintos para IA, narrativa y simulación, pero el color no
  resuelve la arquitectura de información ni explica qué es canónico.
- La densidad es baja donde debería ser compacta (shell y navegación) y alta
  donde debería guiar (formularios complejos, merge e importación).
- Los paneles avanzados compiten por el ancho y la atención aunque el usuario no
  los esté usando.
- El producto mezcla español e inglés: drafts, scope, story time, loose ends,
  Factions, Resources, Stocks, Rules, Kind, Request y Max steps.
- Los IDs y mini-DSL con `|` convierten tareas de autor en edición de formatos de
  transporte.
- Los botones primarios actuales usan blanco `#fff` sobre azul `#0284c7`, con
  contraste aproximado 4.09:1, inferior al mínimo AA 4.5:1 para ese texto.

### Revisión visual tras Fase 0

Se repitieron capturas del frontend compilado con bridge Tauri simulado en
`960x680` y `390x844` después de UX-005–UX-008.

- Proyecto cerrado ya no filtra ninguna superficie del mundo y el árbol de
  accesibilidad contiene solo crear/abrir.
- En `960x680`, el botón final de creación todavía queda bajo el fold; la
  decisión de abrir sí permanece visible. Esto confirma que el wizard y la
  nueva pantalla Inicio de UX-029 no deben conservar el formulario largo.
- En `390x844` no apareció scroll horizontal, pero `Abrir mundo` queda después
  de todo el formulario manual. La futura landing debe presentar primero los
  tres caminos de creación y abrir un proyecto como decisiones equivalentes.
- La primera captura con busy descubrió un bucle del `MutationObserver`: la
  franja reasignaba el mismo texto y se notificaba a sí misma. Se corrigió
  evitando mutar el nodo cuando el mensaje no cambia y se añadió un safety
  check específico.
- El diagnóstico real contra Microsoft Foundry verificó `/openai/v1/responses`
  con estado `completed` y el modelo configurado, sin enviar contexto de mundo
  ni crear propuestas. No es necesario cambiar al contrato chat completions.

### Revisión visual de journeys y versiones

Se capturaron la nueva landing, el paso `Crear una base del mundo con IA` y el
mundo abierto en `960x680`, además de la landing en `390x844`.

- Los tres caminos y `Abrir mundo` aparecen como decisiones del primer viewport
  desktop; ya no es necesario recorrer primero el formulario manual.
- La primera versión narrow conservó tres columnas y volvió ilegible el copy.
  La autocrítica produjo un breakpoint específico: a `390x844` las opciones son
  tarjetas compactas de una columna, sin scroll horizontal.
- El paso IA comunica correctamente que primero crea el proyecto y luego una
  propuesta revisable, pero aún es un formulario largo. UX-031 deberá convertir
  esos campos en pasos, no conservar la página vertical.
- `Escribiendo en: Canon principal · Viendo: versión actual` se entiende sin
  UUID; la revisión interna se movió a `Detalles técnicos`.
- El encabezado de mundo y versiones todavía consume casi todo `960x680` y el
  workspace sigue fuera del fold. La solución pertenece a la topbar/sidebar de
  UX-035/036, no a seguir comprimiendo esta página imperativa.
- El árbol de accesibilidad de la landing expone nombres completos para los tres
  caminos y la consola quedó sin mensajes tras añadir autocomplete al nombre.

### Revisión visual del primer corte React

La landing React se capturó en `960x680` después de un ciclo simulado completo
crear → mundo abierto → cerrar.

- La superficie conserva exactamente la composición aprobada y el foco vuelve a
  `Empezar manualmente` al cerrar el mundo.
- El primer smoke detectó que `busy` y el formulario sobrevivían ocultos porque
  el componente retornaba `null` sin desmontarse. Se corrigió reseteando estado
  al transicionar la sesión a `null` y se añadió un safety check.
- El segundo smoke confirmó landing limpia, tres caminos, ningún botón
  deshabilitado y status inicial correcto después de reabrir/cerrar.
- La landing no genera mensajes de consola. Los avisos `name`/autocomplete
  observados tras abrir pertenecen a formularios legacy del mundo y siguen
  asignados a UX-043/072.
- React aumentó el bundle inicial a 328,46 kB/95,12 kB gzip, todavía por debajo
  del presupuesto de 250 kB gzip. Este coste refuerza migrar por superficies y
  no agregar librerías hasta su primer uso real.
- Playwright repitió la landing en `960x680` con IPC Tauri simulado sobre los
  imports oficiales. El recorrido de teclado y axe no reportaron violaciones
  serious/critical; el azul primario cambió de `#0284c7` a `#0369a1` para
  corregir el contraste base documentado en la auditoría inicial.

### Revisión visual de temas semánticos

Playwright capturó landing light, dark y high contrast en `960x680`, recorrió
los tres formularios en `390x844` y emuló reduced motion.

- Light adopta papel cálido y tipografía oscura; se acerca a mesa editorial y
  abandona la estética de dashboard azul.
- Dark usa negro cálido y separadores neutrales. La jerarquía de las tres
  tarjetas aún es deliberadamente plana; onboarding decidirá si una opción debe
  recomendarse, no el color.
- High contrast aplana superficies y eleva bordes a 2 px sin depender solo del
  color; axe no reportó contraste, serious ni critical.
- El primer test narrow detectó 8 px de overflow causados por extender el fondo
  con margen negativo. Se eliminó la causa en vez de ocultar overflow; manual,
  IA e importación pasan ahora a `390x844`.
- El gutter exterior sigue oscuro porque el documento y `world-view` conservan
  el sistema legacy. No se añadió un scope oscuro de compatibilidad: cada
  superficie migrada debe adoptar tokens y retirar sus literales.
- El selector de tema vive provisionalmente en la landing para hacer verificable
  la preferencia; Settings será su owner final en UX-038.

### Revisión visual app-wide de UX-021

Se capturó el mismo mundo vacío en light, dark y high contrast a `1440x900` y se
repitieron forced colors, read-only y error con axe.

- Light transforma también versiones, layout, IA, narrativa, importación,
  simulación y workspace; ya no es una isla clara rodeada por paneles oscuros.
- Dark conserva la voz cálida sin gradientes por subsistema. IA, narrativa y
  simulación se distinguen por texto/estado, no por fondos decorativos.
- High contrast lleva superficies al canvas y bordes a 2 px. La jerarquía se
  vuelve deliberadamente más austera, pero foco, acciones y solo lectura siguen
  reconocibles.
- Axe encontró inicialmente 13 fugas de selectores legacy más específicos:
  metadata, sliders, empty states, credencial y ayuda de campos. Se corrigieron
  en sus roles semánticos sin excluir reglas del análisis.
- Se eliminaron todos los hex/rgb/color-mix de reglas de componentes y aliases
  `--border/--surface/--accent/--mono/--danger`; los literales viven únicamente
  en los bloques de tokens light/dark.
- La captura confirma una deuda de arquitectura de información, no de tema: la
  barra de versiones sigue densa y los laboratorios preceden al workspace. Se
  mantiene asignada a UX-035/036/068 y no se ocultó dentro de UX-021.

### Revisión visual de Inicio, Settings y shell editorial

El 13 de agosto de 2026 se repitieron capturas Playwright en `960x680`,
`1440x900` y `390x844` después de UX-029–UX-032, UX-035, UX-036 y UX-038.

- Inicio contiene los tres caminos, abrir, recientes, estado IA, Settings y
  Ayuda en el primer viewport `960x680`; ninguna superficie del mundo se monta
  visualmente antes de abrir un proyecto.
- El wizard narrow no tiene scroll horizontal y no invoca `create_world` antes
  del resumen final. La primera captura reveló títulos demasiado grandes y se
  redujo su escala solo bajo `560px`; el formulario sigue siendo vertical por la
  limitación real de `390px`, pero ya no apila laboratorios ajenos al journey.
- Settings usa Dialog/Tabs con foco atrapado y retorno explícito al disparador.
  La prueba detectó que el efecto de foco de Inicio robaba inicialmente ese
  retorno; se corrigió el ownership en vez de relajar la aserción.
- La primera shell heredó el azul primario de las tarjetas de Inicio en todos
  los botones laterales y enfocó el título inicial con un contorno excesivo. La
  autocrítica produjo una sidebar neutral con énfasis solo en el área activa y
  foco programático únicamente después de navegación iniciada por el usuario.
- Mundo abierto presenta proyecto, variante, vista, búsqueda, cambios, Settings
  y Ayuda en una línea a `1440x900`; Inicio del mundo ocupa el área principal y
  narrativa, simulación, importaciones y versiones permanecen ocultas hasta
  entrar en su área, sin desmontar ni perder valores de formularios.
- Unit 5/5, E2E 23/23, safety 20/20 y axe sin serious/critical pasaron. El
  gate Rust quedó en 261/261, 1 omitido, y desktop build pasó. Bundle actual:
  JS 508,92 kB/151,45 kB gzip y CSS 50,29 kB/9,23 kB gzip, dentro de los
  presupuestos iniciales.

### Revisión visual del explorador React

Se capturó Mundo con 200 resultados a `1440x900` después de UX-037, UX-040 y
UX-041.

- Explorador, editor y contexto permanecen visibles en tres columnas; 200
  resultados usan tarjetas compactas con tipo, clasificación y autoridad, sin
  URI, UUID ni `FTS` en el flujo principal.
- La primera prueba axe encontró una estructura ARIA inválida: un `listbox`
  contenía `article`s además de options para alojar detalles técnicos. Se cambió
  a región navegable con botones y teclado explícito, en vez de ocultar la
  violación.
- La primera medición de cambio de filtro dio 56,2 ms porque vaciaba la lista y
  esperaba dos frames. Se conservó el resultado anterior con `placeholderData`
  durante la consulta y el primer paint efectivo quedó bajo 50 ms sin relajar el
  gate.
- La autocrítica visual confirma que el explorador ya sigue la dirección
  editorial, pero el editor legacy aún duplica creación y muestra enums como
  `Place`; se retiró la creación duplicada y el lenguaje/pickers quedan asignados
  a UX-042/043, no parcheados dentro del explorador.
- La segunda captura sustituyó la columna de contexto vacía por tabs Canon,
  Perspectivas, Metas, Cronología y Avisos. La evidencia relacionada ocupa el
  espacio solo cuando existe y muestra autoridad/clasificación sin IDs; el enum
  inglés e identificador abreviado que aún aparecen en el editor central quedan
  como deuda explícita de UX-043.

## Respuestas de producto que la nueva UI debe hacer evidentes

### ¿Dónde genero el mundo?

La pantalla `Nuevo mundo` ofrecerá tres caminos explícitos:

1. **Empezar manualmente:** nombre, premisa y estructura vacía.
2. **Crear una base con IA:** wizard de género, premisa, temas, tono, escala,
   restricciones y elementos iniciales. La IA produce un plan y un conjunto de
   cambios revisable; nunca aplica un mundo completo directamente.
3. **Estructurar material existente:** importar Markdown/texto, extraer
   candidatos y revisar su ingreso al canon.

La acción se llamará `Crear base del mundo con IA`, no `Generar todo el mundo`.
El producto debe explicar que un mundo coherente se desarrolla iterativamente y
que la primera generación crea una base editable, no una verdad completa.

### ¿Cómo pido modificaciones?

- Desde cualquier objeto: acción contextual `Pedir un cambio`.
- Desde el asistente acoplable: modo `Proponer cambios`.
- Desde la command palette: `Proponer cambio sobre la selección`.
- La petición produce siempre una propuesta en la cola global de revisión.
- La tarjeta de revisión reúne validación, crítica final, decisiones y acción
  `Aplicar al mundo`; el usuario no debe recorrer paneles distantes.

### ¿Hay chat?

Hoy existe consulta puntual, no chat. El rediseño debe elegir y cumplir una
promesa clara:

- Implementar conversaciones locales con mensajes usuario/asistente, historial,
  contexto visible, fuentes y borrado sin afectar canon; o
- Si el backend multi-turn no está listo, llamar la superficie `Consulta` y no
  mostrar lenguaje de chat/transcript.

Este backlog adopta conversación local explícita. El historial conversacional
no es canon y la acción `Convertir en propuesta` crea un nuevo workflow de
edición con confirmación del usuario.

### ¿Qué son las cabezas?

Una head es la última revisión de una variante; es un detalle interno. La UI
principal usará:

| Interno | Texto de usuario |
|---|---|
| Active variant | `Escribiendo en: Canon principal` |
| Observed scope | `Viendo: versión actual` o `Viendo: versión del 12 ago` |
| Head | `Última versión` |
| Return to head | `Volver a la versión actual` |
| Stale | `Propuesta desactualizada` |
| Merge source/destination | `Traer cambios de B hacia A` |

IDs, parent revision y heads quedan en `Detalles técnicos` y Settings > Avanzado.

### ¿Debe parecer software con Settings y About?

Sí. La shell tendrá barra superior, navegación primaria, command palette, panel
de asistente, cola de revisión, Settings y About. Settings incluirá apariencia,
IA, proyecto, accesibilidad y opciones avanzadas. About mostrará versión,
licencia, arquitectura local-first, privacidad y documentación.

## Arquitectura de información objetivo

### Shell de escritorio

```text
+--------------------------------------------------------------------------+
| Proyecto | Escribiendo en | Viendo | Buscar / Ctrl+K | Cambios | Ayuda   |
+-------------+------------------------------------------+-----------------+
| Inicio      |                                          | Asistente       |
| Mundo       | Editor / vista principal                 | Preguntar       |
| Cronología  |                                          | Proponer        |
| Narrativa   |                                          | Fuentes         |
| Simulación  |                                          |                 |
| Importar    |                                          |                 |
| Versiones   |                                          |                 |
+-------------+------------------------------------------+-----------------+
| Cola de revisión: resumen, bloqueos y siguiente acción                  |
+--------------------------------------------------------------------------+
```

### Navegación primaria

1. `Inicio`
2. `Mundo`
3. `Cronología`
4. `Estudio narrativo`
5. `Laboratorio de simulación`
6. `Importaciones`
7. `Versiones e historial`
8. `Settings`

About vive en Ayuda. Las features avanzadas se cargan al entrar, no ocupan la
pantalla principal permanentemente.

### Lenguaje de producto

- `Guardar draft` → `Preparar cambios`
- `ChangeSet` → `Conjunto de cambios`
- `Confirmar ChangeSet` → `Aplicar al mundo`
- `Waiver` → `Aceptar advertencia con motivo`
- `DecisionPoint` → `Decisión pendiente`
- `Retcon` → `Tipo de cambio editorial` dentro de opciones avanzadas
- `Story time` → `Orden cronológico`
- `Discourse order` → `Orden en que se cuenta`
- `Loose ends` → `Cabos abiertos`
- `Scope` → `Vista analizada`
- `FTS` → `Buscar`
- `VFS lógico` → `Explorador del mundo`

## Decisión de stack frontend

### Stack recomendado

| Área | Decisión | Motivo |
|---|---|---|
| Framework | React estable + TypeScript estricto | La GUI ya necesita componentes, ownership de estado, lifecycle, formularios y overlays. |
| Build | Vite SPA estática | Tauri recomienda Vite para SPA; aporta HMR, bundling, minificación y code splitting. |
| Estilos | Tailwind CSS v4, condicionado por gate de WebView | CSS estático, tokens y productividad; no soporta WebViews antiguos. |
| Tokens | Variables CSS semánticas | Mantienen identidad propia y permiten light/dark/high contrast sin acoplarse a utilities. |
| Primitivas | `radix-ui` tree-shakeable | Dialog, Select, Tooltip, Tabs, Popover y foco accesible. |
| Componentes | shadcn/ui selectivo, código copiado y revisado | Acelera componentes sin adoptar un kit cerrado ni estética SaaS genérica. |
| Datos Tauri | TanStack Query | Cache e invalidación explícita de estado asíncrono por mundo/variante/revisión. |
| Estado UI | Estado local y `useReducer` por workspace | Evita otro store global; TanStack cubre datos backend. |
| Formularios | React Hook Form | Dirty state, field arrays, foco de error y rerenders localizados. Rust sigue siendo autoridad. |
| Paneles | `react-resizable-panels` | Splitters accesibles por teclado, collapse y persistencia de layout. |
| Command palette | `cmdk`, solo al implementar UX-037 | Búsqueda de objetos y acciones con teclado. |
| Iconos | `lucide-react`, tras inventario de iconos | Evita iconos inconsistentes; no se agrega antes de uso real. |
| Tests | Vitest + Testing Library + user-event | Comportamiento por roles, teclado, foco y forms. |
| Visual/E2E web | Playwright + IPC Tauri simulado + axe | Screenshots, responsive y accesibilidad en entorno estable. |
| E2E binario | WebdriverIO Tauri, condicional | Solo para dialogs/plugin/WebView real no cubiertos por browser tests. |

Fuentes oficiales:

- Tauri frontend/Vite: <https://v2.tauri.app/start/frontend/>
- Tailwind v4 con Vite: <https://tailwindcss.com/docs/installation/using-vite>
- Compatibilidad Tailwind v4: <https://tailwindcss.com/docs/compatibility>
- shadcn para Vite: <https://ui.shadcn.com/docs/installation/vite>
- Radix y accesibilidad: <https://www.radix-ui.com/primitives/docs/overview/introduction>
- TanStack Query: <https://tanstack.com/query/latest/docs/framework/react/overview>
- React Hook Form: <https://react-hook-form.com/get-started>
- Mocks Tauri: <https://v2.tauri.app/develop/tests/mocking/>
- WCAG 2.2: <https://www.w3.org/TR/WCAG22/>

### Gate obligatorio de Tailwind v4

Tailwind v4 requiere Chrome 111, Safari 16.4 o Firefox 128. Antes de adoptarlo:

- Windows: verificar WebView2 111 o superior.
- macOS: fijar macOS 13.3 o superior como piso práctico.
- Linux: fijar distribuciones/WebKitGTK soportados y probar CSS real.
- Si la matriz necesita WebViews más antiguos, conservar React + Vite + Radix y
  usar CSS convencional con variables semánticas; no introducir hacks dobles.

### Dependencias que no se agregan por defecto

- Next.js, SSR o metaframework.
- Redux, Zustand, Jotai o MobX.
- Zod como segunda autoridad del dominio.
- Material UI, Ant Design, Chakra o Mantine junto a Radix.
- CSS-in-JS, Sass o Less.
- Framer Motion para transiciones simples.
- Storybook antes de una necesidad demostrada.
- Router framework; la navegación inicial es estado de shell local.
- Virtualización, data grid o editor Markdown pesado sin medición.
- Catálogo completo de shadcn o barrels de componentes.
- Un cliente IPC genérico que esconda comandos Tauri específicos.

## Gates medibles del rediseño

| Gate | Objetivo |
|---|---|
| Funcionalidad | Paridad de los comandos y workflows actualmente aceptados antes de borrar la UI vieja. |
| Bundle JS inicial | ≤ 250 KiB gzip. |
| CSS inicial | ≤ 60 KiB gzip. |
| Lazy chunk | ≤ 150 KiB gzip por feature sin justificación. |
| Arranque | Shell interactivo P95 ≤ 1 s en hardware Windows de referencia. |
| Paint tras abrir mundo | ≤ 150 ms P95 después de `open_world`. |
| Búsqueda | Keystroke a paint con 200 resultados ≤ 50 ms P95. |
| Command palette | Apertura y primera respuesta visual ≤ 100 ms. |
| Resize | Sin long task > 50 ms; objetivo 55+ fps. |
| Accesibilidad | Cero violaciones axe serious/critical y contraste WCAG 2.2 AA. |
| Teclado | Crear, navegar, editar, revisar y aplicar sin mouse. |
| Responsive | Flujos esenciales operables en `720x520`, `960x680` y `1440x900`. |
| Mobile narrow | Sin scroll horizontal ni controles técnicos obligatorios a `390x844`. |
| Seguridad | Cero `innerHTML`, `dangerouslySetInnerHTML` o scripts remotos en lore/IA. |
| Foco | Dialogs/palette devuelven foco; error enfoca primer campo inválido. |
| IPC | Cero listeners duplicados después de montar/desmontar. |
| Stale safety | Nunca se habilita `Aplicar al mundo` tras cambiar revisión sin revalidar. |
| Regresión visual | Cero diferencias no aprobadas en fixtures desktop/light/dark/read-only/error. |

## Alcance

Incluye:

- Reparar pérdida de trabajo y bloqueos UX existentes.
- Arquitectura de información y lenguaje de producto.
- Onboarding y creación/generación inicial.
- Shell, navegación, Settings, About y Help.
- Workspace editorial, asistentes, revisión, versiones, importaciones,
  simulación y narrativa.
- Migración controlada a React/Vite y sistema visual accesible.
- Responsive, teclado, contrastes, performance y visual regression.

No incluye:

- Cambiar invariantes de dominio o autoridad de ChangeSet.
- Colaboración multiusuario, nube o sincronización.
- Marketplace, plugins o segundo proveedor.
- Generación automática de novelas.
- Reescritura del backend para acomodar un framework frontend.
- Telemetría remota o estudios enviados sin consentimiento.
- Mantener dos implementaciones frontend después de migrar cada superficie.

## Reglas de ejecución

1. Resolver primero pérdida de datos, revisiones huérfanas y estado busy.
2. Probar journeys por tareas humanas, no por presencia de botones.
3. Mantener Rust como validación autoritativa; el frontend solo anticipa errores
   triviales y presenta los errores de dominio junto al campo correcto.
4. Una superficie tiene un solo owner. Al migrarla a React se elimina el renderer
   imperativo correspondiente.
5. shadcn aporta código, no decisiones visuales: cada componente se revisa,
   simplifica y adapta a tokens Nirmata.
6. UUID, URI, JSON, ticks y DSL quedan ocultos por defecto bajo opciones
   avanzadas o reemplazados por pickers.
7. Todo flujo read-only debe verse read-only antes de interactuar.
8. Ninguna salida IA se presenta como canon; toda propuesta termina en la cola
   global de revisión.
9. No declarar una fase completada sin build, comportamiento, teclado,
   accesibilidad y screenshots verificados.
10. Actualizar este backlog con métricas y resultados reales durante la ejecución.

## Leyenda de estados

| Estado | Significado |
|---|---|
| `Pendiente` | No iniciada. |
| `En progreso` | Trabajo activo con dependencias satisfechas. |
| `Bloqueado` | Existe un impedimento concreto registrado. |
| `Completado` | Implementación y criterio de aceptación verificados. |

## Secuencia recomendada

Las fases son secuenciales. P0 estabiliza el producto actual; luego se valida el
stack y se migra por superficies completas. Una fase posterior no justifica
mantener dos UIs para la misma capacidad.

## Fase 0 — Seguridad UX y baseline

**Resultado:** la UI actual deja de perder trabajo, esconder estados o mantener
revisiones huérfanas antes de comenzar la migración visual.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-001 | Completado | — | Reparar visibilidad closed/world. | `[hidden]` vuelve a imponer `display:none`; safety verifica la regla y la captura closed `960x680` excluye por completo `world-view`. | `npm run build` y frontend safety 10/10 pasaron; captura cerrada muestra exclusivamente crear/abrir. | `frontend/index.html`, `styles.css` |
| UX-002 | Completado | UX-001 | Proteger formularios sucios al navegar. | `selectUri` protege el editor; `selectUriInScope` confirma antes de cambiar backend; búsqueda, filtros, citas, diff, revisión y undo reutilizan el guard por alcance y restauran foco/selector al cancelar. | Frontend build y safety verifican helper único; navegación ordinaria ignora revisiones que no perderá y cambios de workspace conservan guard amplio. | `workspace.ts`, `variant-ui.ts`, `assistant.ts` |
| UX-003 | Completado | — | Descartar revisiones realmente. | Revisiones IA invocan `discard_ai_run`; manual/merge/snapshot/simulación invocan `discard_manual_review`. La tarjeta se retira solo después del éxito backend. | Test app confirma que descartar libera la clave estable y permite recrear el mismo draft sin `ReviewSessionConflict`; fallo conserva tarjeta. | `render-pending.ts`, `manual_forms.rs` |
| UX-004 | Completado | UX-003 | Unificar edición de revisión. | Se eliminó `Abrir formulario` y la restauración silenciosa de valores pendientes al navegar. `Editar cambio` usa `begin/apply_manual_review_edit`; `Ver objeto actual` muestra canon solo si existe before. | Test app conserva `reviewKey`, `operationId`, número de operaciones y revisión canónica; frontend safety rechaza rutas duplicadas. | `render-pending.ts`, `editor-model.ts`, manual review |
| UX-005 | Completado | — | Estado global busy de IA. | Tauri expone `get_ai_activity` sin tomar el lock del mundo y la UI comparte una única actividad entre asistente, lore y narrativa. Una franja global cancelable bloquea controles que invocarían el backend, conserva el contenido visible y restaura el estado previo al terminar, fallar o cancelar; `app_busy` tiene recuperación en español. | Frontend build y safety 13/13 pasaron; desktop 15/15 verifica la bandera activa sin bloquear app. | `main.rs`, `state.ts`, `main.ts`, assistant, lore, narrativa |
| UX-006 | Completado | UX-005 | Diagnóstico previo de IA. | `get_ai_provider_status` distingue credencial, secure store, endpoint HTTPS, modelo y conexión no comprobada. `Probar conexión` ejecuta un request mínimo cancelable sin contexto del mundo ni ChangeSet; asistente, lore y escritura narrativa permanecen deshabilitados hasta verificarlo, con acción directa a configuración. | Frontend build/safety 14/14, desktop 16/16 y app check pasaron; tests cubren estados locales y códigos estables de timeout/transporte/HTTP. | `main.rs`, `ai.rs`, `assistant.ts`, provider status |
| UX-007 | Completado | UX-003 | Reparar importación bloqueada. | Los `DecisionPoint`s viven en estado y se renderizan como tarjetas accionables para identidad ambigua, autoridad de claims o rechazo. Lore escucha progreso de extracción/revisión, cada estado indica siguiente acción y borrar lote retira también su tarjeta pendiente local. | Frontend build/safety 14/14, lore import 8/8 y desktop 16/16 pasaron; los prompts sobreviven al `finally` y al rerender. | `lore-import.ts`, `lore_import.rs` |
| UX-008 | Completado | UX-002 | Proteger trabajo efímero. | `ephemeralWork` registra lote activo, escenarios/formulario/mappings/resultados de simulación y derivaciones/formulario narrativo. Cierre y cambio de variante enumeran lo que se perderá; al aceptar se limpian owners locales y simulación se rotula como estado de sesión. | Frontend build/safety 15/15 y desktop 16/16 pasaron; `beforeunload`, cierre y switch comparten el guard y no descartan silenciosamente. | `state.ts`, `workspace.ts`, simulation/lore/narrative |

## Fase 1 — Modelo mental, journeys y sistema visual

**Resultado:** existe una especificación de producto comprensible y una identidad
visual propia antes de elegir componentes.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-009 | En progreso | UX-001–UX-008 | Baseline de usabilidad. | Protocolo B1–B5, registro crudo y métricas honestas definidos; auditoría visual cubre bloqueos cualitativos sin imputar tiempos desde tests. | Pendiente ejecutar sesiones moderadas y publicar tasas/tiempos reales. | `docs/validation/ux-baseline.md` |
| UX-010 | En progreso | UX-009 | Arquitectura de información. | Árbol congelado, cinco tareas críticas y cinco diagnósticas definidos; gate exige 4/5 por tarea sin agregación engañosa. | Pendiente ejecutar cinco tree tests internos independientes. | `docs/validation/information-architecture-tree-test.md` |
| UX-011 | En progreso | UX-009 | Glosario y content design. | Mapa de etiquetas de producto y copy principal migrados: propuestas, cambios, versiones, advertencias, narrativa y simulación. Los formatos técnicos restantes continúan visibles porque la UI antigua apila laboratorios y formularios avanzados. | Pendiente ocultar UUID/URI/JSON/ticks/DSL con la nueva shell y pickers para lograr cero términos internos en flujo básico. | `docs/product/content-design.md`, `helpers.ts`, renderers |
| UX-012 | En progreso | UX-010–UX-011 | Modelo mental de versiones. | Barra implementada con `Escribiendo en`, `Viendo`, `Versión actual` y `Solo lectura`; selectores y rótulo ya no muestran IDs, y revisión interna vive en detalles técnicos. | Safety verifica ausencia de IDs visibles; pendientes cinco explicaciones independientes correctas sobre destino de cambio. | `docs/product/content-design.md`, `variant-ui.ts` |
| UX-013 | Completado | UX-010 | Journey de creación/generación. | Landing navegable presenta manual, base con IA y material existente sobre un único formulario y un solo `create_world`. IA construye un resumen acotado y abre Proponer cambios; importación abre el owner existente, sin duplicar generación ni revisión. | Frontend build y safety 17/17 pasaron; teclado/a11y y capturas `960x680`/`390x844` verifican caminos, atrás sin crear archivo y copy de revisión previa. | `main.ts`, `index.html`, `docs/product/creation-and-ai-journeys.md` |
| UX-014 | Completado | UX-010 | Journey preguntar/modificar. | `AiQueryResponse.proposalAction` produce `Convertir en propuesta` con confirmación y stale scope guard; `Pedir un cambio sobre la selección` y creación IA reutilizan el mismo evento/modo y único `execute_ai_proposal`. | Safety verifica una sola ejecución; recorrido en browser confirmó modo `Proponer cambios`, solicitud heredada y foco, sin escritura ni revisión paralela. | `assistant.ts`, `render-editor.ts`, AI interaction model |
| UX-015 | Completado | UX-010 | Especificación Settings/About. | Inventario limitado a capacidades reales: proveedor fijo, secure store, proyecto, detalles técnicos, metadata Tauri, licencia y privacidad local-first. Se explicitan secciones sin preferencias y datos que no deben inventarse. | Revisión de contratos confirma que secretos nunca regresan a JS y que no se agregaron parámetros hipotéticos. | `docs/product/settings-and-about.md` |
| UX-016 | Completado | UX-011 | Sistema visual Nirmata. | Tokens light/dark, autoridad, severidad, tipografía editorial/UI, densidad, grid desktop/narrow, iconografía, forced colors y reduced motion definidos con gates. | Ratios documentados superan AA; dirección elimina dashboard/cajas por defecto y no prescribe dependencias. | `docs/product/visual-system.md` |

## Fase 2 — Plataforma frontend y gates tecnológicos

**Resultado:** Vite/React y las primitivas elegidas funcionan dentro de Tauri con
tests y presupuestos antes de migrar workflows.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-017 | Bloqueado | UX-016 | Gate de WebViews. | Decisión fechada: CSS convencional sin Tailwind/fallback; matriz inicial Windows 11 WebView2 ≥111, macOS Sonoma 14+ y Ubuntu 24.04 WebKitGTK ≥2.44. Windows 11 build 26200 y runtime 151 fueron detectados. | Pendiente ejecutar fixture/binario real en WKWebView y WebKitGTK; Playwright WebKit no sustituye ese gate. | `docs/architecture/frontend-platform.md` |
| UX-018 | Completado | UX-017 | Migrar build a Vite. | `build.mjs` eliminado; Vite es el único dev/build, TypeScript estricto usa `--noEmit`, puerto dev 1420 estricto y Tauri define `devUrl`/`frontendDist`. Producción no genera source maps. | `npm ci`, typecheck, build y dev HTTP 200 pasaron; bundle actual: JS 133,24 kB/34,09 gzip y CSS 15,15 kB/3,93 gzip; desktop build pasó. | `package.json`, `vite.config.ts`, `tauri.conf.json` |
| UX-019 | Completado | UX-018 | Crear root React. | React posee el root de proyecto cerrado con `RootErrorBoundary` y `SessionProvider`; `ClosedView` reemplazó atómicamente markup/listeners imperativos. `world-view` permanece como superficie hermana con owner único y módulos cargados una vez. | Typecheck/build y safety 19/19 pasaron; smoke browser crear→abrir→cerrar verifica mount/remount, cleanup, foco y reset sin listeners ni renderer duplicado. Bundle 95,12 kB gzip. | `main.tsx`, `closed-view.tsx`, `session-provider.tsx` |
| UX-020 | En progreso | UX-017–UX-019 | Integrar Tailwind v4 o fallback aprobado. | Fallback aprobado significa CSS convencional único con variables semánticas; Vite y la primera superficie React lo consumen sin Tailwind, PostCSS, CDN ni hoja paralela. | Bundle y WebView2 pasan; pendiente cerrar UX-017 con WKWebView/WebKitGTK reales para afirmar matriz completa. | `frontend-platform.md`, `styles.css` |
| UX-021 | Completado | UX-016, UX-020 | Implementar tokens y temas. | Tokens `--n-*` gobiernan landing y mundo abierto; system/light/dark/high contrast persiste localmente, responde al OS, define color-scheme, forced colors, foco, severidad/autoridad y reduced motion. Componentes no contienen colores literales ni aliases legacy. | Unit 3/3, E2E 7/7 y safety 20/20 pasan; axe cubre landing, workspace, read-only y error en temas; screenshots app-wide light/dark/high contrast y forced-colors verificados. | `appearance.ts`, `styles.css`, `visual-system.md`, E2E themes |
| UX-022 | En progreso | UX-019 | Adoptar Radix mínimo. | Dialog y Tabs se usan en Settings/About con imports directos, trap y retorno de foco verificados. Las demás primitivas no se agregan hasta sustituir un uso real de confirmación, menú, tooltip o picker. | E2E verifica Escape y retorno de foco; pendiente retirar confirm/prompt legacy para cerrar la adopción accesible. | `software-dialogs.tsx`, Radix accessibility |
| UX-023 | Pendiente | UX-021–UX-022 | Adoptar shadcn selectivo. | Copiar y revisar Button, Field, Dialog, Sheet, Tabs, Tooltip, Badge, ScrollArea, Skeleton y Toast; eliminar variantes no usadas. | Cada componente tiene owner, tokens y test; no se agrega catálogo completo. | shadcn Vite |
| UX-024 | Completado | UX-018–UX-019 | Migrar API Tauri a imports. | `invoke`/`listen` usan módulos oficiales y dialog usa `@tauri-apps/plugin-dialog`; se eliminaron `TauriApi` y el global manual, y Tauri desactiva `withGlobalTauri`. Los comandos específicos permanecen explícitos. | Frontend build/safety 18/18 y desktop build pasaron; safety rechaza `window.__TAURI__` y verifica imports tree-shakeables. | `state.ts`, `types.ts`, `tauri.conf.json` |
| UX-025 | En progreso | UX-024 | Introducir TanStack Query. | QueryClient posee recientes, diagnóstico IA, metadata, palette y explorador; claves world incluyen mundo/variante/revisión observada o cabeza actual, invalidan tras scope y se eliminan al cerrar mundo, retry false y sin optimistic canon. | Search/VFS ya no cruzan scopes; pendiente migrar contexto, editor, revisiones y demás lecturas legacy antes de cerrar la plataforma. | `main.tsx`, `world-shell.tsx`, `world-explorer.tsx` |
| UX-026 | En progreso | UX-019 | Introducir React Hook Form. | El wizard usa RHF para valores, validación y foco del primer error sin duplicar reglas de dominio ni Zod. | Pendiente migrar editores estructurados y field arrays para medir rerenders del workspace. | `closed-view.tsx`, RHF |
| UX-027 | Pendiente | UX-019, UX-022 | Paneles redimensionables. | Integrar splitters horizontal/vertical, teclado, collapse y persistencia local. | Separadores tienen rol/nombre/valor; 55+ fps y alternativa al drag. | react-resizable-panels, WAI-ARIA splitter |
| UX-028 | Completado | UX-018–UX-024 | Harness de tests frontend. | Vitest/jsdom + Testing Library/user-event cubren comportamiento React; Playwright usa IPC Tauri simulado sobre imports oficiales, axe y screenshot. Safety source permanece como gate de seguridad secundario. | Unit 2/2, E2E 1/1 y safety 19/19 pasaron; landing verifica roles, foco/teclado, cero axe serious/critical y screenshot `960x680`. | `tests/closed-view.test.tsx`, `e2e/landing.spec.ts`, Playwright |

## Fase 3 — Shell, onboarding y software de escritorio

**Resultado:** abrir Nirmata muestra un producto navegable con generación inicial,
Settings y About, no una página larga.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-029 | Completado | UX-019–UX-028 | Pantalla Inicio. | Inicio React reúne crear, abrir, recientes, recuperación de movidos, diagnóstico IA, Settings y Ayuda; con mundo cerrado no muestra features del workspace. | Captura `960x680`, teclado y axe verifican todas las decisiones en el primer viewport y cero contenido del mundo. | `closed-view.tsx`, `landing.spec.ts` |
| UX-030 | Completado | UX-029 | Proyectos recientes. | Tauri persiste hasta 12 rutas/nombre/world ID/última apertura en settings locales; create/open actualizan, un click reabre, quitar no toca canon y `file_not_found` ofrece localizar o retirar. | Unit Rust verifica round-trip sin abrir mundos; Testing Library verifica reapertura en una acción y el fallo de settings nunca bloquea un mundo ya abierto. | `main.rs`, `closed-view.tsx`, desktop tests |
| UX-031 | Completado | UX-013, UX-029 | Wizard Nuevo mundo. | Manual, IA e importación comparten pasos Proyecto, intención/handoff y resumen; Back/Cancelar conservan cero escrituras y un único submit final llama `create_world`. | Unit y E2E verifican foco de error, progreso, ausencia de Crear antes del resumen, cancelación inerte y narrow sin overflow. | `closed-view.tsx`, landing tests |
| UX-032 | Completado | UX-031 | Crear base del mundo con IA. | Brief recoge género, premisa, temas, tono, escala pequeña/mediana y restricciones; el resumen explica alcance y después abre el owner único Proponer cambios. | Una sola creación de proyecto; la IA solo produce la propuesta estándar y no existe escritura anterior a `Aplicar al mundo`. | `closed-view.tsx`, `assistant.ts`, closed-view tests |
| UX-033 | Completado | UX-031 | Inicio desde material existente. | Wizard explica copia inerte y abre el owner existente después de crear el proyecto; selector acepta varios Markdown/textos, Tauri calcula su raíz común y app conserva preview/hash/chunks inertes por fuente antes de extracción y revisión. | Unit Rust verifica selección vacía/raíz compartida; fixtures hostiles existentes conservan originales y ningún candidato escribe canon directamente. | `closed-view.tsx`, `lore-import.ts`, `main.rs` |
| UX-034 | Completado | UX-029–UX-033 | Checklist de onboarding. | Inicio del mundo ofrece checklist no canónico de premisa, reglas, lugares/facciones, personaje/meta, eventos y primera consulta; progreso y descarte se guardan localmente por mundo y Ayuda permite reabrirlo. | E2E verifica marcar, persistir, ocultar y reabrir sin comandos de canon; el experto puede ocultarlo y conserva accesos directos. | `world-shell.tsx`, `software-dialogs.tsx`, workspace E2E |
| UX-035 | Completado | UX-012, UX-019 | Barra superior editorial. | Topbar React muestra proyecto, `Escribiendo en`, `Viendo`, solo lectura, búsqueda, contador de cambios, Settings, Ayuda y cerrar. | E2E light/dark/high contrast verifica una línea desktop, axe y ausencia de UUID en la barra. | `world-shell.tsx`, workspace E2E |
| UX-036 | Completado | UX-010, UX-035 | Sidebar de áreas. | Inicio, Mundo, Cronología, Asistente, Narrativa, Simulación, Importaciones, Versiones y Settings comparten navegación colapsable; solo el owner activo se muestra y los demás permanecen montados. | E2E cambia Simulación→Importaciones→Simulación y conserva el formulario; `aria-current`, anuncio/foco de área y cero overflow pasan. | `world-shell.tsx`, workspace E2E |
| UX-037 | Completado | UX-024–UX-036 | Command palette. | `cmdk` abre con `Ctrl/Cmd+K` y agrupa áreas, objetos de la versión observada, crear por tipo, cambiar variante/versión, preguntar, proponer, cambios, Settings, Ayuda y cerrar. Resultados usan query key mundo/variante/revisión y abren por `selectUri`; read-only deshabilita escritura. | E2E mide apertura <100 ms, filtro/Enter, objeto exacto sin FTS/UUID, creación, variante protegida y Escape con retorno de foco; cambiar mundo limpia su caché. | `command-palette.tsx`, `world-shell.tsx`, workspace E2E |
| UX-038 | Completado | UX-015, UX-021, UX-035 | Settings. | Dialog disponible con/sin mundo reúne General, apariencia, IA, proyecto, accesibilidad y avanzado; usa diagnóstico/conexión y credencial segura sin selector de proveedor inventado. | E2E axe/Escape/foco y unit pasan; versión narrow no desborda y ningún comando devuelve la clave a JS. | `software-dialogs.tsx`, `settings-and-about.md` |
| UX-039 | Completado | UX-015, UX-035 | About y Help. | About muestra versión Tauri, identificador, licencia declarada, local-first y privacidad; Centro de ayuda separado cubre creación, preguntar/proponer, versiones, importación, atajos y glosario, con acceso antes o después de abrir mundo. | E2E verifica ambos dialogs, axe, Escape y retorno de foco; no inventa build ID, URL empaquetada ni texto de licencia inexistente. | `software-dialogs.tsx`, `settings-and-about.md` |

## Fase 4 — Workspace editorial, asistente y revisión

**Resultado:** el trabajo diario ocurre en un IDE claro con referencias por nombre,
asistente acoplable y una sola cola de revisión.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-040 | Completado | UX-025–UX-036 | Explorador del mundo. | React posee búsqueda/lista, árbol plegable, recientes de sesión, filtros y creación por tipo en un único host; renderer/listeners imperativos se eliminaron. URI y UUID viven solo en detalles avanzados. | E2E carga 200 resultados, mide primer paint <50 ms, abre por `selectUri`, conserva selección/nombre tras renombre y no muestra IDs; axe pasa. | `world-explorer.tsx`, search/VFS, workspace E2E |
| UX-041 | Completado | UX-037, UX-040 | Búsqueda global. | Palette y explorador consultan el mismo `search_world` por nombre/contenido y scope exacto; muestran snippet limpio, tipo, autoridad, clasificación, vacío/error y score solo en detalles técnicos. | Teclado abre fuentes exactas mediante URI estable sin mostrar `FTS`; palette y explorador comparten query keys y resultados no sobreviven al mundo/scope. | `command-palette.tsx`, `world-explorer.tsx`, Retrieval |
| UX-042 | En progreso | UX-040–UX-041 | Pickers de objetos. | Un Dialog React reutilizable consulta por scope y completa por nombre campos escalares de relación, evento, afirmación, meta y documento; UUID/URI queda editable bajo disclosure y metas afectadas admiten selección múltiple. | E2E completa entidad destino sin copiar ID y verifica foco/valor técnico; pendientes campos compuestos de participantes/causalidad y simulación para cerrar el criterio completo. | `object-picker.tsx`, `render-editor.ts`, workspace E2E |
| UX-043 | En progreso | UX-026, UX-042 | Editores estructurados. | Los siete agregados conservan paridad backend, dirty guard y errores inline; enums/objetivos están en español, IDs salen del encabezado, pickers cubren referencias escalares y JSON/ticks/DSL viven en opciones avanzadas. | Captura valida lenguaje y disclosure; pendientes tablas de participantes/causalidad, condicionales completos y migración RHF antes de cerrar todos los tipos. | manual forms, `render-editor.ts` |
| UX-044 | Completado | UX-043 | Experiencia Markdown segura. | Todos los campos `*_md`/`body_md` ofrecen preview opcional construido con nodos DOM, sin parser HTML; referencias internas llaman `selectUri` y HTTPS se rotula como enlace externo. | E2E mantiene `<img onerror>` como texto, no crea img/script, navega referencia interna, explicita enlace externo y conserva byte por byte el textarea. | `render-editor.ts`, workspace E2E, Security policy |
| UX-045 | Completado | UX-025, UX-040 | Contexto situado. | React posee tabs Canon, Perspectivas, Metas, Cronología y Avisos; reúne links del agregado, reglas, claims, goals, obligaciones y evidencia con acciones de navegación protegidas. | E2E cambia tabs con evidencia canónica/situada, oculta IDs, axe pasa y el evento de selección actualiza contexto sin recargar editor/explorador. | `world-context.tsx`, ContextBundle, workspace E2E |
| UX-046 | Completado | UX-043, UX-045 | Cronología operable. | Vista React propia muestra tiempo conocido por fecha/tick y no especificado por separado, filtro por resumen/tipo, densidad cómoda/compacta y apertura protegida del evento; discurso permanece en Estudio narrativo. | E2E verifica orden backend, fecha convertida, ongoing/aproximado sin extremo inventado, unknown separado y navegación exacta. | `world-timeline.tsx`, EventTime/calendar, workspace E2E |
| UX-047 | Completado | UX-014, UX-022, UX-035 | Asistente acoplable. | El owner existente se presenta como sheet derecho desktop y pantalla superpuesta narrow; conserva Consultar/Proponer/avanzados, contexto y fuentes sin ocupar Inicio o Mundo permanentemente. | E2E escribe una solicitud, cierra, verifica foco en el disparador y reabre con el texto intacto; creación IA y palette abren el mismo owner. | `world-shell.tsx`, `assistant.ts`, workspace E2E |
| UX-048 | Pendiente | UX-047 | Conversaciones locales reales. | Mensajes usuario/asistente, nueva conversación, borrar, historial local, contexto explícito y sin canonización. | Multi-turn probado; borrar conversación no cambia mundo; no se llama transcript a un solo resultado. | Interaction history |
| UX-049 | Pendiente | UX-047–UX-048 | Convertir respuesta en propuesta. | CTA sobre consulta, muestra request/contexto heredado y exige confirmación antes de cambiar modo. | Nunca cambia silenciosamente a escritura; propuesta llega a revisión global. | Query proposal action |
| UX-050 | Pendiente | UX-047 | Perfiles avanzados de IA. | Revisión profunda y auditoría en menú avanzado con explicación, roles y costes; auditoría claramente read-only. | Usuario distingue autoridad y salida; deep review nunca parece commit automático. | Deep review |
| UX-051 | En progreso | UX-035, UX-043, UX-047 | Cola global de revisión. | Badge y drawer global funcionan sobre cualquier área, conservan foco y reúnen el mapa único de revisiones con origen manual/IA/import/simulación/versiones/snapshot; historial queda dentro de la cola, no en Inicio. | E2E verifica drawer global; pendiente persistir/listar revisiones backend para recuperarlas después de reiniciar el proceso. | `world-shell.tsx`, `render-pending.ts`, ManualReview |
| UX-052 | Completado | UX-051 | Tarjeta de revisión orientada a acción. | Cada tarjeta reúne origen, objeto/tipo, vigencia, objetivo, fuentes navegables, operaciones before/after, issues/waivers/decisiones, crítica final, revalidación y acciones `Aplicar al mundo`/descartar. | Ningún bloqueo exige otra superficie; tests app existentes verifican stale/final critic/commit y descarte libera la revisión backend antes de retirar la tarjeta. | `render-pending.ts`, manual review tests, ChangeSet workflow |

## Fase 5 — Versiones, importaciones y administración del proyecto

**Resultado:** versionado e importación se entienden como herramientas del
proyecto, no como controles técnicos permanentes.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-053 | Pendiente | UX-012, UX-035–UX-036 | Workspace Versiones. | Lista/árbol de variantes, origen, última versión, activa/observada/archivada y acciones nombradas. | No aparece `head`; cambiar y crear desde historia es comprensible en test. | Variants |
| UX-054 | Pendiente | UX-053 | Comparación visual. | Nombres de ambos lados, before/after inline, campos cambiados, retcon, referencias y procedencia. | No usa `izquierda/derecha`; diferencias backend se muestran sin cambiar scope repetidamente. | VariantComparison |
| UX-055 | Pendiente | UX-052–UX-054 | Merge guiado. | `Traer cambios de B hacia A`, operaciones automáticas/manuales y copy humano para keep/take. | Destino y fuente visibles antes de preparar; toda colisión termina en revisión global. | MergeReview |
| UX-056 | Pendiente | UX-053 | Historial y undo. | Timeline de revisiones, filtro por objeto, diff, autor/fecha/fuente y explicación del único undo disponible. | `Revisión` histórica no se confunde con `Revisar cambios`; undo crea nueva versión visible. | RevisionHistory |
| UX-057 | Completado | UX-010, UX-036 | Centro de importaciones. | Área Importaciones reúne tabs `Lore` y `Snapshot`: lore conserva lote/progreso/candidatos y snapshot explica backup estructurado, diff/revisión y diferencia con prosa o `.nirmata`; las acciones snapshot se retiraron de Versiones. | E2E verifica exclusividad de tabs, explicación y acciones correctas; ambos owners mantienen escritura exclusivamente mediante revisión. | `import-center.tsx`, lore/snapshot |
| UX-058 | Pendiente | UX-007, UX-042, UX-057 | Wizard de lore. | Multiarchivo, lotes reanudables, reemplazar fuente, progreso, hashes avanzados, candidatos completos y resolución de identidad. | Cerrar/reabrir permite reanudar; todas las decisiones tienen siguiente acción. | Lore import backend |
| UX-059 | Pendiente | UX-052, UX-057 | Snapshot y backup. | Export/import con resumen de mundo/variante/revisión/hash y diff antes de revisión; nombres sin `window.prompt`. | Snapshot cross-variant/stale se explica; importar nunca escribe directo. | VFS snapshots |
| UX-060 | Pendiente | UX-038, UX-053 | Settings de proyecto. | Ruta, schema, integridad, variante activa, backup y detalles técnicos copiables. | Diagnóstico visible sin exponer SQL; acciones peligrosas confirman objeto exacto. | Store metadata |
| UX-061 | Pendiente | UX-005, UX-023 | Estado y errores globales. | Toast para éxito breve, banners para bloqueo, error boundary, códigos traducidos y acción retry/recover. | Cero errores crudos en inglés en journeys principales; errores no destruyen draft. | CommandError |

## Fase 6 — Experiencias avanzadas y responsive

**Resultado:** simulación, narrativa y calendario usan controles de dominio y
funcionan en ventanas pequeñas sin contaminar el workspace diario.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-062 | Pendiente | UX-036, UX-042, UX-052 | Laboratorio de simulación. | Nombre, pickers de facción/recurso, tabla de stocks, builder de reglas y DSL solo avanzado; pasos/resultados y promoción humana. | Cero UUID obligatorio; rotula persistencia de sesión y claim no es canónico por defecto. | Simulation |
| UX-063 | Pendiente | UX-036, UX-042, UX-045 | Estudio narrativo. | Tabs Cronología, Causalidad, Cabos abiertos y Documentos; vista analizada visible y resultados limpiables. | Story/discourse y fuentes comprensibles; derivar en histórico, escribir solo en actual. | Narrative derivations |
| UX-064 | Pendiente | UX-052, UX-063 | Documento interno con preview. | Pickers de perspectiva/fecha, tipo/título/request, preview seguro y CTA a revisión. | Secreto inaccesible no aparece; fallo/cancelación no crea tarjeta. | Internal document |
| UX-065 | Pendiente | UX-043, UX-046 | Calendario amigable. | Builder de weekdays/meses, date picker ficticio por números y vista tick avanzada; no DSL obligatorio. | Cambiar nombres solo cambia display; round-trip Rust conserva tick. | WorldCalendar |
| UX-066 | Pendiente | UX-032, UX-047 | Plantillas de expansión con IA. | Facción, ciudad, personaje, conflicto, cronología y consecuencias; brief editable y escala visible. | Cada plantilla produce ChangeSet acotado y fuentes; no existe `generar novela`. | Propose/IntentBrief |
| UX-067 | Pendiente | UX-034, UX-040–UX-066 | Estados vacíos contextuales. | Siguiente acción por área, ejemplos y sample prompts; sin bloquear al experto. | Mundo vacío guía hasta primer conjunto aplicado; ayudas se pueden ocultar. | Onboarding |
| UX-068 | En progreso | UX-027, UX-035–UX-067 | Responsive desktop/mobile narrow. | Shell cambia sidebar por navegación horizontal; asistente/dialogs son sheets y cada área muestra un solo owner en `720x520`/`390x844`. | E2E y capturas prueban cero overflow y exclusividad visual; pendientes paneles redimensionables y completar todos los pickers/editores narrow. | workspace E2E, capturas baseline |
| UX-069 | En progreso | UX-022–UX-068 | Teclado y accesibilidad manual. | Dialogs, Help, palette, sidebar y assistant sheet tienen foco/restauración, live labels, reduced motion, forced colors y navegación por teclado. | Axe 14 recorridos sin serious/critical; pendientes recorrido completo de editores legacy, roving focus y smoke NVDA/WebView2 real. | WCAG 2.2, frontend E2E |

## Fase 7 — Calidad visual, performance y aceptación

**Resultado:** la UI React sustituye completamente la implementación imperativa,
cumple presupuestos y se distribuye como software de escritorio coherente.

| Código | Estado | Dependencias | Entregable | Detalle | Criterio de aceptación | Referencias |
|---|---|---|---|---|---|---|
| UX-070 | En progreso | UX-028–UX-069 | Suite de comportamiento. | Testing Library cubre Inicio/wizard/recientes/Settings/About y Playwright cubre shell, áreas, responsive, onboarding, palette, asistente, temas y estados. Safety sigue secundario. | 5 unit y 14 E2E pasan; pendientes paridad React de editar/revisar/aplicar/undo y recorridos completos de features avanzadas. | frontend tests |
| UX-071 | Pendiente | UX-028, UX-070 | Regresión visual Playwright. | Baselines light/dark, 720/960/1440, empty/loaded/read-only/error/review/IA; entorno fijo. | Cero cambios no aprobados; screenshots adjuntos a reporte. | Playwright snapshots |
| UX-072 | Pendiente | UX-021–UX-071 | Gate WCAG automatizado. | axe, contraste, nombre accesible del selector, forms id/name/autocomplete y árbol a11y. | Cero serious/critical; contraste de botones ≥ 4.5:1. | Auditoría Lighthouse actual |
| UX-073 | En progreso | UX-018–UX-071 | Gate de performance y bundle. | Build mide 151,45 KiB JS gzip y 9,23 KiB CSS gzip; palette abre <100 ms y explorer con 200 resultados pinta un filtro <50 ms en Playwright. | Presupuestos medidos pasan; Vite advierte chunk único >500 kB sin comprimir, por lo que quedan code splitting real, P95 startup/open/forms/resize y memory/listeners en hardware de referencia. | Vite build, workspace E2E |
| UX-074 | Pendiente | UX-035, UX-038–UX-039 | Integración nativa de software. | Menú de aplicación para proyecto, edit, view, Settings, Help/About; shortcuts coherentes; dialogs Tauri. | Settings/About accesibles desde menú y shell; versión coincide con Tauri config. | Tauri menus/windows |
| UX-075 | Pendiente | UX-074 | Empaquetado y smoke productivo. | Activar bundle cuando assets/iconos/versionado estén listos; Windows installer inicial y firma como gate posterior si aplica. | Build release e instalador abren, crean/reabren `.nirmata` y desinstalan limpiamente. | Tauri distribute |
| UX-076 | Pendiente | UX-009–UX-075 | Evaluación de usabilidad final. | Repetir tareas baseline y SUS/cualitativo; incluir usuario nuevo y experto. | 90 % completa crear/generar/modificar/revisar sin ayuda; mejora medible frente a baseline. | UX-009 |
| UX-077 | Pendiente | UX-011, UX-039, UX-076 | Documentación y ayuda final. | Manual in-app, glosario, atajos, privacidad, generación, versiones e importaciones; actualizar screenshots. | Ayuda responde las preguntas de este backlog sin requerir conocimiento técnico. | docs/product, docs/architecture |
| UX-078 | Pendiente | UX-070–UX-077 | Retirar frontend antiguo y aceptar rediseño. | Eliminar `querySelector` globals, renderers imperativos, build.mjs, CSS sustituido, tipos Tauri globales y bridges temporales. | Un solo frontend React; build/test/release verdes; Definition of Done UX completa. | AGENTS.md |

## Definition of Done UX/UI

El rediseño está completo únicamente cuando:

1. Proyecto cerrado y mundo abierto son estados mutuamente exclusivos visual y
   semánticamente.
2. Una persona nueva puede crear manualmente, crear una base con IA o importar
   material desde la pantalla inicial.
3. Preguntar y proponer cambios son acciones distintas y evidentes; convertir
   una respuesta en propuesta requiere confirmación.
4. Toda escritura termina en una cola global que explica qué cambia, por qué,
   fuentes, bloqueos y siguiente acción.
5. `Aplicar al mundo` es la única acción de commit y nunca está disponible para
   propuestas desactualizadas o read-only.
6. Variante activa, vista observada e historia se entienden sin usar `head`.
7. Ningún flujo básico exige UUID, URI, JSON, tick o DSL; siguen disponibles en
   detalles avanzados.
8. Navegar nunca pierde formularios, escenarios, lotes o revisiones sin aviso.
9. Settings y About existen como superficies de software y funcionan sin mundo.
10. El workspace diario aparece en el primer viewport; features avanzadas se
    abren por navegación o lazy load.
11. La app funciona por teclado, tiene foco visible/restaurado y pasa WCAG 2.2 AA
    en light/dark/read-only/error.
12. Los viewports `720x520`, `960x680`, `1440x900` y `390x844` no apilan toda la
    aplicación ni requieren scroll horizontal.
13. El stack final es React/Vite con una sola estrategia de estilos aprobada;
    cada dependencia nueva tiene uso, owner y presupuesto.
14. Los tests de comportamiento, screenshots, axe, performance, frontend build,
    Cargo y desktop release pasan sin errores conocidos.
15. La implementación antigua se elimina; no quedan dos UIs ni compatibility
    shims permanentes.
