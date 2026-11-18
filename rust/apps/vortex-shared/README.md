# Vortexine Shared en Rust

Reescritura de `apps/vortex-shared` (Python + PySide6). La app oficial sigue siendo la de Python hasta que esta
la iguale; aquí todo se instala aparte (`vortex-shared-rs`, `vortex-shared-motor`).

| Crate | Qué es |
|---|---|
| `vortex-shared` (este) | El motor sin Qt: protocolo del aviso, mandar y recibir con `sendme`, el servicio `escuchar`, Thunar, ufw y la CLI. Binario `vortex-shared-motor` (lo que arranca `vortex-shared.socket`). |
| `vortex-shared-qt` (`qt/`) | La ventana: cxx-qt con **los mismos QML** que la de Python (copiados sin cambios), `Puente` («vs») y `Colores` en Rust y un arranque corto en C++. Binario `vortex-shared-rs`. |

## Lo que se portó

- `motor.py` → `src/motor.rs`: reglas del ticket y los nombres, quién puede avisar (red local o Tailscale),
  `Envio` (sendme con guardia: si quien manda muere, sendme se corta), `mandar`, `recibir` (nunca pisa nada),
  Tailscale, ufw, espacio libre.
- `servicio.py` → `src/servicio.rs`: escucha el socket de systemd (`LISTEN_FDS`), aviso con Aceptar/Rechazar, uno
  a la vez, no molesta jugando, se apaga solo a los 45 s sin nadie.
- `thunar.py` → `src/thunar.rs`, `__main__.py` → `src/cli.rs`.
- Una sola ventana por persona (`src/instancia.rs`): mismo socket y mismo mensaje que la de Python, así que las
  dos tampoco se abren a la vez.

## Pruebas

```sh
rust/pruebas.sh                      # fmt + clippy -D warnings + test de todo el workspace, con límite de memoria
cargo test -p vortex-shared          # el motor: 28 pruebas (sendme, notify-send y gdbus falsos; sockets en 127.0.0.1)
cargo test -p vortex-shared-qt       # la ventana offscreen: 3 pruebas
```

Las pruebas del motor son las de `pruebas/test_motor.py`, portadas, con programas falsos en un `PATH` temporal.
No tocan la red, el HOME, el cortafuegos ni los avisos del escritorio. La ventana siempre va con
`QT_QPA_PLATFORM=offscreen`. Con `VORTEX_SHARED_CAPTURAS=/carpeta` el recorrido guarda capturas (`con-archivos`,
`codigo`, `mac`).

## RAM (medida en esta máquina, offscreen, con los mismos falsos)

| | Rust | Python |
|---|---|---|
| Servicio `escuchar` en reposo (RssAnon / Rss) | 0,15 MB / 2,8 MB | 8,2 MB / 17,1 MB |
| Ventana, tras el recorrido (RssAnon / Rss, build de depuración) | 21 MB / 79 MB | 38,6 MB / 107 MB |

El servicio, que es lo que está siempre disponible, queda en un binario de 0,8 MB sin Qt ni intérprete.

## Qué falta

- Instalarlo: PKGBUILD y los archivos de `share/` (socket y servicio de systemd, `.desktop`, ícono) se copian
  de la app de Python cuando se decida reemplazarla. Hoy `vortex-shared.socket` sigue arrancando la de Python.
- `abrir-puerto` (pkexec + ufw) y el aviso real con `notify-send` solo se probaron con falsos.
