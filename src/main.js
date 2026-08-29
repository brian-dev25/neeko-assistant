const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const appWindow = getCurrentWindow();

const neekoSprite = document.getElementById('neeko-sprite');
const neekoImg = document.getElementById('neeko-img');
const speechBubble = document.getElementById('speech-bubble');
const bubbleText = document.getElementById('bubble-text');
const chatInput = document.getElementById('chat-input');
const sendBtn = document.getElementById('send-btn');
const minimizeBtn = document.getElementById('minimize-btn');
const closeBtn = document.getElementById('close-btn');

let isProcessing = false;
let currentAbortController = null;
let localAiModelAvailable = false;

const SPRITES = {
    default: "NEEKO.png",
    standing: "NEEKO-standing-costume.png",
    sitting: "NEEKO-sitting.png",
};

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

function getSystemPrompt() {
    const now = new Date();
    const options = { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' };
    const fecha = now.toLocaleDateString('es-AR', options);
    const hora = now.toLocaleTimeString('es-AR', { hour: '2-digit', minute: '2-digit' });

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

Si no es una acción, respondé normal como Neeko.`;
}

let conversationHistory = [
    { role: "system", content: getSystemPrompt() }
];

let currentChatId = null;

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
    const phrase = idlePhrases[Math.floor(Math.random() * idlePhrases.length)];
    showBubble(phrase);
    resetIdleTimer();
}

function showBubble(text) {
    bubbleText.textContent = text;
    speechBubble.classList.remove('hidden');
}

function setTalking(talking) {
    neekoSprite.classList.toggle('talking', talking);
}

function setThinking(thinking) {
    neekoSprite.classList.toggle('thinking', thinking);
}

function parseNeekoResponse(text) {
    const jsonMatch = text.match(/\{[\s\S]*?"action"[\s\S]*?\}/);
    if (jsonMatch) {
        try {
            const action = JSON.parse(jsonMatch[0]);
            const afterJson = text.substring(text.indexOf(jsonMatch[0]) + jsonMatch[0].length);
            const message = afterJson.replace(/^\|+/, '').trim();
            return { action, message };
        } catch {}
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

    // ─── IP Detection ───
    if (lower === 'ip' || lower === 'mi ip' || lower === 'la ip' || (lower.includes('ip') && (lower.includes('cuál') || lower.includes('cual') || lower.includes('que') || lower.includes('cómo') || lower.includes('como') || lower.includes('connect') || lower.includes('conect') || lower.includes('dirección') || lower.includes('direccion')))) {
        return { action: { action: "get_ip" }, message: "" };
    }

    // ─── Git Detection (check before app detection) ───
    const gitPatterns = [
        { pattern: /git\s+init(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_init", path: m[1] || null }) },
        { pattern: /inicializ(?:ar|a)\s+(?:un\s+)?repo(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_init", path: m[1] || null }) },

        { pattern: /git\s+add\s+(.+?)(?:\s+en\s+(.+))?$/i, handler: (m) => ({ action: "git_add", files: m[1].trim(), path: m[2] || null }) },
        { pattern: /(?:agregar|añadir|agreg(?:a|o))\s+(.+?)(?:\s+al\s+repo|\s+en\s+(.+))?$/i, handler: (m) => ({ action: "git_add", files: m[1].trim(), path: m[2] || null }) },

        { pattern: /git\s+commit\s+(?:-m\s+)?["']?(.+?)["']?$/i, handler: (m) => ({ action: "git_commit", path: null, message: m[1].trim() }) },
        { pattern: /haz\s+commit\s+(?:con\s+)?(?:mensaje\s+)?["']?(.+?)["']?$/i, handler: (m) => ({ action: "git_commit", path: null, message: m[1].trim() }) },
        { pattern: /commitea(?:r)?\s+(?:con\s+)?(?:mensaje\s+)?["']?(.+?)["']?$/i, handler: (m) => ({ action: "git_commit", path: null, message: m[1].trim() }) },

        { pattern: /sub(?:e|ir)\s+(?:mi\s+)?repo(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_full_push", path: m[1] || null }) },
        { pattern: /git\s+push(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_push", path: m[1] || null }) },
        { pattern: /pushe(?:a|r?)\s+(?:el\s+)?repo/i, handler: () => ({ action: "git_full_push", path: null }) },

        { pattern: /git\s+pull(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_pull", path: m[1] || null }) },
        { pattern: /baj(?:a|ar)\s+(?:los?\s+)?cambios?\s+(?:en\s+(.+))?/i, handler: (m) => ({ action: "git_pull", path: m[1] || null }) },

        { pattern: /git\s+status(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_status", path: m[1] || null }) },
        { pattern: /estado\s+(?:del\s+)?repo(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_status", path: m[1] || null }) },

        { pattern: /git\s+log(?:(?:\s+ultim(?:os?|as?)\s+)?(\d+))?(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_log", count: m[1] ? parseInt(m[1]) : 10, path: m[2] || null }) },
        { pattern: /(?:últim(?:os?|as?)\s+)?commits?(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_log", count: 10, path: m[1] || null }) },

        { pattern: /git\s+branch(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_branch", path: m[1] || null }) },
        { pattern: /(?:que\s+)?branches?\s+tien(?:e|es?)\s+(?:en\s+(.+))?/i, handler: (m) => ({ action: "git_branch", path: m[1] || null }) },

        { pattern: /git\s+remote\s+add\s+(\S+)\s+(\S+)(?:\s+en\s+(.+))?/i, handler: (m) => ({ action: "git_remote_add", name: m[1], url: m[2], path: m[3] || null }) },
    ];

    for (const { pattern, handler } of gitPatterns) {
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── LOL Detection ───
    const lolPatterns = [
        // With name#tag and optional region
        { pattern: /(?:ultima|última)\s+partida\s+(?:de\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|jp|oce|tr|ru))?$/i,
          handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: m[3]?.toLowerCase() || null, count: 1 }) },
        { pattern: /(?:historial|partidas?|games?)\s+(?:de\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|jp|oce|tr|ru))?$/i,
          handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: m[3]?.toLowerCase() || null, count: 5 }) },
        { pattern: /(?:como\s+)?(?:va|está|esta)\s+([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|jp|oce|tr|ru))?$/i,
          handler: (m) => ({ action: "lol_match_history", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: m[3]?.toLowerCase() || null, count: 1 }) },
        // Without name (use config default) — "mi ultima partida", "ultima partida", "mis partidas"
        { pattern: /(?:mi\s+)?(?:ultima|última)\s+partida$/i,
          handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 1 }) },
        { pattern: /(?:mi\s+)?(?:historial|partidas?|games?)$/i,
          handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 5 }) },
        { pattern: /(?:como\s+)?(?:va|está|esta)\s+(?:mi\s+)?(?:lol|partidas?)$/i,
          handler: () => ({ action: "lol_match_history", riot_id: null, region: null, count: 1 }) },

        // Rank / Elo — with name#tag
        { pattern: /(?:elo|rang[oa]?|clasificaci[oó]n|tier|rank)\s+(?:de\s+)?([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|korea|corea|jp|oce|tr|ru))?$/i,
          handler: (m) => ({ action: "lol_rank", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea|corea/,'kr') || null }) },
        { pattern: /(?:que\s+)?(?:rang[oa]?|tier|elo)\s+(?:tiene|está|esta|es)\s+([a-zA-Z0-9_ ]+?)#([a-zA-Z0-9]+?)(?:\s+en\s+(las?|euw|eune|na|br|kr|korea|corea|jp|oce|tr|ru))?$/i,
          handler: (m) => ({ action: "lol_rank", riot_id: `${m[1].trim()}#${m[2].trim()}`, region: (m[3]?.toLowerCase() || '').replace(/korea|corea/,'kr') || null }) },
        // Rank / Elo — without name (self)
        { pattern: /(?:mi\s+)?(?:elo|rang[oa]?|clasificaci[oó]n|tier|rank)(?:\s+(?:de\s+)?lol)?$/i,
          handler: () => ({ action: "lol_rank", riot_id: null, region: null }) },
        { pattern: /(?:que\s+)?(?:rang[oa]?|tier|elo)\s+(?:tengo|soy|estoy)/i,
          handler: () => ({ action: "lol_rank", riot_id: null, region: null }) },
        { pattern: /(?:en\s+que\s+)?(?:rang[oa]?|tier|elo)\s+(?:estoy|soy|está)/i,
          handler: () => ({ action: "lol_rank", riot_id: null, region: null }) },
        { pattern: /(?:cual\s+es\s+)?(?:mi\s+)?(?:rang[oa]?|tier|elo)\s+(?:de\s+)?lol$/i,
          handler: () => ({ action: "lol_rank", riot_id: null, region: null }) },
    ];

    for (const { pattern, handler } of lolPatterns) {
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── Video Compression Detection ───
    const compressPatterns = [
        { pattern: /comprim(?:í|i|ir|e|o)\s+(?:el\s+)?(?:este\s+)?video\s*:\s*(.+)/i,
          handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() }) },
        { pattern: /comprim(?:í|i|ir|e|o)\s+(.+)\s+para\s+discord/i,
          handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() }) },
        { pattern: /comprim(?:í|i|ir|e|o)\s+(.+\.\w+)/i,
          handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() }) },
        { pattern: /achic(?:á|a|ar)\s+(?:el\s+)?(?:este\s+)?video\s*:\s*(.+)/i,
          handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() }) },
        { pattern: /achic(?:á|a|ar)\s+(.+\.\w+)/i,
          handler: (m) => ({ action: "compress_for_discord", file: m[1].trim() }) },
    ];

    for (const { pattern, handler } of compressPatterns) {
        const match = lower.match(pattern);
        if (match) return { action: handler(match), message: "" };
    }

    // ─── System Detection ───
    if (/cancel(?:ar)?\s+(?:el\s+)?(?:apagado|shutdown|apaga)/i.test(lower)) {
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

    // ─── Config Detection ───
    const configPatterns = [
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
        if (lower === app || lower === `abri ${app}` || lower === `abre ${app}` || lower === `abrir ${app}`) {
            if (app === 'youtube') {
                return { action: { action: "open_url", url: "https://www.youtube.com" }, message: "" };
            }
            return { action: { action: "open_app", app: app }, message: "" };
        }
    }

    const openPatterns = [
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

    const searchInPatterns = [
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

    const searchPatterns = [
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

    const musicPatterns = [
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

    return null;
}

async function executeAction(action) {
    if (!action) return null;
    try {
        switch (action.action) {
            case "get_ip":
                const localIP = await invoke('get_local_ip');
                return `La IP para conectarte es: http://${localIP}:1414\n\nDesde el celular, abrí esa dirección en el navegador 🦎`;
            case "open_app":
                return await invoke('open_any_app', { appName: action.app });
            case "open_url":
                return await invoke('open_url', { url: action.url });
            case "search":
                return await invoke('search_web', { query: action.query });
            case "play_music":
                await invoke('open_url', { url: `https://www.youtube.com/results?search_query=${encodeURIComponent(action.query)}` });
                return `Abriendo YouTube con: ${action.query} 🎵`;
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
                } catch {}
                if (!riotId) return "No tenés tu Riot ID configurado. Ponelo en Configuración ⚙️";
                return await invoke('lol_get_match_history', { riotId, region, count: action.count || 5 });
            }
            case "lol_rank": {
                let region = action.region;
                let riotId = action.riot_id;
                try {
                    const config = JSON.parse(await invoke('lol_get_config'));
                    if (!region) region = config.lol_region || 'las';
                    if (!riotId) riotId = config.riot_id;
                } catch {}
                if (!riotId) return "No tenés tu Riot ID configurado. Ponelo en Configuración ⚙️";
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
        return `No pude hacer eso: ${error}`;
    }
}

async function callLocalAi(message) {
    conversationHistory.push({ role: "user", content: message });

    currentChatId = await invoke('chat_start', { messages: conversationHistory });

    const reply = await invoke('chat_finish', { requestId: currentChatId });
    currentChatId = null;

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

    setThinking(true);

    const detected = detectActionFromText(message);
    if (detected) {
        setTalking(true);
        showBubble("Dale, un segundo... ⏳");
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

    showBubble("Déjame pensar... 🤔");

    try {
        const llamaOn = await invoke('llama_status');
        if (!llamaOn) {
            showBubble("LLaMA está apagado. Activálo en Configuración ⚙️");
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
            setTalking(true);
            showBubble(neekoMsg || "Dale, un segundo... ⏳");
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
        showBubble("No pude procesar eso 🥺");
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
        invoke('chat_cancel', { requestId: currentChatId }).catch(() => {});
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
    try {
        const config = JSON.parse(await invoke('lol_get_config'));
        applyNeekoSprite(config.neeko_sprite);
    } catch {
        applyNeekoSprite(SPRITES.default);
    }

    try {
        const status = await invoke('check_local_ai');
        setLocalAiModelAvailable(true);
        const autoStart = await invoke('get_llama_auto_start');

        if (status === "running") {
            showBubble("¡Hola! Soy Neeko 🦎");
        } else if (autoStart) {
            showBubble("Iniciando llama-server... 🔍");
            await invoke('start_llama_server');
            showBubble("¡Hola! Soy Neeko 🦎");
        } else {
            showBubble("¡Hola! Soy Neeko 🦎\nLLaMA está apagado. Activámlo en Configuración si lo necesitás.");
        }
    } catch (error) {
        console.error('Init error:', error);
        if (error === "no_model") {
            setLocalAiModelAvailable(false);
            try {
                await invoke('set_llama_auto_start', { enabled: false });
            } catch {}
            showBubble("No encontré el modelo GGUF 🦎");
        } else {
            showBubble("¡Hola! Soy Neeko 🦎");
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
const uninstallFfmpegBtn = document.getElementById('uninstall-ffmpeg-btn');
const uninstallGitBtn = document.getElementById('uninstall-git-btn');
const uninstallModelBtn = document.getElementById('uninstall-model-btn');
const dependencyDownloadStatus = document.getElementById('dependency-download-status');
const dependencyDownloadLabel = document.getElementById('dependency-download-label');
const dependencyDownloadPercent = document.getElementById('dependency-download-percent');
const dependencyDownloadBar = document.getElementById('dependency-download-bar');
const dependencyDownloadMessage = document.getElementById('dependency-download-message');
const cancelDownloadBtn = document.getElementById('cancel-download-btn');
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

settingsTabs.forEach((tab) => {
    tab.addEventListener('click', () => setSettingsTab(tab.dataset.settingsTab));
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
    [installFfmpegBtn, installGitBtn, installModelBtn, installModelFileBtn, uninstallFfmpegBtn, uninstallGitBtn, uninstallModelBtn].forEach((btn) => {
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
    dependencyDownloadLabel.textContent = payload.label || 'Descarga';
    dependencyDownloadPercent.textContent = payload.percent == null ? '...' : `${percent}%`;
    dependencyDownloadBar.style.width = `${Math.max(0, Math.min(100, percent))}%`;
    const totalText = payload.total ? ` · ${formatBytes(payload.downloaded)} / ${formatBytes(payload.total)} · faltan ${formatBytes(Math.max(0, payload.total - payload.downloaded))}` : '';
    dependencyDownloadMessage.textContent = `${payload.message || 'Descargando...'}${totalText}`;
}

settingsBtn.addEventListener('click', async () => {
    try {
        const config = JSON.parse(await invoke('lol_get_config'));
        document.getElementById('cfg-git-pat').value = '';
        document.getElementById('cfg-git-path').value = config.git_default_path || '';
        document.getElementById('cfg-neeko-sprite').value = normalizeNeekoSprite(config.neeko_sprite);
        document.getElementById('cfg-lol-region').value = config.lol_region || 'las';
        document.getElementById('cfg-riot-id').value = config.riot_id || '';
    } catch {}
    try {
        const running = await invoke('llama_status');
        updateLlamaUI(running && localAiModelAvailable);
    } catch {}
    try {
        const autoStart = await invoke('get_llama_auto_start');
        const autoStartInput = document.getElementById('cfg-llama-autostart');
        autoStartInput.disabled = !localAiModelAvailable;
        autoStartInput.checked = autoStart && localAiModelAvailable;
    } catch {}
    try {
        const sysCmds = await invoke('get_system_commands_enabled');
        document.getElementById('cfg-system-cmds').checked = sysCmds;
    } catch {}
    settingsModal.classList.remove('hidden');
    setSettingsTab('tools');
    setSettingsMenuOpen(false);
    toolStatusList.innerHTML = '';
});

function updateLlamaUI(running) {
    const state = document.getElementById('cfg-llama-state');
    const toggle = document.getElementById('cfg-llama-toggle');
    state.textContent = running ? '🟢 Encendido' : '🔴 Apagado';
    state.style.color = running ? '#4ade80' : '#f87171';
    toggle.textContent = running ? 'Apagar' : 'Encender';
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
                        await invoke('stop_llama_server').catch(() => {});
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
        name.textContent = `${tool.ok ? 'OK' : 'Falta'} ${tool.name}`;

        const detail = document.createElement('span');
        detail.textContent = `${tool.command}: ${tool.detail || 'Sin detalle'}`;

        item.append(name, detail);
        toolStatusList.appendChild(item);
    });
}

async function saveEnvironmentConfig() {
    await invoke('save_environment_config', { ffmpegPath: null, ffprobePath: null });
}

async function checkEnvironmentTools(showMessage = true) {
    if (!toolStatusList) return;
    toolStatusList.innerHTML = '<div class="tool-status checking">Probando herramientas...</div>';
    checkToolsBtn.disabled = true;
    try {
        await saveEnvironmentConfig();
        const statuses = await invoke('check_environment_tools');
        renderToolStatuses(statuses);
        if (showMessage) {
            const missing = statuses.filter((tool) => !tool.ok).map((tool) => tool.name);
            showBubble(missing.length ? `Falta configurar: ${missing.join(', ')}` : "Herramientas listas");
        }
    } catch (e) {
        toolStatusList.innerHTML = `<div class="tool-status tool-error">No pude probar herramientas: ${e}</div>`;
    }
    checkToolsBtn.disabled = false;
}

closeSettingsBtn.addEventListener('click', () => {
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
}

async function runInstaller(button, command, args = {}) {
    setInstallerButtonsDisabled(true);
    dependencyDownloadStatus.classList.remove('hidden');
    dependencyDownloadLabel.textContent = button.textContent;
    dependencyDownloadPercent.textContent = '...';
    dependencyDownloadBar.style.width = '0%';
    dependencyDownloadMessage.textContent = 'Preparando...';
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
        setSettingsMenuOpen(false);
        settingsModal.classList.add('hidden');
    }
});

saveSettingsBtn.addEventListener('click', async () => {
    const pat = document.getElementById('cfg-git-pat').value.trim();
    const gitPath = document.getElementById('cfg-git-path').value.trim();
    const neekoSpriteValue = normalizeNeekoSprite(document.getElementById('cfg-neeko-sprite').value);
    const region = document.getElementById('cfg-lol-region').value;
    const riotId = document.getElementById('cfg-riot-id').value.trim();
    const autoStartInput = document.getElementById('cfg-llama-autostart');
    let autoStart = autoStartInput.checked;

    try {
        await invoke('lol_save_config', {
            gitPat: pat || null,
            gitPath: gitPath || null,
            neekoSprite: neekoSpriteValue,
            region: region || null,
            riotId: riotId || null,
        });
        applyNeekoSprite(neekoSpriteValue);
    } catch (e) {
        showBubble("Error guardando config: " + e);
    }
    try {
        await invoke('save_environment_config', { ffmpegPath: null, ffprobePath: null });
    } catch (e) {
        showBubble("Error guardando herramientas: " + e);
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
    } catch (e) {}
    try {
        const sysCmds = document.getElementById('cfg-system-cmds').checked;
        await invoke('set_system_commands_enabled', { enabled: sysCmds });
    } catch (e) {}
    showBubble("Configuración guardada ✅");
    setSettingsMenuOpen(false);
    settingsModal.classList.add('hidden');
    resetIdleTimer();
});

const checkUpdateBtn = document.getElementById('check-update-btn');
const applyUpdateBtn = document.getElementById('apply-update-btn');
const updateStatus = document.getElementById('update-status');
const updateNotes = document.getElementById('update-notes');

checkUpdateBtn.addEventListener('click', async () => {
    checkUpdateBtn.disabled = true;
    checkUpdateBtn.textContent = 'Buscando...';
    updateNotes.textContent = '';
    try {
        const result = await invoke('check_updates');
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
        updateStatus.textContent = 'Error: ' + e;
        updateStatus.style.color = '#f87171';
        showBubble('Error: ' + e);
    }
    checkUpdateBtn.disabled = false;
    checkUpdateBtn.textContent = 'Buscar actualizaciones';
});

applyUpdateBtn.addEventListener('click', async () => {
    applyUpdateBtn.disabled = true;
    applyUpdateBtn.textContent = 'Descargando...';
    updateNotes.textContent = 'La app se reiniciará automáticamente al instalar.';
    try {
        const message = await invoke('download_and_install_update');
        showBubble(message);
    } catch (e) {
        updateNotes.textContent = 'Error: ' + e;
        showBubble('Error: ' + e);
        applyUpdateBtn.disabled = false;
        applyUpdateBtn.textContent = 'Actualizar y reiniciar';
    }
});

init();
