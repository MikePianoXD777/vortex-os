# Vortex OS

Arch Linux ligero para **jugar y programar**: kernel y paquetes de CachyOS, Hyprland con el shell
[Serpantinum](https://github.com/ilyamiro/serpantinum) (barra arriba, paneles animados, colores que salen
de tu fondo) optimizado, y **Claude integrado** para que pueda ver lo que haces (con tu permiso) y ayudarte.

> Estado: **0.2 en desarrollo**. En una VM (QEMU/KVM, UEFI, Arch limpio) el instalador corre completo, arranca
> con `linux-cachyos` y `scx_lavd` en ~3 s, y el shell funciona en español con 0 avisos en su registro.
> Falta: terminar de optimizar la RAM, probar en hardware real y construir la ISO.

## Instalar

Sobre un Arch recién instalado (con `archinstall`, perfil «minimal») o sobre tu CachyOS:

```bash
curl -fsSL https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/boot.sh | bash
```

O desde el código:

```bash
./install.sh --prueba     # muestra lo que haría, sin tocar nada
./install.sh              # instala
./install.sh --si --sin-dev --inicio-automatico
```

**No pierdes nada.** Vortex se instala encima de tu sistema, sin formatear: tus juegos de Steam, Minecraft
y sus mods, tus sesiones y tus archivos se quedan donde están. Si tu disco usa btrfs, antes de cambiar nada
el instalador toma una foto del sistema (snapper) y una copia instantánea de `/home`, para volver atrás si
hiciera falta.

O con la ISO de Vortex: arranca, escribe `vortex-instalar`, y `archinstall` te guía. Al reiniciar
y entrar con tu usuario, Vortex se termina de instalar solo.

```bash
sudo iso/construir.sh            # construye la ISO en iso/out/ (necesita archiso)
```

## Qué trae

| Parte | Qué usa | Por qué |
|---|---|---|
| Kernel | `linux-cachyos` (BORE) + `scx_lavd` | baja latencia en juegos; los repos se eligen solos según tu CPU (genérico, v3, v4 o znver4) |
| Ajustes | `cachyos-settings` + extras de Vortex | zram, sysctl y límites ya probados por CachyOS; sin `ananicy-cpp` (sobra con `scx_lavd`) |
| Drivers | `chwd` | detecta NVIDIA, AMD, Intel e híbridas |
| Escritorio | Hyprland + Serpantinum (`shell/`) | barra arriba, calendario y clima, panel de red, centro de control, música con ecualizador, carrusel de fondos, app de Ajustes |
| Colores | matugen | toda la interfaz (y kitty) toma los colores del fondo que pongas |
| Inicio de sesión | greetd + tuigreet | ~3 MB, frente a ~150 MB de SDDM |
| Juegos | Steam, Proton CachyOS, gamemode, MangoHud, gamescope, **Java 27** | modo juego automático; `vortex java aplicar` pone ZGC en tus launchers de Minecraft |
| Programación | Zed, Neovim, mise, uv, GitHub CLI; Docker opcional | Docker arranca por socket: no consume nada hasta que lo usas |
| Claude | Claude Code + servidor MCP `vortex` | Super+A, y puede ver tu pantalla cuando se lo pides |

Todo se personaliza desde la app de **Ajustes** del shell: la barra (posición, estilo, módulos), el fondo,
el tema, la inactividad... En una PC de escritorio Vortex **nunca suspende** (solo bloquea y apaga la
pantalla); en laptops sí.

### Atajos de Vortex

| Atajo | Qué hace |
|---|---|
| Super+A | habla con Claude |
| Super+G | modo juego a mano |
| Super+Shift+A | esconder/mostrar la barra |
| Super+E | archivos (Thunar) |
| Super+S / N / B / M / W / D / H | calendario, red, sistema, música, fondos, lanzador, Ajustes |

### Claude en el sistema

`vortex-mcp` le da a Claude seis herramientas: ver la pantalla, la ventana activa, las ventanas abiertas,
el portapapeles, tus últimos comandos y el estado del sistema.

- Solo mira cuando usa una herramienta; no hay nada corriendo en segundo plano.
- Cada vez que ve la pantalla, las ventanas, el portapapeles o el historial, te sale una notificación.
- `vortex claude pausa` le corta todo; `vortex claude reanudar` lo devuelve.

## ¿Cuánto más ligero?

Los objetivos se miden con `vortex bench`, no se prometen a ojo. En la VM se mide con un arranque en frío por
variante (`vm/medir-frio.sh`), porque reiniciar solo el shell deja memoria acumulada y sesga los resultados.

```bash
vortex bench --guardar                       # en tu sistema actual, antes
vortex bench --guardar                       # en Vortex OS, después
vortex bench --comparar antes.json despues.json
```

La meta es **hasta 45 % menos RAM que un Linux convencional** (CachyOS + Hyprland + Caelestia). Las cifras
definitivas se publican cuando termine la optimización del shell.

**Sobre los FPS:** en un juego que depende de la GPU ninguna distro multiplica los FPS frente a CachyOS,
porque usa el mismo kernel y los mismos drivers. Lo que se gana es menos RAM y procesos de fondo,
*frametimes* más estables y equipos viejos que vuelven a ser usables.

## Hoja de ruta

- [x] **0.1** Instalador por pasos, modo juego, `vortex-bench`, `vortex-mcp`, VM de pruebas
- [x] Instalación completa probada en una VM con Arch limpio
- [x] Shell nuevo: Serpantinum en `shell/`, sin telemetría, en español, 0 avisos en su registro
- [ ] Optimizar la RAM del shell sin cambiar cómo se ve (en curso)
- [ ] Probar en hardware real (BIOS, NVIDIA/AMD/Intel, híbridas, laptops, CPUs sin x86-64-v3)
- [ ] Construir y probar la ISO
- [ ] Publicar la versión estable

## Estructura

```
boot.sh              instalación con un comando (curl | bash)
install.sh           instalador; corre install/NN-*.sh en orden
install/             pasos: revisar, restauración, repos, sistema, escritorio, juegos, dev, claude, final
packages/            listas de paquetes por grupo
shell/               Serpantinum (git subtree del tag v2.2.14) con los parches de Vortex
packaging/           PKGBUILD del shell (vortex-shell)
config/              Hyprland, ajustes iniciales del shell, fondos, gamemode y ajustes del sistema
bin/vortex           comando principal (fondo, java, modo-juego, claude, bench, actualizar)
bin/vortex-bench     medidor de ligereza
bin/vortex-mcp       servidor MCP para Claude
iso/                 perfil de archiso (releng + Vortex) y «vortex-instalar»
vm/                  VM de pruebas con QEMU, mediciones y previews
```

## Probar en una máquina virtual

```bash
vm/vm.sh live                         # arranca la ISO de Arch (en ~/.cache/vortex-os-vm/iso)
vm/vm.sh ssh 'bash -s' < vm/instalar-arch.sh   # Arch mínimo en el disco virtual
vm/vm.sh apagar && vm/vm.sh disco     # arranca lo instalado
VORTEX_VM_USUARIO=vortex vm/vm.sh ssh # entra como el usuario de prueba
vm/preview.sh foto escritorio         # capturas y clips para mostrar Vortex
```

## Licencia

MIT, salvo `shell/`, que es Serpantinum de [@ilyamiro](https://github.com/ilyamiro) con licencia
**AGPL-3.0** (sus cambios en Vortex siguen bajo la misma licencia). Hyprland y CachyOS tienen las suyas.
