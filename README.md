# Vortex OS

Arch Linux ligero para **jugar y programar**: kernel y paquetes de CachyOS, Hyprland con el shell
[Caelestia](https://github.com/caelestia-dots/shell) afinado, y **Claude integrado** para que pueda ver
lo que haces (con tu permiso) y ayudarte.

> Estado: **0.1.0, en desarrollo**. El instalador corre completo en modo prueba; falta probarlo en una VM limpia.

## Instalar

Sobre un Arch recién instalado (con `archinstall`, perfil «minimal») o sobre CachyOS:

```bash
curl -fsSL https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/boot.sh | bash
```

O desde el código:

```bash
./install.sh --prueba     # muestra lo que haría, sin tocar nada
./install.sh              # instala
./install.sh --si --perfil ligero --sin-dev
```

Más adelante habrá una ISO propia (ver la hoja de ruta).

## Qué trae

| Parte | Qué usa | Por qué |
|---|---|---|
| Kernel | `linux-cachyos` (BORE) + `scx_lavd` | baja latencia en juegos; los repos se eligen solos según tu CPU (genérico, v3, v4 o znver4) |
| Ajustes | `cachyos-settings` + extras de Vortex | zram, sysctl y límites ya probados por CachyOS |
| Drivers | `chwd` | detecta NVIDIA, AMD, Intel e híbridas |
| Escritorio | Hyprland + Caelestia con perfiles | ver más abajo |
| Inicio de sesión | greetd + tuigreet | ~3 MB, frente a ~150 MB de SDDM |
| Juegos | Steam, Proton CachyOS, gamemode, MangoHud, gamescope | el modo juego apaga los efectos al abrir un juego |
| Programación | Zed, Neovim, mise, uv, GitHub CLI; Docker opcional | Docker arranca por socket: no consume nada hasta que lo usas |
| Claude | Claude Code + servidor MCP `vortex` | Super+A, y puede ver tu pantalla cuando se lo pides |

### Perfiles gráficos

```bash
vortex perfil ligero        # sin desenfoque, sombras ni animaciones de ventanas
vortex perfil equilibrado   # animaciones y desenfoque de 1 pasada, sin sombras
vortex perfil bonito        # Caelestia tal cual
```

El instalador elige uno según tu equipo (poca RAM o gráficos integrados → ligero).
Tus propios ajustes van en `~/.config/vortex/` y siempre ganan sobre el perfil:
`shell-mio.json` (Caelestia), `hypr-vars-mios.lua` (variables) e `hypr-mio.lua` (config de Hyprland).

### Claude en el sistema

`vortex-mcp` le da a Claude seis herramientas: ver la pantalla, la ventana activa, las ventanas abiertas,
el portapapeles, tus últimos comandos y el estado del sistema.

- Solo mira cuando usa una herramienta; no hay nada corriendo en segundo plano.
- Cada vez que ve la pantalla, el portapapeles o el historial, te sale una notificación.
- `vortex claude pausa` le corta todo; `vortex claude reanudar` lo devuelve.

## ¿Cuánto más ligero?

Los objetivos se miden con `vortex bench`, no se prometen a ojo:

```bash
vortex bench --guardar                       # en CachyOS, antes
vortex bench --guardar                       # en Vortex OS, después
vortex bench --comparar antes.json despues.json
```

Medida en la PC de Mike (CachyOS + Hyprland + Caelestia, 21/10/2026), como punto de partida:

| Medida | CachyOS actual | Meta Vortex |
|---|---|---|
| RAM del escritorio | 840 MB (de ellos, `qs`/Caelestia: 572 MB) | < 400 MB |
| Servicios del sistema activos | 23 | ≤ 12 |
| Paquetes | 1708 | < 900 |
| Arranque | 26 s | < 10 s |

**Sobre los FPS:** en un juego que depende de la GPU, ninguna distro da 5 veces más FPS que CachyOS,
porque CachyOS ya usa el mismo kernel y los mismos drivers. Lo que sí se gana es menos RAM, menos
procesos de fondo, *frametimes* más estables y equipos viejos que vuelven a ser usables.
La meta de «5x» se aplica a lo que el sistema gasta en reposo, no a los FPS.

## Hoja de ruta

- [x] **0.1** Instalador por pasos con modo prueba, perfiles, modo juego, `vortex-bench` y `vortex-mcp`
- [ ] Probar la instalación completa en una VM con Arch limpio (QEMU)
- [ ] **vortex-shell**: fork de Caelestia con **barra arriba** (hoy solo existe la barra vertical a la izquierda) y recortes para bajar la RAM de `qs`
- [ ] ISO propia con archiso que corra el instalador
- [ ] Subir el repo a GitHub y probar `boot.sh`

## Estructura

```
boot.sh              instalación con un comando (curl | bash)
install.sh           instalador; corre install/NN-*.sh en orden
install/             pasos: revisar, repos, sistema, escritorio, juegos, dev, claude, final
packages/            listas de paquetes por grupo
config/              perfiles, Caelestia, gamemode y ajustes del sistema
bin/vortex           comando principal
bin/vortex-bench     medidor de ligereza
bin/vortex-mcp       servidor MCP para Claude
```

## Licencia

MIT. Caelestia, Hyprland y CachyOS tienen sus propias licencias.
