# Vortexine Connect en Rust

Reescritura de `apps/vortex-connect` (Python + PySide6). La app oficial sigue siendo la de Python hasta que esta
la iguale; aquí todo se instala aparte (`vortex-connect-rs`).

| Crate | Qué es |
|---|---|
| `vortex-connect` (este) | El motor sin Qt: redes (nmcli y netsh), protección (warp-cli), portal cautivo, cifrado, estado, modo viaje (`vigilar`), una sola ventana y la CLI. |
| `vortex-connect-qt` (`qt/`) | La ventana: cxx-qt con **los mismos QML** que la de Python, `Puente` («vc») y `Colores` en Rust y un arranque corto en C++. Binario `vortex-connect-rs`. |

## Pruebas

```sh
rust/pruebas.sh                      # fmt + clippy -D warnings + test de todo el workspace, con límite de memoria
cargo test -p vortex-connect         # el motor (tests/motor.rs y tests/viaje.rs), con nmcli, warp-cli, systemctl... falsos
cargo test -p vortex-connect-qt      # la ventana offscreen con un servidor propio en 127.0.0.1
```

Nunca se usa la red real, el WARP, el wifi ni el systemd de la persona: los programas falsos van en un `PATH`
temporal y la ventana siempre con `QT_QPA_PLATFORM=offscreen`. Con `VORTEX_CONNECT_CAPTURAS=/carpeta` el recorrido
guarda capturas (`inicio`, `condiciones`, `protegido`, `sin-proteger`).

## RAM

Ver el informe de la rama (ventana offscreen tras el recorrido, RssAnon / Rss, Rust contra Python).
