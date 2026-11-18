# Vortexine's Shop en Rust

La reescritura en Rust de la tienda de Vortexine's OS (la de Python está en `apps/shop`; la fuente de verdad
es la de la rama de la 4.0, con «Juegos de Android»). **En prueba:** la app oficial sigue siendo la de Python;
esta se instala aparte como `vortex-shop-rs`.

Dos crates:

- `vortex-shop` (este): el motor, Rust puro y sin Qt. Catálogo de Flathub (con `curl` y caché en disco),
  `flatpak` (instalar, quitar y actualizar con progreso real), apps propias (`vortex`), Android
  (`vortex-android`) y `tienda`, el estado que ve la interfaz con su cola de una sola vía.
- `vortex-shop-qt` (`qt/`): la ventana con cxx-qt =0.10.0. **Los QML son los mismos de la 4.0** (copiados sin
  cambios: `diff -r` contra `apps/shop/vortex_shop/qml` da vacío); el objeto `tienda` tiene las mismas
  propiedades, señales y slots que `puente.py`, y `colores` es la paleta del fondo.

## Qué está igual

Todo lo que tiene la de Python: inicio, categorías con «mostrar más», búsqueda, detalle, cola de
instalación, instaladas, actualizaciones (se revisan solas), apps propias (Camera, Runner), página «Juegos de
Android» (preparar, número de Google, Play Store), una sola ventana por persona (el mismo socket que la de
Python, así tampoco se abren las dos) y el enfoque en Hyprland. Las capturas offscreen salen **idénticas pixel
por pixel** a las de Python (solo difieren las animaciones en curso).

## Qué falta

- Empaquetado: no hay PKGBUILD ni lanzador `.desktop` de `vortex-shop-rs` todavía (los QML se buscan en
  `VORTEX_SHOP_QML`, en `qt/qml` del repo o en `/usr/share/vortex-shop-rs/qml`).
- Python también prueba libflatpak por GObject; aquí solo existe la ruta de la CLI de `flatpak`.
- Faltan las pruebas de la ventana de la cola con clics reales (`ui_cola.py`): la cola se prueba en el motor
  (`tests/tienda.rs`), sin ventana.

## Cómo se prueba

Siempre sin ventanas reales, sin internet y sin tocar tu `flatpak`, tu `pacman` ni tu `vortex-android`
(programas falsos en un PATH temporal; HOME y XDG temporales; un Flathub falso en 127.0.0.1):

    rust/pruebas.sh                       # fmt + clippy -D warnings + test, con límite de memoria
    VORTEX_SHOP_CAPTURAS=/carpeta cargo test -p vortex-shop-qt   # además guarda las capturas

- `tests/motor.rs` (70 pruebas): catálogo, texto, ids, flatpak, apps propias y Android (`test_motor.py`).
- `tests/tienda.rs` (14): la tienda y su cola (`ui_cola.py`, `ui_android.py`, `ui_integracion.py` sin ventana).
- `qt/tests/interfaz.rs` (2): la ventana offscreen recorre Inicio, categoría, búsqueda, detalle, Instaladas,
  Actualizaciones, «Quitar» y Android, sin errores de QML; y sin red.
