import { ChatClient, confirmationControls, confirmationText, attachInfo } from './chat-client.mjs';
import { PetRegion } from './pet-region.mjs';
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
const settingsWindow = appWindow.label === 'settings';
if (settingsWindow) {
    document.documentElement.classList.add('settings-window');
    document.getElementById('settings-modal').classList.remove('hidden');
}
document.addEventListener('pointerdown', event => {
    if (event.button !== 0) return;
    if (event.target.closest('button, input, select, textarea, a')) return;
    const header = settingsWindow && document.querySelector('#settings-modal:not(.hidden) .modal-header');
    // Only the strip above the header is draggable, not the empty space beside its title.
    const inTopStrip = header && event.target.closest('#settings-modal')
        && event.clientY >= 0 && event.clientY < header.getBoundingClientRect().top;
    if (!inTopStrip && !event.target.closest('.modal-header h3, #top-bar')) return;
    void appWindow.startDragging().catch(console.error);
});
const petRegion = new PetRegion(settingsWindow ? async () => {} : invoke);

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
setInterval(() => {
    if (!petRegion.enabled || !neekoImg.complete || !neekoImg.naturalWidth || !neekoImg.getClientRects().length) return;
    if (getComputedStyle(neekoImg).visibility === 'hidden') return;
    const rect = neekoImg.getBoundingClientRect();
    const scale = Math.min(rect.width / neekoImg.naturalWidth, rect.height / neekoImg.naturalHeight);
    const width = neekoImg.naturalWidth * scale, height = neekoImg.naturalHeight * scale;
    petRegion.capture(neekoImg, { x: rect.x + (rect.width - width) / 2, y: rect.bottom - height, width, height });
}, 60);

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
                    const owner = NeekoAddons._loadingAddonId;
                    const previous = NeekoAddons._commands.get(id);
                    if (previous && previous._owner !== owner) throw new Error(`Duplicate addon command: ${id}`);
                    NeekoAddons._commands.set(id, { ...config, _owner: owner });
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
                setDesktopHitRegion: (v) => petRegion.enable(v),
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
            await invoke('assistant_addon_loaded', { addonId, commands: NeekoAddons._loaded.has(addonId) ? [...record.commands].filter(id => typeof NeekoAddons._commands.get(id)?.aiHandler === 'function') : [] });
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
        invoke('assistant_addon_loaded', { addonId, commands: [] }).catch(console.error);
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
        startWithWindows: 'Iniciar con Windows',
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
        startWithWindows: 'Start with Windows',
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
                    if (settingsWindow) await window.__TAURI__.event.emitTo('main', 'neeko:addons-changed');
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
    if (settingsWindow) return;
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

    // Native desktop coordinates keep working over other apps and transparent
    // parts of the pet window. Convert physical pixels before aiming the head.
    let stopped = false;
    let cursorTimer;
    let cursorErrorReported = false;
    const trackCursor = async () => {
        try {
            if (neeko3dMouseTracking && neeko3dRendered) {
                const [cursor, origin, scale] = await Promise.all([
                    window.__TAURI__.window.cursorPosition(),
                    appWindow.innerPosition(), appWindow.scaleFactor()
                ]);
                if (stopped) return;
                const bounds = neekoSprite.getBoundingClientRect();
                const x = (cursor.x - origin.x) / scale;
                const y = (cursor.y - origin.y) / scale;
                neeko3dMouseX = Math.max(-1, Math.min(1,
                    (x - bounds.left - bounds.width / 2) / Math.max(1, bounds.width / 2)));
                neeko3dMouseY = Math.max(-1, Math.min(1,
                    (y - bounds.top - bounds.height / 2) / Math.max(1, bounds.height / 2)));
                cursorErrorReported = false;
            }
        } catch (error) {
            if (!cursorErrorReported) console.warn('No se pudo consultar el cursor del escritorio:', error);
            cursorErrorReported = true;
        } finally {
            if (!stopped) cursorTimer = setTimeout(trackCursor, 50);
        }
    };
    void trackCursor();
    neeko3dIdleCleanup = () => {
        stopped = true;
        clearTimeout(cursorTimer);
    };

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
            const desktopPet = document.documentElement.classList.contains('airi-pet');
            const talkSway = !desktopPet && talking ? Math.sin(seconds * 7) * 0.025 : 0;
            neeko3dModel.rotation.y = -0.12 + (desktopPet ? 0 : Math.sin(seconds * 0.8) * 0.035);
            neeko3dModel.position.y = neeko3dModelBaseY + (desktopPet ? 0 : Math.sin(seconds * 1.2) * 0.025) + talkSway;

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
            // Read alpha in the same frame, before WebGL clears its drawing buffer.
            petRegion.capture(neeko3dRenderer.domElement, neeko3d.getBoundingClientRect(), time);
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
    return currentLanguage === 'en' ? 'You are Neeko. Reply briefly in English.' : 'Sos Neeko. Responde brevemente en espanol.';
}

// Legacy addon conversation access; app chat context is assembled once in Rust.
let conversationHistory = [];
async function refreshKnowledgeContext() {}

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
    if (settingsWindow) return;
    clearTimeout(idleTimeout);
    idleTimeout = setTimeout(showIdlePhrase, 20000);
}

function showIdlePhrase() {
    if (isProcessing) return;
    const phrases = t('idle') || idlePhrases;
    const phrase = phrases[Math.floor(Math.random() * phrases.length)];
    showBubble(`${phrase} 🦎`);
    resetIdleTimer();
}

function showBubble(text) {
    if (settingsWindow) {
        let notice = document.getElementById('settings-notice');
        if (!notice) {
            notice = document.createElement('p');
            notice.id = 'settings-notice';
            notice.setAttribute('role', 'status');
            settingsModalContent.querySelector('.modal-header').after(notice);
        }
        notice.textContent = text;
        return;
    }
    speechBubble.querySelectorAll('.assistant-info-trigger').forEach(button => button.remove());
    bubbleText.textContent = text;
    speechBubble.classList.remove('hidden');
}

function confirmChatAction(proposal, signal) {
    setThinking(false);
    showBubble(confirmationText(currentLanguage));
    const yes = document.getElementById('action-yes');
    yes.textContent = currentLanguage === 'en' ? 'Yes' : '\u0053\u00ed';
    return confirmationControls(
        document.getElementById('action-confirmation'), document.getElementById('action-details'),
        yes, document.getElementById('action-no'), proposal, signal, currentLanguage, url => invoke('open_url', { url })
    );
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

const assistantClient = new ChatClient({
    chat: (session, messages) => invoke('assistant_chat', { session: `desktop:${session}`, messages }),
    decide: (session, proposalId, approved, saveAccount = false) => invoke('assistant_decide', { session: `desktop:${session}`, proposalId, approved, saveAccount }),
    cancel: (session) => invoke('assistant_cancel', { session: `desktop:${session}` }),
}, {
    busy: (busy) => {
        isProcessing = busy;
        sendBtn.textContent = busy ? '\u2715' : '\u27a4';
        sendBtn.classList.toggle('cancel-mode', busy);
        setThinking(busy);
        if (busy) { clearTimeout(idleTimeout); showBubble(t('thinking')); }
        else { setThinking(false); resetIdleTimer(); }
    },
    confirm: confirmChatAction,
    executing: (approved) => { if (approved) showBubble(t('working')); },
    message: (text, info) => { showBubble(text); attachInfo(speechBubble, info, currentLanguage, url => invoke('open_url', { url })); setTalking(true); setTimeout(() => setTalking(false), 1200); },
    error: (text) => showBubble(text),
});

async function sendMessage() {
    const message = chatInput.value.trim();
    if (!message || assistantClient.busy) return;
    chatInput.value = '';
    await assistantClient.send(message);
    conversationHistory = assistantClient.history;
}

async function cancelRequest() {
    try { await assistantClient.cancel(); showBubble(currentLanguage === 'en' ? 'Cancelled.' : 'Cancelado.'); }
    catch (error) { showBubble(String(error)); }
    setTalking(false);
}

async function init() {
    await appWindow.setAlwaysOnTop(false);
    NeekoAddons.init();
    window.Neeko.ui.isSettingsWindow = settingsWindow;
    if (settingsWindow) {
        settingsBtn.click();
        try { await invoke('check_local_ai'); setLocalAiModelAvailable(true); }
        catch { setLocalAiModelAvailable(false); }
        await NeekoAddons.loadAddons();
        return;
    }
    await window.__TAURI__.event.listen('neeko:addons-changed', async () => {
        const addons = await invoke('addon_list');
        for (const addon of addons) {
            if (!addon.enabled) NeekoAddons.unloadAddon(addon.manifest.id);
            else await NeekoAddons.loadAddon(addon);
        }
    });
    await window.__TAURI__.event.listen('neeko:settings-saved', async ({ payload }) => {
        const config = JSON.parse(await invoke('lol_get_config'));
        setLanguage(config.language || 'es');
        applyNeekoSprite(config.neeko_sprite);
        neeko3dSelectedIdle = config.neeko_3d_animation || 'Neeko_idle3.anm';
        applyRender3D(config.render_3d !== false);
        neeko3dMouseTracking = payload.mouseTracking;
        await refreshKnowledgeContext();
        resetSystemPrompt();
    });
    await window.__TAURI__.event.listen('neeko:approved-addon', async ({ payload: requestId }) => {
        try {
            const action = await invoke('assistant_addon_claim', { requestId });
            const [, addonId, commandId] = action.action.split(':');
            const command = NeekoAddons._commands.get(commandId);
            if (!NeekoAddons._loaded.get(addonId)?.commands.has(commandId) || !command?.aiHandler || command._owner !== addonId) {
                throw new Error('Addon unavailable');
            }
            const { action: name, ...params } = action;
            const result = await command.aiHandler(params);
            await invoke('assistant_addon_result', { requestId, result: typeof result === 'string' ? result : String(result?.message || ''), error: null });
        } catch (error) {
            await invoke('assistant_addon_result', { requestId, result: null, error: String(error) });
        }
    });
    try {
        const config = JSON.parse(await invoke('lol_get_config'));
        setLanguage(config.language || 'es');
        applyNeekoSprite(config.neeko_sprite);
        neeko3dSelectedIdle = config.neeko_3d_animation || 'Neeko_idle3.anm';
        applyRender3D(config.render_3d !== false);
    } catch {
        applyNeekoSprite(SPRITES.default);
        neeko3dSelectedIdle = 'Neeko_idle3.anm';
        applyRender3D(true);
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
        llamaContextSize: config?.llamaContextSize ?? 8192,
        pythonContextSize: config?.pythonContextSize ?? 8192,
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
        llamaContextSize: readNumberInput('cfg-llama-context-size', 8192, 512, 32768),
        pythonContextSize: readNumberInput('cfg-python-context-size', 8192, 512, 32768),
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

async function loadStartWithWindowsSetting() {
    const input = document.getElementById('cfg-start-with-windows');
    if (!input) return;

    input.disabled = false;
    input.checked = await invoke('get_start_with_windows');
}

async function saveStartWithWindowsSetting() {
    const input = document.getElementById('cfg-start-with-windows');
    if (!input || input.disabled) return;

    await invoke('set_start_with_windows', { enabled: input.checked });
}

async function refreshAppVersionStatus() {
    if (!updateStatus) return;

    try {
        const version = await invoke('get_app_version');
        updateStatus.textContent = `Version actual: v${version}`;
        updateStatus.style.color = '#aab';
    } catch {
        updateStatus.textContent = 'Version actual: ...';
    }
}

settingsBtn.addEventListener('click', async () => {
    if (!settingsWindow) {
        await invoke('open_settings_window');
        return;
    }
    try {
        const config = JSON.parse(await invoke('lol_get_config'));
        settingsOriginalLanguage = normalizeLanguage(config.language || currentLanguage);
        setLanguage(settingsOriginalLanguage);
        document.getElementById('cfg-git-pat').value = '';
        document.getElementById('cfg-git-path').value = config.git_default_path || '';
        document.getElementById('cfg-neeko-sprite').value = normalizeNeekoSprite(config.neeko_sprite);
        const render3dEnabled = config.render_3d !== false;
        document.getElementById('cfg-render-3d').checked = render3dEnabled;
        document.getElementById('cfg-neeko-3d-animation').value = config.neeko_3d_animation || 'Neeko_idle3.anm';
        document.getElementById('cfg-mouse-tracking').checked = neeko3dMouseTracking;
        document.getElementById('cfg-language').value = normalizeLanguage(config.language || currentLanguage);
        document.getElementById('neeko-3d-animation-row').classList.toggle('hidden', !render3dEnabled);
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
    try {
        await loadStartWithWindowsSetting();
    } catch {
        const startWithWindowsInput = document.getElementById('cfg-start-with-windows');
        if (startWithWindowsInput) {
            startWithWindowsInput.checked = false;
            startWithWindowsInput.disabled = true;
        }
    }
    await refreshAppVersionStatus();
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
    if (settingsWindow) void invoke('close_window');
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
        if (settingsWindow) return;
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
    try {
        await saveStartWithWindowsSetting();
    } catch (e) {
        showBubble("Error guardando inicio con Windows: " + e);
        return;
    }
    showBubble(`${t('saved')} ✅`);
    if (settingsWindow) {
        await window.__TAURI__.event.emitTo('main', 'neeko:settings-saved', { mouseTracking: neeko3dMouseTracking });
        await invoke('close_window');
    }
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

init().catch(error => {
    console.error('No se pudo iniciar Neeko:', error);
    if (settingsWindow) showBubble('No se pudo cargar Configuración: ' + String(error));
});
