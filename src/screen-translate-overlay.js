(() => {
    const invoke = window.__TAURI__.core.invoke;
    let revision = -1;
    let latest, blockKey = '';
    function spatial(state) {
        const host=document.getElementById('blocks');
        const key=JSON.stringify([state.blocks,state.overlayRegion,state.settings,innerWidth,innerHeight]);
        if(key===blockKey) return;blockKey=key;host.replaceChildren();
        if(state.settings.mode!=='over' || !state.overlayRegion) return;
        const region=state.overlayRegion;
        const sx=innerWidth/region.width, sy=innerHeight/region.height;
        const vertical=state.settings.verticalText;
        const boxes=(state.blocks ?? []).map(block=>({block,x:(block.x-region.x)*sx,y:(block.y-region.y)*sy,w:block.width*sx,h:block.height*sy}));
        const placed=[];
        const overlaps=(a,b)=>a.x<b.x+b.w && a.x+a.w>b.x && a.y<b.y+b.h && a.y+a.h>b.y;
        for(const box of boxes) {
            const block=box.block;
            const rect={x:Math.max(0,box.x),y:Math.max(0,box.y),w:Math.max(1,Math.min(box.w,innerWidth-Math.max(0,box.x))),h:Math.max(1,Math.min(box.h,innerHeight-Math.max(0,box.y)))};
            const obstacles=boxes.filter(other=>other!==box).concat(placed);
            if(state.settings.expandBlocks !== false && !obstacles.some(other=>overlaps(rect,other))) {
                let right=innerWidth;
                for(const other of obstacles) if(other.x>=rect.x+rect.w && other.y<rect.y+rect.h && other.y+other.h>rect.y) right=Math.min(right,other.x-3);
                rect.w=Math.max(rect.w,Math.min(right-rect.x,Math.max(rect.w,rect.w*1.5)));
                let bottom=innerHeight;
                for(const other of obstacles) if(other.y>=rect.y+rect.h && other.x<rect.x+rect.w && other.x+other.w>rect.x) bottom=Math.min(bottom,other.y-3);
                rect.h=Math.max(rect.h,Math.min(bottom-rect.y,rect.h*(state.settings.showOriginal?3:2)));
            }
            placed.push(rect);
            const element=document.createElement('div');element.className='ocr-block' + (vertical ? ' vertical' : '');
            if(state.settings.showOriginal) {const original=document.createElement('span');original.className='block-original';original.textContent=block.text; element.append(original,document.createElement('br'));}
            const translated=document.createElement('span');translated.textContent=block.translation;element.append(translated);
            element.style.left=`${rect.x}px`;element.style.top=`${rect.y}px`;
            element.style.width=`${rect.w}px`;element.style.height=`${rect.h}px`;
            element.title=block.text+'\n'+block.translation;host.append(element);
            if(state.settings.autoColors && /^#[0-9a-f]{6}$/i.test(block.background ?? '') && /^#[0-9a-f]{6}$/i.test(block.foreground ?? '')) {
                const bg=block.background;
                element.style.background=`rgba(${parseInt(bg.slice(1,3),16)},${parseInt(bg.slice(3,5),16)},${parseInt(bg.slice(5,7),16)},${state.settings.opacity/100})`;
                element.style.color=block.foreground;element.style.setProperty('--outline-color',bg);
            }
            if(state.settings.autoFontSize) {
                let low=state.settings.minFontSize ?? 10, high=Math.max(low,state.settings.maxFontSize ?? state.settings.fontSize);
                for(let attempt=0;attempt<9;attempt++) {
                    const size=(low+high)/2;element.style.fontSize=`${size}px`;
                    if(element.scrollHeight>element.clientHeight+1 || element.scrollWidth>element.clientWidth+1) high=size;else low=size;
                }
                element.style.fontSize=`${low}px`;
            } else {element.style.fontSize=`${state.settings.fontSize}px`;}
            if(element.scrollHeight>element.clientHeight+1 || element.scrollWidth>element.clientWidth+1) {
                element.classList.add('overflowing');element.title+='\nTexto completo disponible en Traducci?n actual.';
            }
        }
    }

    function render(state) {
        if (state.revision < revision) return;
        revision = state.revision;
        const previousTranslation = latest?.translation;
        latest=state;
        document.body.dataset.mode=state.settings.mode ?? 'layer';
        document.body.classList.toggle('locked',!!state.locked);
        document.getElementById('pause').textContent=state.paused ? 'Reanudar' : 'Pausar';
        const status = document.getElementById('status');
        status.textContent = state.message;
        status.hidden = Boolean(state.translation) && state.phase !== 'error' && !state.paused;
        const original = document.getElementById('original');
        original.querySelector('span').textContent = state.original;
        original.hidden = !state.settings.showOriginal || !state.original;
        const translation = document.getElementById('translation');
        translation.querySelector('span').textContent = state.translation;
        translation.hidden = !state.translation;
        document.documentElement.style.setProperty('--font-size', `${state.settings.fontSize}px`);
        document.documentElement.style.setProperty('--opacity', String(state.settings.opacity / 100));
        const root=document.documentElement.style;
        root.setProperty('--text-color',state.settings.textColor ?? '#ffffff');root.setProperty('--outline-color',state.settings.outlineColor ?? '#000000');
        const color=state.settings.backgroundColor ?? '#000000';
        const bgRgba=`rgba(${parseInt(color.slice(1,3),16)},${parseInt(color.slice(3,5),16)},${parseInt(color.slice(5,7),16)},${state.settings.opacity/100})`;
        root.setProperty('--background',bgRgba);
        if(state.settings.mode!=='over') document.body.style.background=bgRgba;
        else document.body.style.background='transparent';
        root.fontFamily=state.settings.fontFamily ?? 'Segoe UI';root.textAlign=state.settings.alignment ?? 'left';
        spatial(state);
        if(state.settings.ttsEnabled && state.translation && state.translation !== previousTranslation) {
            try {
                const utterance=new SpeechSynthesisUtterance(state.translation);
                utterance.rate=1 + (state.settings.ttsRate ?? 0) * 0.1;
                utterance.lang=state.settings.target ?? 'es';
                window.speechSynthesis.cancel();
                window.speechSynthesis.speak(utterance);
            } catch(e) {}
        }
    }
    const call = action => invoke('screen_translate', { action, settings: null, profiles: null });
    const error = e => { const status = document.getElementById('status'); status.hidden = false; status.textContent = String(e); };
    document.getElementById('stop').addEventListener('click', () => call('stop').catch(error));
    document.getElementById('pause').addEventListener('click',()=>call('pause').catch(error));
    document.getElementById('lock').addEventListener('click',()=>call('lock').catch(error));
    window.addEventListener('resize',()=>{if(latest) spatial(latest);});
    document.querySelector('[data-drag]').addEventListener('pointerdown', e => {
        if (e.button === 0) window.__TAURI__.window.getCurrentWindow().startDragging().catch(error);
    });
    document.addEventListener('keydown', e => { if (e.key === 'Escape') call('stop').catch(error); });
    (async () => {
        await window.__TAURI__.event.listen('screen-translate:status', ({ payload }) => render(payload));
        render(await call('status'));
    })().catch(error);
})();
