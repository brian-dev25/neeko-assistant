// One lifecycle for the desktop character while the independent chat works.
export class PetActivity {
    constructor(render, schedule = (callback, delay) => setTimeout(callback, delay), unschedule = timer => clearTimeout(timer)) {
        this.render = render;
        this.schedule = schedule;
        this.unschedule = unschedule;
        this.state = 'idle';
    }

    update({ state, text = '' } = {}) {
        if (!['thinking', 'waiting', 'reply', 'idle', 'cancelled'].includes(state)) return;
        // ChatClient settles immediately after delivering a reply. Let its
        // speaking animation finish instead of clearing it in the same frame.
        if (state === 'idle' && this.state === 'reply') return;
        this.unschedule(this.timer);
        this.state = state;
        this.render({ thinking: state === 'thinking', talking: state === 'reply' });
        if (state === 'reply') {
            const duration = Math.max(2500, Math.min(10000, String(text).length * 45));
            this.timer = this.schedule(() => {
                this.state = 'idle';
                this.render({ thinking: false, talking: false });
            }, duration);
        }
    }
}
