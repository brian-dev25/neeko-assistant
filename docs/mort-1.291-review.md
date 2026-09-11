# Screen Translate frente a MORT 1.291

Revisión posterior: [faltantes priorizados y controles incompletos](mort-remaining-gaps.md), 11 de septiembre de 2026.

## Revisión de OCR: Umamusume (2026-09-11)

El usuario confirmó Tesseract en Umamusume. La configuración guardada inspeccionada tenía `eng`, Google, escala 1, región de escritorio 807 × 1079 y filtros desactivados. Los símbolos ya estaban en el original OCR mostrado: no eran evidencia de un fallo de codificación ni de Papago. La captura proporcionada era del panel de resultados, no del texto fuente del juego, por lo que no permite medir precisión OCR ni identificar exactamente qué elementos visuales se confundieron.

Se revisó además MORT_CORE, en `86ac3b0b5e52f6f2927643a003efba95e936dbb7` (2025-05-18, último commit anterior al release de MORT 1.291). No hay una vinculación verificada entre ese commit y la DLL distribuida en el release. Su [`setTessdata`](https://github.com/killkimno/MORT_CORE/blob/86ac3b0b5e52f6f2927643a003efba95e936dbb7/MORT_CORE/main.cpp) usa `OEM_LSTM_ONLY`, `PSM_AUTO` y una whitelist para inglés. Neeko ya usaba esos modos, pero no limitaba caracteres. La opción rápida de MORT selecciona `eng_fast`/`jpn_fast`; no se verificó que esos archivos sean idénticos a `tessdata_fast` de Neeko.

Se incorporó **Limitar caracteres de Tesseract inglés**, activado por defecto, aplicado en el motor antes de reconocer y desactivable. Usa letras, números y puntuación básica; conserva adicionalmente `%`, `+`, `:` y `/`, necesarios para estadísticas. No se aplica a otros idiomas. No es una eliminación posterior de texto ni un filtro de confianza: aún puede confundir letras/números o interpretar dibujos como palabras.

Validación: compilación Rust correcta, auxiliar publicado en `src-tauri/binaries/screen-translate`, reconocimiento sintético de `HELLO WORLD 12345` y de `Hint Lvl 2 - 20% OFF! +90 (6-8)` correctos. La segunda comprobación se realizó fuera del repositorio. No se recreó `tests/`. La precisión sobre la escena del usuario queda pendiente de disponer del recorte original del juego; seleccionar solo el cuadro de habilidades y probar escala 2 son ajustes a evaluar sobre esa imagen, no resultados ya comprobados.

Revisión del 10 de septiembre de 2026. Referencia exacta: tag `1.291`, commit `3ffcb5f43faab5eb74764570545c850802857cb0` de [MORT](https://github.com/killkimno/MORT/tree/1.291). Se leyó el código de ese tag; no se ejecutó su aplicación. Las auditorías anteriores usaban otra revisión y no describen esta versión.

## Por qué quedaba esperando

Neeko reiniciaba `stable_since` cada vez que cambiaba el texto OCR o la posición de la región. No había vencimiento total: ruido del OCR, animaciones o seguimiento del mouse podían impedir llegar a traducir. Además, en la secuencia A traducido → B pendiente → A, el mensaje de espera permanecía aunque A ya estaba traducido. Estos son defectos comprobados del flujo; sin una captura del caso real no se puede afirmar cuál de ellos ocurrió en el juego del usuario.

El [ciclo de MORT 1.291](https://github.com/killkimno/MORT/blob/1.291/MORT/Service/ProcessTranslateService/ProcessTranslateService.cs) no tiene ese filtro de estabilidad de Neeko. `MakeFinalOcrAndTrans` envía los resultados a `StartTrans`; el ciclo compara `formerOcrString` y `NowOcrString` para actualizar o repintar. Programa lecturas desde el inicio del ciclo mediante `OcrProcessSpeed`.

## Cambios aplicados

- Nuevas configuraciones: espera de estabilización de 0 ms, para procesar la primera lectura. Las configuraciones y perfiles guardados conservan su valor; para lectura inmediata se elige 0 en «Espera opcional de texto».
- La espera opcional se limita a `max(1000, 2 × stabilizeMs)` ms desde la primera lectura pendiente, evaluada al terminar cada captura. El tiempo de OCR y red se suma al tiempo real hasta ver el resultado. Un OCR que cambia continuamente ya no reinicia el límite total.
- La estabilidad considera el texto; el resultado espacial considera también cajas y posición. Mover un texto actualiza su ubicación y aprovecha la caché de traducciones.
- Volver al resultado ya mostrado restaura «Traducción lista», en lugar de dejar el mensaje de espera.
- El intervalo se cuenta desde el inicio de la lectura; ya no se agrega íntegro después de OCR y traducción.
- Se eliminó el historial de 50 entradas del panel y del estado nativo. Se muestra la traducción actual. La caché de traducciones sigue funcionando de manera independiente.
- El modo espacial respeta desactivar el ajuste automático de fuente. La fuente manual puede desbordar una caja pequeña; en la revisión de valores predeterminados se desactivó el ajuste automático, como en MORT.

## Diferencias que siguen existiendo

| Área | MORT 1.291 | Addon actual |
|---|---|---|
| OCR | Tesseract, Windows OCR, NHocr, Google Vision y EasyOCR en `SettingManager.OcrType` | Windows, Tesseract y OneOCR; NHocr, Vision y EasyOCR no están adaptados. **OneOCR no figura en el tag 1.291**, aunque sí estaba en la revisión posterior auditada antes. |
| Preparación de texto | Agrupación de líneas/bloques, opciones de espacios y correcciones | Agrupación geométrica simple, reglas de reemplazo; resultados y contexto pueden diferir. |
| Traducción espacial | `TransManager` combina bloques pendientes con separadores y reparte el resultado | Consulta secuencial por bloque no almacenado. Más bloques pueden implicar más solicitudes y más espera, además de menos contexto compartido. |
| Google Web | Adaptador web con gestión propia | Adaptador de Neeko con varios endpoints de respaldo. El servicio, motor OCR y segmentación deben coincidir para comparar calidad. |
| Papago | Protocolo antiguo `/apis/n2mt/translate`, autenticación PPG, timestamp y deviceId | Protocolo web actual `/api/text/translation`, comprobado contra el JavaScript público y con frases sintéticas. La web actual no envía PPG. Se corrigieron normalización de idiomas, validación de respuesta y pausa entre consultas. La conclusión anterior de que faltaba obligatoriamente esa firma era incorrecta para el endpoint moderno. |
| DeepL | Opciones web/API y configuración propia | No disponible en el selector; queda una ruta heredada en Rust que no envía API key y no constituye soporte funcional. |
| Otros traductores | Naver, Google Sheets, Gemini, ezTrans y API personalizada | No implementados. No se incorporó un modelo de IA al flujo existente. |
| Diccionario/DB | DB propia y editor; carga y guardado de traducciones | Reglas y frases exactas en JSON; no importa los formatos ni reproduce el editor. En modo DB las frases desconocidas permanecen en original. |
| Caché | Resultados anteriores y guardado de archivos | LRU acotada a 256 entradas / 2 MB durante cada ejecución; se reinicia al detener y volver a iniciar. |
| Memoria visual | Lista opcional con cantidad y vencimiento; guardado OCR opcional | Conservación de un resultado durante ausencia de texto; sin lista de memoria visual equivalente. El historial eliminado no cumplía esa función. |
| Overlay | Layout avanzado, detección de colores, dibujo y opciones originales | Cajas, colores manuales y ajuste de fuente propio. `showOriginal` no dibuja ambos textos dentro de las cajas espaciales; sí controla el panel/flotante. |
| Captura y controles | Opciones, atajos y ventanas propias | Varias regiones, exclusiones, WGC, seguimiento, portapapeles y atajos fijos; no paridad de todas las opciones. El segundo checkbox `regionMouseFollow` es redundante y no llega al worker; el funcional es `followMouse`. |
| Errores | Manejo específico por adaptador | Un error OCR/proveedor detiene la sesión. Un timeout HTTP no garantiza igual recuperación que MORT. |

Fuentes primarias: [tipos disponibles](https://github.com/killkimno/MORT/blob/1.291/MORT/SettingManager.cs), [traducción y caché](https://github.com/killkimno/MORT/blob/1.291/MORT/Manager/TransManager.cs), [Papago](https://github.com/killkimno/MORT/blob/1.291/MORT/TransAPI/PapagoWebTranslateAPI.cs), [memoria visual](https://github.com/killkimno/MORT/blob/1.291/MORT/Service/ProcessTranslateService/TranslateResultMemoryService.cs), [overlay](https://github.com/killkimno/MORT/blob/1.291/MORT/TransFormOver.cs).

## Validación y alcance

Validación realizada: `cargo test --manifest-path src-tauri/Cargo.toml --lib screen_translate -- --include-ignored`: **14 pruebas aprobadas**, incluidas Google Web con frases sintéticas y el protocolo/cierre del auxiliar real. Las dos regresiones HTML pasaron en Edge sin interfaz: panel sin historial, escape de OCR, coordenadas negativas, escalado, ajuste automático/manual y borrado de bloques. También pasó la comprobación de sintaxis de ambos archivos JavaScript. No se validó visualmente contra un juego ni se midió equivalencia de calidad entre ambos programas. Se compiló el módulo para pruebas; no se generó un instalador ni se reemplazó la aplicación instalada.

Esto corrige el bloqueo y acerca el ciclo principal a MORT; **no convierte el addon en una réplica completa de MORT 1.291**. Para esa equivalencia faltan principalmente agrupación/traducción por lotes, renderizador, adaptadores y formatos DB/configuración.

## Papago y valores predeterminados (segunda revisión)

Se verificó `SettingManager.SetDefault()`, no solamente los inicializadores de campos: el valor de velocidad inicial del campo es 3 pero `SetDefault()` lo cambia a 2. `FormOption` convierte 2 en **1000 ms**. Fuentes: [SetDefault y GetDefaultResultCode](https://github.com/killkimno/MORT/blob/1.291/MORT/SettingManager.cs), [aplicación de opciones](https://github.com/killkimno/MORT/blob/1.291/MORT/FormOption.cs), [opciones avanzadas](https://github.com/killkimno/MORT/blob/1.291/MORT/AdvencedOptionManager.cs).

| Opción | MORT 1.291 | Nuevo valor en Neeko |
|---|---|---|
| Traductor | Google Web (`google_url`) | Google Web (`google`) |
| OCR y origen | Tesseract, `eng` | Tesseract, `eng` |
| Destino | Según idioma de interfaz; coreano en Auto/Corea | Español, idioma de esta interfaz. No se fuerza coreano. |
| Presentación | `layer` | Texto flotante |
| Intervalo | 1000 ms | 1000 ms |
| Escala de imagen | 2 | 2 |
| Fuente | Malgun Gothic, 15 puntos | Malgun Gothic, 20 píxeles CSS (15 pt a 96 dpi) |
| Fondo | Negro, alfa 170/255 | Negro, opacidad 67% (redondeada) |
| Texto / contorno | Blanco; dos contornos, gris y negro | Blanco y contorno negro; Neeko solo tiene un contorno |
| Mostrar OCR original | Sí | Sí |
| Ajuste automático de fuente | No | No |
| Memoria de traducciones en pantalla | Desactivada | Retención al desaparecer: 0 ms |
| Estabilización adicional / TTS / filtros | Sin estabilización de Neeko; TTS y filtros desactivados | 0 ms; TTS y filtros desactivados |

No se igualaron los binarios/datos Tesseract: MORT deja `nowIsFastTess=false`; Neeko sigue usando el catálogo `tessdata_fast`. Tampoco existe equivalencia exacta entre los renderizadores. El cambio iguala las opciones disponibles con esas excepciones explícitas.

Los valores nuevos se aplican a configuraciones nuevas. El botón **Aplicar valores de MORT 1.291** permite aplicarlos a una configuración existente sin borrar perfiles, áreas, destino ni diccionario. No cambia una sesión activa.

Papago rechazó `en-US` con HTTP 400 y código 60102 durante la comprobación, mientras aceptó `en`. El adaptador ahora normaliza los códigos regionales (incluidos los chinos) antes de enviar, limita la respuesta, comprueba que el origen/destino devueltos coincidan y no almacena respuestas inválidas en caché. Se envían `dict=false`, `useGlossary=false` y `honorific=false`, según el formulario público actual. Se separan las solicitudes 650 ms; MORT usaba una espera aleatoria de hasta 650 ms.

La opción **Idioma de origen al traducir** permite corregir detecciones ambiguas de OneOCR/portapapeles sin cambiar el idioma de reconocimiento. Vacía sigue el OCR; `auto` pide detección al proveedor. Esto no garantiza la calidad semántica de Papago ni repara texto mal reconocido: para diagnosticar una frase hay que contrastarla con el original OCR.

La web verificada es [Papago](https://papago.naver.com/): bundles `3yid7qex1bql-.js` (rutas) y `3bb5rlqacu6fp.js` (cliente y formulario), servidos bajo `/_next/static/chunks/` en esta fecha. Sus nombres pueden cambiar al publicar nuevas versiones.

Validación de esta segunda revisión: `cargo check --lib` correcto; comprobación temporal en Edge de valores iniciales, persistencia del origen explícito, acción de valores predeterminados y bloqueo durante captura. Un ejecutable temporal usando los módulos Rust reales tradujo correctamente «The door is locked. Find the key to open it.» desde `en-US` y desde `auto` hacia `es-AR`, y verificó idioma coincidente y rechazo de texto demasiado largo. Los archivos de comprobación quedaron en la carpeta temporal del sistema; no se recreó `tests/`. No se generó instalador.
