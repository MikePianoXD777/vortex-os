#!/usr/bin/env bash
# Pruebas de las piezas de la ISO (iso/, Calamares, Claude por usuario), sin construir la ISO, sin
# root, sin red y sin tocar tu HOME ni tu sistema: todo pasa en carpetas temporales. La única
# salvedad es el YAML de Calamares, que necesita pyyaml: si no está (python3) ni se puede traer con
# uv, esa prueba se salta, no falla.
#   1. sintaxis (bash -n) y YAML de Calamares (pyyaml), y que los paquetes de la lista existan;
#   2. «vortex claude instalar» (install/claude.sh) con un instalador y un «claude» de mentira:
#      lo instala, registra el MCP, deja la voz sin pisar ajustes, es idempotente, reintenta, y
#      avisa con una notificación (también si el bus de la sesión aún no está en el entorno);
#      «vortex claude» (Super+A) sin Claude instalado lo termina de instalar en la terminal;
#   3. vortex-sesion: lanza esa instalación en segundo plano solo con la marca /etc/vortex/claude
#      (/etc se simula con un overlay en un user namespace);
#   4. iso/paquetes.sh: no recompila un paquete del AUR si el repo ya tiene esa versión, ni si lo
#      compilado sigue enlazando con las bibliotecas del sistema (binarios de verdad, con cc);
#   5. vortex-post-instalacion: el autologin de Calamares pasa a greetd ([initial_session]);
#   6. el hook de pacman que activa RemoveMake en paru, con pacman de verdad en una raíz falsa.
#
#   pruebas/iso.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-iso-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
salto() { printf '  --  %s\n' "$*"; }
probar() { local d="$1"; shift; if "$@"; then bien "$d"; else mal "$d"; fi; }
# El archivo contiene el texto (literal, no regex).
contiene() { grep -qF -- "$2" "$1" 2>/dev/null; }
lineas() { local n; n="$(grep -c -- "$2" "$1" 2>/dev/null || true)"; [[ "${n:-0}" == "$3" ]]; }

# Entorno limpio para correr comandos de Vortex con otro HOME y un PATH sin tu «claude» de verdad.
en_casa() { # en_casa HOME [VAR=valor ...] -- comando...
  local casa="$1"; shift
  local extra=()
  while [[ "$1" != -- ]]; do extra+=("$1"); shift; done
  shift
  env -u XDG_STATE_HOME -u XDG_CONFIG_HOME -u XDG_CACHE_HOME -u XDG_DATA_HOME -u TMPDIR -u HYPRLAND_INSTANCE_SIGNATURE \
    -u DBUS_SESSION_BUS_ADDRESS -u TERMINAL \
    HOME="$casa" PATH="$T/shim:/usr/bin:/bin" "${extra[@]}" "$@"
}

# ---------------------------------------------------------------------------------------------
echo "== Sintaxis y YAML"
for f in bin/vortex bin/vortex-sesion install/claude.sh install/60-claude.sh iso/paquetes.sh iso/construir.sh \
  iso/airootfs/usr/local/bin/vortex-post-instalacion iso/airootfs/usr/local/bin/vortex-live \
  iso/airootfs/usr/local/bin/vortex-instalador pruebas/iso.sh; do
  probar "bash -n $f" bash -n "$AQUI/$f"
done
if command -v shellcheck >/dev/null; then
  probar "shellcheck" shellcheck -x -S warning "$AQUI/bin/vortex-sesion" "$AQUI/install/claude.sh" "$AQUI/iso/paquetes.sh" \
    "$AQUI/iso/airootfs/usr/local/bin/vortex-post-instalacion" "$AQUI/pruebas/iso.sh"
else
  salto "shellcheck no está instalado: se salta"
fi

cat >"$T/yaml.py" <<'PY'
import glob, os, re, subprocess, sys
import yaml
raiz = sys.argv[1]
mal = []
# Todo lo de Calamares es YAML.
for f in sorted(glob.glob(raiz + "/iso/airootfs/etc/calamares/**/*.conf", recursive=True)
                + glob.glob(raiz + "/iso/airootfs/etc/calamares/**/*.yaml", recursive=True)
                + glob.glob(raiz + "/iso/airootfs/etc/calamares/branding/vortex/branding.desc")):
    try:
        yaml.safe_load(open(f, encoding="utf-8"))
    except Exception as e:
        mal.append(f"YAML roto: {f}: {e}")
grupos = yaml.safe_load(open(raiz + "/iso/airootfs/etc/calamares/modules/netinstall_vortex.yaml", encoding="utf-8"))
subs = [s for g in grupos for s in g.get("subgroups", [])]
claude = [s for s in subs if s["name"].startswith("Claude")]
if len(claude) != 1:
    mal.append("tiene que haber una sola app «Claude» en netinstall_vortex.yaml")
else:
    c = claude[0]
    if c["name"] != "Claude (asistente, Super+A, le hablas con el micrófono)":
        mal.append("nombre de Claude: " + c["name"])
    if c.get("selected") is not True:
        mal.append("Claude debe ir marcado por defecto (selected: true)")
    if c.get("packages") != ["vortex-claude"]:
        mal.append("Claude debe instalar vortex-claude: %r" % c.get("packages"))
# Los paquetes vortex-* de la lista salen de algún PKGBUILD de packaging/.
nombres = set()
for pkgbuild in glob.glob(raiz + "/packaging/*/PKGBUILD"):
    out = subprocess.run(["bash", "-c", 'source "$1" >/dev/null 2>&1; printf "%s\\n" "${pkgname[@]}"', "_", pkgbuild],
                         capture_output=True, text=True).stdout
    nombres.update(out.split())
for s in subs:
    for p in s.get("packages", []):
        if p.startswith("vortex-") and p not in nombres:
            mal.append(f"{s['name']}: el paquete {p} no sale de ningún PKGBUILD de packaging/")
u = yaml.safe_load(open(raiz + "/iso/airootfs/etc/calamares/modules/users_vortex.conf", encoding="utf-8"))
if u.get("displayAutologin") is not True:
    mal.append("users_vortex.conf: la casilla de inicio automático debe verse (displayAutologin: true)")
if u.get("doAutologin") is not False:
    mal.append("users_vortex.conf: la casilla debe empezar sin marcar (doAutologin: false)")
if u.get("autologinGroup") != "autologin":
    mal.append("users_vortex.conf: autologinGroup debe ser autologin")
print("\n".join(mal))
sys.exit(1 if mal else 0)
PY
# Con python3 + pyyaml no hace falta red; si no, uv lo trae (y se necesita red la primera vez). Si
# no hay de dónde sacar pyyaml no es un error del código: esta prueba se salta.
python_yaml=()
if python3 -I -c 'import yaml' 2>/dev/null; then
  python_yaml=(python3 -I)
elif command -v uv >/dev/null && timeout 90 uv run --no-project --with pyyaml python -I -c 'import yaml' >/dev/null 2>&1; then
  python_yaml=(uv run --no-project --with pyyaml python -I)
fi
if ((${#python_yaml[@]})); then
  yaml_rc=1
  "${python_yaml[@]}" "$T/yaml.py" "$AQUI" >"$T/yaml.out" 2>&1 && yaml_rc=0
  probar "YAML de Calamares válido; Claude marcado por defecto; paquetes vortex-* existen; casilla de autologin" test "$yaml_rc" -eq 0
  sed 's/^/      /' "$T/yaml.out"
else
  salto "sin pyyaml (ni uv con red para traerlo): se salta la validación del YAML de Calamares"
fi

# ---------------------------------------------------------------------------------------------
echo "== vortex claude instalar"
mkdir -p "$T/shim" "$T/claude"
# Instalador de mentira: deja un «claude» de mentira en ~/.local/bin y cuenta cuántas veces corrió.
cat >"$T/claude/install.sh" <<'EOF'
mkdir -p "$HOME/.local/bin"
cat >"$HOME/.local/bin/claude" <<'FIN'
#!/usr/bin/env bash
echo "$*" >>"$HOME/claude-args.log"
case "$1 $2" in
  "mcp get") [[ -f "$HOME/mcp-vortex" ]] ;;
  "mcp add") touch "$HOME/mcp-vortex" ;;
esac
FIN
chmod +x "$HOME/.local/bin/claude"
echo x >>"$HOME/instalaciones"
EOF
cat >"$T/shim/notify-send" <<'EOF'
#!/usr/bin/env bash
echo "$*" >>"$HOME/notificaciones.log"
echo "${DBUS_SESSION_BUS_ADDRESS:-}" >>"$HOME/notificaciones-bus.log"
EOF
chmod +x "$T/shim/notify-send"
URL_BUENA="file://$T/claude/install.sh"
VORTEX="$AQUI/bin/vortex"
RAPIDO=(VORTEX_CLAUDE_ESPERA=0 VORTEX_CLAUDE_INTENTOS_RED=2 VORTEX_CLAUDE_PAUSA_RED=0)

H="$T/c1"; mkdir -p "$H"
en_casa "$H" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar >"$T/c1.out" 2>&1
probar "instalar: termina bien" test $? -eq 0
probar "instalar: deja claude en ~/.local/bin" test -x "$H/.local/bin/claude"
probar "instalar: registra el MCP «vortex» (alcance user)" contiene "$H/claude-args.log" "mcp add --scope user vortex -- /usr/local/bin/vortex-mcp"
probar "instalar: voz en tap y español" jq -e '.voice == {"enabled": true, "mode": "tap"} and .language == "spanish"' "$H/.claude/settings.json" >/dev/null
probar "instalar: deja la marca claude-listo" test -f "$H/.local/state/vortex/claude-listo"
en_casa "$H" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar >"$T/c1b.out" 2>&1
probar "repetir: no vuelve a correr el instalador" lineas "$H/instalaciones" x 1
probar "repetir: no vuelve a registrar el MCP" lineas "$H/claude-args.log" "mcp add" 1
probar "repetir: lo dice" contiene "$T/c1b.out" "ya está instalado"

H="$T/c2"; mkdir -p "$H/.claude"
echo '{"language":"french","otro":1}' >"$H/.claude/settings.json"
en_casa "$H" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar >"$T/c2.out" 2>&1
probar "ajustes de Claude: respeta el idioma que ya había" jq -e '.language == "french" and .otro == 1 and .voice.mode == "tap"' "$H/.claude/settings.json" >/dev/null
H="$T/c2b"; mkdir -p "$H/.claude"
echo '{roto' >"$H/.claude/settings.json"
en_casa "$H" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar >"$T/c2b.out" 2>&1
probar "ajustes de Claude: JSON roto no se pisa y solo avisa" contiene "$H/.claude/settings.json" "{roto"
probar "ajustes de Claude: JSON roto no impide la marca" test -f "$H/.local/state/vortex/claude-listo"

H="$T/c3"; mkdir -p "$H"
en_casa "$H" VORTEX_CLAUDE_INSTALADOR="file://$T/no-existe.sh" -- "$VORTEX" claude instalar >"$T/c3.out" 2>&1
probar "sin red: sale con error" test $? -ne 0
probar "sin red: no deja la marca ni claude" test ! -e "$H/.local/state/vortex/claude-listo" -a ! -e "$H/.local/bin/claude"

H="$T/c4"; mkdir -p "$H"
en_casa "$H" "${RAPIDO[@]}" VORTEX_CLAUDE_INSTALADOR="file://$T/no-existe.sh" -- "$VORTEX" claude instalar --al-entrar >"$T/c4.out" 2>&1
probar "--al-entrar sin red: sale bien, avisa y no deja marca" test $? -eq 0 -a ! -e "$H/.local/state/vortex/claude-listo" -a ! -e "$H/.local/bin/claude"
probar "--al-entrar sin red: dice que reintenta en el próximo inicio" contiene "$T/c4.out" "próximo inicio"

# El bus de usuario de systemd (lo que ve la sesión): un socket en $XDG_RUNTIME_DIR/bus. Se crea el
# archivo-socket y basta; nadie le habla.
mkdir -p "$T/rt" "$T/sin-bus"
python3 -I -c 'import socket, sys; socket.socket(socket.AF_UNIX).bind(sys.argv[1])' "$T/rt/bus" 2>/dev/null

H="$T/c5"; mkdir -p "$H"
en_casa "$H" "${RAPIDO[@]}" XDG_RUNTIME_DIR="$T/rt" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar --al-entrar >"$T/c5.out" 2>&1
probar "--al-entrar con red: instala y deja la marca" test $? -eq 0 -a -x "$H/.local/bin/claude" -a -f "$H/.local/state/vortex/claude-listo"
probar "--al-entrar con red: avisa con una notificación" contiene "$H/notificaciones.log" "Claude está listo"
if [[ -S "$T/rt/bus" ]]; then
  # vortex-sesion lo lanza antes de Hyprland: sin DBUS_SESSION_BUS_ADDRESS en el entorno.
  probar "la notificación encuentra el bus de la sesión aunque el entorno no lo traiga" contiene "$H/notificaciones-bus.log" "unix:path=$T/rt/bus"
  H="$T/c5b"; mkdir -p "$H"
  en_casa "$H" "${RAPIDO[@]}" XDG_RUNTIME_DIR="$T/rt" DBUS_SESSION_BUS_ADDRESS=unix:path=/otro/bus VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- \
    "$VORTEX" claude instalar --al-entrar >"$T/c5b.out" 2>&1
  probar "si el entorno ya trae el bus, no lo cambia" contiene "$H/notificaciones-bus.log" "unix:path=/otro/bus"
else
  salto "no se pudo crear un socket de prueba: se salta la prueba del bus de la notificación"
fi
H="$T/c5c"; mkdir -p "$H"
en_casa "$H" "${RAPIDO[@]}" XDG_RUNTIME_DIR="$T/sin-bus" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar --al-entrar >"$T/c5c.out" 2>&1
probar "sin bus de sesión: avisa igual y no inventa uno" test "$(head -n1 "$H/notificaciones-bus.log" 2>/dev/null || echo x)" = "" -a -s "$H/notificaciones.log"

H="$T/c6"; mkdir -p "$H/.local/state/vortex"
( exec 9>"$H/.local/state/vortex/claude.lock"; flock 9; sleep 4 ) &
sleep 0.5
inicio=$SECONDS
en_casa "$H" "${RAPIDO[@]}" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar --al-entrar >"$T/c6.out" 2>&1
probar "candado: con otra instalación en curso, --al-entrar no hace nada" test $? -eq 0 -a $((SECONDS - inicio)) -lt 3 -a ! -e "$H/.local/bin/claude" -a ! -e "$H/.local/state/vortex/claude-listo"
wait

H="$T/c8"; mkdir -p "$H/.local/state/vortex"
( exec 9>"$H/.local/state/vortex/claude.lock"; flock 9; sleep 2 ) &
sleep 0.5
en_casa "$H" VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar >"$T/c8.out" 2>&1
probar "a mano con otra instalación en curso: avisa, espera y termina" test $? -eq 0 -a -f "$H/.local/state/vortex/claude-listo"
probar "a mano con otra instalación en curso: lo dice" contiene "$T/c8.out" "Ya hay otra instalación de Claude en curso"
wait

H="$T/c7"; mkdir -p "$H"
en_casa "$H" VORTEX_DRY_RUN=1 VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude instalar >"$T/c7.out" 2>&1
probar "modo prueba: no instala nada ni deja marca" test $? -eq 0 -a ! -e "$H/.local/bin/claude" -a ! -e "$H/.local/state/vortex/claude-listo"
probar "vortex ayuda: lista «claude instalar»" contiene <(en_casa "$T/c7" -- "$VORTEX" ayuda) "claude instalar"

# ---------------------------------------------------------------------------------------------
echo "== vortex claude (Super+A) sin Claude instalado"
# Terminales de mentira: corren de verdad lo que Vortex les pide (bash -lc ...), con la entrada vacía.
cat >"$T/shim/kitty" <<'EOF'
#!/usr/bin/env bash
echo "kitty $*" >>"$HOME/terminal.log"
shift 4   # --title TÍTULO --directory CARPETA
exec "$@"
EOF
cat >"$T/shim/terminal-falsa" <<'EOF'
#!/usr/bin/env bash
echo "terminal-falsa $*" >>"$HOME/terminal.log"
shift     # -e
exec "$@"
EOF
chmod +x "$T/shim/kitty" "$T/shim/terminal-falsa"

# Sin Claude (la ISO lo instala ~30 s después de entrar): la terminal lo instala y luego lo abre.
H="$T/a1"; mkdir -p "$H"
en_casa "$H" TERMINAL=kitty VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude </dev/null >"$T/a1.out" 2>&1
probar "sin Claude: la terminal lo instala (y deja la marca)" test -x "$H/.local/bin/claude" -a -f "$H/.local/state/vortex/claude-listo"
probar "sin Claude: la instalación corre una sola vez" lineas "$H/instalaciones" x 1
probar "sin Claude: después de instalar lo abre" lineas "$H/claude-args.log" '^$' 1
probar "sin Claude: le dice a la persona que se está instalando" contiene "$T/a1.out" "Claude todavía no está instalado"
probar "sin Claude: no queda en «orden no encontrada»" bash -c "! grep -qiE 'no encontrada|not found' '$T/a1.out'"
probar "sin Claude: abre kitty con el título de siempre" contiene "$H/terminal.log" "kitty --title Claude · Vortex --directory $H bash -lc"
# Con otra terminal ($TERMINAL) pasa lo mismo.
H="$T/a2"; mkdir -p "$H"
en_casa "$H" TERMINAL=terminal-falsa VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude </dev/null >"$T/a2.out" 2>&1
probar "sin Claude, otra terminal: lo instala y lo abre" test -x "$H/.local/bin/claude" -a "$(grep -c '^$' "$H/claude-args.log" 2>/dev/null || true)" = 1
probar "sin Claude, otra terminal: se abrió con -e" contiene "$H/terminal.log" "terminal-falsa -e bash -lc"
# Sin red: lo dice en la terminal en vez de quedarse mudo, y no abre nada que no existe.
H="$T/a3"; mkdir -p "$H"
en_casa "$H" TERMINAL=kitty VORTEX_CLAUDE_INSTALADOR="file://$T/no-existe.sh" -- "$VORTEX" claude </dev/null >"$T/a3.out" 2>&1
probar "sin Claude y sin red: avisa que no pudo instalarlo" contiene "$T/a3.out" "No pude instalar Claude Code"
probar "sin Claude y sin red: no deja marca ni claude" test ! -e "$H/.local/state/vortex/claude-listo" -a ! -e "$H/.local/bin/claude"
probar "sin Claude y sin red: no queda en «orden no encontrada»" bash -c "! grep -qiE 'no encontrada|not found' '$T/a3.out'"
# Con Claude ya instalado: abre directo, sin instalar nada.
H="$T/a4"; mkdir -p "$H/.local/bin"
printf '#!/usr/bin/env bash\necho abrio >>"$HOME/claude-abrio"\n' >"$H/.local/bin/claude"; chmod +x "$H/.local/bin/claude"
en_casa "$H" TERMINAL=kitty VORTEX_CLAUDE_INSTALADOR="$URL_BUENA" -- "$VORTEX" claude </dev/null >"$T/a4.out" 2>&1
probar "con Claude instalado: lo abre" test -f "$H/claude-abrio"
probar "con Claude instalado: no instala nada" test ! -e "$H/instalaciones" -a ! -e "$H/.local/state/vortex/claude-listo"
probar "con Claude instalado: no avisa de ninguna instalación" bash -c "! grep -q 'todavía no está instalado' '$T/a4.out'"

# ---------------------------------------------------------------------------------------------
echo "== vortex-sesion lanza Claude en segundo plano"
# /etc se simula con un overlay en un user namespace: la marca /etc/vortex/claude solo existe ahí.
cat >"$T/shim-sesion-vortex" <<'EOF'
#!/usr/bin/env bash
echo "vortex $*" >>"$HOME/llamadas.log"
if [[ "$1 $2" == "claude instalar" ]]; then sleep 1; echo "vortex-fin" >>"$HOME/llamadas.log"; fi
EOF
cat >"$T/shim-sesion-hypr" <<'EOF'
#!/usr/bin/env bash
echo "start-hyprland" >>"$HOME/llamadas.log"
EOF
mkdir -p "$T/sesion/bin" "$T/ov/up" "$T/ov/wk"
cp "$T/shim-sesion-vortex" "$T/sesion/bin/vortex"; cp "$T/shim-sesion-hypr" "$T/sesion/bin/start-hyprland"
chmod +x "$T/sesion/bin/"*
# Dentro del namespace: caso 1 con marca, caso 2 con marca y claude-listo, caso 3 sin marca.
cat >"$T/sesion/dentro.sh" <<EOF
set -u
mount -t overlay overlay -o lowerdir=/etc,upperdir=$T/ov/up,workdir=$T/ov/wk /etc || exit 3
correr() { # correr HOME
  mkdir -p "\$1/.config/vortex"; touch "\$1/.config/vortex/version"   # sin «configurar-usuario»
  env -u XDG_STATE_HOME -u XDG_CONFIG_HOME HOME="\$1" PATH="$T/sesion/bin:/usr/bin:/bin" bash "$AQUI/bin/vortex-sesion"
  sleep 1.6
}
mkdir -p /etc/vortex && touch /etc/vortex/claude
mkdir -p $T/s1; correr $T/s1
mkdir -p $T/s2/.local/state/vortex; touch $T/s2/.local/state/vortex/claude-listo; correr $T/s2
rm -f /etc/vortex/claude
mkdir -p $T/s3; correr $T/s3
EOF
if unshare -rm bash "$T/sesion/dentro.sh" >"$T/sesion.out" 2>&1; then
  probar "con la marca y sin claude-listo: corre «vortex claude instalar --al-entrar»" contiene "$T/s1/llamadas.log" "vortex claude instalar --al-entrar"
  probar "no retrasa el escritorio: start-hyprland entra antes de que termine la instalación" \
    bash -c "grep -n . '$T/s1/llamadas.log' | grep -E 'start-hyprland|vortex-fin' | head -n1 | grep -q start-hyprland"
  probar "su registro queda en ~/.local/state/vortex/claude.log" test -e "$T/s1/.local/state/vortex/claude.log"
  probar "con claude-listo: no vuelve a intentarlo" bash -c "! grep -q 'claude instalar' '$T/s2/llamadas.log'"
  probar "con claude-listo: igual entra al escritorio" contiene "$T/s2/llamadas.log" "start-hyprland"
  probar "sin la marca /etc/vortex/claude: no instala Claude" bash -c "! grep -q 'claude instalar' '$T/s3/llamadas.log'"
  probar "sin la marca: igual entra al escritorio" contiene "$T/s3/llamadas.log" "start-hyprland"
else
  salto "no hay user namespaces con overlay: se salta vortex-sesion ($(head -n1 "$T/sesion.out"))"
fi
[[ -e /etc/vortex/claude ]] && mal "¡/etc/vortex/claude real existe en este equipo!" || bien "el /etc real quedó intacto"

# ---------------------------------------------------------------------------------------------
echo "== iso/paquetes.sh no recompila lo que ya está en el repo"
# paquetes.sh define su propio AQUI (la carpeta iso/): se restaura el de esta prueba.
AQUI_REPO="$AQUI"
# shellcheck source=iso/paquetes.sh
source "$AQUI/iso/paquetes.sh"
AQUI="$AQUI_REPO"
P="$T/p"; mkdir -p "$P/repo" "$P/aur"
nuevo_pkgbuild() { # nuevo_pkgbuild DIR PKGREL [DEPENDS] [pkgver()]
  mkdir -p "$1"
  {
    printf 'pkgname=falso\npkgver=1.0\npkgrel=%s\narch=(any)\ndepends=(%s)\n' "$2" "${3:-bash}"
    printf 'package() { install -Dm644 /dev/null "$pkgdir/usr/share/falso/%s"; }\n' "$2"
    [[ -z "${4:-}" ]] || printf 'pkgver() { echo 1.0; }\n'
  } >"$1/PKGBUILD"
}
armar() { # armar DIR [opciones de makepkg]
  local d="$1"; shift
  (cd "$d" && PKGDEST="$P/repo" BUILDDIR="$P/build" makepkg -f --nodeps "$@" >/dev/null 2>&1)
}
nuevo_pkgbuild "$P/f1" 1; armar "$P/f1"
probar "mismo PKGBUILD y el repo ya lo tiene: no se recompila" ya_en_repo "$P/f1" "$P/repo"
probar "VORTEX_PAQUETES_FORZAR=1 obliga a recompilar" bash -c "VORTEX_PAQUETES_FORZAR=1; source '$AQUI/iso/paquetes.sh'; ! ya_en_repo '$P/f1' '$P/repo'"
nuevo_pkgbuild "$P/f2" 2
probar "el AUR trae otro pkgrel: sí se compila" bash -c "source '$AQUI/iso/paquetes.sh'; ! ya_en_repo '$P/f2' '$P/repo'"
nuevo_pkgbuild "$P/f3" 1 bash pkgver
probar "con pkgver() (paquetes -git) siempre se compila" bash -c "source '$AQUI/iso/paquetes.sh'; ! ya_en_repo '$P/f3' '$P/repo'"
mkdir -p "$P/repo-vacio"
probar "repo sin el archivo: se compila" bash -c "source '$AQUI/iso/paquetes.sh'; ! ya_en_repo '$P/f1' '$P/repo-vacio'"
# Compilado contra una biblioteca que el sistema ya cambió (soname distinto): no se reutiliza.
rm -rf "$P/repo2"; mkdir -p "$P/repo2"
nuevo_pkgbuild "$P/f4" 1 "bash libvortex-que-no-existe.so=9-64"
(cd "$P/f4" && PKGDEST="$P/repo2" BUILDDIR="$P/build" makepkg -f --nodeps >/dev/null 2>&1)
probar "sus dependencias ya no se cumplen (soname cambiado): se compila" bash -c "source '$AQUI/iso/paquetes.sh'; ! ya_en_repo '$P/f4' '$P/repo2'"

# El caso real: un makepkg normal NO escribe sonames en el .PKGINFO (solo lo que el PKGBUILD declare,
# p. ej. boost-libs sin versión), así que pacman -T no se entera de que el soname cambió. Se prueba con
# binarios de verdad: un programa que pide libvortex-prueba.so.9, con la biblioteca dentro del paquete
# (se reutiliza) o sin ella en el sistema (como un Calamares contra un boost de otro soname: se compila).
if command -v cc >/dev/null && command -v readelf >/dev/null; then
  C="$P/c"; mkdir -p "$C"
  echo 'int vortex_prueba(void) { return 7; }' >"$C/lib.c"
  echo 'int vortex_prueba(void); int main(void) { return vortex_prueba() == 7 ? 0 : 1; }' >"$C/prog.c"
  cc -shared -fPIC -Wl,-soname,libvortex-prueba.so.9 -o "$C/libvortex-prueba.so.9" "$C/lib.c" \
    && cc -o "$C/prog" "$C/prog.c" "$C/libvortex-prueba.so.9" 2>/dev/null
  probar "(el programa de prueba pide libvortex-prueba.so.9)" bash -c "readelf -d '$C/prog' | grep -q 'NEEDED.*libvortex-prueba.so.9'"
  elf_pkgbuild() { # elf_pkgbuild DIR [con-biblioteca]: depende de «bash», como un PKGBUILD cualquiera
    mkdir -p "$1"; cp "$C/prog" "$1/"
    {
      printf 'pkgname=falso\npkgver=1.0\npkgrel=1\narch=(any)\ndepends=(bash)\noptions=(!strip !debug)\n'
      printf 'package() {\n  install -Dm755 "$startdir/prog" "$pkgdir/usr/bin/falso-prog"\n'
      if [[ "${2:-}" == con-biblioteca ]]; then
        cp "$C/libvortex-prueba.so.9" "$1/"
        printf '  install -Dm755 "$startdir/libvortex-prueba.so.9" "$pkgdir/usr/lib/libvortex-prueba.so.9"\n'
      fi
      printf '}\n'
    } >"$1/PKGBUILD"
  }
  rm -rf "$P/repo3" "$P/repo4"; mkdir -p "$P/repo3" "$P/repo4"
  elf_pkgbuild "$P/e1" con-biblioteca
  (cd "$P/e1" && PKGDEST="$P/repo3" BUILDDIR="$P/build" makepkg -f --nodeps >/dev/null 2>&1)
  elf_pkgbuild "$P/e2"
  (cd "$P/e2" && PKGDEST="$P/repo4" BUILDDIR="$P/build" makepkg -f --nodeps >/dev/null 2>&1)
  probar "(su .PKGINFO no declara el soname: pacman -T no lo vería)" bash -c "! bsdtar -xOf '$P'/repo4/falso-1.0-1-any.pkg.tar.zst .PKGINFO | grep -q libvortex-prueba"
  probar "(y pacman -T de sus dependencias pasa)" bash -c "bsdtar -xOf '$P'/repo4/falso-1.0-1-any.pkg.tar.zst .PKGINFO | sed -n 's/^depend = //p' | xargs pacman -T >/dev/null"
  probar "pide una biblioteca que trae el propio paquete: no hace falta compilar" bash -c "source '$AQUI/iso/paquetes.sh'; ya_en_repo '$P/e1' '$P/repo3'"
  probar "pide una biblioteca que el sistema ya no tiene (soname cambiado, sin sonames en el .PKGINFO): se compila" \
    bash -c "source '$AQUI/iso/paquetes.sh'; ! ya_en_repo '$P/e2' '$P/repo4'"
else
  salto "sin compilador de C ni readelf: se salta la prueba de bibliotecas (NEEDED) de ya_en_repo"
fi

# construir_aur de punta a punta con un «AUR» local (repos git) y un solo paquete falso.
export GIT_AUTHOR_NAME=prueba GIT_AUTHOR_EMAIL=p@p GIT_COMMITTER_NAME=prueba GIT_COMMITTER_EMAIL=p@p
rm -rf "$P/repo" "$P/aur"; mkdir -p "$P/repo" "$P/aur/falso.git"
nuevo_pkgbuild "$P/aur/falso.git" 1
git -C "$P/aur/falso.git" init -q && git -C "$P/aur/falso.git" add PKGBUILD && git -C "$P/aur/falso.git" commit -q -m v1
aur_corre() { ( AUR=(falso); AUR_URL="file://$P/aur"; export PKGDEST="$P/repo" BUILDDIR="$P/build"; construir_aur "$P/repo" ); }
aur_corre >"$T/aur1.out" 2>&1
probar "AUR: la primera vez compila" test -f "$P/repo/falso-1.0-1-any.pkg.tar.zst"
primero="$(stat -c %Y "$P/repo/falso-1.0-1-any.pkg.tar.zst")"
sleep 1.1
aur_corre >"$T/aur2.out" 2>&1
probar "AUR: la segunda vez dice que no recompila" contiene "$T/aur2.out" "no se vuelve a compilar"
probar "AUR: y no toca el paquete" test "$(stat -c %Y "$P/repo/falso-1.0-1-any.pkg.tar.zst")" == "$primero"
sed -i 's/^pkgrel=1/pkgrel=2/' "$P/aur/falso.git/PKGBUILD"
git -C "$P/aur/falso.git" commit -q -am v2
aur_corre >"$T/aur3.out" 2>&1
probar "AUR: con una versión nueva sí compila" test -f "$P/repo/falso-1.0-2-any.pkg.tar.zst"

# ---------------------------------------------------------------------------------------------
echo "== vortex-post-instalacion: autologin de Calamares → greetd"
POST="$AQUI/iso/airootfs/usr/local/bin/vortex-post-instalacion"
YO="$(id -un)"
NEG=""
for g in root nobody daemon bin; do
  getent group "$g" >/dev/null && ! id -nG | tr ' ' '\n' | grep -qx "$g" && { NEG="$g"; break; }
done
if [[ ! "$YO" =~ ^[a-z_][a-z0-9_-]*$ || -z "$NEG" ]]; then
  salto "este usuario no sirve para la prueba ($YO)"
else
  base_greetd() { sed 's|start-hyprland </dev/null|vortex-sesion|' "$AQUI/config/sistema/greetd.toml"; }
  hacer_raiz() { rm -rf "$T/raiz"; mkdir -p "$T/raiz/etc/greetd"; base_greetd >"$T/raiz/etc/greetd/config.toml"; }
  # tomllib (Python 3.11+): el archivo sigue siendo TOML y dice lo que debe.
  toml() { python3 -I -c "import sys,tomllib; d=tomllib.load(open(sys.argv[1],'rb')); $2" "$T/raiz/etc/greetd/config.toml"; }
  auto() { VORTEX_RAIZ="$T/raiz" VORTEX_GRUPO_AUTOLOGIN="$1" bash "$POST" inicio-automatico "$2" >"$T/auto.out" 2>&1; }

  hacer_raiz; cp "$T/raiz/etc/greetd/config.toml" "$T/greetd.antes"
  auto "$NEG" "$YO"
  probar "casilla sin marcar (el usuario no es del grupo): greetd no cambia" cmp -s "$T/greetd.antes" "$T/raiz/etc/greetd/config.toml"
  auto grupo-que-no-existe "$YO"
  probar "no existe el grupo autologin (nadie marcó la casilla): greetd no cambia" cmp -s "$T/greetd.antes" "$T/raiz/etc/greetd/config.toml"
  auto "$(id -gn)" ""
  probar "sin usuario: no hace nada" cmp -s "$T/greetd.antes" "$T/raiz/etc/greetd/config.toml"
  auto "$(id -gn)" "$YO"
  probar "casilla marcada: agrega [initial_session] con vortex-sesion y el usuario" \
    toml x "assert d['initial_session'] == {'command': 'vortex-sesion', 'user': '$YO'}"
  probar "casilla marcada: la sesión normal (tuigreet con vortex-sesion) sigue igual" \
    toml x "assert d['default_session']['user'] == 'greeter' and 'tuigreet' in d['default_session']['command'] and d['terminal']['vt'] == 1"
  auto "$(id -gn)" "$YO"
  probar "repetirlo no duplica la sección" lineas "$T/raiz/etc/greetd/config.toml" '^\[initial_session\]' 1
  # Como lo deja vortex-live (liveuser) en el live: se reemplaza, no se suma.
  hacer_raiz; printf '\n[initial_session]\ncommand = "vortex-sesion"\nuser = "liveuser"\n' >>"$T/raiz/etc/greetd/config.toml"
  auto "$(id -gn)" "$YO"
  probar "reemplaza un [initial_session] anterior (el del live)" toml x "assert d['initial_session']['user'] == '$YO' and 'liveuser' not in open(sys.argv[1]).read()"
  # Si [initial_session] no es la última sección, lo que viene después se conserva.
  hacer_raiz; printf '\n[initial_session]\ncommand = "x"\nuser = "otro"\n\n[extra]\nclave = 1\n' >>"$T/raiz/etc/greetd/config.toml"
  auto "$(id -gn)" "$YO"
  probar "conserva las secciones que vienen después" toml x "assert d['extra'] == {'clave': 1} and d['initial_session']['user'] == '$YO'"
  rm -rf "$T/raiz"; mkdir -p "$T/raiz"
  auto "$(id -gn)" "$YO"
  probar "sin /etc/greetd/config.toml: avisa y no falla la instalación" contiene "$T/auto.out" "no activo el inicio automático"
fi

# ---------------------------------------------------------------------------------------------
echo "== Hook de pacman: RemoveMake en paru"
HOOK="$AQUI/iso/airootfs/etc/pacman.d/hooks/paru-removemake.hook"
probar "lleva la marca «remove from airootfs» (releng lo borra al terminar la imagen)" grep -qF 'remove from airootfs' "$HOOK"
probar "construir.sh copia todo airootfs (incluidos los hooks)" grep -qF 'cp -a "$AQUI/airootfs/." "$raiz/"' "$AQUI/iso/construir.sh"
if command -v pacman >/dev/null && unshare -rm true 2>/dev/null; then
  Hk="$T/hook"; mkdir -p "$Hk/pk/paru" "$Hk/pk/sed" "$Hk/out"
  cp /etc/paru.conf "$Hk/pk/paru/paru.conf" 2>/dev/null || printf '[options]\n#RemoveMake\n#SudoLoop\n' >"$Hk/pk/paru/paru.conf"
  grep -q '^#RemoveMake' "$Hk/pk/paru/paru.conf" || printf '[options]\n#RemoveMake\n#SudoLoop\n' >"$Hk/pk/paru/paru.conf"
  cat >"$Hk/pk/paru/PKGBUILD" <<'EOF'
pkgname=paru
pkgver=1
pkgrel=1
arch=(any)
backup=(etc/paru.conf)
package() { install -Dm644 "$startdir/paru.conf" "$pkgdir/etc/paru.conf"; }
EOF
  # Un «sed» con sus bibliotecas, porque el hook corre dentro de la raíz (chroot).
  cat >"$Hk/pk/sed/PKGBUILD" <<'EOF'
pkgname=sed
pkgver=1
pkgrel=1
arch=(any)
package() {
  local lib
  install -Dm755 /usr/bin/sed "$pkgdir/usr/bin/sed"
  while read -r lib; do
    install -Dm755 "$lib" "$pkgdir/usr/lib/${lib##*/}"
  done < <(ldd /usr/bin/sed | awk '$3 ~ /^\// {print $3}')
  install -Dm755 "$(readlink -f /usr/lib64/ld-linux-x86-64.so.2)" "$pkgdir/usr/lib/ld-linux-x86-64.so.2"
  ln -s usr/lib "$pkgdir/lib64"
}
EOF
  for p in paru sed; do
    (cd "$Hk/pk/$p" && PKGDEST="$Hk/out" BUILDDIR="$Hk/b" makepkg -f --nodeps >/dev/null 2>&1)
  done
  R="$Hk/root"
  mkdir -p "$R/etc/pacman.d/gnupg" "$R/etc/pacman.d/hooks" "$R/var/lib/pacman" "$R/var/cache/pacman/pkg" "$R/var/log"
  cp "$HOOK" "$R/etc/pacman.d/hooks/"
  cat >"$Hk/pacman.conf" <<EOF
[options]
RootDir = $R
DBPath = $R/var/lib/pacman/
CacheDir = $R/var/cache/pacman/pkg/
LogFile = $R/var/log/pacman.log
GPGDir = $R/etc/pacman.d/gnupg/
HookDir = $R/etc/pacman.d/hooks/
Architecture = auto
SigLevel = Never
LocalFileSigLevel = Never
EOF
  unshare -rm pacman --config "$Hk/pacman.conf" -U --noconfirm --noscriptlet \
    "$Hk"/out/sed-1-1-any.pkg.tar.zst "$Hk"/out/paru-1-1-any.pkg.tar.zst >"$Hk/pacman.out" 2>&1
  probar "pacman instala paru y corre el hook" contiene "$Hk/pacman.out" "Activando RemoveMake"
  probar "/etc/paru.conf de la raíz queda con RemoveMake activo" grep -qx 'RemoveMake' "$R/etc/paru.conf"
  probar "no toca otras líneas (#SudoLoop sigue comentada)" grep -qx '#SudoLoop' "$R/etc/paru.conf"
else
  salto "sin pacman o sin user namespaces: se salta la prueba del hook con pacman de verdad"
fi

echo
if ((FALLOS)); then
  echo "$FALLOS pruebas fallaron (salidas de lo que corrieron, abajo)"
  for f in "$T"/aur?.out "$T"/auto.out "$T"/sesion.out "$T"/c?.out; do
    [[ -s "$f" ]] && { echo "--- ${f##*/}"; sed 's/^/    /' "$f"; }
  done
  exit 1
fi
echo "Todas las pruebas pasaron."
