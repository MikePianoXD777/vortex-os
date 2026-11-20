#!/usr/bin/env bash
# Deja puesto el saludo de la terminal de Vortexine's OS (config/terminal/saludo.sh, «vortex saludo»):
#   - bash: dos líneas al final de ~/.bashrc;
#   - fish: la función fish_greeting (la de Caelestia, que pintaba «Caelestia», se cambia y queda
#     respaldada como fish_greeting.fish.caelestia; una tuya distinta se respeta).
#   - fish sobre CachyOS: su config.fish carga cachyos-config.fish, que define otro fish_greeting
#     (fastfetch con el logo de CachyOS) y tapa el de functions/. Ahí se agrega al final de config.fish
#     un fish_greeting que gana (respaldo en config.fish.antes-vortex).
# Una sola vez por usuario: si después lo quitas, no vuelve (marcas en ~/.config/vortex/saludo-puesto y
# saludo-cachyos-puesto; la segunda llega aparte para quien ya tenía la primera).
# Lo corren install/usuario.sh y «vortex actualizar».
set -u
conf="${XDG_CONFIG_HOME:-$HOME/.config}"
marca="$conf/vortex/saludo-puesto"
marca_cachyos="$conf/vortex/saludo-cachyos-puesto"
funcion="$conf/fish/functions/fish_greeting.fish"

# Solo si el saludo de Vortex sigue puesto en functions/ (si lo borraste, no se pone por otro lado).
arreglar_fish_cachyos() {
  local cf="$conf/fish/config.fish"
  [[ -e "$marca_cachyos" ]] && return 0
  if [[ -f "$cf" ]] && grep -q '^[^#]*cachyos-config\.fish' "$cf" && ! grep -q 'vortex saludo' "$cf" &&
    [[ -f "$funcion" ]] && grep -q 'vortex saludo' "$funcion"; then
    cp -f "$cf" "$cf.antes-vortex"
    cat >>"$cf" <<'FISH'

# Vortexine's OS: el saludo con el logo de Vortexine («vortex saludo»). El archivo de CachyOS de arriba
# define su propio fish_greeting (fastfetch con el logo de CachyOS) y tapa el de functions/; este gana.
# Para quitarlo, borra estas líneas.
function fish_greeting
    command -v vortex >/dev/null; and vortex saludo
end
FISH
  fi
  mkdir -p "${marca_cachyos%/*}" && : >"$marca_cachyos"
}

if [[ -e "$marca" ]]; then
  arreglar_fish_cachyos
  exit 0
fi

bashrc="$HOME/.bashrc"
if ! grep -qs 'vortex saludo' "$bashrc"; then
  printf '\n# Vortexine'\''s OS: el saludo de la terminal. Para quitarlo, borra estas dos líneas.\n%s\n' \
    '[[ $- == *i* ]] && command -v vortex >/dev/null && vortex saludo' >>"$bashrc"
fi

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
arreglar_fish_cachyos
