#!/usr/bin/env bash
# Prueba de la IA local (3.1, bin/vortex-ia): motor, modelos y vigilar, con nvidia-smi, lspci, ollama,
# pacman, sudo, df, hyprctl y notify-send falsos, y /proc y /sys/class/drm de mentira. No instala nada, no
# baja modelos ni avisa en tu escritorio.
#
#   pruebas/ia.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VI="$AQUI/bin/vortex-ia"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-ia.XXXXXX")"
trap 'jobs -p | xargs -r kill 2>/dev/null; rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/proc" "$T/drm" "$T/estado" "$T/run"
printf 'MemTotal:       16777216 kB\nMemAvailable:   12000000 kB\n' >"$T/proc/meminfo"

# nvidia-smi: contesta solo si existe $T/nvidia (si no, como sin driver de NVIDIA)
cat >"$T/bin/nvidia-smi" <<F
#!/bin/sh
[ -e "$T/nvidia" ] || exit 9
case "\$*" in *compute_cap*) cat "$T/cc" ;; *) cat "$T/nvidia" ;; esac
F
cat >"$T/bin/lspci" <<F
#!/bin/sh
cat "$T/lspci" 2>/dev/null
F
# ollama: «list» y «ps» salen de archivos; sin ellos, el servidor no contesta. «ps» deja de mostrar
# modelos después de las veces que diga $T/ps-veces (así se descargan solos).
cat >"$T/bin/ollama" <<F
#!/bin/sh
echo "\$*" >>"$T/ollama-ordenes"
case "\$1" in
  list) [ -e "$T/list" ] || { echo "Error: could not connect to ollama server" >&2; exit 1; }
        cat "$T/list" ;;
  ps) [ -e "$T/ps" ] || { echo "Error: could not connect to ollama server" >&2; exit 1; }
      n=\$(( \$(cat "$T/ps-cuenta" 2>/dev/null || echo 0) + 1 )); echo "\$n" >"$T/ps-cuenta"
      if [ "\$n" -le "\$(cat "$T/ps-veces" 2>/dev/null || echo 999)" ]; then cat "$T/ps"; else head -n1 "$T/ps"; fi ;;
  run) echo "Un arcoíris sale cuando la luz se separa en colores."
       printf 'total duration:       1.2s\neval count:           40 token(s)\neval rate:            87.50 tokens/s\n' >&2 ;;
  *) exit 1 ;;
esac
F
cat >"$T/bin/pacman" <<F
#!/bin/sh
case "\$1" in
  -Q) shift; for p; do grep -qx "\$p" "$T/instalados" 2>/dev/null || exit 1; done ;;
  -Sp) printf 'cccl 12000000\ncuda 2600000000\nollama-cuda 402502933\n' ;;
  -Si) printf 'Name            : cccl\nInstalled Size  : 30.00 MiB\nName            : cuda\nInstalled Size  : 5.20 GiB\nName            : ollama-cuda\nInstalled Size  : 479.99 MiB\n' ;;
  *) echo "pacman \$*" >>"$T/pacman-raro"; exit 1 ;;
esac
F
cat >"$T/bin/sudo" <<F
#!/bin/sh
echo "\$*" >>"$T/sudo"
[ "\$1 \$2" = "pacman -S" ] && echo ollama-cuda >>"$T/instalados"
exit 0
F
cat >"$T/bin/df" <<F
#!/bin/sh
printf 'S.ficheros Bloques Usados Disponibles Uso%% Montado\n/dev/x 900000 1 %s 50%% /\n' "\$(cat "$T/libre")"
F
cat >"$T/bin/hyprctl" <<F
#!/bin/sh
[ "\$1" = dispatch ] && { shift; printf '%s\n' "\$*" >>"$T/ordenes"; }
exit 0
F
cat >"$T/bin/notify-send" <<F
#!/bin/sh
for a in "\$@"; do printf '%s\n' "\$a"; done >>"$T/aviso"; echo --- >>"$T/aviso"
printf '%s' "\${PRUEBA_ACCION:-}"
F
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH" VORTEX_IA_PROC="$T/proc" VORTEX_IA_DRM="$T/drm" XDG_STATE_HOME="$T/estado" \
  XDG_RUNTIME_DIR="$T/run" VORTEX_IA_INTERVALO=0.1 VORTEX_IA_GRACIA=0.3
MARCA="$T/estado/vortex/ia-motor"

LSPCI_NV='01:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA102 [GeForce RTX 3080 Lite Hash Rate] [10de:2216] (rev a1)'
LSPCI_AMD='03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Navi 22 [Radeon RX 6700 XT] [1002:73df] (rev c1)'
LSPCI_INTEL='00:02.0 VGA compatible controller [0300]: Intel Corporation Alder Lake-P GT2 [Iris Xe Graphics] [8086:46a6] (rev 0c)'
nvidia() {  # nombre total_mb libre_mb cc
  echo "$LSPCI_NV" >"$T/lspci"; rm -rf "$T/drm"/*
  printf '%s, %s, %s\n' "$1" "$2" "$3" >"$T/nvidia"; echo "$4" >"$T/cc"
}
amd() {  # vram_gb usada_gb
  echo "$LSPCI_AMD" >"$T/lspci"; rm -f "$T/nvidia"; rm -rf "$T/drm"/*
  mkdir -p "$T/drm/card0/device" "$T/drm/card0-DP-1"
  echo 0x1002 >"$T/drm/card0/device/vendor"
  echo $(($1 * 1073741824)) >"$T/drm/card0/device/mem_info_vram_total"
  echo $(($2 * 1073741824)) >"$T/drm/card0/device/mem_info_vram_used"
}
sin_gpu() { echo "$LSPCI_INTEL" >"$T/lspci"; rm -f "$T/nvidia"; rm -rf "$T/drm"/*; }
paquetes() { printf '%s\n' "$@" >"$T/instalados"; }
# cabe MODELO → «gpu», «parcial», «cpu» o «no» según «modelos --json»
cabe() { python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(next(x["cabe"] for x in d["modelos"]+d["otros"] if x["modelo"]==sys.argv[2]))' "$T/json" "$1"; }
json() { python3 -c "import json,sys; d=json.load(open('$T/json')); print($1)"; }
limpiar() { rm -f "$T/aviso" "$T/sudo" "$T/ordenes" "$T/ollama-ordenes" "$T/ps-cuenta"; }
avisos() { grep -c '^---$' "$T/aviso" 2>/dev/null || echo 0; }
echo 50000 >"$T/libre"

echo "modelos"
nvidia "NVIDIA GeForce RTX 3080" 10240 8000 8.6
"$VI" modelos >"$T/salida"
probar "NVIDIA: dice la gráfica con su nombre corto y su VRAM" "grep -q '^Tu gráfica: RTX 3080 con 10 GB (7.8 GB libres ahora)' '$T/salida'"
probar "NVIDIA: recomienda uno que cabe entero" "grep -q '^Te recomiendo: Qwen 3.5 9B (qwen3.5:9b' '$T/salida'"
probar "sin Ollama corriendo, lo dice (no sabe cuáles tienes)" "grep -q 'Ollama no está corriendo' '$T/salida'"
"$VI" modelos --json >"$T/json"
probar "--json se puede leer" "[[ \"\$(json 'd[\"maquina\"][\"gpu\"][\"tipo\"]')\" == nvidia ]]"
probar "NVIDIA 10 GB: qwen3.5:4b cabe entero" "[[ \$(cabe qwen3.5:4b) == gpu ]]"
probar "NVIDIA 10 GB: qwen2.5:14b solo en parte" "[[ \$(cabe qwen2.5:14b) == parcial ]]"
probar "NVIDIA 10 GB con 16 GB de RAM: qwen2.5:32b no cabe" "[[ \$(cabe qwen2.5:32b) == no ]]"
probar "--json trae el recomendado" "[[ \$(json 'd[\"recomendado\"]') == qwen3.5:9b ]]"

cat >"$T/list" <<'L'
NAME                ID              SIZE      MODIFIED
qwen3.5:4b          2a654d98e6fb    3.4 GB    2 days ago
llama3.1:latest     46e0c10c039e    4.9 GB    3 weeks ago
mi-modelo:latest    0123456789ab    12 GB     1 hour ago
L
"$VI" modelos --json >"$T/json"
probar "marca los que ya tienes" "[[ \$(json '[x[\"modelo\"] for x in d[\"modelos\"] if x[\"tienes\"]]') == \"['qwen3.5:4b', 'llama3.1:8b']\" ]]"
probar "uno tuyo que no está en la tabla sale aparte, y se mide si cabe" "[[ \$(cabe mi-modelo:latest) == parcial ]]"
"$VI" modelos >"$T/salida"
probar "en el texto dice «ya lo tienes»" "grep -q 'qwen3.5:4b .*ya lo tienes' '$T/salida'"

amd 12 1
"$VI" modelos --json >"$T/json"
"$VI" modelos >"$T/salida"
probar "AMD: mide la VRAM en /sys/class/drm" "grep -q '^Tu gráfica: Radeon RX 6700 XT con 12 GB (11 GB libres ahora)' '$T/salida'"
probar "AMD 12 GB: qwen2.5:14b cabe entero" "[[ \$(cabe qwen2.5:14b) == gpu ]]"
probar "AMD 12 GB: gemma2:27b solo en parte" "[[ \$(cabe gemma2:27b) == parcial ]]"

# La PC de entrada: Ryzen con gráficos integrados, RX 550 y 16 GB de RAM.
LSPCI_RX550='03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Lexa PRO [Radeon 540/540X/550/550X / RX 540X/550/550X] [1002:699f] (rev c7)'
LSPCI_RENOIR='30:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Renoir [Radeon Vega Series / Radeon Vega Mobile Series] [1002:1636] (rev d9)'
for gb in 2 4; do
  amd "$gb" 0
  printf '%s\n%s\n' "$LSPCI_RX550" "$LSPCI_RENOIR" >"$T/lspci"
  "$VI" modelos --json >"$T/json"
  "$VI" modelos >"$T/salida"
  probar "RX 550 de $gb GB: dice que se queda corta" "grep -q 'Se queda corta para la IA' '$T/salida'"
  probar "RX 550 de $gb GB: no manda a una IA de internet" "! grep -q 'Mejor una en internet' '$T/salida'"
  probar "RX 550 de $gb GB: recomienda qwen3.5:4b en el procesador" "[[ \$(json 'd[\"recomendado\"]') == qwen3.5:4b && \$(cabe qwen3.5:4b) == cpu ]]"
  probar "RX 550 de $gb GB: qwen2.5:14b cabe en la RAM" "[[ \$(cabe qwen2.5:14b) == cpu ]]"
done
# Una de 2 GB que no es de entrada por su nombre tampoco sirve; una RX 5500 XT sí.
amd 2 0
"$VI" modelos --json >"$T/json"
probar "cualquier tarjeta de 2 GB: los modelos van al procesador" "[[ \$(cabe llama3.2:3b) == cpu ]]"
amd 8 0
echo '03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Navi 14 [Radeon RX 5500/5500M / Pro 5500M] [1002:7340] (rev c5)' >"$T/lspci"
"$VI" modelos --json >"$T/json"
probar "RX 5500 no se confunde con la RX 550" "[[ \$(cabe llama3.2:3b) == gpu ]]"

sin_gpu
"$VI" modelos --json >"$T/json"
"$VI" modelos >"$T/salida"
probar "sin gráfica: se queda con la RAM y lo dice" "grep -q 'viene en el procesador (Iris Xe Graphics): la IA usa tus 16 GB de RAM' '$T/salida'"
probar "sin gráfica: los chicos caben en la RAM" "[[ \$(cabe llama3.2:3b) == cpu ]]"
probar "sin gráfica: los grandes no" "[[ \$(cabe qwen2.5:32b) == no ]]"
probar "sin gráfica: recomienda uno chico" "[[ \$(json 'd[\"recomendado\"]') == qwen3.5:4b ]]"
: >"$T/lspci"
probar "sin nada en lspci tampoco se rompe" "'$VI' modelos | grep -q '^No encontré gráfica'"

nvidia "NVIDIA GeForce RTX 3080" 10240 8000 8.6
limpiar
SALIDA="$("$VI" modelos --medir qwen3.5:4b)"
probar "--medir da los tokens por segundo" "[[ '$SALIDA' == *'Qwen 3.5 4B: 87.5 tokens por segundo (va volando).' ]]"
limpiar
"$VI" modelos --medir gemma2:27b 2>"$T/error"
probar "--medir con uno que no tienes: no lo baja ni lo corre" "grep -q 'ollama pull gemma2:27b' '$T/error' && ! grep -qE '^(run|pull)' '$T/ollama-ordenes'"
probar "nunca baja modelos" "! grep -qE '\"pull\"|ollama\", \"pull' '$VI'"

echo "motor"
paquetes ollama ollama-vulkan
nvidia "NVIDIA GeForce RTX 3080" 10240 8000 8.6
SALIDA="$("$VI" motor --ver)"
probar "NVIDIA con Vulkan: cambiaría a CUDA y dice cuánto baja y ocupa" "[[ '$SALIDA' == 'Cambiaría Ollama a CUDA (ollama-cuda) en tu RTX 3080'*'Baja 2.8 GB y ocupa 5.7 GB'* ]]"
probar "--ver no pregunta ni instala" "[[ ! -e '$T/aviso' && ! -e '$T/sudo' ]]"
amd 12 1
probar "AMD: se queda en Vulkan" "'$VI' motor --ver | grep -q 'AMD: Ollama se queda con Vulkan'"
sin_gpu
probar "sin NVIDIA: se queda en Vulkan" "'$VI' motor --ver | grep -q 'No hay NVIDIA'"
echo "$LSPCI_NV" >"$T/lspci"
probar "NVIDIA sin su driver (nouveau): se queda en Vulkan" "'$VI' motor --ver | grep -q 'su driver no contesta'"
nvidia "NVIDIA GeForce GTX 1060 6GB" 6144 5000 6.1
probar "NVIDIA vieja (Pascal): el CUDA de Arch ya no la soporta" "'$VI' motor --ver | grep -q 'GTX 1060 6GB es de antes de la serie 16/20'"
nvidia "NVIDIA GeForce RTX 3080" 10240 8000 8.6
paquetes
probar "sin Ollama no hay nada que cambiar" "'$VI' motor --ver | grep -q 'Ollama no está instalado'"
paquetes ollama ollama-vulkan ollama-cuda
probar "con ollama-cuda ya puesto, lo dice" "'$VI' motor --ver | grep -q 'ya usa CUDA'"
paquetes ollama ollama-vulkan
echo 4000 >"$T/libre"
probar "sin disco para CUDA, no lo ofrece" "'$VI' motor --ver | grep -q 'no te alcanza el disco'"
echo 50000 >"$T/libre"

limpiar; rm -f "$MARCA"
PRUEBA_ACCION='' "$VI" motor --auto >/dev/null
probar "--auto pregunta con una notificación con botones" "grep -qx 'si=Cambiar ahora' '$T/aviso' && grep -qx 'no=No, gracias' '$T/aviso'"
probar "la pregunta dice cuánto baja" "grep -q 'Hay que bajar 2.8 GB (ocupa 5.7 GB)' '$T/aviso'"
probar "si nadie contesta: no instala y vuelve a preguntar otro día" "[[ ! -e '$T/sudo' && ! -e '$MARCA' ]]"
limpiar
PRUEBA_ACCION=si "$VI" motor --auto >"$T/salida"
probar "«Cambiar ahora» instala ollama-cuda con sudo, sin quitar Vulkan" "[[ \"\$(cat '$T/sudo')\" == 'pacman -S --needed --noconfirm ollama-cuda' ]]"
probar "y queda la marca de la máquina" "[[ \"\$(cat '$MARCA')\" == cuda ]]"
limpiar
PRUEBA_ACCION=si "$VI" motor --auto >/dev/null
probar "--auto pregunta una sola vez" "[[ ! -e '$T/aviso' && ! -e '$T/sudo' ]]"

paquetes ollama ollama-vulkan; limpiar; rm -f "$MARCA"
PRUEBA_ACCION=no "$VI" motor --auto >/dev/null
probar "«No, gracias» no instala nada" "[[ -e '$T/aviso' && ! -e '$T/sudo' && \"\$(cat '$MARCA')\" == no ]]"
limpiar
PRUEBA_ACCION=si "$VI" motor --auto >/dev/null
probar "después del «no», no vuelve a preguntar" "[[ ! -e '$T/aviso' && ! -e '$T/sudo' ]]"
probar "pero a mano sí se puede: --ver dice qué haría" "'$VI' motor --ver | grep -q '^Cambiaría'"

rm -f "$MARCA"; limpiar; amd 12 1
PRUEBA_ACCION=si "$VI" motor --auto >"$T/salida"
probar "--auto en AMD: ni pregunta ni dice nada" "[[ ! -e '$T/aviso' && ! -e '$T/sudo' && ! -s '$T/salida' ]]"
nvidia "NVIDIA GeForce RTX 3080" 10240 8000 8.6; paquetes; limpiar
PRUEBA_ACCION=si "$VI" motor --auto >/dev/null
probar "--auto sin Ollama instalado: nada" "[[ ! -e '$T/aviso' && ! -e '$T/sudo' && ! -e '$MARCA' ]]"
probar "pacman solo se lee (nada de -S sin sudo, ni -R)" "[[ ! -e '$T/pacman-raro' ]]"

echo "vigilar"
nvidia "NVIDIA GeForce RTX 3080" 10240 8000 8.6
cat >"$T/ps" <<'P'
NAME           ID              SIZE      PROCESSOR          CONTEXT    UNTIL
llama3.1:8b    46e0c10c039e    6.9 GB    45%/55% CPU/GPU    8192       4 minutes from now
qwen3.5:4b     2a654d98e6fb    3.4 GB    100% GPU           8192       4 minutes from now
P
limpiar; echo 5 >"$T/ps-veces"
PRUEBA_ACCION=ver timeout 10 "$VI" vigilar
probar "termina solo cuando ya no hay modelos cargados" "(( \$? == 0 )) && (( \$(cat '$T/ps-cuenta') == 6 ))"
probar "avisa una sola vez por modelo, aunque lo vea 5 veces" "[[ \$(avisos) == 1 ]]"
probar "dice qué modelo, cuánto va al procesador y uno más chico" "grep -qx 'Llama 3.1 8B no cabe completo en tu gráfica y va lento (45 % en el procesador). Cierra lo que use la gráfica o prueba uno más chico: qwen2.5:7b.' '$T/aviso'"
probar "el que cabe al 100 % en la gráfica no avisa" "! grep -q 'Qwen 3.5 4B' '$T/aviso'"
probar "el botón abre el administrador de tareas" "grep -qx 'ver=Ver qué usa la gráfica' '$T/aviso' && grep -q 'vortex-task-manager' '$T/ordenes'"

limpiar; echo 999 >"$T/ps-veces"
PRUEBA_ACCION='' "$VI" vigilar &
PRIMERO=$!
sleep 0.6
INICIO=$(date +%s%N)
timeout 5 "$VI" vigilar
probar "candado: el segundo vigilar sale luego luego" "(( \$? == 0 && (\$(date +%s%N) - $INICIO) < 2000000000 ))"
probar "y no avisa otra vez" "[[ \$(avisos) == 1 ]]"
probar "el primero sigue vigilando" "kill -0 $PRIMERO 2>/dev/null"
echo 0 >"$T/ps-veces"
timeout 5 tail --pid="$PRIMERO" -f /dev/null
probar "al descargarse el modelo, el primero termina solo" "! kill -0 $PRIMERO 2>/dev/null"
limpiar
PRUEBA_ACCION='' timeout 5 "$VI" vigilar
probar "el candado se suelta al terminar" "(( \$? == 0 )) && [[ -e '$T/ps-cuenta' ]]"

rm -f "$T/ps"; limpiar
INICIO=$(date +%s%N)
timeout 5 "$VI" vigilar
probar "si Ollama no corre, espera un rato y termina" "(( \$? == 0 && (\$(date +%s%N) - $INICIO) < 3000000000 ))"

cat >"$T/ps" <<'P'
NAME           ID              SIZE      PROCESSOR    CONTEXT    UNTIL
qwen3.5:4b     2a654d98e6fb    3.4 GB    100% CPU     8192       4 minutes from now
P
sin_gpu; limpiar; echo 3 >"$T/ps-veces"
PRUEBA_ACCION='' timeout 5 "$VI" vigilar
probar "sin gráfica todo va al procesador: no avisa (no ayudaría)" "[[ ! -e '$T/aviso' ]]"
nvidia "NVIDIA GeForce RTX 3080" 10240 300 8.6; limpiar
PRUEBA_ACCION='' timeout 5 "$VI" vigilar
probar "con gráfica pero todo en el procesador: avisa" "grep -q 'Qwen 3.5 4B no cupo en tu gráfica y va todo en el procesador' '$T/aviso'"

probar "sintaxis de python" "python3 -c \"import ast;ast.parse(open('$VI').read())\""
probar "sin orden: dice cómo se usa" "! '$VI' 2>&1 | grep -q Traceback && ! '$VI' >/dev/null 2>&1"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
