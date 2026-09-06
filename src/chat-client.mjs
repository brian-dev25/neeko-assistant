// Both UIs use this lifecycle. Only the backend may choose/validate/execute actions.
export class ChatClient {
    constructor(transport, ui, session = Array.from(crypto.getRandomValues(new Uint32Array(4)), n => n.toString(16)).join('-')) {
        this.transport = transport;
        this.ui = ui;
        this.session = session;
        this.history = [];
        this.busy = false;
    }

    async send(text) {
        if (this.busy || !text.trim()) return;
        this.busy = true;
        const controller = new AbortController();
        this.controller = controller;
        this.ui.busy(true);
        this.history.push({ role: 'user', content: text.trim() });
        try {
            const reply = await this.transport.chat(this.session, this.history.slice(-40));
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
                const result = saveAccount
                    ? await this.transport.decide(this.session, reply.proposal_id, approved, true)
                    : await this.transport.decide(this.session, reply.proposal_id, approved);
                if (controller.signal.aborted) return;
                this.history.push({ role: 'assistant', content: `App action result (${approved ? 'approved' : 'declined'}): ${result}` });
                this.ui.message(result, approved ? reply.info : undefined);
            } else if (reply.kind === 'conversation' || reply.kind === 'question') {
                this.ui.message(reply.message, reply.info);
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

export function attachInfo(parent, info, language = 'es', openUrl) {
    const url = sourceUrl(info);
    if (!url) return null;
    const link = document.createElement('a');
    link.className = 'assistant-info-trigger';
    link.textContent = 'Info';
    link.href = url;
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.title = language === 'en' ? 'Open source' : 'Abrir fuente';
    link.setAttribute('aria-label', `${link.title}: ${new URL(url).hostname}`);
    link.style.cssText = 'display:block;width:fit-content;margin:10px auto 0;padding:6px 14px;background:#38265b;color:#f5f3ff;border:1px solid #a78bfa;border-radius:8px;cursor:pointer;text-decoration:none';
    if (openUrl) link.onclick = async event => {
        event.preventDefault();
        try { await openUrl(url); }
        catch (error) { console.error('Could not open source:', error); }
    };
    parent.append(link);
    return link;
}
