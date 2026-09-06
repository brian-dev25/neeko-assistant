# AIRI · Compañera de escritorio

Neeko permanece sobre el escritorio sin fondo ni barra. El chat se abre en una
ventana independiente, con historial, confirmaciones de acciones y cancelación.

## Uso

1. Instalar y reiniciar la versión nueva de Neeko. Actualizar solo el addon no crea
   la ventana de chat en un ejecutable anterior.
2. Activar Configuración → Addons → AIRI · Compañera de escritorio.
3. Clic en Neeko o clic derecho → Hablar con Neeko abre la ventana de chat.
4. Mover, minimizar o cerrar el chat no mueve ni redimensiona el personaje.
5. Cerrar el chat o pulsar Escape lo oculta. Al reabrirlo, conserva su conversación
   y cualquier confirmación pendiente durante esta ejecución de Neeko.
6. Arrastrar a Neeko mueve el personaje. Clic derecho abre sus controles.
7. Cambiar **Tamaño de Neeko** desde el menú de clic derecho o Configuración →
   Escritorio. El rango de 30–100 % corresponde a la altura de su ventana; la base
   queda fija y el chat independiente no cambia. La elección se conserva al reiniciar.
   **Restablecer tamaño** vuelve al 82 % original.

El modo escritorio elimina la oscilación CSS del contenedor y el desplazamiento
artificial del modelo, conservando las animaciones propias del personaje.
En Windows, la región nativa sigue la silueta del render (3D o PNG): el espacio
transparente deja pasar los clics a las aplicaciones de abajo. Se conserva un
pequeño margen en el contorno para no cortar bordes suavizados. Los controles y
Configuración se incluyen mientras están visibles. Al desactivar el addon se
restaura la ventana rectangular. Requiere el ejecutable actualizado.
Las frases automáticas no aparecen sobre el escritorio. Una confirmación ya pendiente
antes de activar el addon sigue accesible hasta responderla.

Desactivar el addon restaura la interfaz normal y el estado anterior de siempre visible.
El chat independiente tiene su propia sesión; no copia el historial del chat integrado.

Para instalación manual: `%APPDATA%/neeko-assistant/addons/airi-theme/`.
Una copia allí tiene prioridad sobre la incluida en la app y también debe actualizarse.
Inspirado en https://github.com/moeru-ai/airi; no añade modelos Live2D/VRM.
