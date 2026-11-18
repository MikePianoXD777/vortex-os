# Vortexine's Runner en Rust (`vortex-runner-rs`)

Abre programas y juegos de Windows (.exe) con Proton y `umu-run`, reescrito en Rust. **La app oficial sigue
siendo `vortex-runner` (Python)** hasta que esta la iguale; esta se instala aparte como `vortex-runner-rs`.
Comparten la biblioteca (`~/.local/share/vortex-runner-datos/biblioteca.json`, los íconos y los prefijos),
los accesos del lanzador y el socket de «una sola ventana» (`vortex-runner-<uid>`).

- `src/` — el **motor sin Qt** (`lib vortex_runner`), con `cargo test`: rutas, lector de PE, Proton,
  biblioteca, `umu-run` y la lógica de `puente.py` (`estado.rs`).
- `qt/` — **la ventana** (cxx-qt): `Puente` (= `runner`), `Colores` (= `colores`) y un arranque en C++ que los
  pone en el contexto de QML. Los **QML (`qt/qml/`) son copia exacta, sin ningún cambio**, de
  `apps/vortex-runner/vortex_runner/qml`: el puente de Rust tiene los mismos nombres de propiedades, señales
  y slots que `puente.py`.
- `vortex-runner-motor` — el motor sin ventana: `--correr ID`, `--estado` (qué ve el motor) y `--version`.

## Funciones de la de Python, una por una

| Función (Python) | Rust | Dónde |
|---|---|---|
| Rutas: `~/.local/share/vortex-runner-datos` (biblioteca, íconos, prefijos), accesos en `applications/`; `VORTEX_RUNNER_HOME` | ✅ (sin estado global: se calculan por llamada) | `rutas.rs` |
| Nombre del programa (ProductName / FileDescription; los genéricos y los de más de 60 letras se ignoran) o el nombre del archivo arreglado | ✅ | `pe.rs` |
| Ícono más grande del .exe (RT_GROUP_ICON) como `.png` o `.ico`; solo lee cabecera y recursos (≤ 64 MB); lo raro da `None` | ✅ (a mano, sin crates) | `pe.rs` |
| Proton: CachyOS (slr, nativo), GE, Steam oficial (`common/Proton*`), sin repetir, orden numérico, «GE-Proton» al final | ✅ | `protones.rs` |
| `biblioteca.json` con el mismo formato (`indent=1`, sin escapar acentos), guardado atómico, claves desconocidas se conservan | ✅ | `biblioteca.rs` |
| Agregar (.exe/.msi/.bat/.lnk), sin repetir, con nombre e ícono; prefijo propio si el .exe vive en uno | ✅ | `biblioteca.rs` |
| Cambiar nombre / Proton / argumentos / .exe (otras claves se ignoran); el acceso se pone al día | ✅ | `biblioteca.rs` |
| Quitar (con o sin su prefijo; nunca prefijos ajenos ni el .exe) y descartar prefijos huérfanos | ✅ | `biblioteca.rs` |
| Después de un instalador: .exe nuevos sin desinstaladores, sin lo de serie de Proton/Wine (`tracked_files`), el más probable primero | ✅ | `biblioteca.rs` |
| Accesos `.desktop` («Agregar al lanzador») | ✅ | `biblioteca.rs` |
| `umu-run` con `WINEPREFIX`, `PROTONPATH`, `GAMEID`, `STORE=none`, `VORTEX_RUNNER_ID`; argumentos con comillas; `.msi` con `msiexec /i`; registro en `registros/<id>.log` | ✅ | `correr.rs` |
| Ver qué sigue abierto (procesos tuyos con su `WINEPREFIX`, una pasada por `/proc`), detener (SIGTERM, luego SIGKILL) | ✅ | `correr.rs` |
| Instalar el motor con `pacman` vía `pkexec` | ✅ | `correr.rs` |
| `--correr ID` (los accesos): abre, espera, suma el tiempo de juego | ✅ | `estado.rs`, `main.rs` |
| Texto «Abierto hace 2 h · 5 h en total», tiempo de juego cada 3 s | ✅ | `estado.rs` |
| Instalador en un prefijo nuevo → «¿cuál es el programa?» → agregar / «Ninguno» (borra lo instalado) | ✅ | `estado.rs` |
| Un .exe abierto con doble clic: se abre si ya está, si no se pregunta (instalador o programa) | ✅ | `estado.rs` |
| Una sola ventana (`vortex-runner-<uid>`, `mostrar` / `abrir<TAB>ruta`), compatible con la de Python | ✅ | `instancia.rs` |
| Paleta del fondo en vivo (`colores.paleta`, `VORTEX_RUNNER_COLORES`) | ✅ (`vortex-core`) | `qt/src/colores.rs` |
| Estilo Basic, Adwaita Sans 14 px, id y nombre de la app, ícono del tema o `logo.svg` | ✅ | `qt/cpp/arranque.cpp` |
| Toda la interfaz (biblioteca, búsqueda, arrastrar y soltar, diálogos, atajos, avisos) | = los mismos QML | `qt/qml/` |

✅ hecho y probado

## Lo que cambia o falta

- `lanzar` usa `process_group(0)` (grupo de procesos propio) en vez de `start_new_session` (`setsid`): la
  biblioteca estándar no tiene `setsid` sin `unsafe`. Para cerrar Runner sin cerrar el juego basta.
- Si las comillas de los argumentos no cierran, Python truena en `shlex.split`; aquí se parte por espacios.
- Los íconos con acentos raros en el id: el «sin acento» es una tabla de letras latinas comunes (sin crate
  de normalización Unicode); el resto se vuelve «-», igual que en Python.
- «Instalar motor»: sin límite de 1 h (Python tenía `timeout=3600`); pkexec cancelado da el mismo aviso.
- Los accesos `.desktop` que crea esta app llevan `Exec=` con **`vortex-runner-rs`** (la ruta completa si es
  el propio ejecutable; si no, `vortex-runner`). Mientras convivan las dos, el último que guarda manda.
- Falta: instalarla (PKGBUILD, `.desktop`, ícono, `/usr/share/vortex-runner-rs/qml`), la prueba en la VM de
  Vortexine's OS con un Proton y un .exe reales (aquí nunca se corre uno) y el CI (`rust/ci`).

## Cómo se prueba

```sh
rust/pruebas.sh        # cargo fmt --check, clippy -D warnings y cargo test, con 2 GB y HOME temporal
```

- `tests/motor.rs` (34) + pruebas dentro de `src/` (8): las **19** de `pruebas/test_motor.py` portadas (menos
  las de «.exe de verdad del Proton de CachyOS», que aquí usan **PE generados en la prueba**: con versión,
  íconos PNG y DIB, cortados…) y las de `puente.py` (`flujo_de_la_biblioteca`, `las_preguntas`,
  `instalar_y_elegir_el_programa`, `sin_motor`…), con **un `umu-run` falso** que solo anota sus argumentos y su
  entorno, carpetas de Proton falsas y una casa temporal. No se corre Proton, umu ni un .exe real, y las
  pruebas no usan variables de entorno globales (cada una arma su `Contexto`), así que corren en paralelo.
- `qt/tests/interfaz.rs` (3): `pruebas/test_interfaz.py` portada, offscreen (`QT_QPA_PLATFORM=offscreen`,
  nunca abre una ventana): los QML cargan **sin una sola advertencia** con la biblioteca vacía, con
  programas y sin el motor; búsqueda, «Configurar», «Quitar», la pregunta de un .exe y los avisos.
  Con `VORTEX_RUNNER_CAPTURAS=/carpeta` guarda capturas (`grabWindow`).
- Para compilar la ventana: Qt 6 con headers y `QMAKE=qmake6`; con 2 GB usa `CARGO_BUILD_JOBS=2` si el
  compilador de C++ se queda sin memoria (el `cxx-qt-lib` de clippy compila muchos archivos C++).

### Capturas offscreen contra la de Python
Mismo escenario en las dos (dos programas, búsqueda, «Configurar», «Quitar», pregunta de un .exe): las 5
capturas de 1180 × 780 son **idénticas píxel por píxel**, salvo 2378 píxeles de «Configurar»: el texto de la
ruta del .exe (cada una corre en su carpeta temporal).
