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
