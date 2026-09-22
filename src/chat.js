import { ChatClient, confirmationControls, attachInfo, renderMessageContent, messageParts } from './chat-client.mjs';

const { invoke } = window.__TAURI__.core;
const messages = document.getElementById('messages');
const input = document.getElementById('chat-input');
const send = document.getElementById('send-btn');
const status = document.getElementById('status');
const openUrl = url => invoke('open_url', { url });
let petEvents = Promise.resolve();
const petState = state => {
    petEvents = petEvents.then(() => window.__TAURI__.event.emitTo('main', 'neeko:chat-state', state)).catch(console.error);
};

function appendMessage(text, role = 'assistant', info, sources) {
    document.getElementById('welcome')?.remove();
    const entry = document.createElement('div');
    entry.className = `message ${role}`;
    if (role === 'assistant') renderMessageContent(entry, text, openUrl);
    else entry.textContent = text;
    messages.appendChild(entry);
    attachInfo(entry, info, 'es', openUrl, sources);
    // Keep the visible log bounded along with the model's conversation history.
    while (messages.children.length > 80) messages.firstElementChild.remove();
    messages.scrollTop = messages.scrollHeight;
}

const client = new ChatClient({
    chat: (session, history, requestId) => invoke('assistant_chat', { session: `chat-window:${session}`, messages: history, requestId }),
    decide: (session, proposalId, approved, saveAccount = false) => invoke('assistant_decide', { session: `chat-window:${session}`, proposalId, approved, saveAccount }),
    cancel: session => invoke('assistant_cancel', { session: `chat-window:${session}` }),
    progress: (requestId) => invoke('research_progress', { requestId }),
}, {
    busy: value => {
        petState({ state: value ? 'thinking' : 'idle' });
        send.textContent = value ? '\u2715' : '\u27a4';
        send.classList.toggle('cancel-mode', value);
        send.title = value ? 'Cancelar' : 'Enviar';
        send.setAttribute('aria-label', send.title);
        status.textContent = value ? 'Neeko Asistente está pensando…' : '';
        if (!value) { send.disabled = false; input.focus(); }
    },
    progress: (step, details) => {
        const labels = {
            thinking: 'Neeko Asistente esta pensando...',
            searching: 'Buscando informacion...',
            reading: 'Leyendo fuentes...',
            checking: 'Comprobando datos...',
            preparing: 'Preparando respuesta...',
            writing: 'Redactando respuesta...',
        };
        status.textContent = details?.text || labels[step] || labels.thinking;
    },
    confirm: (proposal, signal) => {
        petState({ state: 'waiting' });
        status.textContent = 'Esperando tu confirmación';
        return confirmationControls(document.getElementById('action-confirmation'), document.getElementById('action-details'),
            document.getElementById('action-yes'), document.getElementById('action-no'), proposal, signal, 'es', openUrl);
    },
    executing: approved => {
        petState({ state: approved ? 'thinking' : 'waiting' });
        status.textContent = approved ? 'Ejecutando…' : 'Cancelando acción…';
    },
    message: (text, info, sources) => { appendMessage(text, 'assistant', info, sources); petState({ state: 'reply', text: messageParts(text).text }); },
    error: text => appendMessage(text, 'error'),
});

document.getElementById('input-bar').addEventListener('submit', event => {
    event.preventDefault();
    if (client.busy) { void cancelRequest(); return; }
    const text = input.value.trim();
    if (!text || client.busy) return;
    input.value = '';
    appendMessage(text, 'user');
    void client.send(text);
});
async function cancelRequest() {
    petState({ state: 'cancelled' });
    send.disabled = true;
    try { await client.cancel(); appendMessage('Solicitud cancelada.'); }
    catch (error) { appendMessage(String(error), 'error'); }
    finally { send.disabled = false; }
}
// Closing hides this window through the app's existing CloseRequested handler;
// reopening preserves the conversation and any pending confirmation.
document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && !document.querySelector('dialog[open]')) {
        void invoke('close_window').catch(error => appendMessage(String(error), 'error'));
    }
});

const chatWindow = window.__TAURI__.window.getCurrentWindow();
document.getElementById('settings-btn').addEventListener('click', () => {
    void invoke('open_settings_window').catch(error => appendMessage(String(error), 'error'));
});
document.getElementById('top-bar').addEventListener('pointerdown', event => {
    if (event.button === 0 && !event.target.closest('button')) void chatWindow.startDragging().catch(console.error);
});
document.getElementById('minimize-btn').addEventListener('click', () => void chatWindow.minimize().catch(console.error));
document.getElementById('close-btn').addEventListener('click', () => void invoke('close_window').catch(console.error));
