#!/usr/bin/env bash
# Pruebas de los atajos de teclado (config/hypr/hyprland.lua y atajos.lua), sin tocar tu Hyprland:
#   1. «Hyprland --verify-config» sobre una copia temporal, con el shell de este repo (no el de
#      /usr/share/serpantinum): comprueba que las teclas existen y que la config carga ("config ok");
#   2. pruebas/atajos.lua con un Hyprland falso: ningún atajo repetido, qué hace cada uno, que los de
#      Serpantinum que chocaban ya no estén y la lógica de los escritorios, los especiales y los tamaños.
# Todo pasa en una carpeta temporal (HOME y XDG_* propios). No arranca ni reinicia Hyprland ni el shell.
#
#   pruebas/atajos.sh
#
# Lo que esto NO cubre (hay que mirarlo en la VM, vm/vm.sh, con la config instalada por
# «vortex configurar-usuario»): que las funciones de los atajos funcionen contra el Hyprland de
# verdad (escritorios por grupos, especiales, imagen en imagen, reinicio del shell, Super solo al
# soltar, Super+. en el lanzador). Receta rápida dentro de la sesión de la VM:
#   hyprctl binds -j | jq length          # unos 170
#   hyprctl configerrors                  # sin errores de Lua
#   Super+3, Ctrl+Super+2, Super+M, Ctrl+Shift+Esc, Super solo, Super+., Ctrl+Super+Alt+R
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-atajos-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

echo "== Hyprland --verify-config"
if command -v Hyprland >/dev/null; then
  mkdir -p "$T/home/.config/vortex" "$T/run"
  chmod 700 "$T/run"
  # copiar_config CARPETA: la config del repo con el shell del repo (no el de /usr/share/serpantinum).
  copiar_config() {
    mkdir -p "$1"
    cp "$AQUI"/config/hypr/*.lua "$1/"
    sed -i "s|^local serp = .*|local serp = \"$AQUI/shell/compositors/hyprland\"|" "$1/hyprland.lua"
  }
  # verificar CARPETA: la salida de «Hyprland --verify-config» sobre esa copia.
  verificar() {
    env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY \
      HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_RUNTIME_DIR="$T/run" \
      Hyprland --verify-config -c "$1/hyprland.lua" 2>&1
  }

  copiar_config "$T/hypr"
  # Estricto: si atajos.lua falla, hyprland.lua lo tragaría (y avisaría al iniciar sesión); aquí no.
  sed -i 's|pcall(require, "atajos")|true, require("atajos")|; s|pcall(atajos.registrar)|true, atajos.registrar()|' "$T/hypr/hyprland.lua"
  salida="$(verificar "$T/hypr")"
  if grep -q '^config ok' <<<"$salida"; then
    bien "config ok"
  else
    mal "la config no pasa --verify-config"
    sed 's/^/      /' <<<"$salida"
  fi

  # Respaldo (sin el modo estricto): el escritorio tiene que cargar aunque atajos.lua falte o falle.
  copiar_config "$T/sin-atajos"
  rm "$T/sin-atajos/atajos.lua"
  if grep -q '^config ok' <<<"$(verificar "$T/sin-atajos")"; then
    bien "sin atajos.lua la config carga igual (config ok)"
  else
    mal "sin atajos.lua la config no carga"
  fi

  copiar_config "$T/atajos-falla"
  printf 'return { reemplaza = function() return false end, registrar = function() error("boom") end }\n' >"$T/atajos-falla/atajos.lua"
  if grep -q '^config ok' <<<"$(verificar "$T/atajos-falla")"; then
    bien "con un atajos.lua que falla al registrar la config carga igual (config ok)"
  else
    mal "con un atajos.lua que falla al registrar la config no carga"
  fi

  # Con un error de sintaxis Hyprland muestra su propio error de config (la barra roja) y sigue;
  # lo que no debe pasar es que se cuelgue o que el error no diga qué archivo falló.
  copiar_config "$T/atajos-rota"
  echo 'esto no es lua (' >"$T/atajos-rota/atajos.lua"
  salida="$(timeout 30 bash -c "$(declare -f verificar); T='$T'; verificar '$T/atajos-rota'")"
  if grep -q 'atajos.lua' <<<"$salida"; then
    bien "con un atajos.lua roto Hyprland dice cuál es el error y no se cuelga"
  else
    mal "con un atajos.lua roto no se ve el error"
    sed 's/^/      /' <<<"$salida"
  fi
else
  echo "  --  Hyprland no está instalado: se salta"
fi

echo "== Atajos (Hyprland falso)"
lua=""
for candidato in lua lua5.4 luajit; do
  command -v "$candidato" >/dev/null && { lua="$candidato"; break; }
done
if [[ -z "$lua" ]]; then
  echo "  --  no hay lua/luajit: se salta"
elif mkdir -p "$T/home/.config/vortex" \
  && HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_CACHE_HOME="$T/home/.cache" \
    "$lua" "$AQUI/pruebas/atajos.lua" "$AQUI"; then
  :
else
  FALLOS=$((FALLOS + 1))
fi

echo
if ((FALLOS)); then
  echo "Fallaron pruebas de atajos."
  exit 1
fi
echo "Todas las pruebas de atajos pasaron."
