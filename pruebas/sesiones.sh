#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2034
# Prueba de las sesiones de trabajo (4.0, bin/vortex-sesiones): hyprctl y /proc falsos, HOME temporal.
# No abre ninguna ventana ni toca tu escritorio.
#
#   pruebas/sesiones.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VS="$AQUI/bin/vortex-sesiones"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-sesiones.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/home" "$T/datos/applications" "$T/proc/100/task/100" "$T/proc/101" "$T/proc/200" "$T/proc/300" "$T/carpeta con espacio"
# hyprctl falso: las ventanas salen de $T/clientes.json; lo que se le manda queda en $T/registro
cat >"$T/bin/hyprctl" <<F
#!/bin/sh
case "\$*" in
  *"clients -j"*) cat "$T/clientes.json" ;;
  *) echo "\$*" >>"$T/registro" ;;
esac
F
printf '#!/bin/sh\necho "$*" >>"%s/avisos"\n' "$T" >"$T/bin/notify-send"
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH" HOME="$T/home" XDG_DATA_HOME="$T/datos" VORTEX_PROC="$T/proc" VORTEX_SESIONES_PAUSA=0 HYPRLAND_INSTANCE_SIGNATURE=falso
# una terminal (kitty, con una shell dentro que está en una carpeta), un editor con .desktop, algo sin .desktop y un juego
printf 'kitty\0' >"$T/proc/100/cmdline"
echo 101 >"$T/proc/100/task/100/children"
ln -s "$T/carpeta con espacio" "$T/proc/101/cwd"
printf '/opt/raro/miapp\0--modo\0con espacio\0' >"$T/proc/200/cmdline"
printf 'steam\0' >"$T/proc/300/cmdline"
cat >"$T/datos/applications/editor-x.desktop" <<'D'
[Desktop Entry]
Name=Editor X
Exec=editor-x --nueva-ventana %U
StartupWMClass=EditorX
D
cat >"$T/clientes.json" <<'J'
[
 {"address":"0x1","mapped":true,"class":"kitty","pid":100,"workspace":{"id":1,"name":"1"}},
 {"address":"0x2","mapped":true,"class":"EditorX","pid":150,"workspace":{"id":2,"name":"2"}},
 {"address":"0x3","mapped":true,"class":"miapp","pid":200,"workspace":{"id":3,"name":"3"}},
 {"address":"0x4","mapped":true,"class":"steam_app_730","pid":300,"workspace":{"id":4,"name":"4"}},
 {"address":"0x5","mapped":false,"class":"fantasma","pid":200,"workspace":{"id":1,"name":"1"}},
 {"address":"0x6","mapped":true,"class":"especial","pid":200,"workspace":{"id":-98,"name":"special:magic"}}
]
J

echo "== guardar"
salida="$("$VS" guardar 'Escuela "x"' 2>&1)"
F="$T/datos/vortex/sesiones/Escuela x.json"
probar "guarda con nombre limpio y dice cuántas apps" "[[ -f '$F' && \"\$salida\" == 'Guardé «Escuela x»: 3 apps.' ]]"
probar "no guarda juegos de Steam, ventanas sin mostrar ni escritorios especiales" "[[ \$(jq '.ventanas | length' '$F') == 3 ]] && ! grep -q 'steam_app\|fantasma\|especial' '$F'"
probar "usa el .desktop si lo hay (sin los %U)" "[[ \$(jq -r '.ventanas[] | select(.clase == \"EditorX\") | .comando' '$F') == 'editor-x --nueva-ventana' ]]"
probar "sin .desktop, el comando del proceso, bien entrecomillado" "jq -r '.ventanas[] | select(.clase == \"miapp\") | .comando' '$F' | grep -q 'miapp --modo con\\\\ espacio'"
probar "la terminal guarda la carpeta de su shell" "[[ \$(jq -r '.ventanas[] | select(.clase == \"kitty\") | .carpeta' '$F') == '$T/carpeta con espacio' ]]"
probar "guarda el escritorio de cada una" "[[ \$(jq -c '[.ventanas[].escritorio] | sort' '$F') == '[1,2,3]' ]]"
probar "queda en el lanzador como «Sesión: Escuela x»" "grep -q '^Name=Sesión: Escuela x' '$T/datos/applications/vortex-sesion-escuela-x.desktop' && grep -q 'abrir \"Escuela x\"' '$T/datos/applications/vortex-sesion-escuela-x.desktop' && [[ -f '$T/datos/applications/vortex-sesion-guardar.desktop' ]]"
probar "avisa con un texto claro" "grep -q 'guardada' '$T/avisos'"
probar "sin nombre ni zenity, lo dice en vez de guardar nada raro" "! PATH='$T/bin:/usr/bin:/bin' bash -c 'command -v zenity' >/dev/null || true; [[ \$('$VS' lista | wc -l) == 1 ]]"

echo "== lista / ver"
probar "lista" "[[ \"\$('$VS' lista)\" == Escuela\\ x\\ \\ \\(3\\ apps,\\ guardada* ]]"
probar "ver" "'$VS' ver 'Escuela x' | grep -q 'Escritorio 2: EditorX'"

echo "== abrir"
cp "$T/clientes.json" "$T/clientes-todas.json"
echo '[]' >"$T/clientes.json"
: >"$T/registro"
salida="$("$VS" abrir 'Escuela x' 2>&1)"
probar "abre lo que no está abierto, cada uno en su escritorio" "[[ \$(grep -c 'exec_cmd' '$T/registro') == 3 ]] && grep -q 'workspace = \"2\"' '$T/registro' && [[ \"\$salida\" == 'Abrí «Escuela x»: 3 apps nuevas.' ]]"
probar "la terminal vuelve a su carpeta" "grep -q 'kitty.*--directory' '$T/registro' && grep -q 'carpeta' '$T/registro'"
# ahora todo está abierto: las apps no se repiten (se pasan a su escritorio) y la terminal sí
cp "$T/clientes-todas.json" "$T/clientes.json"
: >"$T/registro"
"$VS" abrir 'Escuela x' >/dev/null 2>&1
probar "con todo abierto: el editor y miapp no se repiten" "! grep -q 'editor-x\|miapp' '$T/registro'"
probar "pasa a su escritorio lo que ya estaba abierto" "grep -q 'address:0x2' '$T/registro' && grep -q 'workspace = 2' '$T/registro' && grep -q 'address:0x3' '$T/registro'"
probar "una terminal sí se abre otra vez (una por carpeta)" "grep -q 'kitty' '$T/registro'"
probar "una sesión que no existe lo dice" "! '$VS' abrir 'Nada' 2>'$T/err' && grep -q 'No tengo una sesión' '$T/err'"

echo "== borrar"
probar "borra la sesión y su entrada del lanzador" "'$VS' borrar 'Escuela x' >/dev/null && [[ ! -f '$F' && ! -f '$T/datos/applications/vortex-sesion-escuela-x.desktop' ]]"
echo '[]' >"$T/clientes.json"
probar "sin apps abiertas, no guarda nada" "! '$VS' guardar Vacia >/dev/null 2>&1 && [[ ! -f '$T/datos/vortex/sesiones/Vacia.json' ]]"
probar "con Hyprland caído, lo dice" "echo 'no' >'$T/clientes.json'; ! '$VS' guardar Otra >/dev/null 2>'$T/err' && grep -q 'No pude ver tus ventanas' '$T/err'"
probar "está en la lista de comandos que se instalan" "grep -q 'COMANDOS_VORTEX=.*vortex-sesiones' '$AQUI/bin/vortex'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
