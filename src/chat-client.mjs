// Render only supported image links; all other content remains plain text.
export function pokemonImageUrl(value) {
    try {
        const url = new URL(value);
        return url.protocol === 'https:' && url.hostname === 'raw.githubusercontent.com' && !url.username && !url.password && !url.port && !url.search && !url.hash
            && /^\/PokeAPI\/sprites\/master\/sprites\/pokemon\/(?:other\/(?:official-artwork|home)\/)?\d+\.png$/.test(url.pathname) ? url.href : null;
    } catch { return null; }
}

export function messageParts(text) {
    const images = [];
    const colors = {};
    let plain = String(text ?? '').replace(/!\[([^\]\r\n]{1,100})\]\((https:\/\/[^\s)]+)\)/g, (match, alt, url) => {
        const src = pokemonImageUrl(url);
        if (!src) return match;
        if (images.length < 4 && !images.some(image => image.src === src)) images.push({ src, alt });
        return '';
    }).trim();
    plain = plain.replace(/\[poke-color:([a-z0-9-]{1,100}):(black|blue|brown|gray|green|pink|purple|red|white|yellow)\]/g, (_, name, color) => {
        if (Object.keys(colors).length < 4) colors[name] = color;
        return '';
    }).trim();
    const source = /(?:^|\n)Fuente: PokéAPI \(https:\/\/pokeapi\.co\/\)\.?\s*$/.test(plain) ? 'https://pokeapi.co/' : null;
    if (source) plain = plain.replace(/(?:^|\n)Fuente: PokéAPI \(https:\/\/pokeapi\.co\/\)\.?\s*$/, '').trimEnd();
    if (images.length || Object.keys(colors).length) plain = plain.replace(/\n[ \t]*\n(?:[ \t]*\n)+/g, '\n\n');
    return { text: plain, images, source, colors };
}

// Light shades keep names readable on the dark desktop and web chat backgrounds.
const POKEMON_COLORS = {black:'#c4c4d2',blue:'#8fc7ff',brown:'#dfb78f',gray:'#c9cdd6',green:'#98e6ad',pink:'#ffb4dc',purple:'#d1b0ff',red:'#ffaaa7',white:'#f5f5ff',yellow:'#ffe078'};

export function renderMessageContent(parent, text, openUrl) {
    const parts = messageParts(text);
    parent.textContent = parts.text;
    if (Object.keys(parts.colors).length) {
        parent.textContent = '';
        for (const token of parts.text.split(/([a-z0-9-]+)/gi)) {
            const color = POKEMON_COLORS[parts.colors[token.toLowerCase()]];
            if (!color) { parent.append(document.createTextNode(token)); continue; }
            const name = document.createElement('span');
            name.textContent = token;
            name.style.color = color;
            name.style.fontWeight = '600';
            parent.append(name);
        }
    }
    if (parts.images.length) {
        const gallery = document.createElement('div');
        gallery.className = 'chat-image-gallery';
        gallery.style.cssText = 'display:flex;flex-wrap:wrap;justify-content:center;gap:12px;margin:12px 0;max-width:100%;white-space:normal';
        for (const image of parts.images) {
            const card = document.createElement('figure');
            card.style.cssText = 'margin:0;flex:1 1 110px;min-width:0;max-width:min(180px,100%);text-align:center';
            const img = document.createElement('img');
            img.alt = image.alt;
            img.loading = 'lazy';
            img.decoding = 'async';
            img.referrerPolicy = 'no-referrer';
            img.width = img.height = 144;
            img.style.cssText = 'display:block;width:100%;height:144px;object-fit:contain;border-radius:12px;background:rgba(128,128,128,.12)';
            const caption = document.createElement('figcaption');
            caption.textContent = image.alt;
            caption.style.cssText = 'font-size:12px;margin-top:4px;overflow-wrap:anywhere';
            const color = POKEMON_COLORS[parts.colors[image.alt.toLowerCase()]];
            if (color) { caption.style.color = color; caption.style.fontWeight = '600'; }
            img.onerror = () => { img.hidden = true; img.style.display = 'none'; caption.textContent = image.alt + ' — imagen no disponible'; };
            img.src = image.src;
            card.append(img, caption);
            gallery.append(card);
        }
        parent.append(gallery);
    }
    if (parts.source) {
        const footer = document.createElement('div');
        footer.className = 'chat-source';
        footer.style.cssText = 'margin-top:8px;font-size:12px;line-height:1.4;white-space:normal';
        footer.textContent = 'Fuente: ';
        const link = document.createElement('a');
        link.textContent = 'PokéAPI';
        link.href = parts.source;
        link.target = '_blank';
        link.rel = 'noopener noreferrer';
        link.style.cssText = 'color:#c4b5fd;text-decoration:underline;text-underline-offset:2px;cursor:pointer';
        if (openUrl) link.onclick = async event => {
            event.preventDefault();
            try { await openUrl(parts.source); }
            catch (error) { console.error('Could not open source:', error); link.textContent = 'PokéAPI — no se pudo abrir; reintentá'; }
        };
        footer.append(link);
        parent.append(footer);
    }
}

// Both UIs use this lifecycle. Only the backend may choose/validate/execute actions.
export class ChatClient {
    constructor(transport, ui, session = Array.from(crypto.getRandomValues(new Uint32Array(4)), n => n.toString(16)).join('-')) {
        this.transport = transport;
        this.ui = ui;
        this.session = session;
        this.history = [];
        this.busy = false;
        this.requestId = null;
    }

    async send(text) {
        if (this.busy || !text.trim()) return;
        this.busy = true;
        this.requestId = Array.from(crypto.getRandomValues(new Uint32Array(4)), n => n.toString(16)).join('-');
        const controller = new AbortController();
        this.controller = controller;
        this.ui.busy(true);
        const progress = startProgress(this.ui, controller.signal, this.transport, this.requestId);
        this.history.push({ role: 'user', content: text.trim() });
        try {
            const reply = await this.transport.chat(this.session, this.history.slice(-40), this.requestId);
            progress.stop();
            if (controller.signal.aborted) return;
            this.history.push({ role: 'assistant', content: JSON.stringify({ kind: reply.kind, message: reply.message, proposal: reply.details }) });
            if (reply.kind === 'action' && reply.proposal_id && reply.details) {
                const choice = reply.requires_confirmation === false
                    ? true
                    : await this.ui.confirm(reply, controller.signal);
                const approved = typeof choice === 'object' ? choice?.approved === true : choice === true;
                const saveAccount = approved && reply.can_save_account === true && choice?.saveAccount === true;
                if (controller.signal.aborted) return;
                this.ui.executing?.(approved);
                progress.stop();
                if (approved && typeof reply.execution_message === 'string' && reply.execution_message.trim()) {
                    deliverMessage(this.ui, reply.execution_message);
                }
                const result = saveAccount
                    ? await this.transport.decide(this.session, reply.proposal_id, approved, true)
                    : await this.transport.decide(this.session, reply.proposal_id, approved);
                if (controller.signal.aborted) return;
                this.history.push({ role: 'assistant', content: `App action result (${approved ? 'approved' : 'declined'}): ${result}` });
                deliverMessage(this.ui, result, approved ? reply.info : undefined, approved ? reply.sources : undefined);
            } else if (reply.kind === 'conversation' || reply.kind === 'question') {
                deliverMessage(this.ui, reply.message, reply.info, reply.sources);
            } else {
                throw new Error('Invalid assistant response');
            }
        } catch (error) {
            if (!controller.signal.aborted) {
                const message = String(error?.message || error);
                this.history.push({ role: 'assistant', content: `App error: ${message}` });
                this.ui.error(message);
            }
        } finally {
            progress.stop();
            this.history.splice(0, Math.max(0, this.history.length - 40));
            if (this.controller === controller && !this.cancelling) {
                this.controller = null;
                this.busy = false;
                this.ui.busy(false);
            }
        }
    }

    async cancel() {
        this.cancelling = true;
        this.controller?.abort();
        // Keep the session busy until invalidation is acknowledged, preventing
        // a delayed cancellation from cancelling a subsequent request.
        try { await this.transport.cancel(this.session); }
        finally { this.controller = null; this.cancelling = false; this.busy = false; this.ui.busy(false); }
    }
}

function deliverMessage(ui, text, info, sources) {
    if (Array.isArray(sources) && sources.length) ui.message(text, info, sources);
    else ui.message(text, info);
}

const PHASE_LABELS = {
    planning: { es: 'Preparando investigación…', en: 'Planning research…' },
    searching: { es: 'Buscando fuentes…', en: 'Searching sources…' },
    reading: { es: 'Leyendo contenido…', en: 'Reading content…' },
    checking: { es: 'Comprobando datos…', en: 'Checking data…' },
    writing: { es: 'Preparando respuesta…', en: 'Writing answer…' },
    complete: { es: 'Listo', en: 'Done' },
    cancelled: { es: 'Cancelado', en: 'Cancelled' },
    error: { es: 'Error', en: 'Error' },
    idle: { es: 'Pensando…', en: 'Thinking…' },
};

function startProgress(ui, signal, transport, requestId) {
    if (typeof ui.progress !== 'function') return { stop() {} };
    let timer = null;
    let active = true;
    let lastSequence = 0;

    async function poll() {
        if (!active || signal.aborted) return;
        try {
            const state = await transport.progress(requestId);
            if (!active || signal.aborted) return;
            if (state && (state.sequence || 0) > lastSequence) {
                lastSequence = state.sequence || 0;
                const lang = ui.language || 'es';
                const label = PHASE_LABELS[state.phase] || PHASE_LABELS.idle;
                ui.progress(state.phase, {
                    phase: state.phase,
                    text: label[lang] || label.es,
                    operations: state.completed_operations || 0,
                    domain: state.domain || null,
                });
            }
        } catch {
            // Silently ignore polling errors
        }
        if (active && !signal.aborted) {
            timer = setTimeout(poll, 800);
        }
    }

    // Start with neutral indicator, then begin polling after short delay
    ui.progress('thinking', { phase: 'thinking', text: ui.language === 'en' ? 'Thinking…' : 'Pensando…' });
    if (typeof transport.progress === 'function') timer = setTimeout(poll, 600);

    return {
        stop() {
            active = false;
            if (timer) clearTimeout(timer);
        }
    };
}

export function confirmationText(language) {
    return language === 'en' ? 'Run this action?' : '¿Querés ejecutar esta acción?';
}

export function localizedDetails(details, language) {
    const english = language === 'en';
    const [title, ...parameters] = String(details || '').split('\n');
    const titles = title.split(' / ');
    const labels = {
        app: ['Aplicación', 'App'], url: ['Enlace', 'Link'],
        site: ['Sitio', 'Site'], query: ['Búsqueda', 'Search'],
        folder: ['Carpeta', 'Folder'], file: ['Archivo', 'File'],
        path: ['Ruta', 'Path'], seconds: ['Segundos', 'Seconds'],
        message: ['Mensaje', 'Message'], text: ['Texto', 'Text'],
        category: ['Categoría', 'Category'], key: ['Nombre', 'Name'],
        value: ['Valor', 'Value'], id: ['Identificador', 'ID'],
        targetSizeMb: ['Tamaño máximo (MB)', 'Maximum size (MB)'],
        videoBitrateKbps: ['Calidad de video (kbps)', 'Video bitrate (kbps)'],
        region: ['Región', 'Region'], count: ['Cantidad', 'Count'],
        riot_id: ['Riot ID', 'Riot ID'],
        name: ['Nombre', 'Name'], branch: ['Rama', 'Branch'],
        remote: ['Repositorio remoto', 'Remote'], git_pat: ['Token', 'Token'],
    };
    return [titles[english ? 1 : 0] || titles[0], ...parameters.map(line =>
        line.replace(/^([^:]+):/, (match, key) => labels[key] ? `${labels[key][english ? 1 : 0]}:` : match)
    )].join('\n');
}

export function confirmationControls(container, details, yes, no, proposal, signal, language = 'es', openUrl) {
    return new Promise(resolve => {
        let saveLabel;
        let saveCheckbox;
        if (proposal.can_save_account) {
            saveLabel = document.createElement('label');
            saveLabel.style.cssText = 'display:block;margin:12px 0;color:inherit;font-size:13px;line-height:1.5';
            saveCheckbox = document.createElement('input');
            saveCheckbox.type = 'checkbox';
            saveCheckbox.checked = false;
            saveLabel.append(saveCheckbox, document.createTextNode(language === 'en' ? ' Save this account for future queries' : ' Guardar esta cuenta para próximas consultas'));
            container.insertBefore(saveLabel, details.nextSibling);
        }
        details.textContent = localizedDetails(proposal.details, language);
        yes.textContent = language === 'en' ? 'Yes' : 'Sí';
        no.textContent = 'No';
        container.setAttribute?.('aria-label', confirmationText(language));
        container.hidden = false;
        let settled = false;
        const finish = approved => {
            if (settled) return;
            settled = true;
            container.hidden = true;
            yes.onclick = no.onclick = null;
            signal.removeEventListener('abort', abort);
            const saveAccount = approved && saveCheckbox?.checked === true;
            saveLabel?.remove();
            resolve(saveCheckbox ? { approved, saveAccount } : approved);
        };
        const abort = () => finish(false);
        yes.onclick = () => finish(true);
        no.onclick = () => finish(false);
        signal.addEventListener('abort', abort, { once: true });
        if (signal.aborted) finish(false);
        else no.focus();
    });
}

export function sourceUrl(info) {
    if (typeof info !== 'string' || !info.trim() || /\s/.test(info.trim())) return null;
    try {
        const url = new URL(info.trim());
        if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password) return null;
        return url.href;
    } catch { return null; }
}

export function attachInfo(parent, info, language = 'es', openUrl, sources = []) {
    const validSources = Array.isArray(sources) ? sources.filter(source => sourceUrl(source?.url)) : [];
    const fallbackUrl = sourceUrl(info);
    if (!validSources.length && !fallbackUrl) return null;
    const button = document.createElement(validSources.length > 1 ? 'button' : 'a');
    button.className = 'assistant-info-trigger';
    button.textContent = 'Info';
    button.style.cssText = 'display:block;width:fit-content;margin:10px auto 0;padding:6px 14px;background:#38265b;color:#f5f3ff;border:1px solid #a78bfa;border-radius:8px;cursor:pointer;text-decoration:none;font:inherit';
    if (validSources.length <= 1) {
        const url = validSources[0]?.url || fallbackUrl;
        button.href = url;
        button.target = '_blank';
        button.rel = 'noopener noreferrer';
        button.title = language === 'en' ? 'Open source' : 'Abrir fuente';
        button.setAttribute('aria-label', `${button.title}: ${new URL(url).hostname}`);
        if (openUrl) button.onclick = async event => {
            event.preventDefault();
            try { await openUrl(url); }
            catch (error) { console.error('Could not open source:', error); }
        };
        parent.append(button);
        return button;
    }
    button.type = 'button';
    button.title = language === 'en' ? 'Show sources' : 'Ver fuentes';
    button.onclick = () => {
        const existing = parent.querySelector('.assistant-info-panel');
        if (existing) { existing.remove(); return; }
        const panel = document.createElement('div');
        panel.className = 'assistant-info-panel';
        panel.style.cssText = 'margin:10px 0 0;padding:10px;border:1px solid #7c3aed;border-radius:8px;background:#211b35;color:#f5f3ff;font-size:12px;line-height:1.45;text-align:left;white-space:normal';
        const title = document.createElement('strong');
        title.textContent = language === 'en' ? 'Sources' : 'Fuentes';
        panel.append(title);
        validSources.slice(0, 8).forEach((source, index) => {
            const row = document.createElement('div');
            row.style.cssText = 'margin-top:8px';
            const link = document.createElement('a');
            link.href = source.url;
            link.target = '_blank';
            link.rel = 'noopener noreferrer';
            link.textContent = `[${index + 1}] ${source.title || source.domain || source.url}`;
            link.style.cssText = 'color:#c4b5fd;text-decoration:underline';
            if (openUrl) link.onclick = async event => {
                event.preventDefault();
                try { await openUrl(source.url); }
                catch (error) { console.error('Could not open source:', error); }
            };
            const meta = document.createElement('div');
            const date = source.published_at || source.retrieved_at;
            meta.textContent = [source.domain, source.provider, source.source_type, source.read_status, date].filter(Boolean).join(' | ');
            meta.style.cssText = 'margin-top:2px;color:#ddd6fe';
            row.append(link, meta);
            panel.append(row);
        });
        parent.append(panel);
    };
    parent.append(button);
    return button;
}
