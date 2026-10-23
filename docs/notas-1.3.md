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

El micrófono solo escucha con la ventana de Miku abierta y el audio no se guarda. La pantalla solo
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
