# Vortexine's OS 1.7.5

Ventanas como en KDE para quien no se acostumbra al mosaico, y un shell que gasta menos RAM y la suelta.

## Lo nuevo

- **Gestor de ventanas: Mosaico o KDE.** En **Ajustes → Pantalla**, hasta abajo, se elige:
  - **Mosaico** (de fábrica, como qtile): cada ventana toma su lugar sin encimarse.
  - **KDE**: para quien viene de KDE o Windows. Las ventanas se abren **flotantes y encimadas**, con
    **barra de título** y sus botones **minimizar, maximizar y cerrar**. Se arrastran de la barra, se
    agrandan desde las orillas y **se pegan** a las orillas de la pantalla y a las otras ventanas. Doble
    clic en la barra maximiza. El teclado sigue al **clic**, no al mouse.
  - Una ventana minimizada vuelve al picarle en la barra o en el dock.
  - Al cambiarlo se reinicia el shell; no hay que cerrar sesión. Las ventanas abiertas se acomodan solas.
  - También con un comando: `vortex ventanas kde`, `vortex ventanas mosaico` y `vortex ventanas estado`.

- **Menos RAM y que se suelte.** El shell iba acumulando memoria toda la sesión (en una PC llegó a
  1,1 GB en 3 h; recién arrancado ocupa ~520 MB) y no la devolvía al cerrar sus paneles:
  - **Todos los paneles se liberan** 20 s después de cerrarlos (red, volumen, reloj, música,
    notificaciones, sistema); antes solo Ajustes y fondos, y al minuto.
  - **Ajustes ya no carga sus ~17 pestañas** al abrirse (solo para el buscador): cada una se carga al
    verla y todas al buscar. Abrirlo pasó de +163 MB, que no se devolvían, a ~+30 MB.
  - El shell **devuelve al sistema** la memoria que suelta (jemalloc al segundo, `MALLOC_CONF`).
  - El historial de notificaciones guarda las **últimas 100** (antes, sin tope y con sus imágenes).
  - Los íconos de la bandeja y los fondos ya no se quedan en caché; el fondo se decodifica al tamaño
    de la pantalla y no a 4K completo primero.
  - La basura de JavaScript se recoge cada 5 min.
  - El gestor de recursos ya puede pedirle memoria al shell cuando falta (`soltarMemoria`; el shell no
    tenía cómo contestar).
  - **RAM usada como en htop:** el Panel de rendimiento y la barra restaban «disponible», que cuenta
    también el colchón libre que reserva el kernel y parte de la caché de archivos (salía ~1-1,5 GB más
    que en htop, como si no se soltara). Ahora es total − libre − búferes − caché, como `vortex bench`.

## Por dentro

- La barra de título es el plugin **hyprbars** (de hyprland-plugins, de los autores de Hyprland). Un
  plugin solo carga en el Hyprland para el que se compiló, así que `vortex ventanas kde` lo compila la
  primera vez para tu versión (unos segundos, necesita internet), sin root, en
  `~/.local/share/vortex/hyprbars/`. `vortex actualizar` lo vuelve a compilar si Hyprland cambió. Si no se
  puede compilar, las ventanas flotan igual, sin barra.
- `config/hypr/hyprland.lua` lee el modo de `~/.config/vortex/ventanas`.
- `shell/src/quickshell/guide/display/DisplayWindowManagerSection.qml`: el selector de Pantalla (textos
  `guide.display.window_manager` en `es.json` y `en.json`). vortex-shell 2.2.14-11.
- `pruebas/hyprland-config.sh` comprueba la config con los dos modos.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
