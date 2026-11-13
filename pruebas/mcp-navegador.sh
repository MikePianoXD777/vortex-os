#!/usr/bin/env bash
# Prueba del navegador de Claude (vortex-mcp 0.2) con Firefox headless, un perfil temporal y un servidor de
# juguete en 127.0.0.1 (pruebas/mcp-navegador.py), con límite de memoria y de tiempo: no abre ventanas ni
# toca tu Firefox.
#
#   pruebas/mcp-navegador.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- \
  timeout 300 env -u WAYLAND_DISPLAY -u DISPLAY MOZ_HEADLESS=1 python3 "$AQUI/pruebas/mcp-navegador.py"
