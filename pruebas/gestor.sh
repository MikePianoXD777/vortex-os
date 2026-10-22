#!/usr/bin/env bash
# Pruebas del gestor de recursos (bin/vortex-gestor + src/vortex-eventos.c), sin VM:
#   1. sintaxis (bash -n) y shellcheck si está instalado;
#   2. el ayudante en C contra un Hyprland falso (sockets UNIX servidos con socat) y su
#      aviso de presión de memoria registrado en el /proc/pressure/memory de verdad;
#   3. el gestor entero con /proc, cgroups, busctl, renice, vortex y quickshell falsos:
#      foco, scopes, nice, modo juego (propio y ajeno), presión de memoria y salida.
# No toca el sistema: todo pasa en una carpeta temporal.
#
#   pruebas/gestor.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-gestor-prueba.XXXXXX")"
FALLOS=0
PIDS_FONDO=()

limpiar() {
  local p
  for p in "${PIDS_FONDO[@]}"; do kill "$p" 2>/dev/null; done
  wait 2>/dev/null
  rm -rf "$T"
}
trap limpiar EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }
# Terminó (o es un zombi esperando a que lo recojan).
termino() { [[ ! -e "/proc/$1" ]] || [[ "$(awk '{print $3}' "/proc/$1/stat" 2>/dev/null)" == Z ]]; }

# Espera (hasta 5 s) a que se cumpla una condición.
esperar() {
  local i
  for i in $(seq 100); do
    eval "$1" && return 0
    sleep 0.05
  done
  return 1
}

echo "== Sintaxis"
for f in bin/vortex bin/vortex-gestor install/30-escritorio.sh pruebas/gestor.sh; do
  probar "bash -n $f" "bash -n '$AQUI/$f'"
done
if command -v shellcheck >/dev/null; then
  probar "shellcheck" "shellcheck -x -S warning '$AQUI/bin/vortex-gestor' '$AQUI/bin/vortex' '$AQUI/pruebas/gestor.sh'"
else
  echo "  --  shellcheck no está instalado: se salta"
fi
for c in cc socat; do
  command -v "$c" >/dev/null || { echo "Falta $c para las pruebas"; exit 1; }
done
probar "compila sin avisos" "cc -O2 -Wall -Wextra -Werror -std=c11 -o '$T/vortex-eventos' '$AQUI/src/vortex-eventos.c'"
[[ -x "$T/vortex-eventos" ]] || { echo "No compiló: no sigo"; exit 1; }

# --- Hyprland falso ---
# .socket.sock responde a j/activewindow y j/clients con los JSON de la carpeta;
# .socket2.sock manda lo que la prueba escriba en el FIFO de eventos.
HIS="prueba_1700000000_1"
RUN="$T/run"
HD="$RUN/hypr/$HIS"
mkdir -p "$HD" "$RUN/quickshell/by-id/abc"
cat >"$T/responder" <<EOF
#!/usr/bin/env bash
pedido="\$(dd bs=65536 count=1 2>/dev/null)"
case "\$pedido" in
  j/activewindow) cat "$T/activa.json" ;;
  j/clients) cat "$T/clientes.json" ;;
esac
EOF
chmod +x "$T/responder"

activa() { # PID DIRECCION CLASE [TIPO]
  printf '{\n    "address": "0x%s",\n    "class": "%s",\n    "title": "t",\n    "pid": %s,\n    "contentType": "%s"\n}\n' \
    "$2" "$3" "$1" "${4:-none}" >"$T/activa.json"
}

hyprland_falso() {
  rm -f "$HD/.socket.sock" "$HD/.socket2.sock" "$T/ev"
  mkfifo "$T/ev"
  socat UNIX-LISTEN:"$HD/.socket.sock",fork EXEC:"$T/responder" 2>/dev/null &
  PIDS_FONDO+=($!)
  socat -u OPEN:"$T/ev" UNIX-LISTEN:"$HD/.socket2.sock" 2>/dev/null &
  PIDS_FONDO+=($!)
  exec 7>"$T/ev"
  esperar "[[ -S '$HD/.socket2.sock' && -S '$HD/.socket.sock' ]]"
}
evento() { printf '%s\n' "$@" >&7; }

echo "== Ayudante (vortex-eventos)"
cat >"$T/clientes.json" <<'EOF'
[{
    "address": "0xaaa1",
    "class": "kitty",
    "contentType": "none",
    "grouped": ["0xaaa1"],
    "swallowing": "0x0"
},{
    "address": "0xeee5",
    "class": "steam_app_1",
    "contentType": "game"
}]
EOF
activa 4001 aaa1 kitty
hyprland_falso
"$T/vortex-eventos" "$HD/.socket2.sock" "$HD/.socket.sock" /proc/pressure/memory 150000 2000000 80 >"$T/salida" 2>"$T/errores" 7>&- &
AY=$!
PIDS_FONDO+=($AY)
esperar "grep -q '^foco' '$T/salida'"
SEP=$'\037'
probar "ventanas abiertas al arrancar" "grep -qx 'abre${SEP}aaa1${SEP}none${SEP}kitty' '$T/salida' && grep -qx 'abre${SEP}eee5${SEP}game${SEP}steam_app_1' '$T/salida'"
probar "foco inicial con pid, dirección y clase" "grep -qx 'foco${SEP}4001${SEP}aaa1${SEP}none${SEP}kitty' '$T/salida'"
probar "aviso de presión registrado en el kernel" "! grep -q '^psi${SEP}no' '$T/salida'"
grep "^psi${SEP}no" "$T/salida" | tr '\037' ' ' | sed 's/^/      /'
# Mismo foco (cambio de título): no se pregunta nada.
evento "activewindowv2>>aaa1" "windowtitlev2>>aaa1,otro"
# Foco que pasa rápido por una ventana: solo cuenta donde se queda.
activa 5001 bbb2 firefox
evento "activewindowv2>>ccc3" "activewindowv2>>bbb2"
esperar "grep -q '^foco${SEP}5001' '$T/salida'"
sleep 0.3
probar "antirrebote: un solo foco nuevo" "[[ \$(grep -c '^foco' '$T/salida') == 2 ]]"
evento "openwindow>>ddd4,2,steam_app_570,Dota 2" "closewindow>>ddd4"
esperar "grep -q '^cierra' '$T/salida'"
probar "abre y cierra" "grep -qx 'abre${SEP}ddd4${SEP}${SEP}steam_app_570' '$T/salida' && grep -qx 'cierra${SEP}ddd4' '$T/salida'"
# Se cierra la enfocada y no queda ninguna.
printf '{}' >"$T/activa.json"
evento "closewindow>>bbb2" "activewindowv2>>"
esperar "grep -q '^foco${SEP}0' '$T/salida'"
probar "se cierra la enfocada y no queda ninguna: pid 0" "grep -qx 'foco${SEP}0${SEP}${SEP}${SEP}' '$T/salida'"
probar "sin despertar la CPU en reposo" "sleep 1; read -r -a st < /proc/$AY/stat; t1=\$((st[13] + st[14])); sleep 2; read -r -a st < /proc/$AY/stat; [[ \$((st[13] + st[14])) == \$t1 ]]"
exec 7>&-
probar "termina con 0 cuando Hyprland se cierra" "wait $AY"
probar "sin errores en stderr" "[[ ! -s '$T/errores' ]]"
"$T/vortex-eventos" "$T/no-existe.sock" "$T/no-existe.sock" - >/dev/null 2>&1
probar "sin Hyprland: código 2" "[[ $? == 2 ]]"

echo "== Gestor entero (simulación)"
P="$T/proc"
CGR="$T/cg"
RAIZ="/user.slice/user-$UID.slice/user@$UID.service"
SESION="/user.slice/user-$UID.slice/session-2.scope"
mkdir -p "$P" "$CGR$RAIZ/app.slice" "$T/estado" "$T/conf"
echo enabled >"$T/scx"

# proceso PID COMM CGROUP NICE_MAX "HILOS" "HIJOS"
proceso() {
  local pid="$1" comm="$2" cg="$3" max="$4" hilos="$5" hijos="$6" t
  mkdir -p "$P/$pid"
  echo "$comm" >"$P/$pid/comm"
  echo "0::$cg" >"$P/$pid/cgroup"
  printf 'Limit                     Soft Limit           Hard Limit           Units\nMax nice priority         %s                   %s\n' "$max" "$max" >"$P/$pid/limits"
  for t in $pid $hilos; do
    mkdir -p "$P/$pid/task/$t"
    echo "$t ($comm) S 1 $pid $pid 0 -1 4194304 0 0 0 0 0 0 0 0 20 0 1 0 100 0 0" >"$P/$pid/task/$t/stat"
    : >"$P/$pid/task/$t/children"
  done
  [[ -n "$hijos" ]] && printf '%s ' $hijos >"$P/$pid/task/$pid/children"
  return 0
}
nice_hilo() { local s; read -r s <"$P/$1/task/$2/stat"; s="${s##*) }"; read -r -a f <<<"$s"; echo "${f[16]}"; }

proceso 4001 kitty "$SESION" 30 "4003" "4002"
proceso 4002 bash "$SESION" 30 "" ""
proceso 5001 firefox "$RAIZ/app.slice/app-flatpak-org.mozilla.firefox-1.scope" 0 "" ""
mkdir -p "$CGR$RAIZ/app.slice/app-flatpak-org.mozilla.firefox-1.scope"
echo 100 >"$CGR$RAIZ/app.slice/app-flatpak-org.mozilla.firefox-1.scope/cpu.weight"
proceso 6001 wine64-preloader "$SESION" 30 "6002" ""
proceso 7001 quickshell "$SESION" 30 "" ""
proceso 8001 steam "$SESION" 30 "" ""

# busctl falso: crea el scope en el árbol de cgroups falso y mueve los procesos.
cat >"$T/busctl" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/busctl.log"
[[ -e "$T/busctl-falla" ]] && exit 1
args=("\$@")
metodo="\${args[5]}" unidad="\${args[7]}"
peso=""
for ((i = 0; i < \${#args[@]}; i++)); do [[ "\${args[i]}" == CPUWeight ]] && peso="\${args[i+2]}"; done
dir="$CGR$RAIZ/app.slice/\$unidad"
case "\$metodo" in
  StartTransientUnit)
    mkdir -p "\$dir"; echo "\$peso" >"\$dir/cpu.weight"; echo "default \$peso" >"\$dir/io.weight"; : >"\$dir/memory.reclaim"
    n="\${args[12]}"
    for ((i = 13; i < 13 + n; i++)); do echo "0::$RAIZ/app.slice/\$unidad" >"$P/\${args[i]}/cgroup"; done ;;
  SetUnitProperties)
    [[ -d "\$dir" ]] || exit 1
    echo "\$peso" >"\$dir/cpu.weight" ;;
esac
EOF
# renice falso: cambia el nice en los stat falsos.
cat >"$T/renice" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/renice.log"
v="\$2"; shift 3
for tid in "\$@"; do
  for f in "$P"/*/task/"\$tid"/stat; do
    awk -v v="\$v" '{ i = index(\$0, ") "); pre = substr(\$0, 1, i + 1); n = split(substr(\$0, i + 2), c, " "); c[17] = v; s = c[1]; for (k = 2; k <= n; k++) s = s " " c[k]; print pre s }' "\$f" >"\$f.n" && mv "\$f.n" "\$f"
  done
done
EOF
cat >"$T/vortex" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/vortex.log"
case "\$2" in on) mkdir -p "$T/estado/vortex"; touch "$T/estado/vortex/modo-juego" ;; off) rm -f "$T/estado/vortex/modo-juego" ;; esac
EOF
cat >"$T/quickshell" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/qs.log"
if [[ "\$*" == *" show" ]]; then
  printf 'target wallpaper\n  function setPlayback(screenName: string, state: string): void\ntarget vortex\n  function soltarMemoria(): void\n'
fi
EOF
# El ayudante de verdad, más un FIFO para inyectar avisos de presión de memoria.
mkfifo "$T/inyecta"
cat >"$T/eventos" <<EOF
#!/usr/bin/env bash
# Con un archivo falso el aviso de presión no se registra: se inyecta por el FIFO.
"$T/vortex-eventos" "\$1" "\$2" - "\${@:4}" &
echo \$! >"$T/ayudante.pid"
exec cat "$T/inyecta"
EOF
chmod +x "$T/busctl" "$T/renice" "$T/vortex" "$T/quickshell" "$T/eventos"

cat >"$T/clientes.json" <<'EOF'
[{"address": "0xaaa1", "class": "kitty", "contentType": "none"}]
EOF
activa 4001 aaa1 kitty
hyprland_falso

gestor() {
  env -u HYPRLAND_INSTANCE_SIGNATURE XDG_RUNTIME_DIR="$RUN" XDG_STATE_HOME="$T/estado" XDG_CONFIG_HOME="$T/conf" \
    VORTEX_GESTOR_PROC="$P" VORTEX_GESTOR_CGROOT="$CGR" VORTEX_GESTOR_SCX="$T/scx" VORTEX_GESTOR_PSI="$T/psi" \
    VORTEX_GESTOR_EVENTOS="$T/eventos" VORTEX_GESTOR_BUSCTL="$T/busctl" VORTEX_GESTOR_RENICE="$T/renice" \
    VORTEX_GESTOR_VORTEX="$T/vortex" VORTEX_GESTOR_QS="$T/quickshell" \
    "$AQUI/bin/vortex-gestor" >"$T/gestor.log" 2>&1 7>&- 8>&- &
  GESTOR=$!
  PIDS_FONDO+=($GESTOR)
}
echo 'DEPURAR=1' >"$T/conf/gestor.conf" && mkdir -p "$T/conf/vortex" && mv "$T/conf/gestor.conf" "$T/conf/vortex/"
echo "some avg10=3.10 avg60=1.00 avg300=0.20 total=1" >"$T/psi"
gestor
exec 8>"$T/inyecta"
EST="$RUN/vortex/gestor"
SC_KITTY="$CGR$RAIZ/app.slice/app-vortex-kitty-4001.scope"
esperar "grep -q '^foco_pid=4001' '$EST' 2>/dev/null && [[ -d '$SC_KITTY' ]]"
probar "kitty y su hijo van a su scope" "grep -q 'StartTransientUnit .*app-vortex-kitty-4001.scope fail 5 PIDs au 2 4001 4002 Slice s app.slice' '$T/busctl.log'"
probar "con el peso de foco" "[[ \$(cat '$SC_KITTY/cpu.weight') == 500 ]]"
esperar "[[ \$(nice_hilo 4001 4003) == -5 ]]"
probar "nice -5 en sus hilos (sched_ext)" "[[ \$(nice_hilo 4001 4001) == -5 && \$(nice_hilo 4001 4003) == -5 && \$(nice_hilo 4002 4002) == -5 ]]"

activa 5001 bbb2 firefox
evento "activewindowv2>>bbb2"
esperar "grep -q '^foco_pid=5001' '$EST'"
probar "kitty vuelve al peso de fondo (en systemd, una vez)" "grep -q 'SetUnitProperties sba(sv) app-vortex-kitty-4001.scope true 2 CPUWeight t 50' '$T/busctl.log' && [[ \$(cat '$SC_KITTY/cpu.weight') == 50 ]]"
probar "y su nice vuelve a 0" "[[ \$(nice_hilo 4001 4001) == 0 && \$(nice_hilo 4001 4003) == 0 && \$(nice_hilo 4002 4002) == 0 ]]"
probar "firefox (flatpak) no se mueve: se le sube el peso" "! grep -q 'firefox' '$T/busctl.log' && [[ \$(cat '$CGR$RAIZ/app.slice/app-flatpak-org.mozilla.firefox-1.scope/cpu.weight') == 500 ]]"
probar "sin RLIMIT_NICE no se toca su nice" "[[ \$(nice_hilo 5001 5001) == 0 ]] && ! grep -q ' 5001' '$T/renice.log'"

activa 4001 aaa1 kitty
evento "activewindowv2>>aaa1"
esperar "grep -q '^foco_pid=4001' '$EST'"
probar "firefox recupera su peso original" "[[ \$(cat '$CGR$RAIZ/app.slice/app-flatpak-org.mozilla.firefox-1.scope/cpu.weight') == 100 ]]"
probar "kitty: peso de foco escrito directo (sin busctl)" "[[ \$(cat '$SC_KITTY/cpu.weight') == 500 && \$(grep -c 'kitty' '$T/busctl.log') == 2 ]]"

# Un juego: modo juego, fondo en pausa y su propio scope.
SC_JUEGO="$CGR$RAIZ/app.slice/app-vortex-steam_app_570-6001.scope"
activa 6001 ccc3 steam_app_570
evento "openwindow>>ccc3,1,steam_app_570,Dota 2" "activewindowv2>>ccc3"
esperar "grep -q '^foco_pid=6001' '$EST'"
probar "juego: vortex modo-juego on" "grep -qx 'modo-juego on' '$T/vortex.log'"
probar "juego: fondo animado en pausa" "grep -q 'call wallpaper setPlayback all pause' '$T/qs.log'"
probar "juego: su propio scope" "[[ -d '$SC_JUEGO' && \$(cat '$SC_JUEGO/cpu.weight') == 500 ]]"
probar "estado: modo juego propio" "grep -qx 'modo_juego=propio' '$EST' && grep -qx 'juegos=1' '$EST'"

# Presión de memoria: el shell suelta memoria y se comprime lo de fondo (no el juego).
printf 'psi\037memoria\n' >&8
esperar "grep -q '^reclamos=1' '$EST'"
esperar "[[ -s '$SC_KITTY/memory.reclaim' ]]"
probar "presión: se comprime lo de kitty (fondo)" "[[ \$(cat '$SC_KITTY/memory.reclaim') == 64M ]]"
probar "presión: nada del juego enfocado" "[[ ! -s '$SC_JUEGO/memory.reclaim' ]]"
esperar "grep -q 'call vortex soltarMemoria' '$T/qs.log'"
probar "presión: se le pide al shell que suelte memoria" "grep -q 'call vortex soltarMemoria' '$T/qs.log'"
probar "presión: queda en el registro" "grep -q 'presión de memoria (some avg10=3.10' '$T/gestor.log'"
printf 'psi\037memoria\n' >&8
esperar "grep -q '^psi_eventos=2' '$EST'"
probar "presión: enfriamiento (no repite enseguida)" "grep -q '^reclamos=1' '$EST'"

evento "closewindow>>ccc3"
esperar "grep -qx 'modo-juego off' '$T/vortex.log'"
probar "sin juegos: vortex modo-juego off" "grep -qx 'modo-juego off' '$T/vortex.log'"
probar "sin juegos: el fondo sigue" "grep -q 'call wallpaper setPlayback all play' '$T/qs.log'"

# Modo juego puesto por otro (gamemode): el gestor no lo pone ni lo quita.
touch "$T/estado/vortex/modo-juego"
activa 8001 fff6 steam_app_1
evento "openwindow>>fff6,1,steam_app_1,Juego" "activewindowv2>>fff6"
esperar "[[ \$(grep -c 'setPlayback all pause' '$T/qs.log') == 2 ]]"
evento "closewindow>>fff6"
esperar "grep -q '^juegos=0' '$EST'"
probar "modo juego ajeno: ni on ni off" "[[ \$(grep -c 'modo-juego' '$T/vortex.log') == 2 && -e '$T/estado/vortex/modo-juego' ]]"
# Sigue puesto: el fondo queda en pausa hasta que quien lo puso lo quite.
probar "modo juego ajeno: el fondo sigue en pausa" "[[ \$(grep -c 'setPlayback all play' '$T/qs.log') == 1 ]]"
rm -f "$T/estado/vortex/modo-juego"

# Juego por contentType (sin clase conocida).
activa 4002 abcd "un.juego.raro" game
evento "activewindowv2>>abcd"
esperar "grep -q '^juegos=1' '$EST'"
probar "juego por contentType=game" "[[ \$(grep -c 'modo-juego on' '$T/vortex.log') == 2 ]]"
evento "closewindow>>abcd"
esperar "[[ \$(grep -c 'modo-juego off' '$T/vortex.log') == 2 ]]"

# Shell y Hyprland nunca se mueven.
activa 7001 dead quickshell
evento "activewindowv2>>dead"
esperar "grep -q '^foco_pid=7001' '$EST'"
probar "el shell no se toca" "! grep -q 7001 '$T/busctl.log' && grep -qx 'foco_cgroup=' '$EST'"

# Memoria del gestor (bash + ayudante) mientras corre.
rss() { awk '/^VmRSS/ {print $2}' "/proc/$1/status" 2>/dev/null; }
pss() { awk '/^Pss:/ {print $2}' "/proc/$1/smaps_rollup" 2>/dev/null; }
AYP="$(cat "$T/ayudante.pid")"
echo "  --  RAM: gestor (bash) $(rss "$GESTOR") kB RSS / $(pss "$GESTOR") kB PSS; vortex-eventos $(rss "$AYP") kB RSS / $(pss "$AYP") kB PSS"

# Salida: Hyprland se cierra.
exec 7>&- 8>&-
esperar "termino $GESTOR"
wait "$GESTOR"
probar "termina con 0 al cerrarse Hyprland" "[[ $? == 0 ]]"
probar "deja las apps con el peso normal" "[[ \$(cat '$SC_KITTY/cpu.weight') == 100 ]]"
probar "borra su estado" "[[ ! -e '$EST' ]]"
probar "sin errores en su registro" "! grep -Eiq 'error|command not found|unbound|sintaxis|syntax' '$T/gestor.log'"

# Parada con SIGTERM (systemctl stop) y systemd que no deja mover procesos.
: >"$T/busctl.log"
touch "$T/busctl-falla"
proceso 9001 kitty "$SESION" 30 "" ""
proceso 9002 kitty "$SESION" 30 "" ""
proceso 9003 kitty "$SESION" 30 "" ""
activa 9001 a901 kitty
printf '[]' >"$T/clientes.json"
hyprland_falso
gestor
exec 8>"$T/inyecta"
esperar "grep -q '^foco_pid=9001' '$EST' 2>/dev/null"
for p in 9002 9003; do
  activa "$p" "a$p" kitty
  evento "activewindowv2>>a$p"
  esperar "grep -q '^foco_pid=$p' '$EST'"
done
probar "si systemd no deja mover: deja de intentarlo" "grep -q 'sigo sin moverlas' '$T/gestor.log'"
probar "y el nice sigue funcionando" "[[ \$(nice_hilo 9003 9003) == -5 ]]"
kill -TERM "$GESTOR"
esperar "termino $GESTOR"
wait "$GESTOR"
probar "SIGTERM: termina con 0" "[[ $? == 0 ]]"
probar "SIGTERM: devuelve el nice" "[[ \$(nice_hilo 9003 9003) == 0 ]]"
exec 7>&- 8>&-

echo "== vortex gestor estado"
mkdir -p "$T/shim" "$T/run2/vortex"
cat >"$T/shim/systemctl" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *is-active*) echo active ;;
  *is-enabled*) echo enabled ;;
  *MemoryCurrent*) echo 4718592 ;;
esac
EOF
chmod +x "$T/shim/systemctl"
cat >"$T/run2/vortex/gestor" <<'EOF'
inicio=1700000000
pid=1
foco_pid=4001
foco_clase=kitty
foco_cgroup=/user.slice/user-1000.slice/user@1000.service/app.slice/app-vortex-kitty-4001.scope
foco_nice=-5
apps=3
juegos=1
modo_juego=propio
fondo_pausado=1
psi=vigilada
psi_eventos=2
psi_ultimo=1700000100
psi_valor=some avg10=3.10 avg60=1.00 avg300=0.20, full avg10=1.00 avg60=0.10 avg300=0.00
reclamos=1
shell_soltar=no sabe
EOF
PATH="$T/shim:$PATH" XDG_RUNTIME_DIR="$T/run2" "$AQUI/bin/vortex" gestor estado >"$T/estado.txt" 2>&1
probar "estado: corre y lo cuenta" "grep -q 'activo desde las .*(usa 5 MB)' '$T/estado.txt' && grep -q 'kitty (pid 4001), con prioridad en app-vortex-kitty-4001.scope y nice -5' '$T/estado.txt' && grep -q 'modo juego: encendido por el gestor · fondo animado: en pausa' '$T/estado.txt'"
sed 's/^/      /' "$T/estado.txt"
rm "$T/run2/vortex/gestor"
PATH="$T/shim:$PATH" XDG_RUNTIME_DIR="$T/run2" "$AQUI/bin/vortex" gestor estado >"$T/estado.txt" 2>&1
probar "estado: sin el gestor corriendo" "grep -q 'no está corriendo' '$T/estado.txt'"

echo
if ((FALLOS)); then
  echo "$FALLOS pruebas fallaron (registro del gestor: abajo)"
  sed 's/^/    /' "$T/gestor.log"
  echo "renice:"; sed 's/^/    /' "$T/renice.log"
  exit 1
fi
echo "Todas las pruebas pasaron."
