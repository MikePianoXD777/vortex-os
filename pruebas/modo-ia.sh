#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex modo-ia» (3.1, el hermano del modo juego) con un escritorio falso: hyprctl, quickshell,
# notify-send y systemd-run falsos en un PATH temporal, un HOME temporal y un servicio de clips falso (un
# sleep). No toca la sesión de verdad. El gestor que lo pone y lo quita solo se prueba en pruebas/gestor.sh.
#
#   pruebas/modo-ia.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-modo-ia-prueba.XXXXXX")"
CLIPS_PID=""
trap '[[ -n "$CLIPS_PID" ]] && kill "$CLIPS_PID" 2>/dev/null; rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config" "$T/home/.local/state/vortex" "$T/run/vortex-clips"
for c in hyprctl quickshell notify-send systemd-run; do
  printf '#!/bin/sh\necho "%s $*" >>"%s/registro"\nexit 0\n' "$c" "$T" >"$T/bin/$c"
done
chmod +x "$T/bin/"*
E="$T/home/.local/state/vortex"
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_STATE_HOME="$T/home/.local/state" XDG_RUNTIME_DIR="$T/run" \
    HYPRLAND_INSTANCE_SIGNATURE=prueba VORTEX_SHELL_QML="$T/Shell.qml" PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}
# Los clips grabando: un servicio falso con su PID donde lo deja bin/vortex-clips.
clips_falsos() {
  sleep 300 &
  CLIPS_PID=$!
  echo "$CLIPS_PID" >"$T/run/vortex-clips/servicio.pid"
}

echo "== vortex modo-ia"
clips_falsos
: >"$T/registro"
vortex modo-ia on Ollama
grep -q "hyprctl eval .*animations = { enabled = false }.*blur = { enabled = false }.*shadow = { enabled = false }" "$T/registro" &&
  bien "on: sin animaciones, desenfoque ni sombras" || mal "efectos: $(cat "$T/registro")"
grep -q "quickshell ipc -p $T/Shell.qml call wallpaper setPlayback all pause" "$T/registro" &&
  bien "on: el fondo animado en pausa" || mal "fondo: $(grep quickshell "$T/registro")"
grep -q "notify-send .*Modo IA: le di la gráfica a Ollama" "$T/registro" &&
  bien "on: avisa «Modo IA: le di la gráfica a Ollama»" || mal "aviso: $(grep notify "$T/registro")"
sleep 0.2
! kill -0 "$CLIPS_PID" 2>/dev/null && [[ -e "$E/modo-ia.clips" ]] &&
  bien "on: para los clips que grababan" || mal "clips siguen"
[[ -e "$E/modo-ia" && -e "$E/modo-ia.fondo" ]] && bien "on: deja su marca" || mal "marca"
[[ "$(vortex modo-ia estado)" == encendido ]] && bien "estado: encendido" || mal "estado"

: >"$T/registro"
vortex modo-ia off
grep -q "hyprctl reload" "$T/registro" && bien "off: los efectos vuelven (recarga)" || mal "off recarga: $(cat "$T/registro")"
grep -q "call wallpaper setPlayback all play" "$T/registro" && bien "off: el fondo vuelve a correr" || mal "off fondo"
sleep 0.2
grep -q "systemd-run --user --scope --quiet --collect -- $AQUI/bin/vortex-clips servicio" "$T/registro" &&
  bien "off: los clips vuelven a grabar (estaban grabando)" || mal "off clips: $(cat "$T/registro")"
! grep -q notify-send "$T/registro" && bien "off: sin otro aviso" || mal "off avisó"
[[ ! -e "$E/modo-ia" && ! -e "$E/modo-ia.fondo" && ! -e "$E/modo-ia.clips" ]] && bien "off: borra sus marcas" || mal "marcas"
[[ "$(vortex modo-ia estado)" == apagado ]] && bien "estado: apagado" || mal "estado off"

: >"$T/registro"
vortex modo-ia on Ollama --sin-aviso
! grep -q notify-send "$T/registro" && grep -q "setPlayback all pause" "$T/registro" &&
  bien "--sin-aviso: lo pone callado (vuelve después de un juego)" || mal "--sin-aviso: $(cat "$T/registro")"
[[ ! -e "$E/modo-ia.clips" ]] && bien "sin clips grabando no deja la marca de clips" || mal "marca de clips de más"
vortex modo-ia off

: >"$T/registro"
vortex modo-ia off
[[ ! -s "$T/registro" ]] && bien "off sin estar puesto no hace nada" || mal "off de más: $(cat "$T/registro")"

# El modo juego manda.
touch "$E/modo-juego"
: >"$T/registro"
vortex modo-ia on Ollama 2>/dev/null && mal "on con el modo juego puesto debía fallar" || bien "on con el modo juego puesto: no se pone"
[[ ! -s "$T/registro" && ! -e "$E/modo-ia" ]] && bien "y no toca nada" || mal "tocó algo: $(cat "$T/registro")"
rm -f "$E/modo-juego"

vortex modo-ia on Ollama
touch "$E/modo-juego" "$E/modo-juego.fondo"   # un juego empezó encima (y pausó el fondo él también)
: >"$T/registro"
vortex modo-ia off
[[ ! -s "$T/registro" && ! -e "$E/modo-ia" ]] && bien "off con un juego encima: ni efectos ni fondo (los devuelve el juego)" ||
  mal "off con juego: $(cat "$T/registro")"
rm -f "$E/modo-juego" "$E/modo-juego.fondo"

# Al revés: el modo juego se quita mientras el modo IA sigue (puesto a mano).
vortex modo-ia on Ollama
touch "$E/modo-juego" "$E/modo-juego.fondo"
: >"$T/registro"
vortex modo-juego off
grep -q "hyprctl reload" "$T/registro" && grep -A99 "hyprctl reload" "$T/registro" | grep -q "animations = { enabled = false }" &&
  bien "modo-juego off con el modo IA: recarga y deja los efectos apagados" || mal "modo-juego off: $(cat "$T/registro")"
! grep -q "setPlayback all play" "$T/registro" && bien "y el fondo sigue en pausa" || mal "el modo juego reanudó el fondo"
[[ -e "$E/modo-ia" && ! -e "$E/modo-juego" ]] && bien "y el modo IA sigue puesto" || mal "marcas tras modo-juego off"
: >"$T/registro"
vortex modo-ia alternar
[[ ! -e "$E/modo-ia" ]] && grep -q "setPlayback all play" "$T/registro" && bien "alternar: lo quita y el fondo vuelve" || mal "alternar off"
vortex modo-ia alternar
[[ -e "$E/modo-ia" ]] && bien "alternar: lo pone" || mal "alternar on"
vortex modo-ia off

vortex modo-ia nada >/dev/null 2>&1 && mal "una opción mala debía fallar" || bien "opción desconocida: falla con el uso"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
