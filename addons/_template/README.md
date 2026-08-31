# Crear Addons para Neeko Assistant

Guia completa para crear addons que extiendan las funcionalidades de Neeko.

---

## Estructura de un Addon

Cada addon es una **carpeta** dentro de `addons/` con esta estructura:

```
addons/
  mi-addon/
    addon.json          # REQUERIDO - Manifest con metadata
    main.js             # OPCIONAL - Logica frontend (JavaScript)
    styles.css          # OPCIONAL - Estilos CSS custom
    icon.png            # OPCIONAL - Icono del addon (64x64)
```

### Archivos minimos

Un addon solo necesita `addon.json` para funcionar. Los archivos `main.js` y `styles.css` son opcionales.

---

## addon.json (Manifest)

El manifest es un archivo JSON con la metadata del addon:

```json
{
  "id": "mi-addon",
  "name": "Mi Addon",
  "version": "1.0.0",
  "description": "Que hace mi addon",
  "author": "Mi Nombre",
  "minAppVersion": "1.2.0",
  "permissions": ["chat:commands", "ui:tabs"],
  "commands": [
    {
      "id": "mi-comando",
      "description": "Descripcion del comando",
      "patterns": {
        "es": ["mi\\s+comando\\s+(.+)"],
        "en": ["my\\s+command\\s+(.+)"]
      }
    }
  ]
}
```

### Campos del manifest

| Campo | Requerido | Tipo | Descripcion |
|-------|-----------|------|-------------|
| `id` | Si | string | Identificador unico del addon (sin espacios) |
| `name` | Si | string | Nombre visible |
| `version` | Si | string | Version semver (1.0.0) |
| `description` | No | string | Descripcion corta |
| `author` | No | string | Nombre del autor |
| `minAppVersion` | No | string | Version minima de Neeko requerida |
| `icon` | No | string | Archivo de icono (relativo a la carpeta del addon) |
| `permissions` | No | array | Lista de permisos que necesita |
| `commands` | No | array | Comandos que registra |

### Permisos disponibles

| Permiso | Que permite |
|---------|-------------|
| `chat:commands` | Registrar comandos en el chat |
| `ui:tabs` | Crear tabs en Configuracion |
| `ui:bubble` | Mostrar mensajes en el speech bubble |
| `config:read` | Leer configuracion de Neeko |
| `config:write` | Modificar configuracion de Neeko |
| `network:http` | Hacer peticiones HTTP |
| `fs:read` | Leer archivos |
| `fs:write` | Escribir archivos |

### Formato de comandos

Cada comando tiene:

- **id**: Identificador unico del comando
- **description**: Descripcion del comando
- **patterns**: Objeto con regex por idioma

```json
{
  "patterns": {
    "es": ["traducir\\s+(.+)", "traduce\\s+(.+)"],
    "en": ["translate\\s+(.+)"]
  }
}
```

Los patrones son **regex** (sin las barras `/`). El texto capturado por `(.+)` se pasa al handler como `matches[1]`.

---

## API de JavaScript (main.js)

### Objeto global: `Neeko`

Cuando tu addon se carga, el objeto `Neeko` esta disponible globalmente.

### Registrar comandos

```js
Neeko.commands.register('mi-comando', {
    patterns: {
        es: ['mi\\s+comando\\s+(.+)'],
        en: ['my\\s+command\\s+(.+)'],
    },
    handler: async (matches, message) => {
        // matches[0] = texto completo del regex
        // matches[1] = primer grupo capturado (el texto del usuario)
        const text = matches[1];

        // Tu logica aqui
        const resultado = procesar(text);

        // Devolver respuesta para el chat
        return { message: resultado };
    },
});
```

### Ejecutar acciones

```js
Neeko.actions.register('mi-accion', async (params) => {
    // params contiene los datos de la accion
    return 'Resultado de la accion';
});
```

### UI: Speech Bubble

```js
// Mostrar mensaje en el burbuja de Neeko
Neeko.ui.showBubble('Hola desde mi addon!');

// Controlar animaciones
Neeko.ui.setTalking(true);
Neeko.ui.setThinking(true);
```

### UI: Settings Tab

```js
Neeko.ui.registerSettingsTab('mi-addon-config', 'Mi Addon', `
    <div>
        <h4>Configuracion de Mi Addon</h4>
        <label>
            API Key:
            <input type="text" id="mi-api-key" placeholder="Tu API key" />
        </label>
        <button id="mi-save-btn">Guardar</button>
    </div>
`);

// Para acceder a los elementos despues de registrar el tab:
setTimeout(() => {
    document.getElementById('mi-save-btn')?.addEventListener('click', () => {
        const key = document.getElementById('mi-api-key').value;
        // Guardar la key...
        Neeko.ui.showBubble('API key guardada!');
    });
}, 100);
```

### UI: Modal

```js
const modal = Neeko.ui.createModal({
    title: 'Confirmar',
    content: '<p>Estas seguro?</p>',
    buttons: [
        { text: 'Si', onClick: () => { /* accion */ } },
        { text: 'No', onClick: () => { modal.close(); } },
    ]
});
```

### Backend: Invocar comandos Rust

```js
// Llamar a cualquier comando de Tauri
const result = await Neeko.invoke('get_local_ip');
console.log(result);

// Ejemplo: obtener config
const config = await Neeko.config.get();
console.log(config.lol_region);

// Ejemplo: guardar config
await Neeko.config.set({ language: 'en' });
```

### Chat: Hooks

```js
// Ejecutar antes de cada mensaje del usuario
Neeko.chat.onBeforeMessage(async (message) => {
    if (message.includes('bloquear')) {
        return { cancel: true, response: 'Mensaje bloqueado!' };
    }
    return {}; // No cancelar
});

// Ejecutar despues de cada respuesta de la IA
Neeko.chat.onAfterMessage(async (message, response) => {
    // Modificar la respuesta si queres
    return response;
});

// Obtener historial de chat
const historial = Neeko.chat.getHistory();
```

### LLM: Consultar la IA

```js
const respuesta = await Neeko.llm.chat([
    { role: 'user', content: 'Cual es la capital de Francia?' }
]);
console.log(respuesta);
```

### HTTP: Peticiones web

```js
const data = await fetch('https://api.example.com/data');
const json = await data.json();
```

### Almacenamiento persistente (localStorage)

```js
// Guardar datos (se mantienen entre sesiones)
localStorage.setItem('mi-addon-datos', JSON.stringify({ key: 'value' }));

// Leer datos
const datos = JSON.parse(localStorage.getItem('mi-addon-datos') || '{}');
```

### Eventos

```js
// Escuchar eventos de Tauri
Neeko.events.on('neeko:addons-loaded', () => {
    console.log('Todos los addons cargados');
});

// Emitir eventos
Neeko.events.emit('mi-addon:evento', { dato: 'valor' });
```

### Metadata del addon

```js
console.log(Neeko.addon.id);      // 'mi-addon'
console.log(Neeko.addon.name);    // 'Mi Addon'
console.log(Neeko.addon.version); // '1.0.0'
```

---

## Ejemplo completo: Addon de Clima

### addon.json

```json
{
  "id": "weather",
  "name": "Weather",
  "version": "1.0.0",
  "description": "Muestra el clima de cualquier ciudad",
  "author": "Neeko",
  "minAppVersion": "1.2.0",
  "permissions": ["chat:commands", "ui:tabs", "ui:bubble"],
  "commands": [
    {
      "id": "weather",
      "description": "Consultar clima",
      "patterns": {
        "es": ["clima\\s+en\\s+(.+)", "tiempo\\s+en\\s+(.+)"],
        "en": ["weather\\s+in\\s+(.+)", "temperature\\s+in\\s+(.+)"]
      }
    }
  ]
}
```

### main.js

```js
(function() {
    const API_KEY = localStorage.getItem('weather-api-key') || '';

    Neeko.commands.register('weather', {
        patterns: {
            es: ['clima\\s+en\\s+(.+)', 'tiempo\\s+en\\s+(.+)'],
            en: ['weather\\s+in\\s+(.+)', 'temperature\\s+in\\s+(.+)'],
        },
        handler: async (matches) => {
            if (!API_KEY) {
                return { message: 'Configura tu API key en Configuracion > Weather.' };
            }
            const city = matches[1].trim();
            try {
                const resp = await fetch(
                    `https://api.openweathermap.org/data/2.5/weather?q=${city}&appid=${API_KEY}&units=metric`
                );
                const data = await resp.json();
                const temp = data.main?.temp;
                const desc = data.weather?.[0]?.description;
                return { message: `${city}: ${temp}C, ${desc}` };
            } catch (e) {
                return { message: 'Error consultando el clima.' };
            }
        },
    });

    Neeko.ui.registerSettingsTab('weather-config', 'Weather', `
        <div>
            <h4>Weather API Key</h4>
            <p style="color:#aab;font-size:12px;">
                Obtene tu API key gratis en
                <a href="https://openweathermap.org/api" target="_blank" style="color:#c0a0ff;">openweathermap.org</a>
            </p>
            <input type="text" id="weather-api-key" placeholder="Tu API key"
                   value="${API_KEY}"
                   style="width:100%;padding:6px;border-radius:4px;border:1px solid #444;background:#1a1a25;color:#e0e0ff;margin:8px 0;" />
            <button id="weather-save-btn"
                    style="padding:6px 12px;border-radius:4px;border:1px solid #646;background:#1a1a25;color:#c0a0ff;cursor:pointer;">
                Guardar
            </button>
        </div>
    `);

    setTimeout(() => {
        document.getElementById('weather-save-btn')?.addEventListener('click', () => {
            const key = document.getElementById('weather-api-key').value;
            localStorage.setItem('weather-api-key', key);
            Neeko.ui.showBubble('API key guardada!');
        });
    }, 200);

})();
```

---

## Ejemplo: Addon con Python (Avanzado)

Los addons en Python se ejecutan como subprocesos separados. Comunicate con ellos via HTTP local.

### Estructura

```
addons/
  mi-addon-python/
    addon.json
    main.py
```

### main.py

```python
from http.server import HTTPServer, BaseHTTPRequestHandler
import json, re

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length)) if length else {}

        message = body.get("message", "")
        match = re.search(r"analizar\s+(.+)", message, re.IGNORECASE)

        if match:
            text = match.group(1)
            # Tu logica de procesamiento
            result = f"Analisis: {text.upper()}"
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"ok": True, "message": result}).encode())
        else:
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"ok": False}).encode())

    def log_message(self, *args):
        pass  # Silenciar logs

if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]
    print(f"Addon corriendo en puerto {port}", flush=True)
    server.serve_forever()
```

---

## Instalar un addon

1. Crea una carpeta en `%APPDATA%/neeko-assistant/addons/` con el nombre de tu addon
2. Copia los archivos (`addon.json`, `main.js`, `styles.css`)
3. Abre Neeko y ve a **Configuracion > Addons**
4. Habilita tu addon
5. Reinicia la app

---

## Desarrollo rapido

1. Copia la carpeta `_template/` y renombrala
2. Edita `addon.json` con tu metadata
3. Escribe tu logica en `main.js`
4. Reinicia Neeko para probar
5. Los cambios en `main.js` se aplican al reiniciar

### Tips

- Usa `Neeko.ui.showBubble()` para feedback inmediato
- Usa `localStorage` para guardar datos persistentes
- Los regex se prueban contra el texto en **lowercase**
- Pon el addon en una carpeta que empiece con `_` para que no se cargue (ej: `_template/`)
- Revisa la consola del navegador (F12) para errores

---

## Solucion de problemas

| Problema | Solucion |
|----------|----------|
| Addon no aparece | Verifica que `addon.json` sea JSON valido |
| Comando no funciona | Revisa que el regex sea correcto y que el id coincida |
| JS no carga | Abre consola (F12) y busca errores |
| Tab no aparece | Asegurate de tener permiso `ui:tabs` |
| Error de permisos | Revisa que `permissions` este en el manifest |

---

## Estructura del proyecto Neeko

```
neeko-assistant/
  addons/                    # Carpeta de addons
    quick-notes/             # Addon de ejemplo (viene con la app)
    _template/               # Template para nuevos addons
    mi-addon/                # Tu addon aqui
  src/
    main.js                  # Frontend - aqui se cargan los addons
    index.html               # UI del desktop
    styles.css               # Estilos
  src-tauri/
    src/
      addon_manager.rs       # Sistema de addons (Rust)
      lib.rs                 # Backend
```
