// ─── Mi Addon ───
// Descripcion de lo que hace tu addon.

(function() {

    // ─── Registrar un comando ───
    Neeko.commands.register('my-command', {
        patterns: {
            es: ['mi\\s+comando\\s+(.+)'],
            en: ['my\\s+command\\s+(.+)'],
        },
        handler: async (matches, message) => {
            // matches[1] es el texto capturado por el regex
            const text = matches[1];

            // Mostrar mensaje en el chat
            Neeko.ui.showBubble(`Procesando: ${text}`);

            // Hacer algo con el texto
            const result = text.toUpperCase();

            // Devolver respuesta
            return { message: `Resultado: ${result}` };
        },
    });

    // ─── Crear un tab en Configuracion ───
    Neeko.ui.registerSettingsTab('my-addon-settings', 'Mi Addon', `
        <div>
            <p style="color:#aab;font-size:12px;margin-bottom:10px;">Configuracion de mi addon.</p>
            <label style="display:flex;align-items:center;gap:8px;font-size:12px;color:#e0e0ff;">
                <input type="checkbox" id="my-addon-toggle" />
                Activar opcion
            </label>
        </div>
    `);

    // ─── Hook: antes de enviar un mensaje ───
    Neeko.chat.onBeforeMessage(async (message) => {
        // Si el mensaje contiene "urgente", cancelar y responder
        if (message.toLowerCase().includes('urgente')) {
            return { cancel: true, response: 'Mensaje urgente detectado!' };
        }
        // Si no cancelamos, el mensaje sigue su curso normal
        return {};
    });

    // ─── Hook: despues de recibir respuesta ───
    Neeko.chat.onAfterMessage(async (message, response) => {
        // Modificar la respuesta de la IA antes de mostrarla
        return response;
    });

    // ─── Cuando todo esta listo ───
    console.log('Mi Addon cargado!');

})();
