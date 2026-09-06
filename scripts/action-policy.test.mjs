import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { ChatClient, confirmationControls, sourceUrl, attachInfo } from '../src/chat-client.mjs';

test('Info only links to a source, never approval metadata or URLs embedded in notes', () => {
    for (const value of [undefined, '', 'Action approved', 'Local memory: https://example.com', 'javascript:alert(1)', 'https://user:secret@example.com']) {
        assert.equal(sourceUrl(value), null);
        assert.equal(attachInfo({}, value), null);
    }
    assert.equal(sourceUrl('https://es.wikipedia.org/wiki/Per%C3%B3n'), 'https://es.wikipedia.org/wiki/Per%C3%B3n');
});

const proposal = { kind: 'action', message: 'Open compressor?', proposal_id: 'p1', details: 'Open video compressor' };
const deferred = () => { let resolve; const promise = new Promise(r => { resolve = r; }); return { resolve, promise }; };

test('model actions require explicit approval in both clients', async () => {
    for (const [approval, requires_confirmation] of [[true, true], [false, true], [true, undefined]]) {
        const choice = deferred();
        const decisions = [];
        const client = new ChatClient({ chat: async () => ({ ...proposal, requires_confirmation }), decide: async (...args) => { decisions.push(args); return 'result'; } }, {
            busy() {}, message() {}, error: assert.fail, confirm: () => choice.promise,
        }, 'session');
        const request = client.send('quiero comprimir');
        await Promise.resolve();
        assert.equal(decisions.length, 0);
        await client.send('second message');
        assert.equal(client.history.filter(m => m.role === 'user').length, 1);
        choice.resolve(approval);
        await request;
        assert.deepEqual(decisions, [['session', 'p1', approval]]);
        assert.equal(client.busy, false);
    }
});

test('explicit local commands execute without asking for confirmation', async () => {
    const decisions = [];
    const messages = [];
    const client = new ChatClient({
        chat: async () => ({ ...proposal, requires_confirmation: false }),
        decide: async (...args) => { decisions.push(args); return '192.168.0.4'; },
    }, {
        busy() {}, message: text => messages.push(text), error: assert.fail, confirm: assert.fail,
    }, 'local-session');
    await client.send('ip');
    assert.deepEqual(decisions, [['local-session', 'p1', true]]);
    assert.deepEqual(messages, ['192.168.0.4']);
});

test('saving a resumed account requires a separate explicit choice', async () => {
    for (const saveAccount of [false, true]) {
        const decisions = [];
        const shown = [];
        const client = new ChatClient({
            chat: async () => ({...proposal, requires_confirmation:true, can_save_account:true, info:'OP.GG'}),
            decide: async (...args) => { decisions.push(args); return 'match'; },
        }, {
            busy() {}, error:assert.fail, message:(...args) => shown.push(args),
            confirm: async () => ({approved:true, saveAccount}),
        }, 'followup');
        await client.send('Player#ABC');
        assert.deepEqual(decisions, [saveAccount ? ['followup','p1',true,true] : ['followup','p1',true]]);
        assert.deepEqual(shown, [['match','OP.GG']]);
    }
});

test('conversation and missing-detail questions never ask for execution', async () => {
    for (const kind of ['conversation', 'question']) {
        const messages = [];
        const client = new ChatClient({ chat: async () => ({kind,message:'hello'}), decide: assert.fail }, {
            busy() {}, message: text => messages.push(text), error: assert.fail, confirm: assert.fail,
        }, 'session');
        await client.send('hello');
        assert.deepEqual(messages, ['hello']);
    }
});

test('cancelled in-flight replies are discarded until cancellation is acknowledged', async () => {
    const response = deferred();
    const cancelled = deferred();
    const client = new ChatClient({ chat: () => response.promise, cancel: () => cancelled.promise, decide: assert.fail }, {
        busy() {}, message: assert.fail, error: assert.fail, confirm: assert.fail,
    }, 'session');
    const request = client.send('compress');
    const cancellation = client.cancel();
    response.resolve(proposal);
    await request;
    assert.equal(client.busy, true);
    cancelled.resolve();
    await cancellation;
    assert.equal(client.busy, false);
});

test('confirmation escapes details, rejects on abort and removes double-click handlers', async () => {
    for (const decision of ['yes', 'no', 'abort']) {
        const container = {hidden:true};
        const details = {};
        const yes = {focus() {}};
        const no = {focus() {}};
        const controller = new AbortController();
        const result = confirmationControls(container, details, yes, no, {details:'<script>unsafe</script>'}, controller.signal);
        assert.equal(details.textContent, '<script>unsafe</script>');
        if (decision === 'abort') controller.abort(); else (decision === 'yes' ? yes : no).onclick();
        assert.equal(await result, decision === 'yes');
        assert.equal(container.hidden, true);
        assert.equal(yes.onclick, null);
    }
});

test('bundled addons declare schemas and structured handlers that preserve their behavior', async () => {
    const manifest = JSON.parse(readFileSync(new URL('../addons/quick-notes/addon.json', import.meta.url)));
    const commands = new Map();
    const storage = new Map();
    const context = vm.createContext({
        Neeko: { commands: { register: (id, spec) => commands.set(id,spec), list: () => [...commands].map(([id,spec])=>({id,...spec})) }, ui: {registerSettingsTab() {}}, addon: {onUnload() {}} },
        localStorage: {getItem:key=>storage.get(key),setItem:(key,value)=>storage.set(key,value)},
        document: {documentElement:{lang:'en'},getElementById:()=>null},
        MutationObserver: class {observe() {}}, setTimeout() {},
    });
    vm.runInContext(readFileSync(new URL('../addons/quick-notes/main.js', import.meta.url),'utf8'),context);
    for (const command of manifest.commands) {
        assert.ok(command.ai.label);
        assert.equal(typeof commands.get(command.id).aiHandler,'function');
    }
    await commands.get('save-note').aiHandler({text:'buy milk'});
    assert.match((await commands.get('list-notes').aiHandler({})).message,/buy milk/);
    await commands.get('delete-note').aiHandler({number:1});
    assert.equal(JSON.parse(storage.get('quick-notes-notes')).length,0);
});
