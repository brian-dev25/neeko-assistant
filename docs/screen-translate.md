# Screen Translate: implementación y validación

## Revisión MORT 1.291 (2026-09-10)

La segunda revisión ajusta los valores nuevos a `SetDefault()` de MORT: Google Web, Tesseract inglés, texto flotante, 1000 ms, escala 2, fuente Malgun Gothic de 20 px y opacidad 67%. Conserva español como destino localizado. La configuración existente se puede actualizar con «Aplicar valores de MORT 1.291». Papago utiliza el protocolo web actual, con normalización/validación de idiomas y una opción de origen explícito; ver la auditoría para diferencias frente al protocolo PPG antiguo.

Ver [hallazgos y diferencias vigentes](mort-1.291-review.md). Se eliminó el historial, se acotó la espera opcional (0 por defecto), se corrigió el estado de texto repetido y se actualizan las cajas aunque el texto no cambie. El intervalo ahora incluye el tiempo del ciclo y el ajuste automático de fuente respeta su checkbox. Las notas siguientes describen entregas anteriores cuando contradigan estos cambios.

## Correccion de superposicion (2026-09-09)

El modo `over` dibuja cada traduccion en el rectangulo OCR original; es el valor por defecto para configuraciones nuevas. Las configuraciones existentes mantienen su eleccion. En esta instalacion se cambio explicitamente de `layer` a `over`, con copia de respaldo del JSON anterior. El lienzo es transparente fuera de los bloques y el fondo del texto se configuro opaco para cubrir el original.

La posicion usa coordenadas fisicas de captura y las convierte a coordenadas CSS segun el tamano efectivo del overlay, incluidos monitores con origen negativo. El texto se ajusta siempre al bloque en modo `over`, incluso si la opcion general de fuente automatica estaba desactivada. Una nueva sesion invalida la geometria anterior antes de colocar la ventana.

Se corrigio el mensaje inicial que permanecia visible cuando el motor devolvia una captura sin texto. Ahora se distingue entre ausencia de texto y espera de estabilizacion; una lectura puntual conserva su resultado informativo. El timeout del auxiliar sigue limitado a 30 segundos y los errores detienen la sesion.

Referencia: `MORT/TransFormOver.cs` (SourceRect/ViewRect y origen de captura), revision `ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a`; repositorio https://github.com/killkimno/MORT (redirige a kmonkeyhead/MORT).

Validacion reproducible: `neeko-screen-ocr.exe --self-test-spatial` crea una ventana propia, captura su area cliente por WGC y comprueba texto espanol y coordenadas. `tests/screen-translate-overlay.html` verifica escalado, origen negativo, ajuste de fuente, redimensionado y borrado de bloques. `cargo test --lib screen_translate:: -- --include-ignored` incluye el protocolo del auxiliar y Google Web con frases sinteticas, nunca imagenes del escritorio del usuario.


El [prompt de implementación](screen-translate-implementation-prompt.md) se redactó antes de implementar. El resultado es un addon JS/CSS con backend Rust y un auxiliar C# autocontenido que no muestra consolas ni interfaz de MORT.

## Arquitectura

```text
main / settings → screen_translate (IPC, acciones tipadas)
                       ↓ runtime único + generación de sesión
                auxiliar C# (stdin/stdout JSON)
                 ├─ --languages: Windows OCR disponible
                 ├─ --select: selector visual temporal
                 └─ --serve: captura de región → Windows OCR
                       ↓ solo texto
               Google Web HTTPS (batch → GTX → diccionario)
                       ↓ eventos limitados a main/settings/overlay
               overlay Tauri + traducción actual
```

El auxiliar mantiene un motor OCR por idioma durante una sesión continua. La selección utiliza coordenadas físicas del escritorio virtual y conciencia DPI PerMonitorV2. Valida dimensiones y pertenencia al escritorio; ventanas protegidas o juegos exclusivos pueden no capturarse mediante GDI.

Windows OCR es una de las implementaciones que usa MORT. Se adaptaron la enumeración/selección del idioma y reconocimiento por líneas de `WindowOcr.cs`, quitando UI, TTS, estado global y fallback silencioso de idioma. La conversión de imagen utiliza un bitmap acotado en memoria y liberación determinista. Se conserva MIT y la revisión de procedencia. No se afirma que MORT tenga una API headless completa.

## Estado y ciclo de vida

- `CONTROL` serializa inicios/paradas/configuración y administra una única tarea para todas las ventanas.
- Preparar y seleccionar son tareas cancelables. Cambiar settings requiere detener la captura primero.
- Cada operación nueva incrementa la generación; los resultados viejos no pueden publicarse después de stop/restart.
- `watch` cancela OCR/traducción/espera; el supervisor mata y espera el auxiliar. Hay un timeout de cierre y aborto de respaldo con `kill_on_drop`.
- Cancelar corta la espera HTTP. El addon no utiliza el servidor de IA de Neeko, ni como respaldo. Solo el texto reconocido se envía a Google; nunca imágenes. El panel lo informa antes de iniciar.
- El proceso `--serve` termina al cerrar stdin. El selector también vigila la desconexión del padre.
- El panel solo elimina sus listeners en unload; no controla la vida del proceso. El comando nativo `addon_disable` detiene el addon antes de devolver, y `RunEvent::Exit` cierra su runtime.
- Cerrar el overlay detiene la sesión; finalizar una toma puntual deja el resultado visible sin capturar.
- `revision` permite a ambos paneles y al overlay descartar estados antiguos.
- Error de OCR/traducción detiene el trabajo con un mensaje recuperable. No hay reintentos automáticos ilimitados ni peticiones simultáneas por frame. HTTP 429 detiene sin continuar a otro endpoint; cada petición tiene cinco segundos de límite y respuesta acotada a 256 KiB.

## Comandos y datos

`screen_translate(action, settings?, profiles?)` admite `status`, `prepare`, `language-settings`, `settings`, `select`, `start`, `once`, `stop`, `overlay` y `clear`.

Solo `main` y `settings` administran el addon; el overlay únicamente consulta y detiene. Las operaciones activas requieren el addon habilitado. No se añaden endpoints HTTP ni shell genérico. La identidad por ventana no aísla otros addons que compartan main: conserva esa limitación existente del host.

`screen-translate.json` en la configuración de Neeko guarda settings y hasta 20 perfiles, mediante temporal/rename bajo serialización. Las capturas y el historial OCR no se escriben a disco. Evento: `screen-translate:status`, enviado exclusivamente a las tres ventanas participantes.

El overlay es una ventana Tauri dedicada, con permiso de escuchar eventos y arrastrar su propia ventana. Se activa `content_protected` para excluirlo de captura. No tiene permisos de shell, dispositivos o administración de otros addons.

## Compilación y distribución

Se necesita .NET SDK 9 y Rust en la máquina de desarrollo. El usuario final no necesita el SDK ni un runtime .NET instalado.

```powershell
node scripts/build-screen-ocr.mjs
npm run dev
# o
npm run build
```

Los hooks `beforeDevCommand` y `beforeBuildCommand` publican el auxiliar en `src-tauri/binaries/screen-translate/`. Ese directorio ya queda incluido en los recursos `binaries/` de Tauri; el addon se agrega explícitamente a los recursos. CI instala .NET SDK 9. Los binarios y directorios bin/obj generados se ignoran en Git.

El script copia también los avisos de licencia de los paquetes .NET/Windows restaurados, usando sus versiones resueltas, junto al aviso MIT de MORT.

El worker autocontenido ocupa aproximadamente 140 MB en esta compilación; su tamaño evita una instalación de runtime aparte. El botón de preparación verifica el motor incluido y enumera los idiomas. No descarga ejecutables de fuentes externas ni instala paquetes de idioma con elevación automática. Si el motor falta, informa cómo reparar la distribución.

## Pruebas

```powershell
node scripts/build-screen-ocr.mjs
& src-tauri/binaries/screen-translate/neeko-screen-ocr.exe --self-test | Out-String
& src-tauri/binaries/screen-translate/neeko-screen-ocr.exe --languages | Out-String
cargo test --manifest-path src-tauri/Cargo.toml --lib screen_translate::tests
cargo test --manifest-path src-tauri/Cargo.toml --lib screen_translate::tests::real_worker_protocol_and_shutdown -- --ignored
npm test
```

`--self-test` reconoce una imagen sintética y rechaza geometría inválida, sin capturar el escritorio. La prueba explícita del auxiliar comprueba JSON por pipes, errores antes de captura y salida/recolección del proceso real. Las pruebas Rust cubren límites, perfiles, deduplicación, generaciones, cancelación, parseo Google, idiomas OCR, HTTP 429 y respuestas demasiado grandes mediante un servidor HTTP local de prueba. `screen_translate::google::tests::live_google_translation -- --ignored` comprueba Google con la frase sintética «Hello world».

`tests/screen-translate-ui.html` carga el JS real con IPC simulado. Verifica que los eventos no borren borradores, el orden guardar/iniciar, controles durante captura, perfiles, rechazo de eventos viejos, texto OCR como texto plano y unload.

Verificación manual pendiente en juegos: selector sobre monitores con distintas escalas, latencia bajo carga, ausencia de recaptura del overlay, cierres/reinicio durante traducción y restauración de perfiles tras cambiar resolución. Las pruebas automáticas no sustituyen estas comprobaciones.

El OCR instalado reportó es-ES y es-MX; la prueba sintética reconoció `HELLO WORLD 12345`. No se utilizó un juego ni se midió la latencia durante una partida.

Validación de esta sesión: 33 pruebas Rust ordinarias y 14 JavaScript correctas; además, traducción Google real con texto sintético, protocolo/cierre del auxiliar real, OCR sintético y prueba de la UI real en Edge headless con IPC simulado. No se inició ningún modelo para estas comprobaciones.

El protocolo Google Web se adaptó de `GoogleBasicTranslateAPI.cs` de MORT en la misma revisión MIT que el OCR. No se usa WebView ni un navegador auxiliar. Es una integración web no oficial, dependiente de la disponibilidad y formato de Google; no garantiza servicio ilimitado. Los códigos regionales de Windows OCR se convierten a idiomas de traducción, conservando chino simplificado/tradicional.
