# Cómo colaborar en las apps de Vortexine (Rust)

Estamos reescribiendo las apps de Vortexine's OS en Rust, **una por una**. La interfaz no se rehace:
cada app usa con **cxx-qt** los mismos QML que tiene hoy la app de Python. La app de Python sigue siendo la
oficial hasta que la de Rust la iguale: **no se le quita nada a nadie**.

## Quién hace qué

| Persona | App | Ficha |
|---|---|---|
| **Cris** | **Vortex Player**, el piloto. También termina `vortex-core-qt` | [`apps/vortex-player/README.md`](apps/vortex-player/README.md) |
| **Fósil prehistórico** | **Vortexine's Local AI** | [`apps/vortex-local-ai/README.md`](apps/vortex-local-ai/README.md) |
| **El tester** | Prueba todo en Vortexine's OS (en la VM) antes de cada unión | — |
| **Mike** | Revisa, une a `main`, publica y hace las Releases. **Solo él.** | — |

Miku y Vortex Studio no se reescriben por ahora.

## Qué instalar

No hace falta Vortexine's OS: sirve cualquier Linux, o Windows.

- **Rust** con [rustup](https://rustup.rs). La versión exacta la baja solo `rust-toolchain.toml` (1.100.0).
- **Qt 6** con sus headers y un **compilador de C++** (lo pide cxx-qt):
  - Arch / CachyOS: `qt6-base qt6-declarative gcc`
  - Ubuntu / Debian: `qt6-base-dev qt6-declarative-dev qml6-module-qtquick qml6-module-qtquick-window g++`
  - Fedora: `qt6-qtbase-devel qt6-qtdeclarative-devel gcc-c++`
  - Windows: Visual Studio Build Tools (C++) y Qt 6 con `aqtinstall` (ver `ci/rust-windows.yml`).
- Lo que use tu app. **Player:** `mpv`, `ffmpeg` y `yt-dlp`. **Local AI:** nada para el motor (las
  pruebas usan un Ollama falso); para probar a mano, `ollama`.
- Sin Hyprland ni matugen, la app usa la **paleta de respaldo** y «modo juego» siempre dice que no. Así
  debe funcionar también.

## Cómo se trabaja

1. Haz un **fork** de `vortex-os` y una **rama por tema** de tu app: `rust/player-cola`,
   `rust/local-ai-ollama`…
2. Trabaja **solo dentro de `rust/`**. Las apps de Python (`apps/`) no se tocan desde aquí. Si encuentras
   un error en ellas, abre un issue.
3. Antes de cada Pull Request:
   ```sh
   rust/pruebas.sh     # cargo fmt --check, clippy -D warnings y cargo test, con 2 GB y HOME temporal
   ```
   - **Las pruebas nunca abren ventanas** (`QT_QPA_PLATFORM=offscreen`).
   - **Llevan límite de memoria:** `systemd-run --user --scope -p MemoryMax=2G -p MemorySwapMax=0 -- timeout …`
     (ya viene en `pruebas.sh`).
   - **No usan nada real de tu cuenta:** HOME y XDG temporales, nada de tu llavero, tu cámara, tu
     micrófono, tu Ollama ni tus modelos. Usa programas falsos (un mpv o un Ollama de mentira), como
     las pruebas de Python.
4. Abre el **Pull Request hacia `main`** con qué cambia, cómo lo probaste (comandos y resultado),
   capturas si cambia algo que se ve, y qué falta. Un tema por PR.
5. Lo revisan el tester (en Vortexine's OS) y Mike. **Solo Mike une y publica.**

## Reglas del proyecto

- **Español** en la interfaz, los comentarios, los commits y los PR.
- **Nada de contraseñas, llaves ni tokens** en el repo. Tampoco en las pruebas: usa valores de mentira.
- **Nada con derechos de autor**: imágenes, música, personajes ni fondos de otros.
- **Privacidad:**
  - nada de cámara ni micrófono a escondidas: siempre con indicador y aviso, y solo si la persona lo pide;
  - **nada de telemetría**;
  - nada se manda a internet sin que la persona lo pida.
- Funciones **que sirvan solas**, sin configurar nada.
- Cada app tiene **su propio logo** (el Rotor es solo del sistema).
- Lo compartido va en `crates/vortex-core` (rutas, configuración, paleta, registro). No lo copies en tu app.
- Las versiones de las crates se fijan en `rust/Cargo.toml`. Para agregar una crate nueva, pregúntale
  antes a Mike: solo de crates.io, conocidas y con licencia libre.
- **Vortex Player siempre son 2 versiones, la de la PC y la del cel.** Lo que cambie en las funciones de
  una va también en la otra.

## Cómo está armado `rust/`

```
rust/
  Cargo.toml               workspace (versiones fijas, MSRV 1.100)
  rust-toolchain.toml      la versión de Rust de todos
  pruebas.sh               las pruebas como pide el proyecto
  crates/vortex-core/      rutas, config, paleta (idéntica a colores.py), registro
  crates/vortex-core-qt/   el puente con QML: Colores (la paleta en vivo) + Tema.qml + la demo
  apps/vortex-player/      motor (lib, sin Qt) + bin
  apps/vortex-local-ai/    motor (lib, sin Qt) + bin
  ci/                      los workflows de GitHub (inactivos hasta que Mike los active)
```

Cada app tiene **dos partes**:
- **El motor (`lib`)**, sin Qt: todo lo que se prueba con `cargo test`.
- **La interfaz (`bin`)**: cxx-qt + los QML de la app de Python, delgada; solo llama al motor.

## Primeros pasos

### Cris (Vortex Player)

1. **Termina `vortex-core-qt`** (ver su README): `cargo build`, ajusta lo que pida cxx-qt 0.10 y corre la
   demo sin pantalla. Que guarde su captura con la paleta real. Luego mételo a `members`.
2. **Saca la lista de funciones** del Player: ya hay un borrador en la ficha. Repásala contra la app de
   Python y su `pruebas/`, y márcala conforme avances.
3. **Porta primero el motor**, con sus pruebas:
   - la **biblioteca**: SQLite y el escaneo con ffprobe, con `rusqlite`;
   - la **cola**: aleatorio, repetir y «A continuación»;
   - el **IPC de mpv**: el socket JSON, con un mpv falso en las pruebas, como `VORTEX_PLAYER_AO=null`.
4. Después, el puente (`vp`/`rep`) con cxx-qt y los QML de `apps/vortex-player/vortex_player/qml`.
5. Recuerda la regla de las 2 versiones (PC y cel): si una función cambia, también cambia en
   `Vortex Player Android`.

### Fósil prehistórico (Vortexine's Local AI)

Mientras Cris cierra el paso 0, **porta el motor sin interfaz**, con sus pruebas. Te guías por
`apps/vortex-local-ai/API.md` y `pruebas/test_motor.py`:

1. **Rutas** (con `vortex-core::rutas` y las carpetas de la app vieja, para leer sus chats).
2. **Cliente de Ollama por HTTP** (estado, modelos, bajar con avance y chat en streaming), probado contra
   un servidor HTTP falso dentro de la prueba. Nunca contra el Ollama de verdad.
3. **El vault de notas**: [[enlaces]], #etiquetas, frontmatter, retroenlaces y grafo, con un vault de
   prueba en una carpeta temporal.
4. **La memoria** («Preguntar a mi vault»): fragmentos, embeddings (del Ollama falso), índice en SQLite y
   búsqueda por similitud.
5. Los **chats** (`conversaciones`).

La interfaz va cuando el piloto del Player haya validado cxx-qt.
