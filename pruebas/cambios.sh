#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2034,SC1090
# Prueba de los cambios hechos a mano (4.1, traer_vortex / cmd_cambios / apartar_paquetes de bin/vortex):
# un repo «oficial» de mentira (bare), una copia instalada con archivos cambiados, sueltos y commits sin
# publicar, y un pacman falso que dice que un archivo de vortex-miku no es el original. Todo en un HOME
# temporal: no toca git ni pacman de verdad.
#
#   pruebas/cambios.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-cambios.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
ok_() { printf '  ok  %s\n' "$*"; }
mal_() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then ok_ "$1"; else mal_ "$1"; fi; }

export HOME="$T/home" XDG_DATA_HOME="$T/home/.local/share" XDG_STATE_HOME="$T/home/.local/state"
export XDG_CONFIG_HOME="$T/home/.config" GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=prueba GIT_AUTHOR_EMAIL=prueba@vortex GIT_COMMITTER_NAME=prueba GIT_COMMITTER_EMAIL=prueba@vortex
mkdir -p "$HOME" "$T/fbin"
git config --global init.defaultBranch main

# pacman falso: vortex-miku instalado, con un archivo cambiado y otro borrado.
INST="$T/usr/share/vortex-miku"
mkdir -p "$INST"
echo "mio" >"$INST/corazon.py"
cat >"$T/fbin/pacman" <<FIN
#!/bin/sh
case "\$*" in
  "-Q vortex-miku") echo "vortex-miku 2.1.0-4" ;;
  "-Qkk vortex-miku")
    echo "warning: vortex-miku: $INST/corazon.py (Modification time mismatch)"
    echo "warning: vortex-miku: $INST/corazon.py (SHA256 checksum mismatch)"
    echo "warning: vortex-miku: $INST/voz.py (No such file or directory)"
    echo "vortex-miku: 106 total files, 2 altered files" ;;
  *) exit 1 ;;
esac
FIN
chmod +x "$T/fbin/pacman"
export PATH="$T/fbin:$PATH"

# El repo oficial.
W="$T/oficial-trabajo" O="$T/oficial.git" R="$T/instalado"
mkdir -p "$W/bin" "$W/apps/miku" "$W/packaging/vortex-miku"
cp "$AQUI/bin/vortex" "$W/bin/vortex"
touch "$W/install.sh"
echo "version 1" >"$W/apps/miku/app.py"
echo "pkgname=vortex-miku" >"$W/packaging/vortex-miku/PKGBUILD"
git -C "$W" init -q && git -C "$W" add -A && git -C "$W" commit -qm uno
git clone -q --bare "$W" "$O"
git -C "$W" remote add origin "$O"
git clone -q "$O" "$R"

# Las funciones de bin/vortex sin su «case» final (ni su set -e, que cortaría la prueba al primer fallo).
sed '/^case "${1:-ayuda}" in$/,$d' "$AQUI/bin/vortex" | sed "s#^VORTEX_DIR=.*#VORTEX_DIR='$R'#" >"$T/funciones.sh"
source "$T/funciones.sh"
set +e

# En la copia instalada: un archivo cambiado, uno suelto en el código, lo que baja makepkg (no cuenta)
# y uno suelto en packaging/ que la versión nueva también trae (sí cuenta: choca).
echo "mi version" >"$R/apps/miku/app.py"
echo "privado" >"$R/apps/miku/privacidad.py"
mkdir -p "$R/packaging/vortex-studio"
echo "tarball" >"$R/packaging/vortex-studio/VortexStudio.tar.gz"
echo "mio" >"$R/packaging/vortex-miku/nuevo.txt"

# La versión oficial avanza: cambia app.py y agrega nuevo.txt.
echo "version 2" >"$W/apps/miku/app.py"
echo "oficial" >"$W/packaging/vortex-miku/nuevo.txt"
git -C "$W" add -A && git -C "$W" commit -qm dos && git -C "$W" push -q origin main
git -C "$R" fetch -q origin main

echo "vortex cambios"
salida="$(cmd_cambios)"
probar "enseña el archivo cambiado" "grep -q 'modificado  *apps/miku/app.py' <<<\"\$salida\""
probar "enseña el suelto del código" "grep -q 'nuevo  *apps/miku/privacidad.py' <<<\"\$salida\""
probar "enseña el suelto de packaging/ que choca" "grep -q 'nuevo  *packaging/vortex-miku/nuevo.txt' <<<\"\$salida\""
probar "no enseña lo que bajó makepkg" "! grep -q VortexStudio <<<\"\$salida\""
probar "enseña el archivo cambiado del paquete" "grep -q 'vortex-miku  *$INST/corazon.py' <<<\"\$salida\""
probar "enseña el archivo borrado del paquete" "grep -q 'vortex-miku  *$INST/voz.py' <<<\"\$salida\""
probar "la fecha sola no cuenta" "[[ \$(grep -c corazon.py <<<\"\$salida\") == 1 ]]"
probar "no tocó nada" "[[ \$(cat '$R/apps/miku/app.py') == 'mi version' && -f '$R/apps/miku/privacidad.py' ]]"

echo "vortex actualizar, sin terminal (se guardan solos)"
traer_vortex </dev/null >"$T/salida1" 2>&1
rc=$?
d="$(ls -d "$XDG_DATA_HOME"/vortex/cambios-apartados/* 2>/dev/null | head -1)"
probar "trae lo nuevo" "[[ $rc == 0 && \$(git -C '$R' rev-parse HEAD) == \$(git -C '$O' rev-parse main) ]]"
probar "pone la versión oficial" "[[ \$(cat '$R/apps/miku/app.py') == 'version 2' && \$(cat '$R/packaging/vortex-miku/nuevo.txt') == oficial ]]"
probar "quita el suelto del código" "[[ ! -e '$R/apps/miku/privacidad.py' ]]"
probar "deja lo que bajó makepkg" "[[ -f '$R/packaging/vortex-studio/VortexStudio.tar.gz' ]]"
probar "guarda cada archivo como estaba" "[[ \$(cat '$d/archivos/apps/miku/app.py') == 'mi version' && \$(cat '$d/archivos/apps/miku/privacidad.py') == privado && \$(cat '$d/archivos/packaging/vortex-miku/nuevo.txt') == mio ]]"
probar "guarda el parche, la lista y el LEEME" "grep -q 'mi version' '$d/cambios.patch' && [[ -s '$d/lista.txt' && -s '$d/LEEME.txt' ]]"
probar "lo dice en la terminal" "grep -q 'Tus cambios quedaron\|tus cambios quedaron' '$T/salida1'"
salida="$(cmd_cambios)"
probar "vortex cambios ya no ve nada del código" "! grep -q apps/miku <<<\"\$salida\""

echo "un commit sin publicar"
rm -rf "$XDG_DATA_HOME/vortex/cambios-apartados"
echo "local" >"$R/apps/miku/extra.py"
git -C "$R" add -A && git -C "$R" commit -qm "mio sin publicar"
traer_vortex </dev/null >"$T/salida2" 2>&1
d="$(ls -d "$XDG_DATA_HOME"/vortex/cambios-apartados/* 2>/dev/null | head -1)"
probar "lo avisa" "grep -q '1 commit(s) tuyos' '$T/salida2'"
probar "lo guarda como parche" "grep -q 'mio sin publicar' '$d/commits.patch'"
probar "regresa a la versión oficial" "[[ \$(git -C '$R' rev-parse HEAD) == \$(git -C '$O' rev-parse main) && ! -e '$R/apps/miku/extra.py' ]]"

echo "sin cambios"
rm -rf "$XDG_DATA_HOME/vortex/cambios-apartados"
echo "version 3" >"$W/apps/miku/app.py"
git -C "$W" commit -qam tres && git -C "$W" push -q origin main
traer_vortex </dev/null >"$T/salida3" 2>&1
probar "solo trae lo nuevo" "[[ \$(cat '$R/apps/miku/app.py') == 'version 3' ]]"
probar "sin respaldo ni aviso" "[[ ! -e '$XDG_DATA_HOME/vortex/cambios-apartados' ]] && ! grep -q 'versión oficial' '$T/salida3'"

echo "en la terminal: «n» no toca nada"
echo "mi version" >"$R/apps/miku/app.py"
echo "version 4" >"$W/apps/miku/app.py"
git -C "$W" commit -qam cuatro && git -C "$W" push -q origin main
if command -v script >/dev/null; then
  printf 'source "%s"; set +e; traer_vortex; echo "rc=$?"\n' "$T/funciones.sh" >"$T/pregunta.sh"
  printf 'n\n' | script -qec "bash '$T/pregunta.sh'" /dev/null >"$T/salida4" 2>&1
  probar "pregunta" "grep -q '¿Los guardo en un respaldo' '$T/salida4'"
  probar "devuelve 2 y no toca el archivo" "grep -q 'rc=2' '$T/salida4' && [[ \$(cat '$R/apps/miku/app.py') == 'mi version' ]]"
else
  ok_ "(sin «script»: no se prueba la pregunta)"
fi

echo "otra rama (carpeta de quien programa)"
git -C "$R" checkout -q -b mi-rama
traer_vortex </dev/null >"$T/salida5" 2>&1
probar "avisa y no la toca" "grep -q 'rama «mi-rama»' '$T/salida5' && [[ \$(cat '$R/apps/miku/app.py') == 'mi version' ]]"
git -C "$R" checkout -q -f main

echo "paquetes instalados con archivos cambiados"
rm -rf "$XDG_DATA_HOME/vortex/cambios-apartados"
paquetes="$(apartar_paquetes 2>"$T/salida6")"
d="$(ls -d "$XDG_DATA_HOME"/vortex/cambios-apartados/* 2>/dev/null | head -1)"
probar "dice qué paquete reinstalar" "[[ \$paquetes == vortex-miku ]]"
probar "lo avisa" "grep -q 'no son los originales' '$T/salida6'"
probar "guarda el archivo cambiado" "[[ \$(cat '$d/paquetes$INST/corazon.py') == mio ]]"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS."; exit 1; fi
echo "Todo bien."
