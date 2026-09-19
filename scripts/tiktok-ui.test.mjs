import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

const catalog = JSON.parse(fs.readFileSync(new URL('../src-tauri/scripts/tiktok-engines.json', import.meta.url)));
const source = fs.readFileSync(new URL('../src/tiktok.js', import.meta.url), 'utf8');
class Element {
    constructor(tag = '') { this.tag = tag; this.children = []; this.dataset = {}; this.value = ''; this.disabled = false; }
    append(child) { this.children.push(child); }
    add(option) { this.append(option); if (this.children.length === 1) this.value = option.value; }
    replaceChildren() { this.children = []; this.value = ''; }
    setAttribute() {}
    querySelectorAll(tag) { return this.children.flatMap(child => [...(child.tag === tag ? [child] : []), ...child.querySelectorAll(tag)]); }
}
async function fixture(fail = false) {
    const elements = new Map();
    const get = id => { if (!elements.has(id)) elements.set(id, new Element()); return elements.get(id); };
    const calls = [];
    const context = { document: { getElementById: get, createElement: tag => new Element(tag) },
        Option: class extends Element { constructor(label, value) { super('option'); this.textContent = label; this.value = value; } },
        window: { __TAURI__: { window: { getCurrentWindow: () => ({minimize() {}}) }, core: { invoke: async (name, args) => {
            calls.push([name, args]);
            if (name === 'tiktok_engines') return catalog;
            if (name === 'prepare_tiktok') {
                assert.equal(get('engine-select').disabled, true);
                if (fail) throw new Error('FFmpeg no encontrado');
                return {path: 'C:/video_tiktok.mp4', windows_video: false};
            }
        } } } }
    };
    vm.runInNewContext(source, context);
    await new Promise(resolve => setImmediate(resolve));
    return {get, calls};
}
test('six engines render their options and submit the selected engine without losing the source', async () => {
    const {get, calls} = await fixture();
    assert.equal(get('engine-select').children.length, 6);
    get('video-path').value = 'C:/video con espacios.mp4';
    for (const engine of catalog) {
        get('engine-select').value = engine.id;
        get('engine-select').onchange();
        assert.equal(get('engine-options').querySelectorAll('select').length, engine.fields.length);
        assert.ok(get('engine-use').textContent.length > 30);
        assert.ok(get('engine-change').textContent.length > 30);
        assert.ok(get('engine-limit').textContent.length > 30);
        assert.equal(get('engine-mode-guide').hidden, engine.id !== 'upload120');
        await get('compress-btn').onclick();
        const [name, args] = calls.at(-1);
        assert.equal(name, 'prepare_tiktok');
        assert.equal(args.engine, engine.id);
        assert.equal(args.input, 'C:/video con espacios.mp4');
        assert.deepEqual(JSON.parse(JSON.stringify(args.options)), Object.fromEntries(engine.fields.map(field => [field.id, field.default])));
        assert.equal(get('engine-select').disabled, false);
        assert.equal(get('original-btn').disabled, false);
        assert.match(get('status-title').textContent, /Windows no reconoce/);
    }
});
test('Upload120 guidance follows its selected mode and clears for another engine', async () => {
    const {get} = await fixture();
    get('engine-select').value = 'upload120';
    get('engine-select').onchange();
    const method = get('engine-options').querySelectorAll('select').find(input => input.dataset.field === 'method');
    assert.match(get('engine-mode-guide').textContent, /Web Signal/);
    method.value = 'balanced-sync';
    method.onchange();
    assert.match(get('engine-mode-guide').textContent, /Balanced Sync/);
    get('engine-select').value = 'mistic';
    get('engine-select').onchange();
    assert.equal(get('engine-mode-guide').textContent, '');
    assert.match(get('engine-change').textContent, /Recodifica/);
    get('engine-select').value = 'upload120';
    get('engine-select').onchange();
    assert.match(get('engine-mode-guide').textContent, /Balanced Sync/);
});
test('failed processing restores controls and leaves output actions disabled', async () => {
    const {get} = await fixture(true);
    get('video-path').value = 'C:/video.mp4';
    await get('compress-btn').onclick();
    assert.match(get('status-detail').textContent, /FFmpeg no encontrado/);
    assert.equal(get('compress-btn').disabled, false);
    assert.equal(get('engine-select').disabled, false);
    assert.equal(get('original-btn').disabled, true);
    assert.equal(get('clear-btn').disabled, true);
});
