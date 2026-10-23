# Vortexine's OS 1.2

Más redondo y más compacto. Todo lo de la 1.1 (con Vortexine's Shop), más:

## Lo nuevo

- **Bordes más redondeados en todo:**
  - barra, paneles, notificaciones y Ajustes: redondeo 18 (antes 10);
  - ventanas: redondeo 18 con curva suave;
  - instalador y Vortexine's Shop, también.
- **Todo un poco más chico:** el shell (barra, paneles, lanzador) al 88 % de su tamaño, para que
  quede más espacio a lo tuyo. Se cambia en `~/.config/serpantinum/settings.json`
  (`general.uiScale`; 1 = el tamaño de antes).
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
