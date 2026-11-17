> **16/11/2026:** ya compila (cxx-qt 0.10.0 bajado de crates.io) y está en el workspace. Para que el módulo QML
> se registre, el binario debe enlazar la crate: `extern crate vortex_core_qt;` + `cxx_qt::init_crate!(vortex_core_qt);`.

# vortex-core-qt (todavía sin compilar)

El puente de `vortex-core` con QML usando **cxx-qt 0.10.0** (versión fija):

- `Colores`: un singleton QML (módulo `Vortex.Colores`) con `Colores.paleta`, la paleta real del fondo,
  que se actualiza sola cuando cambia el fondo (hilo que revisa cada 300 ms y `qt_thread().queue`).
- `qml/Vortex/Tema/Tema.qml`: el `Tema.qml` de Vortex Player **sin cambios**, salvo una línea:
  `Colores.paleta` en vez de `colores.paleta`.
- `vortex-demo-tema`: la demo del paso 0 (ventana con los colores del Tema).

## Por qué no compila todavía

El 16/11/2026 no se pudieron bajar las crates de cxx-qt (falta el permiso directo de Mike para bajar), así
que este crate está **fuera del workspace** (`exclude` en `rust/Cargo.toml`) y su código **no se ha
compilado nunca**. Está escrito con la API de cxx-qt 0.8+ que conoce Apps; puede pedir ajustes con 0.10.

## Para terminarlo (lo primero que hace Cris)

```sh
cd rust/crates/vortex-core-qt
cargo build                       # baja cxx-qt 0.10.0 y compila contra el Qt 6 del sistema (qmake6)
# Si truena: comparar build.rs y src/colores.rs con los ejemplos de cxx-qt 0.10
# (https://github.com/KDAB/cxx-qt/tree/v0.10.0/examples) y ajustar.
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software VORTEX_DEMO_CAPTURA=/tmp/demo.png \
  systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- timeout 60 cargo run --bin vortex-demo-tema
```

Cuando compile: meterlo a `members` en `rust/Cargo.toml`, pasar sus versiones a `[workspace.dependencies]`
y agregar su prueba offscreen al CI (`rust/ci/rust-linux.yml`).

**Necesita en Linux:** Qt 6 con headers (`qt6-base`, `qt6-declarative`; en Ubuntu `qt6-base-dev`,
`qt6-declarative-dev`), un compilador de C++ (`gcc` o `clang`) y `qmake6` en el PATH (o `QMAKE=/ruta/qmake6`).
En la PC de Mike ya está todo (Qt 6.12, gcc 16, clang 23).
