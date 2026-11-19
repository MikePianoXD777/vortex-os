#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2053  # bien() y mal() nunca fallan; $salida_py es un patrón a propósito
# Los lanzadores de Python de las apps ya pasadas a Rust (4.0) saltan al binario de Rust si está.
# Usa un binario falso y un python3 falso: nunca abre una app de verdad.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
fallos=0
bien() { printf '  ok  %s\n' "$1"; }
mal() { printf '  MAL %s\n' "$1"; fallos=$((fallos + 1)); }
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/lib" "$tmp/bin"
printf '#!/bin/sh\necho "python $*"\n' >"$tmp/bin/python3"
chmod +x "$tmp/bin/python3"

for a in task-manager runner video shop app-installer connect shared camera tareas enlace memoria; do
  echo "vortex-$a"
  lanzador="apps/vortex-$a/vortex-$a"
  [[ -f "$lanzador" ]] || lanzador="apps/$a/vortex-$a"
  # El de las tareas es un script de Python (en bin/): se corre con el python3 de verdad (con el falso no
  # llegaría ni a leer el salto); «--hola» no es una orden suya y solo imprime cómo se usa.
  py="$tmp/bin"
  salida_py="python*"
  if [[ "$a" == tareas || "$a" == enlace ]]; then
    lanzador="bin/vortex-$a"
    py="$tmp/sin-python"
    salida_py="vortex-$a (Vortex 2.5)*"
    mkdir -p "$py"
  fi
  # vortex-memoria no tiene orden de «uso»: con --ver solo dice qué avisaría (nunca manda el aviso), y con un
  # /proc vacío no lee nada de esta PC.
  arg="--hola"
  if [[ "$a" == memoria ]]; then
    lanzador="bin/vortex-memoria"
    py="$tmp/sin-python"
    salida_py="Quedan 0 MB.*"
    arg="--ver"
    mkdir -p "$py" "$tmp/proc-vacio"
  fi
  var="VORTEX_$(echo "$a" | tr 'a-z-' 'A-Z_')_PYTHON"
  printf '#!/bin/sh\necho "rust $*"\n' >"$tmp/lib/vortex-$a-rs"
  chmod +x "$tmp/lib/vortex-$a-rs"
  r="$(PATH="$py:$PATH" VORTEX_MEMORIA_PROC="$tmp/proc-vacio" VORTEX_RUST_BIN="$tmp/lib" "$lanzador" "$arg" 2>&1)"
  [[ "$r" == "rust $arg" ]] && bien "con el binario de Rust, salta a él (y le pasa los argumentos)" || mal "no saltó a Rust: $r"
  r="$(PATH="$py:$PATH" VORTEX_MEMORIA_PROC="$tmp/proc-vacio" VORTEX_RUST_BIN="$tmp/lib" env "$var=1" "$lanzador" "$arg" 2>&1)"
  [[ "$r" == $salida_py ]] && bien "$var=1 obliga a usar la de Python" || mal "$var=1 no forzó Python: $r"
  rm "$tmp/lib/vortex-$a-rs"
  r="$(PATH="$py:$PATH" VORTEX_MEMORIA_PROC="$tmp/proc-vacio" VORTEX_RUST_BIN="$tmp/lib" "$lanzador" "$arg" 2>&1)"
  [[ "$r" == $salida_py ]] && bien "sin el binario de Rust, sigue con la de Python" || mal "sin Rust no usó Python: $r"
done

echo "vortex-usb (bash)"
# --eventos con la entrada vacía: el de bash no hace nada (nunca se pone a escuchar a udev de verdad)
printf '#!/bin/sh\necho "rust $*"\n' >"$tmp/lib/vortex-usb-rs"
chmod +x "$tmp/lib/vortex-usb-rs"
r="$(VORTEX_RUST_BIN="$tmp/lib" bin/vortex-usb --eventos </dev/null 2>&1)"
[[ "$r" == "rust --eventos" ]] && bien "con el binario de Rust, salta a él (y le pasa los argumentos)" || mal "no saltó a Rust: $r"
r="$(VORTEX_RUST_BIN="$tmp/lib" VORTEX_USB_BASH=1 bin/vortex-usb --eventos </dev/null 2>&1)"
[[ -z "$r" ]] && bien "VORTEX_USB_BASH=1 obliga a usar el de bash" || mal "VORTEX_USB_BASH=1 no forzó bash: $r"
rm "$tmp/lib/vortex-usb-rs"
r="$(VORTEX_RUST_BIN="$tmp/lib" bin/vortex-usb --eventos </dev/null 2>&1)"
[[ -z "$r" ]] && bien "sin el binario de Rust, sigue con el de bash" || mal "sin Rust no usó bash: $r"

echo "vortex-shared: el servicio va al motor sin Qt"
printf '#!/bin/sh\necho "rust $*"\n' >"$tmp/lib/vortex-shared-rs"
printf '#!/bin/sh\necho "motor $*"\n' >"$tmp/lib/vortex-shared-motor"
chmod +x "$tmp/lib/vortex-shared-rs" "$tmp/lib/vortex-shared-motor"
r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" apps/vortex-shared/vortex-shared escuchar 2>&1)"
[[ "$r" == "motor escuchar" ]] && bien "«escuchar» (el socket) lo atiende vortex-shared-motor" || mal "escuchar no fue al motor: $r"
r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" apps/vortex-shared/vortex-shared foto.png 2>&1)"
[[ "$r" == "rust foto.png" ]] && bien "con archivos abre la ventana de Rust" || mal "con archivos no abrió la ventana: $r"

echo "vortex-gestor: el servicio de recursos (sin python: el de bash es el viejo)"
# El de bash, si corriera de verdad, buscaría el Hyprland de la sesión: aquí no hay ninguno (carpeta temporal),
# así que sale enseguida con su aviso. Nada toca tu sesión.
mkdir -p "$tmp/run-gestor" "$tmp/home-gestor"
gestor() { # los comandos corren con el entorno de mentira y sin el Hyprland de verdad
  env -u HYPRLAND_INSTANCE_SIGNATURE PATH="$tmp/bin:$PATH" XDG_RUNTIME_DIR="$tmp/run-gestor" HOME="$tmp/home-gestor" \
    XDG_CONFIG_HOME="$tmp/home-gestor/c" XDG_STATE_HOME="$tmp/home-gestor/s" "$@" 2>&1
}
printf '#!/bin/sh\necho "rust $*"\n' >"$tmp/lib/vortex-gestor-rs"
chmod +x "$tmp/lib/vortex-gestor-rs"
r="$(gestor VORTEX_RUST_BIN="$tmp/lib" bin/vortex-gestor --hola)"
[[ "$r" == "rust --hola" ]] && bien "con el binario de Rust, salta a él (y le pasa los argumentos)" || mal "no saltó a Rust: $r"
r="$(gestor VORTEX_RUST_BIN="$tmp/lib" VORTEX_GESTOR_BASH=1 bin/vortex-gestor)"
[[ "$r" == *"no encuentro Hyprland"* || "$r" == *"falta vortex-eventos"* ]] && bien "VORTEX_GESTOR_BASH=1 obliga a usar el de bash" || mal "VORTEX_GESTOR_BASH=1 no forzó bash: $r"
rm "$tmp/lib/vortex-gestor-rs"
r="$(gestor VORTEX_RUST_BIN="$tmp/lib" bin/vortex-gestor)"
[[ "$r" == *"no encuentro Hyprland"* || "$r" == *"falta vortex-eventos"* ]] && bien "sin el binario de Rust, sigue con el de bash" || mal "sin Rust no usó bash: $r"

echo "Paquete vortex-rust"
grep -q 'vortex-shared-motor' packaging/vortex-rust/PKGBUILD && bien "instala vortex-shared-motor" || mal "no instala vortex-shared-motor"
for a in task-manager runner video shop app-installer connect shared camera tareas usb enlace memoria; do
  grep -q "vortex-$a-rs" packaging/vortex-rust/PKGBUILD && bien "instala vortex-$a-rs" || mal "no instala vortex-$a-rs"
done

grep -q 'vortex-gestor-rs' packaging/vortex-rust/PKGBUILD && bien "instala vortex-gestor-rs" || mal "no instala vortex-gestor-rs"
grep -q '_crates=(.*vortex-gestor' packaging/vortex-rust/PKGBUILD && bien "compila vortex-gestor" || mal "no compila vortex-gestor"

((fallos == 0)) && echo "Todo bien" || { echo "$fallos fallos"; exit 1; }
