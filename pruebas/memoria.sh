#!/usr/bin/env bash
# Prueba del aviso antes de quedarte sin memoria (2.7, bin/vortex-memoria): con /proc, hyprctl y notify-send
# falsos. No avisa ni cierra nada en tu escritorio. También prueba cuándo avisa el gestor (avisar_memoria).
#
#   pruebas/memoria.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VM="$AQUI/bin/vortex-memoria"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-memoria.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

P="$T/proc"
mkdir -p "$T/bin" "$P" "$T/datos/applications"
proceso() {  # pid cgroup anon_kb shmem_kb swap_kb
  mkdir -p "$P/$1"
  echo "0::/user.slice/user-1000.slice/user@1000.service/app.slice/$2" >"$P/$1/cgroup"
  printf 'Name:\tx\nRssAnon:\t%s kB\nRssFile:\t9000000 kB\nRssShmem:\t%s kB\nVmSwap:\t%s kB\n' "$3" "$4" "$5" >"$P/$1/status"
}
printf 'MemTotal:       8000000 kB\nMemAvailable:    600000 kB\n' >"$P/meminfo"
# Discord: dos procesos en su scope (el segundo sin ventana) y ya parte en zram
proceso 100 app-Hyprland-discord-100.scope 1000000 100000 500000
proceso 101 app-Hyprland-discord-100.scope 1000000 0 0
# Kitty usa menos; el demonio sin ventana usa más que todos y no se ofrece
proceso 200 app-Hyprland-kitty-200.scope 400000 0 0
proceso 300 ollama.service 6000000 0 0

cat >"$T/clientes.json" <<'J'
[{"pid": 100, "class": "com.discordapp.Discord", "address": "0xd1", "mapped": true},
 {"pid": 100, "class": "com.discordapp.Discord", "address": "0xd2", "mapped": true},
 {"pid": 200, "class": "kitty", "address": "0xk1", "mapped": true},
 {"pid": 1, "class": "raro", "address": "0x1", "mapped": true}]
J
cat >"$T/bin/hyprctl" <<F
#!/bin/sh
case "\$1" in
  clients) cat "$T/clientes.json" ;;
  dispatch) shift; printf '%s\n' "\$*" >>"$T/ordenes"; [ -z "\${PRUEBA_LUA_NO:-}" ] || case "\$*" in hl.dsp*) exit 1;; esac ;;
esac
F
cat >"$T/bin/notify-send" <<F
#!/bin/sh
for a in "\$@"; do printf '%s\n' "\$a"; done >>"$T/aviso"
printf '%s' "\${PRUEBA_ACCION:-}"
F
chmod +x "$T/bin/"*
printf '[Desktop Entry]\nName=Discord\nStartupWMClass=com.discordapp.Discord\n[Desktop Action x]\nName=Otra\n' >"$T/datos/applications/dc.desktop"
export PATH="$T/bin:$PATH" VORTEX_MEMORIA_PROC="$P" XDG_DATA_HOME="$T/datos" XDG_DATA_DIRS="$T/nada"
correr() { rm -f "$T/aviso" "$T/ordenes"; env "$@" "$VM"; }

SALIDA="$("$VM" --ver)"
probar "--ver no avisa ni cierra" "[[ ! -e '$T/aviso' && ! -e '$T/ordenes' ]]"
probar "nombra la app con ventana que más usa, con su nombre de lanzador" "[[ '$SALIDA' == *'Discord es lo que más usa'* ]]"
probar "suma todos sus procesos y lo que ya está en zram; no la caché" "[[ '$SALIDA' == *'2.5 GB'* ]]"
probar "dice cuánto queda" "[[ '$SALIDA' == 'Quedan 0.6 GB.'* ]]"

correr PRUEBA_ACCION=
probar "el aviso es crítico: sale aunque no molestes" "grep -qx critical '$T/aviso'"
probar "da el botón de cerrarla" "grep -qx 'cerrar=Cerrar Discord' '$T/aviso'"
probar "si solo lo ignoras, no cierra nada" "[[ ! -e '$T/ordenes' ]]"

correr PRUEBA_ACCION=cerrar
probar "Cerrar cierra sus dos ventanas, con amabilidad" "grep -q 'address:0xd1' '$T/ordenes' && grep -q 'address:0xd2' '$T/ordenes'"
probar "no toca otra app" "! grep -q 0xk1 '$T/ordenes'"
probar "nada de matar procesos" "! grep -qE 'kill|SIG' '$VM'"
correr PRUEBA_ACCION=cerrar PRUEBA_LUA_NO=1
probar "también en el Hyprland sin config Lua" "grep -q '^closewindow address:0xd1' '$T/ordenes'"

correr PRUEBA_ACCION=default
probar "picarle a la tarjeta no cierra nada: abre el administrador" "grep -q 'vortex-task-manager' '$T/ordenes' && ! grep -q address '$T/ordenes'"

correr PRUEBA_ACCION=ver
probar "Ver abre el administrador de tareas" "grep -q 'vortex-task-manager' '$T/ordenes'"

# Una app sin scope propio cuenta sola: no arrastra a sus vecinos de sesión
proceso 400 session-2.scope 900000 0 0
proceso 401 session-2.scope 7000000 0 0
echo '[{"pid": 400, "class": "rara-app", "address": "0xr1", "mapped": true}]' >"$T/clientes.json"
SALIDA="$("$VM" --ver)"
probar "sin scope propio se mide solo su proceso" "[[ '$SALIDA' == *'Rara app es lo que más usa: 0.9 GB'* ]]"

# Sin una app que valga la pena: no ofrece cerrar algo que no arreglaría nada
echo '[{"pid": 200, "class": "kitty", "address": "0xk1", "mapped": true}]' >"$T/clientes.json"
proceso 200 app-Hyprland-kitty-200.scope 1000 0 0
correr PRUEBA_ACCION=
probar "app chiquita: sin botón de cerrar, sí el de ver" "! grep -q '^cerrar=' '$T/aviso' && grep -q '^ver=' '$T/aviso'"
echo 'no es json' >"$T/clientes.json"
correr PRUEBA_ACCION=
probar "sin Hyprland o con basura, igual avisa" "grep -q 'quedando sin memoria' '$T/aviso'"

# Cuándo avisa el gestor: sus dos funciones, solas, con un vortex-memoria falso.
# shellcheck disable=SC2034  # JUEGOS, AVISO_* y MEMORIA las leen las funciones que carga el eval
gestor_avisa() {  # disponible_kb total_kb juegos modo_juego ultimo_aviso → 0 si avisó
  (
    PROC="$T/pg"; mkdir -p "$PROC"
    printf 'MemTotal: %s kB\nMemFree: 1 kB\nMemAvailable: %s kB\n' "$2" "$1" >"$PROC/meminfo"
    eval "$(sed -n '/^memoria_baja() {/,/^}/p;/^avisar_memoria() {/,/^}/p' "$AQUI/bin/vortex-gestor")"
    declare -A JUEGOS=(); (($3)) && JUEGOS[x]=y
    EN_MODO=$4; modo_juego_activo() { ((EN_MODO)); }
    log() { :; }
    guardar_estado() { :; }
    AVISO_ENFRIAR_S=600 AVISO_MEMORIA=$5 AVISOS_MEMORIA=0 MEMORIA=true
    avisar_memoria
    ((AVISOS_MEMORIA == 1))
  )
}
AHORA=$(date +%s)
probar "gestor: 8 GB con 600 MB libres: avisa" "gestor_avisa 600000 8000000 0 0 0"
probar "gestor: 8 GB con 3 GB libres: no" "! gestor_avisa 3000000 8000000 0 0 0"
probar "gestor: 32 GB con 3 GB libres: no (como mucho 2 GB)" "! gestor_avisa 3000000 32000000 0 0 0"
probar "gestor: 32 GB con 1.5 GB libres: avisa" "gestor_avisa 1500000 32000000 0 0 0"
probar "gestor: con un juego abierto no sale" "! gestor_avisa 600000 8000000 1 0 0"
probar "gestor: en modo juego no sale" "! gestor_avisa 600000 8000000 0 1 0"
probar "gestor: no repite antes de 10 minutos" "! gestor_avisa 600000 8000000 0 0 $((AHORA - 300))"
probar "gestor: después de 10 minutos, sí" "gestor_avisa 600000 8000000 0 0 $((AHORA - 700))"
PSIN="$(cd "$T" && (PROC="$T/no-existe"; eval "$(sed -n '/^memoria_baja() {/,/^}/p' "$AQUI/bin/vortex-gestor")"; memoria_baja && echo si || echo no))"
probar "gestor: sin /proc/meminfo no se rompe" "[[ '$PSIN' == no ]]"

probar "sintaxis de python" "python3 -c \"import ast;ast.parse(open('$VM').read())\""
probar "se instala con los demás comandos" "grep -q 'vortex-memoria' '$AQUI/install/30-escritorio.sh' && grep -q 'COMANDOS_VORTEX=.*vortex-memoria' '$AQUI/bin/vortex'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
