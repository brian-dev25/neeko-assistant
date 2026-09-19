import test from 'node:test';
import assert from 'node:assert/strict';
import { initAppearance, normalizeTheme } from '../src/appearance.mjs';

test('appearance migrates once, saves explicitly and restores cancelled edits', async () => {
    const stored = new Map([['neeko-airi-size', '67']]);
    const elements = new Map();
    const element = id => {
        if (!elements.has(id)) elements.set(id, { value: '', handlers: {}, addEventListener(event, fn) { this.handlers[event] = fn; } });
        return elements.get(id);
    };
    const savedGlobals = [globalThis.document, globalThis.window, globalThis.localStorage];
    globalThis.document = { getElementById: element };
    globalThis.window = { addEventListener() {} };
    globalThis.localStorage = { getItem: key => stored.get(key) ?? null, setItem: (key, value) => stored.set(key, String(value)) };
    try {
        let migrations = 0;
        const options = { isSettingsWindow: true, setDesktopHitRegion() { assert.fail('Settings must not change native hit regions'); },
            invoke: async () => { migrations++; return JSON.stringify({legacy_airi_enabled:true}); } };
        const appearance = await initAppearance(options);
        const theme = element('cfg-appearance-theme');
        const size = element('cfg-appearance-size');
        assert.equal(theme.value, 'desktop');
        assert.equal(Number(size.value), 67);
        theme.value = 'classic';
        size.value = 50;
        appearance.loadSettings(); // Cancel/reopen restores saved values.
        assert.equal(theme.value, 'desktop');
        assert.equal(Number(size.value), 67);
        theme.value = 'classic';
        size.value = 55;
        appearance.save();
        assert.equal(stored.get('neeko-appearance-theme'), 'classic');
        assert.equal(stored.get('neeko-airi-size'), '55');
        await initAppearance(options);
        assert.equal(migrations, 1);
        assert.equal(theme.value, 'classic');
    } finally {
        [globalThis.document, globalThis.window, globalThis.localStorage] = savedGlobals;
    }
});

test('legacy theme name preserves desktop preference', () => {
    assert.equal(normalizeTheme('airi'), 'desktop');
});
