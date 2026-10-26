#!/usr/bin/env bash
# Saludo de la terminal de Vortexine's OS: el nombre en morado y, debajo, fastfetch con el logo de Vortex
# y los componentes de la PC. En kitty (la terminal de Vortex) el logo es la imagen; en otras, una espiral
# de texto. Lo llaman ~/.bashrc y la función fish_greeting (los dejan puestos install/usuario.sh y
# «vortex actualizar»). A mano: «vortex saludo».
aqui="$(dirname "$(realpath "${BASH_SOURCE[0]}")")"

printf '\e[1;38;2;167;139;250m'
cat <<'LETRERO'

  █   █            █            ▀           █         ▄▀▀▀▄ ▄▀▀▀▀
  ▀▄ ▄▀ ▄▀▀▄ █▄▀▀ ▀█▀ ▄██▄ ▀▄▄▀ █ █▀▀▄ ▄██▄   ▄█▀▀    █   █  ▀▀▀▄
   ▀▄▀  ▀▄▄▀ █     ▀▄ ▀▄▄▄ ▄▀▀▄ █ █  █ ▀▄▄▄   ▄▄█▀    ▀▄▄▄▀ ▄▄▄▄▀
LETRERO
printf '\e[0m'

command -v fastfetch >/dev/null || exit 0
if [[ -n "${KITTY_WINDOW_ID:-}" || "${TERM:-}" == xterm-kitty ]]; then
  exec fastfetch -c "$aqui/fastfetch.jsonc" --logo-type kitty --logo "$aqui/vortex-logo.png" \
    --logo-width 20 --logo-height 10
fi
exec fastfetch -c "$aqui/fastfetch.jsonc" --logo-type file --logo "$aqui/vortex-logo.txt"
