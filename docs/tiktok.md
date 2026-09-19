# Preparar para TikTok

Escribir `tiktok`, `tik tok` o `preparar para tiktok` abre la ventana local sin IA. Elegir el video, el motor y sus opciones, y pulsar **Preparar para TikTok**. El resultado se guarda junto al original con el motor en el nombre; si existe, se agrega un número. Bastien conserva el nombre `nombre_tiktok.mp4`. Upload120 también incluye el modo. **Abrir carpeta** muestra la ubicación del último resultado.

| Motor | Implementación local | Opciones |
| --- | --- | --- |
| Bastien | `transform` original sin cambios | ×10 y comentario original |
| Upload120 | `patcher.browser.js` original, ejecutado por Node.js sin servidor | Web Signal, Balanced Sync, Classic Force; multiplicador automático o 2–16 |
| ut0ku | Adaptación Python de `patch_mvhd` y `patch_mdhd` C++, incluidos sus offsets y división de duración | 60/120 FPS, divisor 2/4 |
| LuisAlves | `patcher.py` original, pasando factor 30/FPS explícito | FPS automáticos o 60/120/240 |
| Pascha v2 | Misma invocación FFmpeg del `.bat`, sin ejecutar su interfaz ni sus borrados | `itsscale`: 60→2, 120→6, 240→12 |
| MisticGG | Adaptación del encode FFmpeg y parche `elst+8 = 0x10000001` | 1080p/20 Mbps, 720p/12 Mbps, 540p/7 Mbps; Auto/NVENC/AMF/QSV/CPU |

Mistic recodifica a HEVC a 60 FPS usando el cuadrado máximo 1920/1280/960 del código original. Se conservan sus parámetros de encoder; el preset personalizado de su GUI no está expuesto. Los valores 12/7 Mbps provienen del código fijado, aunque el README menciona 10/6. Los demás motores no recodifican los cuadros reales. Ningún modo garantiza un resultado específico en TikTok.

Los archivos de referencia están fijados por commit en `src-tauri/vendor/tiktok-engines/*/UPSTREAM.txt`. Upload120 y LuisAlves incluyen MIT; ut0ku incluye GPLv3 y la adaptación Python conserva esa licencia. Mistic y Pascha no traen un archivo LICENSE en las revisiones consultadas; no se les atribuye MIT. El `.bat` de Pascha proviene del release v2.0, ZIP SHA-256 `1b841af05812fb28814db644c6b9b0a7962325e7b143b4f8abbfc2203d583e4a`.

La interfaz se comprueba con `node --test scripts/tiktok-ui.test.mjs`: carga seis motores, envía el ID y opciones seleccionados, conserva el archivo de entrada y restaura los controles después de un error. El catálogo compartido `src-tauri/scripts/tiktok-engines.json` define las opciones válidas para la interfaz y el procesador.

**Ver original** abre el video de entrada para revisarlo. Después del procesamiento, en Windows se consulta Media Foundation: si no reconoce una pista de video, se muestra una advertencia en lugar de un éxito sin matices. Esta consulta no modifica la salida y tampoco certifica la reproducción completa ni la aceptación en TikTok. Si la consulta no está disponible, no se afirma compatibilidad.

Se distribuye el código original de BastienGimbert/tiktok-quality, commit `5fc3907734a464c9c064233bfc3d341d8f4dbfdb`, con su licencia MIT en `src-tauri/vendor/tiktok-quality`. Los archivos originales no están modificados. El wrapper llama a `transform` con el multiplicador ×10 y comentario predeterminados, sin recodificar ni sustituir el algoritmo. Conserva también los requisitos y limitaciones del motor original.

Todos requieren Python 3.10+ instalado. Se reutiliza el descubrimiento de Python de Neeko, incluyendo `NEEKO_SYSTEM_PYTHON`. Upload120 requiere Node.js; Pascha y Mistic requieren FFmpeg; la detección automática de FPS de ut0ku y LuisAlves usa FFprobe. Se respetan las rutas de FFmpeg/FFprobe configuradas en Neeko. No necesita instalar paquetes con pip. Los subprocesos se ejecutan sin shell ni ventanas de consola. Solo se publica la copia cuando termina, sin reemplazar archivos existentes. La carpeta de destino debe permitir enlaces duros (por ejemplo NTFS); si no los admite, se informa el error y se conserva el original.

Verificación: `python -B scripts/tiktok.test.py` genera videos reales y prueba los seis motores, los tres métodos Upload120, los factores de FPS, conservación de `mdat` en parches sin remux, comparación binaria con Bastien/LuisAlves/Pascha, encode HEVC y parche Mistic, colisiones y errores. `node --test src-tauri/vendor/tiktok-engines/upload120/test/patcher.test.js` ejecuta las pruebas upstream de Upload120. Las pruebas locales de Mistic usan CPU; las tres rutas GPU conservan los argumentos del proyecto, pero dependen del hardware disponible. La calidad obtenida después de subir a TikTok requiere una comprobación en la plataforma.

## Diagnóstico de reproducción (2026-09-13)

Se comprobó el caso reportado `ACEEEE.mp4`, exportado por DaVinci Resolve en H.264 Main, 1920×1080, 60 FPS y 2164 cuadros reales. La salida existente coincide byte por byte con el motor original. FFmpeg decodifica los 2164 cuadros con los mismos hashes y tiempos que el original, pero reporta errores al encontrar las muestras ficticias de ocho bytes. Media Foundation reconoce video en el original y solo audio en la salida ×10.

Para aislar la causa se generaron variantes temporales, sin editar los archivos del motor distribuido: quitar la extensión avcC agregada no restableció el reconocimiento; usar multiplicador 1 sí lo restableció, con y sin esa extensión. Esto identifica la inflación de cuadros como desencadenante en este caso. El multiplicador distribuido sigue siendo ×10, respetando el sistema original solicitado. No se modificaron los videos del usuario.

Las pruebas de integración ahora también comparan cuadros decodificados y verifican que la incompatibilidad de las muestras generadas se informe en Windows. La coincidencia binaria con upstream no se considera prueba de reproducibilidad en un reproductor.
