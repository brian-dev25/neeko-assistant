// Avatars are installed by the user and shared by all application windows.
export function validateAvatar(bytes, name) {
    if (!/\.glb$/i.test(name)) throw new Error('Elegí un archivo .glb exportado desde Khada.');
    if (!(bytes instanceof ArrayBuffer) || bytes.byteLength < 20 || bytes.byteLength > 100 * 1024 * 1024) {
        throw new Error('El archivo GLB está vacío, incompleto o supera los 100 MB.');
    }
    const view = new DataView(bytes);
    if (view.getUint32(0, true) !== 0x46546c67 || view.getUint32(4, true) !== 2
        || view.getUint32(8, true) !== bytes.byteLength || view.getUint32(16, true) !== 0x4e4f534a) {
        throw new Error('El archivo no es un modelo GLB 2 válido.');
    }
    const jsonLength = view.getUint32(12, true);
    if (jsonLength > bytes.byteLength - 20) throw new Error('El modelo GLB está incompleto.');
    const json = JSON.parse(new TextDecoder().decode(new Uint8Array(bytes, 20, jsonLength)));
    for (const resource of [...(json.buffers || []), ...(json.images || [])]) {
        if (resource.uri && !resource.uri.startsWith('data:')) {
            throw new Error('Exportá un GLB con las texturas incluidas, sin archivos externos.');
        }
    }
    const animations = new Set((json.animations || []).map(animation => animation.name));
    if (!['Neeko_idle3.anm', 'Idlein_Animal', 'Joke_Loop'].every(name => animations.has(name))) {
        throw new Error('Importá el GLB de Neeko con todas sus animaciones desde Khada.');
    }
    return json;
}

function openDatabase() {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open('assistant-avatar', 1);
        request.onupgradeneeded = () => request.result.createObjectStore('avatars');
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}

async function transaction(mode, operation) {
    const db = await openDatabase();
    try {
        return await new Promise((resolve, reject) => {
            const tx = db.transaction('avatars', mode);
            const request = operation(tx.objectStore('avatars'));
            tx.oncomplete = () => resolve(request.result);
            tx.onerror = tx.onabort = () => reject(tx.error || request.error || new Error('No se pudo guardar el modelo.'));
        });
    } finally {
        db.close();
    }
}

export function readAvatar() {
    return transaction('readonly', store => store.get('current'));
}

export function saveAvatar(avatar) {
    validateAvatar(avatar.bytes, avatar.name);
    return transaction('readwrite', store => store.put(avatar, 'current'));
}
