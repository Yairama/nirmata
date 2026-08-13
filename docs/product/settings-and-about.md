# Settings y About

**Estado:** especificacion aprobada.

## Settings

| Seccion | Contenido vigente permitido |
|---|---|
| General | Sin preferencias configurables por ahora. |
| Apariencia | Tema del sistema, claro, oscuro o alto contraste, persistido en el equipo. |
| IA | Microsoft Foundry, estado de credencial, persistencia, secure store, endpoint/modelo, probar conexion, reemplazar y borrar clave. |
| Proyecto | Ruta, nombre, variante activa, version observada y solo lectura si hay mundo. |
| Accesibilidad | Sin preferencias hasta implementar comportamiento real. |
| Avanzado | IDs, URI, revision, variante y detalles tecnicos copiables. |

No se inventan selector de proveedor, temperatura, prompts, telemetria,
preferencias de autosave ni configuracion directa de SQLite. `BASE_URL` y el
modelo pueden reportarse como configurados, faltantes o invalidos, pero no se
expone la clave ni se devuelve su valor a JavaScript.

## About

- Producto: Nirmata.
- Version: `0.1.0` desde configuracion Tauri.
- Identificador: `com.nirmata.desktop`.
- Licencia declarada: MIT.
- Arquitectura: escritorio local-first; canon en un `.nirmata` local.
- Privacidad: IA opcional, contexto seleccionado enviado al proveedor con
  `store: false`, credencial en secure store o fallback explicito de sesion.
- Documentacion: indice local en [`../README.md`](../README.md).

No se muestra un build ID, URL de documentacion empaquetada o texto de licencia
completo hasta que exista una fuente distribuible real.
