(() => {
    const panel = document.createElement('div');
    panel.className = 'st-panel';
    panel.innerHTML = `
        <h4>Screen Translate</h4>
        <p>Traducí los diálogos de un juego desde Neeko. OCR local y Google Web por defecto, como MORT 1.291. No usa tu IA.</p>
        <p class="st-hint">La captura empieza únicamente al pulsar Traducir. Solo el texto reconocido se envía al traductor web elegido; las imágenes no se guardan ni se envían. Usá ventana o ventana sin bordes.</p>
        <fieldset><legend>1 · Preparar OCR</legend>
            <div class="st-actions"><button type="button" data-action="prepare">Comprobar motor e idiomas</button>
            <button type="button" data-action="language-settings">Idiomas de Windows</button></div>
            <label>Motor OCR<select name="engine"><option value="windows">Windows OCR</option><option value="tesseract">Tesseract · idiomas independientes</option><option value="oneocr">OneOCR · Recortes de Windows</option></select></label>
            <button type="button" data-action="install">Descargar / reparar idioma Tesseract seleccionado</button>
            <label class="st-check"><input name="tesseractEnglishFilter" type="checkbox" checked> Limitar caracteres de Tesseract inglés</label>
            <p class="st-hint">Reduce símbolos interpretados como texto. Conserva números, % y +; desactivalo si necesitás otros caracteres. Solo afecta a Tesseract con idioma inglés.</p>
            <p>Windows usa sus idiomas instalados. Tesseract ofrece datos Normal, Rápido y Precisión; descarga solo la variante elegida desde el repositorio oficial. Precisión puede tardar más. No usa la IA de Neeko.</p>
            <p>OneOCR detecta idiomas automáticamente y prepara una copia local de los componentes de Recortes/Fotos instalados. Si no está disponible, elegí Tesseract.</p>
        </fieldset>
        <fieldset data-options><legend>2 · Configurar traducción</legend>
            <label>Leer texto desde<select name="inputMode"><option value="screen">Pantalla (OCR)</option><option value="clipboard">Portapapeles (sin OCR)</option></select></label>
            <label>Traductor<select name="provider"><option value="google">Google Web</option><option value="papago">Papago Web</option><option value="db">Diccionario local · sin red</option></select></label>
            <label>Idioma del texto (OCR)<select name="source"><option value="">Comprobá OCR primero</option></select></label>
            <label>Idioma de origen al traducir<select name="translationSource"><option value="">Según el motor OCR</option><option value="auto">Detectar automáticamente</option></select></label>
            <p class="st-hint">Con OneOCR o portapapeles, fijá el idioma de origen si la detección confunde nombres o frases cortas. Papago admite menos idiomas que Google.</p>
            <label>Traducir a<select name="target">
                <option value="es">Español</option><option value="en">Inglés</option><option value="pt">Portugués</option>
                <option value="fr">Francés</option><option value="de">Alemán</option><option value="it">Italiano</option>
                <option value="ja">Japonés</option><option value="ko">Coreano</option><option value="zh">Chino</option>
            </select></label>
            <label>Intervalo entre lecturas (ms)<input name="intervalMs" type="number" min="500" max="10000" step="100" value="1000"></label>
            <label>Letra del overlay<input name="fontSize" type="number" min="14" max="48" value="20"></label>
            <label>Fondo del texto (0 = transparente)<input name="opacity" type="number" min="0" max="100" value="67"></label>
            <label class="st-check"><input name="showOriginal" type="checkbox" checked> Mostrar texto original</label>
            <label>Capturar<select name="windowId"><option value="">Escritorio · ventana o sin bordes</option></select></label>
            <label class="st-check"><input name="followMouse" type="checkbox"> Seguir el mouse con la región principal</label>
            <div class="st-actions"><button type="button" data-action="select">Seleccionar región</button>
            <button type="button" data-action="select-add">Añadir región</button><button type="button" data-action="select-exclude">Excluir región</button>
            <button type="button" data-reset-regions>Quitar regiones adicionales y exclusiones</button>
            <button type="button" data-action="settings">Guardar configuración</button></div>
            <p data-region>Sin región seleccionada.</p>
            <details><summary>Imagen y estabilidad</summary>
                <label>Escala de imagen<input name="scale" type="number" min="1" max="3" step="0.25" value="2"></label>
                <label>Umbral (0 = desactivado)<input name="threshold" type="number" min="0" max="255" value="0"></label>
                <label class="st-check"><input name="invert" type="checkbox"> Invertir colores antes del OCR</label>
                <label>Brillo (-100 a 100)<input name="brightness" type="number" min="-100" max="100" value="0"></label>
                <label>Contraste (-100 a 100)<input name="contrast" type="number" min="-100" max="100" value="0"></label>
                <label class="st-check"><input name="grayscale" type="checkbox"> Escala de grises antes del OCR</label>
                <label class="st-check"><input name="regionMouseFollow" type="checkbox"> Región sigue al mouse</label>
                <label>Espera opcional de texto (ms)<input name="stabilizeMs" type="number" min="0" max="2000" step="100" value="0"></label>
                <p>0 traduce cada lectura nueva, como MORT. Una espera mayor ayuda con diálogos que aparecen letra por letra; se limita a un máximo de dos veces la espera (mínimo 1 segundo), más el tiempo de lectura.</p>
                <label>Conservar texto cuando desaparece (ms)<input name="retainMs" type="number" min="0" max="10000" step="100" value="0"></label>
            </details>
            <details open><summary>Ventana de traducción</summary>
                <label>Presentación<select name="mode"><option value="over">Sobre el texto original (MORT)</option><option value="layer">Texto flotante</option><option value="dark">Panel de resultados</option></select></label>
                <label>Tipografía<select name="fontFamily"><option>Malgun Gothic</option><option>Segoe UI</option><option>Arial</option><option>Verdana</option><option>Georgia</option><option>Consolas</option></select></label>
                <label>Alineación<select name="alignment"><option value="left">Izquierda</option><option value="center">Centro</option><option value="right">Derecha</option></select></label>
                <label>Texto<input name="textColor" type="color" value="#ffffff"></label>
                <label>Fondo<input name="backgroundColor" type="color" value="#000000"></label>
                <label>Contorno<input name="outlineColor" type="color" value="#000000"></label>
                <label class="st-check"><input name="verticalText" type="checkbox"> Texto vertical (japonés)</label>
                <label class="st-check"><input name="autoFontSize" type="checkbox"> Tamaño automático de fuente</label>
                <label>Fuente m?nima (px)<input name="minFontSize" type="number" min="8" max="48" value="10"></label>
                <label>Fuente m?xima (px)<input name="maxFontSize" type="number" min="8" max="72" value="48"></label>
                <label class="st-check"><input name="expandBlocks" type="checkbox" checked> Ampliar cajas sin tapar otros bloques</label>
                <label class="st-check"><input name="autoColors" type="checkbox"> Colores autom?ticos desde la imagen</label>
                <label class="st-check"><input name="ttsEnabled" type="checkbox"> Leer traducción en voz alta</label>
                <label>Velocidad TTS (-10 a 10)<input name="ttsRate" type="number" min="-10" max="10" value="0"></label>
                <p>El modo sobre bloques deja pasar los clics al juego. Ctrl + Alt + F10 permite desbloquearlo.</p>
            </details>
            <details><summary>Texto y traducci?n</summary>
                <label class="st-check"><input name="mergeParagraphs" type="checkbox" checked> Agrupar l?neas de un mismo p?rrafo</label>
                <label class="st-check"><input name="joinLines" type="checkbox" checked> Unir saltos de l?nea dentro de cada bloque al traducir</label>
                <label class="st-check"><input name="batchTranslation" type="checkbox" checked> Traducir bloques juntos (con comprobaci?n de separadores)</label>
                <label class="st-check"><input name="persistentCache" type="checkbox"> Conservar cach? de traducciones al reiniciar</label>
                <p>La cach? guarda texto y traducciones en este equipo. No a?ade un historial al panel.</p>
                <button type="button" data-action="clear-cache">Borrar cach? guardada</button>
            </details>
            <details><summary>Correcciones y diccionario por juego</summary>
                <label class="st-check"><input name="wholeWords" type="checkbox"> Aplicar correcciones solo a palabras completas</label>
                <label>Corregir OCR · una regla «original =&gt; corrección» por línea<textarea name="corrections" rows="4" placeholder="Nccko => Neeko"></textarea></label>
                <label>Traducciones preferidas · frases completas<textarea name="glossary" rows="4" placeholder="Start game => Iniciar partida"></textarea></label>
                <p>El diccionario local reemplaza frases completas conocidas; las demás se muestran en su idioma original.</p>
                <details><summary>Importar diccionario MORT</summary>
                    <p>Cargá archivos .txt en formato MORT DB (/s original /t traducción /e) o formato simple (original =&gt; traducción). Se mergean con el diccionario actual.</p>
                    <input type="file" id="mort-import" accept=".txt,.db" multiple style="display:none">
                    <div class="st-actions">
                        <button type="button" data-action="import-mort-db">Importar archivos MORT DB</button>
                        <button type="button" data-action="import-mort-simple">Importar formato simple (→)</button>
                        <button type="button" data-action="export-mort-db">Exportar como MORT DB</button>
                        <button type="button" data-action="clear-glossary">Limpiar diccionario</button>
                    </div>
                    <p data-mort-count></p>
                </details>
            </details>
        </fieldset>
        <fieldset data-profiles><legend>Perfiles por juego</legend>
            <label>Perfil guardado<select data-profile-list><option value="">Elegí un perfil</option></select></label>
            <div class="st-actions"><button type="button" data-profile-load>Cargar</button><button type="button" data-profile-delete>Borrar perfil</button></div>
            <label>Nombre<input data-profile-name type="text" maxlength="60" placeholder="Ej.: diálogos de mi juego"></label>
            <button type="button" data-profile-save>Guardar perfil actual</button>
        </fieldset>
        <fieldset><legend>Configuración</legend>
            <div class="st-actions">
                <button type="button" data-action="export">Exportar configuración</button>
                <button type="button" data-action="import">Importar configuración</button>
                <button type="button" data-action="defaults">Aplicar valores de MORT 1.291</button>
                <button type="button" data-action="toggle-remote">Mostrar control remoto</button>
            </div>
        </fieldset>
        <fieldset><legend>3 · Traducir</legend>
            <p>Requiere Internet, sin clave API ni modelo local. Google Web puede limitar solicitudes; ante un error la sesión se detiene.</p>
            <div class="st-actions"><button type="button" data-action="once">Traducir una vez</button>
            <button type="button" data-action="start">Iniciar continuo</button>
            <button type="button" data-action="stop">Detener</button>
            <button type="button" data-action="pause">Pausar / reanudar</button>
            <button type="button" data-action="lock">Bloquear / desbloquear clics</button>
            <button type="button" data-action="hide">Ocultar overlay</button>
            <button type="button" data-action="overlay">Mostrar overlay</button></div>
            <p>Ctrl + Alt + F6 inicia · F7 toma puntual · F8 pausa · F9 detiene · F10 bloquea clics · F11 muestra/oculta. Se habilitan al preparar el addon.</p>
            <p data-metrics></p>
            <p role="status" aria-live="polite" data-status>Consultando estado…</p>
            <p role="alert" data-error hidden></p>
        </fieldset>
        <fieldset><legend>Traducción actual</legend>
            <p data-result-empty>La traducción aparecerá acá y en la ventana de traducción.</p>
            <p class="st-original" data-result-original hidden></p>
            <p data-result-translation hidden></p>
        </fieldset>`;
    const $ = selector => panel.querySelector(selector);
    let disposed = false, busy = false, state = null, off, offShortcuts;
    let settingsKey = '', languagesKey = '', profilesKey = '', windowsKey = '';
    const displayNames = new Intl.DisplayNames(['es'], { type: 'language' });
    const targetCodes = 'af am ar as az be bg bn bo br bs ca ceb co cs cy da de dv dz el en eo es et eu fa fi fil fo fr fy ga gd gl gu ha haw he hi hr ht hu hy id ig is it iu ja jw ka kk km kn ko ku ky la lb lo lt lv mg mi mk ml mn mr ms mt my ne nl no ny oc or pa pl ps pt qu ro ru sa sd si sk sl sm sn so sq sr st su sv sw ta te tg th ti tk tl tr tt ug uk ur uz vi xh yi yo zh-CN zh-TW zu'.split(' ');
    $('[name="target"]').replaceChildren(...targetCodes.map(code => new Option(displayNames.of(code), code)).sort((a,b)=>a.text.localeCompare(b.text,'es')));
    $('[name="target"]').value = 'es';
    $('[name="translationSource"]').append(...targetCodes.map(code => new Option(displayNames.of(code), code)).sort((a,b)=>a.text.localeCompare(b.text,'es')));
    function rules(name) { return $(`[name="${name}"]`).value.split('\n').filter(line=>line.trim()).map(line=> {
        const index=line.indexOf('=>'); if(index<1) throw new Error('Cada regla necesita original => reemplazo.');
        return {from:line.slice(0,index).trim(),to:line.slice(index+2).trim()};
    }); }
    function languages(engine, selected) {
        const select=$('[name="source"]'); select.replaceChildren();
        for(const lang of state?.languages ?? []) if((lang.engine ?? 'windows')===engine)
            select.add(new Option(`${lang.name} (${lang.code})${lang.installed === false ? (engine==='tesseract' ? ` · descargar ${(lang.size/1048576).toFixed(1)} MB` : ' · no disponible') : ''}`,lang.code));
        if(!select.options.length) select.add(new Option('Comprobá el motor primero',''));
        if([...select.options].some(o=>o.value===selected)) select.value=selected;
        else if(engine==='tesseract' && [...select.options].some(o=>o.value==='eng')) select.value='eng';
    }
    $('[name="engine"]').addEventListener('change',()=>languages($('[name="engine"]').value,''));

    function error(value) { if (!disposed) { $('[data-error]').textContent = String(value); $('[data-error]').hidden = false; } }
    function readSettings() {
        for (const input of panel.querySelectorAll('input[type="number"]')) if (!input.reportValidity()) throw new Error('Revisá los valores de configuración.');
        return {
            ...(state?.settings ?? {}),
            ...Object.fromEntries(['colorFilter','filterColor','colorTolerance','erode','mergeParagraphs','joinLines','batchTranslation','persistentCache','wholeWords','minFontSize','maxFontSize','autoColors','expandBlocks'].map(name=>{
                const input=$(`[name="${name}"]`);return [name,input.type==='checkbox'?input.checked:input.type==='number'?Number(input.value):input.value];
            })),
            inputMode:$('[name="inputMode"]').value,
            region: state?.settings.region ?? null,
            source: $('[name="source"]').value, target: $('[name="target"]').value,
            translationSource: $('[name="translationSource"]').value,
            tesseractEnglishFilter: $('[name="tesseractEnglishFilter"]').checked,
            intervalMs: Number($('[name="intervalMs"]').value), fontSize: Number($('[name="fontSize"]').value),
            opacity: Number($('[name="opacity"]').value), showOriginal: $('[name="showOriginal"]').checked,
            engine:$('[name="engine"]').value, provider:$('[name="provider"]').value,mode:$('[name="mode"]').value,
            autoFontSize:$('[name="autoFontSize"]').checked,verticalText:$('[name="verticalText"]').checked,
            ttsEnabled:$('[name="ttsEnabled"]').checked,ttsRate:Number($('[name="ttsRate"]').value),
            scale:Number($('[name="scale"]').value),threshold:Number($('[name="threshold"]').value),invert:$('[name="invert"]').checked,
            brightness:Number($('[name="brightness"]').value),contrast:Number($('[name="contrast"]').value),grayscale:$('[name="grayscale"]').checked,
            retainMs:Number($('[name="retainMs"]').value),stabilizeMs:Number($('[name="stabilizeMs"]').value),followMouse:$('[name="followMouse"]').checked,
            windowId:$('[name="windowId"]').value,
            windowX:(state?.windows ?? []).find(w=>w.id===$('[name="windowId"]').value)?.x ?? state?.settings.windowX ?? 0,
            windowY:(state?.windows ?? []).find(w=>w.id===$('[name="windowId"]').value)?.y ?? state?.settings.windowY ?? 0,
            textColor:$('[name="textColor"]').value,backgroundColor:$('[name="backgroundColor"]').value,outlineColor:$('[name="outlineColor"]').value,
            fontFamily:$('[name="fontFamily"]').value,alignment:$('[name="alignment"]').value,
            corrections:rules('corrections'),glossary:rules('glossary'),
        };
    }
    async function call(action, settings = null, profiles = null) {
        const result = await Neeko.invoke('screen_translate', { action, settings, profiles });
        render(result);
        return result;
    }
    function controls() {
        const running = !!state?.active || ['preparing', 'selecting'].includes(state?.phase);
        $('[data-options]').disabled = busy || running || !state;
        $('[data-profiles]').disabled = busy || running || !state;
        $('[name="engine"]').disabled=busy || running || !state;
        $('[name="tesseractEnglishFilter"]').disabled=busy || running || !state;
        panel.querySelectorAll('[data-action]').forEach(button => {
            const action = button.dataset.action;
            button.disabled = busy || !state || (running && !['stop', 'overlay','pause','lock','hide'].includes(action));
            if (['start', 'once'].includes(action) && $('[name="inputMode"]').value !== 'clipboard') button.disabled ||= !state?.prepared || !state?.settings.region;
        });
    }
    function render(value) {
        if (disposed || !value || (state && value.revision < state.revision)) return;
        state = value;
        const langKey = JSON.stringify(value.languages);
        if (langKey !== languagesKey) {
            languagesKey = langKey;
            languages(value.settings.engine ?? 'windows',value.settings.source);
            settingsKey = '';
        }
        const key = JSON.stringify(value.settings);
        if (key !== settingsKey) {
            settingsKey = key;
            languages(value.settings.engine ?? 'windows',value.settings.source);
            for (const [name, field] of Object.entries(value.settings)) {
                const input = panel.querySelector(`[name="${name}"]`);
                if (!input) continue;
                if (name==='corrections' || name==='glossary') input.value=field.map(r=>`${r.from} => ${r.to}`).join('\n');
                else if (input.type === 'checkbox') input.checked = !!field;
                else input.value = field;
            }
        }
        const windowKey=JSON.stringify(value.windows ?? []);
        if(windowKey!==windowsKey) { windowsKey=windowKey;const select=$('[name="windowId"]');
            select.replaceChildren(new Option('Escritorio · ventana o sin bordes',''));
            for(const item of value.windows ?? []) select.add(new Option(item.title,item.id));
            if(value.settings.windowId) select.value=value.settings.windowId;
        }
        $('[data-metrics]').textContent=`Consultas: ${value.requests ?? 0} · Caché: ${value.cacheHits ?? 0} · ${value.paused ? 'Pausado' : value.locked ? 'Clics pasan al juego' : 'Overlay editable'}`;
        const region = value.settings.region;
        $('[data-region]').textContent = region ? `Principal: ${region.width} × ${region.height} px en (${region.x}, ${region.y}). Adicionales: ${value.settings.regions?.length ?? 0} · Exclusiones: ${value.settings.exclusions?.length ?? 0}.` : 'Sin región seleccionada.';
        const profileKey = JSON.stringify(value.profiles);
        if (profilesKey !== profileKey) {
            profilesKey = profileKey;
            const list = $('[data-profile-list]'), previous = list.value;
            list.replaceChildren(new Option('Elegí un perfil', ''));
            value.profiles.forEach((profile, index) => list.add(new Option(profile.name, String(index))));
            if ([...list.options].some(option => option.value === previous)) list.value = previous;
        }
        $('[data-result-empty]').hidden = Boolean(value.translation);
        $('[data-result-original]').textContent = value.original || '';
        $('[data-result-original]').hidden = !value.settings.showOriginal || !value.original;
        $('[data-result-translation]').textContent = value.translation || '';
        $('[data-result-translation]').hidden = !value.translation;
        $('[data-status]').textContent = value.message;
        $('[data-status]').dataset.phase = value.phase;
        controls();
    }
    $('[name="inputMode"]').addEventListener('change', () => {
        if ($('[name="inputMode"]').value === 'clipboard' && $('[name="mode"]').value === 'over') $('[name="mode"]').value = 'layer';
        controls();
    });
    async function perform(operation) {
        if (busy || disposed || !state) return;
        busy = true; $('[data-error]').hidden = true; controls();
        try { await operation(); }
        catch (e) { error(e); }
        finally { busy = false; if (!disposed) controls(); }
    }
    panel.querySelectorAll('[data-action]').forEach(button => button.addEventListener('click', () => perform(async () => {
        const action = button.dataset.action;
        if (['settings', 'start', 'once', 'select','select-add','select-exclude','install','export'].includes(action)) await call('settings', readSettings());
        if (action !== 'settings') await call(action);
    })));
    $('[data-reset-regions]').addEventListener('click',()=>perform(()=>call('settings',{...readSettings(),regions:[],exclusions:[]})));
    $('[data-profile-save]').addEventListener('click', () => perform(async () => {
        const name = $('[data-profile-name]').value.trim();
        if (!name) throw new Error('Escribí un nombre para el perfil.');
        const settings = readSettings();
        if (settings.inputMode !== 'clipboard' && !settings.region) throw new Error('Seleccioná una región antes de guardar el perfil.');
        const profiles = [...state.profiles];
        if (profiles.some(profile => profile.name.toLowerCase() === name.toLowerCase())) throw new Error('Ya existe ese perfil. Usá otro nombre o borrá el anterior.');
        profiles.push({ name, settings });
        await call('settings', settings, profiles);
        $('[data-profile-name]').value = '';
    }));
    function selectedProfile() {
        const value = $('[data-profile-list]').value;
        if (value === '' || !state.profiles[Number(value)]) throw new Error('Elegí un perfil guardado.');
        return Number(value);
    }
    $('[data-profile-load]').addEventListener('click', () => perform(() => call('settings', state.profiles[selectedProfile()].settings)));
    $('[data-profile-delete]').addEventListener('click', () => perform(() => {
        const index = selectedProfile();
        return call('settings', readSettings(), state.profiles.filter((_, i) => i !== index));
    }));
    function parseMortDb(text) {
        const rules = [];
        const lines = text.split(/\r?\n/);
        let i = 0;
        while (i < lines.length) {
            if (lines[i].trim() === '/s') {
                const fromParts = [];
                const toParts = [];
                i++;
                while (i < lines.length && lines[i].trim() !== '/t') {
                    fromParts.push(lines[i]);
                    i++;
                }
                if (i < lines.length) i++;
                while (i < lines.length && lines[i].trim() !== '/e') {
                    toParts.push(lines[i]);
                    i++;
                }
                if (i < lines.length) i++;
                const from = fromParts.join('\n').trim();
                const to = toParts.join('\n').trim();
                if (from && to) rules.push({ from, to });
            } else if (lines[i].includes('=>')) {
                const idx = lines[i].indexOf('=>');
                const from = lines[i].slice(0, idx).trim();
                const to = lines[i].slice(idx + 2).trim();
                if (from && to) rules.push({ from, to });
                i++;
            } else {
                i++;
            }
        }
        return rules;
    }
    function parseSimpleDict(text) {
        return text.split(/\r?\n/).filter(l => l.includes('=>')).map(l => {
            const idx = l.indexOf('=>');
            return { from: l.slice(0, idx).trim(), to: l.slice(idx + 2).trim() };
        }).filter(r => r.from && r.to);
    }
    function exportMortDb(rules) {
        return rules.map(r => `/s\n${r.from}\n/t\n${r.to}\n/e`).join('\n\n');
    }
    function updateMortCount() {
        const count = state?.settings?.glossary?.length ?? 0;
        const el = $('[data-mort-count]');
        if (el) el.textContent = count > 0 ? `${count} reglas en el diccionario actual.` : 'Diccionario vacío.';
    }
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.accept = '.txt,.db';
    fileInput.multiple = true;
    fileInput.style.display = 'none';
    panel.appendChild(fileInput);
    fileInput.addEventListener('change', () => {
        if (!fileInput.files.length) return;
        perform(async () => {
            const allRules = [];
            for (const file of fileInput.files) {
                const text = await file.text();
                allRules.push(...parseMortDb(text));
            }
            const existing = readSettings().glossary || [];
            const seen = new Set(existing.map(r => r.from));
            let added = 0;
            for (const rule of allRules) {
                if (!seen.has(rule.from)) {
                    existing.push(rule);
                    seen.add(rule.from);
                    added++;
                }
            }
            await call('settings', { ...readSettings(), glossary: existing });
            updateMortCount();
            if (added > 0) alert(`Agregadas ${added} reglas nuevas de ${allRules.length} totales.`);
            else alert(`No se agregaron reglas nuevas. Las ${allRules.length} reglas ya existían.`);
            fileInput.value = '';
        });
    });
    panel.querySelector('[data-action="import-mort-db"]')?.addEventListener('click', () => fileInput.click());
    panel.querySelector('[data-action="import-mort-simple"]')?.addEventListener('click', () => {
        const input = prompt('Pegá el texto con formato "original => traducción" (una por línea):');
        if (!input) return;
        perform(async () => {
            const newRules = parseSimpleDict(input);
            const existing = readSettings().glossary || [];
            const seen = new Set(existing.map(r => r.from));
            let added = 0;
            for (const rule of newRules) {
                if (!seen.has(rule.from)) {
                    existing.push(rule);
                    seen.add(rule.from);
                    added++;
                }
            }
            await call('settings', { ...readSettings(), glossary: existing });
            updateMortCount();
            alert(`Agregadas ${added} reglas nuevas.`);
        });
    });
    panel.querySelector('[data-action="export-mort-db"]')?.addEventListener('click', () => {
        const glossary = state?.settings?.glossary || [];
        if (!glossary.length) { alert('El diccionario está vacío.'); return; }
        const blob = new Blob([exportMortDb(glossary)], { type: 'text/plain' });
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = 'neeko-glossary.txt';
        a.click();
        URL.revokeObjectURL(a.href);
    });
    panel.querySelector('[data-action="clear-glossary"]')?.addEventListener('click', () => {
        if (!confirm('¿Borrar todo el diccionario?')) return;
        perform(async () => {
            await call('settings', { ...readSettings(), glossary: [] });
            updateMortCount();
        });
    });
    Neeko.ui.registerSettingsTab('screen-translate', 'Screen Translate', panel);
    Neeko.addon.onUnload(() => { disposed = true; off?.(); offShortcuts?.(); });
    controls();
    (async () => {
        try {
            const unsubscribe = await Neeko.events.on('screen-translate:status', ({ payload }) => render(payload));
            if (disposed) { unsubscribe?.(); return; }
            off = unsubscribe;
            if(!Neeko.ui.isSettingsWindow) {
                const unsubscribeShortcut=await Neeko.events.on('screen-translate:shortcut',({payload})=>{if(!disposed) call(payload).catch(error);});
                if(disposed)unsubscribeShortcut?.();else offShortcuts=unsubscribeShortcut;
            }
            await call('status');
        } catch (e) { error(e); }
    })();
})();
