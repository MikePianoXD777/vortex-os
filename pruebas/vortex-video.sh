#!/usr/bin/env bash
# Pruebas de Vortexine Video (2.7): el motor y la ventana sin pantalla, sin abrir tu cámara. Con una cámara
# (de verdad o falsa, en la VM) usa pruebas/vortex-video-camara.py.
#
#   pruebas/vortex-video.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI/apps/vortex-video" || exit 1
python3 -W error::ResourceWarning -m unittest discover -s pruebas "$@"
