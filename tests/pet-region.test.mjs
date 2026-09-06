import { test } from 'node:test';
import assert from 'node:assert/strict';
import { alphaRects, PetRegion } from '../src/pet-region.mjs';

test('alpha silhouette excludes transparent margins and holes at different sizes', () => {
    const pixels = new Uint8ClampedArray(32 * 32 * 4);
    for (let y = 4; y < 28; y++) for (let x = 4; x < 28; x++) {
        if (x >= 12 && x < 20 && y >= 12 && y < 20) continue;
        pixels[(y * 32 + x) * 4 + 3] = 255;
    }
    for (const scale of [1, 2, 4]) {
        const rects = alphaRects(pixels, 32, 32, { x: 10, y: 20, width: 32 * scale, height: 32 * scale });
        const hit = (x, y) => rects.some(r => x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height);
        assert(!hit(10 + scale, 20 + 10 * scale));
        assert(hit(10 + 8 * scale, 20 + 8 * scale));
        assert(!hit(10 + 16 * scale, 20 + 16 * scale));
    }
    assert.deepEqual(alphaRects(new Uint8ClampedArray(16), 2, 2, {x:0,y:0,width:10,height:10}), []);
});

test('disabling restores the native region after an in-flight shape update', async () => {
    const original = globalThis.document;
    globalThis.document = { createElement: () => ({ getContext: () => ({}) }) };
    try {
        const calls = [], finishes = [];
        const region = new PetRegion((command, args) => {
            calls.push(args.rects);
            return new Promise(resolve => finishes.push(resolve));
        });
        finishes.shift()();
        await Promise.resolve();
        region.queue([{ x:10,y:20,width:30,height:40 }]);
        region.enable(false);
        assert.equal(calls.length, 2);
        finishes.shift()();
        await Promise.resolve();
        assert.equal(calls.length, 3);
        assert.equal(calls[2], null);
        finishes.shift()();
        await Promise.resolve();
    } finally { globalThis.document = original; }
});
