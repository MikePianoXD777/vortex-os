# shellcheck shell=bash
paso "Escritorio: Hyprland + Caelestia"

# Comandos de Vortex disponibles para todos (también para los atajos de Hyprland).
srun ln -sf "$VORTEX_DIR/bin/vortex" /usr/local/bin/vortex
srun ln -sf "$VORTEX_DIR/bin/vortex-bench" /usr/local/bin/vortex-bench
srun ln -sf "$VORTEX_DIR/bin/vortex-mcp" /usr/local/bin/vortex-mcp

# Audio con PipeWire antes que Caelestia: si otro paquete trae jack2 primero, choca con
# pipewire-jack y pacman no lo resuelve solo con --noconfirm.
if pacman -Qq jack2 >/dev/null 2>&1; then
  srun pacman -Rdd --noconfirm jack2
fi
instalar_pacman pipewire pipewire-pulse pipewire-alsa pipewire-jack wireplumber

# Caelestia pide quickshell-git. El repo de CachyOS trae noctalia-qs, un fork que dice
# «proveer» quickshell-git, y paru lo prefiere por venir de un repo; pero se compila contra
# otra versión de Qt y el shell no arranca. Por eso se pide el del AUR por nombre completo.
if pacman -Qq noctalia-qs >/dev/null 2>&1; then
  srun pacman -Rdd --noconfirm noctalia-qs
fi
instalar_aur aur/quickshell-git
instalar_aur caelestia-shell caelestia-cli

# El CLI de Caelestia instala Hyprland, foot, fish, PipeWire, NetworkManager, temas, etc.
# Se quita «micro» porque Vortex ya trae editores.
run caelestia install --aur-helper paru --disable-components micro --noconfirm
bien "Caelestia instalado"

run mkdir -p "$HOME/.config/vortex"
copiar_config "$VORTEX_DIR/config/caelestia/hypr-vars.lua" "$HOME/.config/caelestia/hypr-vars.lua"
copiar_config "$VORTEX_DIR/config/caelestia/hypr-user.lua" "$HOME/.config/caelestia/hypr-user.lua"
# «vortex perfil» reescribe shell.json; el que hubiera se guarda en el respaldo.
if [[ -f "$HOME/.config/caelestia/shell.json" && ! -f "$HOME/.config/vortex/shell-mio.json" ]]; then
  run mkdir -p "$VORTEX_RESPALDO/.config/caelestia"
  run cp -a "$HOME/.config/caelestia/shell.json" "$VORTEX_RESPALDO/.config/caelestia/shell.json"
fi
run "$VORTEX_DIR/bin/vortex" perfil "$VORTEX_PERFIL" --sin-recargar
bien "Perfil «$VORTEX_PERFIL» aplicado (cámbialo con: vortex perfil ligero|equilibrado|bonito)"

if es_laptop; then
  srun systemctl enable power-profiles-daemon.service
fi
