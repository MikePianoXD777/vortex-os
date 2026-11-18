# Vortexine Task Manager (Rust)

La reescritura en Rust de [`apps/vortex-task-manager`](../../../apps/vortex-task-manager) (Python + PySide6/QML),
el administrador de tareas de Vortexine's. **La app oficial sigue siendo la de Python** hasta que Mike diga.
La interfaz no se rehizo: son los mismos QML, copiados en [`qml/`](qml), con **un solo cambio**:
`PaginaProcesos.qml` lee la lista de `filasModelo` (una propiedad del contexto) en vez de `tm.filas`.

```
vortex-task-manager/
  src/        el motor (lib `vortex_task_manager`, sin Qt): /proc, grupos, rendimiento, VRAM, servicios…
  tests/      pruebas del motor, con un /proc y un /sys falsos
  qml/        los QML de Python
  qt/         la ventana (bin `vortex-task-manager-rs`): cxx-qt 0.10.0 + un poco de C++ (arranque y modelo)
```

## Qué es igual a la de Python

Todo lo que hacía la de Python, con los mismos nombres de propiedades y slots (`tm.*`):

- **Procesos:** apps (por ventana de Hyprland, con lo que cuelga de ella), segundo plano y sistema; nombres e
  íconos de los lanzadores `.desktop`; CPU en % de toda la máquina, memoria privada, disco por segundo, VRAM
  (nvidia-smi y fdinfo de AMD/Intel cada 3 s); buscar, ordenar, abrir/cerrar secciones y apps.
- **Acciones:** finalizar (SIGTERM, y SIGCONT para los pausados), forzar cierre (SIGKILL), cerrar ventana,
  suspender/reanudar, prioridad, abrir ubicación, copiar comando y PID; lo ajeno pide permiso con `pkexec`.
- **Rendimiento:** CPU (núcleos, GHz, temperatura), memoria y swap, discos físicos (modelo, tipo, uso, lectura,
  escritura, % activo), red (Wi-Fi/Ethernet, IP, SSID), tarjetas NVIDIA (nvidia-smi) y AMD/Intel (sysfs); la
  historia de 60 muestras.
- **Servicios:** `systemctl` de usuario y del sistema, «Al iniciar», iniciar/detener/reiniciar.
- **Ritmo:** cada 1 s (cada 3 s sin foco), procesos solo si se ven (o cada 5 s); las acciones van en otro hilo.
- **Una sola ventana** por persona: mismo socket que la de Python (`$TMPDIR/vortex-task-manager-<uid>`), así que
  Rust y Python tampoco se abren dos veces; Ctrl+Shift+Esc sigue encontrando la clase `vortex-task-manager`.
- **Colores:** `Colores.paleta` de `vortex-core` (idéntica a `colores.py`); `VORTEX_TASK_MANAGER_COLORES` la cambia.

## Qué cambia por dentro

- `psutil` → lectura directa de `/proc` y `/sys` (más `statvfs` y `getifaddrs` de libc, en `sys.rs`, el único
  módulo con `unsafe`). El motor no depende de ninguna crate nueva (solo `serde_json` y `libc`).
- `difflib` (las diferencias entre listas de filas) → `modelo.rs`: el motor calcula qué filas se quitan y se
  meten, y el modelo de Qt (C++, `qt/cpp/arranque.cpp`) las aplica; no parpadea ni pierde el desplazamiento.
- Los datos van a QML como JSON → `QVariant` (un ayudante de C++): los mismos mapas y listas que daba PySide.

## Qué falta o se ve distinto

- **No está instalada ni empaquetada:** no hay `.desktop`, ícono ni PKGBUILD de la de Rust; se corre desde el repo.
  Los QML se buscan en `$VORTEX_TASK_MANAGER_DIR/qml`, `qml/` del repo o `/usr/share/vortex-task-manager/qml`.
- **Sin probar en pantalla de verdad** (solo offscreen), ni con `pkexec` real, ni con procesos ajenos.
- Los procesos zombi salen (psutil los trataba igual salvo en algunas lecturas); `/proc/PID/io` de otros
  usuarios sigue sin leerse (igual que en Python).
- El temporizador del muestreo es un hilo propio (50 ms de resolución para despertar) y no un `QTimer`.
- La temperatura solo se lee de `hwmon` (psutil también miraba `/sys/class/thermal`).

## Cómo se prueba

Todo con límite de memoria y sin abrir ventanas (`rust/pruebas.sh` lo hace: fmt, clippy `-D warnings`, tests):

```sh
rust/pruebas.sh
# o solo esta app:
cd rust
CARGO_BUILD_JOBS=4 QMAKE=qmake6 systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- \
  env -u WAYLAND_DISPLAY -u DISPLAY QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 1200 cargo test -p vortex-task-manager -p vortex-task-manager-qt
```

- `tests/motor.rs` (36 pruebas): las de `test_motor.py` y las de lo que antes hacía psutil, con `/proc` y `/sys`
  falsos; **solo se mata un `sleep` propio**, nunca un proceso de la persona.
- `qt/tests/interfaz.rs`: `test_interfaz.py` portada. Lanza la ventana offscreen con `VORTEX_TASK_MANAGER_PRUEBA=1`
  (recorre lista, búsqueda, finalizar un `sleep` propio con nombre único, abrir una app, ordenar por VRAM y las
  tres páginas) y revisa que no haya ningún error de QML.
- Una captura sin pantalla: `VORTEX_TASK_MANAGER_CAPTURA=/ruta.png [VORTEX_TASK_MANAGER_PAGINA=rendimiento]
  [VORTEX_TASK_MANAGER_CAPTURA_MS=4000] vortex-task-manager-rs` (con `QT_QPA_PLATFORM=offscreen`).
