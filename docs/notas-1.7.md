# Vortexine's OS 1.7

Fondos animados (con Gargantua de regalo), los íconos de tus apps en la barra, todo más ligero, y la misma voz de Miku en más PCs.

## Lo nuevo

- **Fondos animados (videos) que no gastan:**
  - Los videos (`.mp4`, `.webm`, `.mkv`, `.mov`) que pones de fondo desde el selector (Super+Shift+W) ahora se
    **pausan solos** cuando una ventana los tapa, cuando la PC usa la batería y tras 3 minutos sin usar
    la PC: si no los ves, casi no gastan.
  - **Capa de optimización:** la primera vez se hace una copia del video al tamaño de tu monitor, a 30 fps
    como mucho y en H.264 (lo decodifica la GPU en casi cualquier PC). Un fondo 4K a 60 fps pasa a gastar
    una fracción. La copia se hace con prioridad baja y se borra si no la usas en 30 días.
  - **Gargantua, el primer fondo animado de Vortex:** un agujero negro con su disco de acreción girando,
    24 s en bucle sin corte. Está en tu carpeta de fondos (`gargantua.mp4`); en una instalación que ya
    tenías llega con `vortex actualizar` (si lo borras, no vuelve).
  - Todo se ajusta en **Ajustes → Tema → Fondos animados**: que se muevan o queden quietos, pausar al taparlos,
    pausar con batería y la optimización (todo encendido de fábrica).
- **Los íconos de tus apps en los escritorios:** la barra muestra, en cada escritorio, los íconos de las apps
  que tiene abiertas (hasta 3). Se apaga en Ajustes → Barra → Escritorios.
- **Todo más ligero:**
  - El shell redibujaba el desenfoque del fondo aunque la barra no fuera transparente. Ahora solo lo hace
    con transparencia real: de ~213 a ~12 despertares por segundo y unos 270 MB menos de RAM, y se ve igual.
  - El aviso de actualizaciones corre con prioridad mínima y ya no se queda colgado.
  - Ya no sale el aviso «Actualización disponible» del shell base: en Vortexine's se actualiza con
    `vortex actualizar`.
  - La ISO trae los decodificadores de video por GPU para Intel y NVIDIA (AMD ya los tenía).
  - **Miku:** si ella arrancó la IA local (Ollama), la apaga al cerrarse; Ollama atiende una plática y un
    modelo a la vez (cabe junto a la voz en una GPU de 6 GB); la voz y el oído usan los núcleos físicos; y el
    avatar duerme entre cuadros en vez de despertar con cada refresco de la pantalla.
- **La voz natural de Miku, la misma en más PCs:**
  - **NVIDIA serie 10 (como la GTX 1060) o de 4-5 GB:** antes caían al procesador o a la voz de siempre.
    Ahora usan el mismo modelo que las GPU nuevas (la misma voz), en la GPU, con qwentts.cpp (~4 GB).
  - **En el procesador:** con 12 GB de RAM o más, ahora es también la misma voz que en las GPU (antes era un
    modelo más chico), casi igual de rápida. Con menos RAM sigue el modelo ligero.
  - **Se arregló que algunas frases cortas salieran mudas** («Hola, ¿cómo estás?»).
  - Si tu voz natural quedó en el procesador, Miku la mejora sola en segundo plano la próxima vez que la
    abras (y mientras, sigue hablando con la de antes). Una voz que ya va en una GPU serie 20 o más nueva no
    cambia.
  - `vortex-miku natural estado` dice en qué modo va.

## Por dentro

- Gargantua se genera con `config/fondos/gargantua.py` (numpy + ffmpeg): sigue la luz alrededor del agujero
  negro con la relatividad de Schwarzschild, por eso el disco de atrás se ve doblado por arriba y por
  abajo. Cada anillo del disco da vueltas enteras en los 24 s, así el bucle no tiene salto. Es original, no
  sale de ninguna película.
- `shell/src/scripts/wallpaper/optimizar_video.sh` hace la copia optimizada (ffmpeg con `nice`/`ionice`, una
  sola vez por video y tamaño).
- La voz natural tiene tres modos: `gpu` (PyTorch, serie 20+), `cuda` (qwentts.cpp con CUDA 12, serie 10+) y
  `cpu` (qwentts.cpp). Las compilaciones de qwentts.cpp son de Vortex, fijadas con sha256, con un parche
  (`QWENTTS_MIN_NEW_TOKENS`) que no deja al modelo terminar antes de tiempo: el Release `miku-qwentts-2`.
- La voz natural tiene un tope de duración proporcional al texto: a veces se desbocaba y generaba casi 3
  minutos de audio.

## Actualizar

```bash
vortex actualizar
```

## Requisitos

Los mismos de la 1.6.
