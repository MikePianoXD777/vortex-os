#!/bin/sh
# Las pruebas de Rust como manda el proyecto: límite de memoria, sin abrir ventanas y con HOME temporal.
# Uso: rust/pruebas.sh   (desde cualquier carpeta)
set -eu
cd "$(dirname "$0")"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# Con 2 GB, más de 4 compilaciones de C++ (cxx-qt) a la vez se quedan sin memoria.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" QMAKE="${QMAKE:-qmake6}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export HOME="$tmp" XDG_CONFIG_HOME="$tmp/config" XDG_STATE_HOME="$tmp/state" XDG_DATA_HOME="$tmp/data" \
       XDG_CACHE_HOME="$tmp/cache" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
unset WAYLAND_DISPLAY DISPLAY
cargo fmt --all -- --check
systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- timeout 3000 \
  sh -c 'cargo clippy --all-targets -- -D warnings && cargo test'
