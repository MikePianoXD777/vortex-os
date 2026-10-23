#!/usr/bin/env bash
# Pruebas de los atajos de teclado (config/hypr/hyprland.lua y atajos.lua), sin tocar tu Hyprland:
#   1. «Hyprland --verify-config» sobre una copia temporal, con el shell de este repo (no el de
#      /usr/share/serpantinum): comprueba que las teclas existen y que la config carga ("config ok");
#   2. pruebas/atajos.lua con un Hyprland falso: ningún atajo repetido, qué hace cada uno, que los de
#      Serpantinum que chocaban ya no estén y la lógica de los escritorios, los especiales y los tamaños.
# Todo pasa en una carpeta temporal (HOME y XDG_* propios). No arranca ni reinicia Hyprland ni el shell.
#
#   pruebas/atajos.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-atajos-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

echo "== Hyprland --verify-config"
if command -v Hyprland >/dev/null; then
  mkdir -p "$T/home/.config/vortex" "$T/hypr" "$T/run"
  chmod 700 "$T/run"
  cp "$AQUI"/config/hypr/*.lua "$T/hypr/"
  sed -i "s|^local serp = .*|local serp = \"$AQUI/shell/compositors/hyprland\"|" "$T/hypr/hyprland.lua"
  # Estricto: si atajos.lua falla, hyprland.lua lo tragaría (y avisaría al iniciar sesión); aquí no.
  sed -i 's|pcall(require, "atajos")|true, require("atajos")|; s|pcall(atajos.registrar)|true, atajos.registrar()|' "$T/hypr/hyprland.lua"
  salida="$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY \
    HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_RUNTIME_DIR="$T/run" \
    Hyprland --verify-config -c "$T/hypr/hyprland.lua" 2>&1)"
  if grep -q '^config ok' <<<"$salida"; then
    bien "config ok"
  else
    mal "la config no pasa --verify-config"
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
elif "$lua" "$AQUI/pruebas/atajos.lua" "$AQUI"; then
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
