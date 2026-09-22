# 🌸 Neeko Asistente

Asistente virtual estilo Clippy con IA local usando Qwen3 4B.

## Requisitos

- **Rust** (rustc 1.94.1 o superior)
- **Node.js** (v22 o superior)
- **Tauri CLI** (`cargo install tauri-cli`)

## Instalación

```bash
cd neeko-assistant
npm install
```

## Ejecutar en modo desarrollo

```bash
npm run dev
```

## Build para producción

```bash
npm run build
```

## Uso

1. **Click en Neeko Asistente** - Abre el panel de chat
2. **Escribe un mensaje** - Neeko Asistente responderá con su IA
3. **Speech bubbles** - Neeko Asistente muestra frases aleatorias cuando está idle
4. **Controles** - Botones para minimizar/cerrar en la esquina superior derecha

## Personalización

- Cambia `src/styles.css` para modificar colores y animaciones
- Editá `src-tauri/src/ia.rs` para cambiar el prompt del sistema
- Modificá `src/main.js` para agregar más frases idle

## Estructura

```
neeko-assistant/
├── src/               # Frontend (HTML/CSS/JS)
├── src-tauri/         # Backend Rust
├── IA/                # Modelo Qwen3 4B
└── public/            # Assets estáticos
```

## Notas

- El modelo se carga la primera vez que se usa
- La IA responde localmente (sin internet)
- Neeko Asistente puede ayudar con preguntas simples y tareas básicas


## Avatar 3D

Neeko Asistente funciona con el chat sin instalar un personaje. No incluye las imágenes antiguas ni el GLB de Neeko. Desde el botón de configuración del chat, abrí **Apariencia de IA**:

1. Abrí [Khada](https://modelviewer.lol/?lang=es-ES) con el botón de descarga.
2. Elegí el modelo de **Neeko** y exportalo como **GLB**, con texturas y todas sus animaciones incluidas.
3. Usá **Importar GLB de Neeko** para instalar el archivo (hasta 100 MB).

Al importarlo aparece Neeko con sus animaciones y controles habituales. El archivo se guarda localmente y se conserva al reiniciar. Si falta o no se puede cargar, la aplicación abre el chat. Los modelos pertenecen a sus titulares; descargarlos no transfiere sus derechos. Las claves internas históricas se conservan para mantener configuraciones y compatibilidad con addons.
