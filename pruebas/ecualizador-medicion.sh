#!/usr/bin/env bash
# Mide el ecualizador con ruido rosa (pruebas/ecualizador-medicion.py): un PipeWire aparte con una salida
# nula de prueba, su propio D-Bus y un EasyEffects sin ventana. No toca la salida, el volumen ni el
# EasyEffects de la persona. Con límite de memoria y de tiempo.
#
#   pruebas/ecualizador-medicion.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- \
  timeout 300 env -u WAYLAND_DISPLAY -u DISPLAY QT_QPA_PLATFORM=offscreen \
  dbus-run-session -- python3 "$AQUI/pruebas/ecualizador-medicion.py"
