#!/usr/bin/env bash
# Pruebas del escritorio del live y del idioma (teclado de Calamares → Hyprland, teclado inicial,
# idioma del shell, aviso de Secure Boot), sin construir la ISO, sin root, sin red y sin tocar tu
# HOME, tu sistema ni tu Hyprland: todo pasa en carpetas temporales y con comandos de mentira
# (busctl, gdbus, hyprctl, sudo, localectl, notify-send...). Las salvedades: el YAML de Calamares
# necesita pyyaml (si no está ni se puede traer con uv, esa prueba se salta) y la config de
# Hyprland se valida con «Hyprland --verify-config» si está instalado (solo la analiza).
#   1. sintaxis, YAML de Calamares (instancia keyboard@vortex) y config de Hyprland;
#   2. vortex-teclado-live: aplica la distribución de localed a Hyprland al arrancar y en cada
#      señal PropertiesChanged, no repite, rechaza valores raros, una sola copia, no gasta CPU
#      mientras espera y cierra su gdbus al salir;
#   3. vortex-instalador: deja latam (o es) en localed según el idioma, sin pisar una elegida;
#      avisa si Secure Boot está activo;
#   4. idioma del shell (install/usuario.sh): sale de /etc/locale.conf, con respaldo en español.
#
#   pruebas/live.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-live-prueba.XXXXXX")"
FALLOS=0
PIDS=()
limpiar() {
  local p
  for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done
  rm -rf "$T"
}
trap limpiar EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
salto() { printf '  --  %s\n' "$*"; }
probar() { local d="$1"; shift; if "$@"; then bien "$d"; else mal "$d"; fi; }
contiene() { grep -qF -- "$2" "$1" 2>/dev/null; }
no_contiene() { ! grep -qF -- "$2" "$1" 2>/dev/null; }
lineas() { [[ "$(wc -l <"$1" 2>/dev/null || echo 0)" -eq "$2" ]]; }
# jq -e sin salida (no se le puede redirigir a «probar»: se llevaría también su «ok»).
jq_ok() { jq -e "$@" >/dev/null 2>&1; }
# Espera (hasta ~6 s) a que se cumpla un comando; las pruebas no deben depender de la velocidad.
espera() { local _; for _ in $(seq 60); do "$@" && return 0; sleep 0.1; done; return 1; }
# Tiempo de CPU (ticks) que lleva un proceso.
cpu() { awk '{print $14 + $15}' "/proc/$1/stat" 2>/dev/null; }

# ---------------------------------------------------------------------------------------------
echo "== Sintaxis, YAML de Calamares y config de Hyprland"
for f in iso/airootfs/usr/local/bin/vortex-teclado-live iso/airootfs/usr/local/bin/vortex-instalador \
  iso/airootfs/usr/local/bin/vortex-post-instalacion install/usuario.sh pruebas/live.sh; do
  probar "bash -n $f" bash -n "$AQUI/$f"
done
probar "vortex-teclado-live es ejecutable" test -x "$AQUI/iso/airootfs/usr/local/bin/vortex-teclado-live"
probar "vortex-post-instalacion lo quita del sistema instalado" \
  contiene "$AQUI/iso/airootfs/usr/local/bin/vortex-post-instalacion" "/usr/local/bin/vortex-teclado-live"
probar "hyprland.lua lo lanza solo si existe /etc/vortex/live" \
  bash -c "grep -B3 -A3 'vortex-teclado-live' '$AQUI/config/hypr/hyprland.lua' | grep -q 'io.open(\"/etc/vortex/live\")'"

cat >"$T/yaml.py" <<'PY'
import os, sys
import yaml
raiz = sys.argv[1]
mal = []
cal = raiz + "/iso/airootfs/etc/calamares"
s = yaml.safe_load(open(cal + "/settings.conf", encoding="utf-8"))
k = [i for i in s["instances"] if i["module"] == "keyboard"]
if len(k) != 1 or k[0]["id"] != "vortex" or k[0]["config"] != "keyboard_vortex.conf":
    mal.append("settings.conf: falta la instancia keyboard@vortex con keyboard_vortex.conf")
for i in s["instances"]:
    if not os.path.isfile(cal + "/modules/" + i["config"]):
        mal.append("settings.conf: no existe modules/" + i["config"])
mostrar = [m for bloque in s["sequence"] if "show" in bloque for m in bloque["show"]]
ejecutar = [m for bloque in s["sequence"] if "exec" in bloque for m in bloque["exec"]]
for nombre, lista in (("show", mostrar), ("exec", ejecutar)):
    if "keyboard@vortex" not in lista:
        mal.append("settings.conf: " + nombre + " debe llevar keyboard@vortex")
    if "keyboard" in lista:
        mal.append("settings.conf: " + nombre + " no debe llevar el keyboard a secas (ignoraría keyboard_vortex.conf)")
if mostrar.index("locale@vortex") > mostrar.index("keyboard@vortex"):
    mal.append("settings.conf: keyboard va después de locale (el idioma se elige antes)")
c = yaml.safe_load(open(cal + "/modules/keyboard_vortex.conf", encoding="utf-8"))
if c.get("guessLayout") is not False:
    mal.append("keyboard_vortex.conf: guessLayout debe ser false (si no, es_MX cae en el teclado de España)")
if c.get("xOrgConfFileName") != "/etc/X11/xorg.conf.d/00-keyboard.conf":
    mal.append("keyboard_vortex.conf: xOrgConfFileName debe ser el archivo de localed")
print("\n".join(mal))
sys.exit(1 if mal else 0)
PY
python_yaml=()
if python3 -I -c 'import yaml' 2>/dev/null; then
  python_yaml=(python3 -I)
elif command -v uv >/dev/null && timeout 90 uv run --no-project --with pyyaml python -I -c 'import yaml' >/dev/null 2>&1; then
  python_yaml=(uv run --no-project --with pyyaml python -I)
fi
if ((${#python_yaml[@]})); then
  yaml_rc=1
  "${python_yaml[@]}" "$T/yaml.py" "$AQUI" >"$T/yaml.out" 2>&1 && yaml_rc=0
  probar "Calamares: keyboard@vortex en show y exec, guessLayout apagado, configs existen" test "$yaml_rc" -eq 0
  [[ -z "$(<"$T/yaml.out")" ]] || sed 's/^/      /' "$T/yaml.out"
else
  salto "sin pyyaml (ni uv con red para traerlo): se salta la validación del YAML de Calamares"
fi

if command -v Hyprland >/dev/null; then
  # Copia temporal con package.path al shell del repo (el hyprland.lua real busca en /usr/share).
  mkdir -p "$T/hypr/home"
  cp "$AQUI/config/hypr/hyprland.lua" "$AQUI/config/hypr/atajos.lua" "$T/hypr/"
  sed -i "s|^local serp = .*|local serp = \"$AQUI/shell/compositors/hyprland\"|" "$T/hypr/hyprland.lua"
  env -i PATH=/usr/bin:/bin HOME="$T/hypr/home" XDG_RUNTIME_DIR="$T/hypr/home" \
    Hyprland --verify-config --config "$T/hypr/hyprland.lua" >"$T/hypr/verif.out" 2>&1
  probar "Hyprland --verify-config: config ok" contiene "$T/hypr/verif.out" "config ok"
  # Que la verificación sí detecta errores (si no, la anterior no probaría nada).
  echo 'esto no es lua (' >>"$T/hypr/hyprland.lua"
  env -i PATH=/usr/bin:/bin HOME="$T/hypr/home" XDG_RUNTIME_DIR="$T/hypr/home" \
    Hyprland --verify-config --config "$T/hypr/hyprland.lua" >"$T/hypr/verif2.out" 2>&1
  probar "Hyprland --verify-config: un error de Lua no pasa" no_contiene "$T/hypr/verif2.out" "config ok"
else
  salto "Hyprland no está instalado: se salta --verify-config"
fi

# ---------------------------------------------------------------------------------------------
echo "== vortex-teclado-live"
TECLADO="$AQUI/iso/airootfs/usr/local/bin/vortex-teclado-live"
mkdir -p "$T/shim" "$T/run" "$T/raiz/etc/vortex" "$T/vacio"
chmod 700 "$T/run"
touch "$T/raiz/etc/vortex/live"
mkfifo "$T/fifo"
exec 7<>"$T/fifo"   # abierto para leer y escribir: el gdbus de mentira no se queda sin escritor

# localed de mentira: la distribución, la variante y las opciones salen de $T/estado (3 líneas).
cat >"$T/shim/busctl" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/busctl.log"
{ read -r l; read -r v; read -r o; } <"$T/estado"
printf 's "%s"\ns "%s"\ns "%s"\n' "\$l" "\$v" "\$o"
EOF
# gdbus monitor de mentira: dice lo mismo que el de verdad al empezar y luego repite lo que haya en
# la tubería; guarda su pid (es el mismo proceso: exec).
cat >"$T/shim/gdbus" <<EOF
#!/usr/bin/env bash
echo "\$*" >"$T/gdbus.args"
echo \$\$ >"$T/gdbus.pid"
echo "Monitoring signals on object /org/freedesktop/locale1 owned by org.freedesktop.locale1"
exec cat "$T/fifo"
EOF
cat >"$T/shim/hyprctl" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/hypr.log"
EOF
chmod +x "$T/shim"/*
estado() { printf '%s\n%s\n%s\n' "$1" "${2:-}" "${3:-}" >"$T/estado"; }
senal() { echo "/org/freedesktop/locale1: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.locale1', {'X11Layout': <'$1'>}, @as [])" >&7; }
SENAL_GENERICA="/org/freedesktop/locale1: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.locale1', @a{sv} {}, ['X11Layout'])"
lanzar() { # lanzar RAIZ [VAR=valor ...]: el vigilante en segundo plano, con un entorno limpio
  local raiz="$1"; shift
  env -i PATH="$T/shim:/usr/bin:/bin" HOME="$T" XDG_RUNTIME_DIR="$T/run" VORTEX_RAIZ="$raiz" \
    VORTEX_TECLADO_BLOQUEO="$T/run/teclado.lock" "$@" bash "$TECLADO" 2>"$T/teclado.err" &
  VIGILANTE=$!
  PIDS+=("$VIGILANTE")
}

# Sin la marca del live no hace nada.
estado latam
lanzar "$T/vacio" HYPRLAND_INSTANCE_SIGNATURE=abc
wait "$VIGILANTE"
probar "fuera del live: sale sin hacer nada" test ! -e "$T/hypr.log" -a ! -e "$T/gdbus.pid"

# En el live: al arrancar aplica lo que ya tenga localed.
estado latam
lanzar "$T/raiz" HYPRLAND_INSTANCE_SIGNATURE=abc
V1=$VIGILANTE
probar "al arrancar aplica la distribución de localed a Hyprland" espera contiene "$T/hypr.log" 'kb_layout = "latam"'
probar "usa hyprctl eval con hl.config (keyword ya no sirve con Lua)" \
  contiene "$T/hypr.log" 'eval hl.config({ input = { kb_layout = "latam", kb_variant = "" } })'
probar "sin opciones de localed no toca kb_options" no_contiene "$T/hypr.log" "kb_options"
probar "escucha el servicio de localed en el bus del sistema" \
  bash -c "grep -q -- '--system' '$T/gdbus.args' && grep -q -- 'org.freedesktop.locale1' '$T/gdbus.args'"
probar "lee las propiedades de localed" contiene "$T/busctl.log" "X11Layout X11Variant X11Options"

# Una señal con otra distribución, variante y opciones.
estado de nodeadkeys grp:alt_shift_toggle
senal de
probar "señal: aplica distribución, variante y opciones" \
  espera contiene "$T/hypr.log" 'kb_layout = "de", kb_variant = "nodeadkeys", kb_options = "grp:alt_shift_toggle"'

# Líneas que no son PropertiesChanged no hacen nada aunque localed haya cambiado.
estado fr
echo "The name org.freedesktop.locale1 is owned by :1.99" >&7
sleep 0.5
probar "otras líneas del monitor no disparan nada" lineas "$T/hypr.log" 2

# Repetir lo mismo no se vuelve a mandar; un cambio real sí.
estado de nodeadkeys grp:alt_shift_toggle
senal de
sleep 0.5
probar "la misma distribución otra vez: no se repite" lineas "$T/hypr.log" 2
estado fr
echo "$SENAL_GENERICA" >&7
probar "señal de invalidación: relee y aplica la nueva" espera contiene "$T/hypr.log" 'kb_layout = "fr", kb_variant = ""'
probar "sin opciones nuevas manda kb_options solo cuando hay (la de antes no se arrastra)" \
  bash -c "tail -n1 '$T/hypr.log' | grep -qv kb_options"

# Valores raros no llegan a Lua.
estado 'us"}}); os.execute("touch '"$T"'/inyectado"); --'
echo "$SENAL_GENERICA" >&7
sleep 0.5
probar "una distribución con comillas o paréntesis se descarta" test ! -e "$T/inyectado" -a "$(wc -l <"$T/hypr.log")" -eq 3
estado it "bad variant!" "caps:escape; x"
echo "$SENAL_GENERICA" >&7
probar "variante y opciones raras se ignoran, la distribución buena se aplica" \
  espera contiene "$T/hypr.log" 'eval hl.config({ input = { kb_layout = "it", kb_variant = "" } })'
estado ""
echo "$SENAL_GENERICA" >&7
sleep 0.5
probar "localed sin distribución: no se manda nada" lineas "$T/hypr.log" 4

# Una sola copia a la vez.
cp "$T/gdbus.pid" "$T/gdbus.pid.antes"
lanzar "$T/raiz" HYPRLAND_INSTANCE_SIGNATURE=abc
V2=$VIGILANTE
# Sin «wait» pelado: si el candado fallara, la segunda copia se quedaría escuchando y la prueba se colgaría.
probar "una segunda copia sale al instante" espera bash -c "! kill -0 $V2 2>/dev/null"
probar "la segunda copia no abre otro monitor" cmp -s "$T/gdbus.pid" "$T/gdbus.pid.antes"

# Sin CPU mientras espera: ni un tick en un segundo.
c1="$(cpu "$V1")"
sleep 1
c2="$(cpu "$V1")"
probar "esperando no gasta CPU (ticks: $c1 → $c2)" test -n "$c1" -a "$c1" = "$c2"

# Al cerrarlo se va también el monitor de D-Bus.
GDBUS="$(cat "$T/gdbus.pid")"
kill "$V1"
wait "$V1" 2>/dev/null
probar "al cerrar el vigilante se cierra su gdbus" espera bash -c "! kill -0 $GDBUS 2>/dev/null"

# Sin la firma de Hyprland en el entorno usa la primera instancia (-i 0), como bin/vortex.
rm -f "$T/hypr.log"
estado es
lanzar "$T/raiz"
V3=$VIGILANTE
probar "sin HYPRLAND_INSTANCE_SIGNATURE: hyprctl -i 0" espera contiene "$T/hypr.log" '-i 0 eval hl.config({ input = { kb_layout = "es"'
kill "$V3"
wait "$V3" 2>/dev/null
exec 7>&-

# ---------------------------------------------------------------------------------------------
echo "== vortex-instalador (teclado inicial y Secure Boot)"
INSTALADOR="$AQUI/iso/airootfs/usr/local/bin/vortex-instalador"
mkdir -p "$T/inst/shim" "$T/inst/run"
chmod 700 "$T/inst/run"
# sudo de mentira: anota lo que le piden y lo que Calamares recibiría en LANG; no ejecuta nada.
cat >"$T/inst/shim/sudo" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/inst/sudo.log"
case "\$*" in *calamares*) echo "LANG=\$LANG" >>"$T/inst/sudo.log" ;; esac
EOF
# localectl de mentira: «status» dice la distribución X11 que haya en $T/inst/x11.
cat >"$T/inst/shim/localectl" <<EOF
#!/usr/bin/env bash
echo "    System Locale: LANG=C.UTF-8"
echo "       X11 Layout: \$(cat "$T/inst/x11")"
EOF
cat >"$T/inst/shim/notify-send" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$T/inst/notify.log"
EOF
printf '#!/usr/bin/env bash\nexit 1\n' >"$T/inst/shim/pgrep"
chmod +x "$T/inst/shim"/*
instalar() { # instalar LAYOUT_X11 [VAR=valor ...]: corre el instalador y deja sus anotaciones en inst/
  local x11="$1"; shift
  rm -f "$T/inst/sudo.log" "$T/inst/notify.log"
  # Cada corrida es un arranque nuevo del live, salvo con MANTENER=1 (se vuelve a abrir).
  [[ "${MANTENER:-0}" == 1 ]] || rm -f "$T/inst/run/vortex-teclado-inicial"
  echo "$x11" >"$T/inst/x11"
  env -i PATH="$T/inst/shim:/usr/bin:/bin" HOME="$T" XDG_RUNTIME_DIR="$T/inst/run" WAYLAND_DISPLAY=wayland-1 \
    "$@" bash "$INSTALADOR" >"$T/inst/out" 2>&1
}
fijo="localectl --no-convert set-x11-keymap"

instalar "n/a"
probar "es_MX (el de siempre) con localed sin distribución: deja latam" contiene "$T/inst/sudo.log" "-n $fijo latam pc105"
probar "y abre Calamares en es_MX" contiene "$T/inst/sudo.log" "LANG=es_MX.UTF-8"
instalar "(unset)" VORTEX_INSTALADOR_IDIOMA=es_AR.UTF-8
probar "es_AR: latam" contiene "$T/inst/sudo.log" "$fijo latam pc105"
instalar "n/a" VORTEX_INSTALADOR_IDIOMA=es_ES.UTF-8
probar "es_ES: es (España)" contiene "$T/inst/sudo.log" "$fijo es pc105"
instalar "n/a" VORTEX_INSTALADOR_IDIOMA=en_US.UTF-8
probar "otro idioma: no toca el teclado" no_contiene "$T/inst/sudo.log" "set-x11-keymap"
probar "otro idioma: Calamares abre igual" contiene "$T/inst/sudo.log" "LANG=en_US.UTF-8"
instalar "us"
probar "el live trae us de fábrica: igual deja latam" contiene "$T/inst/sudo.log" "$fijo latam pc105"
MANTENER=1 instalar "de"
probar "al abrirlo otra vez en el mismo arranque no pisa la que se eligió" no_contiene "$T/inst/sudo.log" "set-x11-keymap"
probar "pero Calamares abre" contiene "$T/inst/sudo.log" "calamares"

# Secure Boot: la variable EFI son 4 bytes de atributos y luego el valor.
printf '\x06\x00\x00\x00\x01' >"$T/inst/sb-activo"
printf '\x06\x00\x00\x00\x00' >"$T/inst/sb-apagado"
instalar "n/a" VORTEX_SB_VARIABLE="$T/inst/sb-activo"
probar "Secure Boot activo: notificación crítica" contiene "$T/inst/notify.log" "-u critical"
probar "Secure Boot activo: el aviso dice qué hacer" contiene "$T/inst/notify.log" "Enroll MOK"
probar "Secure Boot activo: también sale por la terminal" contiene "$T/inst/out" "Secure Boot está activado"
probar "Secure Boot activo: no impide abrir Calamares" contiene "$T/inst/sudo.log" "calamares"
instalar "n/a" VORTEX_SB_VARIABLE="$T/inst/sb-apagado"
probar "Secure Boot apagado: sin aviso" test ! -e "$T/inst/notify.log"
instalar "n/a" VORTEX_SB_VARIABLE="$T/inst/no-existe"
probar "sin variable EFI ni mokutil (BIOS): sin aviso" test ! -e "$T/inst/notify.log"
printf '#!/usr/bin/env bash\necho "SecureBoot enabled"\n' >"$T/inst/shim/mokutil"
chmod +x "$T/inst/shim/mokutil"
instalar "n/a" VORTEX_SB_VARIABLE="$T/inst/no-existe"
probar "sin variable EFI pero mokutil dice «enabled»: avisa" contiene "$T/inst/notify.log" "Secure Boot está activado"
printf '#!/usr/bin/env bash\necho "SecureBoot disabled"\n' >"$T/inst/shim/mokutil"
instalar "n/a" VORTEX_SB_VARIABLE="$T/inst/no-existe"
probar "mokutil dice «disabled»: sin aviso" test ! -e "$T/inst/notify.log"

# ---------------------------------------------------------------------------------------------
echo "== Idioma del shell (install/usuario.sh)"
# La función sola, sacada del script (el resto de usuario.sh configura todo el usuario).
sed -n '/^idioma_del_shell()/,/^}/p' "$AQUI/install/usuario.sh" >"$T/idioma-fn.sh"
probar "se encuentra idioma_del_shell en usuario.sh" test -s "$T/idioma-fn.sh"
mkdir -p "$T/serp/src/assets/languages"
for l in ar az bg de en es fa fr hi hy id it ja ko nl pl pt ru tr ua vi zh zh_tw; do : >"$T/serp/src/assets/languages/$l.json"; done
# Los archivos de verdad del shell que el script da por existentes.
for l in en es ua zh zh_tw pt; do
  probar "el shell trae $l.json" test -f "$AQUI/shell/src/assets/languages/$l.json"
done
idioma() { # idioma 'contenido de locale.conf'
  printf '%s\n' "$1" >"$T/locale.conf"
  serpantinum="$T/serp" VORTEX_LOCALE_CONF="$T/locale.conf" bash -c 'source "$1"; idioma_del_shell' _ "$T/idioma-fn.sh"
}
esperado() { # esperado 'locale.conf' idioma
  local r; r="$(idioma "$1")"
  if [[ "$r" == "$2" ]]; then bien "$(printf '%s' "$1" | tr '\n' ' ') → $2"; else mal "$(printf '%s' "$1" | tr '\n' ' ') → «$r» (esperado $2)"; fi
}
esperado 'LANG=es_MX.UTF-8' es
esperado 'LANG=en_US.UTF-8' en
esperado 'LANG=pt_BR.UTF-8' pt
esperado 'LANG=fr_FR.UTF-8' fr
esperado 'LANG="de_DE.UTF-8"' de
esperado 'LANG=de_DE@euro' de
esperado 'LANG=uk_UA.UTF-8' ua
esperado 'LANG=zh_TW.UTF-8' zh_tw
esperado 'LANG=zh_HK.UTF-8' zh_tw
esperado 'LANG=zh_CN.UTF-8' zh
esperado 'LANG=C.UTF-8' es
esperado 'LANG=POSIX' es
esperado 'LANG=sv_SE.UTF-8' es      # el shell no tiene sv.json: español
esperado 'LANG=../../etc/passwd' es
esperado $'LC_TIME=en_US.UTF-8\nLANG=ja_JP.UTF-8' ja
esperado $'LANG=en_US.UTF-8\nLANG=it_IT.UTF-8' it
esperado '' es
rm -f "$T/locale.conf"
r="$(serpantinum="$T/serp" VORTEX_LOCALE_CONF="$T/no-hay" bash -c 'source "$1"; idioma_del_shell' _ "$T/idioma-fn.sh")"
probar "sin /etc/locale.conf: español" test "$r" = es
r="$(printf 'LANG=en_US.UTF-8\n' >"$T/locale.conf"; serpantinum="$T/vacio" VORTEX_LOCALE_CONF="$T/locale.conf" bash -c 'source "$1"; idioma_del_shell' _ "$T/idioma-fn.sh")"
probar "sin el shell instalado (no hay idiomas): español" test "$r" = es

# De punta a punta: «vortex configurar-usuario» con un HOME de mentira deja el idioma en settings.json.
mkdir -p "$T/e2e/shim" "$T/serp/config/serpantinum" "$T/serp/config/kitty"
cp "$AQUI/shell/config/serpantinum/settings.json" "$T/serp/config/serpantinum/settings.json"
: >"$T/serp/config/kitty/kitty.conf"
for c in sudo localectl xdg-user-dirs-update; do printf '#!/usr/bin/env bash\nexit 0\n' >"$T/e2e/shim/$c"; done
printf '#!/usr/bin/env bash\necho "${HOME}/Pictures"\n' >"$T/e2e/shim/xdg-user-dir"
chmod +x "$T/e2e/shim"/*
configurar() { # configurar HOME 'LANG=...'
  mkdir -p "$1"
  printf '%s\n' "$2" >"$T/e2e/locale.conf"
  env -i PATH="$T/e2e/shim:/usr/bin:/bin" HOME="$1" VORTEX_SERPANTINUM="$T/serp" VORTEX_LOCALE_CONF="$T/e2e/locale.conf" \
    bash "$AQUI/bin/vortex" configurar-usuario >"$1/salida.log" 2>&1
}
configurar "$T/e2e/h-en" 'LANG=en_US.UTF-8'
probar "configurar-usuario con LANG=en_US: el shell queda en inglés" \
  jq_ok '.general.language == "en" and .notifications.dnd == false and .general.quickactions == false' "$T/e2e/h-en/.config/serpantinum/settings.json"
probar "…y lo dice en la salida" contiene "$T/e2e/h-en/salida.log" "idioma: en"
configurar "$T/e2e/h-live" 'LANG=C.UTF-8'
probar "configurar-usuario en el live (C.UTF-8): español" \
  jq_ok '.general.language == "es"' "$T/e2e/h-live/.config/serpantinum/settings.json"
printf '{"general":{"language":"fr"}}\n' >"$T/e2e/h-live/.config/serpantinum/settings.json"
configurar "$T/e2e/h-live" 'LANG=en_US.UTF-8'
probar "unos ajustes que ya existen no se tocan" jq_ok '.general.language == "fr"' "$T/e2e/h-live/.config/serpantinum/settings.json"

# ---------------------------------------------------------------------------------------------
echo
if ((FALLOS)); then
  echo "$FALLOS prueba(s) fallaron."
  exit 1
fi
echo "Todo bien."
