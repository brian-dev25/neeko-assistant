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
        <p role="status" aria-live="polite" data-status>Listo para escuchar.</p>
        <h4>Historial de canciones</h4>
        <label for="shazam-history-limit">Cantidad máxima de canciones</label>
        <div class="shazam-actions"><input id="shazam-history-limit" type="number" min="1" max="1000" step="1" value="15" required>
        <button type="button" data-save-limit>Aplicar límite</button></div>
        <p>Por defecto: 15. Al reducir el límite se conservan las más recientes y se borran las anteriores.</p>
        <ul class="shazam-history" data-history></ul>`;
    const listen = panel.querySelector('[data-listen]');
    const prepare = panel.querySelector('[data-prepare]');
    const cancel = panel.querySelector('[data-cancel]');
    const status = panel.querySelector('[data-status]');
    let disposed = false;
    let busy = false;
    let preparing = false;
    let unlisten;
    const limitInput = panel.querySelector('#shazam-history-limit');
    const saveLimit = panel.querySelector('[data-save-limit]');
    Neeko.invoke('shazam_history_limit').then(limit => { if (!disposed) limitInput.value = limit; }).catch(error => { status.textContent = String(error); });
    saveLimit.addEventListener('click', async () => {
        if (!limitInput.reportValidity()) return;
        saveLimit.disabled = true;
        try {
            await Neeko.invoke('shazam_set_history_limit', { limit: Number(limitInput.value) });
            await refreshHistory();
            status.textContent = 'Límite de canciones guardado.';
        } catch (error) { status.textContent = String(error); }
        finally { saveLimit.disabled = false; }
    });
    async function refreshHistory() {
        try {
            const songs = await Neeko.invoke('shazam_history');
            if (disposed) return;
            const list = panel.querySelector('[data-history]');
            list.replaceChildren();
            if (!songs.length) {
                const empty = document.createElement('li');
                empty.textContent = 'Todavía no hay canciones reconocidas.';
                list.appendChild(empty);
            }
            for (const song of songs) {
                const row = document.createElement('li');
                const title = document.createElement('strong');
                title.textContent = song.title;
                const detail = document.createElement('span');
                detail.textContent = `${song.artist} · ${new Date(song.found_at).toLocaleString()}`;
                const remove = document.createElement('button');
                remove.type = 'button';
                remove.textContent = 'Borrar';
                remove.setAttribute('aria-label', `Borrar ${song.title} del historial`);
                remove.addEventListener('click', async () => {
                    remove.disabled = true;
                    try { await Neeko.invoke('shazam_history_delete', { id: song.id }); await refreshHistory(); }
                    catch (error) { status.textContent = String(error); remove.disabled = false; }
                });
                row.append(title, detail, remove);
                list.appendChild(row);
            }
        } catch (error) { if (!disposed) status.textContent = String(error); }
    }
    void refreshHistory();
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
            if (!disposed) {
                status.textContent = result.message + (result.history_error ? ` (No se pudo guardar el historial: ${result.history_error})` : '');
                await refreshHistory();
            }
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
