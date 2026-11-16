# Vortex Player en Rust (ficha) — Cris

La app oficial sigue siendo la de Python (`apps/vortex-player`, 1.7.3). Esta solo la **sustituye**
cuando cumple todo lo de abajo. **Regla del Player:** siempre son 2 versiones, la de la PC y la del cel
(`Vortex Player Android`, en Kotlin): cada función nueva va en las dos, y cada versión sale para Linux,
Windows y el cel.

## Lo que hay que igualar (de la app de Python)

**Motor (sin Qt, va primero, con `cargo test`):**
- [ ] **Biblioteca** (`biblioteca.py`): SQLite con las canciones de Música, Música/Vortex Player y las
      carpetas que agregues; cada archivo se lee una vez con ffprobe y se vuelve a leer si cambia su fecha;
      los que desaparecen se quitan. Playlists por rutas, y las importadas recuerdan su URL.
- [ ] **Calidad real** (`calidad.py`): códec, bits y frecuencia; «Hi-Res», «CD» o códec + kbps.
- [ ] **Reproductor** (`reproductor.py`): mpv sin ventana por su socket JSON (IPC), a la frecuencia y bits
      originales por PipeWire, sin normalizar el volumen. La cola, el aleatorio y el repetir los lleva la
      app; la cola «A continuación»; temporizador para dormir; **crossfade** con un segundo mpv; radio sin
      fin (`seAcaba`) y cuánto escuchaste de cada canción («Tu resumen»).
- [ ] **Salida de audio** (`audio.py`): códec Bluetooth (LDAC a 990 kbps) y frecuencia nativa.
- [ ] **Descargas** (`descargas.py`): YouTube y YouTube Music con yt-dlp sin recomprimir; Spotify solo
      lee nombres de la página pública y el audio sale de YouTube Music.
- [ ] **Avance** (`avance.py`): 30 s de una canción de YouTube Music antes de bajarla (mpv aparte).
- [ ] **Letras** (`letras.py`): LRCLIB sincronizadas, si no las de YouTube Music; caché en
      `~/.cache/vortex-player/letras`; el mismo orden que `Letras.kt` del cel.
- [ ] **Nube y sincronía con el cel** (`cuenta_google.py`, `nube.py`, `sincronia.py`): Google con permiso
      solo para la carpeta oculta de la app en Drive; el mismo JSON que `Nube.kt`.
- [ ] **Ecualizador** (`ecualizador.py`): el mismo del panel de música (`eq_state.json` + `equalizer.sh`).
- [ ] **MPRIS** (`mpris.py`, en Rust con `zbus`) y **Discord Rich Presence** (`presencia_discord.py`).
- [ ] **Una sola ventana** (`__main__.py`): una segunda instancia le pasa el enlace o los archivos a la
      primera por un socket local; app_id `vortex-player`.
- [ ] **Windows** (`plataforma.py`): mpv, ffmpeg, ffprobe, yt-dlp y deno en `bin/`, junto al .exe.

**Interfaz (cxx-qt + los QML de `vortex_player/qml`, sin rehacerlos):** Main, Barra, PaginaCanciones,
PaginaCola, PaginaAgregar, PaginaAjustes, PaginaEcualizador, PaginaLetra, MiniReproductor y los
componentes. Hoy hablan con `vp` y `rep` (`puente.py`, ~1 800 líneas): el puente en Rust expone los
mismos nombres para que los QML casi no cambien.

**Pruebas que hay que portar** (`apps/vortex-player/pruebas/`): `test_motor.py` (35 casos),
`test_interfaz.py` (15), `test_discord.py` (21) y `test_nube.py` (11). Usan mpv falso
(`VORTEX_PLAYER_AO=null`), carpetas temporales y QML offscreen.

## «Listo para sustituir»
1. Todas las casillas de arriba, revisadas por el tester en Vortexine's OS.
2. Las pruebas portadas pasan (las mismas situaciones que las de Python).
3. Mismo aspecto: capturas lado a lado en la VM con la misma paleta.
4. Rendimiento igual o mejor: arranque, RAM en reposo y CPU tocando música, medidos contra la de Python.
5. Lee la biblioteca, las playlists y la configuración de la de Python sin perder nada, y se puede volver.
6. Corre en Linux y en Windows (instalador de Inno Setup).
