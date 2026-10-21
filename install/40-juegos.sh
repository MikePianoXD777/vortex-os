# shellcheck shell=bash
if ((!VORTEX_JUEGOS)) || ! preguntar "¿Instalar lo de juegos (Steam, Proton CachyOS, gamemode, MangoHud, gamescope)?" s; then
  info "Juegos: omitido"
  return 0
fi

paso "Juegos"

mapfile -t pkgs < <(leer_paquetes "$VORTEX_DIR/packages/juegos.txt")
instalar_pacman "${pkgs[@]}"

# gamemode necesita que el usuario esté en su grupo para subir prioridades.
srun usermod -aG gamemode "$USER"

# Al abrir un juego con gamemode, el escritorio apaga animaciones, desenfoque y sombras.
copiar_config "$VORTEX_DIR/config/gamemode.ini" "$HOME/.config/gamemode.ini"

if preguntar "¿Agregar también Heroic (Epic/GOG) y ProtonPlus?" n; then
  instalar_aur heroic-games-launcher-bin protonplus
fi
bien "Listo. En Steam: Propiedades → Opciones de lanzamiento → gamemoderun mangohud %command%"
