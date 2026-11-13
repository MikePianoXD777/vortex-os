#!/usr/bin/env bash
# Prueba del navegador de Claude (vortex-mcp 0.3) con Firefox headless y con el controlador de Brave
# (Electron haciéndose pasar por Brave, sin ventanas), cada uno con un perfil temporal y un servidor de
# juguete en 127.0.0.1 (pruebas/mcp-navegador.py), con límite de memoria y de tiempo: no abre ventanas ni
# toca tus navegadores ni tus carpetas.
#
#   pruebas/mcp-navegador.sh [firefox|brave]    (sin argumento, los dos)
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rc=0
for nav in "${@:-firefox brave}"; do
  for n in $nav; do
    systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- \
      timeout 400 env -u WAYLAND_DISPLAY -u DISPLAY MOZ_HEADLESS=1 python3 "$AQUI/pruebas/mcp-navegador.py" "$n" || rc=1
  done
done
exit $rc
