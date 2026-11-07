#!/usr/bin/env bash
# Prueba del aviso de disco casi lleno (3.0, bin/vortex-disco): con df, notify-send y vortex-arreglar falsos.
# No avisa ni borra nada en tu escritorio. También prueba cuándo lo llama el gestor (revisar_disco).
#
#   pruebas/disco.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VD="$AQUI/bin/vortex-disco"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-disco.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/estado"
# df falso: «uso libre_mb» salen de $T/df (después de liberar, de $T/df-despues si existe)
cat >"$T/bin/df" <<F
#!/bin/sh
f="$T/df"; [ -e "$T/liberado" ] && [ -e "$T/df-despues" ] && f="$T/df-despues"
read -r uso libre <"\$f"
printf 'S.ficheros Bloques Usados Disponibles Uso%% Montado\n/dev/x 100000 1 %s %s%% /\n' "\$libre" "\$uso"
F
cat >"$T/bin/notify-send" <<F
#!/bin/sh
printf '%s\n' "\$@" >>"$T/aviso"; echo --- >>"$T/aviso"
case "\$*" in *liberar=*) printf '%s' "\${PRUEBA_ACCION:-}" ;; esac
F
cat >"$T/arreglar" <<F
#!/bin/sh
echo "\$*" >"$T/liberado"; echo "Se liberaron 2.0 GB."
F
chmod +x "$T/bin/"* "$T/arreglar"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/estado" VORTEX_DISCO_ARREGLAR="$T/arreglar"
disco() { printf '%s %s\n' "$1" "$2" >"$T/df"; }
correr() { rm -f "$T/aviso" "$T/liberado"; env "$@" "$VD" revisar; }

disco 81 93000
probar "--ver con espacio de sobra: bien" "[[ \"\$('$VD' --ver)\" == 'Bien: quedan 91 GB (81 % usado).' ]]"
correr PRUEBA_ACCION=
probar "con espacio de sobra no avisa" "[[ ! -e '$T/aviso' ]]"

disco 92 200000
correr PRUEBA_ACCION=
probar "disco grande al 92 % con 195 GB libres: no molesta" "[[ ! -e '$T/aviso' ]]"

disco 70 12000
correr PRUEBA_ACCION=
probar "12 GB libres pero al 70 %: no molesta" "[[ ! -e '$T/aviso' ]]"

disco 90 12000
probar "--ver dice cuánto queda y no avisa" "[[ \"\$('$VD' --ver)\" == 'Quedan 12 GB. Libera espacio antes de que se llene.' && ! -e '$T/aviso' ]]"
correr PRUEBA_ACCION=
probar "se llena (12 GB, 90 %): aviso normal que se va solo" "grep -qx 'El disco se está llenando' '$T/aviso' && grep -qx normal '$T/aviso' && grep -qx 30000 '$T/aviso'"
probar "con el botón de liberar" "grep -qx 'liberar=Liberar espacio' '$T/aviso'"
probar "ignorarlo no borra nada" "[[ ! -e '$T/liberado' ]]"
correr PRUEBA_ACCION=
probar "no repite en la misma semana" "[[ ! -e '$T/aviso' ]]"
touch -d '8 days ago' "$T/estado/vortex/aviso-disco-llenando"
correr PRUEBA_ACCION=
probar "a la semana, sí" "[[ -e '$T/aviso' ]]"

disco 97 3000
correr PRUEBA_ACCION=
probar "casi lleno (menos de 5 GB): aviso crítico, aunque ya avisó que se llenaba" "grep -qx 'El disco está casi lleno' '$T/aviso' && grep -qx critical '$T/aviso'"
probar "dice cuánto queda y qué pasa" "grep -q 'Quedan 2.9 GB. Con el disco lleno fallan' '$T/aviso'"
correr PRUEBA_ACCION=liberar
probar "casi lleno no repite en el mismo día" "[[ ! -e '$T/aviso' ]]"
touch -d '25 hours ago' "$T/estado/vortex/aviso-disco-lleno"

printf '50 40000\n' >"$T/df-despues"
correr PRUEBA_ACCION=liberar
probar "Liberar usa lo de «Algo no funciona → Disco»" "grep -qx 'arreglar disco' '$T/liberado'"
probar "y dice cuánto liberó y cuánto queda" "grep -q 'Se liberaron 2.0 GB. Ahora quedan 39 GB.' '$T/aviso'"
probar "si ya alcanza, no te regaña" "! grep -q 'es tuyo' '$T/aviso'"

touch -d '25 hours ago' "$T/estado/vortex/aviso-disco-lleno"
printf '96 4000\n' >"$T/df-despues"
correr PRUEBA_ACCION=liberar
probar "si sigue lleno, dice que lo demás es tuyo (no lo borra por ti)" "grep -q 'Lo que ocupa el resto es tuyo' '$T/aviso'"
probar "nunca borra en tu carpeta" "! grep -qE 'rm -|Descargas/|Vídeos/' '$VD'"

touch -d '25 hours ago' "$T/estado/vortex/aviso-disco-lleno"
correr PATH="$T/nada:/usr/bin:/bin" PRUEBA_ACCION=
probar "sin notify-send no se rompe" "[[ ! -e '$T/aviso' ]]"
echo basura >"$T/df"
correr PRUEBA_ACCION=
probar "con df raro no avisa ni se rompe" "[[ ! -e '$T/aviso' ]]"

# Cuándo lo llama el gestor: su función, sola.
# shellcheck disable=SC2034  # JUEGOS, DISCO_* y EN_MODO las lee la función que carga el eval
gestor_revisa() {  # juegos modo_juego ultimo → 0 si lo llamó
  (
    eval "$(sed -n '/^revisar_disco() {/,/^}/p' "$AQUI/bin/vortex-gestor")"
    declare -A JUEGOS=(); (($1)) && JUEGOS[x]=y
    EN_MODO=$2; modo_juego_activo() { ((EN_MODO)); }
    DISCO_REVISAR_S=600 DISCO_ULTIMO=$3 DISCO="$T/gestor-disco"
    printf '#!/bin/sh\n: >"%s/llamado"\n' "$T" >"$DISCO"; chmod +x "$DISCO"
    rm -f "$T/llamado"
    revisar_disco
    wait
    [[ -e "$T/llamado" ]]
  )
}
AHORA=$(date +%s)
probar "gestor: lo revisa" "gestor_revisa 0 0 0"
probar "gestor: con un juego abierto, no" "! gestor_revisa 1 0 0"
probar "gestor: en modo juego, no" "! gestor_revisa 0 1 0"
probar "gestor: no antes de 10 minutos" "! gestor_revisa 0 0 $((AHORA - 300))"
probar "gestor: lo llama con su ruta de siempre" "grep -q 'DISCO=\"\${VORTEX_GESTOR_DISCO:-vortex-disco}\"' '$AQUI/bin/vortex-gestor'"

probar "se instala con los demás comandos" "grep -q 'vortex-disco' '$AQUI/install/30-escritorio.sh' && grep -q 'COMANDOS_VORTEX=.*vortex-disco' '$AQUI/bin/vortex'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
