# Vortexine's OS 1.2

Más redondo y más compacto. Todo lo de la 1.1 (con Vortexine's Shop), más:

## Lo nuevo

- **Bordes más redondeados en todo:**
  - barra, paneles, notificaciones y Ajustes: redondeo 18 (antes 10);
  - ventanas: redondeo 18 con curva suave;
  - instalador y Vortexine's Shop, también.
- **Todo un poco más chico y nítido:**
  - el shell (barra, paneles, lanzador) al 85 % de su tamaño;
  - la letra de las apps al 90 %;
  - la terminal con letra 10.

  Los monitores se quedan en escala 1, sin borroso. El tamaño del shell se cambia en
  `~/.config/serpantinum/settings.json` (`general.uiScale`; 1 = el tamaño de antes).
- **Más espacio entre ventanas:** un 15 % más (5 y 7 px).
- **Capturas directas, como en Caelestia:**
  - **Impr Pant** congela la pantalla, eliges un área y queda en el portapapeles (y en
    Imágenes/Capturas);
  - **Super+Impr Pant**: toda la pantalla;
  - **Alt+Impr Pant**: la ventana activa.
- **Los paneles se cierran al hacer clic fuera** (Ajustes, red, audio, música…). Pasar el ratón
  por encima de otra ventana no los cierra.
- **Arreglos:**
  - los botones de la barra no abrían nada si el shell estaba en una carpeta con espacios;
  - `vortex actualizar` recarga Hyprland al terminar, para que no quede en modo de emergencia;
  - el paquete del shell no se construye si hay un error de QML que lo dejaría sin cargar.
- **Modo otaku:** sus bordes suben a 24, para que se sigan notando más suaves.
- **Para quien llega desde otro sistema:**
  - `install.sh` ya no falla si tienes Java JRE (lo cambia por el JDK) o una app en otra variante
    (`-bin`, `-git`): se queda la tuya;
  - `sudo vortex-limpiar` deja limpio un CachyOS u otro Arch que pasó a Vortexine's: quita KDE
    Plasma y Caelestia, apaga servicios que sobran, pone la pantalla de inicio de Vortexine's (con
    el llavero abriéndose solo) y deja GRUB sin menú. No toca tus archivos ni tus sesiones.
- **Guías:** instalar junto a Windows sin USB ([guia-sin-usb.md](guia-sin-usb.md)) y cómo apagar
  Secure Boot sin problemas con BitLocker.

## Requisitos

Los mismos de la 1.0: PC x86-64 con 4 GB de RAM (8 GB recomendado), 40 GB de disco y una USB de 8 GB.
