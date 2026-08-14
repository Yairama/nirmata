# Plataforma frontend y WebViews

**Estado:** decisión fechada el 12 de agosto de 2026; validación macOS/Linux
pendiente de hardware real.

## Estrategia de estilos

Nirmata usa CSS convencional con variables semánticas. No adopta Tailwind CSS
v4 ni mantiene una hoja alternativa.

La decisión aplica YAGNI: el sistema visual ya define tokens CSS, React todavía
no ha demostrado duplicación que justifique utilities y Tailwind v4 eleva el
piso a Chrome 111, Safari 16.4 y Firefox 128 sin publicar una equivalencia
oficial para WebKitGTK.

La integración de estilos queda cerrada independientemente del gate de hardware:
existe una sola hoja de producción, sin Tailwind, CDN, PostCSS paralelo ni
fallback duplicado. La validación del binario en WKWebView y WebKitGTK permanece
como requisito separado de la matriz de plataformas.

Las primitivas nativas, Radix importado desde sus módulos reales y componentes
locales de cada feature cubren Button, Field, Dialog, Sheet, Tabs, Badge,
ScrollArea y Toast. No se adopta un catálogo shadcn: copiarlo ahora duplicaría
soluciones ya verificadas. Un componente individual solo se reconsiderará ante
una interacción concreta y reutilización real.

## Matriz inicial

| Plataforma | Piso de producto | WebView | Estado |
|---|---|---|---|
| Windows | Windows 11 x64 actualizado | WebView2 Evergreen `>=111` | Windows 11 build 26200 y runtime 151 detectados; build y capturas pasan. |
| macOS | Sonoma 14+ | WKWebView del sistema actualizado | Requiere Mac Intel/Apple Silicon real. |
| Linux | Ubuntu 24.04 LTS x64 actualizado | `libwebkit2gtk-4.1-0 >=2.44` | Requiere Wayland/X11 y paquete real. |

Windows 7/8, Windows 10 sin ESU, macOS anteriores a Sonoma, Ubuntu 20.04,
Linux ARM y distribuciones universales quedan fuera del soporte inicial. Una
necesidad real puede ampliar la matriz con un nuevo gate.

## Evidencia y limitaciones

- Tauri usa WebView2 en Windows, WKWebView en macOS y WebKitGTK en Linux.
- WebView2 es Evergreen; WKWebView depende de actualizaciones de macOS y
  WebKitGTK del gestor de paquetes de la distribución.
- Playwright WebKit no sustituye WKWebView ni WebKitGTK para gráficos, controles
  nativos o accesibilidad.
- El gate de release exige fixture CSS y binario Tauri real en las tres filas.
- Linux debe registrar distribución, arquitectura, WebKitGTK y sesión
  Wayland/X11; resize debe probarse también en NVIDIA.

Fuentes:

- <https://v2.tauri.app/reference/webview-versions/>
- <https://v2.tauri.app/start/prerequisites/>
- <https://tailwindcss.com/docs/compatibility>

## Build

Vite es el único build frontend. Desarrollo usa `http://localhost:1420` con
puerto estricto; producción genera assets estáticos locales en `dist` sin source
maps. TypeScript estricto se comprueba sin emitir una segunda copia de módulos.

## Frontera de datos React

TanStack Query posee las lecturas idempotentes, cacheables y compartidas del
frontend. `WorkspaceDataProvider` es el owner de objeto seleccionado, contexto
relacionado, cronologia e historial. Sus claves incluyen mundo, variante
observada, revision observada o `head` y, cuando corresponde, URI. Explorador,
palette, pickers y nombres de referencias usan la misma raiz de scope; una
variante de escritura nunca sustituye por accidente a la variante observada.

`selectedUri` es solo intencion local de navegacion. `selectUri` la cambia
despues del dirty guard y no llama al backend. `StructuredEditor` hidrata el
formulario solo cuando objeto y contexto pertenecen a la clave vigente; una
respuesta tardia de otra URI, mundo, variante o revision no puede reemplazar el
editor actual. Cambiar scope cancela y retira las consultas del scope anterior,
y cambiar mundo limpia tambien seleccion y formulario antes de mostrar el nuevo
owner.

Estado local conserva formularios, dirty, progreso, overlays, filtros y
workflows. Las mutaciones invalidan consultas despues del exito y nunca escriben
canon de forma optimista. Operaciones de una sola ejecucion como comparar
scopes, preparar merge, derivar narrativa, ejecutar simulacion o previsualizar
un draft permanecen como `invoke` imperativo: su resultado pertenece al
workflow que las inicia, no es una lectura compartida que justifique cache.
