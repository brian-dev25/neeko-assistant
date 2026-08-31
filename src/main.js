let THREE = null;
let GLTFLoaderClass = null;

try {
    THREE = await import('three');
    const gltfMod = await import('three/addons/loaders/GLTFLoader.js');
    GLTFLoaderClass = gltfMod.GLTFLoader;
} catch (e) {
    console.warn('Three.js no se pudo cargar, 3D deshabilitado:', e);
}

const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const appWindow = getCurrentWindow();

const neekoSection = document.getElementById('neeko-section');
const neekoSprite = document.getElementById('neeko-sprite');
const neekoImg = document.getElementById('neeko-img');
const neeko3d = document.getElementById('neeko-3d');
const speechBubble = document.getElementById('speech-bubble');
const bubbleText = document.getElementById('bubble-text');
const chatInput = document.getElementById('chat-input');
const sendBtn = document.getElementById('send-btn');
const minimizeBtn = document.getElementById('minimize-btn');
const closeBtn = document.getElementById('close-btn');

let isProcessing = false;
let currentAbortController = null;
let localAiModelAvailable = false;
let currentModelLoadEngine = 'llama';
let currentModelRuntimeConfig = null;
let currentLanguage = 'es';
let settingsOriginalLanguage = 'es';
let memoryTabInitialized = false;
let neeko3dAnimationId = null;
let neeko3dRendered = false;
let neeko3dScene = null;
let neeko3dCamera = null;
let neeko3dRenderer = null;
let neeko3dClock = null;
let neeko3dMixer = null;
let neeko3dModel = null;
let neeko3dModelBaseY = 0;
let neeko3dActions = new Map();
let neeko3dActiveAction = null;
let neeko3dResizeObserver = null;
let neeko3dMouseTracking = true;
let neeko3dMouseX = 0;
let neeko3dMouseY = 0;
let neeko3dSelectedIdle = 'Neeko_idle3.anm';
let neeko3dIdleCleanup = null;
let neeko3dHeadBone = null;
let neeko3dNeckBone = null;
let neeko3dHeadTargetRotX = 0;
let neeko3dHeadTargetRotY = 0;
let neeko3dHeadCurrentRotX = 0;
let neeko3dHeadCurrentRotY = 0;
let _headOffsetQuat, _headSavedQuat, _headAxis, _headSideAxis;
if (THREE) {
    _headOffsetQuat = new THREE.Quaternion();
    _headSavedQuat = new THREE.Quaternion();
    _headAxis = new THREE.Vector3(0, 1, 0);
    _headSideAxis = new THREE.Vector3(1, 0, 0);
}

// ─── Addon System ───
const NeekoAddons = {
    _commands: new Map(),
    _actions: new Map(),
    _settingsTabs: new Map(),
    _chatHooks: { before: [], after: [] },
    _loaded: new Map(),
    _loadingAddonId: null,

    _ensureRecord(addonId) {
        if (!addonId) return null;
        if (!NeekoAddons._loaded.has(addonId)) {
            NeekoAddons._loaded.set(addonId, {
                commands: new Set(),
                actions: new Set(),
                tabs: new Set(),
                beforeHooks: [],
                afterHooks: [],
                disposers: [],
                style: null,
                script: null,
            });
        }
        return NeekoAddons._loaded.get(addonId);
    },

    _activeRecord() {
        return NeekoAddons._ensureRecord(NeekoAddons._loadingAddonId);
    },

    init() {
        window.Neeko = {
            commands: {
                register: (id, config) => {
                    NeekoAddons._commands.set(id, config);
                    NeekoAddons._activeRecord()?.commands.add(id);
                },
                unregister: (id) => {
                    NeekoAddons._commands.delete(id);
                },
                list: () => Array.from(NeekoAddons._commands.entries()).map(([id, c]) => ({ id, ...c })),
            },
            actions: {
                register: (name, handler) => {
                    NeekoAddons._actions.set(name, handler);
                    NeekoAddons._activeRecord()?.actions.add(name);
                },
                unregister: (name) => {
                    NeekoAddons._actions.delete(name);
                },
            },
            ui: {
                showBubble: (text) => showBubble(text),
                setTalking: (v) => setTalking(v),
                setThinking: (v) => setThinking(v),
                registerSettingsTab: (id, title, content) => NeekoAddons._registerTab(id, title, content),
                unregisterSettingsTab: (id) => NeekoAddons._removeTab(id),
            },
            invoke: invoke,
            config: {
                get: async () => JSON.parse(await invoke('lol_get_config')),
                set: async (partial) => await invoke('lol_save_config', partial),
            },
            events: {
                on: (event, cb) => window.__TAURI__?.event?.listen(event, cb),
                emit: (event, payload) => window.__TAURI__?.event?.emit(event, payload),
            },
            chat: {
                onBeforeMessage: (cb) => {
                    NeekoAddons._chatHooks.before.push(cb);
                    NeekoAddons._activeRecord()?.beforeHooks.push(cb);
                    return () => NeekoAddons._removeHook('before', cb);
                },
                onAfterMessage: (cb) => {
                    NeekoAddons._chatHooks.after.push(cb);
                    NeekoAddons._activeRecord()?.afterHooks.push(cb);
                    return () => NeekoAddons._removeHook('after', cb);
                },
                getHistory: () => [...conversationHistory],
                addSystemMessage: (text) => conversationHistory.push({ role: 'system', content: text }),
            },
            llm: {
                chat: async (messages) => {
                    const id = await invoke('chat_start', { messages });
                    return invoke('chat_finish', { requestId: id });
                },
            },
            addon: {
                id: null,
                name: null,
                version: null,
                onUnload: (cb) => {
                    if (typeof cb === 'function') NeekoAddons._activeRecord()?.disposers.push(cb);
                },
            },
        };
    },

    async loadAddons() {
        try {
            const addons = JSON.parse(JSON.stringify(await invoke('addon_list')));
            for (const addon of addons.filter((item) => item.enabled)) {
                await NeekoAddons.loadAddon(addon);
            }
            window.__TAURI__?.event?.emit('neeko:addons-loaded');
        } catch (e) {
            console.error('[NEEKO ADDON] Error cargando addons:', e);
        }
    },

    async loadAddon(addon) {
        const addonId = addon.manifest.id;
        if (NeekoAddons._loaded.has(addonId)) return;

        const record = NeekoAddons._ensureRecord(addonId);
        try {
            if (addon.has_css) {
                const css = await invoke('addon_get_css', { addonId });
                if (css) {
                    const style = document.createElement('style');
                    style.id = `neeko-addon-style-${addonId}`;
                    style.dataset.addonId = addonId;
                    style.textContent = css;
                    document.head.appendChild(style);
                    record.style = style;
                }
            }

            if (addon.has_js) {
                const js = await invoke('addon_get_js', { addonId });
                if (js) {
                    const script = document.createElement('script');
                    script.id = `neeko-addon-script-${addonId}`;
                    script.dataset.addonId = addonId;
                    NeekoAddons._loadingAddonId = addonId;
                    window.Neeko.addon.id = addonId;
                    window.Neeko.addon.name = addon.manifest.name;
                    window.Neeko.addon.version = addon.manifest.version;
                    script.textContent = `try {\n${js}\n} catch(e) { console.error('[NEEKO ADDON Error: ${addonId}]', e); }`;
                    document.body.appendChild(script);
                    record.script = script;
                }
            }
        } catch (e) {
            NeekoAddons.unloadAddon(addonId);
            console.error(`[NEEKO ADDON] Error cargando ${addonId}:`, e);
            throw e;
        } finally {
            NeekoAddons._loadingAddonId = null;
            window.Neeko.addon.id = null;
            window.Neeko.addon.name = null;
            window.Neeko.addon.version = null;
        }
    },

    unloadAddon(addonId) {
        const record = NeekoAddons._loaded.get(addonId);
        if (!record) return;

        record.commands.forEach((id) => NeekoAddons._commands.delete(id));
        record.actions.forEach((name) => NeekoAddons._actions.delete(name));
        record.tabs.forEach((id) => NeekoAddons._removeTab(id));
        record.beforeHooks.forEach((hook) => NeekoAddons._removeHook('before', hook));
        record.afterHooks.forEach((hook) => NeekoAddons._removeHook('after', hook));
        record.disposers.forEach((dispose) => {
            try {
                dispose();
            } catch (e) {
                console.error(`[NEEKO ADDON] Error descargando ${addonId}:`, e);
            }
        });
        record.style?.remove();
        record.script?.remove();
        NeekoAddons._loaded.delete(addonId);
    },

    _registerTab(id, title, content) {
        NeekoAddons._settingsTabs.set(id, { title, content });
        const existing = document.querySelector(`[data-settings-tab="${id}"]`);
        if (existing) return;
        NeekoAddons._activeRecord()?.tabs.add(id);

        const tabBtn = document.createElement('button');
        tabBtn.className = 'settings-tab';
        tabBtn.dataset.settingsTab = id;
        tabBtn.type = 'button';
        tabBtn.role = 'tab';
        tabBtn.setAttribute('aria-selected', 'false');
        tabBtn.textContent = title;
        tabBtn.addEventListener('click', () => setSettingsTab(id));
        document.querySelector('.settings-tabs')?.appendChild(tabBtn);
        settingsTabs.push(tabBtn);

        const panel = document.createElement('section');
        panel.id = `settings-tab-${id}`;
        panel.className = 'settings-section settings-panel';
        panel.dataset.settingsPanel = id;
        panel.role = 'tabpanel';
        panel.hidden = true;
        if (typeof content === 'string') panel.innerHTML = content;
        else if (content instanceof HTMLElement) panel.appendChild(content);
        document.querySelector('.settings-scroll')?.appendChild(panel);
        settingsPanels.push(panel);
    },

    _removeTab(id) {
        NeekoAddons._settingsTabs.delete(id);
        const tab = document.querySelector(`[data-settings-tab="${id}"]`);
        const panel = document.querySelector(`[data-settings-panel="${id}"]`);
        const wasActive = tab?.classList.contains('active') || panel?.classList.contains('active');
        tab?.remove();
        panel?.remove();
        const tabIndex = settingsTabs.findIndex((tab) => tab.dataset.settingsTab === id);
        if (tabIndex >= 0) settingsTabs.splice(tabIndex, 1);
        const panelIndex = settingsPanels.findIndex((panel) => panel.dataset.settingsPanel === id);
        if (panelIndex >= 0) settingsPanels.splice(panelIndex, 1);
        if (wasActive) setSettingsTab('addons');
    },

    _removeHook(type, hook) {
        const hooks = NeekoAddons._chatHooks[type];
        const index = hooks.indexOf(hook);
        if (index >= 0) hooks.splice(index, 1);
    },

    detectCommand(text) {
        const lower = text.toLowerCase().trim();
        for (const [id, cmd] of NeekoAddons._commands) {
            const patterns = cmd.patterns?.[currentLanguage] || cmd.patterns?.['es'] || [];
            for (const patStr of patterns) {
                try {
                    const match = lower.match(new RegExp(patStr, 'i'));
                    if (match) {
                        return {
                            action: { action: `addon:${id}`, addonId: id, matches: [...match], message: text },
                            message: '',
                        };
                    }
                } catch (e) {
                    console.error(`[NEEKO ADDON] Regex invalida en comando "${id}":`, e);
                }
            }
        }
        return null;
    },

    async executeAddonAction(action) {
        const normalizeAddonResult = (result) => {
            if (result == null) return null;
            if (typeof result === 'string') return result;
            if (typeof result.message === 'string') return result.message;
            if (typeof result.text === 'string') return result.text;
            try {
                return JSON.stringify(result);
            } catch {
                return String(result);
            }
        };

        const command = NeekoAddons._commands.get(action.addonId);
        if (command?.handler) {
            return normalizeAddonResult(await command.handler(action.matches || [], action.message || ''));
        }

        const handler = NeekoAddons._actions.get(action.addonId);
        if (handler) {
            return normalizeAddonResult(await handler(action));
        }
        return null;
    },
};

const SPRITES = {
    default: "NEEKO.png",
    standing: "NEEKO-standing-costume.png",
    sitting: "NEEKO-sitting.png",
};

const NEEKO_3D_ANIMATIONS = {
    idle: ['Idle1_Base', 'Idle2_Base', 'Neeko_idle3.anm'],
    thinking: 'Idlein_Animal',
    talking: 'Joke_Loop',
};

const NEEKO_3D_PORTRAIT = {
    centerX: 0,
    centerY: 5.9,
    viewHeight: 4,
};

const I18N = {
    es: {
        locale: 'es-AR',
        chatPlaceholder: 'Hablale a Neeko...',
        settingsTitle: 'Configuracion',
        checkTools: 'Probar',
        save: 'Guardar',
        cancel: 'Cancelar',
        tabGeneral: 'General',
        tabTools: 'Herramientas',
        tabAi: 'IA',
        tabSystem: 'Sistema',
        generalTitle: 'General',
        languageLabel: 'Idioma:',
        toolsTitle: 'Herramientas',
        downloadFfmpeg: 'Descargar FFmpeg + FFprobe',
        uninstallFfmpeg: 'Desinstalar FFmpeg + FFprobe',
        downloadGit: 'Descargar Git',
        uninstallGit: 'Desinstalar Git',
        fixedModel: 'Modelo fijo en Google Drive',
        downloadModel: 'Descargar modelo',
        openBrowser: 'Abrir en navegador',
        installFromFile: 'Instalar desde archivo',
        uninstallModel: 'Desinstalar modelo',
        downloadLabel: 'Descarga',
        ready: 'Listo',
        defaultGitPath: 'Ruta Git por defecto:',
        riotIdLabel: 'Riot ID (Usuario#Tag):',
        lolRegion: 'Region LOL:',
        aiAppearance: 'Apariencia de IA',
        neekoSprite: 'Sprite de Neeko:',
        render3d: 'Renderizar Neeko en 3D',
        animation3d: 'Animacion 3D:',
        mouseTracking: 'Seguir mouse',
        loadEngine: 'Motor de carga:',
        preparePython: 'Preparar motor Python',
        advanced: 'Avanzado',
        autostartLlama: 'Auto-iniciar LLaMA al abrir la app',
        backToAi: 'Volver a IA',
        explanation: 'Explicacion',
        gpuLayers: 'Capas GPU:',
        contextSize: 'Contexto:',
        cpuThreads: 'Hilos CPU:',
        systemCommands: 'Comandos de sistema (Peligroso)',
        enableSystemCommands: 'Activar comandos de sistema (apagar PC, reiniciar WiFi, etc.)',
        systemCommandsHelp: 'Permite comandos como "apaga la pc", "reiniciar wifi", "reiniciar bluetooth". Desactivado por defecto.',
        updates: 'Actualizaciones',
        checkUpdates: 'Buscar actualizaciones',
        updateRestart: 'Actualizar y reiniciar',
        on: 'Encendido',
        off: 'Apagado',
        turnOn: 'Encender',
        turnOff: 'Apagar',
        missing: 'Falta',
        noDetail: 'Sin detalle',
        checkingTools: 'Probando herramientas...',
        missingConfig: 'Falta configurar:',
        toolsReady: 'Herramientas listas',
        preparing: 'Preparando...',
        downloading: 'Descargando...',
        searching: 'Buscando...',
        connectingIp: 'La IP para conectarte es:',
        webPassword: 'Contraseña web:',
        phoneOpenAddress: 'Desde el celular, abri esa direccion en el navegador',
        openingYoutube: 'Abriendo YouTube con:',
        actionError: 'No pude hacer eso:',
        processError: 'No pude procesar eso',
        commandLanguageMismatch: 'Ese comando no esta disponible en este idioma.',
        thinking: 'Dejame pensar...',
        working: 'Dale, un segundo...',
        llamaOff: 'LLaMA esta apagado. Activalo en Configuracion.',
        missingRiot: 'No tenes tu Riot ID configurado. Ponelo en Configuracion.',
        saved: 'Configuracion guardada',
        hello: 'Hola! Soy Neeko',
        helloLlamaOff: 'Hola! Soy Neeko\nLLaMA esta apagado. Activalo en Configuracion si lo necesitas.',
        noModel: 'No encontre el modelo GGUF',
        idle: [
            'Necesitas ayuda con algo?',
            'Estoy aqui si me necesitas!',
            'Hay algo que quieras saber?',
            'No seas timido, preguntame!',
            'Puedo abrir apps, buscar en Google y mas.',
            'Queres que abra Spotify o YouTube?'
        ],
        tabAddons: 'Addons',
        addonsTitle: 'Addons',
        addonsEnabled: 'Habilitados',
        addonsDisabled: 'Deshabilitados',
        addonNoAddons: 'No hay addons instalados',
        addonNoAddonsHint: 'Pone carpetas de addons en la carpeta "addons/" de la config de Neeko.',
        addonRequiresReload: 'Cambios se aplican al reiniciar la app.',
        addonEnable: 'Habilitar',
        addonDisable: 'Deshabilitar',
        addonInfo: 'Info',
        addonInfoTitle: 'Comandos',
        addonNoCommands: 'Este addon no declara comandos.',
        addonNoCommandsForLanguage: 'Este addon no tiene comandos para este idioma.',
        addonNotAdaptedEnglish: 'Not adapted to English',
        addonCloseInfo: 'Cerrar',
        addonCommandPatterns: 'Frases',
        addonHasJs: 'JS',
        addonHasCss: 'CSS',
        addonBy: 'por',
        addonVersion: 'v',
        tabMemory: 'Memoria',
        memoryTitle: 'Memoria',
        memorySearch: 'Buscar...',
        memoryNoFacts: 'No hay nada guardado aun.',
        memoryNoFactsHint: 'Decile algo a Neeko y lo guarda automaticamente. O deci "guarda que X es Y".',
        memoryExport: 'Exportar',
        memoryImport: 'Importar',
        memoryClearAll: 'Borrar todo',
        memoryAdd: 'Agregar',
        memoryCategory: 'Categoria:',
        memoryKey: 'Clave:',
        memoryValue: 'Valor:',
    },
    en: {
        locale: 'en-US',
        chatPlaceholder: 'Talk to Neeko...',
        settingsTitle: 'Settings',
        checkTools: 'Test',
        save: 'Save',
        cancel: 'Cancel',
        tabGeneral: 'General',
        tabTools: 'Tools',
        tabAi: 'AI',
        tabSystem: 'System',
        generalTitle: 'General',
        languageLabel: 'Language:',
        toolsTitle: 'Tools',
        downloadFfmpeg: 'Download FFmpeg + FFprobe',
        uninstallFfmpeg: 'Uninstall FFmpeg + FFprobe',
        downloadGit: 'Download Git',
        uninstallGit: 'Uninstall Git',
        fixedModel: 'Fixed model on Google Drive',
        downloadModel: 'Download model',
        openBrowser: 'Open in browser',
        installFromFile: 'Install from file',
        uninstallModel: 'Uninstall model',
        downloadLabel: 'Download',
        ready: 'Ready',
        defaultGitPath: 'Default Git path:',
        riotIdLabel: 'Riot ID (User#Tag):',
        lolRegion: 'LoL region:',
        aiAppearance: 'AI Appearance',
        neekoSprite: 'Neeko sprite:',
        render3d: 'Render Neeko in 3D',
        animation3d: '3D animation:',
        mouseTracking: 'Follow mouse',
        loadEngine: 'Load engine:',
        preparePython: 'Prepare Python engine',
        advanced: 'Advanced',
        autostartLlama: 'Auto-start LLaMA when opening the app',
        backToAi: 'Back to AI',
        explanation: 'Explanation',
        gpuLayers: 'GPU layers:',
        contextSize: 'Context:',
        cpuThreads: 'CPU threads:',
        systemCommands: 'System Commands (Dangerous)',
        enableSystemCommands: 'Enable system commands (shut down PC, restart WiFi, etc.)',
        systemCommandsHelp: 'Allows commands like "shut down the pc", "restart wifi", "restart bluetooth". Off by default.',
        updates: 'Updates',
        checkUpdates: 'Check for updates',
        updateRestart: 'Update and restart',
        on: 'On',
        off: 'Off',
        turnOn: 'Turn on',
        turnOff: 'Turn off',
        missing: 'Missing',
        noDetail: 'No details',
        checkingTools: 'Testing tools...',
        missingConfig: 'Missing configuration:',
        toolsReady: 'Tools ready',
        preparing: 'Preparing...',
        downloading: 'Downloading...',
        searching: 'Searching...',
        connectingIp: 'The IP to connect is:',
        webPassword: 'Web password:',
        phoneOpenAddress: 'From your phone, open that address in the browser',
        openingYoutube: 'Opening YouTube with:',
        actionError: 'I could not do that:',
        processError: 'I could not process that',
        commandLanguageMismatch: 'That command is not available in this language.',
        thinking: 'Let me think...',
        working: 'Sure, one second...',
        llamaOff: 'LLaMA is off. Turn it on in Settings.',
        missingRiot: 'Your Riot ID is not configured. Add it in Settings.',
        saved: 'Settings saved',
        hello: 'Hi! I am Neeko',
        helloLlamaOff: 'Hi! I am Neeko\nLLaMA is off. Turn it on in Settings if you need it.',
        noModel: 'I could not find the GGUF model',
        idle: [
            'Need help with anything?',
            'I am here if you need me!',
            'Anything you want to know?',
            'Do not be shy, ask me!',
            'I can open apps, search Google, and more.',
            'Want me to open Spotify or YouTube?'
        ],
        tabAddons: 'Addons',
        addonsTitle: 'Addons',
        addonsEnabled: 'Enabled',
        addonsDisabled: 'Disabled',
        addonNoAddons: 'No addons installed',
        addonNoAddonsHint: 'Put addon folders in the Neeko config "addons/" folder.',
        addonRequiresReload: 'Changes apply after restarting the app.',
        addonEnable: 'Enable',
        addonDisable: 'Disable',
        addonInfo: 'Info',
        addonInfoTitle: 'Commands',
        addonNoCommands: 'This addon does not declare commands.',
        addonNoCommandsForLanguage: 'This addon has no commands for this language.',
        addonNotAdaptedEnglish: 'Not adapted to English',
        addonCloseInfo: 'Close',
        addonCommandPatterns: 'Phrases',
        addonHasJs: 'JS',
        addonHasCss: 'CSS',
        addonBy: 'by',
        addonVersion: 'v',
        tabMemory: 'Memory',
        memoryTitle: 'Memory',
        memorySearch: 'Search...',
        memoryNoFacts: 'Nothing saved yet.',
        memoryNoFactsHint: 'Tell Neeko something and she saves it automatically. Or say "remember that X is Y".',
        memoryExport: 'Export',
        memoryImport: 'Import',
        memoryClearAll: 'Clear all',
        memoryAdd: 'Add',
        memoryCategory: 'Category:',
        memoryKey: 'Key:',
        memoryValue: 'Value:',
    },
};

function normalizeLanguage(language) {
    return I18N[language] ? language : 'es';
}

function t(key) {
    return I18N[currentLanguage]?.[key] ?? I18N.es[key] ?? key;
}

function setLanguage(language) {
    currentLanguage = normalizeLanguage(language);
    document.documentElement.lang = currentLanguage;
    chatInput.placeholder = t('chatPlaceholder');
    document.querySelectorAll('[data-i18n]').forEach((el) => {
        const value = t(el.dataset.i18n);
        if (typeof value === 'string') el.textContent = value;
    });
    const settingsTitle = document.querySelector('.settings-title-row h3');
    if (settingsTitle) settingsTitle.textContent = t('settingsTitle');
    if (checkToolsBtn) checkToolsBtn.textContent = t('checkTools');
    if (saveSettingsBtn) saveSettingsBtn.textContent = t('save');
    if (closeSettingsBtn) closeSettingsBtn.textContent = t('cancel');
}

async function renderAddonsList() {
    const container = document.getElementById('addons-list');
    if (!container) return;
    try {
        const addons = JSON.parse(JSON.stringify(await invoke('addon_list')));
        if (!addons.length) {
            container.innerHTML = `<div class="addons-empty"><p>${t('addonNoAddons')}</p><p style="font-size:11px;color:#aab;">${t('addonNoAddonsHint')}</p></div>`;
            return;
        }
        const enabled = addons.filter(a => a.enabled);
        const disabled = addons.filter(a => !a.enabled);
        let html = '';
        if (enabled.length) {
            html += `<div class="addons-section"><h5>${t('addonsEnabled')} (${enabled.length})</h5>`;
            for (const a of enabled) html += renderAddonCard(a);
            html += '</div>';
        }
        if (disabled.length) {
            html += `<div class="addons-section"><h5>${t('addonsDisabled')} (${disabled.length})</h5>`;
            for (const a of disabled) html += renderAddonCard(a);
            html += '</div>';
        }
        container.innerHTML = html;
        container.querySelectorAll('.addon-toggle-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const id = btn.dataset.addonId;
                const enabled = btn.dataset.enabled === 'true';
                try {
                    if (enabled) {
                        await invoke('addon_disable', { addonId: id });
                        NeekoAddons.unloadAddon(id);
                    } else {
                        await invoke('addon_enable', { addonId: id });
                        const addon = addons.find((item) => item.manifest.id === id);
                        if (addon) await NeekoAddons.loadAddon(addon);
                    }
                    renderAddonsList();
                } catch (e) {
                    showBubble('Error: ' + e);
                }
            });
        });
        container.querySelectorAll('.addon-info-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const addon = addons.find((item) => item.manifest.id === btn.dataset.addonId);
                if (addon) showAddonInfo(addon);
            });
        });
    } catch (e) {
        container.innerHTML = `<div class="addons-empty"><p>Error: ${e}</p></div>`;
    }
}

function renderAddonCard(addon) {
    const badges = [];
    if (addon.has_js) badges.push('<span class="addon-badge addon-badge-js">JS</span>');
    if (addon.has_css) badges.push('<span class="addon-badge addon-badge-css">CSS</span>');
    const addonName = localizeAddonText(addon.manifest.name);
    const addonDescription = localizeAddonText(addon.manifest.description);
    return `
        <div class="addon-card">
            <div class="addon-card-header">
                <div class="addon-card-info">
                    <strong>${escapeHtml(addonName)}</strong>
                    <span class="addon-card-version">${t('addonVersion')}${addon.manifest.version}</span>
                    ${badges.join('')}
                </div>
                <div class="addon-card-actions">
                    <button class="addon-info-btn" data-addon-id="${addon.manifest.id}" type="button">
                        ${t('addonInfo')}
                    </button>
                    <button class="addon-toggle-btn" data-addon-id="${addon.manifest.id}" data-enabled="${addon.enabled}" type="button">
                        ${addon.enabled ? t('addonDisable') : t('addonEnable')}
                    </button>
                </div>
            </div>
            <p class="addon-card-desc">${escapeHtml(addonDescription)}</p>
            ${addon.manifest.author ? `<small class="addon-card-author">${t('addonBy')} ${escapeHtml(addon.manifest.author)}</small>` : ''}
        </div>
    `;
}

function localizeAddonText(text) {
    if (!text) return '';
    const parts = text.split(' / ');
    if (parts.length < 2) return text;
    return currentLanguage === 'en' ? parts.slice(1).join(' / ') : parts[0];
}

function getAddonCommandsForCurrentLanguage(addon) {
    const commands = addon.manifest.commands || [];
    const hasAnyEnglish = commands.some((command) => command.patterns?.en?.length);

    if (currentLanguage === 'en' && !hasAnyEnglish) {
        return { message: t('addonNotAdaptedEnglish'), commands: [] };
    }

    const commandsForLanguage = commands
        .map((command) => ({
            ...command,
            patternsForLanguage: command.patterns?.[currentLanguage] || [],
        }))
        .filter((command) => command.patternsForLanguage.length);

    if (!commands.length) {
        return { message: t('addonNoCommands'), commands: [] };
    }

    if (!commandsForLanguage.length) {
        return { message: t('addonNoCommandsForLanguage'), commands: [] };
    }

    return { message: '', commands: commandsForLanguage };
}

function formatAddonPattern(pattern) {
    return pattern
        .replace(/\(\\d\+\)/g, '<numero>')
        .replace(/\\d\+/g, '<numero>')
        .replace(/\(\.\+\)/g, '<texto>')
        .replace(/\.\+/g, '<texto>')
        .replace(/\\s\+/g, ' ')
        .replace(/\[\s\]\+/g, ' ')
        .replace(/\\/g, '')
        .replace(/\(\?:/g, '(')
        .replace(/\[\s\]/g, ' ')
        .replace(/\?/g, '')
        .replace(/\|/g, ' / ');
}

function showAddonInfo(addon) {
    const existing = document.getElementById('addon-info-panel');
    if (existing) existing.remove();

    const { message, commands } = getAddonCommandsForCurrentLanguage(addon);
    const addonName = localizeAddonText(addon.manifest.name);
    const panel = document.createElement('div');
    panel.id = 'addon-info-panel';
    panel.className = 'addon-info-panel';
    panel.innerHTML = `
        <div class="addon-info-box">
            <div class="addon-info-header">
                <div>
                    <span>${t('addonInfoTitle')}</span>
                    <strong>${escapeHtml(addonName)}</strong>
                </div>
                <button id="addon-info-close" type="button" aria-label="${t('addonCloseInfo')}">x</button>
            </div>
            <div class="addon-info-body">
                ${message ? `<p class="addon-info-empty">${escapeHtml(message)}</p>` : commands.map((command) => `
                    <div class="addon-command">
                        <strong>${escapeHtml(localizeAddonText(command.description || command.id))}</strong>
                        <span>${t('addonCommandPatterns')}</span>
                        <ul>
                            ${command.patternsForLanguage.map((pattern) => `<li><code>${escapeHtml(formatAddonPattern(pattern))}</code></li>`).join('')}
                        </ul>
                    </div>
                `).join('')}
            </div>
        </div>
    `;
    settingsModalContent.appendChild(panel);
    panel.addEventListener('click', (event) => {
        if (event.target === panel) panel.remove();
    });
    panel.querySelector('#addon-info-close')?.addEventListener('click', () => panel.remove());
}

function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// ─── Memory / Knowledge UI ───
async function renderMemoryList(searchQuery = '') {
    const container = document.getElementById('memory-list');
    if (!container) return;
    try {
        let facts;
        if (searchQuery) {
            facts = JSON.parse(JSON.stringify(await invoke('knowledge_search', { query: searchQuery })));
        } else {
            facts = JSON.parse(JSON.stringify(await invoke('knowledge_list')));
        }
        if (!facts.length) {
            container.innerHTML = `<div class="memory-empty"><p>${t('memoryNoFacts')}</p><p style="font-size:11px;color:#aab;">${t('memoryNoFactsHint')}</p></div>`;
            return;
        }
        // Agrupar por categoria
        const groups = {};
        for (const f of facts) {
            const cat = f.category || 'general';
            if (!groups[cat]) groups[cat] = [];
            groups[cat].push(f);
        }
        let html = '';
        for (const [cat, items] of Object.entries(groups)) {
            html += `<div class="memory-section"><h5>${cat.charAt(0).toUpperCase() + cat.slice(1)}</h5>`;
            for (const f of items) {
                html += `
                    <div class="memory-item">
                        <div class="memory-item-content">
                            <span class="memory-item-key">${escapeHtml(f.key)}</span>
                            <span class="memory-item-value">${escapeHtml(f.value)}</span>
                        </div>
                        <button class="memory-item-delete" data-id="${f.id}" title="Borrar">x</button>
                    </div>`;
            }
            html += '</div>';
        }
        container.innerHTML = html;
        container.querySelectorAll('.memory-item-delete').forEach(btn => {
            btn.addEventListener('click', async () => {
                await invoke('knowledge_delete', { id: btn.dataset.id });
                await refreshKnowledgeContext();
                syncSystemPrompt();
                renderMemoryList(searchQuery);
            });
        });
    } catch (e) {
        container.innerHTML = `<div class="memory-empty"><p>Error: ${e}</p></div>`;
    }
}

function initMemoryTab() {
    if (memoryTabInitialized) {
        renderMemoryList(document.getElementById('memory-search')?.value || '');
        return;
    }
    memoryTabInitialized = true;

    const searchInput = document.getElementById('memory-search');
    if (searchInput) {
        searchInput.addEventListener('input', () => renderMemoryList(searchInput.value));
    }
    const exportBtn = document.getElementById('memory-export-btn');
    if (exportBtn) {
        exportBtn.addEventListener('click', async () => {
            try {
                const json = await invoke('knowledge_export');
                const blob = new Blob([json], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = 'neeko-knowledge.json';
                a.click();
                URL.revokeObjectURL(url);
            } catch (e) {
                showBubble('Error: ' + e);
            }
        });
    }
    const importBtn = document.getElementById('memory-import-btn');
    if (importBtn) {
        importBtn.addEventListener('click', () => {
            const input = document.createElement('input');
            input.type = 'file';
            input.accept = '.json';
            input.onchange = async (e) => {
                const file = e.target.files[0];
                if (!file) return;
                const text = await file.text();
                try {
                    const count = await invoke('knowledge_import', { json: text });
                    await refreshKnowledgeContext();
                    syncSystemPrompt();
                    renderMemoryList();
                    showBubble(`${count} facts imported`);
                } catch (err) {
                    showBubble('Error: ' + err);
                }
            };
            input.click();
        });
    }
    const clearBtn = document.getElementById('memory-clear-btn');
    if (clearBtn) {
        clearBtn.addEventListener('click', async () => {
            if (confirm(currentLanguage === 'en' ? 'Clear all memory?' : 'Borrar toda la memoria?')) {
                await invoke('knowledge_clear');
                await refreshKnowledgeContext();
                syncSystemPrompt();
                renderMemoryList();
            }
        });
    }
    renderMemoryList();
}

function resetSystemPrompt() {
    conversationHistory = [{ role: "system", content: getSystemPrompt() }];
}

function syncSystemPrompt() {
    if (!conversationHistory.length || conversationHistory[0].role !== 'system') {
        resetSystemPrompt();
        return;
    }
    conversationHistory[0].content = getSystemPrompt();
}

function normalizeNeekoSprite(sprite) {
    return Object.values(SPRITES).includes(sprite) ? sprite : SPRITES.default;
}

function applyNeekoSprite(sprite) {
    const selected = normalizeNeekoSprite(sprite);
    neekoImg.src = selected;
    neekoImg.classList.remove('sprite-loading');
    neekoSprite.classList.toggle('sprite-standing', selected === SPRITES.standing);
    neekoSprite.classList.toggle('sprite-sitting', selected === SPRITES.sitting);
}

function applyRender3D(enabled) {
    neeko3dRendered = !!enabled && !!THREE;
    neekoSection.classList.toggle('render-3d', neeko3dRendered);
    neekoSprite.classList.toggle('using-3d', neeko3dRendered);

    if (neeko3dRendered) {
        neekoImg.style.display = 'none';
        neeko3d.style.display = 'block';
        initNeeko3d();
        syncNeeko3dAnimation();
        startNeeko3dIdle();
    } else {
        neekoImg.style.display = '';
        neeko3d.style.display = 'none';
        stopNeeko3dIdle();
    }
}

function initNeeko3d() {
    if (neeko3dRenderer || !THREE || !GLTFLoaderClass) return;

    neeko3dClock = new THREE.Clock();
    neeko3dScene = new THREE.Scene();

    neeko3dCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 50);
    neeko3dCamera.position.set(0, NEEKO_3D_PORTRAIT.centerY, 7);
    neeko3dCamera.lookAt(0, NEEKO_3D_PORTRAIT.centerY, 0);

    neeko3dRenderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    neeko3dRenderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    neeko3dRenderer.setClearColor(0x000000, 0);
    neeko3dRenderer.outputColorSpace = THREE.SRGBColorSpace;
    neeko3d.appendChild(neeko3dRenderer.domElement);

    const ambientLight = new THREE.HemisphereLight(0xdff7ff, 0x2a2140, 2.5);
    neeko3dScene.add(ambientLight);

    const keyLight = new THREE.DirectionalLight(0xffffff, 2.4);
    keyLight.position.set(2.5, 4.5, 5);
    neeko3dScene.add(keyLight);

    const fillLight = new THREE.DirectionalLight(0x8fdcff, 1.2);
    fillLight.position.set(-3, 2.4, 3);
    neeko3dScene.add(fillLight);

    neeko3dResizeObserver = new ResizeObserver(resizeNeeko3d);
    neeko3dResizeObserver.observe(neeko3d);
    resizeNeeko3d();

        new GLTFLoaderClass().load('neeko.glb', ({ scene, animations }) => {
        neeko3dModel = scene;
        neeko3dModel.rotation.y = -0.12;

        const box = new THREE.Box3().setFromObject(neeko3dModel);
        const center = box.getCenter(new THREE.Vector3());
        const size = box.getSize(new THREE.Vector3());
        const avatarHeight = 2.95;
        const modelScale = avatarHeight / Math.max(size.y, 0.001);

        neeko3dModel.scale.setScalar(modelScale);
        neeko3dModel.position.set(-center.x * modelScale, -box.min.y * modelScale - 0.08, -center.z * modelScale);
        neeko3dModelBaseY = neeko3dModel.position.y;

        neeko3dHeadBone = null;
        neeko3dNeckBone = null;

        const boneNames = [];
        neeko3dModel.traverse((child) => {
            const name = child.name.toLowerCase();
            if (child.isMesh) {
                child.frustumCulled = false;
            }
            if (child.name && !child.isMesh && !child.isLight && !child.isCamera) {
                boneNames.push(child.name);
            }
            if (!neeko3dHeadBone && (name.includes('head') || name.includes('cabeza'))) {
                neeko3dHeadBone = child;
            }
            if (!neeko3dNeckBone && (name.includes('neck') || name.includes('cuello'))) {
                neeko3dNeckBone = child;
            }
        });
        console.log('Bones found:', boneNames);
        console.log('Head bone:', neeko3dHeadBone?.name, 'Neck bone:', neeko3dNeckBone?.name);

        neeko3dScene.add(neeko3dModel);
        neeko3dMixer = new THREE.AnimationMixer(neeko3dModel);
        animations.forEach((clip) => {
            neeko3dActions.set(clip.name, neeko3dMixer.clipAction(clip));
        });
        syncNeeko3dAnimation();
    }, undefined, (error) => {
        console.error('No pude cargar neeko.glb:', error);
        neekoImg.style.display = '';
        neeko3d.style.display = 'none';
    });
}

function resizeNeeko3d() {
    if (!neeko3dRenderer || !neeko3dCamera) return;

    const rect = neeko3d.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width));
    const height = Math.max(1, Math.floor(rect.height));
    const aspect = width / height;
    const viewHeight = NEEKO_3D_PORTRAIT.viewHeight;
    const viewWidth = viewHeight * aspect;

    neeko3dRenderer.setSize(width, height, false);
    neeko3dCamera.position.x = NEEKO_3D_PORTRAIT.centerX;
    neeko3dCamera.position.y = NEEKO_3D_PORTRAIT.centerY;
    neeko3dCamera.lookAt(NEEKO_3D_PORTRAIT.centerX, NEEKO_3D_PORTRAIT.centerY, 0);
    neeko3dCamera.left = -viewWidth / 2;
    neeko3dCamera.right = viewWidth / 2;
    neeko3dCamera.top = viewHeight / 2;
    neeko3dCamera.bottom = -viewHeight / 2;
    neeko3dCamera.updateProjectionMatrix();
}

function setNeeko3dAnimation(name) {
    const nextAction = neeko3dActions.get(name);
    if (!nextAction || neeko3dActiveAction === nextAction) return;

    nextAction.reset().setLoop(THREE.LoopRepeat, Infinity).fadeIn(0.25).play();
    if (neeko3dActiveAction) {
        neeko3dActiveAction.fadeOut(0.25);
    }
    neeko3dActiveAction = nextAction;
}

function syncNeeko3dAnimation() {
    if (!neeko3dRendered) return;

    if (neekoSprite.classList.contains('talking')) {
        setNeeko3dAnimation(NEEKO_3D_ANIMATIONS.talking);
        NEEKO_3D_PORTRAIT.centerX = 0.5;
        NEEKO_3D_PORTRAIT.centerY = 9;
        NEEKO_3D_PORTRAIT.viewHeight = 4;
    } else if (neekoSprite.classList.contains('thinking')) {
        setNeeko3dAnimation(NEEKO_3D_ANIMATIONS.thinking);
        NEEKO_3D_PORTRAIT.centerX = 0;
        NEEKO_3D_PORTRAIT.centerY = 3.5;
        NEEKO_3D_PORTRAIT.viewHeight = 5;
    } else {
        setNeeko3dAnimation(neeko3dSelectedIdle);
        NEEKO_3D_PORTRAIT.centerX = 0;
        NEEKO_3D_PORTRAIT.centerY = 5.9;
        NEEKO_3D_PORTRAIT.viewHeight = 4;
    }
    resizeNeeko3d();
}

function startNeeko3dIdle() {
    if (neeko3dAnimationId) return;

    const onmousemove = (e) => {
        neeko3dMouseX = (e.clientX / window.innerWidth) * 2 - 1;
        neeko3dMouseY = (e.clientY / window.innerHeight) * 2 - 1;
    };
    window.addEventListener('mousemove', onmousemove);
    neeko3dIdleCleanup = () => window.removeEventListener('mousemove', onmousemove);

    const animate = (time) => {
        if (!neeko3dRendered) {
            neeko3dAnimationId = null;
            return;
        }

        const seconds = time / 1000;
        const talking = neekoSprite.classList.contains('talking');
        const thinking = neekoSprite.classList.contains('thinking');

        const delta = neeko3dClock.getDelta();
        if (neeko3dMixer) {
            neeko3dMixer.update(delta);
        }
        if (neeko3dModel) {
            const talkSway = talking ? Math.sin(seconds * 7) * 0.025 : 0;
            neeko3dModel.rotation.y = -0.12 + Math.sin(seconds * 0.8) * 0.035;
            neeko3dModel.position.y = neeko3dModelBaseY + Math.sin(seconds * 1.2) * 0.025 + talkSway;

            if (neeko3dMouseTracking && (neeko3dHeadBone || neeko3dNeckBone)) {
                neeko3dHeadTargetRotY = neeko3dMouseX * 0.35;
                neeko3dHeadTargetRotX = -neeko3dMouseY * 0.2;
            } else {
                neeko3dHeadTargetRotX = 0;
                neeko3dHeadTargetRotY = 0;
            }

            const lerpFactor = 1 - Math.pow(0.001, delta);
            neeko3dHeadCurrentRotX += (neeko3dHeadTargetRotX - neeko3dHeadCurrentRotX) * lerpFactor;
            neeko3dHeadCurrentRotY += (neeko3dHeadTargetRotY - neeko3dHeadCurrentRotY) * lerpFactor;

            if (neeko3dHeadBone) {
                neeko3dHeadBone.rotation.y = neeko3dHeadCurrentRotY;
                neeko3dHeadBone.rotation.x = neeko3dHeadCurrentRotX;
            }
            if (neeko3dNeckBone) {
                neeko3dNeckBone.rotation.y = neeko3dHeadCurrentRotY * 0.4;
                neeko3dNeckBone.rotation.x = neeko3dHeadCurrentRotX * 0.3;
            }
        }
        if (neeko3dRenderer && neeko3dScene && neeko3dCamera) {
            neeko3dRenderer.render(neeko3dScene, neeko3dCamera);
        }

        neeko3dAnimationId = requestAnimationFrame(animate);
    };

    neeko3dAnimationId = requestAnimationFrame(animate);
}

function stopNeeko3dIdle() {
    if (neeko3dAnimationId) {
        cancelAnimationFrame(neeko3dAnimationId);
        neeko3dAnimationId = null;
    }
    if (neeko3dIdleCleanup) {
        neeko3dIdleCleanup();
        neeko3dIdleCleanup = null;
    }
}

function getSystemPrompt() {
    const now = new Date();
    const options = { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' };
    const locale = t('locale');
    const fecha = now.toLocaleDateString(locale, options);
    const hora = now.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });

    if (currentLanguage === 'en') {
        return `You are Neeko, a vastaya from the Oovi-Kat tribe in League of Legends.
You are playful, curious, cheerful, and a little childlike.
Always speak in English, with short, sweet, fun replies.
Use the lizard emoji often and phrases like "Neeko is Neeko!".

CURRENT DATE AND TIME: ${fecha}, ${hora}.

MEMORY RULES:
- The "Saved user memory" section below contains true facts about the user.
- When the user asks about themselves, their likes, hardware, preferences, work, or personal details, answer using saved memory first.
- If several saved facts are relevant, mention all of them.
- Do not invent user details that are not in saved memory.
- If saved memory does not contain the answer, say you do not have that saved yet.

${knowledgeContext}

ACTIONS (answer with JSON at the start of the response, separated by |||):
- open_app: {"action": "open_app", "app": "name"}|||reply
- play_music: {"action": "play_music", "query": "song or artist"}|||reply
- open_url: {"action": "open_url", "url": "https://..."}|||reply
- search: {"action": "search", "query": "search query"}|||reply
If it is not an action, reply normally as Neeko.`;
    }

    return `Sos Neeko, una vastaya de la tribu Oovi-Kat de League of Legends.
Sos juguetona, curiosa, alegre y un poco infantil.
Hablás siempre en español, de forma corta, cariñosa y divertida.
Usás mucho el emoji 🦎 y frases como "¡Neeko es Neeko!".

FECHA Y HORA ACTUAL: ${fecha}, son las ${hora}.

ACCIONES (respondé con JSON al inicio de la respuesta, separado por |||):
- open_app: {"action": "open_app", "app": "nombre"}|||respuesta
- play_music: {"action": "play_music", "query": "canción o artista"}|||respuesta
- open_url: {"action": "open_url", "url": "https://..."}|||respuesta
- search: {"action": "search", "query": "busqueda"}|||respuesta
REGLAS DE MEMORIA:
- La seccion "Memoria guardada del usuario" de abajo contiene datos reales del usuario.
- Cuando el usuario pregunte sobre si mismo, sus gustos, hardware, preferencias, trabajo o datos personales, responde usando primero esa memoria.
- Si hay varias memorias relevantes, menciona todas las que correspondan.
- No inventes datos del usuario que no esten en la memoria.
- Si la memoria no contiene la respuesta, deci que todavia no tenes eso guardado.

${knowledgeContext}
Si no es una acción, respondé normal como Neeko.`;
}

let currentChatId = null;
let knowledgeContext = '';
let conversationHistory = [
    { role: "system", content: getSystemPrompt() }
];

async function refreshKnowledgeContext() {
    try {
        const facts = JSON.parse(JSON.stringify(await invoke('knowledge_list')));
        if (!facts.length) {
            knowledgeContext = '';
            return;
        }
        const isEnglish = currentLanguage === 'en';
        let ctx = isEnglish
            ? '\n\n## Saved user memory\nThese are true saved facts about the user. Use them to answer any question about the user:\n'
            : '\n\n## Memoria guardada del usuario\nEstos son datos reales guardados sobre el usuario. Usalos para responder cualquier pregunta sobre el usuario:\n';
        let currentCat = '';
        for (const f of facts) {
            if (f.category !== currentCat) {
                currentCat = f.category;
                ctx += `\n### ${currentCat.charAt(0).toUpperCase() + currentCat.slice(1)}\n`;
            }
            ctx += `- ${f.key}: ${f.value}\n`;
        }
        ctx += isEnglish
            ? '\nIf the user asks about these topics, answer from this memory instead of guessing.\n'
            : '\nSi el usuario pregunta sobre estos temas, responde desde esta memoria en vez de adivinar.\n';
        ctx += isEnglish
            ? '\nWhen the user tells you something important about themselves, include this hidden JSON block in your response so the app can save it:\n'
            : '\nCuando el usuario te cuente algo importante sobre si mismo, incluye este bloque JSON oculto para que la app pueda guardarlo:\n';
        ctx += '{"_save_knowledge": {"category": "...", "key": "...", "value": "..."}}\n';
        ctx += isEnglish
            ? 'The user will not see this JSON block. Valid categories: hardware, personal, trabajo, software, preferencia, general\n'
            : 'El usuario no vera este bloque JSON. Categorias validas: hardware, personal, trabajo, software, preferencia, general\n';
        knowledgeContext = ctx;
    } catch (e) {
        console.error('[Knowledge] Error loading context:', e);
        knowledgeContext = '';
    }
}

async function buildRuntimeMemoryReminder(userMessage) {
    try {
        const facts = JSON.parse(JSON.stringify(await invoke('knowledge_list')));
        if (!facts.length) return null;

        const lines = facts.map((fact) => {
            const category = fact.category || 'general';
            const key = fact.key || 'info';
            return `- ${category} / ${key}: ${fact.value}`;
        });

        if (currentLanguage === 'en') {
            return `Internal memory reminder for the next answer.
User message: "${userMessage}"
Saved user facts:
${lines.join('\n')}

If the user asks anything about themselves, their likes, preferences, hardware, work, or personal details, answer only from these saved facts. If several facts match, include all of them. Do not add guesses, jokes, or invented user details.`;
        }

        return `Recordatorio interno de memoria para la proxima respuesta.
Mensaje del usuario: "${userMessage}"
Datos guardados del usuario:
${lines.join('\n')}

Si el usuario pregunta algo sobre si mismo, sus gustos, preferencias, hardware, trabajo o datos personales, responde solo con estos datos guardados. Si varios datos coinciden, incluilos todos. No agregues suposiciones, chistes ni datos inventados del usuario.`;
    } catch (error) {
        console.error('[Knowledge] Error building runtime reminder:', error);
        return null;
    }
}

async function parseAndSaveKnowledge(reply) {
    try {
        const match = reply.match(/\{"_save_knowledge":\s*(\{[^}]+\})\}/);
        if (match) {
            const data = JSON.parse(match[1]);
            if (data.category && data.key && data.value) {
                await invoke('knowledge_add', { category: data.category, key: data.key, value: data.value });
                await refreshKnowledgeContext();
                syncSystemPrompt();
                return reply.replace(match[0], '').trim();
            }
        }
    } catch (e) {
        console.error('[Knowledge] Error parsing save:', e);
    }
    return reply;
}

function cleanKnowledgeValue(value) {
    return value
        .trim()
        .replace(/^[\s:,-]+/, '')
        .replace(/[.!?]+$/, '')
        .trim();
}

function pushKnowledgeFact(facts, category, key, value) {
    const cleaned = cleanKnowledgeValue(value);
    if (!cleaned || cleaned.length < 2) return;
    const duplicate = facts.some((fact) => (
        fact.category === category
        && fact.key.toLowerCase() === key.toLowerCase()
        && fact.value.toLowerCase() === cleaned.toLowerCase()
    ));
    if (!duplicate) facts.push({ category, key, value: cleaned });
}

function extractKnowledgeFromUserMessage(message) {
    const text = message.trim();
    const lower = text.toLowerCase();
    const facts = [];

    if (!text || /^(que|qué|cual|cu[aá]l|como|c[oó]mo|when|what|which|how)\b/i.test(lower)) {
        return facts;
    }

    const preferencePatterns = [
        { pattern: /\bme\s+gustan?\s+(.+)/i, key: 'me gusta' },
        { pattern: /\bme\s+gustaban?\s+(.+)/i, key: 'me gusta' },
        { pattern: /\bme\s+encantan?\s+(.+)/i, key: 'me encanta' },
        { pattern: /\bamo\s+(.+)/i, key: 'me gusta' },
        { pattern: /\bi\s+like\s+(.+)/i, key: 'likes' },
        { pattern: /\bi\s+love\s+(.+)/i, key: 'likes' },
    ];
    for (const { pattern, key } of preferencePatterns) {
        const match = text.match(pattern);
        if (match) {
            pushKnowledgeFact(facts, 'preferencia', key, match[1]);
            break;
        }
    }

    const cpuMatch = text.match(/\b(?:ryzen\s+\d(?:\s+\d{3,5}[a-z0-9]*)?|intel\s+core\s+i[3579][-\s]?\d+[a-z0-9]*|core\s+i[3579][-\s]?\d+[a-z0-9]*|i[3579][-\s]?\d{3,5}[a-z0-9]*)\b/i);
    if (cpuMatch) {
        pushKnowledgeFact(facts, 'hardware', 'cpu', cpuMatch[0]);
    }

    const gpuMatch = text.match(/\b(?:radeon\s+)?(?:rx\s+\d{3,5}\s*xt|rx\s+\d{3,5}|rtx\s+\d{3,5}(?:\s*ti)?|gtx\s+\d{3,5}(?:\s*ti)?)\b/i);
    if (gpuMatch) {
        pushKnowledgeFact(facts, 'hardware', 'gpu', gpuMatch[0]);
    }

    const ramMatch = text.match(/\b(?:tengo|uso|i\s+have|my\s+pc\s+has)\s+(\d{1,3})\s*(?:gb|gigas?)\s+(?:de\s+)?ram\b/i);
    if (ramMatch) {
        pushKnowledgeFact(facts, 'hardware', 'ram', `${ramMatch[1]} GB`);
    }

    const personalPatterns = [
        { pattern: /\bsoy\s+(.+)/i, key: 'soy' },
        { pattern: /\bvivo\s+en\s+(.+)/i, key: 'vive en' },
        { pattern: /\btrabajo\s+en\s+(.+)/i, key: 'trabaja en' },
        { pattern: /\bmy\s+name\s+is\s+(.+)/i, key: 'name' },
        { pattern: /\bi\s+live\s+in\s+(.+)/i, key: 'lives in' },
        { pattern: /\bi\s+work\s+at\s+(.+)/i, key: 'works at' },
    ];
    for (const { pattern, key } of personalPatterns) {
        const match = text.match(pattern);
        if (match) {
            pushKnowledgeFact(facts, 'personal', key, match[1]);
            break;
        }
    }

    return facts;
}

async function saveKnowledgeFromUserMessage(message) {
    const facts = extractKnowledgeFromUserMessage(message);
    if (!facts.length) return 0;

    for (const fact of facts) {
        await invoke('knowledge_add', fact);
    }
    await refreshKnowledgeContext();
    syncSystemPrompt();

    const memoryPanel = document.querySelector('[data-settings-panel="memory"].active');
    if (memoryPanel) {
        renderMemoryList(document.getElementById('memory-search')?.value || '');
    }

    return facts.length;
}

const idlePhrases = [
    "¿Necesitas ayuda con algo? 🌸",
    "¡Estoy aquí si me necesitas! ✨",
    "¿Hay algo que quieras saber? 🤔",
    "¡No seas tímido, pregúntame! 💜",
    "Puedo abrir apps, buscar en Google y más. 🦎",
    "¿Querés que abra Spotify o YouTube? 🎵"
];

let idleTimeout;

function resetIdleTimer() {
    clearTimeout(idleTimeout);
    idleTimeout = setTimeout(showIdlePhrase, 20000);
}

function showIdlePhrase() {
    const phrases = t('idle') || idlePhrases;
    const phrase = phrases[Math.floor(Math.random() * phrases.length)];
    showBubble(`${phrase} 🦎`);
    resetIdleTimer();
}

function showBubble(text) {
    bubbleText.textContent = text;
    speechBubble.classList.remove('hidden');
}

function cleanAiReply(text) {
    return String(text || '')
        .replace(/<think>[\s\S]*?<\/think>/gi, '')
        .replace(/<\/?think>/gi, '')
        .trim();
}

function setTalking(talking) {
    neekoSprite.classList.toggle('talking', talking);
    syncNeeko3dAnimation();
}

function setThinking(thinking) {
    neekoSprite.classList.toggle('thinking', thinking);
    syncNeeko3dAnimation();
}

function parseNeekoResponse(text) {
    text = cleanAiReply(text);
    const jsonMatch = text.match(/\{[\s\S]*?"action"[\s\S]*?\}/);
    if (jsonMatch) {
        try {
            const action = JSON.parse(jsonMatch[0]);
            const afterJson = text.substring(text.indexOf(jsonMatch[0]) + jsonMatch[0].length);
            const message = afterJson.replace(/^\|+/, '').trim();
            return { action, message };
        } catch { }
    }
    return { action: null, message: text };
}

function buildSearchUrl(site, query) {
    const s = site.toLowerCase().trim();
    const q = encodeURIComponent(query.trim());
    const targets = {
        google: `https://www.google.com/search?q=${q}`,
        g: `https://www.google.com/search?q=${q}`,
        youtube: `https://www.youtube.com/results?search_query=${q}`,
        yt: `https://www.youtube.com/results?search_query=${q}`,
        github: `https://github.com/search?q=${q}`,
        gh: `https://github.com/search?q=${q}`,
        reddit: `https://www.reddit.com/search/?q=${q}`,
        mercado: `https://listado.mercadolibre.com.ar/${q}`,
        mercadolibre: `https://listado.mercadolibre.com.ar/${q}`,
        ml: `https://listado.mercadolibre.com.ar/${q}`,
        wikipedia: `https://es.wikipedia.org/w/index.php?search=${q}`,
        wiki: `https://es.wikipedia.org/w/index.php?search=${q}`,
        spotify: `https://open.spotify.com/search/${q}`,
        steam: `https://store.steampowered.com/search/?term=${q}`,
    };

    return targets[s] || `https://www.google.com/search?q=site%3A${encodeURIComponent(site.trim())}+${q}`;
}

function detectActionFromText(text) {
    const lower = text.toLowerCase().trim();
    const isEnglish = currentLanguage === 'en';

    // ─── Addon Commands (check first) ───
    const addonMatch = NeekoAddons.detectCommand(text);
    if (addonMatch) return addonMatch;

    // ─── IP Detection ───
    const isIpCommand = isEnglish
        ? (lower === 'ip' || lower === 'my ip' || lower === 'local ip' || (lower.includes('ip') && (lower.includes('what') || lower.includes('connect') || lower.includes('address'))))
        : (lower === 'ip' || lower === 'mi ip' || lower === 'la ip' || (lower.includes('ip') && (lower.includes('cuál') || lower.includes('cual') || lower.includes('que') || lower.includes('cómo') || lower.includes('como') || lower.includes('conect') || lower.includes('dirección') || lower.includes('direccion'))));
    if (isIpCommand) {
        return { action: { action: "get_ip" }, message: "" };
    }

    // ─── Git Detection (check before app detection) ───
    const gitPatterns = [
        { pattern: /git\s+init(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_init", path: m[1] || null }) },
        { lang: 'es', pattern: /inicializ(?:ar|a)\s+(?:un\s+)?repo(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_init", path: m[1] || null }) },

        { pattern: /git\s+add\s+(.+?)(?:\s+en\s+(.+))?$/i, handler: (m) => ({ action: "git_add", files: m[1].trim(), path: m[2] || null }) },
        { lang: 'es', pattern: /(?:agregar|añadir|agreg(?:a|o))\s+(.+?)(?:\s+al\s+repo|\s+en\s+(.+))?$/i, handler: (m) => ({ action: "git_add", files: m[1].trim(), path: m[2] || null }) },

        { pattern: /git\s+commit\s+(?:-m\s+)?["']?(.+?)["']?$/i, handler: (m) => ({ action: "git_commit", path: null, message: m[1].trim() }) },
        { lang: 'es', pattern: /haz\s+commit\s+(?:con\s+)?(?:mensaje\s+)?["']?(.+?)["']?$/i, handler: (m) => ({ action: "git_commit", path: null, message: m[1].trim() }) },
        { lang: 'es', pattern: /commitea(?:r)?\s+(?:con\s+)?(?:mensaje\s+)?["']?(.+?)["']?$/i, handler: (m) => ({ action: "git_commit", path: null, message: m[1].trim() }) },

        { lang: 'es', pattern: /sub(?:e|ir)\s+(?:mi\s+)?repo(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_full_push", path: m[1] || null }) },
        { pattern: /git\s+push(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_push", path: m[1] || null }) },
        { lang: 'es', pattern: /pushe(?:a|r?)\s+(?:el\s+)?repo/i, handler: () => ({ action: "git_full_push", path: null }) },

        { pattern: /git\s+pull(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_pull", path: m[1] || null }) },
        { lang: 'es', pattern: /baj(?:a|ar)\s+(?:los?\s+)?cambios?\s+(?:en\s+(.+))?/i, handler: (m) => ({ action: "git_pull", path: m[1] || null }) },

        { pattern: /git\s+status(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_status", path: m[1] || null }) },
        { lang: 'es', pattern: /estado\s+(?:del\s+)?repo(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_status", path: m[1] || null }) },

        { pattern: /git\s+log(?:(?:\s+ultim(?:os?|as?)\s+)?(\d+))?(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_log", count: m[1] ? parseInt(m[1]) : 10, path: m[2] || null }) },
        { lang: 'es', pattern: /(?:últim(?:os?|as?)\s+)?commits?(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_log", count: 10, path: m[1] || null }) },

        { pattern: /git\s+branch(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_branch", path: m[1] || null }) },
        { lang: 'es', pattern: /(?:que\s+)?branches?\s+tien(?:e|es?)\s+(?:en\s+(.+))?/i, handler: (m) => ({ action: "git_branch", path: m[1] || null }) },

        { pattern: /git\s+remote\s+add\s+(\S+)\s+(\S+)(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_remote_add", name: m[1], url: m[2], path: m[3] || null }) },
    ];

    for (const { lang, pattern, handler } of gitPatterns) {
        if (lang && lang !== currentLanguage) continue;
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── LOL Detection ───
    const lolPatterns = [
        {
            lang: 'en',
            pattern: /last\s+match\s+(?:of\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+in\s+(las?|euw|eune|na|br|kr|korea|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea/, 'kr') || null, count: 1 })
        },
        {
            lang: 'en',
            pattern: /(?:match\s+history|matches|games)\s+(?:of\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+in\s+(las?|euw|eune|na|br|kr|korea|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea/, 'kr') || null, count: 5 })
        },
        {
            lang: 'en',
            pattern: /(?:my\s+)?last\s+match$/i,
            handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 1 })
        },
        {
            lang: 'en',
            pattern: /(?:my\s+)?(?:match\s+history|matches|games|lol)$/i,
            handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 5 })
        },
        {
            lang: 'en',
            pattern: /(?:rank|elo|tier)\s+(?:of\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+in\s+(las?|euw|eune|na|br|kr|korea|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_rank", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea/, 'kr') || null })
        },
        {
            lang: 'en',
            pattern: /(?:what\s+)?(?:rank|elo|tier)\s+(?:does\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)\s+(?:have|is)(?:\s+in\s+(las?|euw|eune|na|br|kr|korea|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_rank", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea/, 'kr') || null })
        },
        {
            lang: 'en',
            pattern: /(?:my\s+)?(?:elo|rank|tier)(?:\s+in\s+lol)?$/i,
            handler: () => ({ action: "lol_rank", riot_id: null, region: null })
        },
        {
            lang: 'en',
            pattern: /what\s+(?:rank|elo|tier)\s+am\s+i/i,
            handler: () => ({ action: "lol_rank", riot_id: null, region: null })
        },
        // With name#tag and optional region
        {
            pattern: /(?:ultima|última)\s+partida\s+(?:de\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: m[3]?.toLowerCase() || null, count: 1 })
        },
        {
            pattern: /(?:historial|partidas?)\s+(?:de\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: m[3]?.toLowerCase() || null, count: 5 })
        },
        {
            pattern: /(?:como\s+)?(?:va|está|esta)\s+([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: m[3]?.toLowerCase() || null, count: 1 })
        },
        // Without name (use config default) — "mi ultima partida", "ultima partida", "mis partidas"
        {
            pattern: /(?:mi\s+)?(?:ultima|última)\s+partida$/i,
            handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 1 })
        },
        {
            pattern: /(?:mi\s+)?(?:historial|partidas?)$/i,
            handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 5 })
        },
        {
            pattern: /(?:como\s+)?(?:va|está|esta)\s+(?:mi\s+)?(?:lol|partidas?)$/i,
            handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 1 })
        },

        // Rank / Elo — with name#tag
        {
            pattern: /(?:elo|rang[oa]?|clasificaci[oó]n)\s+(?:de\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|korea|corea|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_rank", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea|corea/, 'kr') || null })
        },
        {
            pattern: /(?:que\s+)?(?:rang[oa]?|elo)\s+(?:tiene|está|esta|es)\s+([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|korea|corea|jp|oce|tr|ru))?$/i,
            handler: (m) => ({ action: "lol_rank", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea|corea/, 'kr') || null })
        },
        // Rank / Elo — without name (self)
        {
            pattern: /(?:mi\s+)?(?:elo|rang[oa]?|clasificaci[oó]n)(?:\s+(?:de\s+)?lol)?$/i,
            handler: () => ({ action: "lol_rank", riot_id: null, region: null })
        },
        {
            pattern: /(?:que\s+)?(?:rang[oa]?|elo)\s+(?:tengo|soy|estoy)/i,
            handler: () => ({ action: "lol_rank", riot_id: null, region: null })
        },
        {
            pattern: /(?:en\s+que\s+)?(?:rang[oa]?|elo)\s+(?:estoy|soy|está)/i,
            handler: () => ({ action: "lol_rank", riot_id: null, region: null })
        },
        {
            pattern: /(?:cual\s+es\s+)?(?:mi\s+)?(?:rang[oa]?|elo)\s+(?:de\s+)?lol$/i,
            handler: () => ({ action: "lol_rank", riot_id: null, region: null })
        },
    ];

    for (const { lang, pattern, handler } of lolPatterns) {
        if (isEnglish && lang !== 'en') continue;
        if (!isEnglish && lang === 'en') continue;
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── Video Compression Detection ───
    const compressPatterns = isEnglish
        ? [
            {
                pattern: /compress\s+(?:the\s+)?(?:this\s+)?video\s*:\s*(.+)/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
            {
                pattern: /compress\s+(.+)\s+for\s+discord/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
            {
                pattern: /compress\s+(.+\.\w+)/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
        ]
        : [
            {
                pattern: /comprim(?:í|i|ir|e|o)\s+(?:el\s+)?(?:este\s+)?video\s*:\s*(.+)/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
            {
                pattern: /comprim(?:í|i|ir|e|o)\s+(.+)\s+para\s+discord/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
            {
                pattern: /comprim(?:í|i|ir|e|o)\s+(.+\.\w+)/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
            {
                pattern: /achic(?:á|a|ar)\s+(?:el\s+)?(?:este\s+)?video\s*:\s*(.+)/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
            {
                pattern: /achic(?:á|a|ar)\s+(.+\.\w+)/i,
                handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() })
            },
        ];

    for (const { pattern, handler } of compressPatterns) {
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── System Detection ───
    if (isEnglish) {
        if (/cancel\s+(?:shutdown|shut\s*down)/i.test(lower)) {
            return { action: { action: "cancel_shutdown" }, message: "" };
        }
        if (/(?:shutdown|shut\s*down)\s+(?:the\s+)?pc\s+in\s+(\d+)\s*(min(?:ute)?s?|hours?|h|s(?:econd)?s?)/i.test(lower)) {
            const m = lower.match(/(?:shutdown|shut\s*down)\s+(?:the\s+)?pc\s+in\s+(\d+)\s*(min(?:ute)?s?|hours?|h|s(?:econd)?s?)/i);
            let secs = parseInt(m[1]);
            if (/hours?|h/i.test(m[2])) secs *= 3600;
            else if (/min/i.test(m[2])) secs *= 60;
            return { action: { action: "shutdown", seconds: secs }, message: "" };
        }
        if (/(?:shutdown|shut\s*down)\s+(?:the\s+)?pc/i.test(lower)) {
            return { action: { action: "shutdown", seconds: 0 }, message: "" };
        }
        if (/restart\s+(?:explorer|icons?|taskbar|desktop|windows\s*explorer)/i.test(lower)) {
            return { action: { action: "restart_explorer" }, message: "" };
        }
        if (/restart\s+(?:wifi|wi-fi|internet|network|connection)/i.test(lower)) {
            return { action: { action: "restart_wifi" }, message: "" };
        }
        if (/restart\s+(?:bluetooth|blue\s*tooth)/i.test(lower)) {
            return { action: { action: "restart_bluetooth" }, message: "" };
        }
    } else {
        if (/cancel(?:ar)?\s+(?:el\s+)?(?:apagado|apaga)/i.test(lower)) {
            return { action: { action: "cancel_shutdown" }, message: "" };
        }
        if (/apag(?:a|ar|o)\s+(?:la\s+)?pc\s+en\s+(\d+)\s*(min(?:uto)?s?|horas?|h|s(?:egundo)?s?)/i.test(lower)) {
            const m = lower.match(/apag(?:a|ar|o)\s+(?:la\s+)?pc\s+en\s+(\d+)\s*(min(?:uto)?s?|horas?|h|s(?:egundo)?s?)/i);
            let secs = parseInt(m[1]);
            if (/hor?/i.test(m[2])) secs *= 3600;
            else if (/min/i.test(m[2])) secs *= 60;
            return { action: { action: "shutdown", seconds: secs }, message: "" };
        }
        if (/apag(?:a|ar|o)\s+(?:la\s+)?pc/i.test(lower)) {
            return { action: { action: "shutdown", seconds: 0 }, message: "" };
        }
        if (/reinici(?:a|ar|o)\s+(?:el\s+)?(?:explorer|iconos?|barra|escritorio|windows\s*explorer)/i.test(lower)) {
            return { action: { action: "restart_explorer" }, message: "" };
        }
        if (/reinici(?:a|ar|o)\s+(?:el\s+)?(?:wifi|wi-fi|internet|red|conexion|conexi[oó]n)/i.test(lower)) {
            return { action: { action: "restart_wifi" }, message: "" };
        }
        if (/reinici(?:a|ar|o)\s+(?:el\s+)?(?:bluetooth|blue\s*tooth)/i.test(lower)) {
            return { action: { action: "restart_bluetooth" }, message: "" };
        }
    }

    // ─── Config Detection ───
    const configPatterns = isEnglish
        ? [
            { pattern: /save\s+git\s+pat\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: m[1].trim(), region: null, git_path: null }) },
            { pattern: /set\s+region\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: null, region: m[1].trim(), git_path: null }) },
            { pattern: /set\s+git\s+path\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: null, region: null, git_path: m[1].trim() }) },
        ]
        : [
            { pattern: /configurar\s+lol\s+api\s*key\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: null, region: null, git_path: null }) },
            { pattern: /guardar\s+git\s+pat\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: m[1].trim(), region: null, git_path: null }) },
            { pattern: /configurar?\s+region\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: null, region: m[1].trim(), git_path: null }) },
            { pattern: /configurar?\s+git\s+path\s+(.+)/i, handler: (m) => ({ action: "lol_save_config", git_pat: null, region: null, git_path: m[1].trim() }) },
        ];

    for (const { pattern, handler } of configPatterns) {
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── App Detection ───
    const knownApps = [
        'spotify', 'discord', 'steam', 'chrome', 'firefox', 'edge',
        'notepad', 'calculadora', 'calculator', 'explorer', 'vscode',
        'code', 'powershell', 'terminal', 'whatsapp', 'telegram',
        'obs', 'youtube', 'spotify premium',
        'league of legends', 'lol', 'riot client',
        '7-zip', '7zip', 'winrar', 'obsidian', 'brave',
        'bluestacks', 'roblox', 'fightcade', 'qbittorrent',
        'davinci', 'filmora', 'photoshop', 'photoshop cs6',
        'virtualbox', 'node', 'python', 'git'
    ];
    for (const app of knownApps) {
        const appMatches = isEnglish
            ? (lower === app || lower === `open ${app}` || lower === `start ${app}` || lower === `launch ${app}`)
            : (lower === app || lower === `abri ${app}` || lower === `abre ${app}` || lower === `abrir ${app}`);
        if (appMatches) {
            if (app === 'youtube') {
                return { action: { action: "open_url", url: "https://www.youtube.com" }, message: "" };
            }
            return { action: { action: "open_app", app: app }, message: "" };
        }
    }

    const openPatterns = isEnglish
        ? [
            /open[\s]+(.+)/,
            /start[\s]+(.+)/,
            /launch[\s]+(.+)/,
        ]
        : [
            /abr[ií]?[\s]+(.+)/,
            /abrime[\s]+(.+)/,
            /abrir[\s]+(.+)/,
            /abri[\s]+(.+)/,
            /pone[r]?[\s]+(.+)/,
            /iniciar[\s]+(.+)/,
            /ejecutar[\s]+(.+)/,
            /abri(?:r)?\s+(?:el\s+|la\s+)?(.+)/,
        ];
    for (const pattern of openPatterns) {
        const match = lower.match(pattern);
        if (match) {
            const appName = match[1].trim();
            return { action: { action: "open_app", app: appName }, message: "" };
        }
    }

    const searchInPatterns = isEnglish
        ? [
            /search\s+(?:on|in)\s+([^:]+)\s*:\s*(.+)/,
        ]
        : [
            /busca[r]?\s+en\s+([^:]+)\s*:\s*(.+)/,
            /buscar\s+en\s+([^:]+)\s*:\s*(.+)/,
        ];
    for (const pattern of searchInPatterns) {
        const match = lower.match(pattern);
        if (match) {
            const site = match[1].trim();
            const query = match[2].trim();
            return {
                action: {
                    action: "open_url",
                    url: buildSearchUrl(site, query),
                },
                message: `Buscando en ${site}: ${query}`,
            };
        }
    }

    const searchPatterns = isEnglish
        ? [
            /search[\s]+(.+)/,
            /look\s+up[\s]+(.+)/,
        ]
        : [
            /busca[r]?[\s]+(.+)/,
            /buscar[\s]+(.+)/,
            /investigar[\s]+(.+)/,
        ];
    for (const pattern of searchPatterns) {
        const match = lower.match(pattern);
        if (match) {
            return { action: { action: "search", query: match[1].trim() }, message: "" };
        }
    }

    const musicPatterns = isEnglish
        ? [
            /play[\s]+(.+)/,
            /listen\s+to[\s]+(.+)/,
        ]
        : [
            /pon[eé]?[\s]+m[uú]sica[\s]+(.+)/,
            /reproducir[\s]+(.+)/,
            /escuchar[\s]+(.+)/,
        ];
    for (const pattern of musicPatterns) {
        const match = lower.match(pattern);
        if (match) {
            return { action: { action: "play_music", query: match[1].trim() }, message: "" };
        }
    }

    // ─── Knowledge / Memory Detection ───
    const knowledgePatterns = [
        { lang: 'es', pattern: /(?:que|cuales?|cuantos?)\s+(?:sabes|sabe|tenes|tiene)\s+(?:de\s+)?mi/i, handler: () => ({ action: "knowledge_list" }) },
        { lang: 'en', pattern: /(?:what|how\s+much)\s+(?:do\s+you|does\s+neeko)\s+know\s+(?:about\s+)?me/i, handler: () => ({ action: "knowledge_list" }) },
        { lang: 'es', pattern: /(?:guarda|recuerda|anota|acordate)\s+(?:que\s+)?(.+)/i, handler: (m) => ({ action: "knowledge_save_manual", text: m[1].trim() }) },
        { lang: 'en', pattern: /(?:save|remember|note|store)\s+(?:that\s+)?(.+)/i, handler: (m) => ({ action: "knowledge_save_manual", text: m[1].trim() }) },
        { lang: 'es', pattern: /(?:borra|elimina|olvida|limpia)\s+(?:la\s+)?memoria\s+(?:de\s+)?(.*)/i, handler: (m) => ({ action: "knowledge_delete_by_text", text: m[1].trim() }) },
        { lang: 'en', pattern: /(?:forget|delete|remove|clear)\s+(?:my\s+)?(?:memory|knowledge)\s*(?:of\s+)?(.*)/i, handler: (m) => ({ action: "knowledge_delete_by_text", text: m[1].trim() }) },
        { lang: 'es', pattern: /limpia(?:r)?\s+toda\s+(?:la\s+)?memoria/i, handler: () => ({ action: "knowledge_clear" }) },
        { lang: 'en', pattern: /clear\s+(?:all\s+)?(?:my\s+)?(?:memory|knowledge)/i, handler: () => ({ action: "knowledge_clear" }) },
    ];
    for (const { lang, pattern, handler } of knowledgePatterns) {
        if (lang && lang !== currentLanguage) continue;
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    return null;
}

async function executeAction(action) {
    if (!action) return null;
    try {
        // ─── Addon Actions ───
        if (action.action?.startsWith('addon:')) {
            const result = await NeekoAddons.executeAddonAction(action);
            if (result !== null) return result;
            return null;
        }

        // ─── Knowledge Actions ───
        if (action.action === 'knowledge_list') {
            const facts = JSON.parse(JSON.stringify(await invoke('knowledge_list')));
            if (!facts.length) return currentLanguage === 'en'
                ? 'I don\'t have anything saved about you yet. Tell me something and I\'ll remember it!'
                : 'No tengo nada guardado sobre vos aun. Decime algo y lo recuerdo! 🦎';
            let msg = currentLanguage === 'en' ? 'Here\'s what I know about you:\n' : 'Esto es lo que se de vos:\n';
            let cat = '';
            for (const f of facts) {
                if (f.category !== cat) {
                    cat = f.category;
                    msg += `\n${cat.charAt(0).toUpperCase() + cat.slice(1)}:\n`;
                }
                msg += `  ${f.key}: ${f.value}\n`;
            }
            return msg;
        }
        if (action.action === 'knowledge_save_manual') {
            const text = action.text;
            // Intentar parsear "X es Y" o "X = Y"
            const parts = text.match(/^(.+?)\s+(?:es|=|soy|tengo|uso|me gusta|trabajo en| vivo en)\s+(.+)$/i);
            let key, value;
            if (parts) {
                key = parts[1].trim();
                value = parts[2].trim();
            } else {
                key = 'info';
                value = text;
            }
            await invoke('knowledge_add', { category: 'general', key, value });
            await refreshKnowledgeContext();
            syncSystemPrompt();
            return currentLanguage === 'en'
                ? `Got it! I'll remember: ${key}: ${value} 🦎`
                + '\nYou can say "what do you know about me?" to see everything I have saved.'
                : `Listo! Guardado: ${key}: ${value} 🦎`
                + '\nDecime "que sabes de mi?" para ver todo lo que tengo guardado.';
        }
        if (action.action === 'knowledge_delete_by_text') {
            const query = action.text;
            const facts = JSON.parse(JSON.stringify(await invoke('knowledge_search', { query })));
            if (!facts.length) return currentLanguage === 'en'
                ? `I couldn't find anything matching "${query}" in my memory.`
                : `No encontré nada que coincida con "${query}" en mi memoria.`;
            for (const f of facts) {
                await invoke('knowledge_delete', { id: f.id });
            }
            await refreshKnowledgeContext();
            syncSystemPrompt();
            return currentLanguage === 'en'
                ? `Forgotten ${facts.length} thing(s) about "${query}" 🦎`
                : `Olvidé ${facts.length} cosa(s) sobre "${query}" 🦎`;
        }
        if (action.action === 'knowledge_clear') {
            await invoke('knowledge_clear');
            await refreshKnowledgeContext();
            syncSystemPrompt();
            return currentLanguage === 'en'
                ? 'Memory cleared! I don\'t remember anything about you now. 🦎'
                : 'Memoria limpiada! No me acuerdo de nada sobre vos ahora. 🦎';
        }

        switch (action.action) {
            case "get_ip":
                const localIP = await invoke('get_local_ip');
                const webPassword = await invoke('get_web_password');
                return `${t('connectingIp')} http://${localIP}:1414\n${t('webPassword')} ${webPassword}\n\n${t('phoneOpenAddress')} 🦎`;
            case "open_app":
                return await invoke('open_any_app', { appName: action.app });
            case "open_url":
                return await invoke('open_url', { url: action.url });
            case "search":
                return await invoke('search_web', { query: action.query });
            case "play_music":
                await invoke('open_url', { url: `https://www.youtube.com/results?search_query=${encodeURIComponent(action.query)}` });
                return `${t('openingYoutube')} ${action.query} 🎵`;
            case "open_folder":
                return await invoke('open_folder', { folder: action.folder });
            case "git_init":
                return await invoke('git_init', { path: action.path });
            case "git_add":
                return await invoke('git_add', { path: action.path, files: action.files });
            case "git_commit":
                return await invoke('git_commit', { path: action.path, message: action.message });
            case "git_full_push": {
                const p = action.path;
                const steps = [];
                try {
                    steps.push(await invoke('git_add', { path: p, files: "." }));
                } catch (e) { steps.push(`git add: ${e}`); }
                try {
                    steps.push(await invoke('git_commit', { path: p, message: "update from neeko" }));
                } catch (e) { steps.push(`git commit: ${e}`); }
                try {
                    steps.push(await invoke('git_push', { path: p }));
                } catch (e) { steps.push(`git push: ${e}`); }
                return steps.join("\n");
            }
            case "git_push":
                return await invoke('git_push', { path: action.path });
            case "git_pull":
                return await invoke('git_pull', { path: action.path });
            case "git_status":
                return await invoke('git_status', { path: action.path });
            case "git_log":
                return await invoke('git_log', { path: action.path, count: action.count });
            case "git_branch":
                return await invoke('git_branch', { path: action.path });
            case "git_remote_add":
                return await invoke('git_remote_add', { path: action.path, name: action.name, url: action.url });
            case "lol_match_history": {
                let region = action.region;
                let riotId = action.riot_id;
                try {
                    const config = JSON.parse(await invoke('lol_get_config'));
                    if (!region) region = config.lol_region || 'las';
                    if (!riotId) riotId = config.riot_id;
                } catch { }
                if (!riotId) return t('missingRiot');
                return await invoke('lol_get_match_history', { riotId, region, count: action.count || 5 });
            }
            case "lol_rank": {
                let region = action.region;
                let riotId = action.riot_id;
                try {
                    const config = JSON.parse(await invoke('lol_get_config'));
                    if (!region) region = config.lol_region || 'las';
                    if (!riotId) riotId = config.riot_id;
                } catch { }
                if (!riotId) return t('missingRiot');
                return await invoke('lol_get_rank', { riotId, region });
            }
            case "lol_save_config":
                return await invoke('lol_save_config', { gitPat: action.git_pat, region: action.region, gitPath: action.git_path, neekoSprite: null });
            case "compress_for_discord":
                return await invoke('compress_for_discord', { input: action.file });
            case "shutdown":
                return await invoke('system_shutdown', { seconds: action.seconds });
            case "cancel_shutdown":
                return await invoke('system_cancel_shutdown');
            case "restart_explorer":
                return await invoke('system_restart_explorer');
            case "restart_wifi":
                return await invoke('system_restart_wifi');
            case "restart_bluetooth":
                return await invoke('system_restart_bluetooth');
            default:
                return null;
        }
    } catch (error) {
        console.error('Error ejecutando acción:', error);
        return `${t('actionError')} ${error}`;
    }
}

async function callLocalAi(message) {
    conversationHistory.push({ role: "user", content: message });

    const messagesForModel = [...conversationHistory];
    const memoryReminder = await buildRuntimeMemoryReminder(message);
    if (memoryReminder) {
        messagesForModel.push({ role: "system", content: memoryReminder });
    }

    currentChatId = await invoke('chat_start', { messages: messagesForModel });

    let reply = cleanAiReply(await invoke('chat_finish', { requestId: currentChatId }));
    currentChatId = null;

    reply = await parseAndSaveKnowledge(reply);

    conversationHistory.push({ role: "assistant", content: reply });

    if (conversationHistory.length > 20) {
        conversationHistory = [conversationHistory[0], ...conversationHistory.slice(-18)];
    }

    return reply;
}

async function sendMessage() {
    const message = chatInput.value.trim();
    if (!message || isProcessing) return;

    sendBtn.textContent = '✕';
    sendBtn.classList.add('cancel-mode');
    isProcessing = true;
    clearTimeout(idleTimeout);
    currentAbortController = new AbortController();
    chatInput.value = '';

    try {
        await saveKnowledgeFromUserMessage(message);
    } catch (error) {
        console.error('[Knowledge] Error saving user message:', error);
    }

    setThinking(true);

    const detected = detectActionFromText(message);
    if (detected) {
        setTalking(true);
        showBubble(t('working'));
        const result = await executeAction(detected.action);
        if (result) showBubble(result);
        setTimeout(() => setTalking(false), 1200);
        isProcessing = false;
        currentAbortController = null;
        sendBtn.textContent = '➤';
        sendBtn.classList.remove('cancel-mode');
        setThinking(false);
        resetIdleTimer();
        return;
    }

    showBubble(t('thinking'));

    try {
        const llamaOn = await invoke('llama_status');
        if (!llamaOn) {
            showBubble(t('llamaOff'));
            isProcessing = false;
            currentAbortController = null;
            sendBtn.textContent = '➤';
            sendBtn.classList.remove('cancel-mode');
            setThinking(false);
            resetIdleTimer();
            return;
        }
        const reply = await callLocalAi(message);
        if (currentAbortController?.signal.aborted) return;

        let { action, message: neekoMsg } = parseNeekoResponse(reply);

        if (action) {
            const actionAllowedByLanguage = detectActionFromText(message);
            if (!actionAllowedByLanguage) {
                action = null;
                neekoMsg = t('commandLanguageMismatch');
            }
        }

        if (action) {
            setTalking(true);
            showBubble(neekoMsg || t('working'));
            const result = await executeAction(action);
            if (currentAbortController?.signal.aborted) return;
            if (result) {
                showBubble(result);
            } else if (neekoMsg) {
                showBubble(neekoMsg);
            }
        } else {
            setTalking(true);
            showBubble(neekoMsg || reply);
        }

        setTimeout(() => setTalking(false), 1200);
    } catch (error) {
        if (!isProcessing) return;
        console.error('Error:', error);
        setThinking(false);
        showBubble(t('processError'));
    }

    isProcessing = false;
    currentAbortController = null;
    sendBtn.textContent = '➤';
    sendBtn.classList.remove('cancel-mode');
    setThinking(false);
    resetIdleTimer();
}

function cancelRequest() {
    if (currentChatId) {
        invoke('chat_cancel', { requestId: currentChatId }).catch(() => { });
        currentChatId = null;
    }
    if (currentAbortController) {
        currentAbortController.abort();
        currentAbortController = null;
    }
    isProcessing = false;
    sendBtn.textContent = '➤';
    sendBtn.classList.remove('cancel-mode');
    setThinking(false);
    setTalking(false);
    showBubble(" cancelado ✋");
    resetIdleTimer();
}

async function init() {
    NeekoAddons.init();
    try {
        const config = JSON.parse(await invoke('lol_get_config'));
        setLanguage(config.language || 'es');
        applyNeekoSprite(config.neeko_sprite);
        neeko3dSelectedIdle = config.neeko_3d_animation || 'Neeko_idle3.anm';
        applyRender3D(config.render_3d);
    } catch {
        applyNeekoSprite(SPRITES.default);
        applyRender3D(false);
    }

    await refreshKnowledgeContext();
    resetSystemPrompt();

    try {
        const status = await invoke('check_local_ai');
        setLocalAiModelAvailable(true);
        const startOnLaunch = await invoke('get_llama_start_on_launch');

        if (status === "running") {
            showBubble(`${t('hello')} 🦎`);
        } else if (startOnLaunch) {
            showBubble("Iniciando llama-server... 🔍");
            await invoke('start_llama_server');
            showBubble(`${t('hello')} 🦎`);
        } else {
            showBubble(t('helloLlamaOff'));
        }
    } catch (error) {
        console.error('Init error:', error);
        if (error === "no_model") {
            setLocalAiModelAvailable(false);
            try {
                await invoke('set_llama_auto_start', { enabled: false });
            } catch { }
            showBubble(`${t('noModel')} 🦎`);
        } else {
            showBubble(`${t('hello')} 🦎`);
        }
    }

    try {
        await invoke('check_dependencies');
    } catch (error) {
        const missing = error.split(',');
        const msgs = [];
        if (missing.includes('git')) msgs.push("git (lo necesito para los comandos de repositorios)");
        if (missing.includes('ffmpeg')) msgs.push("ffmpeg (lo necesito para comprimir videos)");
        if (msgs.length) {
            setTimeout(() => {
                showBubble("⚠️ No tenés instalado: " + msgs.join('. ') + ".\nDescargalo para que pueda ayudarte con esas cosas 🦎");
            }, 3000);
        }
    }

    NeekoAddons.loadAddons();
    resetIdleTimer();
}

minimizeBtn.addEventListener('click', () => appWindow.minimize());
closeBtn.addEventListener('click', () => invoke('close_window'));
sendBtn.addEventListener('click', () => {
    if (isProcessing) {
        cancelRequest();
    } else {
        sendMessage();
    }
});
chatInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter' && !isProcessing) sendMessage();
});

// Settings Modal
const settingsBtn = document.getElementById('settings-btn');
const settingsModal = document.getElementById('settings-modal');
const settingsModalContent = settingsModal.querySelector('.modal-content');
const saveSettingsBtn = document.getElementById('save-settings-btn');
const closeSettingsBtn = document.getElementById('close-settings-btn');
const checkToolsBtn = document.getElementById('check-tools-btn');
const settingsMenuBtn = document.getElementById('settings-menu-btn');
const settingsMenuScrim = document.getElementById('settings-menu-scrim');
const toolStatusList = document.getElementById('tool-status-list');
const installFfmpegBtn = document.getElementById('install-ffmpeg-btn');
const installGitBtn = document.getElementById('install-git-btn');
const installModelBtn = document.getElementById('install-model-btn');
const installModelFileBtn = document.getElementById('install-model-file-btn');
const preparePythonEngineBtn = document.getElementById('prepare-python-engine-btn');
const uninstallFfmpegBtn = document.getElementById('uninstall-ffmpeg-btn');
const uninstallGitBtn = document.getElementById('uninstall-git-btn');
const uninstallModelBtn = document.getElementById('uninstall-model-btn');
const dependencyDownloadStatus = document.getElementById('dependency-download-status');
const dependencyDownloadLabel = document.getElementById('dependency-download-label');
const dependencyDownloadPercent = document.getElementById('dependency-download-percent');
const dependencyDownloadBar = document.getElementById('dependency-download-bar');
const dependencyDownloadMessage = document.getElementById('dependency-download-message');
const cancelDownloadBtn = document.getElementById('cancel-download-btn');
const pythonEngineStatus = document.getElementById('python-engine-status');
const pythonEngineLabel = document.getElementById('python-engine-label');
const pythonEnginePercent = document.getElementById('python-engine-percent');
const pythonEngineBar = document.getElementById('python-engine-bar');
const pythonEngineMessage = document.getElementById('python-engine-message');
const openAdvancedAiBtn = document.getElementById('open-advanced-ai-btn');
const backToAiBtn = document.getElementById('back-to-ai-btn');
const modelRuntimeHelpBtn = document.getElementById('model-runtime-help-btn');
const modelRuntimeHelp = document.getElementById('model-runtime-help');
const settingsTabs = Array.from(document.querySelectorAll('.settings-tab'));
const settingsPanels = Array.from(document.querySelectorAll('.settings-panel'));
const compactSettingsQuery = window.matchMedia('(max-width: 420px), (max-height: 620px)');

function setSettingsMenuOpen(open) {
    settingsModalContent.classList.toggle('settings-menu-open', open);
    settingsMenuBtn.setAttribute('aria-expanded', open ? 'true' : 'false');
}

function setSettingsTab(tabName) {
    settingsTabs.forEach((tab) => {
        const active = tab.dataset.settingsTab === tabName;
        tab.classList.toggle('active', active);
        tab.setAttribute('aria-selected', active ? 'true' : 'false');
    });

    settingsPanels.forEach((panel) => {
        const active = panel.dataset.settingsPanel === tabName;
        panel.classList.toggle('active', active);
        panel.hidden = !active;
    });

    setSettingsMenuOpen(false);
}

function setLocalAiModelAvailable(available) {
    localAiModelAvailable = available;

    const autoStartInput = document.getElementById('cfg-llama-autostart');
    if (autoStartInput) {
        autoStartInput.disabled = !available;
        if (!available) {
            autoStartInput.checked = false;
        }
    }
}

function readNumberInput(id, fallback, min, max) {
    const input = document.getElementById(id);
    const value = Number.parseInt(input?.value, 10);
    const normalized = Number.isFinite(value) ? value : fallback;
    return Math.max(min, Math.min(max, normalized));
}

function applyModelRuntimeConfig(config) {
    currentModelRuntimeConfig = {
        llamaGpuLayers: config?.llamaGpuLayers ?? 15,
        pythonGpuLayers: config?.pythonGpuLayers ?? 0,
        llamaContextSize: config?.llamaContextSize ?? 1024,
        pythonContextSize: config?.pythonContextSize ?? 4096,
        llamaThreads: config?.llamaThreads ?? 4,
        pythonThreads: config?.pythonThreads ?? 4,
    };

    document.getElementById('cfg-llama-gpu-layers').value = currentModelRuntimeConfig.llamaGpuLayers;
    document.getElementById('cfg-python-gpu-layers').value = currentModelRuntimeConfig.pythonGpuLayers;
    document.getElementById('cfg-llama-context-size').value = currentModelRuntimeConfig.llamaContextSize;
    document.getElementById('cfg-python-context-size').value = currentModelRuntimeConfig.pythonContextSize;
    document.getElementById('cfg-llama-threads').value = currentModelRuntimeConfig.llamaThreads;
    document.getElementById('cfg-python-threads').value = currentModelRuntimeConfig.pythonThreads;
}

function collectModelRuntimeConfig() {
    return {
        llamaGpuLayers: readNumberInput('cfg-llama-gpu-layers', 15, 0, 200),
        pythonGpuLayers: readNumberInput('cfg-python-gpu-layers', 0, 0, 200),
        llamaContextSize: readNumberInput('cfg-llama-context-size', 1024, 512, 32768),
        pythonContextSize: readNumberInput('cfg-python-context-size', 4096, 512, 32768),
        llamaThreads: readNumberInput('cfg-llama-threads', 4, 1, 64),
        pythonThreads: readNumberInput('cfg-python-threads', 4, 1, 64),
    };
}

function modelRuntimeConfigChanged(nextConfig) {
    return JSON.stringify(nextConfig) !== JSON.stringify(currentModelRuntimeConfig);
}

settingsTabs.forEach((tab) => {
    tab.addEventListener('click', () => {
        setSettingsTab(tab.dataset.settingsTab);
        if (tab.dataset.settingsTab === 'addons') renderAddonsList();
        if (tab.dataset.settingsTab === 'memory') initMemoryTab();
    });
});

openAdvancedAiBtn?.addEventListener('click', () => setSettingsTab('advanced'));
backToAiBtn?.addEventListener('click', () => setSettingsTab('ai'));
modelRuntimeHelpBtn?.addEventListener('click', () => {
    modelRuntimeHelp?.classList.toggle('hidden');
});

settingsMenuBtn.addEventListener('click', () => {
    setSettingsMenuOpen(!settingsModalContent.classList.contains('settings-menu-open'));
});

settingsMenuScrim.addEventListener('click', () => {
    setSettingsMenuOpen(false);
});

settingsModalContent.addEventListener('mousemove', (e) => {
    if (!compactSettingsQuery.matches || settingsModal.classList.contains('hidden')) return;

    const rect = settingsModalContent.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const menuIsOpen = settingsModalContent.classList.contains('settings-menu-open');

    if (x <= 12) {
        setSettingsMenuOpen(true);
        return;
    }

    if (menuIsOpen && x > 236) {
        setSettingsMenuOpen(false);
    }
});

settingsModalContent.addEventListener('mouseleave', () => {
    setSettingsMenuOpen(false);
});

function setInstallerButtonsDisabled(disabled) {
    [installFfmpegBtn, installGitBtn, installModelBtn, installModelFileBtn, preparePythonEngineBtn, uninstallFfmpegBtn, uninstallGitBtn, uninstallModelBtn].forEach((btn) => {
        if (btn) btn.disabled = disabled;
    });
}

function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB';
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function showDependencyProgress(payload) {
    if (!dependencyDownloadStatus) return;
    const percent = payload.percent ?? 0;
    dependencyDownloadStatus.classList.remove('hidden');
    dependencyDownloadLabel.textContent = payload.label || t('downloadLabel');
    dependencyDownloadPercent.textContent = payload.percent == null ? '...' : `${percent}%`;
    dependencyDownloadBar.style.width = `${Math.max(0, Math.min(100, percent))}%`;
    const totalText = payload.total ? ` · ${formatBytes(payload.downloaded)} / ${formatBytes(payload.total)} · faltan ${formatBytes(Math.max(0, payload.total - payload.downloaded))}` : '';
    dependencyDownloadMessage.textContent = `${payload.message || t('downloading')}${totalText}`;
}

function showPythonEngineProgress(payload) {
    if (!pythonEngineStatus) return;
    const percent = payload.percent ?? 0;
    pythonEngineStatus.classList.remove('hidden');
    pythonEngineLabel.textContent = payload.label || 'Motor Python';
    pythonEnginePercent.textContent = payload.percent == null ? '...' : `${percent}%`;
    pythonEngineBar.style.width = `${Math.max(0, Math.min(100, percent))}%`;
    pythonEngineMessage.textContent = payload.message || 'Preparando...';
}

settingsBtn.addEventListener('click', async () => {
    try {
        const config = JSON.parse(await invoke('lol_get_config'));
        settingsOriginalLanguage = normalizeLanguage(config.language || currentLanguage);
        setLanguage(settingsOriginalLanguage);
        document.getElementById('cfg-git-pat').value = '';
        document.getElementById('cfg-git-path').value = config.git_default_path || '';
        document.getElementById('cfg-neeko-sprite').value = normalizeNeekoSprite(config.neeko_sprite);
        document.getElementById('cfg-render-3d').checked = !!config.render_3d;
        document.getElementById('cfg-neeko-3d-animation').value = config.neeko_3d_animation || 'Neeko_idle3.anm';
        document.getElementById('cfg-mouse-tracking').checked = neeko3dMouseTracking;
        document.getElementById('cfg-language').value = normalizeLanguage(config.language || currentLanguage);
        document.getElementById('neeko-3d-animation-row').classList.toggle('hidden', !config.render_3d);
        document.getElementById('cfg-lol-region').value = config.lol_region || 'las';
        document.getElementById('cfg-riot-id').value = config.riot_id || '';
    } catch { }
    try {
        const running = await invoke('llama_status');
        updateLlamaUI(running && localAiModelAvailable);
    } catch { }
    try {
        currentModelLoadEngine = await invoke('get_model_load_engine');
        document.getElementById('cfg-model-load-engine').value = currentModelLoadEngine;
    } catch {
        currentModelLoadEngine = 'llama';
        document.getElementById('cfg-model-load-engine').value = currentModelLoadEngine;
    }
    try {
        applyModelRuntimeConfig(await invoke('get_model_runtime_config'));
    } catch {
        applyModelRuntimeConfig(null);
    }
    try {
        const autoStart = await invoke('get_llama_auto_start');
        const autoStartInput = document.getElementById('cfg-llama-autostart');
        autoStartInput.disabled = !localAiModelAvailable;
        autoStartInput.checked = autoStart && localAiModelAvailable;
    } catch { }
    try {
        const sysCmds = await invoke('get_system_commands_enabled');
        document.getElementById('cfg-system-cmds').checked = sysCmds;
    } catch { }
    settingsModal.classList.remove('hidden');
    setSettingsTab('general');
    setSettingsMenuOpen(false);
    toolStatusList.innerHTML = '';
});

function updateLlamaUI(running) {
    const state = document.getElementById('cfg-llama-state');
    const toggle = document.getElementById('cfg-llama-toggle');
    state.textContent = running ? `🟢 ${t('on')}` : `🔴 ${t('off')}`;
    state.style.color = running ? '#4ade80' : '#f87171';
    toggle.textContent = running ? t('turnOff') : t('turnOn');
    toggle.onclick = async () => {
        toggle.textContent = '...';
        toggle.disabled = true;
        try {
            try {
                await invoke('get_model_path_cmd');
                setLocalAiModelAvailable(true);
            } catch (error) {
                if (error === "no_model") {
                    setLocalAiModelAvailable(false);
                    if (running) {
                        await invoke('stop_llama_server').catch(() => { });
                    }
                    showBubble("No encontre el modelo GGUF. Instala la IA primero.");
                    updateLlamaUI(false);
                    toggle.disabled = false;
                    return;
                }
                throw error;
            }

            if (running) {
                await invoke('stop_llama_server');
                showBubble("LLaMA apagado 🦎");
                updateLlamaUI(false);
            } else {
                showBubble("Encendiendo LLaMA... 🦎");
                await invoke('start_llama_server');
                showBubble("LLaMA encendido 🦎");
                updateLlamaUI(true);
            }
        } catch (e) {
            showBubble("Error: " + e);
        }
        toggle.disabled = false;
    };
}

function renderToolStatuses(statuses) {
    toolStatusList.innerHTML = '';
    statuses.forEach((tool) => {
        const item = document.createElement('div');
        item.className = `tool-status ${tool.ok ? 'tool-ok' : 'tool-error'}`;

        const name = document.createElement('strong');
        name.textContent = `${tool.ok ? 'OK' : t('missing')} ${tool.name}`;

        const detail = document.createElement('span');
        detail.textContent = `${tool.command}: ${tool.detail || t('noDetail')}`;

        item.append(name, detail);
        toolStatusList.appendChild(item);
    });
}

async function saveEnvironmentConfig() {
    await invoke('save_environment_config', { ffmpegPath: null, ffprobePath: null });
}

async function checkEnvironmentTools(showMessage = true) {
    if (!toolStatusList) return;
    toolStatusList.innerHTML = `<div class="tool-status checking">${t('checkingTools')}</div>`;
    checkToolsBtn.disabled = true;
    try {
        await saveEnvironmentConfig();
        const statuses = await invoke('check_environment_tools');
        renderToolStatuses(statuses);
        if (showMessage) {
            const missing = statuses.filter((tool) => !tool.ok).map((tool) => tool.name);
            showBubble(missing.length ? `${t('missingConfig')} ${missing.join(', ')}` : t('toolsReady'));
        }
    } catch (e) {
        toolStatusList.innerHTML = `<div class="tool-status tool-error">No pude probar herramientas: ${e}</div>`;
    }
    checkToolsBtn.disabled = false;
}

document.getElementById('cfg-render-3d').addEventListener('change', (e) => {
    document.getElementById('neeko-3d-animation-row').classList.toggle('hidden', !e.target.checked);
});

document.getElementById('cfg-language').addEventListener('change', (e) => {
    setLanguage(e.target.value);
});

closeSettingsBtn.addEventListener('click', () => {
    setLanguage(settingsOriginalLanguage);
    setSettingsMenuOpen(false);
    settingsModal.classList.add('hidden');
});

checkToolsBtn.addEventListener('click', () => {
    setSettingsTab('tools');
    checkEnvironmentTools(true);
});

if (window.__TAURI__.event?.listen) {
    window.__TAURI__.event.listen('dependency-download-progress', (event) => {
        showDependencyProgress(event.payload || {});
    });
    window.__TAURI__.event.listen('python-engine-progress', (event) => {
        showPythonEngineProgress(event.payload || {});
    });
}

async function runInstaller(button, command, args = {}) {
    setInstallerButtonsDisabled(true);
    dependencyDownloadStatus.classList.remove('hidden');
    dependencyDownloadLabel.textContent = button.textContent;
    dependencyDownloadPercent.textContent = '...';
    dependencyDownloadBar.style.width = '0%';
    dependencyDownloadMessage.textContent = t('preparing');
    cancelDownloadBtn.style.display = 'inline-block';
    cancelDownloadBtn.dataset.downloadId = command.includes('ffmpeg') ? 'ffmpeg' : command.includes('git') ? 'git' : command.includes('model') ? 'model' : '';
    try {
        const message = await invoke(command, args);
        dependencyDownloadPercent.textContent = '100%';
        dependencyDownloadBar.style.width = '100%';
        dependencyDownloadMessage.textContent = message;
        if (command === 'install_model' || command === 'install_model_from_file') {
            setLocalAiModelAvailable(true);
            document.getElementById('cfg-llama-autostart').checked = true;
            updateLlamaUI(false);
        }
        showBubble(message);
        await checkEnvironmentTools(false);
    } catch (e) {
        dependencyDownloadMessage.textContent = 'Error: ' + e;
        showBubble('Error: ' + e);
    }
    cancelDownloadBtn.style.display = 'none';
    setInstallerButtonsDisabled(false);
}

cancelDownloadBtn.addEventListener('click', async () => {
    const id = cancelDownloadBtn.dataset.downloadId;
    if (!id) return;
    try {
        await invoke('cancel_download', { id });
        showBubble('Cancelando...');
    } catch (e) {
        showBubble('Error: ' + e);
    }
});

installFfmpegBtn.addEventListener('click', () => {
    runInstaller(installFfmpegBtn, 'install_ffmpeg');
});

installGitBtn.addEventListener('click', () => {
    runInstaller(installGitBtn, 'install_git');
});

installModelBtn.addEventListener('click', () => {
    runInstaller(installModelBtn, 'install_model', { modelUrl: 'https://drive.google.com/file/d/1eHu-UkJ0cdK35kvpt9YPoNBVPtuTgGHe/view?usp=drive_link' });
});

preparePythonEngineBtn?.addEventListener('click', async () => {
    setInstallerButtonsDisabled(true);
    showPythonEngineProgress({ label: 'Motor Python', percent: 0, message: 'Preparando...' });
    try {
        const message = await invoke('prepare_python_engine');
        showPythonEngineProgress({ label: 'Motor Python', percent: 100, message });
        showBubble(message);
    } catch (e) {
        showPythonEngineProgress({ label: 'Motor Python', percent: null, message: 'Error: ' + e });
        showBubble('Error: ' + e);
    }
    setInstallerButtonsDisabled(false);
});

document.getElementById('install-model-browser-btn').addEventListener('click', () => {
    invoke('open_url', { url: 'https://drive.google.com/file/d/1eHu-UkJ0cdK35kvpt9YPoNBVPtuTgGHe/view' });
    showBubble('Abri el navegador para descargar el modelo');
});

installModelFileBtn.addEventListener('click', async () => {
    try {
        const sourcePath = await invoke('pick_model_file');
        if (!sourcePath) return;
        runInstaller(installModelFileBtn, 'install_model_from_file', { sourcePath });
    } catch (e) {
        showBubble('Error: ' + e);
    }
});

async function runUninstaller(button, command, confirmation) {
    if (!confirm(confirmation)) return;
    setInstallerButtonsDisabled(true);
    dependencyDownloadStatus.classList.remove('hidden');
    dependencyDownloadLabel.textContent = button.textContent;
    dependencyDownloadPercent.textContent = '';
    dependencyDownloadBar.style.width = '0%';
    dependencyDownloadMessage.textContent = 'Desinstalando...';
    try {
        const message = await invoke(command);
        dependencyDownloadMessage.textContent = message;
        if (command === 'uninstall_model') {
            setLocalAiModelAvailable(false);
            document.getElementById('cfg-llama-autostart').checked = false;
            updateLlamaUI(false);
        }
        showBubble(message);
        await checkEnvironmentTools(false);
    } catch (e) {
        dependencyDownloadMessage.textContent = 'Error: ' + e;
        showBubble('Error: ' + e);
    }
    setInstallerButtonsDisabled(false);
}

uninstallFfmpegBtn.addEventListener('click', () => {
    runUninstaller(uninstallFfmpegBtn, 'uninstall_ffmpeg', '¿Eliminar FFmpeg y FFprobe descargados por Neeko?');
});

uninstallGitBtn.addEventListener('click', () => {
    runUninstaller(uninstallGitBtn, 'uninstall_git', 'Git se desinstala desde Windows. ¿Abrir Apps instaladas?');
});

uninstallModelBtn.addEventListener('click', () => {
    runUninstaller(uninstallModelBtn, 'uninstall_model', '¿Eliminar los modelos GGUF descargados por Neeko?');
});

settingsModal.addEventListener('click', (e) => {
    if (e.target === settingsModal) {
        setLanguage(settingsOriginalLanguage);
        setSettingsMenuOpen(false);
        settingsModal.classList.add('hidden');
    }
});

saveSettingsBtn.addEventListener('click', async () => {
    const pat = document.getElementById('cfg-git-pat').value.trim();
    const gitPath = document.getElementById('cfg-git-path').value.trim();
    const neekoSpriteValue = normalizeNeekoSprite(document.getElementById('cfg-neeko-sprite').value);
    const render3d = document.getElementById('cfg-render-3d').checked;
    const region = document.getElementById('cfg-lol-region').value;
    const riotId = document.getElementById('cfg-riot-id').value.trim();
    const language = normalizeLanguage(document.getElementById('cfg-language').value);
    const modelLoadEngine = document.getElementById('cfg-model-load-engine').value;
    const modelRuntimeConfig = collectModelRuntimeConfig();
    const autoStartInput = document.getElementById('cfg-llama-autostart');
    let autoStart = autoStartInput.checked;

    try {
        await invoke('lol_save_config', {
            gitPat: pat || null,
            gitPath: gitPath || null,
            neekoSprite: neekoSpriteValue,
            region: region || null,
            riotId: riotId || null,
            language,
        });
        applyNeekoSprite(neekoSpriteValue);
        setLanguage(language);
        await refreshKnowledgeContext();
        resetSystemPrompt();
        settingsOriginalLanguage = language;
    } catch (e) {
        showBubble("Error guardando config: " + e);
    }
    try {
        await invoke('set_render_3d', { enabled: render3d });
        applyRender3D(render3d);
    } catch (e) {
        showBubble("Error guardando render 3D: " + e);
    }
    try {
        const anim = document.getElementById('cfg-neeko-3d-animation').value;
        await invoke('set_neeko_3d_animation', { animation: anim });
        neeko3dSelectedIdle = anim;
        syncNeeko3dAnimation();
    } catch (e) {
        showBubble("Error guardando animacion 3D: " + e);
    }
    neeko3dMouseTracking = document.getElementById('cfg-mouse-tracking').checked;
    try {
        await invoke('save_environment_config', { ffmpegPath: null, ffprobePath: null });
    } catch (e) {
        showBubble("Error guardando herramientas: " + e);
        return;
    }
    try {
        const engineChanged = modelLoadEngine !== currentModelLoadEngine;
        const runtimeChanged = modelRuntimeConfigChanged(modelRuntimeConfig);
        const wasRunning = await invoke('llama_status').catch(() => false);
        await invoke('set_model_load_engine', { engine: modelLoadEngine });
        await invoke('set_model_runtime_config', modelRuntimeConfig);
        currentModelLoadEngine = modelLoadEngine;
        currentModelRuntimeConfig = modelRuntimeConfig;

        if ((engineChanged || runtimeChanged) && wasRunning) {
            showBubble("Cambiando motor de carga...");
            await invoke('stop_llama_server');
            await invoke('start_llama_server');
            updateLlamaUI(true);
        }
    } catch (e) {
        showBubble("Error guardando motor de carga: " + e);
        return;
    }
    try {
        if (autoStart && !localAiModelAvailable) {
            autoStart = false;
            autoStartInput.checked = false;
            showBubble("No encontre el modelo GGUF. Auto-iniciar LLaMA queda apagado.");
        }
        if (false && autoStart) {
            try {
                await invoke('get_model_path_cmd');
            } catch (error) {
                if (error === "no_model") {
                    autoStart = false;
                    autoStartInput.checked = false;
                    showBubble("No encontrÃ© el modelo GGUF. Auto-iniciar LLaMA queda apagado.");
                } else {
                    throw error;
                }
            }
        }
        await invoke('set_llama_auto_start', { enabled: autoStart });
    } catch (e) { }
    try {
        const sysCmds = document.getElementById('cfg-system-cmds').checked;
        await invoke('set_system_commands_enabled', { enabled: sysCmds });
    } catch (e) { }
    showBubble(`${t('saved')} ✅`);
    setSettingsMenuOpen(false);
    settingsModal.classList.add('hidden');
    resetIdleTimer();
});

const checkUpdateBtn = document.getElementById('check-update-btn');
const applyUpdateBtn = document.getElementById('apply-update-btn');
const updateStatus = document.getElementById('update-status');
const updateNotes = document.getElementById('update-notes');

function normalizeUpdateResult(value) {
    if (typeof value === 'string') {
        return JSON.parse(value);
    }
    return value;
}

function formatUpdateError(error) {
    const raw = error instanceof Error ? error.message : (typeof error === 'string' ? error : (() => {
        try {
            return JSON.stringify(error);
        } catch {
            return String(error);
        }
    })());
    if (/signature verification failed/i.test(raw)) {
        return 'La firma de la actualizacion no coincide. Instala manualmente la ultima version una vez; despues las actualizaciones automaticas van a funcionar.';
    }
    return raw;
}

checkUpdateBtn.addEventListener('click', async () => {
    checkUpdateBtn.disabled = true;
    checkUpdateBtn.textContent = t('searching');
    updateNotes.textContent = '';
    try {
        const result = normalizeUpdateResult(await invoke('check_updates'));
        if (result.hasUpdate) {
            updateStatus.textContent = `Nueva versión: v${result.latestVersion} (actual: v${result.currentVersion})`;
            updateStatus.style.color = '#4ade80';
            applyUpdateBtn.style.display = 'inline-block';
            if (result.notes) updateNotes.textContent = result.notes;
            showBubble(`Hay una actualización: v${result.latestVersion}`);
        } else {
            updateStatus.textContent = `Estás en la última versión (v${result.currentVersion})`;
            updateStatus.style.color = '#aab';
            applyUpdateBtn.style.display = 'none';
            showBubble('Ya tienes la última versión');
        }
    } catch (e) {
        const message = formatUpdateError(e);
        updateStatus.textContent = 'Error: ' + message;
        updateStatus.style.color = '#f87171';
        showBubble('Error: ' + message);
    }
    checkUpdateBtn.disabled = false;
    checkUpdateBtn.textContent = t('checkUpdates');
});

applyUpdateBtn.addEventListener('click', async () => {
    applyUpdateBtn.disabled = true;
    applyUpdateBtn.textContent = t('downloading');
    updateNotes.textContent = 'La app se reiniciará automáticamente al instalar.';
    try {
        const message = await invoke('download_and_install_update');
        showBubble(message);
    } catch (e) {
        updateNotes.textContent = 'Error: ' + e;
        showBubble('Error: ' + e);
        applyUpdateBtn.disabled = false;
        applyUpdateBtn.textContent = t('updateRestart');
    }
});

init();
