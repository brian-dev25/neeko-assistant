(() => {
    const invoke = window.__TAURI__.core.invoke;
    let latest = null;
    function render(state) {
        if (!state) return;
        latest = state;
        const running = !!state.active;
        const paused = state.paused;
        document.getElementById('toggle').textContent = running ? '⏹' : '▶';
        document.getElementById('toggle').classList.toggle('active', running);
        document.getElementById('once').disabled = running || !state.prepared || !state.settings.region;
        document.getElementById('pause').textContent = paused ? '▶' : '⏸';
        document.getElementById('pause').classList.toggle('active', paused);
        document.getElementById('pause').disabled = !running;
        document.getElementById('search').disabled = running;
        document.getElementById('lock').textContent = state.locked ? '🔒' : '🔓';
        document.getElementById('lock').classList.toggle('active', state.locked);
        document.getElementById('lock').disabled = !running;
        document.getElementById('overlay').classList.toggle('active', state.phase !== 'idle');
        document.getElementById('status').textContent = state.phase === 'idle' ? 'OK' :
            state.phase === 'capturing' ? 'OCR…' :
            state.phase === 'translating' ? 'Trad…' :
            state.phase === 'waiting' ? 'Espera' :
            state.phase === 'error' ? 'Error' :
            state.phase === 'selecting' ? 'Select' :
            state.phase === 'preparing' ? 'Prep' : state.phase;
        document.getElementById('status').title = state.message;
    }
    function call(action) {
        return invoke('screen_translate', { action, settings: null, profiles: null }).then(render).catch(e => {
            document.getElementById('status').textContent = 'Error';
            document.getElementById('status').title = String(e);
        });
    }
    document.getElementById('toggle').addEventListener('click', () => {
        if (!latest) return call('status').then(s => { if (!s.active) call('start'); });
        call(latest.active ? 'stop' : 'start');
    });
    document.getElementById('once').addEventListener('click', () => call('once'));
    document.getElementById('pause').addEventListener('click', () => call('pause'));
    document.getElementById('search').addEventListener('click', () => {
        if (latest?.settings) {
            const settings = { ...latest.settings };
            invoke('screen_translate', { action: 'select', settings, profiles: null }).then(render).catch(e => {
                document.getElementById('status').textContent = 'Error';
                document.getElementById('status').title = String(e);
            });
        }
    });
    document.getElementById('lock').addEventListener('click', () => call('lock'));
    document.getElementById('overlay').addEventListener('click', () => call('toggle-overlay'));
    document.querySelector('[data-drag]').addEventListener('pointerdown', e => {
        if (e.button === 0) window.__TAURI__.window.getCurrentWindow().startDragging().catch(() => {});
    });
    document.addEventListener('keydown', e => {
        if (e.key === 'Escape') call('stop');
    });
    (async () => {
        await window.__TAURI__.event.listen('screen-translate:status', ({ payload }) => render(payload));
        render(await invoke('screen_translate', { action: 'status', settings: null, profiles: null }));
    })().catch(() => {});
})();
