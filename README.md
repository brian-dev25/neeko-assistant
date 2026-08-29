# 🌸 Neeko Assistant

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

1. **Click en Neeko** - Abre el panel de chat
2. **Escribe un mensaje** - Neeko responderá con su IA
3. **Speech bubbles** - Neeko muestra frases aleatorias cuando está idle
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
- Neeko puede ayudar con preguntas simples y tareas básicas
