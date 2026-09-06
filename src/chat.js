import { ChatClient, confirmationControls, attachInfo } from './chat-client.mjs';

const { invoke } = window.__TAURI__.core;
const messages = document.getElementById('messages');
const input = document.getElementById('chat-input');
const send = document.getElementById('send-btn');
const status = document.getElementById('status');
const openUrl = url => invoke('open_url', { url });

function appendMessage(text, role = 'assistant', info) {
    document.getElementById('welcome')?.remove();
    const entry = document.createElement('div');
    entry.className = `message ${role}`;
    entry.textContent = text;
    messages.appendChild(entry);
    attachInfo(entry, info, 'es', openUrl);
    // Keep the visible log bounded along with the model's conversation history.
    while (messages.children.length > 80) messages.firstElementChild.remove();
    messages.scrollTop = messages.scrollHeight;
}

const client = new ChatClient({
    chat: (session, history) => invoke('assistant_chat', { session: `chat-window:${session}`, messages: history }),
    decide: (session, proposalId, approved, saveAccount = false) => invoke('assistant_decide', { session: `chat-window:${session}`, proposalId, approved, saveAccount }),
    cancel: session => invoke('assistant_cancel', { session: `chat-window:${session}` }),
}, {
    busy: value => {
        send.textContent = value ? '\u2715' : '\u27a4';
        send.classList.toggle('cancel-mode', value);
        send.title = value ? 'Cancelar' : 'Enviar';
        send.setAttribute('aria-label', send.title);
        status.textContent = value ? 'Neeko está pensando…' : '';
        if (!value) { send.disabled = false; input.focus(); }
    },
    confirm: (proposal, signal) => {
        status.textContent = 'Esperando tu confirmación';
        return confirmationControls(document.getElementById('action-confirmation'), document.getElementById('action-details'),
            document.getElementById('action-yes'), document.getElementById('action-no'), proposal, signal, 'es', openUrl);
    },
    executing: approved => { status.textContent = approved ? 'Ejecutando…' : 'Cancelando acción…'; },
    message: (text, info) => appendMessage(text, 'assistant', info),
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
document.getElementById('top-bar').addEventListener('pointerdown', event => {
    if (event.button === 0 && !event.target.closest('button')) void chatWindow.startDragging().catch(console.error);
});
document.getElementById('minimize-btn').addEventListener('click', () => void chatWindow.minimize().catch(console.error));
document.getElementById('close-btn').addEventListener('click', () => void invoke('close_window').catch(console.error));
