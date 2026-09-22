import test from 'node:test';
import assert from 'node:assert/strict';
import { validateAvatar } from '../src/avatar.mjs';

function glb(overrides = {}) {
    const model = { asset: { version: '2.0' }, animations: ['Neeko_idle3.anm', 'Idlein_Animal', 'Joke_Loop'].map(name => ({ name })), ...overrides };
    const json = new TextEncoder().encode(JSON.stringify(model));
    const length = Math.ceil(json.length / 4) * 4;
    const bytes = new ArrayBuffer(20 + length);
    const view = new DataView(bytes);
    [0x46546c67, 2, bytes.byteLength, length, 0x4e4f534a].forEach((value, i) => view.setUint32(i * 4, value, true));
    new Uint8Array(bytes, 20).fill(32);
    new Uint8Array(bytes, 20).set(json);
    return bytes;
}

test('Neeko GLB metadata accepts embedded assets and required animations', () => {
    assert.equal(validateAvatar(glb({ images: [{ bufferView: 0 }] }), 'Neeko.GLB').asset.version, '2.0');
});

test('broken downloads and unsupported exports fail before storage', () => {
    assert.throws(() => validateAvatar(glb(), 'Neeko.png'), /\.glb/);
    assert.throws(() => validateAvatar(new ArrayBuffer(0), 'Neeko.glb'), /incompleto/);
    const truncated = glb().slice(0, -4);
    assert.throws(() => validateAvatar(truncated, 'Neeko.glb'), /válido/);
    const wrongVersion = glb();
    new DataView(wrongVersion).setUint32(4, 1, true);
    assert.throws(() => validateAvatar(wrongVersion, 'Neeko.glb'), /válido/);
    assert.throws(() => validateAvatar(glb({ images: [{ uri: 'texture.png' }] }), 'Neeko.glb'), /sin archivos externos/);
    assert.throws(() => validateAvatar(glb({ buffers: [{ uri: 'https://example.com/model.bin' }] }), 'Neeko.glb'), /sin archivos externos/);
});

test('exports without the Neeko animation set are rejected', () => {
    assert.throws(() => validateAvatar(glb({ animations: [] }), 'other.glb'), /Neeko con todas sus animaciones/);
    assert.throws(() => validateAvatar(glb({ animations: [{ name: 'Neeko_idle3.anm' }] }), 'Neeko.glb'), /Neeko con todas sus animaciones/);
});
