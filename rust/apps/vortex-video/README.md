# Vortexine Video en Rust (`vortex-video-rs`)

La cámara web de Vortexine's OS, reescrita en Rust. **La app oficial sigue siendo `vortex-video` (Python)**
hasta que esta la iguale; esta se instala aparte como `vortex-video-rs`.

- `src/` — el **motor sin Qt** (`lib vortex_video`), con `cargo test`. Lee y escribe los mismos archivos que la
  de Python: `~/.config/vortex/video.json`, Imágenes/Cámara y Vídeos/Cámara, `~/.cache/vortex-video/miniatura.jpg`
  y el mismo socket de «una sola ventana» (`vortex-video-<uid>`).
- `qt/` — **la ventana** (cxx-qt): `Puente` (= `vv`), `Colores` (= `colores`) y un arranque en C++ que los pone en
  el contexto de QML. Carga **los mismos QML** de `apps/vortex-video/vortex_video/qml` (no se copian): la captura
  offscreen sale idéntica a la de Python, píxel por píxel.
- `vortex-video-motor` — el motor sin ventana, para revisarlo (`--estado`) y medirlo (`--medir`).

## Funciones de la de Python, una por una

| Función (Python) | Rust | Dónde |
|---|---|---|
| Ajustes en `vortex/video.json`, con valores de fábrica y claves con tipo equivocado ignoradas | ✅ | `ajustes.rs` |
| «modo» solo foto/video; temporizador solo 0, 3, 5, 10 | ✅ | `ajustes.rs` |
| `guardar(clave, valor)` solo si cambió; escritura atómica | ✅ (además revisa el tipo y no borra claves desconocidas) | `ajustes.rs`, `estado.rs` |
| Elegir formato: la resolución elegida, si no la mayor fluida (≥ 24 fps), con más fps | ✅ | `formatos.rs` |
| Lista de resoluciones fluidas de mayor a menor | ✅ | `formatos.rs` |
| Etiquetas «1920 × 1080 (Full HD)», 4K, 2K, HD | ✅ | `formatos.rs` |
| Duración «MM:SS» / «H:MM:SS» | ✅ | `formatos.rs` |
| Nombres «Foto 2026-11-05 14.03.21.jpg», «… (2)» sin pisar, hora local | ✅ | `capturas.rs`, `hora.rs` |
| Fotos en Imágenes/Cámara, videos en Vídeos/Cámara (`user-dirs.dirs`); `VORTEX_VIDEO_CARPETA` en pruebas | ✅ | `capturas.rs` |
| Última captura al abrir (la más nueva de las dos carpetas) | ✅ | `capturas.rs` |
| `registrar(ruta o file://)` | ✅ | `estado.rs` |
| Miniatura del último video con ffmpeg (20 s máx., `?mtime` para la caché de QML) | ✅ | `miniatura.rs`, `qt/src/puente.rs` |
| Abrir la última captura y la carpeta | ✅ (con `xdg-open`; no se prueba para no abrir nada) | `qt/src/puente.rs` |
| Una sola ventana (`vortex-video-<uid>`, «mostrar»), compatible con la de Python | ✅ | `instancia.rs` |
| `sinCamara` (`VORTEX_VIDEO_SIN_CAMARA`) para pruebas | ✅ | `qt/src/puente.rs` |
| Paleta del fondo en vivo (`colores.paleta`, `VORTEX_VIDEO_COLORES`) | ✅ motor (vortex-core) / ⏳ ventana | `qt/src/colores.rs` |
| Estilo Basic, Adwaita Sans 14 px, id `vortex-video`, `QSG_RENDER_LOOP=basic` | ✅ | `qt/cpp/arranque.cpp` |
| Cámara, foto, grabar, micrófono, temporizador, cuadrícula, pantalla completa | = los mismos QML (Qt Multimedia) | `apps/vortex-video/vortex_video/qml` |

✅ hecho y probado

## Pruebas
- `tests/motor.rs`: las 11 de `pruebas/test_motor.py`, portadas, y 8 más (JSON escrito por Python, guardar con
  tipos, rutas y `registrar`, URLs, `user-dirs.dirs`, hora, miniatura con ffmpeg falso, una sola ventana).
- `qt/tests/interfaz.rs`: `pruebas/test_interfaz.py` portada (offscreen, sin cámara): pasa.

## Rendimiento (offscreen, sin cámara, release)
| | Python | Rust |
|---|---|---|
| Ventana: arranque hasta lista | ~1.0 s | ~0.95 s |
| Ventana: pico de RAM | ~347 MB | ~314 MB |
| Motor solo (5000 capturas) | ~30 ms, 16 MB | ~4.5 ms, 2.7 MB |

La mayor parte de la RAM es Qt Quick + Qt Multimedia, igual en las dos.

## Listo para sustituir cuando
Mike la pruebe con su cámara de verdad (foto, video, micrófono, temporizador) y se empaquete como `vortex-video-rs`
(PKGBUILD aparte; los binarios en Release/CI esperan su decisión).
