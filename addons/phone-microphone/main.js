(() => {
    const panel = document.createElement('div');
    panel.className = 'phone-microphone-panel';
    panel.innerHTML = `<h4>Phone Microphone</h4>
        <p>Usá el cliente oficial MicYou en Android. El audio se procesa en esta PC.</p>
        <p>Instalá VB-CABLE. En Discord/OBS elegí <strong>CABLE Output</strong> como entrada de micrófono.</p>
        <fieldset data-connection><legend>Conexión</legend>
            <label>Modo<select name="mode"><option value="wifi">Wi-Fi / LAN</option><option value="usb">USB / ADB</option></select></label>
            <label>Dirección de esta PC<select name="bindAddress"><option value="">Automática (LAN privada)</option></select></label>
            <label>Puerto TCP<input name="port" type="number" min="1024" max="65534" value="9123"></label>
            <label data-output-device-row>Salida hacia micrófono virtual<select name="outputDevice"><option value="">Detectar VB-CABLE</option></select></label>
            <label>Teléfono USB<select name="usbSerial"><option value="">Único teléfono autorizado</option></select></label>
            <div class="phone-microphone-actions"><button data-action="devices">Actualizar dispositivos</button><button data-action="usb-devices">Detectar USB</button></div>
        </fieldset>
        <p data-address>&nbsp;</p>
        <p data-device>&nbsp;</p>
        <label>Nivel de micrófono<meter data-level min="0" max="100" value="0" aria-label="Nivel de micrófono"></meter></label>
        <p data-metrics>Sample rate: se configura en Android; se recomienda 48 kHz mono. Procesamiento: 48 kHz.</p>
        <fieldset data-dsp><legend>Audio</legend>
            <label>Volumen de entrada <output data-gain>100 % · 0 dB</output><input name="gain" type="range" min="-50" max="50" step="1" value="0"></label>
            <label>Modo de entrada<select name="inputMode"><option value="voice">Actividad de voz</option><option value="ptt">Pulsar para hablar</option></select></label>
            <div data-sensitivity-panel>
                <label class="phone-microphone-check"><input name="sensitivityEnabled" type="checkbox">Usar umbral de sensibilidad</label>
                <label>Sensibilidad de entrada <output data-threshold>-40 dB</output>
                    <span class="phone-microphone-sensitivity"><span data-input-fill></span><input name="sensitivity" type="range" min="-100" max="0" step="1" value="-40" aria-label="Umbral de sensibilidad de entrada"></span>
                </label>
                <p>Amarillo: por debajo del umbral. Verde: zona de voz. La barra muestra la entrada antes del procesamiento.</p>
            </div>
            <label>Tecla para hablar<select name="talkKey"></select></label>
            <label>Atajo para activar/desactivar mute<select name="muteKey"><option value="">Sin atajo</option></select></label>
            <p>Los atajos funcionan con el servidor iniciado, incluso con Neeko en segundo plano. Mute tiene prioridad sobre pulsar para hablar.</p>
            <p data-talk-state role="status"></p>
            <label class="phone-microphone-check"><input name="noiseSuppression" type="checkbox">Supresión de ruido (RNNoise)</label>
            <label class="phone-microphone-check"><input name="echoCancellation" type="checkbox" disabled>Cancelación de eco (AEC)</label>
            <p data-aec>&nbsp;</p>
            <label class="phone-microphone-check"><input name="automaticGain" type="checkbox">Ganancia automática</label>
            <label class="phone-microphone-check"><input name="autoReconnect" type="checkbox" checked>Reconectar automáticamente el teléfono aprobado durante esta sesión</label>
            <label class="phone-microphone-check"><input name="muted" type="checkbox">Mute / Silenciar</label>
        </fieldset>
        <div class="phone-microphone-actions">
            <button data-action="start">Iniciar servidor</button><button data-action="stop" disabled>Detener</button>
            <button data-action="connect" disabled>Conectar audio</button><button data-action="disconnect" disabled>Desconectar</button>
        </div>
        <p data-status role="status" aria-live="polite">Esperando servidor…</p>
        <p data-error role="alert" hidden></p>
        <p>Wi-Fi: misma red, seleccioná esta PC en MicYou o ingresá la IP y puerto mostrados. USB: activá depuración USB, autorizá la PC y usá modo USB/TCP en MicYou. Android inicia la conexión; luego pulsá Conectar audio aquí.</p>`;
    const $ = s => panel.querySelector(s);
    const controls = [...panel.querySelectorAll('[name]')];
    const keys = ['Space', ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789', ...Array.from({length: 24}, (_, i) => `F${i + 1}`)];
    for (const name of ['talkKey', 'muteKey']) {
        for (const prefix of ['', 'Ctrl+', 'Shift+', 'Alt+', 'Ctrl+Shift+']) {
            for (const key of keys) $(`[name="${name}"]`).add(new Option(prefix + key, prefix + key));
        }
    }
    let disposed = false, busy = false, initialized = false;
    let unsubscribe;

    function error(err) {
        if (disposed) return;
        $('[data-error]').hidden = false;
        $('[data-error]').textContent = String(err);
        console.error('[NEEKO Phone Microphone]', err);
    }

    function setBusy(v) {
        busy = v;
        syncButtons(lastState);
        $('[data-dsp]').disabled = v || !initialized;
    }

    function audioLabels() {
        const gain = Number($('[name="gain"]').value);
        const percent = 100 * 10 ** (gain / 20);
        $('[data-gain]').textContent = `${percent < 1 ? percent.toFixed(1) : Math.round(percent)} % · ${gain > 0 ? '+' : ''}${gain} dB`;
        const threshold = Number($('[name="sensitivity"]').value);
        $('[data-threshold]').textContent = `${threshold} dB`;
        $('.phone-microphone-sensitivity').style.setProperty('--threshold', `${threshold + 100}%`);
        $('[data-sensitivity-panel]').hidden = $('[name="inputMode"]').value === 'ptt';
        $('[name="sensitivity"]').disabled = !$('[name="sensitivityEnabled"]').checked;
    }

    function read() {
        return Object.fromEntries(controls.map(input => [input.name,
            input.type === 'checkbox' ? input.checked : ['number', 'range'].includes(input.type) ? Number(input.value) : input.value]));
    }

    function fillOptions(select, values, labelFn = v => v, valueFn = v => v) {
        const prev = select.value;
        while (select.options.length > 1) select.remove(1);
        values.forEach(item => select.add(new Option(labelFn(item), valueFn(item))));
        select.value = prev;
        if (select.selectedIndex < 0) select.selectedIndex = 0;
    }

    let lastState = {};
    let lastSettingsJson = '';
    function syncButtons(s) {
        if (!s || disposed) return;
        const hasDevice = !!s.device;
        $('[data-action="start"]').disabled = !initialized || s.running || busy;
        $('[data-action="stop"]').disabled = !s.running || busy;
        $('[data-action="connect"]').disabled = !hasDevice || s.connected || busy;
        $('[data-action="disconnect"]').disabled = !hasDevice || busy;
        $('[data-connection]').disabled = s.running || busy;
    }

    function render(s, syncSettings = false) {
        if (disposed || !s) return;
        initialized = true;
        lastState = s;
        $('[data-status]').textContent = s.message || '';
        $('[data-device]').textContent = s.device
            ? `${s.device.name} \u00B7 ${s.device.ip} \u00B7 ${s.connected ? 'Conectado' : 'Detectado; esperando aprobaci\u00F3n'}`
            : 'Sin tel\u00E9fono';
        $('[data-level]').value = s.level || 0;
        $('[data-input-fill]').style.width = `${Math.max(0, Math.min(100, (s.inputDb ?? -100) + 100))}%`;
        $('[data-talk-state]').textContent = s.settings.muted ? 'Silenciado (PC / Android)' : !s.connected ? 'Audio sin conectar' : s.settings.inputMode === 'ptt' ? (s.talkPressed ? 'Tecla presionada · transmitiendo' : 'Mantené la tecla para hablar') : 'Actividad de voz';
        $('[data-address]').textContent = s.running ? `${s.settings.bindAddress}:${s.settings.port} \u00B7 UDP ${s.settings.port + 1}` : '';
        if (s.metrics) $('[data-metrics]').textContent = `${s.metrics.sampleRate} Hz \u00B7 Latencia ${s.metrics.networkLatencyMs} ms \u00B7 P\u00E9rdida ${s.metrics.packetLossRate.toFixed(1)} % \u00B7 DSP 48 kHz`;
        if (s.aec) $('[data-aec]').textContent = s.aec.available ? 'AEC local con referencia del audio de Windows.' : `AEC no disponible: ${s.aec.reason || 'sin referencia de audio'}`;
        // Audio events must never write over a slider/selection the user is editing.
        const settingsJson = JSON.stringify(s.settings);
        if (syncSettings || (!busy && settingsJson !== lastSettingsJson)) {
        controls.forEach(input => {
            const v = s.settings[input.name];
            if (v === undefined || v === null) return;
            if (!syncSettings && input === document.activeElement && input.name !== 'muted') return;
            if (input.type === 'checkbox') input.checked = Boolean(v);
            else {
                if (input.tagName === 'SELECT' && v && ![...input.options].some(o => o.value === v)) input.add(new Option(v, v));
                input.value = v;
            }
        });
        lastSettingsJson = settingsJson;
        }
        audioLabels();
        syncButtons(s);
    }

    async function call(action, settings) {
        return Neeko.invoke('phone_microphone', { action, settings: settings ?? null });
    }

    async function loadDevices() {
        const data = await call('devices');
        if (disposed) return;
        fillOptions($('[name="outputDevice"]'), data.outputs);
        fillOptions($('[name="bindAddress"]'), data.interfaces, v => `${v.ip} (${v.interface_name})`, v => v.ip);
        $('[name="echoCancellation"]').disabled = !data.aecAvailable;
        $('[data-aec]').textContent = data.aecAvailable ? 'AEC local con referencia del audio de Windows.' : 'AEC requiere el modelo AEC7 y onnxruntime.dll.';
        syncButtons(lastState);
    }

    async function execute(action) {
        if (busy || disposed || !initialized) return;
        if (!$('[name="port"]').reportValidity()) return;
        const draft = ['start', 'settings', 'mute'].includes(action) ? read() : null;
        setBusy(true);
        $('[data-error]').hidden = true;
        $('[data-status]').textContent = ({start: 'Iniciando servidor…', connect: 'Conectando audio…', stop: 'Deteniendo servidor…', disconnect: 'Desconectando…', settings: 'Guardando audio…'})[action] || 'Buscando dispositivos…';
        try {
            if (action === 'devices') await loadDevices();
            else if (action === 'usb-devices') {
                const list = await call(action);
                if (!disposed) fillOptions($('[name="usbSerial"]'), list, d => `${d.description} (${d.state})`, d => d.serial);
            } else {
                const result = await call(action, draft);
                render(result, true);
            }
        } catch (err) { error(err); }
        finally {
            setBusy(false);
            if (!disposed) {
                try { render(await call('status'), true); } catch (err) { error(err); }
            }
        }
    }

    panel.querySelectorAll('[data-action]').forEach(btn => btn.addEventListener('click', () => execute(btn.dataset.action)));
    controls.forEach(input => input.addEventListener('change', () => { if (!busy) execute(input.name === 'muted' ? 'mute' : 'settings'); }));
    for (const name of ['gain', 'sensitivity', 'inputMode']) $('[name="' + name + '"]').addEventListener('input', audioLabels);
    Neeko.ui.registerSettingsTab('phone-microphone', 'Phone Microphone', panel);
    let polling = false;
    Neeko.addon.onUnload(() => { disposed = true; unsubscribe?.(); });

    let eventThrottle = 0;
    (async () => {
        try {
            const off = await Neeko.events.on('phone-microphone:status', ({ payload }) => {
                if (disposed) return;
                const now = Date.now();
                const isLevelOnly = payload.level !== undefined && payload.level !== 0
                    && payload.running === lastState.running && payload.connected === lastState.connected
                    && !!payload.device === !!lastState.device
                    && payload.talkPressed === lastState.talkPressed
                    && JSON.stringify(payload.settings) === JSON.stringify(lastState.settings);
                if (isLevelOnly && now - eventThrottle < 200) return;
                eventThrottle = now;
                render(payload);
            });
            if (disposed) { off?.(); return; }
            unsubscribe = off;
            render(await call('status'));
            await loadDevices();
        } catch (err) { error(err); }
    })();
})();
