# Vortexine's OS 2.5.0

Tres cosas para todos los días, ya prendidas y sin configurar nada: **tus tareas en la barra**, **bajar
con copiar el enlace** y **fotos de la tarea a PDF**.

## Lo nuevo

### Tus tareas en la barra

- **La barra cuenta tus tareas de hoy y de mañana** (y las vencidas). Con clic en la cuenta se abre el
  panel **Tareas**; también está en el lanzador como «Tareas».
- **En el panel:**
  - Salen por fecha: Hoy, Mañana, el día de la semana o la fecha.
  - **Para agregar una:** la escribes, eliges para cuándo es (Hoy, Mañana, Pasado mañana, En una semana o
    Sin fecha) y das Enter.
  - **Para marcarla hecha:** clic en el círculo.
- **Viven en tu Obsidian**, en la bóveda que tengas abierta:
  - Las que agregas van a `Tareas.md`.
  - También salen las tareas de tus notas que tengan fecha. La fecha se escribe como en el plugin Tasks
    (`- [ ] Maqueta de ciencias 📅 2026-11-03`), como `due:: 2026-11-03`, o corta: `(3/11)` o «para el
    3/11».
  - Al marcarla hecha queda en su nota como `- [x] … ✅ fecha`.
  - Sin Obsidian, se guardan en `Documentos/Tareas.md`.
- **Te avisa una vez por tarea:**
  - Las de hoy y las vencidas, desde las 7 am.
  - Las de mañana, desde las 5 pm del día antes.

  El aviso trae el botón «Ver mis tareas» y no sale mientras juegas.

### Bajar con copiar el enlace

Cuando copias el enlace de un video de **YouTube, TikTok, Instagram o X**, sale un aviso con dos botones:

- **«Bajar video»:** un mp4 de hasta 1080p, en tu carpeta **Videos**.
- **«Solo audio»:** un mp3 con su portada, artista y título, en tu carpeta **Música**. Ahí lo encuentra
  Vortex Player.

El avance se ve en el mismo aviso. Al terminar dice «Listo», con «Abrir» y «Ver en la carpeta».

No sale mientras juegas, ni si copias dos veces el mismo enlace, ni con un texto que solo trae un enlace
adentro.

### Fotos de la tarea a PDF

En el gestor de archivos (Super+E), selecciona las fotos de tu cuaderno → clic derecho → **«Hacer PDF»**.

A cada foto:

- la endereza;
- la recorta a la hoja (si se ve la mesa alrededor);
- le quita las sombras y deja el papel blanco parejo.

Todas quedan en un PDF junto a las fotos, en el orden de sus nombres. Un aviso te dice cuando está listo,
con «Abrir».

## Para actualizar

```
vortex actualizar
```

Instala lo que necesitan estas funciones (yt-dlp, ffmpeg, deno e ImageMagick), agrega «Hacer PDF» al gestor
de archivos y arranca el aviso de enlaces, sin cerrar sesión.
