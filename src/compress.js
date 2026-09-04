const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const appWindow = getCurrentWindow();
const pathInput = document.getElementById('video-path');
const targetInput = document.getElementById('target-size');
const bitrateInput = document.getElementById('video-bitrate');
const pickFileBtn = document.getElementById('pick-file-btn');
const compressBtn = document.getElementById('compress-btn');
const clearBtn = document.getElementById('clear-btn');
const statusBox = document.getElementById('status');
const statusTitle = document.getElementById('status-title');
const statusDetail = document.getElementById('status-detail');
const presetButtons = document.querySelectorAll('.preset');

function setStatus(kind, title, detail) {
    statusBox.className = `status ${kind}`;
    statusTitle.textContent = title;
    statusDetail.textContent = detail;
}

function setBusy(busy) {
    compressBtn.disabled = busy;
    pickFileBtn.disabled = busy;
    clearBtn.disabled = busy;
    compressBtn.textContent = busy ? 'Comprimiendo...' : 'Comprimir';
}

function readNumber(input) {
    const value = Number.parseInt(input.value, 10);
    return Number.isFinite(value) && value > 0 ? value : null;
}

function setActivePreset(target) {
    presetButtons.forEach((button) => {
        button.classList.toggle('active', button.dataset.target === String(target ?? ''));
    });
}

function applyOptions(options = {}) {
    if (typeof options.input === 'string' && options.input.trim()) {
        pathInput.value = options.input.trim();
    }
    if (Number.isFinite(options.targetSizeMb)) {
        targetInput.value = options.targetSizeMb;
        setActivePreset(options.targetSizeMb);
    }
    if (Number.isFinite(options.videoBitrateKbps)) {
        bitrateInput.value = options.videoBitrateKbps;
    }
}

window.setCompressorOptionsFromNeeko = applyOptions;
window.setCompressorInputFromNeeko = (input) => applyOptions({ input });

document.getElementById('minimize-btn').addEventListener('click', () => appWindow.minimize());
document.getElementById('close-btn').addEventListener('click', () => invoke('close_window'));

pickFileBtn.addEventListener('click', async () => {
    try {
        const path = await invoke('pick_video_file');
        if (path) pathInput.value = path;
    } catch (error) {
        setStatus('error', 'No pude elegir archivo', String(error));
    }
});

presetButtons.forEach((button) => {
    button.addEventListener('click', () => {
        presetButtons.forEach((item) => item.classList.remove('active'));
        button.classList.add('active');
        targetInput.value = button.dataset.target;
    });
});

targetInput.addEventListener('input', () => setActivePreset(readNumber(targetInput)));

clearBtn.addEventListener('click', () => {
    pathInput.value = '';
    targetInput.value = '8';
    bitrateInput.value = '';
    setActivePreset(8);
    setStatus('idle', 'Listo', 'Elegí un archivo y ajustá el tamaño máximo o el bitrate.');
});

compressBtn.addEventListener('click', async () => {
    const input = pathInput.value.trim();
    const targetSizeMb = readNumber(targetInput);
    const videoBitrateKbps = readNumber(bitrateInput);

    if (!input) {
        setStatus('error', 'Falta el archivo', 'Elegí un video o pegá la ruta completa.');
        return;
    }
    if (!targetSizeMb && !videoBitrateKbps) {
        setStatus('error', 'Falta un ajuste', 'Indicá un tamaño máximo o un bitrate de video.');
        return;
    }

    setBusy(true);
    setStatus('working', 'Comprimiendo', 'Neeko está procesando el video con FFmpeg. Esto puede tardar un poco.');

    try {
        const message = await invoke('compress_for_discord', {
            input,
            targetSizeMb,
            videoBitrateKbps,
        });
        setStatus('success', 'Video comprimido', message);
    } catch (error) {
        setStatus('error', 'No pude comprimir', String(error));
    } finally {
        setBusy(false);
    }
});

const params = new URLSearchParams(window.location.search);
applyOptions({
    input: params.get('input') || '',
    targetSizeMb: params.has('targetSizeMb') ? Number.parseInt(params.get('targetSizeMb'), 10) : 8,
    videoBitrateKbps: params.has('videoBitrateKbps') ? Number.parseInt(params.get('videoBitrateKbps'), 10) : null,
});
