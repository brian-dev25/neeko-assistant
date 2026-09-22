(function () {
    'use strict';
    const API = 'https://pokeapi.co/api/v2/';
    const names = { normal: 'Normal', fire: 'Fuego', water: 'Agua', electric: 'Eléctrico', grass: 'Planta', ice: 'Hielo', fighting: 'Lucha', poison: 'Veneno', ground: 'Tierra', flying: 'Volador', psychic: 'Psíquico', bug: 'Bicho', rock: 'Roca', ghost: 'Fantasma', dragon: 'Dragón', dark: 'Siniestro', steel: 'Acero', fairy: 'Hada' };
    const normalize = value => String(value || '').toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').trim();
    const slug = value => normalize(value).replace(/♀/g, '-f').replace(/♂/g, '-m').replace(/[.'’]/g, '').replace(/\s+/g, '-');
    const aliases = Object.fromEntries(Object.entries(names).flatMap(([id, name]) => [[id, id], [normalize(name), id]]));
    const label = id => names[id] || id;
    async function artwork(target) {
        const sprites = target.pokemon?.sprites;
        const candidates = [sprites?.other?.['official-artwork']?.front_default, sprites?.other?.home?.front_default, sprites?.front_default];
        const url = candidates.find(value => typeof value === 'string' && /^https:\/\/raw\.githubusercontent\.com\/PokeAPI\/sprites\/master\/sprites\/pokemon\/(?:other\/(?:official-artwork|home)\/)?\d+\.png$/.test(value));
        const name = target.name.replace(/[\[\]\r\n]/g, '').slice(0, 100);
        let color = '';
        if (target.pokemon?.species?.name) {
            try {
                const species = await get(`pokemon-species/${target.pokemon.species.name}/`);
                if (/^(black|blue|brown|gray|green|pink|purple|red|white|yellow)$/.test(species.color?.name) && /^[a-z0-9-]+$/.test(name)) {
                    color = `\n[poke-color:${name}:${species.color.name}]\n`;
                }
            } catch { /* Optional decoration must never prevent a response. */ }
        }
        return (url ? `\n![${name}](${url})\n` : '') + color;
    }
    const memory = new Map();
    const pending = new Map();
    const controllers = new Set();
    let disposed = false;

    async function get(path) {
        if (disposed) throw new Error('POKE está desactivado.');
        if (!/^[a-z-]+\/(?:[a-z0-9-]+\/)?(?:\?limit=\d+&offset=\d+)?$/.test(path)) throw new Error('Consulta inválida.');
        if (memory.has(path)) return memory.get(path);
        try {
            const cached = JSON.parse(localStorage.getItem('poke:v1:' + path));
            if (cached && Date.now() - cached.time < 7 * 86400000) {
                memory.set(path, cached.data);
                return cached.data;
            }
        } catch { /* Storage may be unavailable. */ }
        if (pending.has(path)) return pending.get(path);
        const request = (async () => {
            const controller = new AbortController();
            controllers.add(controller);
            const timer = setTimeout(() => controller.abort(), 10000);
            try {
                const response = await fetch(API + path, { signal: controller.signal });
                if (response.status === 404) {
                    const error = new Error('No encontré ese nombre o forma en PokéAPI. Probá el nombre oficial, número de Pokédex o forma (por ejemplo, charizard-mega-x).');
                    error.status = 404;
                    throw error;
                }
                if (!response.ok) throw new Error('PokéAPI no está disponible en este momento.');
                const data = await response.json();
                memory.set(path, data);
                try { localStorage.setItem('poke:v1:' + path, JSON.stringify({ time: Date.now(), data })); } catch { /* Cache is optional. */ }
                return data;
            } catch (error) {
                if (error.name === 'AbortError' || error instanceof TypeError) throw new Error('No pude conectar con PokéAPI. Revisá tu conexión e intentá nuevamente.');
                throw error;
            } finally { clearTimeout(timer); controllers.delete(controller); }
        })();
        pending.set(path, request);
        try { return await request; } finally { pending.delete(path); }
    }

    async function entity(value) {
        const key = normalize(value).replace(/^tipo\s+/, '');
        if (!key || key.length > 80) throw new Error('Indicá un tipo o un Pokémon.');
        if (aliases[key]) return { name: label(aliases[key]), types: [aliases[key]] };
        let pokemon;
        try { pokemon = await get(`pokemon/${slug(key)}/`); }
        catch (error) {
            if (error.status !== 404) throw error;
            const species = await get(`pokemon-species/${slug(key)}/`);
            const defaultForm = species.varieties.find(v => v.is_default)?.pokemon.name;
            if (!defaultForm) throw error;
            pokemon = await get(`pokemon/${defaultForm}/`);
        }
        return { name: pokemon.name, types: pokemon.types.map(t => t.type.name), pokemon };
    }

    async function defenses(target) {
        const relations = await Promise.all(target.types.map(type => get(`type/${type}/`)));
        return Object.fromEntries(Object.keys(names).map(attack => {
            const multiplier = relations.reduce((total, type) => {
                const r = type.damage_relations;
                const has = key => r[key].some(t => t.name === attack);
                return total * (has('no_damage_from') ? 0 : has('double_damage_from') ? 2 : has('half_damage_from') ? 0.5 : 1);
            }, 1);
            return [attack, multiplier];
        }));
    }

    function defenseText(values) {
        const group = predicate => Object.entries(values).filter(([, n]) => predicate(n)).map(([type, n]) => `${label(type)} ×${n}`).join(', ') || 'ninguna';
        return `Debilidades: ${group(n => n > 1)}.\nResistencias: ${group(n => n > 0 && n < 1)}.\nInmunidades: ${group(n => n === 0)}.`;
    }
    const caveat = 'Reglas de tipos de los juegos principales modernos (desde generación VI). Ventaja por tipos, no victoria garantizada: influyen nivel, movimientos, habilidades, objetos, estadísticas y reglas del juego; no es una simulación de combate ni de Pokémon GO/TCG.';

    async function compare(a, b) {
        const [left, right] = await Promise.all([entity(a), entity(b)]);
        const [ld, rd] = await Promise.all([defenses(left), defenses(right)]);
        const l = Math.max(...left.types.map(t => rd[t]));
        const r = Math.max(...right.types.map(t => ld[t]));
        const attacks = (source, target, defense) => `${source.name} → ${target.name}: ${source.types.map(t => `${label(t)} ×${defense[t]}`).join(', ')}.`;
        const stats = p => p.pokemon ? `\n${p.name}: ${p.pokemon.stats.map(s => `${s.stat.name} ${s.base_stat}`).join(', ')} (estadísticas base).` : '';
        return `${left.name} (${left.types.map(label).join('/')}) vs ${right.name} (${right.types.map(label).join('/')})\n${attacks(left, right, rd)}\n${attacks(right, left, ld)}\n${l === r ? 'No hay un favorito claro solamente por efectividad de tipos.' : `${l > r ? left.name : right.name} tiene ventaja teórica por efectividad de sus tipos de ataque.`}${stats(left)}${stats(right)}\nSe consideran ataques de los tipos propios; los multiplicadores no incluyen STAB ni habilidades.\n${caveat}${await artwork(left)}${await artwork(right)}`;
    }

    async function weaknesses(value) {
        const target = await entity(value);
        const values = await defenses(target);
        const counters = Object.entries(values).filter(([, n]) => n > 1).sort((a, b) => b[1] - a[1]);
        const examples = await Promise.all(counters.map(async ([type]) => {
            const data = await get(`type/${type}/`);
            return `${label(type)}: ${data.pokemon.slice(0, 3).map(p => p.pokemon.name).join(', ')}`;
        }));
        return `${target.name} (${target.types.map(label).join('/')})\n${defenseText(values)}\n${examples.length ? `Tipos de ataque recomendados y ejemplos de Pokémon de esos tipos: ${examples.join('; ')}. Deben llevar un movimiento del tipo indicado; no son counters garantizados.` : 'No tiene debilidades por combinación de tipos.'}\n${caveat}${await artwork(target)}`;
    }

    const localized = entries => (entries || []).find(e => e.language.name === 'es') || (entries || []).find(e => e.language.name === 'en');
    async function info(value) {
        const target = await entity(value);
        if (!target.pokemon) return weaknesses(value);
        const p = target.pokemon;
        const [species, defense] = await Promise.all([get(`pokemon-species/${p.species.name}/`), defenses(target)]);
        const flavor = localized(species.flavor_text_entries);
        return `${p.name} — Pokédex nacional #${species.id}\nTipos: ${target.types.map(label).join('/')}. Altura: ${p.height / 10} m. Peso: ${p.weight / 10} kg.\n${flavor ? flavor.flavor_text.replace(/\s+/g, ' ') + ` (${flavor.version.name}).\n` : ''}Estadísticas base: ${p.stats.map(s => `${s.stat.name}: ${s.base_stat}`).join(', ')}.\nHabilidades: ${p.abilities.map(a => a.ability.name + (a.is_hidden ? ' (oculta)' : '')).join(', ')}.\n${defenseText(defense)}\nGeneración: ${species.generation.name}. Legendario: ${species.is_legendary ? 'sí' : 'no'}. Mítico: ${species.is_mythical ? 'sí' : 'no'}.\nFormas: ${species.varieties.map(v => v.pokemon.name).join(', ')}.\n${caveat}${await artwork(target)}`;
    }

    async function detail(kind, value) {
        if (kind === 'evolution') {
            const target = await entity(value);
            if (!target.pokemon) throw new Error('Para evoluciones indicá un Pokémon, no un tipo.');
            const species = await get(`pokemon-species/${target.pokemon.species.name}/`);
            if (!species.evolution_chain) return 'No hay datos de evolución disponibles.';
            const id = species.evolution_chain.url.match(/\/evolution-chain\/(\d+)\/$/)?.[1];
            if (!id) throw new Error('Cadena de evolución inválida.');
            const chain = await get(`evolution-chain/${id}/`);
            const lines = [];
            function walk(node, parent) {
                if (parent) lines.push(`${parent} → ${node.species.name}: ${node.evolution_details.map(d => Object.entries(d).filter(([, v]) => v !== null && v !== false && v !== '' && v !== 0).map(([k, v]) => `${k}=${typeof v === 'object' ? v.name : v}`).join(', ')).join(' O ') || 'condiciones no especificadas'}`);
                node.evolves_to.forEach(child => walk(child, node.species.name));
            }
            walk(chain.chain, null);
            return lines.join('\n') || `${target.name} no tiene evoluciones registradas.`;
        }
        if (!['move', 'ability', 'item'].includes(kind)) throw new Error('Categoría no válida.');
        let data;
        try { data = await get(`${kind}/${slug(value)}/`); }
        catch (error) { throw new Error(`${error.message} Para movimientos, habilidades y objetos usá el identificador en inglés (ej.: thunderbolt, levitate, leftovers).`); }
        const effect = localized(data.effect_entries);
        const flavor = localized(data.flavor_text_entries);
        const description = (effect?.language.name === 'es' ? effect.effect : flavor?.language.name === 'es' ? flavor.flavor_text : effect?.effect || flavor?.flavor_text)?.replace(/\s+/g, ' ').replace(/\$effect_chance/g, String(data.effect_chance ?? '?')) || 'Sin descripción disponible.';
        return `${localized(data.names)?.name || data.name} (${data.name})\n${description}${kind === 'move' ? `\nTipo: ${label(data.type.name)}. Clase: ${{physical:'físico',special:'especial',status:'estado'}[data.damage_class.name] || data.damage_class.name}. Potencia: ${data.power ?? 'variable/sin daño directo'}. Precisión: ${data.accuracy ?? 'no aplica'}. PP: ${data.pp}. Prioridad: ${data.priority}. Probabilidad de efecto: ${data.effect_chance == null ? 'no aplica' : data.effect_chance + '%'}.` : ''}`;
    }

    const safe = fn => async (...args) => {
        try { return { message: await fn(...args) + '\nFuente: PokéAPI (https://pokeapi.co/).' }; }
        catch (error) { return { message: `POKE: ${error.message}` }; }
    };
    function register(id, patterns, run) {
        Neeko.commands.register(id, { patterns: { es: patterns, en: patterns }, aiHandler: safe(run), handler: safe((matches, message) => run({ query: message, first: matches[1], second: matches[2], name: matches[1] })) });
    }
    register('poke-weaknesses', [
        '^\\s*(?:¿)?(?:poke\\s+)?(.+?)\\s+(?:vs\\.?|versus|contra)\\s*\\?[?!]*\\s*$',
        '^(?:¿)?(?:qu[eé]\\s+(?:le\\s+)?gana\\s+a|qu[eé]\\s+de[bhv]ilidades?\\s+tiene|de[bhv]ilidades?\\s+(?:de|del))\\s+(.+?)[?!]*$',
        '^\\s*(?:¿)?(?:poke\\s+)?counters?\\s+(?:(?:de|del|para)\\s+)?(.+?)[?!]*\\s*$',
        '^\\s*(?:¿)?(?:poke\\s+)?(.+?)\\s+counters?[?!]*\\s*$',
    ], p => weaknesses(p.name));
    register('poke-compare', ['^\\s*(?:poke\\s+)?(?<first>[\\wáéíóúñü♀♂ .-]+?)\\s+(?:vs\\.?|versus|contra)\\s+(?<second>[\\wáéíóúñü♀♂ .-]+?)[?¿!]*\\s*$'], p => compare(p.first, p.second));
    register('poke-info', ['^(?:poke|pok[eé]dex|pokemon|pok[eé]mon|info\\s+pokemon)\\s+(.+?)[?!]*$'], p => info(p.name));
    register('poke-evolution', ['^(?:evoluciones?\\s+de|c[oó]mo\\s+evoluciona)\\s+(.+?)[?!]*$'], p => detail('evolution', p.name));
    register('poke-move', ['^movimiento\\s+(.+?)[?!]*$'], p => detail('move', p.name));
    register('poke-ability', ['^habilidad\\s+pokemon\\s+(.+?)[?!]*$'], p => detail('ability', p.name));
    register('poke-item', ['^objeto\\s+pokemon\\s+(.+?)[?!]*$'], p => detail('item', p.name));
    register('poke-moves', ['^movimientos\\s+de\\s+(.+?)[?!]*$'], async p => {
        const target = await entity(p.name);
        if (!target.pokemon) throw new Error('Indicá un Pokémon para consultar movimientos.');
        const moves = target.pokemon.moves.map(m => ({ name: m.move.name, versions: m.version_group_details.filter(v => !p.version || v.version_group.name === slug(p.version)).map(v => `${v.version_group.name}: ${v.move_learn_method.name}${v.level_learned_at ? ' nivel ' + v.level_learned_at : ''}`) })).filter(m => m.versions.length);
        if (!moves.length) return 'No hay movimientos registrados para esa versión.';
        return `${target.name}: ${moves.map(m => p.version ? `${m.name} (${m.versions.join('; ')})` : m.name).join('\n')}\nLista de movimientos registrados${p.version ? ' para ' + p.version : ' entre distintas versiones, no necesariamente disponibles juntos en un juego'}. Para ver métodos y niveles de aprendizaje indicá una versión, como scarlet-violet.`;
    });

    Neeko.ui.registerSettingsTab('poke', 'POKE', `<div class="poke-panel"><h4>POKE · Pokédex y combates</h4><p>Consultá Pokémon y formas de PokéAPI por nombre o número. La IA puede consultar fichas, tipos, evoluciones, movimientos, habilidades y objetos.</p><p>Probá en el chat:</p><ul><li>Fuego vs agua</li><li>¿Qué debilidades tiene Charizard?</li><li>¿Qué le gana a fuego?</li><li>Pikachu vs Squirtle</li><li>Pokédex 25</li><li>Cómo evoluciona Eevee</li></ul><p>Ventajas teóricas de los juegos principales modernos. Los tipos dobles multiplican las debilidades y resistencias. No predice una victoria segura.</p><p>Requiere internet para datos nuevos. Las consultas se guardan en caché durante 7 días. Sin clave API. Movimientos, habilidades y objetos: identificador inglés.</p><p>Fuente: https://pokeapi.co/</p></div>`);
    Neeko.addon.onUnload(() => { disposed = true; controllers.forEach(c => c.abort()); memory.clear(); pending.clear(); });
})();
