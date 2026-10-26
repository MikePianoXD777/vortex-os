#!/usr/bin/env bash
# Deja puesto el saludo de la terminal de Vortexine's OS (config/terminal/saludo.sh, «vortex saludo»):
#   - bash: dos líneas al final de ~/.bashrc;
#   - fish: la función fish_greeting (la de Caelestia, que pintaba «Caelestia», se cambia y queda
#     respaldada como fish_greeting.fish.caelestia; una tuya distinta se respeta).
# Una sola vez por usuario: si después lo quitas, no vuelve (marca en ~/.config/vortex/saludo-puesto).
# Lo corren install/usuario.sh y «vortex actualizar».
set -u
conf="${XDG_CONFIG_HOME:-$HOME/.config}"
marca="$conf/vortex/saludo-puesto"
[[ -e "$marca" ]] && exit 0

bashrc="$HOME/.bashrc"
if ! grep -qs 'vortex saludo' "$bashrc"; then
  printf '\n# Vortexine'\''s OS: el saludo de la terminal. Para quitarlo, borra estas dos líneas.\n%s\n' \
    '[[ $- == *i* ]] && command -v vortex >/dev/null && vortex saludo' >>"$bashrc"
fi

funcion="$conf/fish/functions/fish_greeting.fish"
if [[ -d "$conf/fish" ]] || command -v fish >/dev/null; then
  if [[ -f "$funcion" ]] && grep -q '/ ____/___ ____' "$funcion"; then
    mv -f "$funcion" "$funcion.caelestia"
  fi
  if [[ ! -f "$funcion" ]]; then
    mkdir -p "${funcion%/*}"
    cat >"$funcion" <<'FISH'
function fish_greeting
    # Vortexine's OS: el saludo de la terminal («vortex saludo»). Para quitarlo, borra este archivo.
    command -v vortex >/dev/null; and vortex saludo
end
FISH
  fi
fi

mkdir -p "${marca%/*}" && : >"$marca"
