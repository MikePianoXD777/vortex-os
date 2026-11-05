#!/usr/bin/env bash
# Pruebas de Vortexine Connect: el motor y la ventana con nmcli, netsh y warp-cli falsos.
#
#   pruebas/vortex-connect.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI/apps/vortex-connect" || exit 1
python3 -W error::ResourceWarning -m unittest discover -s pruebas "$@"
