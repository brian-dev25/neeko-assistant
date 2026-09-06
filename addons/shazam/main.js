(() => {
    const panel = document.createElement('div');
    panel.className = 'shazam-panel';
    panel.innerHTML = `<h4>Shazam</h4>
        <p>Primera vez: prepará Shazam para instalar Python si falta y las dependencias de reconocimiento.</p>
        <div class="shazam-actions"><button type="button" data-prepare>Preparar Shazam</button></div>
        <p>Reconocé la música que suena en la salida de audio predeterminada de Windows.</p>
        <p>Al pulsar Escuchar se capturan 10 segundos del escritorio y se consulta Shazam por Internet. No usa el micrófono.</p>
        <div class="shazam-actions"><button type="button" data-listen>♫ Escuchar</button>
        <button type="button" data-cancel hidden>Cancelar</button></div>
        <p role="status" aria-live="polite" data-status>Listo para escuchar.</p>`;
    const listen = panel.querySelector('[data-listen]');
    const prepare = panel.querySelector('[data-prepare]');
    const cancel = panel.querySelector('[data-cancel]');
    const status = panel.querySelector('[data-status]');
    let disposed = false;
    let busy = false;
    let preparing = false;
    let unlisten;
    Neeko.events.on('shazam-setup-progress', event => {
        if (!disposed && preparing) status.textContent = event.payload;
    }).then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(console.error);
    prepare.addEventListener('click', async () => {
        if (busy || preparing) return;
        preparing = true;
        prepare.disabled = listen.disabled = true;
        status.textContent = 'Preparando Shazam…';
        try {
            const message = await Neeko.invoke('shazam_prepare');
            if (!disposed) status.textContent = message;
        } catch (error) {
            if (!disposed) status.textContent = String(error);
        } finally {
            preparing = false;
            if (!disposed) prepare.disabled = listen.disabled = false;
        }
    });
    listen.addEventListener('click', async () => {
        if (busy || preparing) return;
        busy = true;
        listen.disabled = true;
        prepare.disabled = true;
        cancel.hidden = false;
        status.textContent = 'Escuchando el escritorio durante 10 segundos…';
        const timer = setTimeout(() => {
            if (!disposed) status.textContent = 'Buscando la canción…';
        }, 10000);
        try {
            const result = await Neeko.invoke('shazam_listen');
            if (!disposed) status.textContent = result.message;
        } catch (error) {
            if (!disposed) status.textContent = String(error);
        } finally {
            clearTimeout(timer);
            busy = false;
            if (!disposed) { prepare.disabled = listen.disabled = false; cancel.hidden = true; }
        }
    });
    cancel.addEventListener('click', () => {
        status.textContent = 'Cancelando…';
        void Neeko.invoke('shazam_cancel').catch(error => { status.textContent = String(error); });
    });
    Neeko.ui.registerSettingsTab('shazam', 'Shazam', panel);
    Neeko.addon.onUnload(() => {
        disposed = true;
        unlisten?.();
        if (busy) void Neeko.invoke('shazam_cancel').catch(console.error);
    });
})();
