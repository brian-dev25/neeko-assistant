// Discord Rich Presence Addon

(function() {
    const STORAGE_KEY = 'discord-rich-presence-config';
    const DEFAULT_CONFIG = {
        appId: '1543987086439219290',
        details: 'Neeko Assistant',
        state: 'Hablando con Neeko',
        largeImage: 'icon',
        largeText: 'Neeko Assistant',
        smallImage: 'icon',
        smallText: 'Neeko',
        buttonLabel: '',
        buttonUrl: '',
        buttonLabel2: '',
        buttonUrl2: '',
        autoStart: false,
    };

    const TEXT = {
        es: {
            tab: 'Discord Rich',
            appId: 'Application ID',
            details: 'Detalles',
            state: 'Estado',
            largeImage: 'Imagen grande',
            largeText: 'Texto imagen grande',
            smallImage: 'Imagen chica',
            smallText: 'Texto imagen chica',
            buttonLabel: 'Boton 1',
            buttonUrl: 'URL boton 1',
            buttonLabel2: 'Boton 2',
            buttonUrl2: 'URL boton 2',
            autoStart: 'Activar al cargar addon',
            assetHint: 'Usa "icon" como asset por defecto. Subi el icono en Discord Developer Portal con ese nombre.',
            buttonHint: 'Discord oculta los botones en tu propio perfil. Para probarlos, miralo desde otra cuenta o pedile a alguien que vea tu perfil.',
            invalidButtonUrl: 'El boton necesita una URL http:// o https://.',
            placeholders: {
                appId: 'ID de tu app de Discord',
                details: 'Neeko Assistant',
                state: 'Hablando con Neeko',
                largeImage: 'icon',
                largeText: 'Neeko Assistant',
                smallImage: 'icon',
                smallText: 'Neeko',
                buttonLabel: 'Repositorio',
                buttonUrl: 'https://...',
                buttonLabel2: 'Abrir Neeko',
                buttonUrl2: 'https://...',
            },
            save: 'Guardar',
            start: 'Iniciar',
            stop: 'Detener',
            clear: 'Limpiar',
            status: 'Estado',
            reset: 'Reset',
            missingAppId: 'Configura el Application ID de Discord primero.',
            saved: 'Discord Rich Presence guardado.',
            started: 'Discord Rich Presence activado.',
            stopped: 'Discord Rich Presence detenido.',
            cleared: 'Actividad limpiada.',
            running: 'Discord Rich Presence esta activo.',
            stoppedStatus: 'Discord Rich Presence esta apagado.',
            buttonSelfHidden: 'Los botones solo los ven otras cuentas.',
            error: 'Error Discord Rich Presence:',
        },
        en: {
            tab: 'Discord Rich',
            appId: 'Application ID',
            details: 'Details',
            state: 'State',
            largeImage: 'Large image',
            largeText: 'Large image text',
            smallImage: 'Small image',
            smallText: 'Small image text',
            buttonLabel: 'Button 1',
            buttonUrl: 'Button 1 URL',
            buttonLabel2: 'Button 2',
            buttonUrl2: 'Button 2 URL',
            autoStart: 'Start when addon loads',
            assetHint: 'Uses "icon" as the default asset. Upload the app icon in Discord Developer Portal with that name.',
            buttonHint: 'Discord hides buttons on your own Rich Presence. Test them from another account or ask someone else to view your profile.',
            invalidButtonUrl: 'The button needs an http:// or https:// URL.',
            placeholders: {
                appId: 'Your Discord app ID',
                details: 'Neeko Assistant',
                state: 'Talking with Neeko',
                largeImage: 'icon',
                largeText: 'Neeko Assistant',
                smallImage: 'icon',
                smallText: 'Neeko',
                buttonLabel: 'Repository',
                buttonUrl: 'https://...',
                buttonLabel2: 'Open Neeko',
                buttonUrl2: 'https://...',
            },
            save: 'Save',
            start: 'Start',
            stop: 'Stop',
            clear: 'Clear',
            status: 'Status',
            reset: 'Reset',
            missingAppId: 'Configure the Discord Application ID first.',
            saved: 'Discord Rich Presence saved.',
            started: 'Discord Rich Presence started.',
            stopped: 'Discord Rich Presence stopped.',
            cleared: 'Activity cleared.',
            running: 'Discord Rich Presence is running.',
            stoppedStatus: 'Discord Rich Presence is off.',
            buttonSelfHidden: 'Buttons are only visible to other accounts.',
            error: 'Discord Rich Presence error:',
        },
    };

    function lang() {
        return document.documentElement.lang === 'en' ? 'en' : 'es';
    }

    function t(key) {
        return TEXT[lang()][key] || TEXT.es[key] || key;
    }

    function ph(key) {
        return TEXT[lang()].placeholders[key] || TEXT.es.placeholders[key] || '';
    }

    function loadConfig() {
        try {
            return { ...DEFAULT_CONFIG, ...JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}') };
        } catch {
            return { ...DEFAULT_CONFIG };
        }
    }

    function saveConfig(config) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
    }

    let saveTimer = null;

    function queueSaveFromForm() {
        saveConfig(getFormConfig());
        clearTimeout(saveTimer);
        saveTimer = setTimeout(async () => {
            const config = getFormConfig();
            try {
                if (await isRunning()) {
                    await updateActivity(config);
                }
            } catch (error) {
                console.error('[DRPC] autosave update error:', error);
            }
        }, 250);
    }

    function getFormConfig() {
        const valueOf = (id) => document.getElementById(id)?.value.trim() ?? '';
        return {
            appId: valueOf('drpc-app-id'),
            details: valueOf('drpc-details'),
            state: valueOf('drpc-state'),
            largeImage: valueOf('drpc-large-image'),
            largeText: valueOf('drpc-large-text'),
            smallImage: valueOf('drpc-small-image'),
            smallText: valueOf('drpc-small-text'),
            buttonLabel: valueOf('drpc-button-label'),
            buttonUrl: valueOf('drpc-button-url'),
            buttonLabel2: valueOf('drpc-button-label-2'),
            buttonUrl2: valueOf('drpc-button-url-2'),
            autoStart: !!document.getElementById('drpc-auto-start')?.checked,
        };
    }

    function isValidButtonUrl(value) {
        try {
            const url = new URL(value);
            return url.protocol === 'http:' || url.protocol === 'https:';
        } catch {
            return false;
        }
    }

    function getButtonsFromConfig(config) {
        return [
            { label: config.buttonLabel, url: config.buttonUrl },
            { label: config.buttonLabel2, url: config.buttonUrl2 },
        ].filter((button) => button.label && button.url && isValidButtonUrl(button.url)).slice(0, 2);
    }

    function hasInvalidButtonUrl(config) {
        return [config.buttonUrl, config.buttonUrl2].some((url) => url && !isValidButtonUrl(url));
    }

    function activityFromConfig(config) {
        const activity = {
            details: config.details,
            state: config.state,
            timestamps: { start: Date.now() },
            activity_type: 0,
        };

        const assets = {};
        if (config.largeImage) assets.large_image = config.largeImage;
        if (config.largeText) assets.large_text = config.largeText;
        if (config.smallImage) assets.small_image = config.smallImage;
        if (config.smallText) assets.small_text = config.smallText;
        if (Object.keys(assets).length) activity.assets = assets;

        const buttons = getButtonsFromConfig(config);
        if (buttons.length) {
            activity.buttons = buttons;
        }

        return activity;
    }

    async function updateActivity(config) {
        await Neeko.invoke('plugin:drpc|set_activity', {
            activityJson: JSON.stringify(activityFromConfig(config)),
        });
    }

    async function isRunning() {
        return await Neeko.invoke('plugin:drpc|is_running');
    }

    async function stopPresence() {
        try {
            if (await isRunning()) {
                await Neeko.invoke('plugin:drpc|clear_activity');
                await Neeko.invoke('plugin:drpc|destroy_thread');
            }
        } catch (error) {
            console.error('[DRPC] stop error:', error);
        }
    }

    async function startPresence(config = loadConfig()) {
        if (!config.appId) {
            throw new Error(t('missingAppId'));
        }
        if (hasInvalidButtonUrl(config)) {
            throw new Error(t('invalidButtonUrl'));
        }

        await stopPresence();
        await Neeko.invoke('plugin:drpc|spawn_thread', { id: config.appId });
        await updateActivity(config);
    }

    async function clearPresence() {
        if (await isRunning()) {
            await Neeko.invoke('plugin:drpc|clear_activity');
        }
    }

    function show(message) {
        Neeko.ui.showBubble(message);
    }

    function renderSettingsHtml() {
        const config = loadConfig();
        return `
            <div class="drpc-panel">
                <label>${t('appId')}</label>
                <input id="drpc-app-id" type="text" value="${escapeHtml(config.appId)}" placeholder="${escapeHtml(ph('appId'))}">

                <label>${t('details')}</label>
                <input id="drpc-details" type="text" value="${escapeHtml(config.details)}" placeholder="${escapeHtml(ph('details'))}">

                <label>${t('state')}</label>
                <input id="drpc-state" type="text" value="${escapeHtml(config.state)}" placeholder="${escapeHtml(ph('state'))}">

                <label>${t('largeImage')}</label>
                <input id="drpc-large-image" type="text" value="${escapeHtml(config.largeImage)}" placeholder="${escapeHtml(ph('largeImage'))}">

                <label>${t('largeText')}</label>
                <input id="drpc-large-text" type="text" value="${escapeHtml(config.largeText)}" placeholder="${escapeHtml(ph('largeText'))}">

                <label>${t('smallImage')}</label>
                <input id="drpc-small-image" type="text" value="${escapeHtml(config.smallImage)}" placeholder="${escapeHtml(ph('smallImage'))}">

                <label>${t('smallText')}</label>
                <input id="drpc-small-text" type="text" value="${escapeHtml(config.smallText)}" placeholder="${escapeHtml(ph('smallText'))}">

                <p class="drpc-hint">${t('assetHint')}</p>

                <label>${t('buttonLabel')}</label>
                <input id="drpc-button-label" type="text" value="${escapeHtml(config.buttonLabel)}" placeholder="${escapeHtml(ph('buttonLabel'))}">

                <label>${t('buttonUrl')}</label>
                <input id="drpc-button-url" type="text" value="${escapeHtml(config.buttonUrl)}" placeholder="${escapeHtml(ph('buttonUrl'))}">

                <label>${t('buttonLabel2')}</label>
                <input id="drpc-button-label-2" type="text" value="${escapeHtml(config.buttonLabel2)}" placeholder="${escapeHtml(ph('buttonLabel2'))}">

                <label>${t('buttonUrl2')}</label>
                <input id="drpc-button-url-2" type="text" value="${escapeHtml(config.buttonUrl2)}" placeholder="${escapeHtml(ph('buttonUrl2'))}">

                <p class="drpc-hint">${t('buttonHint')}</p>

                <label class="drpc-check">
                    <input id="drpc-auto-start" type="checkbox" ${config.autoStart ? 'checked' : ''}>
                    <span>${t('autoStart')}</span>
                </label>

                <div class="drpc-actions">
                    <button id="drpc-start" type="button">${t('start')}</button>
                    <button id="drpc-stop" type="button">${t('stop')}</button>
                    <button id="drpc-clear" type="button">${t('clear')}</button>
                    <button id="drpc-status" type="button">${t('status')}</button>
                    <button id="drpc-reset" type="button">${t('reset')}</button>
                </div>
            </div>
        `;
    }

    function bindSettingsEvents() {
        document.querySelectorAll('.drpc-panel input').forEach((input) => {
            input.addEventListener('input', queueSaveFromForm);
            input.addEventListener('change', queueSaveFromForm);
            input.addEventListener('blur', queueSaveFromForm);
        });
        document.getElementById('drpc-start')?.addEventListener('click', async () => {
            try {
                const config = getFormConfig();
                saveConfig(config);
                await startPresence(config);
                show(getButtonsFromConfig(config).length ? `${t('started')} ${t('buttonSelfHidden')}` : t('started'));
            } catch (error) {
                show(`${t('error')} ${error.message || error}`);
            }
        });
        document.getElementById('drpc-stop')?.addEventListener('click', async () => {
            await stopPresence();
            show(t('stopped'));
        });
        document.getElementById('drpc-clear')?.addEventListener('click', async () => {
            await clearPresence();
            show(t('cleared'));
        });
        document.getElementById('drpc-status')?.addEventListener('click', async () => {
            const config = getFormConfig();
            if (hasInvalidButtonUrl(config)) {
                show(t('invalidButtonUrl'));
                return;
            }
            show((await isRunning())
                ? `${t('running')} ${getButtonsFromConfig(config).length ? t('buttonSelfHidden') : ''}`.trim()
                : t('stoppedStatus'));
        });
        document.getElementById('drpc-reset')?.addEventListener('click', () => {
            saveConfig({ ...DEFAULT_CONFIG });
            renderSettings();
            show(t('saved'));
        });
    }

    function renderSettings() {
        const panel = document.querySelector('[data-settings-panel="discord-rich-presence"]');
        if (!panel) return;
        panel.innerHTML = renderSettingsHtml();
        bindSettingsEvents();
    }

    function syncLanguage() {
        const tab = document.querySelector('[data-settings-tab="discord-rich-presence"]');
        if (tab) tab.textContent = t('tab');
        renderSettings();
    }

    function escapeHtml(value) {
        return String(value || '')
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    Neeko.commands.register('drpc-start', {
        patterns: {
            es: ['discord\\s+rich\\s+(?:on|activar|iniciar)', 'activar\\s+discord\\s+rich', 'iniciar\\s+discord\\s+rich'],
            en: ['discord\\s+rich\\s+(?:on|start|enable)', 'start\\s+discord\\s+rich', 'enable\\s+discord\\s+rich'],
        },
        handler: async () => {
            try {
                await startPresence();
                return { message: t('started') };
            } catch (error) {
                return { message: `${t('error')} ${error.message || error}` };
            }
        },
    });

    Neeko.commands.register('drpc-stop', {
        patterns: {
            es: ['discord\\s+rich\\s+(?:off|apagar|desactivar|detener)', 'apagar\\s+discord\\s+rich', 'desactivar\\s+discord\\s+rich'],
            en: ['discord\\s+rich\\s+(?:off|stop|disable)', 'stop\\s+discord\\s+rich', 'disable\\s+discord\\s+rich'],
        },
        handler: async () => {
            await stopPresence();
            return { message: t('stopped') };
        },
    });

    Neeko.commands.register('drpc-status', {
        patterns: {
            es: ['estado\\s+discord\\s+rich', 'discord\\s+rich\\s+estado'],
            en: ['discord\\s+rich\\s+status', 'status\\s+discord\\s+rich'],
        },
        handler: async () => ({ message: (await isRunning()) ? t('running') : t('stoppedStatus') }),
    });

    Neeko.commands.register('drpc-set-state', {
        patterns: {
            es: ['discord\\s+rich\\s+texto\\s+(.+)', 'discord\\s+rich\\s+estado\\s+(.+)'],
            en: ['discord\\s+rich\\s+text\\s+(.+)', 'discord\\s+rich\\s+state\\s+(.+)'],
        },
        handler: async (matches) => {
            const text = (matches[1] || '').trim();
            const config = { ...loadConfig(), state: text || DEFAULT_CONFIG.state };
            saveConfig(config);
            if (await isRunning()) {
                await updateActivity(config);
            }
            renderSettings();
            return { message: t('saved') };
        },
    });

    Neeko.ui.registerSettingsTab('discord-rich-presence', t('tab'), renderSettingsHtml());
    bindSettingsEvents();

    const languageObserver = new MutationObserver(syncLanguage);
    languageObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['lang'],
    });

    const config = loadConfig();
    if (config.autoStart) {
        startPresence(config).catch((error) => console.error('[DRPC] auto-start error:', error));
    }

    Neeko.addon.onUnload(() => {
        clearTimeout(saveTimer);
        languageObserver.disconnect();
        stopPresence();
    });
})();
