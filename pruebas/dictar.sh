#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «Dictar» (2.7, bin/vortex-dictar) con un oído de mentira (apps/miku no se importa: el paquete
# miku falso da el audio y el texto), wtype, wl-copy, notify-send y gdbus falsos. No abre el micrófono ni
# escribe en tu escritorio. Para probarlo con whisper de verdad: VORTEX_MIKU_ENTRADA=voz.wav vortex-dictar.
#
#   pruebas/dictar.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VD="$AQUI/bin/vortex-dictar"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-dictar.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/miku/miku" "$T/run"
cat >"$T/bin/notify-send" <<FIN
#!/bin/sh
printf '%s\n' "\$*" >>"$T/avisos"
echo 7
FIN
cat >"$T/bin/gdbus" <<FIN
#!/bin/sh
echo cerrar >>"$T/avisos"
FIN
cat >"$T/bin/wl-copy" <<FIN
#!/bin/sh
shift
printf '%s' "\$*" >"$T/copiado"
FIN
chmod +x "$T/bin/"*
: >"$T/miku/miku/__init__.py"
cat >"$T/miku/miku/descargas.py" <<'FIN'
import os
class ErrorDescarga(Exception): pass
def paquete_listo(p): return not os.path.exists(os.environ["PRUEBA_SIN_MODELO"]) if os.environ.get("PRUEBA_SIN_MODELO") else True
class Descarga:
    def __init__(self, n):
        import threading
        self.cancelar = threading.Event()
    def correr(self):
        raise ErrorDescarga("Sin internet.")
FIN
cat >"$T/miku/miku/oido.py" <<'FIN'
import os, threading
class ErrorOido(Exception): pass
def encontrar_whisper():
    if os.environ.get("PRUEBA_SIN_WHISPER"): raise ErrorOido("No encuentro el oído.")
    return "w", "m"
def transcribir(pcm): return os.environ.get("PRUEBA_TEXTO", "")
class Oido:
    def __init__(self, **k): self.corte = threading.Event()
    def cortar_ahora(self): self.corte.set()
    def escuchar(self):
        if os.environ.get("PRUEBA_ESPERAR"):
            open(os.environ["PRUEBA_ESPERAR"], "w").close()
            if not self.corte.wait(10): return None
        return b"audio" if os.environ.get("PRUEBA_TEXTO") is not None else None
FIN
export PATH="$T/bin:$PATH" XDG_RUNTIME_DIR="$T/run" VORTEX_MIKU_DIR="$T/miku"
correr() { rm -f "$T/avisos" "$T/escrito" "$T/copiado"; env "$@" "$VD"; }
con_wtype() {
  cat >"$T/bin/wtype" <<FIN
#!/bin/sh
shift
printf '%s' "\$*" >"$T/escrito"
printf '%s' "\$LC_ALL" >"$T/idioma"
exit ${1:-0}
FIN
  chmod +x "$T/bin/wtype"
}

con_wtype
correr LC_ALL=C PRUEBA_TEXTO="Hola, ¿cómo estás? -- con guiones"
probar "wtype siempre recibe UTF-8 (sin él falla con los acentos)" "[[ \$(cat '$T/idioma') == C.UTF-8 ]]"
probar "escribe lo dictado, con acentos y signos" "[[ \$(cat '$T/escrito') == 'Hola, ¿cómo estás? -- con guiones' ]]"
probar "avisa una sola cosa a la vez (cierra la anterior)" "grep -q '^cerrar\$' '$T/avisos' && grep -q 'Te escucho' '$T/avisos'"
probar "no deja el candado" "[[ ! -e '$T/run/vortex-dictar.pid' ]]"

correr PRUEBA_TEXTO=""
probar "sin palabras no escribe nada" "[[ ! -e '$T/escrito' ]] && grep -q 'No te oí' '$T/avisos'"

correr A=1
probar "si nadie habla, lo dice" "[[ ! -e '$T/escrito' ]] && grep -q 'No te oí' '$T/avisos'"

con_wtype 1
correr PRUEBA_TEXTO="no hay teclado virtual"
probar "si no se puede escribir, queda en el portapapeles" \
  "[[ \$(cat '$T/copiado') == 'no hay teclado virtual' ]] && grep -q 'portapapeles' '$T/avisos'"

rm -f "$T/bin/wtype"
correr PRUEBA_TEXTO="sin wtype"
probar "sin wtype, también al portapapeles" "[[ \$(cat '$T/copiado') == 'sin wtype' ]]"
con_wtype

correr PRUEBA_SIN_WHISPER=1 PRUEBA_TEXTO="x"
probar "sin whisper explica qué falta" "[[ ! -e '$T/escrito' ]] && grep -q 'No encuentro el oído' '$T/avisos'"

touch "$T/sin-modelo"
correr PRUEBA_SIN_MODELO="$T/sin-modelo" PRUEBA_TEXTO="x"
probar "si no se puede bajar el modelo, dice por qué" "[[ ! -e '$T/escrito' ]] && grep -q 'Sin internet' '$T/avisos'"

# Apretar el atajo otra vez corta la grabación y se escribe lo que iba
rm -f "$T/avisos" "$T/escrito" "$T/grabando"
PRUEBA_ESPERAR="$T/grabando" PRUEBA_TEXTO="cortado a mano" "$VD" &
PID=$!
for _ in {1..50}; do [[ -e "$T/grabando" ]] && break; sleep 0.1; done
"$VD"
wait "$PID"
probar "la segunda pulsación corta y no abre otro dictado" "[[ \$(cat '$T/escrito') == 'cortado a mano' ]] && [[ \$(grep -c 'Te escucho' '$T/avisos') == 1 ]]"

echo 999999 >"$T/run/vortex-dictar.pid"
correr PRUEBA_TEXTO="tras un candado viejo"
probar "un candado viejo no estorba" "[[ \$(cat '$T/escrito') == 'tras un candado viejo' ]]"

probar "la barra no toma el teclado virtual de wtype por el tuyo (se quedaba en «ER»)" \
  "[[ \$(grep -c 'hl-virtual-keyboard' '$AQUI/shell/src/quickshell/bar/faces/kb/KbFace.qml') == 2 ]]"
probar "Super+Shift+D está en Hyprland y en la hoja de atajos" \
  "grep -q 'SUPER + SHIFT + D.*vortex-dictar' '$AQUI/config/hypr/hyprland.lua' && grep -q 'SUPER + SHIFT + D' '$AQUI/shell/src/assets/atajos.json'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
