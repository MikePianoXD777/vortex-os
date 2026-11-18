# Vortexine's App Installer, en Rust

Reescritura de `apps/app-installer` (Python + PySide6/QML). **La app de Python sigue siendo la oficial**
hasta que Mike una esta a `main`: este crate se instala aparte (`vortex-app-installer-rs`) y no le quita nada
a nadie. La interfaz **no se rehizo**: la ventana usa con cxx-qt los mismos 14 QML (copiados en `qt/qml/`,
sin cambios). Lo único nuevo es el objeto `instalador` (en Rust) y la señal `cambio`, que el QML ya esperaba.

## Qué hay

| Crate | Qué es |
|---|---|
| `vortex-app-installer` (`src/`) | El motor, **sin Qt**: todo lo que no es la ventana. Lo mismo que los módulos de Python, uno a uno. |
| `vortex-app-installer-qt` (`qt/`) | La ventana (cxx-qt): `Puente` (= `instalador`), `Colores` (la paleta del fondo) y el arranque en C++. |

Módulos del motor (entre paréntesis, el `.py` del que salen): `textos` (`textos.py`, es/en), `rutas`,
`comprimidos` (listar y descomprimir sin peligro: rutas con `../`, enlaces que se salen, contraseñas,
cancelar sin dejar restos), `detectar` (qué es el archivo, por firma y extensión), `paquetes` (.deb y .rpm),
`flathub` y `flatpak`, `mods` (Minecraft), `registro` y `papelera` (lo instalado y cómo se deshace, siempre a
la papelera), `instalar` (AppImage, programas .tar.gz, scripts, Windows, Java), `motor` (qué se ofrece y
hacerlo), `sesion` (lo que hacía `puente.py` sin Qt) e `instancia` (una sola ventana por persona, el mismo
socket que la de Python).

El motor llama a los mismos programas que Python (`bsdtar`, `7z`, `unrar`, `flatpak`, `curl`, `kitty`,
`wine`…); no reimplementa formatos. Nada de lo que corre en pruebas es real.

## Cómo se usa

```sh
cargo build -p vortex-app-installer-qt      # compila la ventana (cxx-qt, Qt 6)
vortex-app-installer-rs [ARCHIVO]           # abre la ventana
vortex-app-installer-motor ARCHIVO          # el motor sin ventana: lo que se le ofrecería, como JSON
vortex-app-installer-motor --instalados     # lo ya instalado
```

Los QML se buscan en `VORTEX_APP_INSTALLER_QML`, en `qt/qml/` (al compilar desde el repo) o en
`/usr/share/vortex-app-installer-rs/qml`. Variables que también entiende la de Python:
`VORTEX_APP_INSTALLER_IDIOMA` (`es` o `en`), `VORTEX_APP_INSTALLER_COLORES` (otro `qs_colors.json`) y
`VORTEX_APP_INSTALLER_FLATHUB`.

## Pruebas

```sh
rust/pruebas.sh
# o a mano, con el límite de memoria de siempre:
systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- timeout 1800 cargo test
```

- **`tests/comprimidos.rs`, `detectar.rs`, `instalar.rs`, `motor.rs`** son las pruebas de Python
  (`test_comprimidos.py`, `test_detectar.py`, `test_instalar.py`, `test_motor.py`) portadas. Cada una corre en una
  casa temporal con programas falsos (`flatpak`, `curl`, `vortex`, `kitty`, `wine`…) en un `PATH` propio:
  **nunca** instala nada de verdad ni toca la carpeta personal, el lanzador, Flatpak, Minecraft ni la red.
  Los archivos de muestra (tar, zip, .deb, AppImage, mods) se generan en la prueba (`tests/comun/mod.rs`).
- **`qt/tests/interfaz.rs`** (`test_interfaz.py`): la ventana offscreen, en español y en inglés, abre un zip, un
  AppImage, un mod y un script, descomprime el zip y mira la pestaña «Instalado». Falla si QML o Qt avisan de algo.
- Siempre `QT_QPA_PLATFORM=offscreen`, sin `WAYLAND_DISPLAY`: ninguna prueba abre ventanas en tu pantalla.

### Capturas (como `pruebas/capturas.py`)

```sh
env -u WAYLAND_DISPLAY -u DISPLAY QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  vortex-app-installer-rs --capturas CARPETA ARCHIVO[::ACCION]… [--accion ID] [--contrasena X] [--ancho 760] [--alto 720]
```

Por archivo saca la captura al abrirlo y, si hay acción, el progreso y el resultado; al final, «Instalado».
Con `HOME` temporal y `VORTEX_APP_INSTALLER_COLORES=/nonexistent` (paleta fija) las capturas salen
**idénticas píxel a píxel** a las de la app de Python (se compararon vacío, zip, descomprimir, AppImage,
script, desconocido e «Instalado»).

## Memoria (offscreen, 30 s en reposo, sin archivo)

| | RssAnon | Rss |
|---|---|---|
| Python (PySide6) | 43,8 MB | 116 MB |
| Rust (build de depuración) | 21,5 MB | 82 MB |

## Qué falta

- Instalar el binario, el `.desktop`, los íconos y los QML en un paquete (`packaging/`) y la prueba en la VM con
  Hyprland de verdad (enfocar la ventana, arrastrar y soltar, el selector de archivos). Aquí solo se probó offscreen.
- Compararlo con la de Python en pantalla real, con una paleta de matugen y con la ventana del Runner/Shop.
