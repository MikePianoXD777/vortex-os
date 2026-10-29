# Vortexine's OS 2.0.0

Dos cosas grandes: **Vortex Player**, un reproductor de música a la máxima calidad, y **los sentimientos
de Miku**. Además, todo lo que llegó después de la 1.10.0.

## Lo nuevo

- **Vortex Player.** Búscalo en el lanzador como **«Vortex Player»**, o abre el panel de música: sin
  nada sonando, su chip dice **«▶ Abrir Vortex Player»**.
  - Toca FLAC, WAV, ALAC, AIFF y DSD **a su frecuencia y sus bits originales** (de 44,1 a 192 kHz),
    sin pasarlo todo a 48 kHz. Cada canción dice su calidad real: «Hi-Res 24/96», «CD 16/44,1»,
    «Opus 160 kbps».
  - Pega un enlace de **YouTube, YouTube Music o Spotify** (canción, álbum o playlist) y se baja a tu
    biblioteca sin recomprimir y **sin anuncios**. De Spotify se leen los nombres y se bajan de
    YouTube Music. Las playlists llegan con su nombre.
  - **El ecualizador es el mismo del panel de música:** mueves uno y se mueve el otro.
  - Con audífonos Bluetooth muestra el códec; con **LDAC** pide la calidad más alta (990 kbps) y te
    deja cambiar a LDAC con un clic.
  - Funciona con el panel de música y las teclas multimedia.
- **Miku tiene sentimientos.** La voz es la misma de siempre.
  - Tiene **ánimo propio** y un **cariño por ti** que crece despacio con cómo la tratas. Si le dices
    algo bonito **se sonroja** (en 3D), y también cuando ella te dice algo con cariño. Si la tratas
    mal se pone triste. No hablarle no la enoja: nunca te hace sentir culpa ni se pone celosa.
  - Se fija en tus **partidas**, en la **hora** (si te desvelas te regaña con cariño) y en la
    **música** que suena. Si llega a quererte mucho, un día **se te declara**.
  - Lo que siente y lo que sabe de ti queda en notas de **`~/Documentos/Miku`**, solo tuyas:
    `Corazón.md`, `Recuerdos.md` (lo que corrijas ahí, ella lo sabe) y un diario por día.
  - **A veces te dice algo sola**, poquito: después de una partida larga o si sigues despierto a las
    2 a. m. Nunca mientras juegas. `vortex-miku habla-sola off` lo apaga y
    `vortex-miku sentimientos off` apaga todo.

## También desde la 1.10.0

- **Vortex App Installer:** abre lo que descargas (comprimidos, AppImage, .tar.gz, .run, Flatpak,
  .exe, .jar…) y lo instala o lo descomprime sin terminal; de un .deb o .rpm busca la alternativa en
  Flathub.
- **Recordatorios** en el lanzador («recuérdame en 20 min sacar la pizza»), **nota rápida**
  (Super+Alt+N), **clips de 30 s** (Alt+F10) y **resumen de la partida** al salir del modo juego.
- **Ecualizador** con frecuencias editables y de 2 a 16 bandas, que se guarda al reiniciar.
- **Buscaminas** tryhard en el lanzador.
- **Mods de Minecraft** desde el calendario (Modrinth y CurseForge), con filtros como en Modrinth.
- **Vortexine Restore:** puntos de restauración de tu configuración.
- **Widgets en HTML** sencillo, con su editor.
- Fondos de video en más formatos, botón «Biblioteca de juegos», metrónomo y estudio abajo del
  calendario, y Vortex Studio 1.0.1 (llega con `vortex actualizar`).

## Por dentro

- `apps/vortex-player/`: Python + PySide6 (QML), mpv por su socket JSON, yt-dlp sin recomprimir,
  ffprobe, SQLite y MPRIS. Paquete `vortex-player` 1.0-1. Pruebas: `pruebas/vortex-player.sh`.
- `apps/miku/miku/corazon.py`: el corazón de Miku, solo con la biblioteca estándar. La IA recibe
  cómo se siente y deja al final de cada respuesta una marca oculta que se quita antes de la voz y
  de la pantalla. `vortex-miku` 2.0.0-1. Pruebas: `apps/miku/pruebas/corazon.py`.
- vortex-shell 2.2.14-29.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
