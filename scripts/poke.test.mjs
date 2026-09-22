import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';
import { ChatClient, messageParts, pokemonImageUrl, renderMessageContent } from '../src/chat-client.mjs';

const source = readFileSync(new URL('../addons/poke/main.js', import.meta.url), 'utf8');
const manifest = JSON.parse(readFileSync(new URL('../addons/poke/addon.json', import.meta.url)));
const type = (double, half, zero = []) => ({ damage_relations: { double_damage_from: double.map(name => ({name})), half_damage_from: half.map(name => ({name})), no_damage_from: zero.map(name => ({name})) }, pokemon: [{pokemon:{name:'example'}}] });
const pokemon = (name, types) => ({ name, types: types.map(name => ({type:{name}})), stats:[{stat:{name:'speed'},base_stat:90}] });
const sprite = id => `https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/${id}.png`;
const art = id => `https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/other/official-artwork/${id}.png`;
const fixtures = {
    'type/fire/': type(['water','ground','rock'], ['fire','grass','ice','bug','steel','fairy']),
    'type/water/': type(['electric','grass'], ['fire','water','ice','steel']),
    'type/flying/': type(['electric','ice','rock'], ['grass','fighting','bug'], ['ground']),
    'type/ground/': type(['water','grass','ice'], ['poison','rock'], ['electric']),
    'type/rock/': type(['water','grass','fighting','ground','steel'], ['normal','fire','poison','flying']),
    'type/electric/': type(['ground'], ['electric','flying','steel']),
    'pokemon/charizard/': pokemon('charizard',['fire','flying']),
    'pokemon/pikachu/': { ...pokemon('pikachu',['electric']), sprites:{other:{'official-artwork':{front_default:art(25)}},front_default:sprite(25)}, species:{name:'pikachu'}, abilities:[], height:4, weight:60 },
    'pokemon-species/pikachu/': {id:25,generation:{name:'generation-i'},varieties:[{pokemon:{name:'pikachu'}}]},
    'pokemon/squirtle/': {...pokemon('squirtle',['water']),sprites:{front_default:sprite(7)}},
    'pokemon/sandshrew/': pokemon('sandshrew',['ground']),
    'pokemon-species/deoxys/': {varieties:[{is_default:true,pokemon:{name:'deoxys-normal'}}]},
    'pokemon/deoxys-normal/': pokemon('deoxys-normal',['fire'])
};
function setup(fetcher) {
    const commands = new Map(), storage = new Map(), calls = [];
    let unload;
    vm.runInNewContext(source, {
        AbortController, setTimeout, clearTimeout,
        localStorage: {getItem:k=>storage.get(k) || null,setItem:(k,v)=>storage.set(k,v)},
        fetch: async (url, options) => { const path = url.replace('https://pokeapi.co/api/v2/', ''); calls.push(path); return fetcher ? fetcher(url,options) : {ok:!!fixtures[path],status:fixtures[path]?200:404,json:async()=>fixtures[path]}; },
        Neeko:{commands:{register:(id,c)=>commands.set(id,c)},ui:{registerSettingsTab(){}},addon:{onUnload:cb=>unload=cb}}
    });
    return {commands,calls,unload, run:async(id,params)=>(await commands.get(id).aiHandler(params)).message};
}
test('all POKE tools match the manifest and expose AI handlers', () => {
    const {commands} = setup();
    assert.equal(commands.size, manifest.commands.length);
    for (const c of manifest.commands) { assert.equal(typeof commands.get(c.id).aiHandler,'function'); assert.deepEqual(JSON.parse(JSON.stringify(commands.get(c.id).patterns)),c.patterns); }
});
test('fire vs water describes both directions and water advantage', async () => {
    const s = setup();
    const result = await s.run('poke-compare',{first:'Fuego',second:'agua'});
    assert.match(result,/Fuego → Agua: Fuego ×0.5/);
    assert.match(result,/Agua → Fuego: Agua ×2/);
    assert.match(result,/Agua tiene ventaja/);
});
test('dual types multiply weaknesses, cancel resistances and preserve immunity', async () => {
    const result = await setup().run('poke-weaknesses',{name:'charizard'});
    assert.match(result,/Roca ×4/);
    assert.match(result,/Planta ×0.25/);
    assert.match(result,/Tierra ×0/);
    assert.doesNotMatch(result,/Hielo ×/);
});
test('Pokémon matchups use their actual types, including immunities', async () => {
    const s = setup();
    assert.match(await s.run('poke-compare',{first:'pikachu',second:'squirtle'}),/pikachu tiene ventaja/);
    const result = await s.run('poke-compare',{first:'pikachu',second:'sandshrew'});
    assert.match(result,/Eléctrico ×0/);
    assert.match(result,/sandshrew tiene ventaja/);
});
test('equal effectiveness does not invent a winner', async () => {
    assert.match(await setup().run('poke-compare',{first:'fire',second:'fuego'}),/No hay un favorito claro/);
});
test('species whose default form has a different name can be resolved', async () => {
    assert.match(await setup().run('poke-compare',{first:'deoxys',second:'water'}),/deoxys-normal/);
});
test('cache avoids repeat requests and deduplicates concurrent fetches', async () => {
    const s = setup();
    await s.run('poke-compare',{first:'fire',second:'fire'});
    await s.run('poke-compare',{first:'fire',second:'fire'});
    assert.deepEqual(s.calls,['type/fire/']);
});
test('unknown names, failed service and unload return actionable errors', async () => {
    assert.match(await setup().run('poke-info',{name:'inventado'}),/No encontré/);
    const failed = setup(async()=>({ok:false,status:503}));
    assert.match(await failed.run('poke-info',{name:'pikachu'}),/no está disponible/);
    const s=setup(); s.unload();
    assert.match(await s.run('poke-info',{name:'pikachu'}),/desactivado/);
});
test('Spanish examples and typo match command patterns', () => {
    const s = setup();
    for (const [id, text] of [['poke-compare','Fuego vs agua'],['poke-compare','pikachu vs squirtle'],['poke-weaknesses','¿que devilidades tiene fuego?'],['poke-weaknesses','¿qué le gana a fuego?']]) {
        assert.ok(s.commands.get(id).patterns.es.some(pattern=>new RegExp(pattern,'i').test(text)), text);
    }
});

test('fiches, weaknesses and comparisons include artwork with sprite fallback', async () => {
    const s = setup();
    for (const id of ['poke-info','poke-weaknesses']) {
        assert.deepEqual(messageParts(await s.run(id,{name:'pikachu'})).images,[{src:art(25),alt:'pikachu'}]);
    }
    const result = await s.run('poke-compare',{first:'pikachu',second:'squirtle'});
    assert.deepEqual(messageParts(result).images,[{src:art(25),alt:'pikachu'},{src:sprite(7),alt:'squirtle'}]);
    assert.equal(messageParts(await s.run('poke-compare',{first:'fire',second:'water'})).images.length,0);
    assert.equal(messageParts(await s.run('poke-compare',{first:'sandshrew',second:'water'})).images.length,0);
});

test('counter aliases route to weaknesses and return recommendations without AI', async () => {
    const s = setup();
    for (const text of ['pikachu counter','Pikachu counters','counter pikachu','counter de pikachu','counters para pikachu','¿pikachu counter?','poke pikachu counter']) {
        const command = s.commands.get('poke-weaknesses');
        const match = command.patterns.es.map(p => text.match(new RegExp(p,'i'))).find(Boolean);
        assert.ok(match,text);
        assert.equal(match[1].toLowerCase(),'pikachu');
        const result = await command.handler(match,text);
        assert.match(result.message,/Tierra ×2/);
        assert.match(result.message,/Tipos de ataque recomendados/);
    }
    const command=s.commands.get('poke-weaknesses');
    const match=command.patterns.es.map(p=>'fuego counter'.match(new RegExp(p,'i'))).find(Boolean);
    assert.match((await command.handler(match,'fuego counter')).message,/Agua ×2/);
    for(const text of ['counter','counterstrike','hola']) assert.ok(!command.patterns.es.some(p=>new RegExp(p,'i').test(text)));
});

test('unknown opponent queries select counters before comparisons', async () => {
    const s=setup();
    for(const text of ['pikachu vs ?','Pikachu vs   ?','pikachu vs.?','pikachu versus ?','poke pikachu contra ?','fuego vs ?']) {
        let selected;
        for(const [id,command] of s.commands) {
            const match=command.patterns.es.map(p=>text.match(new RegExp(p,'i'))).find(Boolean);
            if(match){selected={id,command,match};break;}
        }
        assert.equal(selected?.id,'poke-weaknesses',text);
        const response=await selected.command.handler(selected.match,text);
        assert.match(response.message,text.startsWith('fuego') ? /Agua ×2/ : /Tierra ×2/);
        assert.match(response.message,/Tipos de ataque recomendados/);
    }
});

test('image links survive action transport and render without interpreting HTML', async () => {
    const result = await setup().run('poke-compare',{first:'pikachu',second:'squirtle'});
    let delivered;
    const client = new ChatClient({chat:async()=>({kind:'action',proposal_id:'1',details:'POKE',requires_confirmation:false}),decide:async()=>result},
        {busy(){},message:text=>delivered=text,error:assert.fail},'image-test');
    await client.send('Pikachu vs Squirtle');
    const node = tag => ({tag,style:{},children:[],append(...children){this.children.push(...children);}});
    const previous = globalThis.document;
    globalThis.document = {createElement:node};
    try {
        const parent=node('div');
        renderMessageContent(parent,'<script>alert(1)</script>\n'+delivered);
        assert.match(parent.textContent,/<script>/);
        const cards=parent.children[0].children;
        assert.equal(cards.length,2);
        const [img,caption]=cards[0].children;
        assert.equal(img.src,art(25));
        assert.equal(img.alt,'pikachu');
        img.onerror();
        assert.equal(img.hidden,true);
        assert.match(caption.textContent,/imagen no disponible/);
    } finally { globalThis.document=previous; }
});

test('image parser rejects untrusted URLs and limits and deduplicates images', () => {
    for(const url of ['javascript:alert(1)','http://localhost/a.png','https://example.com/a.png',sprite(25)+'?x=1',sprite(25).replace('raw.githubusercontent.com','raw.githubusercontent.com.evil.test'),sprite(25).replace('https://','https://user@')]) {
        assert.equal(pokemonImageUrl(url),null);
    }
    const markup=Array.from({length:8},(_,id)=>`![name](${sprite(id)})`).join('\n');
    assert.equal(messageParts(markup).images.length,4);
    assert.equal(messageParts(`![name](${sprite(1)})\n![name](${sprite(1)})`).images.length,1);
    assert.equal(messageParts('texto normal').text,'texto normal');
});

test('POKE source becomes a compact external link without image placeholder gaps', async () => {
    const original = `Resultado\n\n![pikachu](${art(25)})\n\n![squirtle](${sprite(7)})\n\nFuente: PokéAPI (https://pokeapi.co/).`;
    assert.equal(messageParts(original).text, 'Resultado');
    const node = tag => ({tag,style:{},children:[],append(...v){this.children.push(...v);}});
    const previous=globalThis.document;
    globalThis.document={createElement:node};
    try {
        const parent=node('div');
        let opened, prevented=false;
        renderMessageContent(parent,original,async url=>{opened=url;});
        assert.equal(parent.textContent,'Resultado');
        const footer=parent.children.at(-1), link=footer.children[0];
        assert.equal(footer.className,'chat-source');
        assert.equal(link.textContent,'PokéAPI');
        await link.onclick({preventDefault(){prevented=true;}});
        assert.equal(prevented,true);
        assert.equal(opened,'https://pokeapi.co/');
        const web=node('div');
        renderMessageContent(web,'Fuego vs Agua\nFuente: PokéAPI (https://pokeapi.co/).');
        const webLink=web.children[0].children[0];
        assert.equal(webLink.href,'https://pokeapi.co/');
        assert.equal(webLink.target,'_blank');
        assert.equal(webLink.rel,'noopener noreferrer');
        assert.equal(webLink.onclick,undefined);
        assert.equal(messageParts('Fuente: PokéAPI (https://example.com/).').source,null);
    } finally {globalThis.document=previous;}
});

test('species color highlights exact names and captions without interpreting HTML', async () => {
    const base = fixtures['pokemon-species/pikachu/'];
    const s=setup(async url=>{
        const path=url.replace('https://pokeapi.co/api/v2/','');
        return {ok:true,status:200,json:async()=>path==='pokemon-species/pikachu/' ? {...base,color:{name:'yellow'}} : fixtures[path]};
    });
    const result=await s.run('poke-info',{name:'pikachu'});
    const parts=messageParts(result);
    assert.equal(parts.colors.pikachu,'yellow');
    assert.ok(!parts.text.includes('[poke-color:'));
    const node=tag=>({tag,style:{},children:[],append(...v){this.children.push(...v);}});
    const previous=globalThis.document;
    globalThis.document={createElement:node,createTextNode:text=>({textContent:text})};
    try {
        const parent=node('div');
        renderMessageContent(parent,'pikachu pikachus <b>pikachu</b>\n'+result);
        const colored=parent.children.filter(n=>n.tag==='span');
        assert.ok(colored.length>0);
        assert.ok(colored.every(n=>n.textContent==='pikachu' && n.style.color==='#ffe078'));
        assert.ok(parent.children.some(n=>n.textContent==='pikachus'));
        assert.ok(!parent.children.some(n=>n.tag==='b'));
        const gallery=parent.children.find(n=>n.className==='chat-image-gallery');
        assert.equal(gallery.children[0].children[1].style.color,'#ffe078');
    } finally {globalThis.document=previous;}
});

test('web chat executes POKE local actions and renders both images through its real adapter', async () => {
    const html = readFileSync(new URL('../web/index.html', import.meta.url), 'utf8');
    const start = html.indexOf('async function getChatClient()');
    const end = html.indexOf('let chatStarting', start);
    assert.ok(start > 0 && end > start);
    const result = await setup().run('poke-compare', {first:'pikachu',second:'squirtle'});
    const node = () => ({style:{},children:[],append(...v){this.children.push(...v);},prepend(v){this.children.unshift(v);},scrollIntoView(){}});
    const entries=[], requests=[];
    const document = {createElement:node,getElementById:node,querySelector:()=>null};
    const previous=globalThis.document;
    globalThis.document=document;
    try {
        const context=vm.createContext({
            sharedChatClient:null,chatModule:Promise.resolve(await import('../src/chat-client.mjs')),
            document,currentLanguage:'es',showTyping(){},hideTyping(){},scrollToBottom(){},
            addMessage(role,text){const entry=node();entry.textContent=text;entries.push(entry);return entry;},
            chatApi:async(path,payload)=>{
                requests.push({path,payload});
                if(path==='') return {kind:'action',proposal_id:'poke-web',details:'POKE',requires_confirmation:false};
                assert.equal(path,'/decide');
                assert.equal(payload.approved,true);
                return {ok:true,message:result};
            }
        });
        vm.runInContext(html.slice(start,end),context);
        const client=await context.getChatClient();
        await client.send('Pikachu vs Squirtle');
        assert.deepEqual(requests.map(r=>r.path),['','/decide']);
        assert.equal(entries.length,1);
        assert.match(entries[0].textContent,/pikachu tiene ventaja/);
        assert.equal(entries[0].children[0].textContent,'Neeko Asistente');
        assert.equal(entries[0].children[1].children.length,2);
    } finally { globalThis.document=previous; }
});
