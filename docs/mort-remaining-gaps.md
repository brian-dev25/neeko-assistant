# Faltantes de Screen Translate frente a MORT

Revisión: 11 de septiembre de 2026. Se contrastó el código actual del addon, MORT tag `1.291` (`3ffcb5f`) y MORT_CORE `86ac3b0` (última revisión anterior al lanzamiento, sin verificar correspondencia binaria con su DLL). Esta revisión documenta diferencias; no las implementa ni certifica precisión en Umamusume.

## Prioridad para Umamusume

| Prioridad | Diferencia comprobada | Efecto posible | Trabajo pendiente |
|---|---|---|---|
| Alta | MORT ofrece máscaras RGB/HSV y erosión. Neeko tiene brillo, contraste, gris, inversión y umbral, pero no esas operaciones. | Iconos, bordes y fondos pueden entrar al reconocimiento como letras. | Selección de rangos de color y erosión opcionales; comparar imagen original/procesada. No activarlos universalmente: también pueden borrar letras. |
| Alta | No se igualaron los archivos de idioma distribuidos por MORT. Neeko solo descarga `tessdata_fast`. MORT selecciona `eng` o `eng_fast` según configuración. | Reconocimiento potencialmente distinto con la misma imagen. | Identificar versiones y hashes de los modelos del release y comparar resultados. El nombre `eng_fast` no demuestra equivalencia con `tessdata_fast`. |
| Alta | `merge_lines` de Neeko solo compara con el bloque anterior, aplica umbrales propios, une como máximo unas pocas líneas y no considera el punto final en su lista de terminadores. MORT conserva palabras, posiciones y orientación, y tiene su propia opción de fusión. | Nombres/títulos pueden juntarse con descripciones o una habilidad puede fragmentarse. | Agrupación configurable que conserve estructura y distinga párrafos, títulos y columnas. No copiar ciegamente una única regla de distancia. |
| Alta | En modo espacial Neeko consulta secuencialmente cada bloque. `TransManager` de MORT puede reunir bloques pendientes con separadores y repartir los resultados. | Menos contexto por consulta, más llamadas y más espera. | Traducción por lotes con comprobación de correspondencia, caché por bloque y alternativa cuando el proveedor altere separadores. |
| Alta | Neeko conserva saltos OCR dentro del texto enviado; MORT tiene `AdjustText` y opciones para reemplazar/eliminar saltos/espacios. | Cortes visuales de una misma oración pueden afectar la traducción. | Separar ajuste del texto para traducir de la geometría para dibujarlo; conservar separaciones reales entre habilidades. |
| Media | MORT tiene diccionario de corrección, opciones de coincidencia/reprocesamiento, DB y editor. Neeko aplica `String.replace` en orden y equivalencias de frases completas. | Nombres de habilidades y términos recurrentes pueden permanecer incorrectos o variar. | Coincidencia por palabra/frase, reglas revisables y formatos de importación explícitos. No inventar sustituciones a partir de OCR dudoso. |
| Media | MORT analiza colores y usa rectángulos de presentación además de los de OCR. Neeko dibuja dentro de las cajas originales con colores manuales. | Traducciones largas se cortan o terminan con letra diminuta; no hay colocación equivalente. | Límites mínimo/máximo de fuente, redistribución y análisis de colores opcional. |
| Media | La caché de Neeko se crea dentro de cada ejecución y desaparece al detener. MORT puede cargar/guardar resultados anteriores. | Volver a iniciar repite solicitudes para habilidades ya vistas. | Caché persistente opcional por idioma/proveedor/reglas, con borrado explícito. Esto no exige recuperar el historial que el usuario eliminó. |

No está demostrado que ninguno de estos faltantes, individualmente, explique la captura enviada. El texto original del juego todavía no se proporcionó; solo se vio el resultado del OCR. No se realizó una comparación A/B con MORT ejecutándose.

## Errores y controles incompletos del addon

- **«Región sigue al mouse» duplicado:** `regionMouseFollow` aparece en la UI, pero `readSettings()` no lo recoge y el auxiliar no tiene ese campo. El control funcional es `followMouse`.
- **«Texto vertical (japonés)»:** cambia el modo de escritura CSS del resultado, no el reconocimiento. Tesseract activa reconocimiento vertical cuando el código de datos termina en `_vert`. La etiqueta actual no explica esta diferencia.
- **«Mostrar texto original»:** funciona en panel/flotante, pero las cajas de `over` solo dibujan la traducción; el original queda como título del elemento y no como texto visible.
- **Tamaño automático:** busca tamaños desde 1 px. No ofrece el mínimo/máximo configurables de MORT. Desactivarlo permite que el texto se recorte dentro de la caja.
- **DeepL heredado:** todavía existe una ruta Rust que llama a la API sin clave, aunque ya no aparece en el selector. No cuenta como proveedor funcional.
- **Posición del panel flotante:** `place()` lo ubica debajo de la región sin ajustarlo al área útil del monitor. Una región que llega al borde inferior puede dejar el panel fuera de pantalla. Es un defecto propio; esta revisión no certificó cómo maneja MORT cada combinación de monitores.

## Otras funciones sin equivalencia completa

- NHocr, EasyOCR y Google Cloud Vision OCR de MORT 1.291. OneOCR es una opción adicional de Neeko respecto de ese tag.
- DeepL web/API, Naver, Google Sheets, ezTrans y API personalizada. Gemini también figura en MORT, pero incorporarlo cambia el requisito previo de evitar modelos generativos en este flujo.
- Archivos DB, diccionarios, perfiles y configuración con formatos originales de MORT.
- Atajos configurables; Neeko usa combinaciones fijas.
- Salida OCR/traducción al portapapeles y guardado opcional de resultados. Neeko tiene entrada desde el portapapeles.
- Memoria visual con cantidad/vencimiento, distinta de caché y de historial de sesión. El usuario no pidió recuperar el historial.
- Opciones avanzadas de TTS y espera de fin de lectura; Neeko usa la voz del WebView con selección por idioma y velocidad.
- Manejo de errores y reinicio de sesión equivalente por proveedor. Neeko detiene la sesión al propagarse un error de OCR/red.

## Orden recomendado

1. Obtener el recorte original de Umamusume y comparar motores/configuraciones con esa misma imagen. Añadir una vista opcional de imagen procesada sería útil para diagnosticar, pero no se afirma aquí que sea una ventana idéntica a una función de MORT.
2. Corregir controles engañosos y posición fuera de pantalla.
3. Añadir filtros de color/erosión y comparar datos Tesseract con evidencia visual.
4. Adaptar agrupación, normalización y traducción por lotes.
5. Mejorar layout, diccionarios y caché; después incorporar otros proveedores según necesidad.

## Evidencia de código

Neeko: `native/screen-translate/Capture.cs`, `OcrModels.cs`, `Program.cs`; `src-tauri/src/screen_translate/pipeline.rs`; `addons/screen-translate/main.js`; `src/screen-translate-overlay.js` y `.css`.

MORT: [aplicación de opciones OCR](https://github.com/killkimno/MORT/blob/1.291/MORT/FormOption.cs), [procesamiento nativo](https://github.com/killkimno/MORT_CORE/blob/86ac3b0b5e52f6f2927643a003efba95e936dbb7/MORT_CORE/MainCore.cpp), [agrupación OCR](https://github.com/killkimno/MORT/blob/1.291/MORT/Manager/OCRDataManager.cs), [ajuste de texto y ciclo](https://github.com/killkimno/MORT/blob/1.291/MORT/Service/ProcessTranslateService/ProcessTranslateService.cs), [traducción/caché](https://github.com/killkimno/MORT/blob/1.291/MORT/Manager/TransManager.cs), [overlay](https://github.com/killkimno/MORT/blob/1.291/MORT/TransFormOver.cs), [opciones avanzadas](https://github.com/killkimno/MORT/blob/1.291/MORT/AdvencedOptionManager.cs).
