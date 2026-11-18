#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2034
# Prueba del respaldo de partidas (4.0, bin/vortex-guardados): una Steam falsa con un prefijo de Proton, un
# /proc falso para un juego de Heroic y HOME temporal. No toca tus guardados de verdad.
#
#   pruebas/guardados.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VG="$AQUI/bin/vortex-guardados"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-guardados.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/home" "$T/steam/steamapps" "$T/proc"
printf '#!/bin/sh\necho "$*" >>"%s/avisos"\n' "$T" >"$T/bin/notify-send"
chmod +x "$T/bin/notify-send"
export PATH="$T/bin:$PATH" HOME="$T/home" XDG_DATA_HOME="$T/home/datos" VORTEX_STEAM="$T/steam" VORTEX_PROC="$T/proc"
export VORTEX_GUARDADOS_MINUTOS=0
printf '"AppState"\n{\n\t"appid"\t"1234"\n\t"name"\t"Mi Juego: Edición Épica"\n}\n' >"$T/steam/steamapps/appmanifest_1234.acf"
U="$T/steam/steamapps/compatdata/1234/pfx/drive_c/users/steamuser"
mkdir -p "$U/Documents/My Games" "$U/AppData/Local/Cache" "$U/AppData/Local/MiJuego" "$U/AppData/Roaming/Otra Cosa" "$U/Desktop"
echo "partida uno" >"$U/Documents/My Games/slot1.sav"
echo "ajustes" >"$U/AppData/Local/MiJuego/config.ini"
head -c 200000 /dev/urandom >"$U/AppData/Local/Cache/shaders.bin"
echo "no" >"$U/Desktop/atajo.lnk"
D="$T/home/datos/vortex/partidas-guardadas"
nombre_copias() { ls -1 "$D"/mi-juego-edicion-epica-1234/*.tar.gz 2>/dev/null | wc -l; }

echo "== Steam / Proton"
"$VG" antes steam_app_1234 0
probar "copia al abrir un juego de Steam, con su nombre" "[[ \$(nombre_copias) == 1 && \$(cat '$D'/mi-juego-edicion-epica-1234/.nombre) == 'Mi Juego: Edición Épica' ]]"
lista="$(tar -tzf "$D"/mi-juego-edicion-epica-1234/*.tar.gz)"
probar "guarda Documentos y AppData" "grep -q 'My Games/slot1.sav' <<<\"\$lista\" && grep -q 'MiJuego/config.ini' <<<\"\$lista\""
probar "deja fuera cachés y lo que no es del juego (Escritorio)" "! grep -qi 'cache\|shaders\|atajo' <<<\"\$lista\""
probar "no avisa (en silencio)" "[[ ! -e '$T/avisos' ]]"
"$VG" antes steam_app_1234 0
probar "sin cambios, no repite" "[[ \$(nombre_copias) == 1 ]]"
sleep 1.1
echo "partida dos" >"$U/Documents/My Games/slot2.sav"
"$VG" antes steam_app_1234 0
probar "con cambios, otra copia" "[[ \$(nombre_copias) == 2 ]]"
for i in 3 4 5 6 7 8; do sleep 1.1; echo "partida $i" >"$U/Documents/My Games/slot$i.sav"; "$VG" antes steam_app_1234 0; done
probar "rotación: se quedan 5" "[[ \$(nombre_copias) == 5 ]]"
probar "y se van los .carpetas de las viejas" "[[ \$(ls -1 '$D'/mi-juego-edicion-epica-1234/*.carpetas | wc -l) == 5 ]]"
MIN="$(VORTEX_GUARDADOS_MINUTOS=60 bash -c "echo '' >'$U/Documents/My Games/slot9.sav'; '$VG' antes steam_app_1234 0; ls -1 '$D'/mi-juego-edicion-epica-1234/*.tar.gz | wc -l")"
probar "con la última copia de hace poco (60 min), no insiste" "[[ \$MIN == 5 ]]"

echo "== lista y restaurar"
probar "lista dice el juego y cuántas" "[[ \"\$('$VG' lista)\" == 'Mi Juego: Edición Épica  (5 copias;'* ]]"
rm -f "$U/Documents/My Games/slot1.sav" "$U/Documents/My Games/slot2.sav"
echo "rota" >"$U/Documents/My Games/slot8.sav"
mkdir -p "$T/bin2"; printf '#!/bin/sh\nexit 1\n' >"$T/bin2/vortex"; chmod +x "$T/bin2/vortex"
# Con la regla de siempre (una copia por hora como mucho): restaurar justo después de una copia también debe
# guardar lo de ahora.
PATH="$T/bin2:$PATH" VORTEX_GUARDADOS_MINUTOS=60 "$VG" restaurar "mi juego"
probar "restaurar devuelve lo borrado" "[[ -f '$U/Documents/My Games/slot8.sav' && \$(cat '$U/Documents/My Games/slot8.sav') == 'partida 8' ]]"
probar "y avisa claro" "grep -q 'Partidas restauradas' '$T/avisos'"
probar "lo que había antes queda como otra copia (aunque la última sea de hace segundos)" "[[ \$(nombre_copias) == 5 ]] && tar -xzOf \$(ls -1t '$D'/mi-juego-edicion-epica-1234/*.tar.gz | head -n1) '${U#/}/Documents/My Games/slot8.sav' | grep -qx 'rota'"
mkdir -p "$T/bin3"; printf '#!/bin/sh\nexit 0\n' >"$T/bin3/vortex"; chmod +x "$T/bin3/vortex"
probar "con un juego abierto, no restaura" "! PATH='$T/bin3:$PATH' '$VG' restaurar 'mi juego' 2>'$T/err' && grep -q 'Estás jugando' '$T/err'"
probar "un juego sin copias lo dice" "! '$VG' restaurar 'Nada' 2>'$T/err' && grep -q 'No tengo copias' '$T/err'"

echo "== Heroic / Lutris (prefijo de Wine en el entorno)"
P="$T/Games/Heroic/Prefixes/Otro Juego"
mkdir -p "$P/drive_c/users/$USER/Documents/Saves" "$T/proc/777"
echo "heroic" >"$P/drive_c/users/$USER/Documents/Saves/a.sav"
printf 'PATH=/usr/bin\0WINEPREFIX=%s\0' "$P" >"$T/proc/777/environ"
"$VG" antes otrojuego.exe 777
probar "copia los guardados del prefijo, con el nombre de su carpeta" "[[ -f \$(ls -1 '$D'/otro-juego/*.tar.gz | head -n1) ]] && tar -tzf \$(ls -1 '$D'/otro-juego/*.tar.gz | head -n1) | grep -q 'Saves/a.sav'"

echo "== carpetas conocidas y casos raros"
mkdir -p "$T/home/.local/share/Terraria/Players"
echo p >"$T/home/.local/share/Terraria/Players/yo.plr"
"$VG" antes Terraria 0
probar "un juego de Linux conocido (Terraria)" "tar -tzf \$(ls -1 '$D'/terraria/*.tar.gz | head -n1) | grep -q 'Players/yo.plr'"
before="$(ls "$D" | wc -l)"
"$VG" antes juego-desconocido 0
"$VG" antes "" 0
probar "un juego del que no sabe nada: no hace nada" "[[ \$(ls '$D' | wc -l) == $before ]]"
probar "los guardados enormes no se respaldan" "VORTEX_GUARDADOS_LIMITE_MB=0 '$VG' antes steam_app_1234 0; true"
probar "la prioridad de disco y CPU más baja" "grep -q 'ionice -c3' '$AQUI/bin/vortex-guardados'"
probar "el gestor lo llama al empezar un juego" "grep -q 'GUARDADOS.*antes' '$AQUI/bin/vortex-gestor'"
probar "se instala con los demás comandos" "grep -q 'COMANDOS_VORTEX=.*vortex-guardados' '$AQUI/bin/vortex' && grep -q 'vortex-guardados' '$AQUI/install/30-escritorio.sh'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
