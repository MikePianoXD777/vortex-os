#!/usr/bin/env bash
# Pruebas de la integración del asistente Miku (vortex-miku) con el resto de Vortex, sin root, sin
# red, sin VM y sin tocar tu escritorio ni tu HOME: todo pasa en carpetas temporales con un
# vortex-miku, un makepkg y un notify-send de mentira.
#   1. sintaxis (bash -n) y shellcheck si está instalado;
#   2. «vortex miku …» le pasa todo a vortex-miku (argumentos, código de salida, sin argumentos) y,
#      si el comando falta, avisa en español con código 127; la ayuda lo lista;
#   3. la ISO: iso/paquetes.sh construye vortex-miku, iso/paquetes.txt lo trae instalado, y
#      iso/miku-modelo.sh (lo que usa construir.sh) pone el modelo 3D en la imagen: modelo.vrm y
#      CREDITO.txt en /usr/share/vortex-miku/modelo/ con modo 0644, avisa y sigue si falta el
#      modelo o el crédito, y rechaza lo que no es un VRM; el *.vrm nunca va a git;
#   4. install.sh: el paso install/65-miku.sh arma el paquete con makepkg -sif (en modo prueba solo
#      lo muestra), se puede omitir (--sin-miku) y, si falla, avisa y sigue;
#   5. el README habla del asistente, de Super+O y de los créditos del modelo.
# El interruptor de Ajustes tiene su propia prueba (pruebas/miku-qml.sh) y las reglas de la ventana
# y el atajo Super+O, las suyas (pruebas/atajos.sh).
#
#   pruebas/miku.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-miku-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
salto() { printf '  --  %s\n' "$*"; }
probar() { local d="$1"; shift; if "$@"; then bien "$d"; else mal "$d"; fi; }
contiene() { grep -qF -- "$2" "$1" 2>/dev/null; }

# ---------------------------------------------------------------------------------------------
echo "== Sintaxis"
for f in bin/vortex iso/miku-modelo.sh iso/construir.sh iso/paquetes.sh install.sh install/65-miku.sh \
  install/90-final.sh pruebas/miku.sh pruebas/miku-qml.sh; do
  probar "bash -n $f" bash -n "$AQUI/$f"
done
if command -v shellcheck >/dev/null; then
  probar "shellcheck" shellcheck -x -S warning "$AQUI/iso/miku-modelo.sh" "$AQUI/install/65-miku.sh" "$AQUI/pruebas/miku.sh" \
    "$AQUI/pruebas/miku-qml.sh"
else
  salto "shellcheck no está instalado: se salta"
fi

# ---------------------------------------------------------------------------------------------
echo "== vortex miku"
mkdir -p "$T/shim" "$T/shim-sin" "$T/home"
# vortex-miku de mentira: anota sus argumentos y sale con el código que le pidan.
cat >"$T/shim/vortex-miku" <<'EOF'
#!/usr/bin/env bash
printf 'args(%d):' "$#" >>"$T_LOG"
for a in "$@"; do printf ' [%s]' "$a" >>"$T_LOG"; done
printf '\n' >>"$T_LOG"
[[ "${1:-}" == falla ]] && exit 7
[[ "${1:-}" == estado ]] && { echo "avatar=on"; echo "abierta=no"; }
exit 0
EOF
chmod +x "$T/shim/vortex-miku"
# Un PATH sin nada más que lo que bin/vortex usa: así, aunque en este equipo ya esté instalado
# vortex-miku de verdad, la prueba de «falta el comando» no lo encuentra.
for c in bash dirname readlink sed cat; do ln -s "$(command -v "$c")" "$T/shim-sin/$c"; done
# en_casa PATH comando...: lo corre con un HOME temporal y ese PATH.
en_casa() { env -i HOME="$T/home" PATH="$1" T_LOG="$T/vortex-miku.log" "${@:2}"; }
CON="$T/shim:$T/shim-sin"
VORTEX="$AQUI/bin/vortex"

: >"$T/vortex-miku.log"
en_casa "$CON" "$VORTEX" miku
probar "«vortex miku» sin argumentos llama a vortex-miku sin argumentos (alterna)" contiene "$T/vortex-miku.log" "args(0):"
: >"$T/vortex-miku.log"
en_casa "$CON" "$VORTEX" miku avatar off
probar "«vortex miku avatar off» pasa los argumentos tal cual" contiene "$T/vortex-miku.log" "args(2): [avatar] [off]"
: >"$T/vortex-miku.log"
en_casa "$CON" "$VORTEX" miku voz "una voz con espacios" >/dev/null
probar "los argumentos con espacios llegan enteros" contiene "$T/vortex-miku.log" "args(2): [voz] [una voz con espacios]"
salida="$(en_casa "$CON" "$VORTEX" miku estado)"
probar "la salida de vortex-miku llega tal cual" test "$salida" == $'avatar=on\nabierta=no'
en_casa "$CON" "$VORTEX" miku falla >/dev/null 2>&1
probar "el código de salida de vortex-miku se conserva (7)" test $? -eq 7
salida="$(en_casa "$T/shim-sin" "$VORTEX" miku 2>&1)"
rc=$?
probar "sin vortex-miku sale con 127" test "$rc" -eq 127
probar "…y avisa en español dónde está el paquete" bash -c '[[ "$1" == *"falta el asistente Miku (paquete vortex-miku)"* && "$1" == *"packaging/vortex-miku"* ]]' _ "$salida"
probar "la ayuda de vortex lista «miku» y Super+O" bash -c '"$1" ayuda | grep -q "miku \[avatar on|off|estado\].*Super+O"' _ "$VORTEX"

# ---------------------------------------------------------------------------------------------
echo "== ISO"
probar "iso/paquetes.sh construye vortex-miku (PROPIOS)" bash -c 'grep -E "^PROPIOS=\(.*\bvortex-miku\b" "$1" >/dev/null' _ "$AQUI/iso/paquetes.sh"
probar "iso/paquetes.txt trae vortex-miku instalado en la base" bash -c 'sed "s/#.*//" "$1" | grep -qx "vortex-miku"' _ "$AQUI/iso/paquetes.txt"
if [[ -f "$AQUI/packaging/vortex-miku/PKGBUILD" ]]; then
  nombres="$(bash -c 'source "$1" >/dev/null 2>&1; printf "%s\n" "${pkgname[@]}"' _ "$AQUI/packaging/vortex-miku/PKGBUILD")"
  probar "packaging/vortex-miku/PKGBUILD define el paquete vortex-miku" bash -c 'grep -qx vortex-miku <<<"$1"' _ "$nombres"
else
  salto "packaging/vortex-miku/PKGBUILD no está en esta rama (lo trae la rama de la app): no se revisa"
fi

# iso/miku-modelo.sh: lo que construir.sh usa para poner el modelo 3D en la imagen.
# shellcheck source=iso/miku-modelo.sh
source "$AQUI/iso/miku-modelo.sh"
vrm_falso() { printf 'glTF\x02\x00\x00\x00' >"$1"; head -c 4096 /dev/urandom >>"$1"; }
modo() { stat -c '%a' "$1"; }

mkdir -p "$T/modelo-bueno" "$T/modelo-sin-credito" "$T/modelo-malo" "$T/modelo-vacio"
vrm_falso "$T/modelo-bueno/modelo.vrm"
printf 'MIKU NAKANO por Vewertyg (de prueba)\n' >"$T/modelo-bueno/CREDITO.txt"
vrm_falso "$T/modelo-sin-credito/modelo.vrm"
printf 'esto no es un VRM\n' >"$T/modelo-malo/modelo.vrm"

raiz="$T/raiz1"
mkdir -p "$raiz"
VORTEX_MIKU_MODELO="$T/modelo-bueno" miku_poner_modelo "$raiz" >"$T/m1.out" 2>"$T/m1.err"
rc=$?
probar "con modelo y crédito: sale con 0" test "$rc" -eq 0
probar "copia el modelo a /usr/share/vortex-miku/modelo/modelo.vrm, idéntico" cmp -s "$T/modelo-bueno/modelo.vrm" "$raiz/usr/share/vortex-miku/modelo/modelo.vrm"
probar "copia el crédito a CREDITO.txt, idéntico" cmp -s "$T/modelo-bueno/CREDITO.txt" "$raiz/usr/share/vortex-miku/modelo/CREDITO.txt"
probar "el modelo y el crédito quedan 0644" test "$(modo "$raiz/usr/share/vortex-miku/modelo/modelo.vrm")/$(modo "$raiz/usr/share/vortex-miku/modelo/CREDITO.txt")" == 644/644
probar "anota los dos archivos para los permisos de profiledef.sh (0:0:644)" test "${MIKU_ARCHIVOS[*]}" == "/usr/share/vortex-miku/modelo/modelo.vrm /usr/share/vortex-miku/modelo/CREDITO.txt"
probar "sin avisos" test ! -s "$T/m1.err"
# Con un modo raro en el origen (0600), igual queda 0644 en la imagen.
chmod 600 "$T/modelo-bueno/modelo.vrm"
raiz="$T/raiz1b"
mkdir -p "$raiz"
VORTEX_MIKU_MODELO="$T/modelo-bueno" miku_poner_modelo "$raiz" >/dev/null 2>&1
probar "aunque el original sea 0600, en la imagen queda 0644" test "$(modo "$raiz/usr/share/vortex-miku/modelo/modelo.vrm")" == 644

raiz="$T/raiz2"
mkdir -p "$raiz"
VORTEX_MIKU_MODELO="$T/modelo-sin-credito" miku_poner_modelo "$raiz" >"$T/m2.out" 2>"$T/m2.err"
rc=$?
probar "sin CREDITO.txt: copia el modelo, avisa del crédito y sale con 0" bash -c '[[ "$1" -eq 0 && -f "$2/usr/share/vortex-miku/modelo/modelo.vrm" && ! -e "$2/usr/share/vortex-miku/modelo/CREDITO.txt" ]] && grep -q "CREDITO.txt" "$3"' _ "$rc" "$raiz" "$T/m2.err"
probar "…y solo anota el modelo" test "${MIKU_ARCHIVOS[*]}" == "/usr/share/vortex-miku/modelo/modelo.vrm"

raiz="$T/raiz3"
mkdir -p "$raiz"
VORTEX_MIKU_MODELO="$T/modelo-vacio" miku_poner_modelo "$raiz" >"$T/m3.out" 2>"$T/m3.err"
rc=$?
probar "carpeta sin modelo: no copia nada, avisa y sale con 0 (la ISO sigue)" bash -c '[[ "$1" -eq 0 && ! -e "$2/usr/share/vortex-miku" ]] && grep -q "sin el modelo 3D de Miku" "$3"' _ "$rc" "$raiz" "$T/m3.err"
probar "…y no anota nada" test "${#MIKU_ARCHIVOS[@]}" -eq 0

raiz="$T/raiz4"
mkdir -p "$raiz"
VORTEX_MIKU_MODELO="$T/modelo-malo" miku_poner_modelo "$raiz" >"$T/m4.out" 2>"$T/m4.err"
rc=$?
probar "un archivo que no es VRM (no empieza con glTF) se rechaza con aviso y sale con 0" bash -c '[[ "$1" -eq 0 && ! -e "$2/usr/share/vortex-miku" ]] && grep -q "no es un modelo VRM" "$3"' _ "$rc" "$raiz" "$T/m4.err"

raiz="$T/raiz5"
mkdir -p "$raiz"
VORTEX_MIKU_MODELO="" miku_poner_modelo "$raiz" >"$T/m5.out" 2>"$T/m5.err"
rc=$?
probar "VORTEX_MIKU_MODELO vacía: la ISO sale sin modelo, avisando" bash -c '[[ "$1" -eq 0 && ! -e "$2/usr/share/vortex-miku" ]] && grep -q "ninguna carpeta" "$3"' _ "$rc" "$raiz" "$T/m5.err"

# Sin la variable, la carpeta es la de quien corre sudo (~/.local/share/vortex/miku). Con root no
# existe ahí nada, así que no se toca ningún modelo real; solo se mira de dónde lo buscaría.
raiz="$T/raiz6"
mkdir -p "$raiz"
env -u VORTEX_MIKU_MODELO SUDO_USER=root bash -c 'source "$1"; miku_poner_modelo "$2"' _ "$AQUI/iso/miku-modelo.sh" "$raiz" >"$T/m6.out" 2>"$T/m6.err"
probar "sin la variable y con SUDO_USER=root busca en ~root/.local/share/vortex/miku" bash -c 'grep -qF "$(getent passwd root | cut -d: -f6)/.local/share/vortex/miku" "$1"' _ "$T/m6.err"
raiz="$T/raiz7"
mkdir -p "$raiz"
env -u VORTEX_MIKU_MODELO -u SUDO_USER bash -c 'source "$1"; miku_poner_modelo "$2"' _ "$AQUI/iso/miku-modelo.sh" "$raiz" >"$T/m7.out" 2>"$T/m7.err"
probar "sin variable ni sudo no inventa una carpeta: avisa y sigue" bash -c 'grep -q "ninguna carpeta" "$1"' _ "$T/m7.err"

probar "construir.sh carga iso/miku-modelo.sh y pone el modelo antes de mkarchiso" bash -c '
  c="$1/iso/construir.sh"
  a="$(grep -n "^source \"\$AQUI/miku-modelo.sh\"" "$c" | cut -d: -f1)"
  b="$(grep -n "^miku_poner_modelo \"\$raiz\"" "$c" | cut -d: -f1)"
  m="$(grep -n "^mkarchiso " "$c" | cut -d: -f1)"
  [[ -n "$a" && -n "$b" && -n "$m" ]] && ((a < b && b < m))' _ "$AQUI"
probar "construir.sh deja los archivos del modelo 0:0:644 en profiledef.sh" contiene "$AQUI/iso/construir.sh" 'file_permissions+=([\"$archivo\"]=\"0:0:644\")'
# El bucle de construir.sh que escribe los permisos en profiledef.sh, ejecutado tal cual, y el
# resultado cargado como lo hace mkarchiso (file_permissions es un arreglo asociativo).
perfil="$T/perfil"
mkdir -p "$perfil"
printf 'file_permissions=(\n  ["/usr/local/bin/"]="0:0:755"\n)\n' >"$perfil/profiledef.sh"
MIKU_ARCHIVOS=(/usr/share/vortex-miku/modelo/modelo.vrm /usr/share/vortex-miku/modelo/CREDITO.txt)
eval "$(sed -n '/^for archivo in "\${MIKU_ARCHIVOS\[@\]}"; do$/,/^done$/p' "$AQUI/iso/construir.sh")"
permisos="$(bash -c 'declare -A file_permissions; source "$1"; for k in "${!file_permissions[@]}"; do echo "$k=${file_permissions[$k]}"; done | sort' _ "$perfil/profiledef.sh")"
probar "profiledef.sh queda con el modelo y el crédito en 0:0:644 y lo anterior intacto" \
  test "$permisos" == $'/usr/local/bin/=0:0:755\n/usr/share/vortex-miku/modelo/CREDITO.txt=0:0:644\n/usr/share/vortex-miku/modelo/modelo.vrm=0:0:644'
probar "ningún .vrm está en git" bash -c '[[ -z "$(git -C "$1" ls-files | grep -i "\.vrm$")" ]]' _ "$AQUI"
probar "*.vrm está en .gitignore (git no lo agrega)" git -C "$AQUI" check-ignore -q packaging/vortex-miku/modelo.vrm

# ---------------------------------------------------------------------------------------------
echo "== install.sh y el paso de Miku"
probar "install.sh entiende --sin-miku (y lo dice en su ayuda)" bash -c '"$1/install.sh" --ayuda | grep -q -e "--sin-miku" && grep -q -e "--sin-miku) VORTEX_MIKU=0" "$1/install.sh"' _ "$AQUI"
probar "install.sh exporta VORTEX_MIKU a los pasos" bash -c 'grep -E "^export .*\bVORTEX_MIKU\b" "$1/install.sh" >/dev/null' _ "$AQUI"
probar "install.sh --ayuda ya no imprime la línea «set -Eeuo»" bash -c '! "$1/install.sh" --ayuda | grep -q "^set "' _ "$AQUI"

# El paso, con un repo falso, un makepkg de mentira (anota dónde y cómo corrió) y un HOME temporal.
mkdir -p "$T/repo/packaging/vortex-miku" "$T/bin-makepkg" "$T/home-install"
cat >"$T/bin-makepkg/makepkg" <<'EOF'
#!/usr/bin/env bash
echo "makepkg en $PWD: $*" >>"$T_LOG"
[[ -e "$T_FALLA" ]] && exit 1
exit 0
EOF
chmod +x "$T/bin-makepkg/makepkg"
# paso_miku "VAR=valor ..." → salida del paso 65-miku.sh con lib.sh (en modo no interactivo: --si).
paso_miku() {
  env -i HOME="$T/home-install" PATH="$T/bin-makepkg:/usr/bin:/bin" T_LOG="$T/install.log" T_FALLA="$T/falla" \
    VORTEX_DIR="$T/repo" VORTEX_SI=1 VORTEX_LOG="$T/install-lib.log" "$@" \
    bash -c 'source "$1/install/lib.sh"; source "$1/install/65-miku.sh"; echo "sigue: rc=$?"' _ "$AQUI" 2>&1
}

: >"$T/install.log"
salida="$(paso_miku VORTEX_MIKU=1 VORTEX_DRY_RUN=0)"
probar "con Miku elegida corre «makepkg -sif --noconfirm» en packaging/vortex-miku" contiene "$T/install.log" "makepkg en $T/repo/packaging/vortex-miku: -sif --noconfirm"
probar "…dice que quedó instalada y explica Super+O" bash -c '[[ "$1" == *"Miku instalada"* && "$1" == *"Super+O"* && "$1" == *"sigue: rc=0"* ]]' _ "$salida"

: >"$T/install.log"
salida="$(paso_miku VORTEX_MIKU=1 VORTEX_DRY_RUN=1)"
probar "en modo prueba (--prueba) solo muestra el comando, no lo corre" bash -c '[[ "$1" == *"makepkg -sif --noconfirm"* && "$1" == *"packaging/vortex-miku"* && ! -s "$2" ]]' _ "$salida" "$T/install.log"

: >"$T/install.log"
salida="$(paso_miku VORTEX_MIKU=0 VORTEX_DRY_RUN=0)"
probar "con --sin-miku se omite y no corre makepkg" bash -c '[[ "$1" == *"Miku: omitida"* && ! -s "$2" ]]' _ "$salida" "$T/install.log"

: >"$T/install.log"
touch "$T/falla"
# Sin esperar los 10 s entre reintentos ni tocar pacman: sleep y sudo de mentira.
printf '#!/usr/bin/env bash\nexit 0\n' >"$T/bin-makepkg/sleep"
printf '#!/usr/bin/env bash\nexit 0\n' >"$T/bin-makepkg/sudo"
chmod +x "$T/bin-makepkg/sleep" "$T/bin-makepkg/sudo"
salida="$(paso_miku VORTEX_MIKU=1 VORTEX_DRY_RUN=0)"
rm -f "$T/falla"
probar "si makepkg falla, avisa cómo reintentar y la instalación sigue (rc=0)" bash -c '[[ "$1" == *"No se pudo instalar a Miku"* && "$1" == *"makepkg -sif"* && "$1" == *"sigue: rc=0"* ]]' _ "$salida"

# ---------------------------------------------------------------------------------------------
echo "== README"
probar "el README habla de Super+O y del interruptor de Ajustes" bash -c 'grep -q "Super+O" "$1" && grep -q "Miku en 3D" "$1"' _ "$AQUI/README.md"
probar "…y da el crédito del modelo (autor y VRoid Hub)" bash -c 'grep -q "Vewertyg" "$1" && grep -qi "vroid" "$1"' _ "$AQUI/README.md"
probar "…y de three.js y three-vrm" bash -c 'grep -q "three.js" "$1" && grep -q "three-vrm" "$1"' _ "$AQUI/README.md"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS pruebas"
  exit 1
fi
echo "Todo bien"
