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
            <label>Salida hacia micrófono virtual<select name="outputDevice"><option value="">Detectar VB-CABLE</option></select></label>
            <label>Teléfono USB<select name="usbSerial"><option value="">Único teléfono autorizado</option></select></label>
            <div class="phone-microphone-actions"><button data-action="devices">Actualizar dispositivos</button><button data-action="usb-devices">Detectar USB</button></div>
        </fieldset>
        <p data-address>&nbsp;</p>
        <p data-device>&nbsp;</p>
        <label>Nivel de micrófono<meter data-level min="0" max="100" value="0" aria-label="Nivel de micrófono"></meter></label>
        <p data-metrics>Sample rate: se configura en Android; se recomienda 48 kHz mono. Procesamiento: 48 kHz.</p>
        <fieldset data-dsp><legend>Audio</legend>
            <label>Ganancia <output data-gain>0 dB</output><input name="gain" type="range" min="-50" max="50" step="1" value="0"></label>
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
    let disposed = false, busy = false, busyTimer = null;
    let unsubscribe;

    function error(err) {
        if (disposed) return;
        $('[data-error]').hidden = false;
        $('[data-error]').textContent = String(err);
        console.error('[NEEKO Phone Microphone]', err);
    }

    function setBusy(v) {
        busy = v;
        if (busyTimer) { clearTimeout(busyTimer); busyTimer = null; }
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
    function syncButtons(s) {
        if (!s || disposed) return;
        const hasDevice = !!s.device;
        $('[data-action="start"]').disabled = s.running || busy;
        $('[data-action="stop"]').disabled = !s.running || busy;
        $('[data-action="connect"]').disabled = !hasDevice || s.connected || busy;
        $('[data-action="disconnect"]').disabled = !hasDevice || busy;
        $('[data-connection]').disabled = s.running || busy;
    }

    function render(s) {
        if (disposed || !s) return;
        lastState = s;
        $('[data-status]').textContent = s.message || '';
        $('[data-device]').textContent = s.device
            ? `${s.device.name} \u00B7 ${s.device.ip} \u00B7 ${s.connected ? 'Conectado' : 'Detectado; esperando aprobaci\u00F3n'}`
            : 'Sin tel\u00E9fono';
        $('[data-level]').value = s.level || 0;
        $('[data-address]').textContent = s.running ? `${s.settings.bindAddress}:${s.settings.port} \u00B7 UDP ${s.settings.port + 1}` : '';
        if (s.metrics) $('[data-metrics]').textContent = `${s.metrics.sampleRate} Hz \u00B7 Latencia ${s.metrics.networkLatencyMs} ms \u00B7 P\u00E9rdida ${s.metrics.packetLossRate.toFixed(1)} % \u00B7 DSP 48 kHz`;
        if (s.aec) $('[data-aec]').textContent = s.aec.available ? 'AEC local con referencia del audio de Windows.' : `AEC no disponible: ${s.aec.reason || 'sin referencia de audio'}`;
        controls.forEach(input => {
            const v = s.settings[input.name];
            if (v === undefined || v === null) return;
            if (input.type === 'checkbox') input.checked = Boolean(v);
            else {
                if (input.tagName === 'SELECT' && v && ![...input.options].some(o => o.value === v)) input.add(new Option(v, v));
                input.value = v;
            }
        });
        $('[data-gain]').textContent = `${$('[name="gain"]').value} dB`;
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
    }

    async function execute(action) {
        if (busy || disposed) return;
        if (!$('[name="port"]').reportValidity()) return;
        setBusy(true);
        $('[data-error]').hidden = true;
        try {
            if (action === 'devices') await loadDevices();
            else if (action === 'usb-devices') {
                const list = await call(action);
                if (!disposed) fillOptions($('[name="usbSerial"]'), list, d => `${d.description} (${d.state})`, d => d.serial);
            } else {
                const result = await call(action, ['start', 'settings'].includes(action) ? read() : null);
                render(result);
            }
        } catch (err) { error(err); }
        finally {
            setBusy(false);
            if (!disposed) {
                try { render(await call('status')); } catch (err) { error(err); }
            }
        }
    }

    panel.querySelectorAll('[data-action]').forEach(btn => btn.addEventListener('click', () => execute(btn.dataset.action)));
    controls.forEach(input => input.addEventListener('change', () => { if (!busy) execute('settings'); }));
    $('[name="gain"]').addEventListener('input', e => { $('[data-gain]').textContent = `${e.target.value} dB`; });
    Neeko.ui.registerSettingsTab('phone-microphone', 'Phone Microphone', panel);
    Neeko.addon.onUnload(() => { disposed = true; if (busyTimer) clearTimeout(busyTimer); unsubscribe?.(); });

    let eventThrottle = 0;
    (async () => {
        try {
            const off = await Neeko.events.on('phone-microphone:status', ({ payload }) => {
                if (disposed) return;
                const now = Date.now();
                const isLevelOnly = payload.level !== undefined && payload.level !== 0
                    && payload.running === lastState.running && payload.connected === lastState.connected
                    && !!payload.device === !!lastState.device;
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
