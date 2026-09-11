# Comparacion actual con MORT

> Referencia actual solicitada: [auditoría de MORT 1.291](mort-1.291-review.md). La tabla de este documento corresponde a la revisión posterior `ea9afbc`; no debe atribuirse a 1.291. El historial fue eliminado y la espera/actualización del overlay se corrigieron en la revisión nueva.

## Decision de integracion

El usuario rechazo el panel para instalar y abrir MORT como aplicacion externa. Se retiraron el panel, las acciones del backend, el instalador auxiliar y la supervision de ese proceso. El addon mantiene un unico flujo integrado dentro de Neeko.

MORT se usa como referencia de implementacion. Su disponibilidad como programa externo no cuenta como una funcion implementada dentro del addon. La equivalencia funcional completa sigue pendiente; la tabla identifica las diferencias reales.

## Comparacion de capacidades

| Capacidad | Modo integrado | MORT original (referencia) |
|---|---|---|
| Windows OCR | Implementado; depende de idiomas OCR Windows | Implementacion oficial |
| Tesseract | Implementado; 124 conjuntos descargables de tessdata_fast | Implementacion oficial y sus datos |
| Snipping Tool / OneOCR | Implementado; usa componentes Microsoft disponibles; predeterminado para nuevas configuraciones | Implementacion oficial |
| EasyOCR | No adaptado | Opcion original; requiere preparar sus dependencias |
| Google Cloud Vision OCR | No adaptado | Opcion original; requiere credenciales y respeta sus restricciones de captura puntual |
| Google Web | Implementado y probado con texto sintetico | Proveedor original |
| Papago | Adaptador existente; no verificado contra el servicio en esta entrega | Proveedor original |
| DeepL, Google Sheets, ezTrans, API personalizada | Sin equivalencia completa; retirada la opcion DeepL API sin credenciales que no podia funcionar | Opciones originales; requieren configuracion propia |
| Superposicion por bloques | Coordenadas, fondo, ajuste de fuente, escalado de pantalla | Renderizador oficial, incluyendo sus algoritmos de distribucion y colores |
| Analisis automatico de colores y distribucion avanzada | No equivalente al algoritmo de MORT | Implementacion oficial |
| Varias areas y exclusiones | Implementado con limites propios | Implementacion oficial |
| Captura por ventana | WGC y coordenadas de ventana | Implementacion oficial |
| Brillo, contraste y escala de grises | Reparados: los controles llegan al procesamiento de imagen | Implementacion oficial |
| Portapapeles | Ahora accesible sin OCR ni area de captura, en panel o texto flotante | Implementacion oficial |
| Diccionarios y correcciones | Reglas simples y frases completas; no equivale al editor/DB de MORT | Editor, DB y formatos originales |
| Importar/exportar | Reparado con selector de archivo JSON; formatos propios | Formatos originales |
| Atajos y control remoto | Subconjunto propio de Neeko | Controles originales |
| Historial y cache | Memoria limitada por sesion; reglas diferentes | Implementacion oficial |
| Interfaz y localizacion | Panel propio de Neeko | Ventanas e idiomas de interfaz originales |


## Validacion y limites

Se conservan OneOCR automatico, Tesseract con idiomas independientes, OCR Windows, traduccion Google, superposicion espacial, procesamiento de imagen, portapapeles y archivos de configuracion. No se alteraron las configuraciones personales de la instalacion externa que habia sido descargada durante las pruebas.

Referencia de codigo: https://github.com/kmonkeyhead/MORT/tree/ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a . La auditoria original esta en mort-gap-analysis.md.
