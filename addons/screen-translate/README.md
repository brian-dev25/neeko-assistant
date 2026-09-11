# Screen Translate

Traduccion integrada dentro de Neeko. No utiliza el modelo de IA de Neeko.

## Modo integrado de Neeko

1. Pulsar **Comprobar motor e idiomas**. El auxiliar OCR y su runtime .NET vienen con Neeko.
2. Tesseract en inglés es el valor predeterminado, como MORT 1.291. Descargar el idioma si falta. También están disponibles OneOCR (detección automática, componentes de Recortes/Fotos) y Windows OCR.
3. El idioma OCR es el de la imagen. El idioma de destino solo afecta a la traduccion; no necesita instalarse en Windows.
4. Elegir región y destino, y pulsar **Traducir una vez** o **Iniciar continuo**. Por defecto usa Google Web, texto flotante e intervalo de 1000 ms. El modo sobre bloques es opcional; activar tamaño automático si la traducción no cabe.
5. Para texto copiado, elegir **Portapapeles (sin OCR)** y un panel o texto flotante. No requiere area ni idioma OCR instalado.
6. Guardar perfiles o exportar/importar archivos JSON desde los botones correspondientes.
7. **Aplicar valores de MORT 1.291** actualiza una configuración existente conservando áreas, destino y diccionario. **Idioma de origen al traducir** permite fijar el idioma cuando la detección automática de Papago confunde una frase corta.

Incluye varias regiones, exclusiones, captura WGC por ventana, correcciones y diccionario de frases, caché durante la ejecución, traducción actual, ajustes de imagen y controles de overlay. La espera opcional vale 0 por defecto; si un perfil antiguo tiene otro valor, puede cambiarse a 0 para traducir desde la primera lectura. No reproduce todos los algoritmos, formatos DB y proveedores de MORT. La auditoría de la versión solicitada está en `docs/mort-1.291-review.md`.

Los atajos integrados son Ctrl+Alt+F6 iniciar, F7 toma puntual, F8 pausa, F9 detener, F10 bloqueo de clics y F11 mostrar/ocultar. 

La captura integrada solo comienza por accion del usuario. Con Google Web se envia texto reconocido; las capturas no se guardan ni se envian a Google. 

## Compilacion y licencias

`node scripts/build-screen-ocr.mjs` compila el auxiliar con .NET SDK 9. La distribucion necesita todos los archivos de `binaries/screen-translate`, no solo el EXE. Los avisos de licencia se copian junto al runtime.

El codigo adaptado de MORT conserva su licencia MIT y avisos de procedencia.
