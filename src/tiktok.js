const { invoke } = window.__TAURI__.core;
const appWindow = window.__TAURI__.window.getCurrentWindow();
const path = document.getElementById('video-path');
const pick = document.getElementById('pick-file-btn');
const prepare = document.getElementById('compress-btn');
const folder = document.getElementById('clear-btn');
const original = document.getElementById('original-btn');
const engineSelect = document.getElementById('engine-select');
const optionsPanel = document.getElementById('engine-options');
let engines = [];
const rememberedOptions = new Map();
const engineGuidance = {
    bastien: {
        use: 'Para experimentar con el método de ghost frames en un MP4 H.264 que ya tenga la calidad y los FPS que querés conservar.',
        change: 'Conserva las imágenes codificadas y agrega cuadros ficticios declarados ×10. No genera FPS reales.',
        limit: 'En el video que probamos, Windows dejó de reconocer la imagen. Elegilo si aceptás esa incompatibilidad para comparar la subida con el original.'
    },
    upload120: {
        use: 'Para comparar parches del contenedor sin volver a comprimir la imagen. Elegí el modo según cuánto quieras modificar los tiempos.',
        change: 'No recodifica. Los cambios de metadata y reproducción dependen del modo seleccionado.',
        limit: 'No tenemos una comparación publicada que demuestre que sus etiquetas mejoren la calidad final en TikTok.'
    },
    utoku: {
        use: 'Para probar el método clásico de tiempos con un video de entrada de exactamente 60 o 120 FPS.',
        change: 'Divide campos de tiempo y duración sin recodificar los cuadros.',
        limit: 'Puede alterar la velocidad o sincronización local. No es la opción indicada si necesitás una copia de reproducción normal.'
    },
    luisalves: {
        use: 'Para experimentar con el parche de timescale en un video de más de 30 FPS, manteniendo los datos de imagen.',
        change: 'Aplica el factor 30/FPS a los tiempos de película y pistas; no recodifica.',
        limit: 'Puede ralentizar video y audio. Los FPS elegidos deben coincidir con los del archivo de entrada.'
    },
    paschafps: {
        use: 'Para probar el método FFmpeg de escala temporal con una entrada de 60, 120 o 240 FPS.',
        change: 'Copia video y audio y escala sus marcas de tiempo. No recodifica la imagen.',
        limit: 'La salida puede verse lenta o defectuosa localmente. El modo 240 FPS es experimental; seleccionarlo no crea cuadros nuevos.'
    },
    mistic: {
        use: 'Para probar una nueva codificación H.265 con un preset de tamaño y bitrate, seguida del parche del proyecto.',
        change: 'Recodifica a 60 FPS y ajusta la resolución al preset. Puede cambiar detalle, tamaño y calidad antes de la subida.',
        limit: 'Tarda más y requiere un encoder HEVC disponible. Si querés conservar la imagen codificada original, elegí un motor sin recodificación.'
    }
};
const uploadModeGuidance = {
    'extension-signal': 'Web Signal: para empezar por el modo que conserva los tiempos normales. Agrega metadata y reemplaza o crea la lista de edición a velocidad ×1; no convierte 60 FPS en 120. Una lista de edición preexistente puede cambiar.',
    'balanced-sync': 'Balanced Sync: para comparar un cambio de tiempos después de probar Web Signal. También agrega una lista de edición; revisá duración y sincronización.',
    'classic-force': 'Classic Force: para probar el parche temporal antiguo sin la lista de edición compensatoria. Puede producir reproducción lenta o extraña.'
};
function renderGuidance() {
    const engine = selectedEngine();
    const guide = engineGuidance[engine.id];
    document.getElementById('engine-use').textContent = guide.use;
    document.getElementById('engine-change').textContent = guide.change;
    document.getElementById('engine-limit').textContent = guide.limit;
    const mode = document.getElementById('engine-mode-guide');
    mode.hidden = engine.id !== 'upload120';
    mode.textContent = engine.id === 'upload120' ? uploadModeGuidance[readOptions().method] : '';
}
function selectedEngine() { return engines.find(engine => engine.id === engineSelect.value); }
function readOptions() {
    return Object.fromEntries([...optionsPanel.querySelectorAll('select')].map(input => [input.dataset.field, input.value]));
}
function renderEngine() {
    const engine = selectedEngine();
    document.getElementById('engine-description').textContent = engine.description;
    document.getElementById('engine-requirements').textContent = `Requiere: ${engine.requirements}. Formatos: ${engine.extensions.join(', ')}.`;
    optionsPanel.replaceChildren();
    const previous = rememberedOptions.get(engine.id) || {};
    for (const field of engine.fields) {
        const label = document.createElement('label');
        label.textContent = field.label;
        const select = document.createElement('select');
        select.dataset.field = field.id;
        select.setAttribute('aria-label', field.label);
        for (const [value, name] of field.choices) select.add(new Option(name, value));
        select.value = previous[field.id] || field.default;
        select.onchange = () => {
            rememberedOptions.set(engine.id, readOptions());
            renderGuidance();
        };
        label.append(select);
        optionsPanel.append(label);
    }
    renderGuidance();
}
engineSelect.onchange = renderEngine;
async function loadEngines() {
    try {
        engines = await invoke('tiktok_engines');
        engineSelect.replaceChildren();
        for (const engine of engines) engineSelect.add(new Option(engine.name, engine.id));
        renderEngine();
        engineSelect.disabled = prepare.disabled = false;
    } catch (error) { status('error', 'No pude cargar los motores', String(error)); }
}
function status(kind, title, detail) {
    document.getElementById('status').className = `status ${kind}`;
    document.getElementById('status-title').textContent = title;
    document.getElementById('status-detail').textContent = detail;
}
document.getElementById('minimize-btn').onclick = () => appWindow.minimize();
document.getElementById('close-btn').onclick = () => invoke('close_window');
pick.onclick = async () => {
    try {
        const selected = await invoke('pick_video_file');
        if (selected) path.value = selected;
    } catch (error) { status('error', 'No pude elegir el video', String(error)); }
};
folder.onclick = async () => {
    try { await invoke('open_tiktok_output_folder'); }
    catch (error) { status('error', 'No pude abrir la carpeta', String(error)); }
};
original.onclick = async () => {
    try { await invoke('open_tiktok_original'); }
    catch (error) { status('error', 'No pude abrir el original', String(error)); }
};
prepare.onclick = async () => {
    if (!path.value.trim()) {
        status('error', 'Falta el video', 'Elegí un archivo compatible con el motor seleccionado.');
        return;
    }
    prepare.disabled = pick.disabled = path.disabled = true;
    engineSelect.disabled = true;
    optionsPanel.querySelectorAll('select').forEach(input => { input.disabled = true; });
    folder.disabled = true;
    original.disabled = true;
    prepare.textContent = 'Preparando…';
    const engine = selectedEngine();
    const options = readOptions();
    status('working', 'Preparando video', `${engine.name} está procesando el archivo. Podés minimizar esta ventana.`);
    try {
        const result = await invoke('prepare_tiktok', { input: path.value, engine: engine.id, options });
        if (result.windows_video === false) {
            status('warning', 'Copia creada · Windows no reconoce la imagen',
                `${engine.name} generó el archivo, pero el reproductor de Windows no reconoce su pista de video. Usá «Ver original» para revisarlo. La reproducción y la aceptación en TikTok no están verificadas.\nGuardado en: ${result.path}`);
        } else if (result.windows_video == null) {
            status('warning', 'Copia creada · Reproducción sin comprobar',
                `No pude comprobar si Windows reconoce la imagen. Usá «Ver original» para revisar el video. La aceptación en TikTok no está verificada.\nGuardado en: ${result.path}`);
        } else {
            status('success', 'Copia creada para TikTok', `Guardado en: ${result.path}\nLa reproducción y la aceptación en TikTok no están verificadas.`);
        }
        folder.disabled = false;
        original.disabled = false;
    } catch (error) { status('error', 'No pude preparar el video', String(error)); }
    finally {
        prepare.disabled = pick.disabled = path.disabled = false;
        engineSelect.disabled = false;
        optionsPanel.querySelectorAll('select').forEach(input => { input.disabled = false; });
        prepare.textContent = 'Preparar para TikTok';
    }
};
loadEngines();
