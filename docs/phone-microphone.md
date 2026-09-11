# Phone Microphone: integración y auditoría

## Contrato de addons encontrado en Neeko

La fuente de verdad es `src-tauri/src/addon_manager.rs` y `NeekoAddons` en
`src/main.js`; se usaron Shazam, Discord Rich Presence y `_template` como referencias.

| Área | Implementación existente y uso en este addon |
| --- | --- |
| Descubrimiento | Carpetas con `addon.json`; prioridad: configuración del usuario, recursos empaquetados, directorios de desarrollo. Duplicados por ID conservan el primero. |
| Registro/carga | `addon_list`, `addon_get_js/css`; inyección de `main.js` y CSS por addon habilitado. No hay carga dinámica de bibliotecas Rust. |
| Estado habilitado | `%APPDATA%/neeko-assistant/addons.json`; el manifest nuevo usa `enabled:false`. |
| Frontend/backend | `Neeko.invoke` usa comandos registrados en el Tauri existente. Como Shazam, el backend Rust se compila dentro de Neeko. |
| Navegación/UI | `Neeko.ui.registerSettingsTab`, contenido DOM y estilos propios con el patrón visual de Shazam. No se añadió Vue, otra ventana ni runtime. |
| Lifecycle | `Neeko.addon.onUnload` limpia listeners; el cargador elimina pestaña, CSS, script, comandos y hooks. El servicio pertenece al backend, por lo que cerrar Configuración no corta el micrófono. |
| Disable/salida | `addon_disable` detiene este servicio en Rust; `RunEvent::Exit` hace la misma limpieza. Ocultar Neeko en la bandeja no es salir. |
| Eventos | `Neeko.events.on`/Tauri; evento nuevo `phone-microphone:status` con estado y medidor. No transporta audio por la webview. |
| Configuración | No hay una API de storage por addon; `_template` recomienda localStorage y Shazam persiste en Rust. Se sigue este último patrón en `phone-microphone.json`, dentro de la configuración de Neeko, compartida por sus ventanas. |
| Logs/notificaciones | El proyecto usa `console` y `eprintln!`; el addon conserva esos canales, mensajes de estado y errores visibles en su panel. |
| Procesos | No existe un supervisor genérico de addons. El núcleo de audio se ejecuta en threads/tareas del proceso de Neeko. ADB es el único ejecutable externo, con argumentos locales fijos, ventana oculta y timeout. |
| Permisos | El manifest declara `ui:tabs`, como Shazam. Los permisos del manifest no constituyen un sandbox de JavaScript. El comando del addon comprueba ventana local `main/settings` y addon habilitado. |
| Distribución | `tauri.conf.json.bundle.resources` incluye la carpeta del addon. El núcleo se enlaza en el ejecutable. Copiar solamente la carpeta a una versión anterior de Neeko no agrega comandos Rust. |

No se alteró el contrato de addons. Los cambios de core se limitan al registro
del módulo/comando, su parada al deshabilitar/salir, dependencias y recursos.

## MicYou revisado y reutilizado

Repositorio: https://github.com/LanRhyme/MicYou

Revisión fijada: `0bf286b0d06552a5fd406c0b1695a1bd52e9916b` (workspace 2.0.3).

Se leyó `LICENSE` antes de extraer código. Se conservaron los headers y la licencia
completa junto al núcleo y dentro del addon distribuido. MicYou declara GPL v3 o
posterior con MicYou Plugin Exception. Esta adaptación **extrae componentes internos**:
no utiliza las interfaces de plugins de MicYou a las que se refiere esa excepción.
La distribución del programa combinado debe satisfacer GPL, incluyendo el código
fuente correspondiente y los avisos; los avisos por sí solos no sustituyen esas
obligaciones. No se cambió silenciosamente la licencia del resto del repositorio.

| Código original | Uso |
| --- | --- |
| `crates/micyou-protocol` | Schema Protobuf, constantes, framing y compatibilidad de campos. |
| `tcp_server.rs`, `udp_server.rs`, `audio_stream.rs` | Handshake, control, audio TCP/UDP, sesiones por IP/ID/epoch, límites de mensajes, validación, cancelación. |
| `network.rs`, `server.rs` | Anuncio `_micyou._tcp.local.`, interfaces locales, lifecycle y cierre con timeout. |
| `jitter_buffer.rs`, `opus.rs`, `stats.rs` | Reordenamiento/FEC, decoder Opus y métricas. Se mantiene el parche Rusopus de upstream, fijado a `025a8e4e77fa544b66c4ad90cf49efe684a45faf`. |
| `commands/system.rs` | Se extrajeron las funciones internas start/stop y el pipeline; se omitieron comandos de UI, bandeja, efectos de ventana, web y host de plugins. |
| `audio_output.rs`, `crates/micyou-audio` | CPAL/WASAPI, ring buffer, resampling Rubato, RNNoise, ganancia, AGC y AEC7 con loopback de Windows. |
| `adb_manager.rs` | Descubrimiento de ADB y teléfonos y `adb reverse`. Se añadieron timeout, ventana oculta, `--no-rebind` y retirada del mapping propio. |
| `resources/aec7_ep0185.onnx` | Modelo AEC original con su licencia MIT. ONNX Runtime oficial CPU con licencia y avisos de terceros. |

El código Android revisado está en `composeApp/src/main/kotlin/com/lanrhyme/micyou/`,
principalmente `audio/AudioEngine.kt`, `network` y `viewmodel/AudioStreamViewModel.kt`.
El teléfono captura con AudioRecord y genera PCM u Opus. El PC no solicita captura
remota: el usuario inicia el stream en Android. Su discovery busca el servidor,
no anuncia un teléfono inactivo para que la PC lo descubra.

No se modifica ni se bifurca el cliente Android. Se conserva el handshake
`MicYouCheck1`/`MicYouCheck2`, el framing con magic/longitud big-endian, el schema
y los puertos por defecto TCP 9123 y UDP 9124. USB hace reverse de TCP 9123 hacia
loopback de la PC; requiere modo USB/TCP en Android.

## Audio y controles

Android → Protobuf TCP/UDP → validación de sesión → jitter/FEC → PCM/Opus →
float → resampling a 48 kHz sólo si hace falta → DSP de MicYou → aprobación/mute
local → buffer/salida CPAL `CABLE Input` → driver VB-CABLE → entrada Windows
`CABLE Output` → Discord, OBS, juegos o navegador.

La frecuencia de captura se elige **en Android**, porque el protocolo no tiene un
comando para configurarla desde la PC. El panel muestra la frecuencia recibida;
recomendado: 48 kHz mono. El pipeline interno conserva 48 kHz y adapta la salida
sólo si la configuración real del dispositivo lo exige. Las métricas de latencia
de red no se presentan como una medición de latencia acústica de extremo a extremo.

Se conservan los buffers de MicYou (incluyendo el headroom de salida de 300 ms,
que es capacidad, no una promesa de latencia constante). No se persiste ni se envía
audio a servidores externos. RNNoise funciona sin descarga en runtime. AEC usa
audio de reproducción de Windows como referencia y reporta fallos de captura/modelo.

Cambios deliberados sobre upstream:

- Bind explícito a una IPv4 privada/local, nunca `0.0.0.0`. USB usa loopback.
- Se rechazan peers públicos y no se permite que otro teléfono suplante una sesión activa.
- Un teléfono detectado requiere **Conectar audio** en Neeko. Mute se sincroniza
  con Android; la aprobación y la compuerta PTT permanecen bajo control local.
- No hay fallback al altavoz cuando falta la salida virtual seleccionada.
- Se ignoran los mensajes opcionales de plugins de MicYou; no se carga su host de plugins.
- Se rechazan codecs desconocidos y PCM float no finito antes de DSP.
- Se actualiza la frecuencia reportada también en TCP/USB.
- Se prefiere el endpoint estéreo `CABLE Input` frente a `CABLE In 16ch` al
  seleccionar automáticamente; ambos fueron comprobados en esta PC.
- La UI no sobrescribe controles en edición con eventos del medidor; los botones
  reflejan inmediatamente una operación pendiente y se liberan al terminarla.
- Los guards del lifecycle se liberan antes de reentrar al mutex en start/stop
  y rollback. Retenerlos bloqueaba la respuesta del servidor y los botones.
- El thread de salida se cierra y se espera al detener/deshabilitar; no permanece
  reteniendo el dispositivo mientras el addon está detenido.
- ONNX Runtime se carga con telemetría deshabilitada y API 24, compatible con la
  DLL CPU 1.24.3 incluida; no se usan los defaults de `ort` que exigirían API 27.

## Reconexión y límites reales

El servidor continúa escuchando tras una pérdida de conexión. Si el cliente vuelve
desde la misma IP durante la misma ejecución del servidor y está habilitada la
opción, Neeko reaprueba el audio. Desconectar explícitamente olvida esa aprobación.
La identidad basada en IP es conveniencia para una LAN de confianza, no autenticación
criptográfica. El protocolo oficial no cifra ni autentica dispositivos con claves.

El cliente Android inspeccionado expone reintento manual (`retryAfterError`); el
addon no puede obligar a una app Android detenida a volver a capturar. Si Android
queda en error, hay que pulsar reintentar allí. No se afirma una reconexión autónoma
del cliente que el código oficial no implementa.

El protocolo no transmite el modelo/nombre real del teléfono: upstream identifica
la sesión como `MicYou Mobile` más IP. En USB, la lista de ADB sí muestra el modelo.
No se instala un driver propio llamado Phone Microphone: Windows ve el nombre
de VB-CABLE. VB-CABLE y ADB son dependencias externas; Neeko no instala drivers ni
modifica el firewall automáticamente. El daemon compartido de ADB no se mata,
para no interrumpir Android Studio u otras aplicaciones; se retira sólo el reverse propio.

## Validación reproducible

### Controles de entrada y atajos

El volumen muestra porcentaje de amplitud y dB (100 % = 0 dB). La sensibilidad
es un control distinto: activa el VAD existente de MicYou y configura su umbral
de -100 a 0 dB. La barra muestra la entrada antes del procesamiento; el umbral
se aplica dentro de la cadena DSP, después de ganancia y reducción de ruido.
No se implementó un ajuste automático del umbral.

En modo **Pulsar para hablar**, la tecla predeterminada es **Space**. El atajo
de mute predeterminado es **Ctrl+Shift+M**. Ambos se pueden cambiar en el panel,
incluyendo letras como X, números, F1–F24 y combinaciones con modificadores.
El atajo de mute también puede desactivarse. Se guardan en la configuración del addon.
Los atajos funcionan mientras el servidor esté iniciado, aunque se cierre el panel.
El addon consulta únicamente las teclas configuradas mediante GetAsyncKeyState
cada 16 ms, sin registrar texto ni consumir las teclas de otras aplicaciones.
No se garantizan atajos en escritorios seguros o aplicaciones con privilegios
que Windows impida consultar. Al cambiar el atajo hay que soltarlo antes de usarlo.
Detener/deshabilitar el addon cancela y espera la tarea de teclas.

El mute explícito se comparte con Android mediante el `MuteMessage` original.
`AudioEngine.kt` del cliente oficial actualiza su StateFlow al recibirlo; no hace
falta otra APK. El estado inicial comunicado por Android al conectarse se refleja
en Neeko. No hay confirmación dedicada del protocolo: un fallo al encolar el
comando se informa, manteniendo el silencio local si se solicitó mute.
PTT es una compuerta local independiente y nunca desactiva un mute explícito.
No cambia el botón de mute de Android cada vez que se pulsa/suelta la tecla.

Se verificaron mensajes de mute en ambas direcciones por TCP real y la UI con
estados Android simulados. En VB-CABLE real, PTT cerrado produjo RMS 0,
PTT abierto RMS 0.057305 y mute con PTT abierto RMS 0. No se probó una llamada
real con Android ni pulsaciones físicas durante un juego.

```powershell
npm test
node --check addons/phone-microphone/main.js
cargo test --manifest-path src-tauri/Cargo.toml --lib phone_microphone
cargo test --manifest-path src-tauri/Cargo.toml -p neeko-micyou-core
cargo test --manifest-path src-tauri/Cargo.toml -p micyou-audio
cargo test --manifest-path src-tauri/Cargo.toml -p neeko-micyou-core --test virtual_microphone -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p micyou-audio --features noise-suppression --test aec_runtime -- --ignored --nocapture
./node_modules/.bin/tauri.cmd build --no-bundle
```

Los tests del núcleo incluyen validación de datagramas, sesiones, jitter/FEC y
lifecycle heredados de MicYou, más una prueba de handshake/PCM fragmentado,
reconexión y liberación de un puerto real. No sustituyen una prueba física de
Android → VB-CABLE → Discord, ni una medición de latencia o cortes bajo carga.

`tests/phone-microphone-ui.html` ejecuta la UI real del addon con un backend de
prueba en navegador: verifica que Conectar invoca el comando, el estado ocupado,
que los eventos de audio no pisen la ganancia, mute, desconexión y unload.

La prueba de hardware se ejecutó explícitamente en esta PC con VB-CABLE: recibió
señal en `CABLE Output` desde TCP (RMS 0.052814) y UDP (RMS 0.056740), y silencio
antes de aprobar y con mute (RMS 0). También verificó la liberación de los puertos.
No graba el micrófono físico: genera un tono y captura solamente el cable virtual.

También pasó la prueba explícita de carga e inferencia de AEC7 con el runtime
incluido, usando muestras sintéticas. Suites ordinarias: 68 tests del núcleo,
22 de DSP/audio y 3 de integración del transporte; backend Neeko: 22 aprobados,
2 pruebas externas omitidas; JavaScript existente: 14 aprobados.
