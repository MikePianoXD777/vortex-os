# Vortexine's OS 1.0

La primera versión estable. Basado en Arch con los repos de CachyOS, Hyprland y el shell
Serpantinum. Las funciones de los grandes, fáciles de usar.

## Lo que trae

- **Instalador gráfico** (Calamares) en español, con tema oscuro de Vortexine's:
  - idioma, teclado latinoamericano por defecto, disco, usuario e inicio de sesión automático;
  - apps y juegos con casillas: Steam, Prism Launcher, Sober, el lanzador de Genshin, Heroic,
    Lutris, Firefox, Vortex Studio, Discord, Zoom, OBS, VLC, LibreOffice y Spotify.
- **Secure Boot sin tocar la BIOS.** Arranca con shim, firmado por Microsoft. La primera vez,
  MokManager pide aceptar la llave de Vortexine's, y al instalar, la de tu PC (contraseña `1234`).
- **UEFI y BIOS** (systemd-boot en UEFI, GRUB en BIOS). **Dual boot con Windows**, con la guía
  [docs/guia-dual-boot.md](guia-dual-boot.md) y «Reiniciar en Windows».
- **Hecho para jugar:**
  - kernel de CachyOS con scx_lavd;
  - modo juego automático (Super+G);
  - gamemode, MangoHud y gamescope;
  - Proton de CachyOS con Steam;
  - drivers de la GPU detectados solos con chwd (NVIDIA, AMD, Intel e híbridas).
- **Pantallas a su frecuencia más alta**, hasta 240 Hz, en su resolución nativa.
- **Ligero:** el escritorio en reposo usa 0,5 % de CPU y 584 MB de RAM ocupada (VM de 6 GB).
- **Programación con los lenguajes al día:** Java 27, Node, Rust, Go, Clang, CMake, Python, uv y
  mise, más Zed y Neovim.
- **Claude**, apagado de fábrica. Se activa en Ajustes → General → Claude y necesita tu cuenta de
  Claude. Le hablas con el micrófono y ve tu pantalla solo cuando se lo pides.
- Ventanas al 95 % de opacidad con el fondo desenfocado. Los colores del escritorio salen del fondo.
- Extras: zram, Flatpak con Flathub, AppImage (fuse2), btop, nvtop y fastfetch.

## Requisitos

- PC x86-64 con 4 GB de RAM (8 GB recomendado) y 40 GB de disco.
- USB de 8 GB.
- Internet al instalar, para las apps que elijas.

## Avisos

- Algunos textos del instalador salen en inglés con «Español (México)»: la traducción de
  Calamares para es_MX está incompleta.
- Las apps de Flatpak (Sober, Zoom, el lanzador de Genshin, Heroic, Spotify) se instalan en el
  primer arranque con internet.
