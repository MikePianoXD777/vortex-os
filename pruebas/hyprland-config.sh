#!/usr/bin/env bash
# La config de Hyprland de Vortex (config/hypr/*.lua) pasa «Hyprland --verify-config», en un HOME
# temporal como el que deja install/usuario.sh. Hyprland sale con 0 aunque haya errores: lo que vale
# es que diga «config ok».
#   pruebas/hyprland-config.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-hypr-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT

if ! command -v Hyprland >/dev/null; then
  echo "Falta Hyprland" >&2
  exit 1
fi

mkdir -p "$T/home/.config/hypr" "$T/run"
chmod 700 "$T/run"
cp "$AQUI"/config/hypr/*.lua "$T/home/.config/hypr/"
# Los módulos de Serpantinum, del repo (lo que va en la ISO) y no de /usr/share: así sirve también sin
# el shell instalado, como en el CI.
sed -i "s|^local serp = \"/usr/share/serpantinum/compositors/hyprland\"|local serp = \"$AQUI/shell/compositors/hyprland\"|" \
  "$T/home/.config/hypr/hyprland.lua"
grep -q "^local serp = \"$AQUI/shell/" "$T/home/.config/hypr/hyprland.lua" || { echo "No encontré «local serp» en hyprland.lua" >&2; exit 1; }
salida="$(cd "$T" && HOME="$T/home" XDG_RUNTIME_DIR="$T/run" XDG_CONFIG_HOME="$T/home/.config" \
  timeout 60 Hyprland --verify-config -c "$T/home/.config/hypr/hyprland.lua" 2>&1)"
resultado="$(sed -n '/Config parsing result/,$p' <<<"$salida" | grep -v -e '^=*$' -e 'Config parsing result')"
if [[ "$resultado" == "config ok" ]]; then
  echo "  ok  Hyprland --verify-config: config ok"
  echo "Todo bien"
else
  echo "  MAL Hyprland --verify-config:"
  printf '%s\n' "${resultado:-$salida}" | sed 's/^/        /'
  exit 1
fi
