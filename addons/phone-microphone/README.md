# Phone Microphone

Addon nativo de Neeko que usa el backend adaptado de [MicYou](https://github.com/LanRhyme/MicYou).
No requiere abrir MicYou desktop.

1. Instalá [VB-CABLE](https://vb-audio.com/Cable/) en Windows y reiniciá si su instalador lo pide.
2. Abrí Neeko → Configuración → Addons → **Phone Microphone → Habilitar**.
3. En su pestaña seleccioná **CABLE Input** e **Iniciar servidor**.
4. Abrí el [cliente Android de MicYou](https://github.com/LanRhyme/MicYou/releases).
   En Wi-Fi, misma red: buscá la PC o ingresá la IP/puerto mostrados en Neeko.
   Configurá 48 kHz mono y comenzá el stream en el teléfono.
5. Cuando aparezca el teléfono, pulsá **Conectar audio** en Neeko.
6. En Discord/OBS/juego elegí **CABLE Output** como dispositivo de entrada.

Para USB, instalá Android SDK Platform Tools (ADB en PATH o la ubicación estándar
del SDK), activá depuración USB y autorizá esta PC en el teléfono. Elegí USB,
**Detectar USB**, seleccioná el dispositivo e iniciá el servidor. En MicYou elegí
USB/TCP. ADB usa un reverse propio que Neeko retira al detener el servicio.

Ganancia, RNNoise, AGC y mute se pueden cambiar en vivo. Configuración persistente:
`%APPDATA%/neeko-assistant/phone-microphone.json`. Cambios de red/salida requieren detener.
Cerrar Configuración o esconder Neeko conserva el audio; Detener, Deshabilitar o
Salir de Neeko libera el stream y sockets.

El volumen muestra porcentaje y dB. **Usar umbral de sensibilidad** permite
ajustar qué nivel abre el audio con el VAD de MicYou. Para hablar con una tecla,
elegí **Pulsar para hablar** y configurá **Tecla para hablar** (Space por defecto).
El atajo de mute es Ctrl+Shift+M por defecto; podés cambiarlo o desactivarlo.
Funcionan en segundo plano con el servidor iniciado. Mute se sincroniza con
el cliente Android oficial y tiene prioridad sobre PTT. PTT cierra la salida
local al soltar la tecla; no cambia el botón de mute de Android.

AEC usa el modelo AEC7 incluido y ONNX Runtime CPU x64. Si tu checkout no contiene
el runtime, ejecutá `./addons/phone-microphone/prepare-aec.ps1` antes de compilar.
Descarga el paquete oficial de Microsoft 1.24.3 y verifica su SHA-256; no instala
nada en Windows. El checkbox indica si faltan recursos. AEC requiere una salida
de reproducción de Windows disponible para su referencia.

Limitaciones: Windows x64 y VB-CABLE; frecuencia de captura configurada en Android;
nombre de sesión `MicYou Mobile` porque el protocolo no envía el modelo. La opción
de reconexión acepta al teléfono aprobado si éste vuelve desde la misma IP durante
la sesión; si Android queda en error, pulsá Reintentar allí. Usar en una LAN de
confianza: el protocolo oficial no proporciona cifrado ni identidad criptográfica.

El backend está compilado en Neeko: este addon requiere la versión que incluye
`phone_microphone`, no alcanza con copiar estos archivos a un Neeko anterior.

Código derivado de MicYou © 2026 LanRhyme, GPL-3.0-or-later con MicYou Plugin
Exception; ver `LICENSE-MicYou.txt`. La excepción de plugins de MicYou no convierte
esta extracción del núcleo en código propietario. Modelo AEC7 y ONNX Runtime:
licencias/avisos en `resources/`.

Detalle técnico y archivos reutilizados: [auditoría de integración](../../docs/phone-microphone.md).
