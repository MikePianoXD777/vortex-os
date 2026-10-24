# Vortexine's OS

Parte de **Vortex Suite**: las funciones de los grandes, pero fáciles de usar.
La capacidad sí; la complejidad no.

Arch Linux ligero para **jugar y programar**: kernel y paquetes de CachyOS, Hyprland con el shell
[Serpantinum](https://github.com/ilyamiro/serpantinum) (barra arriba, paneles animados, colores que salen
de tu fondo) optimizado, y **Claude integrado** para que pueda ver lo que haces (con tu permiso) y ayudarte.

> Estado: **0.2 en desarrollo**. En una VM (QEMU/KVM, UEFI, Arch limpio) el instalador corre completo, arranca
> con `linux-cachyos` y `scx_lavd` en ~3 s, y el shell funciona en español con 0 avisos en su registro.
> La ISO con instalador gráfico está en pruebas. Falta probar en hardware real.

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

O con la **ISO de Vortex** (en pruebas): arranca en el escritorio de Vortex para probarlo, y el
instalador gráfico («Instalar Vortexine's OS», se abre solo) te guía: idioma, teclado, disco y usuario, y
eliges tus apps y juegos con casillas (Steam, Sober, Prism Launcher, el lanzador de Genshin, Heroic, Zoom,
Discord, OBS, Vortex Studio...). Se instala sin internet; las apps sí la necesitan. «Claude» (el
asistente de Super+A) viene **apagado**: si lo marcas, se le instala Claude Code a cada usuario en su
primer inicio con internet; si no, se activa cuando quieras en Ajustes → General → Claude. Y en la pantalla de usuario hay una casilla para entrar al escritorio sin
pedir la contraseña.

La ISO arranca e instala en PC con **UEFI** (gestor de arranque systemd-boot) y con **BIOS/legacy**
(GRUB, que además pone a Windows en el menú). Junto a Windows, la partición EFI que ya existe a veces
es chica (100-260 MB) y también hace de `/boot`: con poco espacio libre Vortex deja solo el kernel
`linux-cachyos` (sin el de respaldo de Arch); con menos de ~150 MB libres, elige el particionado manual
y crea una partición EFI nueva de 500 MB o más. Si los drivers de la GPU no se pudieron bajar durante
la instalación (sin internet), el sistema los termina de instalar solo en cuanto haya red y avisa.

El teclado que eliges en el instalador se aplica también al escritorio del live, para que la contraseña
y la frase de cifrado se escriban con el teclado que vas a usar (el instalador abre con **latam** si el
idioma es el español de Latinoamérica), y el shell queda en el idioma del sistema que elegiste.

> **Secure Boot, sin tocar la BIOS:** la ISO arranca con shim (firmado por Microsoft, como Fedora y
> Ubuntu). La primera vez que arranca el USB con Secure Boot activo sale **MokManager** (pantalla
> azul): «Enroll key from disk» → ARCHISO_EFI → EFI → `vortexines.cer` → Continue → Yes → Reboot.
> Al instalar, cada PC recibe su propia llave (`/etc/vortex/secureboot`, solo root) con la que se
> firman systemd-boot y los kernels, también en cada actualización (hook de pacman). En el primer
> arranque, MokManager pide aceptarla una vez: Enroll MOK → Continue → Yes → `1234` → Reboot. Estado:
> `sudo vortex-firmar estado`. Si activas Secure Boot después de instalar: `sudo vortex-firmar inscribir`.
>
> **Junto a Windows (dual boot):** la guía paso a paso para quien nunca ha instalado Linux está en
> [docs/guia-dual-boot.md](docs/guia-dual-boot.md). Desde Vortexine's, «Reiniciar en Windows» (o
> `vortex windows`) entra una vez a Windows.

```bash
iso/paquetes.sh                  # como usuario: arma el repo local [vortex] (shell, Vortex Studio...)
sudo iso/construir.sh            # construye la ISO en iso/out/ (necesita archiso y el repo de CachyOS)
```

`iso/paquetes.sh` no vuelve a compilar los paquetes del AUR (Calamares tarda ~10 min) si el repo ya tiene esa
misma versión; `VORTEX_PAQUETES_FORZAR=1` los compila de todos modos. Todo se compila para un x86-64
genérico (no para tu CPU, aunque tu `makepkg.conf` diga `-march=native`, como el de CachyOS): la ISO tiene que
arrancar también en equipos sin AVX2. Lo que trae binarios (Calamares, wl-gammarelay-rs, Vortex Studio) se
compila en un chroot limpio de Arch (necesita `devtools` y sudo; vive en `/var/tmp/vortex-chroot`), porque el
glibc «v3» de CachyOS marca todo lo que se enlaza con él como «necesita x86-64-v3». Al final, y otra vez antes
de construir la ISO, se revisa que ningún programa del repo pida más que x86-64 básico (`readelf -n`). Las
piezas de la ISO se prueban sin construirla: `pruebas/iso.sh`.

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
| Recursos | `vortex-gestor` (servicio de usuario, ~2 MB de RAM) | reparte solo la CPU y la RAM: prioridad a la ventana enfocada, modo juego al abrir un juego y memoria liberada cuando falta |
| Programación | Zed, Neovim, mise, uv, GitHub CLI; Docker opcional | Docker arranca por socket: no consume nada hasta que lo usas |
| Claude | Claude Code + servidor MCP `vortex` | viene apagado (Ajustes → General → Claude); Super+A, le hablas con el micrófono y ve tu pantalla cuando se lo pides |
| Pantallas | `vortex-pantallas` | cada monitor a su resolución máxima y su frecuencia más alta (144, 165, 200, 240 Hz), al entrar y al conectarlo; respeta lo que pongas en Ajustes → Pantalla o en `hypr-mio.lua` |
| Tienda de apps | **Vortexine's Shop** (`vortex-shop`) | todo Flathub a un clic, sin terminal ni contraseñas; las apps no vienen en la ISO |
| Asistente de voz | Miku (`vortex-miku`): modelo 3D VRM + Claude Sonnet + voz y oído locales | Super+O; viene instalada y encendida (necesita Claude activado) y se apaga del todo al cerrarla |

Todo se personaliza desde la app de **Ajustes** del shell: la barra (posición, estilo, módulos), el fondo,
el tema, la inactividad... En una PC de escritorio Vortex **nunca suspende** (solo bloquea y apaga la
pantalla); en laptops sí.

### Vortexine's Shop

La tienda de aplicaciones (Super+I, o «Vortexine's Shop» en el lanzador). Muestra **todo lo que hay en
[Flathub](https://flathub.org)** —juegos, navegadores, chat, oficina, edición— con búsqueda, categorías,
fichas con capturas y una lista de recomendadas por Vortexine's (Steam, Prism Launcher, Sober, Heroic,
Discord, Firefox, OBS...).

- **Sin terminal ni contraseñas:** instala con Flatpak **para tu usuario** (`flatpak --user`), así que no pide
  `sudo` ni polkit; ve el progreso real de la descarga y puede quitar y actualizar lo que instaló. Las apps
  que ya vienen con el sistema (las que se eligieron en el instalador) aparecen como instaladas, pero solo
  un administrador las quita.
- **No ocupan lugar en la ISO:** la ISO solo trae la tienda (`vortex-shop` y PySide6, unos 55 MB en total);
  cada app se descarga de Flathub cuando la eliges. Flathub se agrega solo la primera vez.
- Todo viene de la API pública de Flathub y se guarda en `~/.cache/vortexines-shop/` (listas 1 h, fichas
  24 h): sin internet muestra lo último que vio.
- Su código está en `apps/shop/` (motor en Python, interfaz en QML) y su paquete en `packaging/vortex-shop`;
  las pruebas, que no instalan nada, son `pruebas/tienda.sh`.

### Atajos de teclado

Son los mismos que usaba el autor en CachyOS con Caelestia, con cada acción llevada a su equivalente en
Serpantinum. Viven en `~/.config/hypr/atajos.lua` (junto a `hyprland.lua`); lo tuyo va en
`~/.config/vortex/hypr-mio.lua`, que se carga al final. Los atajos de Serpantinum que chocaban con estos se
quitaron (Super+F ya no abre Firefox: ahora es pantalla completa y Firefox es Super+W); los demás siguen igual.

**Shell y paneles**

| Atajo | Qué hace |
|---|---|
| Super (solo) / Super+Espacio | lanzador |
| Super+N | notificaciones |
| Super+B / Ctrl+Alt+Supr | panel de sistema: bloquear, dormir, hibernar, reiniciar, apagar, cerrar sesión |
| Super+H | Ajustes |
| Super+K | calendario, reloj y clima |
| Super+. | selector de emojis (la pestaña Emojis del lanzador: al elegir uno se copia). El lanzador recuerda la última pestaña: Super abrirá en Emojis hasta que vuelvas a Aplicaciones con Tab |
| Super+V / Super+Alt+V | historial del portapapeles (Supr borra la entrada elegida) |
| Ctrl+Alt+V | volumen y dispositivos de audio |
| Super+Shift+N / Super+Shift+W / Super+Alt+M | red / fondos / música (paneles que cedieron su atajo de siempre) |
| Super+Shift+A | esconder/mostrar la barra |
| Ctrl+Alt+C | borrar todas las notificaciones |
| Super+L | bloquear |
| Super+Shift+L | dormir: bloquea y suspende (con hibernación diferida si el equipo puede) |
| Super+Alt+L | recuperar el bloqueo si el shell murió con la pantalla bloqueada |
| Ctrl+Super+Shift+R / Ctrl+Super+Alt+R | cerrar el shell / reiniciarlo |

**Apps**

| Atajo | Qué hace |
|---|---|
| Super+T (o Super+Enter) | terminal (kitty) |
| Super+W | Firefox |
| Super+E | archivos (Thunar) |
| Super+C | editor (Zed) |
| Super+A | habla con Claude (si lo activaste) |
| Super+I | Vortexine's Shop: la tienda de apps |
| Super+O | Miku, el asistente de voz: la abre (y se pone a escuchar) o la cierra del todo (necesita Claude activado) |
| Super+G | modo juego a mano |

**Escritorios**

| Atajo | Qué hace |
|---|---|
| Super+1…0 | ir al escritorio 1 a 10 (dentro de su grupo de 10) |
| Super+Alt+1…0 (o Super+Shift+1…0) | mandar la ventana a ese escritorio (también dentro de su grupo de 10) |
| Ctrl+Super+←/→, Super+Re Pág/Av Pág, Super+rueda | escritorio anterior/siguiente |
| Ctrl+Super+1…0 | saltar al mismo escritorio de otro grupo de 10 |
| Ctrl+Super+Alt+1…0 | mandar la ventana a ese grupo |
| Ctrl+Super+rueda | grupo anterior/siguiente |
| Ctrl+Super+Shift+←/→, Super+Alt+Re Pág/Av Pág, Super+Alt+rueda | mandar la ventana al escritorio anterior/siguiente |
| Super+S | escritorio especial (cajón de ventanas escondidas) |
| Super+Alt+S o Ctrl+Super+Shift+↑ / Ctrl+Super+Shift+↓ | mandar la ventana al especial / sacarla |
| Super+M / Super+D / Super+R | especial de música (Spotify) / chat (Discord) / tareas (Todoist): abre la app si hace falta |
| Ctrl+Shift+Esc | monitor del sistema (btop en un especial) |

**Ventanas**

| Atajo | Qué hace |
|---|---|
| Super+flechas / Super+Shift+flechas | enfocar / mover la ventana |
| Ctrl+Super+↑ / Ctrl+Super+↓ | mover la ventana hacia arriba / abajo (de Serpantinum; Ctrl+Super+←/→ cambian de escritorio) |
| Super+- / Super+= | achicar / agrandar el ancho |
| Super+Shift+- / Super+Shift+= | achicar / agrandar el alto |
| Super+Alt+flechas | achicar y agrandar con las flechas |
| Super+clic izq. o der., Super+Z / Super+X | mover / redimensionar con el ratón |
| Super+Q | cerrar |
| Super+F / Super+Alt+F | pantalla completa / maximizar |
| Super+Alt+Espacio | flotante o en mosaico |
| Super+P | fijar en todos los escritorios |
| `Super+Alt+\` | imagen en imagen (flotante, chica, en la esquina) |
| `Ctrl+Super+\` / `Ctrl+Super+Alt+\` | centrar / tamaño normal (55 × 70 %) y centrar |
| Alt+Tab (con Shift, al revés) | ciclar ventanas |
| Ctrl+Alt+Tab | ciclar los grupos |
| Super+, / Super+U / Super+Shift+, | agrupar en pestañas / sacar del grupo / bloquear el grupo |

**Capturas, grabación y sonido**

| Atajo | Qué hace |
|---|---|
| Impr / Super+Shift+S | captura de una región (se copia y se guarda) |
| Super+Impr / Alt+Impr | captura de toda la pantalla / de la ventana activa |
| Shift+Impr / Super+Shift+Impr | región / pantalla completa, abriendo el editor de capturas |
| Ctrl+Alt+R / Super+Alt+R | grabar la pantalla sin sonido / con el sonido del equipo (otra vez para parar) |
| Super+Shift+Alt+R | grabar una región |
| Super+Shift+C | gotero de colores |
| Teclas de volumen | subir/bajar 10 % (quitan el silencio); Super+Shift+M o la tecla de silencio lo alterna |
| Ctrl+Super+Espacio / = / - / Retroceso | reproducir-pausar / siguiente / anterior / parar |
| Ctrl+Shift+Alt+V | escribir lo último del portapapeles (necesita `ydotool` y su servicio) |
| Super+Alt+F12 | notificación de prueba |

Lo que Caelestia tenía y Vortex no: el «mostrar todos los paneles» (Super+K abre el calendario, que es lo
más parecido).

Las pruebas de los atajos corren sin VM ni sesión de Hyprland, en una carpeta temporal: `pruebas/atajos.sh`
(`Hyprland --verify-config`, la lógica de cada atajo con un Hyprland falso, los comandos de shell ejecutados de
verdad y el respaldo si `atajos.lua` falta o falla). Falta probarlos en una sesión real (VM).

### Gestor de recursos

`vortex-gestor` arranca con la sesión y no hace nada mientras no pase algo (escucha a Hyprland y al
kernel; no hay bucles que despierten la CPU):

- **Ventana enfocada primero:** su app va a su propio scope de systemd con más peso de CPU y disco, y
  las demás bajan. Con `scx_lavd` (que no usa esos pesos) además sube un poco su prioridad (nice) cuando
  el sistema deja devolverla después (usuarios del grupo `gamemode`, que se agrega con lo de juegos).
- **Modo juego solo:** al abrir un juego (Steam, gamescope, Wine, Minecraft o una ventana que dice ser
  un juego) apaga los efectos y pausa el fondo animado; al cerrarlo lo deja todo como estaba.
- **Poca memoria:** cuando el kernel avisa presión de memoria, comprime lo que no usan las apps de fondo
  (nunca la enfocada ni el juego) para que lo de delante no dé tirones.

`vortex gestor estado` muestra qué está haciendo; `vortex gestor apagar` / `encender` lo controlan.
Las pruebas corren sin VM: `pruebas/gestor.sh`.

### Modo otaku (easter egg)

Un interruptor en **Ajustes > Tema** (o `vortex otaku on|off|estado|otro`) que pone de fondo a Miku
Nakano («Gotoubun no Hanayome») y le da un toque suave al shell (fuente y bordes más redondeados);
al apagarlo vuelve el fondo y los ajustes de antes.

**Este repo no lleva imágenes del personaje.** Al encender, `vortex-otaku` baja unos fondos de la API
pública de [Wallhaven](https://wallhaven.cc) (solo SFW) a `Imágenes/Fondos/otaku/`, guarda de dónde
salió cada uno en `fuentes.json` y los borra al apagar; los derechos son de sus autores. Sin red usa un
fondo original de Vortex (audífonos estilizados, `config/fondos/otaku/generar.py`).
Las pruebas corren sin red ni shell: `pruebas/otaku.sh` (el comando) y `pruebas/otaku-qml.sh` (el
interruptor, con el quickshell de verdad sin pantalla).

**En un sistema ya instalado:** `vortex actualizar` copia los comandos de `bin/` a `/usr/local/bin`
(con sudo) para que el shell y lo que llama se actualicen juntos. Como el `vortex` anterior no hace
eso, la primera vez hay que correr, desde la carpeta de Vortex,
`sudo install -m755 bin/vortex bin/vortex-otaku /usr/local/bin/` y luego `vortex actualizar --forzar`
(para que llegue el shell con el interruptor).

### Claude en el sistema

`vortex-mcp` le da a Claude seis herramientas: ver la pantalla, la ventana activa, las ventanas abiertas,
el portapapeles, tus últimos comandos y el estado del sistema.

- Solo mira cuando usa una herramienta; no hay nada corriendo en segundo plano.
- Cada vez que ve la pantalla, las ventanas, el portapapeles o el historial, te sale una notificación.
- `vortex claude pausa` le corta todo; `vortex claude reanudar` lo devuelve.

**Hablarle con el micrófono:** en la ventana de Claude (Super+A), toca `Espacio`, habla y toca `Espacio` otra
vez para enviar. Es el dictado de Claude Code (`/voice`), ya activado en español; necesita tu cuenta de
Claude.ai, la voz se transcribe en los servidores de Anthropic y no gasta tokens. Para cambiar el modo:
`/voice hold` (mantener Espacio) o `/voice off`.

**Activar Claude:** viene apagado (Super+A solo avisa cómo activarlo). Se activa en Ajustes → General →
Claude o con `vortex claude activar` (y se apaga con `vortex claude desactivar`); necesita tu cuenta de
Claude, y la primera vez que lo abres te pide iniciar sesión. `install.sh` pregunta en el paso de Claude
(por defecto no). Con la ISO es una casilla del instalador, desmarcada
(paquete `vortex-claude`, que deja la marca `/etc/vortex/claude`): en el primer inicio de cada usuario,
`vortex-sesion` corre `vortex claude instalar --al-entrar` en segundo plano; espera a que haya red (si no
hay, lo reintenta en el próximo inicio) y al terminar avisa con una notificación. A mano:
`vortex claude instalar`.

### Miku, el asistente de voz

Viene instalada y encendida en la ISO (paquete `vortex-miku`; `install.sh` la pregunta, y `--sin-miku` la
omite). **Necesita Claude activado** (Ajustes → General → Claude), porque Claude es su cerebro: con Claude
apagado, ella te lo dice. **Super+O** la abre —sale pegada al borde derecho, debajo de la barra, y se pone a escuchar— y
**Super+O otra vez la cierra del todo**: sin ventana no queda ningún proceso (ni la voz, ni el oído, ni
Claude), así que no gasta RAM cuando no la usas.

- **Qué es:** Miku Nakano en 3D (un modelo VRM) con **Claude Sonnet** de cerebro, que puede ver tu pantalla,
  y una voz de IA genérica (no imita a ninguna persona). El oído (whisper.cpp) y la voz (Supertonic 3)
  corren en tu PC; lo único que sale es lo que ella piensa, que pasa por Claude con tu cuenta (como Super+A).
  Los modelos de voz y de oído **no vienen en la ISO**: se bajan la primera vez que la abres (necesita
  internet esa vez) y quedan en tu usuario.
- **Solo voz:** en **Ajustes → General → Miku en 3D** (o `vortex miku avatar off`) se apaga el 3D y queda un
  panel chico de asistente de voz, que gasta bastante menos RAM. `vortex miku avatar on` lo vuelve a encender.
  (En un sistema ya instalado, el interruptor y `vortex miku` llegan con `vortex actualizar --forzar`.)
- **Privacidad:** el micrófono solo escucha mientras su ventana está abierta, y el audio no se guarda. La
  pantalla solo la ve cuando se lo pides, y cada vez te sale una notificación (`vortex claude pausa` le corta
  el acceso). Sin Claude instalado o sin sesión la voz sigue funcionando: ella te dice qué falta.
- **Comandos:** `vortex miku` abre o cierra, `vortex miku estado` dice si el 3D está encendido y si está
  abierta, `vortex miku voz [ID]` lista o elige la voz y `vortex miku modelo RUTA.vrm` pone otro modelo 3D.
  Las opciones quedan en `~/.config/vortex/miku.json`.
- **El modelo 3D no viene en este repo ni en el paquete** (es del autor del personaje y su licencia pide
  crédito). La ISO lo toma al construirse de la carpeta `VORTEX_MIKU_MODELO` (con `modelo.vrm` y
  `CREDITO.txt`; si no se la das, usa `~/.local/share/vortex/miku` de quien corre `sudo`) y lo deja en
  `/usr/share/vortex-miku/modelo/`; sin modelo la ISO sale igual y Miku funciona como solo voz. `install.sh`
  tampoco lo trae: la app muestra cómo ponerlo. La ventana de Miku lleva el crédito a la vista.

Las pruebas corren sin VM ni pantalla: `pruebas/miku.sh` (el comando, la ISO y el instalador),
`pruebas/miku-qml.sh` (el interruptor de Ajustes, con el quickshell de verdad) y `pruebas/atajos.sh` (Super+O y
las reglas de su ventana). Falta probar la ventana transparente sobre una sesión real de Hyprland.

## ¿Cuánto más ligero?

Los objetivos se miden con `vortex bench`, no se prometen a ojo. En la VM se mide con un arranque en frío por
variante (`vm/medir-frio.sh`), porque reiniciar solo el shell deja memoria acumulada y sesga los resultados.

```bash
vortex bench --guardar                       # en tu sistema actual, antes
vortex bench --guardar                       # en Vortexine's OS, después
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
- [x] Optimizar el shell sin cambiar cómo se ve (−36 % de RAM y −82 % de CPU en reposo frente a Caelestia)
- [ ] Probar en hardware real (BIOS, NVIDIA/AMD/Intel, híbridas, laptops, CPUs sin x86-64-v3)
- [ ] ISO con instalador gráfico (Calamares) y apps para elegir (en pruebas)
- [ ] Publicar la versión estable

## Estructura

```
boot.sh              instalación con un comando (curl | bash)
install.sh           instalador; corre install/NN-*.sh en orden
install/             pasos: revisar, restauración, repos, sistema, escritorio, juegos, dev, claude, final
packages/            listas de paquetes por grupo
shell/               Serpantinum (git subtree del tag v2.2.14) con los parches de Vortex
packaging/           PKGBUILD del shell (vortex-shell) y de la tienda (vortex-shop)
apps/shop/           Vortexine's Shop: la tienda de apps de Flathub (motor Python + interfaz QML); sus pruebas: pruebas/tienda.sh
config/              Hyprland, ajustes iniciales del shell, fondos, gamemode y ajustes del sistema
bin/vortex           comando principal (fondo, java, modo-juego, gestor, claude, bench, actualizar)
bin/vortex-gestor    gestor de recursos automático (con src/vortex-eventos.c, su ayudante en C)
bin/vortex-bench     medidor de ligereza
bin/vortex-otaku     modo otaku (easter egg): fondo de Wallhaven + toque de personalización
bin/vortex-mcp       servidor MCP para Claude
apps/miku/           Miku, el asistente de voz (app de Qt con el modelo 3D; la empaqueta packaging/vortex-miku)
iso/                 la ISO: perfil de archiso, instalador gráfico (Calamares) y repo local [vortex]; sus pruebas: pruebas/iso.sh
vm/                  VM de pruebas con QEMU, mediciones y previews
pruebas/             pruebas que corren sin VM (gestor de recursos, atajos, modo otaku y Miku)
```

## Probar en una máquina virtual

```bash
vm/vm.sh live                         # arranca la ISO de Arch (en ~/.cache/vortex-os-vm/iso)
vm/vm.sh ssh 'bash -s' < vm/instalar-arch.sh   # Arch mínimo en el disco virtual
vm/vm.sh apagar && vm/vm.sh disco     # arranca lo instalado
VORTEX_VM_USUARIO=vortex vm/vm.sh ssh # entra como el usuario de prueba
vm/preview.sh foto escritorio         # capturas y clips para mostrar Vortex
```

## Créditos del asistente Miku

- **Modelo 3D:** «MIKU NAKANO» por **Vewertyg**, en [VRoid Hub](https://hub.vroid.com/en/characters/7619526108943069134/models/40799402200114173).
  Condiciones del autor (van dentro del VRM): avatar para cualquiera, se permite modificar y redistribuir,
  **crédito obligatorio, sin uso comercial** (personal ni corporativo) **ni expresión sexual**
  ([licencia](https://hub.vroid.com/license?allowed_to_use_user=everyone&characterization_allowed_user=everyone&corporate_commercial_use=disallow&credit=necessary&modification=allow&personal_commercial_use=disallow&redistribution=allow&sexual_expression=disallow&version=1&violent_expression=allow)).
  Miku Nakano es un personaje de «Gotoubun no Hanayome»; este repo no incluye el modelo ni imágenes suyas.
- **3D en pantalla:** [three.js](https://threejs.org) 0.180.0 y [@pixiv/three-vrm](https://github.com/pixiv/three-vrm)
  3.5.5 (MIT), dentro de Qt WebEngine.
- **Voz:** [Supertonic 3](https://github.com/supertone-inc/supertonic) de Supertone (modelo con licencia
  OpenRAIL-M, que permite su uso y redistribución con sus restricciones de uso), con ONNX Runtime (MIT). Es
  una voz de IA genérica: no clona ni imita a ninguna persona. Se baja la primera vez que se usa.
- **Oído:** [Whisper](https://github.com/openai/whisper) de OpenAI (MIT) con
  [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (MIT). El modelo se baja la primera vez que se usa.
- **Cerebro:** Claude, de Anthropic, con tu cuenta.

## Licencia

MIT, salvo `shell/`, que es Serpantinum de [@ilyamiro](https://github.com/ilyamiro) con licencia
**AGPL-3.0** (sus cambios en Vortex siguen bajo la misma licencia). Hyprland y CachyOS tienen las suyas.
