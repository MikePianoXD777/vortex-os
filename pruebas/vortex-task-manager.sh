#!/usr/bin/env bash
# Pruebas de Vortexine Task Manager (2.3): el motor (procesos, rendimiento, servicios) y la
# ventana sin pantalla. Solo finaliza procesos que lanza ella misma.
#
#   pruebas/vortex-task-manager.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI/apps/vortex-task-manager" || exit 1
python3 -W error::ResourceWarning -m unittest discover -s pruebas "$@"
