# Vortexine's OS 1.5

Miku graba tus partidas y te arma sus mejores momentos para Vortex Studio, el shell ya no enseña textos
sin traducir, y el repo tiene pruebas automáticas en cada cambio.

## Lo nuevo

- **Miku graba tus partidas:**
  - Dile «Miku, grábame» y graba el monitor que estás usando a 60 fps, con el sonido del sistema (no el
    micrófono), en `~/Vídeos/Miku`.
  - Sigue grabando aunque la cierres (Super+O) hasta que le digas que pare, o con
    `vortex-miku grabar parar`. `vortex-miku grabar` y `vortex-miku grabar estado` también sirven.
  - Solo graba cuando se lo pides, y siempre con tu permiso en su ventana (**Autorizar** / **Cancelar**).
- **Mejores momentos para Vortex Studio:**
  - Si se lo pides («arma los mejores momentos»), busca los picos de sonido y de movimiento de la
    grabación, y Claude mira unas miniaturas para elegir los que valen la pena y ponerles título.
  - Claude no tiene ninguna otra herramienta en ese paso: solo ve las miniaturas y contesta cuáles.
  - Te deja `<video> - mejores momentos.vortex` en `~/Vídeos/Miku`: los cortes del video original (sin
    recodificar), con su audio enlazado y un marcador con el título de cada momento. Ábrelo en Vortex
    Studio para pulirlo y exportarlo.
  - Corre aparte y te avisa con una notificación (con el botón «Abrir en Vortex Studio»). Sin Claude se
    queda con los picos más altos. También: `vortex-miku momentos [VIDEO]`.
- **Miku gasta menos tokens al correr comandos:** si tienes [RTK](https://github.com/rtk-ai/rtk)
  instalado, la salida de los comandos que corre pasa por él. Sin RTK todo sigue igual.
- **Miku inicia sesión en Claude sin dejar una consola abierta:** «Activar Claude» e «Iniciar sesión»
  abren una ventanita que se cierra sola (`vortex claude login`).
- **El shell en español, completo:** 32 textos que faltaban (estilos de la barra y de los espacios de
  trabajo, el menú del dock, el lanzador, la pantalla de bloqueo, widgets…) ya están traducidos, y si
  algún día falta uno, sale el texto en inglés en vez de la clave interna (`guide.bar.height.title`).
- **Instalar sobre Arch o CachyOS** ahora es con `git clone` + `./install.sh` (se quitó `boot.sh`, el
  `curl | bash`), o con la ISO.

## Por dentro

- Pruebas automáticas en GitHub en cada cambio: shellcheck en todos los scripts, las claves de idioma
  del shell (español e inglés), `Hyprland --verify-config` con la config de Vortex, y las acciones y los
  mejores momentos de Miku.
- shellcheck limpio en los 52 scripts.

## Actualizar

```bash
vortex actualizar
```

## Requisitos

Los mismos de la 1.0: PC x86-64 con 4 GB de RAM (8 GB recomendado), 40 GB de disco y una USB de 8 GB.
