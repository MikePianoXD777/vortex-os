# shellcheck shell=bash
if ((!VORTEX_DEV)) || ! preguntar "¿Instalar lo de programación (Zed, Neovim, mise, uv, GitHub CLI)?" s; then
  info "Programación: omitido"
  return 0
fi

paso "Programación"

mapfile -t pkgs < <(leer_paquetes "$VORTEX_DIR/packages/dev.txt" | grep -v '^docker')
instalar_pacman "${pkgs[@]}"

# mise activa la versión correcta de node/python/go… al entrar a cada proyecto.
# Se activa en ~/.bashrc (bash es el shell de las terminales y de Zed).
bashrc_usuario="$HOME/.bashrc"
if ! grep -qs 'mise activate' "$bashrc_usuario"; then
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    run "echo 'mise activate bash' >> $bashrc_usuario"
  else
    printf '\n# Vortex OS\ncommand -v mise >/dev/null && eval "$(mise activate bash)"\n' >>"$bashrc_usuario"
  fi
fi

if preguntar "¿Instalar Docker? (arranca solo cuando lo usas)" n; then
  mapfile -t docker < <(leer_paquetes "$VORTEX_DIR/packages/dev.txt" | grep '^docker')
  instalar_pacman "${docker[@]}"
  # Activación por socket: el demonio no consume nada hasta el primer «docker …».
  srun systemctl enable docker.socket
  srun usermod -aG docker "$USER"
  aviso "El grupo docker equivale a tener root. Si no te sirve, quítalo con: sudo gpasswd -d $USER docker"
fi
bien "Programación lista"
