# Chat compartido

El chat de escritorio y `/api/chat` usan `src-tauri/src/assistant.rs`. El navegador
y la ventana Tauri comparten `src/chat-client.mjs`; ninguno ejecuta JSON del modelo.

1. El servidor intenta reconocer comandos locales explícitos primero. Si no hay
   coincidencia y la IA está encendida, solicita JSON restringido al modelo.
2. Valida `kind`, `message` y `action`: conversación, pregunta o acción.
3. Una acción válida crea una propuesta con parámetros normalizados y una caducidad
   de cinco minutos. No se ejecuta todavía.
4. El servidor marca `requires_confirmation` según el origen: los comandos locales
   se ejecutan directamente; las acciones interpretadas por IA muestran Sí / No.
   El modelo no puede elegir ni modificar esta marca.
5. La decisión envía solamente sesión, ID de propuesta y aprobación. Rust consume
   la propuesta una sola vez, vuelve a comprobar disponibilidad y la ejecuta.
6. Cancelar o enviar otro mensaje invalida las propuestas anteriores de esa sesión.

La cancelación impide ejecutar propuestas pendientes. No revierte una acción que
ya fue aprobada y comenzó a ejecutarse. Los resultados de addons tienen un límite
de espera de 60 segundos; un timeout no implica que una operación ya iniciada se
haya deshecho, y nunca se reintenta automáticamente.

## Comandos sin modelo y búsqueda por sitio

`src-tauri/src/local_commands.rs` reconoce comandos locales con el motor encendido
o apagado: `ip`, `abrí Discord`, `comprimir`, `git status`, comandos de sistema y
búsquedas, entre otros. Los addons habilitados pueden usar sus patrones declarados
si sus parámetros son compatibles (cero campos o un campo capturado). Nunca se
ejecuta un handler para reconocer un mensaje. También se acepta JSON explícito del
catálogo, siempre sujeto a la misma validación. Estos comandos escritos por el
usuario no requieren confirmación adicional.

Si un mensaje no es un comando local y el modelo está apagado, se indica cómo
activar la IA en vez de mostrar un error de conexión. Desactivar los comandos de
sistema sigue bloqueándolos aunque el reconocimiento local encuentre una coincidencia.

`search` tiene los parámetros `query` y `site` (opcional; Google por defecto).
Una orden explícita como `busca en yt Rust & C++` se interpreta localmente con el
motor encendido o apagado: `site=youtube`, `query=Rust & C++`. El sitio se muestra
en los parámetros validados. Los alias incluyen `yt`, `gh`, `wiki`, `ml` y `ddg`.
Los sitios conocidos tienen su URL de búsqueda propia; un dominio como
`example.com` usa una búsqueda restringida con `site:example.com`. Un sitio no
reconocido se rechaza, sin convertirlo silenciosamente en una búsqueda general.

## Catálogo y contexto

Las consultas de rango y partidas de LoL usan el Riot ID y la región guardados.
Los valores propuestos por el modelo solo reemplazan esa configuración si el
mensaje del usuario contiene explícitamente el Riot ID completo o indica una
región de juego. El idioma de la app no es una región. Sin una cuenta guardada
ni un Riot ID explícito, el chat pregunta cuál usar.

La consulta queda pendiente en esa sesión durante cinco minutos. Una respuesta
como `Jugador#ABC en LAS` completa la cuenta y retoma la misma consulta (incluida
la cantidad de partidas). La confirmación permite marcar «Guardar esta cuenta»,
desactivado por defecto; rechazar o cancelar no guarda nada. Guardar actualiza
solo Riot ID y región mediante la configuración existente. Cambiar de comando
descarta la consulta pendiente y las sesiones no comparten ese estado.

«Info» es un enlace directo a la fuente externa de la respuesta, en escritorio,
chat independiente y celular. No aparece en propuestas ni muestra información
de aprobación, ejecución o memoria. `Reply.info` contiene solamente la URL de la
fuente o una cadena vacía; los clientes rechazan texto explicativo y enlaces no HTTP(S).
Las consultas ejecutadas de LoL enlazan al perfil correspondiente de OP.GG.

El módulo de investigación (`research/`) usa un agente con herramientas para
investigar preguntas del usuario. El flujo es:

1. **Resolver**: el modelo extrae la pregunta concreta, intención y entidades.
2. **Planificar**: el modelo recibe un resumen operativo y elige una operación.
3. **Ejecutar**: el backend valida y ejecuta (búsqueda/lectura/seguir enlace/github/reddit).
4. **Repetir**: pasos 2-3 hasta que el modelo elija `finish` o se agote el presupuesto.
5. **Redactar**: el modelo genera la respuesta final con citas a evidencia.
6. **Verificar**: las citas se comprueban contra fuentes reales; las inválidas se eliminan.

Presupuesto: 8 operaciones totales, 4 búsquedas, 6 documentos leídos, profundidad
de seguimiento 3, deadline 180s. Las fuentes obtienen IDs estables (`s1`, `s2`, ...)
que no se reasignan durante la investigación.

El progreso se报告a las tres interfaces mediante polling cada 800ms:
- Tauri: comando `research_progress`
- Web: `GET /api/research/progress?request_id=...`
Fases: `planning → searching → reading → checking → writing → complete`

Google se renderiza con Edge en modo headless y un perfil temporal independiente.
No lee cookies del navegador personal ni utiliza el modelo de Neeko. Se procesa
un resultado orgánico con su título y fragmento; los enlaces opacos `/goto` se
resuelven leyendo la redirección de Google, sin descargar destinos arbitrarios.
El chat muestra el fragmento recuperado, no una síntesis completa del artículo.
Los resultados web se tratan como datos y no pueden ejecutar comandos del chat.

La consulta tiene límites de tiempo, tamaño y concurrencia. Los procesos del
navegador temporal pertenecen a un Job Object que se cierra al terminar o cancelar.
Si Google no responde, una pregunta enciclopédica puede consultar Wikipedia como
alternativa: el mensaje lo indica y el botón Info apunta a Wikipedia. Para otras
consultas se muestra el error sin inventar una respuesta ni una fuente. No se
intenta resolver captchas. Edge instalado e Internet son necesarios para Google.

Validación: pruebas de enrutamiento, asociación texto/fuente, URLs y entrega de
Info al cliente compartido. Las pruebas de Google real están marcadas `ignored`
para que el test habitual no dependa de la red. Pueden ejecutarse con
`cargo test --lib live_google -- --ignored --nocapture`.

## Respuestas restringidas

Se envía `response_format: {type: "json_object", schema: ...}` al motor y se valida
otra vez su respuesta en Rust. El puente Python reenvía `response_format` y
`temperature` a `llama_cpp`. Si el motor no admite el formato o devuelve datos
inválidos, se muestra un error; no se degrada a ejecución de texto sin validar.

Los límites de longitud de texto se verifican en Rust, después de generar JSON.
No se incluyen `maxLength` grandes en el esquema: llama.cpp los expande en reglas
repetidas y puede rechazar el catálogo con `failed to parse grammar`. Los errores
HTTP muestran el motivo enviado por el motor, además del código de estado.

Referencias: [gramáticas de llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md)
y [API de llama-cpp-python](https://llama-cpp-python.readthedocs.io/en/latest/api-reference/).

## Addons

En cada entrada de `commands` de `addon.json`, declarar:

```json
{
  "id": "save-note",
  "description": "Guardar una nota",
  "ai": {
    "label": "Guardar una nota / Save a note",
    "fields": { "text": { "type": "string" } }
  }
}
```

`label` describe la operación que verá el usuario al confirmar. La aplicación
agrega los valores de todos los parámetros a esa descripción mediante texto plano.
Tipos admitidos: `string`, `url` (HTTP/HTTPS) y `number`. Los campos pueden declarar
`optional: true`; los números requieren `min` y `max`, y admiten `integer: true`.
Los textos pueden limitar valores mediante `values`.

Registrar un handler explícito que reciba parámetros validados:

```js
Neeko.commands.register('save-note', {
    aiHandler: async ({ text }) => saveNote(text),
    // patterns y handler anteriores pueden mantenerse como API de compatibilidad.
});
```

Los comandos se identifican como `addon:<addon-id>:<command-id>`. Solo se ofrecen
si el addon está habilitado, su esquema es válido y su `aiHandler` está cargado.
La ventana de escritorio ejecuta los handlers aprobados, incluso cuando la
propuesta se originó en el celular. Esto conserva un único almacenamiento de notas
y una única conexión a Discord. Nunca se ejecutan handlers para descubrir qué hace
un comando. Los addons antiguos sin `ai`/`aiHandler` deben migrarse usando la plantilla.

## Verificación

```text
node --test scripts/action-policy.test.mjs
cargo test --manifest-path src-tauri/Cargo.toml --lib assistant::tests
cargo check --manifest-path src-tauri/Cargo.toml
```

Para verificar el esquema completo contra un modelo cargado, configurar
`NEEKO_TEST_MODEL_URL` con la dirección de una instancia local de prueba y ejecutar:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib live_model_accepts_the_actual_catalog_schema -- --ignored
```

Esta prueba genera un saludo y una propuesta de compresión; no ejecuta acciones.

Prueba manual con el motor iniciado: conversar, pedir "quiero comprimir", rechazar,
volver a pedir y aprobar, probar una pregunta con datos faltantes, guardar una nota
con el addon habilitado desde ambos clientes y desactivar un addon antes de aprobar.
