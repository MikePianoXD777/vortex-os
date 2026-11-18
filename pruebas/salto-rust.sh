#!/usr/bin/env bash
# shellcheck disable=SC2015  # bien() y mal() nunca fallan
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

for a in task-manager runner video shop app-installer connect shared; do
  echo "vortex-$a"
  lanzador="apps/vortex-$a/vortex-$a"
  [[ -f "$lanzador" ]] || lanzador="apps/$a/vortex-$a"
  var="VORTEX_$(echo "$a" | tr 'a-z-' 'A-Z_')_PYTHON"
  printf '#!/bin/sh\necho "rust $*"\n' >"$tmp/lib/vortex-$a-rs"
  chmod +x "$tmp/lib/vortex-$a-rs"
  r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" "$lanzador" --hola 2>&1)"
  [[ "$r" == "rust --hola" ]] && bien "con el binario de Rust, salta a él (y le pasa los argumentos)" || mal "no saltó a Rust: $r"
  r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" env "$var=1" "$lanzador" --hola 2>&1)"
  [[ "$r" == python* ]] && bien "$var=1 obliga a usar la de Python" || mal "$var=1 no forzó Python: $r"
  rm "$tmp/lib/vortex-$a-rs"
  r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" "$lanzador" --hola 2>&1)"
  [[ "$r" == python* ]] && bien "sin el binario de Rust, sigue con la de Python" || mal "sin Rust no usó Python: $r"
done

echo "vortex-shared: el servicio va al motor sin Qt"
printf '#!/bin/sh\necho "rust $*"\n' >"$tmp/lib/vortex-shared-rs"
printf '#!/bin/sh\necho "motor $*"\n' >"$tmp/lib/vortex-shared-motor"
chmod +x "$tmp/lib/vortex-shared-rs" "$tmp/lib/vortex-shared-motor"
r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" apps/vortex-shared/vortex-shared escuchar 2>&1)"
[[ "$r" == "motor escuchar" ]] && bien "«escuchar» (el socket) lo atiende vortex-shared-motor" || mal "escuchar no fue al motor: $r"
r="$(PATH="$tmp/bin:$PATH" VORTEX_RUST_BIN="$tmp/lib" apps/vortex-shared/vortex-shared foto.png 2>&1)"
[[ "$r" == "rust foto.png" ]] && bien "con archivos abre la ventana de Rust" || mal "con archivos no abrió la ventana: $r"

echo "Paquete vortex-rust"
grep -q 'vortex-shared-motor' packaging/vortex-rust/PKGBUILD && bien "instala vortex-shared-motor" || mal "no instala vortex-shared-motor"
for a in task-manager runner video shop app-installer connect shared; do
  grep -q "vortex-$a-rs" packaging/vortex-rust/PKGBUILD && bien "instala vortex-$a-rs" || mal "no instala vortex-$a-rs"
done

((fallos == 0)) && echo "Todo bien" || { echo "$fallos fallos"; exit 1; }
