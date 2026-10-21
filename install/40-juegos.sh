# shellcheck shell=bash
if ((!VORTEX_JUEGOS)) || ! preguntar "¿Instalar lo de juegos (Steam, Proton CachyOS, gamemode, MangoHud, gamescope)?" s; then
  info "Juegos: omitido"
  return 0
fi

paso "Juegos"

mapfile -t pkgs < <(leer_paquetes "$VORTEX_DIR/packages/juegos.txt")
instalar_pacman "${pkgs[@]}"

# Java más reciente como predeterminado, y ZGC en los launchers de Minecraft que haya.
jdk="$(archlinux-java status 2>/dev/null | awk '/java-[0-9]+-openjdk/ {print $1}' | sort -V | tail -n1)"
[[ -n "$jdk" ]] && srun archlinux-java set "$jdk"
run "$VORTEX_DIR/bin/vortex" java aplicar

# gamemode necesita que el usuario esté en su grupo para subir prioridades.
srun usermod -aG gamemode "$USER"

# Al abrir un juego con gamemode, el escritorio apaga animaciones, desenfoque y sombras.
copiar_config "$VORTEX_DIR/config/gamemode.ini" "$HOME/.config/gamemode.ini"

if preguntar "¿Agregar también Heroic (Epic/GOG) y ProtonPlus?" n; then
  instalar_aur heroic-games-launcher-bin protonplus
fi
bien "Listo. En Steam: Propiedades → Opciones de lanzamiento → gamemoderun mangohud %command%"
