# Prompt de implementación: Screen Translate

Implementar un addon integrado de traducción OCR para Neeko Assistant, inspirado en MORT (https://github.com/kmonkeyhead/MORT). El usuario administra todo desde Neeko: no debe abrir ni manejar otra aplicación.

Antes de implementar, revisar el código actual, las modificaciones locales y las partes reutilizables de MORT y sus licencias. Conservar los cambios existentes. Elegir la integración técnica según evidencia: reutilizar componentes adecuados; no asumir que MORT ofrece una API headless completa ni copiar su interfaz entera. Documentar cualquier alternativa elegida.

Entregar:

- Addon `screen-translate` con manifest, panel JS/CSS en español, instrucciones y configuración persistente.
- Preparación/verificación del motor OCR desde el panel; explicar dependencias reales y errores recuperables.
- Selección visual de una región de pantalla, idiomas de origen/destino y traducción puntual o continua.
- Captura y OCR locales en segundo plano, sin consolas ni interfaz de MORT. Capturar únicamente cuando el usuario lo inicia.
- Corrección solicitada por el usuario: no utilizar su IA local. Revisar el traductor original de MORT e integrar Google Web con tiempos límite y control de concurrencia. Informar en el panel que solo el texto reconocido se envía a Google; nunca enviar capturas. No cargar modelos ni usar llama-server, tampoco como respaldo.
- Overlay de traducciones controlado desde Neeko, ajustable y con original/traducción e historial limitado; perfiles de región e idiomas.
- Evitar traducciones repetidas de texto idéntico y resultados antiguos después de detener/cambiar región. Excluir el overlay de las capturas cuando el sistema lo permita.
- Un único runtime nativo para main/settings; eventos de estado; iniciar/detener de forma determinista, cerrar recursos al desactivar y al salir. Cerrar Configuración no debe detener una sesión activa.
- Operaciones nativas acotadas a este addon y ventanas autorizadas; validar rutas/entradas/geometría; no exponer shell arbitrario ni endpoints de red nuevos.
- Integrar recursos y permisos de ventanas en el empaquetado existente.
- Pruebas significativas de estado, cancelación, deduplicación y UI; comprobar compilación y pruebas existentes. No afirmar pruebas de juegos/hardware que no se hayan realizado.

Completar la implementación y validación; informar qué funciona, cómo usarlo y cualquier límite pendiente. No crear commits ni publicar releases.
