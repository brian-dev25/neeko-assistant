// Use rendered alpha (including transparent textures), not the canvas bounding box.
export function alphaRects(pixels, width, height, bounds) {
    const rects = [];
    const sx = bounds.width / width, sy = bounds.height / height;
    for (let y = 0; y < height; y++) {
        let start = -1;
        for (let x = 0; x <= width; x++) {
            const solid = x < width && pixels[(y * width + x) * 4 + 3] > 0;
            if (solid && start < 0) start = x;
            if (!solid && start >= 0) {
                // One-cell margin avoids shaving off antialiased moving edges.
                rects.push({ x: bounds.x + Math.max(0, start - 1) * sx,
                    y: bounds.y + Math.max(0, y - 1) * sy,
                    width: (Math.min(width, x + 1) - Math.max(0, start - 1)) * sx,
                    height: (Math.min(height, y + 2) - Math.max(0, y - 1)) * sy });
                start = -1;
            }
        }
    }
    return rects;
}

export class PetRegion {
    constructor(invoke) {
        this.invoke = invoke;
        this.canvas = document.createElement('canvas');
        this.context = this.canvas.getContext('2d', { willReadFrequently: true });
        this.enabled = false;
        this.shape = [];
        this.lastCapture = -Infinity;
        // Clear a region left by a hot reload before enabling the addon again.
        this.queue(null);
    }
    enable(value) {
        this.enabled = value;
        clearInterval(this.timer);
        this.shape = [];
        this.lastCapture = -Infinity;
        if (value) this.timer = setInterval(() => this.refresh(), 60);
        else this.queue(null);
    }
    capture(source, bounds, now = performance.now()) {
        if (!this.enabled || now - this.lastCapture < 60 || bounds.width <= 0 || bounds.height <= 0) return;
        this.lastCapture = now;
        try {
            const width = Math.min(200, Math.max(1, Math.ceil(bounds.width / 4)));
            const height = Math.min(240, Math.max(1, Math.ceil(bounds.height / 4)));
            this.canvas.width = width;
            this.canvas.height = height;
            this.context.drawImage(source, 0, 0, width, height);
            const pixels = this.context.getImageData(0, 0, width, height).data;
            const shape = alphaRects(pixels, width, height, bounds);
            // During model/image loading retain the last usable shape.
            if (shape.length) this.shape = shape;
            this.refresh();
        } catch (error) {
            console.error('[Neeko region]', error);
            this.enable(false); // Leave controls recoverable if capture is unavailable.
        }
    }
    refresh() {
        if (!this.enabled) return;
        const ui = [];
        for (const element of document.querySelectorAll('.modal, dialog[open], #airi-pet-menu, #speech-bubble')) {
            if (element.hidden || !element.getClientRects().length) continue;
            const rect = element.getBoundingClientRect();
            ui.push({ x: rect.x - 4, y: rect.y - 4, width: rect.width + 8, height: rect.height + 8 });
        }
        if (!this.shape.length && !ui.length) return;
        this.queue([...this.shape, ...ui]);
    }
    queue(rects) {
        const key = JSON.stringify(rects);
        if (key === this.key) return;
        this.key = key;
        this.pending = { rects };
        void this.flush();
    }
    async flush() {
        if (this.sending) return;
        this.sending = true;
        try {
            while (this.pending) {
                const next = this.pending;
                this.pending = null;
                await this.invoke('pet_set_region', next);
            }
        } catch (error) {
            this.key = undefined;
            console.error('[Neeko region]', error);
        } finally {
            this.sending = false;
            if (this.pending) void this.flush();
        }
    }
}
