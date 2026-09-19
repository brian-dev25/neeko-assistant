# Screen Translate: inicio y panel simplificado

## Error corregido

Al pulsar Iniciar continuo, readSettings intentaba consultar .type de cuatro controles ausentes (colorFilter, filterColor, colorTolerance y erode). El fallo ocurria antes de enviar la accion al backend. La lectura ahora omite controles ausentes y conserva sus valores guardados; no los resetea.

## Flujo visible

1. Preparar traductor y elegir idiomas.
2. Elegir el area de texto.
3. Probar una vez, Iniciar continuo o Detener.

La traduccion actual y los errores quedan junto a las acciones. Motor, proveedores, apariencia, imagen, diccionarios, perfiles, cache y controles secundarios estan dentro de Ajustes avanzados, cerrado inicialmente. Se reutilizan los controles existentes, sin duplicar configuraciones. Los idiomas quedan bloqueados durante una sesion y se habilitan al detenerla.

La prueba scripts/screen-translate-ui.test.html carga el addon real con un transporte simulado. Comprueba que Iniciar continuo guarda y envia start, conserva configuracion avanzada sin controles visibles, respeta borradores, restaura controles al detener y permite el portapapeles sin OCR.
