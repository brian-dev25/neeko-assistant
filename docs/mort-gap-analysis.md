# Auditoría de MORT frente a Screen Translate de Neeko

> Estado historico. Consultar [comparacion actual](mort-parity-current.md) para los cambios y el modo MORT original.

Fecha de revisión: 8 de septiembre de 2026. Repositorio: `kmonkeyhead/MORT`. Revisión examinada: `ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a`, commit del 25 de agosto de 2026 según su fecha de autoría/commit publicada por Git. Comparación con los archivos actuales de Neeko, incluidos los cambios locales todavía sin commit.

## Dictamen

Screen Translate es una integración funcional de captura de una región, Windows OCR, Google Web y texto flotante. Todavía no reproduce la amplitud de MORT. Las diferencias más importantes son los motores OCR alternativos, los datos de posición de los textos, la captura por ventana, el overlay espacial, la caché de traducciones y los controles utilizables sin abandonar el juego.

La solución al pedido de idiomas no consiste en llenar un desplegable: hace falta disponer de un motor capaz de reconocerlos y de sus datos. La solución al parecido visual tampoco termina en CSS: MORT tiene un modo que posiciona cada bloque traducido sobre su origen, y Neeko actualmente descarta esos datos.

La prioridad propuesta respeta el requisito del usuario: no utilizar el modelo de Neeko para traducir. Eso no implica consumo cero. Los motores OCR procesan imágenes localmente y algunos incorporan redes de reconocimiento propias; deben diferenciarse de un modelo generativo de chat.

## Alcance y método

Se descargó una copia aislada del repositorio, se fijó la revisión y se inventariaron sus 260 archivos versionados. Hay 145 archivos C# y 49.665 líneas C#, incluyendo archivos Designer; esta cifra no representa 49.665 líneas de lógica escrita manualmente. El [inventario adjunto](mort-repository-inventory.md) enlaza cada archivo a la revisión examinada.

La revisión funcional siguió los puntos de entrada, selectores de motores, servicios de captura/procesamiento, modelos OCR, ventanas de traducción, adaptadores, configuración, proyectos auxiliares y herramientas de mantenimiento. Se contrastó el código con la wiki de implementación incluida. La profundidad se concentró en los caminos que afectan a la integración; inventariar todos los recursos gráficos y diseñadores no equivale a probar cada control.

No se ejecutaron los binarios incluidos en MORT, no se compiló su distribución completa ni se probaron todos sus proveedores web. No se hizo un benchmark CPU/RAM/GPU en juegos. «Presente en MORT» significa localizado en esta revisión del código, no una garantía de que un servicio externo siga aceptando sus solicitudes. Las dependencias nativas de repositorios separados no están cubiertas por una auditoría completa de su implementación interna.

Esta entrega modifica documentación, no agrega motores ni funciones a la app.

## 1. Mapa del repositorio

| Parte | Función | Qué representa para Neeko |
|---|---|---|
| `MORT/Program.cs` | Inicio, ejecución única e inyección de dependencias | Neeko ya tiene su propio ciclo de aplicación; interesa separar servicios, no duplicar el arranque |
| `Form1`, `FormOption`, `Form1Button` | UI principal, aplicar configuración y acciones | Hay que trasladar capacidades al panel del addon |
| `Manager/FormManager` | Ventanas, áreas OCR y sus modos | Falta un administrador equivalente de varias regiones |
| `Service/ProcessTranslateService` | Coordinación de captura, OCR, traducción, cancelación y publicación | Equivale parcialmente al supervisor Rust actual |
| `Manager/OCRDataManager` | Normalización espacial de palabras, líneas y bloques | Es una de las piezas conceptuales más importantes que falta |
| `OcrApi`, `Manager/OcrManager` | Adaptadores y preparación de motores OCR | Actualmente solo se adaptó Windows OCR |
| `ScreenCapture` | Captura por Windows Graphics Capture y Direct3D | El auxiliar actual usa GDI sobre el escritorio |
| `TransAPI`, `Manager/TransManager` | Proveedores, caché, traducciones del usuario y coordinación | Actualmente solo Google Web, sin caché reutilizable |
| `TransForm`, `TransFormLayer`, `TransFormOver` | Tres modos de presentación | Neeko solo se aproxima a parte del modo flotante |
| `SettingManager`, `AdvencedOptionManager`, `SettingData` | Persistencia y opciones extensas | Los perfiles actuales guardan un subconjunto pequeño |
| `ClipboardAssist`, `KeyHook`, `CustomControl` | Portapapeles y atajos globales | Funciones ausentes en el addon |
| `DicEditor`, `SettingBrowser` | Correcciones y configuraciones/DB por juego | Ausentes |
| `CloudVision` | Cliente Google Cloud Vision OCR | Opcional; supone enviar imágenes fuera de la PC |
| `GSTrans` | Traducción mediante Google Sheets | Proveedor diferente de Google Web; no es necesario para el camino básico |
| `PipeClient`, `PipeServer` | Integración con ezTrans | IPC específico, no una API universal de MORT |
| `Updater`, `VersionCheck` | Actualización del producto MORT | Conviene mantener el actualizador de Neeko |
| `LocalizeManager`, recursos, formularios auxiliares | Idiomas de UI, ayuda, selección de colores, etc. | No aumentan por sí mismos los idiomas reconocidos por OCR |
| `docs/wiki`, `tools`, `.github` | Documentación, generación de wiki y mantenimiento | Útiles para entender intenciones y compatibilidad |

La solución principal combina WinForms y WPF sobre .NET 9, x64. Coexisten servicios con inyección de dependencias y managers globales; no es una biblioteca OCR autónoma que pueda importarse directamente desde JavaScript. Hay llamadas a `MORT_CORE.dll` y otras dependencias que no se implementan en este repositorio. [Proyecto](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/MORT.csproj), [entrada](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Program.cs).

## 2. Flujo completo de funcionamiento

MORT configura un origen de captura y una o varias áreas. Antes de capturar prepara la ventana de traducción; en el modo espacial limpia el dibujo anterior y sincroniza composición. Obtiene imágenes de las áreas, aplica las opciones de procesamiento, invoca el motor OCR y normaliza el resultado. Conserva el texto junto con coordenadas, orientación y agrupación. Aplica correcciones, consulta traducciones conocidas y llama al proveedor cuando hace falta. Finalmente presenta el resultado según el modo elegido y repite o termina si era una toma puntual.

La normalización no es un detalle decorativo: permite mantener separadas burbujas, títulos, nombres y columnas. La traducción puede alargarse respecto al original, por lo que el renderizador calcula el espacio disponible, ajusta fuente y evita invadir otros bloques.

Neeko tiene un recorrido más corto:

```text
Rectángulo fijo del escritorio
        ↓ GDI
Imagen en memoria
        ↓ Windows OCR
Cadena con líneas de texto, sin coordenadas
        ↓ comparación con la última cadena
Google Web
        ↓
Una ventana flotante + historial de 50 resultados
```

Fuentes: [coordinación](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/ProcessTranslateService/ProcessTranslateService.cs), [inicialización](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/ProcessTranslateService/TranslationProcessInitializationService.cs), [normalización OCR](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Manager/OCRDataManager.cs).

## 3. Motores OCR e idiomas

| Motor en MORT | Dependencia e idiomas | Estado en Neeko | Encaje con bajo consumo |
|---|---|---|---|
| Windows OCR | Reconocedores disponibles en Windows y selección por código de idioma | Implementado | Conservar como opción; medir con las escenas reales |
| Tesseract | Motor nativo y datos de reconocimiento independientes de los paquetes de Windows | Ausente | Candidato inicial con datos rápidos y solo idiomas elegidos |
| OneOCR / Snipping Tool | `oneocr.dll`, `oneocr.onemodel` y `onnxruntime.dll`; el adaptador busca componentes de aplicaciones Windows | Ausente | Candidato a evaluar; no es el mismo API que Windows OCR |
| EasyOCR | Python, biblioteca EasyOCR, modelos y dependencias; el adaptador solicita GPU | Ausente | Opcional, no recomendado como motor inicial sin medir |
| Google Cloud Vision | Credenciales y envío de imagen al servicio remoto | Ausente | Reduce reconocimiento local, pero cambia privacidad y consumo remoto |

**Windows OCR.** El código original consulta `OcrEngine.AvailableRecognizerLanguages`. No puede reconocer un idioma adicional solo porque la UI lo muestre. Neeko conserva este límite; además valida que el idioma exista y no cambia silenciosamente a otro. [Adaptador original](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/OcrApi/WindowOcr/WindowOcr.cs).

**Tesseract.** MORT expone selección de datos, opciones para inglés/japonés/otros y variante rápida; la configuración termina en llamadas nativas como `setTessdata`. La implementación interna se delega a MORT_CORE. Los paquetes de Tesseract son independientes de los idiomas Windows. Para Neeko hace falta integrar el motor, un catálogo real de datos compatibles, descarga/importación, comprobación de integridad, estado de instalación y mapeo de códigos hacia Google. Añadir el catálogo no significa cargar todos los modelos en RAM. La variante `tessdata_fast` está orientada a velocidad, con un compromiso de precisión que debe evaluarse en juegos. [Aplicación de opciones](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/FormOption.cs), [datos oficiales de Tesseract](https://tesseract-ocr.github.io/tessdoc/Data-Files.html), [datos rápidos](https://github.com/tesseract-ocr/tessdata_fast).

**OneOCR.** Mi explicación anterior fue incompleta: existe esta tercera vía local además de Windows OCR y Tesseract. MORT busca componentes en Snipping Tool y, en ciertas condiciones, Photos, y los copia a su carpeta DLL. El adaptador no usa el selector de paquetes de `Windows.Media.Ocr`. Sin embargo, el código inspeccionado no constituye una lista garantizada de «todos los idiomas»: hay que comprobar la versión de los componentes, su cobertura y la disponibilidad en el equipo. La licencia MIT de MORT no determina por sí sola las condiciones de redistribución de esos componentes. No corresponde prometer que se pueden incluir en Neeko sin revisar esa procedencia. [Adaptador OneOCR](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/OcrApi/OneOcr/OneOcr.cs).

**EasyOCR.** En esta revisión la lista del adaptador contiene seis códigos: inglés, japonés, coreano, chino simplificado/tradicional e indonesio. Eso no equivale al catálogo completo de la biblioteca. La creación del lector usa `gpu: true`; el comportamiento final depende también de PyTorch y del hardware. Python y los modelos agregan preparación, almacenamiento y mantenimiento. Hay además referencias de rutas a Python 3.9 en el servicio y un paquete Python.Included de otra versión en el proyecto; es un punto de compatibilidad que habría que validar, no una prueba de fallo. [Adaptador](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/OcrApi/EasyOcr/EasyOcr.cs), [preparación Python](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/PythonService/PythonModouleService.cs).

**Google Vision.** Es OCR remoto, distinto de Google Translate. El inicializador de MORT examinado rechaza Google OCR para traducción continua y permite su uso puntual, incluyendo una prioridad opcional para capturas únicas. No sería correcto describir los cinco motores como intercambiables en todos los modos. [Cliente](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/CloudVision/Api.cs), [restricción de modo](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/ProcessTranslateService/TranslationProcessInitializationService.cs).

Hay tres catálogos que deben separarse: idioma de la interfaz, idioma que el OCR puede leer e idioma al que el proveedor puede traducir. Neeko limita actualmente el destino a nueve códigos en Rust y nueve opciones en JS. Esa restricción fue una decisión de la integración, no de Windows. Ampliarla requiere una fuente común y códigos que el proveedor admita; no inventar opciones que después fallen.

## 4. Captura y regiones

MORT tiene varias regiones OCR, regiones de excepción, selección rápida, snapshot, seguimiento del mouse y captura vinculada a una ventana. El servicio de Windows Graphics Capture maneja frames Direct3D y usa límites DWM para alinear el origen físico de la ventana. Eso resuelve problemas que no puede resolver un simple rectángulo absoluto del escritorio.

Neeko conserva una sola región con coordenadas físicas. Si se mueve el juego, el rectángulo no lo sigue. Los perfiles guardan esa geometría; no guardan una relación con la ventana. Una ventana superpuesta puede entrar en la captura GDI. La conciencia DPI del selector ayuda a elegir coordenadas correctas, pero no implementa seguimiento ni restauración inteligente después de cambiar de monitor.

| Falta | Efecto práctico | Trabajo necesario |
|---|---|---|
| Seleccionar ventana objetivo | Capturar el juego con mayor independencia de lo que lo tape | Windows Graphics Capture, ciclo de frames, cierre y cambios de tamaño |
| Regiones relativas a ventana | Mover el juego sin volver a seleccionar todo | Identidad del origen y transformación entre coordenadas |
| Varias regiones | Leer nombre, diálogo y opciones sin todo el fondo intermedio | IDs de región, resultados separados, presupuesto compartido |
| Exclusiones | Ignorar HUD, reloj o zonas animadas | Máscaras aplicadas antes del OCR |
| Seguimiento del mouse | Leer tooltips y menús | Temporización y región móvil acotada |
| Inspección previa | Ver qué está leyendo realmente | Vista previa voluntaria, sin escritura automática a disco |

Captura por ventana tampoco garantiza funcionar con cualquier juego minimizado, contenido protegido o pantalla completa exclusiva. Eso se valida por backend y aplicación. [Captura](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/ScreenCapture/ScreenCaptureProcesser.cs), [gestión de áreas](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Manager/FormManager.cs), [seguimiento](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/MouseFollowOcrArea/MouseFollowOcrAreaService.cs).

## 5. Preparación de imagen y corrección del texto

MORT expone zoom, umbral de binarización, selección de colores RGB/HSV, grupos de color y erosión. Parte del procesamiento se ejecuta en MORT_CORE; el repositorio principal permite seguir cómo se configuran esas opciones, pero no verificar cada algoritmo nativo. También aplica diccionario de correcciones, eliminación de espacios y tratamiento de saltos de línea según modo.

Neeko únicamente reduce imágenes que exceden el máximo del OCR y las convierte a un formato aceptado por Windows. No tiene un ajuste para agrandar letras pequeñas, eliminar fondos complejos o corregir confusiones sistemáticas.

Esto afecta directamente a Google: si el OCR entrega un nombre incorrecto o mezcla dos bocadillos, el traductor recibe ese error. Cambiar de proveedor no arregla por sí mismo la entrada.

Conviene incorporar ajustes sencillos y reversibles por perfil: escala, contraste/umbral opcional, inversión cuando corresponda y vista previa. Un modo automático agresivo puede destruir letras finas; debe conservarse el original y permitir comparar. Los diccionarios deben distinguir corrección OCR de traducciones preferidas para nombres y terminología. [Opciones de imagen](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/FormOption.cs), [tratamiento textual](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/ProcessTranslateService/ProcessTranslateService.cs).

## 6. Las tres ventanas de traducción de MORT

| Modo | Comportamiento | Equivalencia actual |
|---|---|---|
| `TransForm` / dark | Ventana de resultados con texto | El addon muestra resultados, pero no reproduce ese modo específico |
| `TransFormLayer` / layer | Texto flotante con fondo/contornos/alineación y control de interacción | Aproximación visual parcial |
| `TransFormOver` / over | Distribuye bloques traducidos sobre sus áreas de origen | No implementado |

El último cambio visual de Neeko agregó contorno negro, fondo por línea, transparencia y controles al pasar el mouse. Eso mejora la presentación del texto flotante. **No implementó el overlay espacial de MORT.**

El modo espacial trabaja con rectángulos de origen, de presentación y de contenido; mantiene dirección, títulos y agrupaciones; resuelve colisiones; mide si el texto cabe; ajusta tamaño y puede analizar colores del fondo y de la fuente. En la revisión actual hay opciones independientes para contorno y alpha, además de conservación de dirección. Por eso tampoco existe una única apariencia universal de «MORT»: depende del modo y de la configuración.

Neeko devuelve solo `text` desde el auxiliar. Aunque Windows OCR dispone de cajas de palabras, nuestro adaptador las descarta. Para avanzar se necesita un resultado estructurado con región, tamaño de imagen, escala, líneas, palabras, rectángulos y orientación cuando exista. Confianza solo debe incluirse si el motor la expone; no inventar una puntuación homogénea.

Luego hay que agrupar bloques sin mezclar títulos, nombres o columnas, traducir manteniendo un identificador por bloque y dibujar cada resultado dentro de sus límites. El texto español puede ocupar más espacio; hacen falta reglas de ajuste y colisión.

También falta **dejar pasar clics al juego**. Transparencia visual no equivale a transparencia de entrada. MORT usa estilos de ventana para ello; Neeko no llama a una operación equivalente y su ventana puede interceptar interacción en su superficie. Se necesita un modo bloqueado y una forma accesible de desbloquearlo desde el panel o un atajo global. Ocultar la barra al salir el mouse no resuelve esto.

Otras diferencias: color de texto/fondo/contorno configurables, familia y estilo de fuente, alineación, dirección RTL, persistencia de posición/tamaño del overlay y controles separados para ocultar, pausar y detener. Actualmente Neeko conserva posición durante la vida de la ventana, pero no la persiste en sus perfiles; la región OCR y la ventana de traducción son dos geometrías diferentes.

Fuentes: [ventana básica](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/TransForm.cs), [flotante](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/TransFormLayer.cs), [espacial](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/TransFormOver.cs), [colores](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/Overlay/OverlayColorAnalyzer.cs).

## 7. Proveedores de traducción

MORT registra diez tipos base, más presets de Custom API. No todos requieren modelos locales; tampoco todos tienen la misma preparación ni garantía de disponibilidad.

| Tipo | Mecanismo observado | Neeko / recomendación |
|---|---|---|
| Google Web | RPC web, GTX y endpoint de diccionario | Implementado parcialmente respecto al comportamiento completo de MORT |
| DB | Traducciones precargadas/búsqueda nativa | Falta; útil para traducciones conocidas |
| Papago Web | HTTP al sitio de Papago y temporización | Falta; opción futura con pruebas de disponibilidad |
| Naver/Papago API | Endpoint y credenciales | Falta; opcional |
| Google Sheets | Cliente Sheets y preparación de hoja | Falta; prioridad baja para esta app |
| DeepL web | WebView2 y lectura/escritura de elementos web | Falta; implica navegador adicional y mantenimiento del DOM |
| DeepL API | HTTP autenticado con endpoint Free/Pro | Falta; alternativa directa si se configura una clave |
| Gemini | API de modelo remoto | Existe en MORT; no hace falta para cumplir el pedido del usuario |
| ezTrans | Motor externo con IPC específico | Falta; caso particular, no traductor universal |
| Custom API | URL, cabeceras y plantillas de solicitud/respuesta | Falta; útil después de estabilizar el contrato de proveedores |

Los tres endpoints de Google actuales siguen siendo un solo proveedor. Si Google limita el uso, no hay un traductor independiente al que recurrir. MORT además conserva un estado temporal de degradación para evitar insistir continuamente con su ruta preferida y tiene una opción de traducción intermedia por japonés. Neeko no reproduce esas dos funciones. La segunda duplicaría trabajo de traducción y no es prioritaria para el objetivo de consumo.

El addon limita cada solicitud a cinco segundos, con hasta tres rutas secuenciales; un error 429 detiene la sesión inmediatamente. Es una decisión propia razonable, pero no debe presentarse como clon exacto del comportamiento de MORT. Otros fallos terminan en un mensaje genérico. Faltan diagnóstico por proveedor, pausa recuperable y una política explícita de reintento acotado. No se propone alternar endpoints para eludir límites.

La ampliación de idiomas de destino necesita eliminar el catálogo duplicado de nueve elementos. El mapeo actual corta etiquetas por el guion salvo casos chinos; al agregar motores y proveedores debe convertirse en un mapeo explícito para no perder distinciones significativas.

Fuentes: [catálogo real](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/TranslateTyp/TranslateTypListService.cs), [Google](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/TransAPI/GoogleBasicTranslateAPI.cs), [DeepL web](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/TransAPI/DeeplWebView.cs), [presets API](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/docs/CustomApiPreset.md).

## 8. Caché, memoria visual, diccionarios y DB

Son cuatro funciones distintas:

| Función | Para qué sirve | Estado actual |
|---|---|---|
| Deduplicación inmediata | No retraducir la misma lectura consecutiva | Sí, tras normalizar espacios |
| Caché de traducción | Reutilizar una frase vista antes, aunque haya desaparecido | No |
| Memoria visual | Mantener resultados recientes en pantalla durante cierto tiempo | Solo historial en panel; no modo de retención equivalente |
| Diccionario/DB | Corregir OCR o imponer traducciones conocidas | No |

En MORT `TransManager` consulta resultados anteriores y traducciones del usuario; puede cargar/guardar archivos y traducir solo bloques no encontrados. `TranslateResultMemoryService` mantiene resultados para presentación con límite y vencimiento. Confundir esa memoria visual con la caché de red llevaría a diseñar mal la integración.

Ejemplo actual: aparece A, después B y luego vuelve A. Neeko consulta Google tres veces; una caché podría resolver la tercera localmente. Si A desaparece y reaparece, la deduplicación también se reinicia. Guardar el historial de 50 entradas no evita estas solicitudes porque no se consulta como caché.

Propuesta: caché acotada de sesión, con clave que incluya texto normalizado, origen, destino, proveedor y versión de reglas/glosario; límite por entradas y bytes. Persistencia opcional por perfil, con borrado visible. El esquema de MORT no debe copiarse ciegamente: la estructura principal observada agrupa por tipo de traductor y texto, de modo que en Neeko conviene hacer explícitos todos los parámetros para evitar reutilizaciones entre idiomas o presets.

Fuente: [TransManager](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Manager/TransManager.cs), [memoria visual](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Service/ProcessTranslateService/TranslateResultMemoryService.cs).

## 9. Controles, portapapeles y uso en juegos

MORT tiene mando separado, atajos globales, snapshot, inicio/parada, acciones de áreas, seguimiento del cursor y portapapeles. El portapapeles permite recibir texto de herramientas externas sin ejecutar OCR; el soporte no equivale a que MORT incluya un extractor universal de texto de cualquier juego. También hay salida al portapapeles, guardado de resultados y TTS.

Neeko depende del panel o del botón del overlay. Escape funciona cuando esa ventana tiene foco; no es una tecla de emergencia global mientras el juego está activo. Faltan pausa/reanudación, ocultar sin detener, bloquear interacción, snapshot por atajo, copiar resultado y traducción desde portapapeles.

Para el uso cotidiano, atajos y paso de clics tienen una relación esfuerzo/beneficio alta. Portapapeles puede ahorrar todo el OCR cuando otra herramienta ya entrega texto; debe activarse expresamente y distinguir entrada externa de lo que la propia app copia para evitar bucles. TTS es opcional y no debería utilizar por defecto los recursos del asistente si el usuario quiere evitarlo.

Fuentes: [atajos](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/KeyHook/GlobalKeyboardHook.cs), [acciones](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Form1Button.cs), [portapapeles](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/ClipboardAssist/ClipboardMonitor.cs).

## 10. Consumo de recursos y defectos visibles del addon

No hay mediciones que permitan afirmar «X% menos CPU» o un costo fijo de cada motor. Sí hay trabajo identificable en el código:

| Hallazgo en Neeko | Consecuencia | Mejora propuesta |
|---|---|---|
| OCR en cada vuelta aunque los píxeles no cambien | La deduplicación ahorra traducción, no captura/OCR | Detección de cambio de región antes del reconocimiento, con tolerancia y medición |
| Conversión bitmap → PNG en memoria → decodificación | Compresión, copias y asignaciones por lectura | Medir y considerar acceso directo a pixels/SoftwareBitmap |
| Ausencia de caché histórica | Repite consultas de diálogos recurrentes | Caché acotada con claves completas |
| Borra la traducción antes de recibir la nueva | El recuadro puede parpadear o quedar vacío durante la red | Retener el resultado anterior con estado claro hasta el reemplazo |
| Un OCR vacío limpia inmediatamente | Un fallo de una lectura hace desaparecer el diálogo | Retención breve configurable o confirmación de varias lecturas vacías |
| Sin estabilización de texto que aparece letra por letra | Puede traducir frases incompletas sucesivas | Esperar estabilidad con latencia máxima acotada |
| Intervalo se suma al tiempo OCR y de red | 1500 ms de pausa no significa una traducción cada 1500 ms | Mostrar duración efectiva y separar frecuencia de lectura/envío |
| Estado completo emitido varias veces por ciclo | Incluye historial/configuración que no siempre cambiaron | Eventos más pequeños y suscripción según vista visible |
| Overlay se oculta, no se destruye, al detener | Su WebView puede permanecer residente | Medir costo y decidir reutilización frente a liberación |
| Auxiliar autocontenido incluido siempre | Aproximadamente 140 MB de archivo, aun con addon deshabilitado | Preparación modular si el tamaño de distribución resulta importante |

Los 140 MB son tamaño en disco de la compilación previa, no RAM consumida. Neeko usa WebView2 por ser Tauri en Windows; decir que no usa un navegador oculto significa que Google Web no abre uno adicional para traducir, no que la aplicación carezca de procesos WebView.

Priorizar caché, región pequeña, detección de cambios y estabilización antes de incorporar OCR con GPU. Estas últimas optimizaciones son propuestas para Neeko; no se atribuye a MORT una detección de cambios de imagen que no se haya verificado. MORT también compara texto previo y posterior en su ciclo.

## 11. Preparación, distribución y licencia

La parte JS no es autónoma: el addon depende del comando Rust compilado en Neeko, su ventana autorizada y el auxiliar C#. Copiar solamente la carpeta del addon a una versión vieja no incorpora el backend. Hoy la preparación comprueba un motor ya distribuido; no instala motores ni paquetes de idioma independientes.

Una experiencia instalable más completa necesita manifest de componentes, versiones, arquitectura, descargas verificadas, progreso/cancelación, reparación y eliminación segura de recursos propios. No hace falta integrar el actualizador de MORT: Neeko ya tiene su distribución y actualización.

MORT tiene licencia MIT para el código cubierto por su aviso. Los componentes de Microsoft, motores externos, datos de idioma y dependencias tienen su propia procedencia. Para una distribución se necesita inventario de esos componentes y sus avisos. En particular, no tratar las DLL/modelos OneOCR como si heredaran MIT por aparecer en un adaptador MIT. Esto delimita trabajo de distribución; no se emite aquí un dictamen legal sobre su reutilización.

Los binarios auxiliares de MORT_CORE y nhocr se referencian desde el proyecto, y el README advierte que el árbol principal no basta para disponer de todas las dependencias de ejecución. El pipe de ezTrans resuelve esa integración concreta. No se encontró una interfaz general documentada y usada por el flujo principal que permita arrancar MORT entero como servicio headless con todas sus opciones. Esa ausencia observada no prueba que jamás pueda desarrollarse una.

Fuentes: [licencia](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/LICENSE), [interoperabilidad nativa](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/Form1.cs), [pipe específico](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/PipeClient/PipeEzTransAPI.cs), [actualizador](https://github.com/kmonkeyhead/MORT/blob/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a/MORT/VersionCheck/VersionCheckLogic.cs).

## 12. Qué conviene conservar y qué evitar copiar literalmente

Conservar en Neeko el supervisor único, cancelación, generación para rechazar resultados viejos, validación de geometría, historial acotado, renderizado por `textContent`, destinos de red controlados y captura solo tras acción del usuario. Son bases útiles aunque MORT tenga muchas más funciones.

Reutilizar conceptos y adaptadores aislados. El código de MORT muestra convivencia de UI y estado global, hilos dedicados con espera de tareas, compatibilidad histórica de enums/settings, procesamiento nativo y asignaciones de imágenes. Trasplantar managers completos acoplaría Neeko a los formularios y a todas esas dependencias.

Hay aspectos que merecen validación específica si se portan: inicialización/cancelación de threads, cambios de tamaño/DPI, vida de DLL y objetos gráficos, versiones de Python, ajustes de Release que deshabilitan optimización en el proyecto principal, y proveedores web dependientes de formatos externos. Son observaciones estáticas; no se afirma que todos produzcan un fallo o que MORT tenga peor rendimiento.

No se encontró un proyecto convencional de pruebas automatizadas en la solución examinada. La wiki, herramientas de generación y snapshots de depuración ayudan, pero no sustituyen pruebas de integración. Neeko ya tiene pruebas propias; las comprobaciones previas no certifican todavía selección multi-monitor, paso de clics ni traducción dentro de juegos.

## 13. Arquitectura de destino recomendada

```text
Panel Neeko: perfil, motores instalados, idiomas, región, controles
                         ↓
Supervisor nativo de sesión
    ├─ Origen: escritorio / ventana / texto del portapapeles
    ├─ Regiones y exclusiones + detección de cambios
    ├─ OCR seleccionado: Windows / Tesseract / OneOCR evaluado
    ├─ Resultado estructurado: líneas, cajas, escalas, dirección
    ├─ Corrección y agrupación por bloques
    ├─ Caché de traducciones + proveedor web configurado
    └─ Presentación: flotante / bloques sobre el original
```

Cada motor debe declarar capacidades: idiomas, necesidad de instalación, cajas/orientación disponibles, compatibilidad de modo continuo y si envía imágenes a un servicio. La UI se construye con esa información, en lugar de mantener listas incompatibles en JS, Rust y C#.

El worker permanece independiente de la IA de Neeko. Los resultados incluyen identificadores de sesión, frame, región y bloque para impedir que una traducción retrasada aparezca sobre otra escena. Las cajas necesitan una transformación explícita después de escalar o recortar la imagen.

Para varios bloques, no enviar una solicitud ilimitada por palabra ni concatenar todo perdiendo correspondencia. Usar agrupación, presupuesto y caché. Si un proveedor no preserva separadores de forma fiable, su adaptador necesita otra estrategia de asociación y validación.

## 14. Orden propuesto de implementación

| Prioridad | Entrega | Dificultad relativa | Cómo verificar que terminó |
|---|---|---|---|
| P0 | Catálogo unificado de motores/idiomas y destino ampliable | Media | No ofrece idiomas como instalados si no lo están; perfiles antiguos siguen abriendo |
| P0 | Tesseract con preparación de idiomas independiente de Windows | Media/alta | Reconoce una imagen en idioma ausente de Windows usando datos propios; solo carga el seleccionado |
| P0 | Paso de clics, bloqueo y atajos globales | Media | Se juega debajo del overlay y se desbloquea sin quedar atrapado |
| P0 | Caché de sesión, retención visual y estabilización | Media | Secuencia A-B-A reutiliza A; texto progresivo no genera una petición por cada cambio |
| P1 | OCR estructurado y agrupación | Alta | Conserva posiciones después de escalar y no mezcla dos burbujas |
| P1 | Captura por ventana y coordenadas relativas | Alta | Mover/redimensionar objetivo mantiene alineación, con pruebas DPI |
| P1 | Overlay espacial equivalente al concepto de `over` | Alta | Traducciones más largas caben sin cubrir bloques vecinos |
| P1 | Preprocesamiento y correcciones por perfil | Media | Mejora casos de prueba sin degradar el original por defecto |
| P2 | Varias regiones/exclusiones y mouse follow | Media/alta | Presupuesto acotado y asociación correcta de cada región |
| P2 | Proveedores alternativos y Custom API | Media/alta | Errores, credenciales, idiomas y respuestas verificados por adaptador |
| P2 | Portapapeles y exportación opcional | Media | No se retraduce la salida propia ni se persisten capturas por accidente |
| P3 | OneOCR tras validar disponibilidad/procedencia | Alta por distribución | Matriz de equipos/versiones y origen de componentes documentados |
| P3 | EasyOCR, Google Vision y TTS opcionales | Alta/variable | Consumo y transmisión de datos visibles; no se activan implícitamente |
| P3 | Catálogo de perfiles/DB compartidos | Media/alta | Importación versionada y validación de contenido |

P0 no significa que todos deban hacerse en un único cambio. El OCR estructurado debería diseñarse desde el principio para no tener que rehacer la interfaz del worker después. OneOCR puede adelantarse si una prueba corta demuestra una integración disponible y distribuible adecuada; la prioridad actual refleja incertidumbre, no inferioridad técnica del motor.

## 15. Plan de validación necesario

Una batería representativa debe cubrir diálogos sintéticos y muestras autorizadas: inglés/español, japonés horizontal/vertical, chino, texto pequeño, fondos animados, dos burbujas, título más cuerpo, nombres, diálogos que aparecen progresivamente y ausencia temporal de texto. No hace falta instalar todos los idiomas del catálogo para validar el mecanismo de instalación, pero sí probar motores y alfabetos elegidos.

Medir por separado captura, OCR, traducción, dibujo, solicitudes por minuto, aciertos de caché, RAM estable y liberación tras detener/desactivar. Comparar la misma región y secuencia en cada motor, con muestras repetidas y latencias típicas/p95. La prueba de OCR sintético anterior demuestra que el worker funciona, no que sea el mejor para letras de un juego.

Pruebas de escritorio: monitores con 100/150/200% de escala, coordenadas negativas, ventana que se mueve, resolución que cambia, overlay sobre región, paso de clics, foco del juego, stop durante OCR/red, cierre del objetivo, cambio de perfil y salida de Neeko. Probar red lenta, 429 y formatos inválidos sin disparar reintentos ilimitados.

La finalización de cada fase debe demostrar comportamiento observable. «El menú lista japonés» no basta para completar soporte de japonés; «se ve transparente» no basta para completar paso de clics; «las frases aparecen» no basta para completar overlay espacial.

## Conclusión para este proyecto

La base actual es aprovechable y ya evita usar la IA local. Lo que falta para la experiencia pedida es una integración de OCR más amplia y una representación espacial del resultado, acompañadas de controles y caché. La evolución más útil empieza por idiomas propios, clics/atajos y estabilidad; luego captura por ventana y overlay por bloques. Copiar MORT entero en segundo plano añadiría una segunda aplicación compleja sin resolver automáticamente su control desde Neeko.

Referencias locales principales: [supervisor](../src-tauri/src/screen_translate.rs), [Google Web](../src-tauri/src/screen_translate/google.rs), [worker](../native/screen-translate/Program.cs), [Windows OCR](../native/screen-translate/WindowsOcr.cs), [panel](../addons/screen-translate/main.js), [overlay](../src/screen-translate-overlay.js), [documentación actual](screen-translate.md).
