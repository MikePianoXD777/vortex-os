# Vortexine's OS 1.3 (borrador)

El asistente. Todo lo de la 1.2, más **Miku**, un asistente de voz con Claude de cerebro.

## Lo que trae

- **Miku, el asistente de voz** (`vortex-miku`), en la ISO y en `install.sh`:
  - **Super+O** la abre —sale pegada al borde derecho, debajo de la barra, y se pone a escuchar— y
    **Super+O otra vez la cierra del todo**: sin ventana no queda ningún proceso, así que no gasta RAM;
  - por defecto es **Miku Nakano en 3D** (un modelo VRM, con parpadeo, boca que sigue a la voz y
    emociones) al lado de la pantalla; con un **Claude Sonnet** de cerebro que puede ver tu pantalla
    cuando se lo pides, y una voz de IA genérica (Supertonic 3; no imita a ninguna persona);
  - el oído (whisper.cpp) y la voz corren en tu PC; solo lo que ella piensa pasa por Claude;
  - **no está atada al Modo otaku**: son cosas aparte.
- **Ajustes → General → Miku en 3D** (junto a la sección de Claude): un interruptor, encendido de
  fábrica. Apagado queda **solo la voz**, en un panel chico que gasta menos RAM. También con
  `vortex miku avatar on|off`. Si Claude está apagado, la descripción lo dice.
- **Necesita Claude activado** (Ajustes → General → Claude): Claude sigue viniendo apagado de fábrica
  y Miku piensa con él; sin Claude, ella te dice qué falta.
- **La ventana** es flotante y fija en todos los escritorios, sin borde, sombra ni desenfoque y con
  el fondo transparente, a 7 px del borde derecho (los mismos espacios que el resto de ventanas) y
  justo debajo de la barra. No te quita el teclado al abrirse.
- **Comandos:** `vortex miku` (abre o cierra), `vortex miku estado`, `vortex miku voz [ID]` y
  `vortex miku modelo RUTA.vrm`. Las opciones quedan en `~/.config/vortex/miku.json`.
- **Miku hace cosas, con tu permiso:** instalar o quitar apps de Flathub, abrir apps, carpetas o páginas,
  correr un comando (nunca con sudo), cambiar volumen, brillo, Wi-Fi, Bluetooth, modo juego o fondo, y dejar
  listos correos y mensajes (los envías tú: ella no envía nada). Antes de cada acción, su ventana muestra
  exactamente qué va a hacer con los botones **Autorizar** y **Cancelar**; sin tu clic no pasa nada, y si no
  contestas en 2 minutos se cancela. Lo que ve en tu pantalla no puede autorizar nada.
- **Voz natural en tu GPU (opcional):** `vortex-miku natural instalar` le pone a Miku una voz natural y
  expresiva (melancólica, un poco grave) que corre en tu tarjeta NVIDIA, sin internet ni cuentas. Usa
  Qwen3-TTS (Apache-2.0) acelerado con faster-qwen3-tts: en una RTX 3080 habla 2,5 veces más rápido que el
  tiempo real. Necesita una NVIDIA serie 20 o más nueva con 6 GB o más (la GTX 10 no) y ~7 GB de disco.
  Solo usa la GPU (~5 GB) mientras Miku está abierta; al cerrarla se libera. Si no carga, Miku sigue con la
  voz de siempre. La voz se inventó a partir de una descripción: no es la de ninguna persona.
  `vortex-miku natural quitar` la borra.
- **Voz con ElevenLabs (opcional, en la nube):** `vortex-miku elevenlabs VOICE_ID` usa una voz que elijas en
  [elevenlabs.io](https://elevenlabs.io) (Voice Library o Voice Design), con emociones de verdad: Claude pone
  etiquetas como `[sighs]` o `[whispers]` que cambian cómo lo dice (y la cara del 3D), y no salen en pantalla.
  Usa el modelo `eleven_v4_turbo` (o `eleven_v3` si tu cuenta no lo tiene). La clave se pide una vez, sin
  mostrarse, y queda en el llavero. Si no hay internet o se acaban los créditos, Miku sigue con la voz local.
  `vortex-miku elevenlabs quitar` la borra y vuelve a la F1.
- **`vortex actualizar` más listo:** sigue con el código recién bajado (antes usaba el de antes del
  pull) y reconstruye el shell, la tienda o Miku cuando cambia su paquete, o los instala si faltan;
  también pone al día la config de Hyprland (`~/.config/hypr/hyprland.lua` y `atajos.lua`, con
  respaldo; lo tuyo sigue en `hypr-mio.lua`). Ya no hace falta `--forzar`. Desde la 1.2 o antes,
  córrelo **dos veces** (la primera trae el `vortex` nuevo).

## Qué viene en la ISO y qué no

- Viene el paquete `vortex-miku` y lo que usa de los repos (`pyside6`, `qt6-webengine`,
  `whisper-cpp`, `python-onnxruntime-cpu`).
- **Los modelos de voz y de oído no vienen en la ISO**: se bajan la primera vez que se usa, y
  hace falta internet esa vez.
- **El modelo 3D tampoco va en el repo**: es de su autor y su licencia pide crédito. La ISO lo toma
  al construirse de la carpeta `VORTEX_MIKU_MODELO` (con `modelo.vrm` y `CREDITO.txt`) y lo deja en
  `/usr/share/vortex-miku/modelo/`. Sin modelo, Miku funciona como asistente de voz.
- `install.sh` instala el paquete (pregunta; `--sin-miku` lo omite), pero no el modelo 3D.

## Privacidad

El micrófono solo escucha con la ventana de Miku abierta y el audio no se guarda. Con la voz de ElevenLabs,
el texto de lo que Miku dice (no tu voz) se manda a ElevenLabs para convertirlo en audio. La pantalla solo
la ve cuando se lo pides, y cada vez sale una notificación. `vortex claude pausa` le corta el acceso.

## Créditos

- Modelo «MIKU NAKANO» de **Vewertyg** ([VRoid Hub](https://hub.vroid.com/en/characters/7619526108943069134/models/40799402200114173)):
  crédito obligatorio, sin uso comercial. La ventana de Miku muestra el crédito.
- [three.js](https://threejs.org) y [@pixiv/three-vrm](https://github.com/pixiv/three-vrm) (MIT).
- Supertonic 3 (OpenRAIL-M) y [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (MIT).

## Pendiente de este borrador

- Probar la ventana transparente y su posición en una sesión real de Hyprland (con la barra de la 1.2).
- Probar la ISO completa con el paquete y el modelo (la integración se probó sin VM).
- Pasar `VORTEX_VERSION` y las notas a su forma final cuando se cierre la 1.3.

## Requisitos

Los mismos de la 1.0: PC x86-64 con 4 GB de RAM (8 GB recomendado), 40 GB de disco y una USB de 8 GB.
Miku en 3D usa la GPU para dibujar el modelo; apagada («solo voz»), gasta bastante menos.
