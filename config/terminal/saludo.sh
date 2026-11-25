#!/usr/bin/env bash
# Saludo de la terminal de Vortexine's OS: el nombre en blanco y, debajo, fastfetch con el logo de Vortex
# y los componentes de la PC. En kitty (la terminal de Vortex) el logo es la imagen; en otras, una espiral
# de texto. Lo llaman ~/.bashrc y la función fish_greeting (los dejan puestos install/usuario.sh y
# «vortex actualizar»). A mano: «vortex saludo».
aqui="$(dirname "$(realpath "${BASH_SOURCE[0]}")")"

# 4.3: primero la lluvia de dígitos (vortex-lluvia, ~1,3 s; cualquier tecla la salta). Solo se ve en una
# terminal de verdad y no deja rastro. «vortex saludo lluvia off» la apaga.
command -v vortex-lluvia >/dev/null && vortex-lluvia

printf '\e[1;38;2;255;255;255m'
cat <<'LETRERO'

  █   █            █            ▀           █         ▄▀▀▀▄ ▄▀▀▀▀
  ▀▄ ▄▀ ▄▀▀▄ █▄▀▀ ▀█▀ ▄██▄ ▀▄▄▀ █ █▀▀▄ ▄██▄   ▄█▀▀    █   █  ▀▀▀▄
   ▀▄▀  ▀▄▄▀ █     ▀▄ ▀▄▄▄ ▄▀▀▄ █ █  █ ▀▄▄▄   ▄▄█▀    ▀▄▄▄▀ ▄▄▄▄▀
LETRERO
printf '\e[0m'

command -v fastfetch >/dev/null || exit 0
# 4.4: con el filtro de daltonismo, la RAM y el disco llevan además una barra (lo lleno no se nota solo por el
# rojo del porcentaje).
extra=()
[[ -s "${XDG_CONFIG_HOME:-$HOME/.config}/vortex/daltonismo" ]] && extra=(--percent-type 11)
if [[ -n "${KITTY_WINDOW_ID:-}" || "${TERM:-}" == xterm-kitty ]]; then
  exec fastfetch -c "$aqui/fastfetch.jsonc" --logo-type kitty --logo "$aqui/vortex-logo.png" \
    --logo-width 20 "${extra[@]}"
fi
exec fastfetch -c "$aqui/fastfetch.jsonc" --logo-type file --logo "$aqui/vortex-logo.txt" "${extra[@]}"
