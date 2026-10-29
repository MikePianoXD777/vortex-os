#!/usr/bin/env bash
# Pruebas de Vortex Player (2.0): el motor (calidad, biblioteca, descargas, audio, ecualizador) y la
# ventana con mpv sin sonido y MPRIS en un D-Bus aparte. No toca tu música, tu audio ni tu panel.
#
#   pruebas/vortex-player.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI/apps/vortex-player" || exit 1
python3 -W error::ResourceWarning -m unittest discover -s pruebas "$@"
