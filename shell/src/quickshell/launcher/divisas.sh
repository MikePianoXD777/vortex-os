#!/usr/bin/env bash
# Vortex 1.9: tipos de cambio para la calculadora del lanzador (Calculadora.js), base USD, de
# open.er-api.com (gratis y sin clave; se actualiza una vez al día). Se guardan en
# ~/.cache/vortex/divisas.json y se reusan 12 h; sin internet, los últimos que haya. Imprime
# {"USD": 1, "MXN": 18.5, ...}, o {} si nunca se pudieron bajar.
set -uo pipefail
cache="${XDG_CACHE_HOME:-$HOME/.cache}/vortex/divisas.json"
url="${VORTEX_DIVISAS_URL:-https://open.er-api.com/v6/latest/USD}"

if [[ ! -s "$cache" ]] || (($(date +%s) - $(stat -c %Y "$cache") > 43200)); then
  tmp="$(mktemp "${TMPDIR:-/tmp}/vortex-divisas.XXXXXX")"
  if curl -fsSL --max-time 6 "$url" -o "$tmp" 2>/dev/null &&
    jq -e '.result == "success" and .rates.USD == 1' "$tmp" >/dev/null 2>&1; then
    mkdir -p "${cache%/*}"
    jq -c '.rates' "$tmp" >"$cache.nuevo" && mv "$cache.nuevo" "$cache"
  fi
  rm -f "$tmp"
fi
if [[ -s "$cache" ]]; then cat "$cache"; else echo '{}'; fi
