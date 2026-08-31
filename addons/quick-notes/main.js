// ─── Quick Notes Addon ───
// Guarda y gestiona notas rapidas desde el chat.
// Usage: "nota comprar leche" / "note buy milk"

(function() {
    const STORAGE_KEY = 'quick-notes-notes';
    const TEXT = {
        es: {
            tabTitle: 'Notas',
            hint: 'Escribe',
            hintCommand: 'nota <texto>',
            hintSuffix: 'en el chat para guardar notas.',
            empty: 'No hay notas todavia.',
            deleteTitle: 'Borrar',
            missingText: 'Escribe algo para guardar.',
            saved: (count, text) => `Nota #${count} guardada: "${text}"`,
            noNotes: 'No tienes notas. Escribe "nota <texto>" para guardar una.',
            listTitle: (count) => `Tus notas (${count}):`,
            invalidNumber: (count) => `Numero invalido. Tienes ${count} notas.`,
            deleted: (id, text) => `Nota #${id} borrada: "${text}"`,
            cleared: (count) => `${count} notas borradas.`,
        },
        en: {
            tabTitle: 'Notes',
            hint: 'Type',
            hintCommand: 'note <text>',
            hintSuffix: 'in chat to save notes.',
            empty: 'No notes yet.',
            deleteTitle: 'Delete',
            missingText: 'Write something to save.',
            saved: (count, text) => `Note #${count} saved: "${text}"`,
            noNotes: 'You have no notes. Type "note <text>" to save one.',
            listTitle: (count) => `Your notes (${count}):`,
            invalidNumber: (count) => `Invalid number. You have ${count} notes.`,
            deleted: (id, text) => `Note #${id} deleted: "${text}"`,
            cleared: (count) => `${count} notes deleted.`,
        },
    };

    function lang() {
        return document.documentElement.lang === 'en' ? 'en' : 'es';
    }

    function t(key, ...args) {
        const value = TEXT[lang()][key] || TEXT.es[key];
        return typeof value === 'function' ? value(...args) : value;
    }

    async function getNotes() {
        try {
            const raw = localStorage.getItem(STORAGE_KEY);
            return raw ? JSON.parse(raw) : [];
        } catch {
            return [];
        }
    }

    async function saveNotes(notes) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(notes));
    }

    // ─── Comandos ───

    Neeko.commands.register('save-note', {
        patterns: {
            es: ['nota\\s+(.+)', 'guarda(?:r)?\\s+nota\\s+(.+)'],
            en: ['note\\s+(.+)', 'save\\s+note\\s+(.+)'],
        },
        handler: async (matches, message) => {
            const text = matches[1]?.trim();
            if (!text) return { message: t('missingText') };

            const notes = await getNotes();
            notes.push({
                id: notes.length + 1,
                text: text,
                date: new Date().toLocaleString(),
            });
            await saveNotes(notes);
            await refreshNotesTab();

            return { message: t('saved', notes.length, text) };
        },
    });

    Neeko.commands.register('list-notes', {
        patterns: {
            es: ['notas', 'ver\\s+notas', 'mis\\s+notas', 'lista\\s+de\\s+notas'],
            en: ['notes', 'show\\s+notes', 'my\\s+notes', 'list\\s+notes'],
        },
        handler: async () => {
            const notes = await getNotes();
            if (!notes.length) {
                return { message: t('noNotes') };
            }
            const list = notes.map((n, i) => `${i + 1}. ${n.text}`).join('\n');
            return { message: `${t('listTitle', notes.length)}\n${list}` };
        },
    });

    Neeko.commands.register('delete-note', {
        patterns: {
            es: ['borrar\\s+nota\\s+(\\d+)', 'eliminar\\s+nota\\s+(\\d+)'],
            en: ['delete\\s+note\\s+(\\d+)', 'remove\\s+note\\s+(\\d+)'],
        },
        handler: async (matches) => {
            const id = parseInt(matches[1]);
            const notes = await getNotes();
            if (id < 1 || id > notes.length) {
                return { message: t('invalidNumber', notes.length) };
            }
            const deleted = notes.splice(id - 1, 1)[0];
            // Re-number
            notes.forEach((n, i) => n.id = i + 1);
            await saveNotes(notes);
            await refreshNotesTab();
            return { message: t('deleted', id, deleted.text) };
        },
    });

    Neeko.commands.register('clear-notes', {
        patterns: {
            es: ['borrar\\s+todas\\s+las\\s+notas', 'limpiar\\s+notas'],
            en: ['clear\\s+all\\s+notes', 'delete\\s+all\\s+notes'],
        },
        handler: async () => {
            const notes = await getNotes();
            const count = notes.length;
            await saveNotes([]);
            await refreshNotesTab();
            return { message: t('cleared', count) };
        },
    });

    // ─── Settings Tab ───

    function renderNotesHtml(notes) {
        if (!notes.length) {
            return `<p class="qn-empty">${t('empty')}</p>`;
        }
        return notes.map((n, i) => `
            <div class="qn-note" data-idx="${i}">
                <div class="qn-note-text">
                    <span class="qn-note-num">#${i + 1}</span>
                    <span>${escapeHtml(n.text)}</span>
                </div>
                <div class="qn-note-meta">
                    <span class="qn-note-date">${n.date || ''}</span>
                    <button class="qn-note-delete" data-idx="${i}" title="${t('deleteTitle')}">x</button>
                </div>
            </div>
        `).join('');
    }

    function escapeHtml(str) {
        return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function renderSettingsHtml() {
        return `
            <div class="qn-container">
                <p class="qn-hint">${t('hint')} <code>${t('hintCommand')}</code> ${t('hintSuffix')}</p>
                <div id="qn-notes-container"></div>
            </div>
        `;
    }

    async function refreshNotesTab() {
        const container = document.getElementById('qn-notes-container');
        if (!container) return;
        const notes = await getNotes();
        container.innerHTML = renderNotesHtml(notes);
        container.querySelectorAll('.qn-note-delete').forEach(btn => {
            btn.addEventListener('click', async () => {
                const idx = parseInt(btn.dataset.idx);
                const notes = await getNotes();
                notes.splice(idx, 1);
                notes.forEach((n, i) => n.id = i + 1);
                await saveNotes(notes);
                refreshNotesTab();
            });
        });
    }

    Neeko.ui.registerSettingsTab('quick-notes', t('tabTitle'), renderSettingsHtml());

    function syncLanguage() {
        const tab = document.querySelector('[data-settings-tab="quick-notes"]');
        if (tab) tab.textContent = t('tabTitle');
        const panel = document.querySelector('[data-settings-panel="quick-notes"]');
        if (panel) {
            panel.innerHTML = renderSettingsHtml();
            refreshNotesTab();
        }
    }

    const languageObserver = new MutationObserver(syncLanguage);
    languageObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['lang'],
    });
    Neeko.addon.onUnload(() => languageObserver.disconnect());

    // Render when tab is ready
    setTimeout(refreshNotesTab, 500);

})();
