(() => {
    if (Neeko.ui.isSettingsWindow) {
        const panel = document.createElement('div');
        panel.className = 'airi-preferences';
        panel.innerHTML = `<h4>Neeko sobre el escritorio</h4><div class="airi-size-control">
            <label for="airi-settings-size">Tamaño de Neeko <output></output></label>
            <input id="airi-settings-size" type="range" min="30" max="100" step="1">
            <button type="button">Restablecer tamaño</button></div>
            <p>Arrastrá a Neeko para moverla. Configuración y chat tienen sus propias ventanas.</p>`;
        const input = panel.querySelector('input');
        const update = value => {
            input.value = String(value);
            panel.querySelector('output').textContent = `${value}%`;
            localStorage.setItem('neeko-airi-size', String(value));
        };
        update(Math.max(30, Math.min(100, Number(localStorage.getItem('neeko-airi-size')) || 82)));
        input.addEventListener('input', () => update(input.value));
        panel.querySelector('button').addEventListener('click', () => update(82));
        Neeko.ui.registerSettingsTab('airi-appearance', 'Escritorio', panel);
        return;
    }
    const root = document.documentElement;
    const sprite = document.getElementById('neeko-sprite');
    if (!sprite || root.classList.contains('airi-pet')) return;
    const win = window.__TAURI__?.window?.getCurrentWindow();
    const listeners = new AbortController();
    const options = { signal: listeners.signal };
    const bubble = document.getElementById('speech-bubble');
    const confirmation = document.getElementById('action-confirmation');
    const settings = document.getElementById('settings-modal');
    const original = ['role', 'tabindex', 'aria-label', 'aria-expanded'].map(name => [name, sprite.getAttribute(name)]);
    let disposed = false, pointer;
    let openingChat = false;
    let dragged = false;
    const sizeKey = 'neeko-airi-size';
    const previousSize = root.style.getPropertyValue('--airi-pet-size');
    const previousSizePriority = root.style.getPropertyPriority('--airi-pet-size');
    let size = 82;
    try {
        const saved = Number(localStorage.getItem(sizeKey));
        if (Number.isFinite(saved) && saved >= 30 && saved <= 100) size = saved;
    } catch (_) {}
    const sizeControls = [];
    window.addEventListener('storage', event => {
        if (event.key === sizeKey) applySize(event.newValue);
    }, options);
    function applySize(value) {
        size = Math.max(30, Math.min(100, Number(value) || 82));
        root.style.setProperty('--airi-pet-size', `${size}%`);
        for (const control of sizeControls) {
            control.querySelector('input').value = String(size);
            control.querySelector('output').textContent = `${size}%`;
        }
        try { localStorage.setItem(sizeKey, String(size)); } catch (_) {}
    }
    function createSizeControl(id) {
        const control = document.createElement('div');
        control.className = 'airi-size-control';
        control.innerHTML = `<label for="${id}">Tamaño de Neeko <output for="${id}"></output></label>
            <input id="${id}" type="range" min="30" max="100" step="1" aria-label="Tamaño de Neeko">
            <button type="button">Restablecer tamaño</button>`;
        control.querySelector('input').addEventListener('input', event => applySize(event.target.value), options);
        control.querySelector('button').addEventListener('click', () => applySize(82), options);
        sizeControls.push(control);
        return control;
    }
    const menu = document.createElement('div');
    menu.id = 'airi-pet-menu';
    menu.hidden = true;
    menu.setAttribute('role', 'group');
    menu.setAttribute('aria-label', 'Controles de Neeko');
    menu.innerHTML = `<button type="button" data-action="chat">Hablar con Neeko</button>
        <button type="button" data-action="settings">Configuración</button>
        <button type="button" data-action="minimize">Minimizar</button>
        <button type="button" data-action="close">Cerrar</button>`;
    document.body.appendChild(menu);
    window.addEventListener('blur', () => hideMenu(), options);
    menu.appendChild(createSizeControl('airi-size-menu'));
    function hideMenu() { ++menuRequest; menu.hidden = true; }
    async function openChat() {
        hideMenu();
        if (openingChat || disposed) return;
        openingChat = true;
        try { await Neeko.invoke('open_chat_window'); }
        catch (error) {
            console.error('[AIRI] No se pudo abrir el chat:', error);
            if (!disposed) {
                const notice = document.createElement('p');
                notice.textContent = 'No se pudo abrir el chat. Reiniciá Neeko con la versión nueva.';
                notice.style.cssText = 'color:#f5f3ff;padding:8px;font-size:12px';
                menu.querySelector('p')?.remove();
                menu.appendChild(notice);
                showMenu(16, 16);
            }
        } finally { openingChat = false; }
    }
    let menuRequest = 0;
    async function showMenu(x, y) {
        const request = ++menuRequest;
        let left = 0, top = 0, right = window.innerWidth, bottom = window.innerHeight;
        try {
            if (win) {
                const [monitor, origin, scale] = await Promise.all([
                    window.__TAURI__.window.currentMonitor(), win.innerPosition(), win.scaleFactor()
                ]);
                if (monitor) {
                    const area = monitor.workArea || { position: monitor.position, size: monitor.size };
                    left = Math.max(left, (area.position.x - origin.x) / scale);
                    top = Math.max(top, (area.position.y - origin.y) / scale);
                    right = Math.min(right, (area.position.x + area.size.width - origin.x) / scale);
                    bottom = Math.min(bottom, (area.position.y + area.size.height - origin.y) / scale);
                }
            }
        } catch (error) { console.error('[AIRI] Menu bounds:', error); }
        if (disposed || request !== menuRequest) return;
        // Fit the menu into the visible portion of the pet window, without moving the pet.
        menu.style.maxWidth = `${Math.max(1, right - left - 16)}px`;
        menu.style.maxHeight = `${Math.max(1, bottom - top - 16)}px`;
        menu.hidden = false;
        menu.style.left = `${Math.max(left + 8, Math.min(x, right - menu.offsetWidth - 8))}px`;
        menu.style.top = `${Math.max(top + 8, Math.min(y, bottom - menu.offsetHeight - 8))}px`;
        menu.querySelector('button').focus();
    }
    const observer = new MutationObserver(() => {
        root.classList.toggle('airi-confirming', !confirmation.hidden);
    });
    observer.observe(bubble, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['class', 'hidden'] });
    const settingsObserver = new MutationObserver(() => {
        if (!settings.classList.contains('hidden')) {
            hideMenu();
        }
    });
    settingsObserver.observe(settings, { attributes: true, attributeFilter: ['class'] });
    sprite.setAttribute('role', 'button');
    sprite.setAttribute('tabindex', '0');
    sprite.setAttribute('aria-label', 'Neeko: clic para hablar, arrastrar para mover, clic derecho para controles');
    sprite.addEventListener('pointerdown', event => {
        if (event.button !== 0) return;
        pointer = { x: event.clientX, y: event.clientY };
        dragged = false;
    }, options);
    sprite.addEventListener('pointermove', event => {
        if (!pointer || !(event.buttons & 1)) return;
        if (Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) < 6) return;
        pointer = null;
        dragged = true;
        hideMenu();
        win?.startDragging().catch(error => console.error('[AIRI] No se pudo mover Neeko:', error));
    }, options);
    window.addEventListener('pointerup', () => { pointer = null; }, options);
    window.addEventListener('pointercancel', () => { pointer = null; }, options);
    sprite.addEventListener('dragstart', event => event.preventDefault(), options);
    sprite.addEventListener('click', () => {
        if (!dragged) void openChat();
        dragged = false;
    }, options);
    sprite.addEventListener('keydown', event => {
        if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            void openChat();
        }
        if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
            event.preventDefault();
            const rect = sprite.getBoundingClientRect();
            showMenu(rect.left + 20, rect.top + 20);
        }
    }, options);
    document.getElementById('app').addEventListener('contextmenu', event => {
        event.preventDefault();
        showMenu(event.clientX, event.clientY);
    }, options);
    menu.addEventListener('click', event => {
        const action = event.target.closest('button')?.dataset.action;
        if (!action) return;
        hideMenu();
        if (action === 'chat') void openChat();
        else document.getElementById({ settings: 'settings-btn', minimize: 'minimize-btn', close: 'close-btn' }[action])?.click();
    }, options);
    document.addEventListener('pointerdown', event => {
        if (!menu.contains(event.target)) hideMenu();
    }, options);
    document.addEventListener('keydown', event => {
        if (event.key === 'Escape' && settings.classList.contains('hidden')) {
            hideMenu();
            sprite.focus();
        }
    }, options);
    Neeko.addon.onUnload(() => {
        Neeko.ui.setDesktopHitRegion?.(false);
        disposed = true;
        listeners.abort();
        observer.disconnect();
        settingsObserver.disconnect();
        menu.remove();
        if (previousSize) root.style.setProperty('--airi-pet-size', previousSize, previousSizePriority);
        else root.style.removeProperty('--airi-pet-size');
        root.classList.remove('airi-pet', 'airi-chat-open', 'airi-reply-visible', 'airi-confirming');
        for (const [name, value] of original) {
            if (value === null) sprite.removeAttribute(name);
            else sprite.setAttribute(name, value);
        }
        win?.setAlwaysOnTop(false).catch(console.error);
    });
    const preferences = document.createElement('div');
    preferences.className = 'airi-preferences';
    preferences.innerHTML = `
        <h4>Neeko sobre el escritorio</h4>
        <p>Clic en Neeko: abrir el chat en otra ventana. Arrastrar: mover el personaje.</p>
        <p>Clic derecho: configuración, minimizar o cerrar. Escape: cerrar el menú.</p>
        <p>Desactivá AIRI en Addons para recuperar la interfaz normal.</p>
        <p>La transparencia requiere reiniciar la versión de Neeko que incluye este modo.</p>`;
    preferences.prepend(createSizeControl('airi-size-settings'));
    Neeko.ui.registerSettingsTab('airi-appearance', 'Escritorio', preferences);
    applySize(size);
    root.classList.add('airi-pet');
    Neeko.ui.setDesktopHitRegion?.(true);
    root.classList.toggle('airi-confirming', !confirmation.hidden);
    if (win) void (async () => {
        try {
            if (disposed) return;
            await win.setAlwaysOnTop(true);
            if (disposed) await win.setAlwaysOnTop(false);
        } catch (error) { console.error('[AIRI] No se pudo ajustar la ventana:', error); }
    })();
})();
