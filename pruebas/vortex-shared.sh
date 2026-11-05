#!/usr/bin/env bash
# Pruebas de Vortexine Shared (2.7): el motor y el servicio con un sendme falso, y la ventana sin pantalla.
#
#   pruebas/vortex-shared.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI/apps/vortex-shared" || exit 1
python3 -W error::ResourceWarning -m unittest discover -s pruebas "$@"
