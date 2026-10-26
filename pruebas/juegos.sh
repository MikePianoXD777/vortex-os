#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de juegos.sh (1.8, la biblioteca de juegos de Super+Shift+G) con un HOME falso: dos
# bibliotecas de Steam (una en otro disco), herramientas de Steam que no son juegos, portadas
# verticales y horizontales, y la caché de Heroic.
#
#   pruebas/juegos.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$AQUI/shell/src/quickshell/juegos/juegos.sh"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-juegos-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

H="$T/home"; S="$H/.local/share/Steam"; OTRO="$T/disco/SteamLibrary"
mkdir -p "$S/steamapps" "$OTRO/steamapps" "$H/.config/heroic/store_cache" "$H/.config/heroic/store"
printf '"libraryfolders"\n{\n\t"0"\n\t{\n\t\t"path"\t\t"%s"\n\t}\n\t"1"\n\t{\n\t\t"path"\t\t"%s"\n\t}\n}\n' "$S" "$OTRO" \
  >"$S/steamapps/libraryfolders.vdf"
manifiesto() { # BIBLIOTECA APPID NOMBRE ULTIMO
  printf '"AppState"\n{\n\t"appid"\t\t"%s"\n\t"name"\t\t"%s"\n\t"LastPlayed"\t\t"%s"\n\t"InstalledDepots"\n\t{\n\t\t"name"\t\t"no"\n\t}\n}\n' \
    "$2" "$3" "$4" >"$1/steamapps/appmanifest_$2.acf"
}
manifiesto "$S" 1493710 "Proton Experimental" 0
manifiesto "$S" 1391110 "Steam Linux Runtime 2.0 (soldier)" 0
manifiesto "$S" 228980 "Steamworks Common Redistributables" 0
manifiesto "$OTRO" 236390 "War Thunder" 1700000400
manifiesto "$OTRO" 517630 "Just Cause 4" 1700000100
manifiesto "$OTRO" 582500 'We "Were" Here' 0
portada() { mkdir -p "$S/appcache/librarycache/$1/${3:-}"; head -c 2000 /dev/zero >"$S/appcache/librarycache/$1/${3:-}/$2"; }
portada 236390 library_capsule.jpg abc123
portada 236390 library_header.jpg def456
portada 517630 library_600x900.jpg
portada 582500 header.jpg
cat >"$H/.config/heroic/store_cache/legendary_library.json" <<'JSON'
{"__timestamp": 1, "library": [
  {"app_name": "Sugar", "title": "Rocket League®", "is_installed": true, "runner": "legendary",
   "art_square": "https://cdn.example/rocket.jpg", "art_cover": "https://cdn.example/wide.jpg"},
  {"app_name": "Fortnite", "title": "Fortnite", "is_installed": false, "art_square": "x"}
]}
JSON
printf '{"Sugar": {"firstPlayed": "2023-11-14T22:13:20.000Z", "lastPlayed": "2023-11-14T22:18:20.500Z"}}\n' \
  >"$H/.config/heroic/store/timestamp.json"

echo "== juegos.sh"
salida="$(HOME="$H" XDG_CONFIG_HOME="$H/.config" bash "$SCRIPT")"
jq -e 'type == "array"' >/dev/null <<<"$salida" && bien "JSON válido" || mal "JSON: $salida"
nombres="$(jq -r '[.[].nombre] | join(" | ")' <<<"$salida")"
[[ "$nombres" == 'War Thunder | Rocket League® | Just Cause 4 | We "Were" Here' ]] &&
  bien "lo último jugado primero, de las dos bibliotecas y de Heroic" || mal "orden: $nombres"
grep -qE "Proton|Runtime|Redistributables|Fortnite" <<<"$nombres" && mal "metió herramientas o lo no instalado" ||
  bien "sin Proton, runtimes, redistribuibles ni lo no instalado de Heroic"
[[ "$(jq -r '.[] | select(.nombre == "War Thunder") | "\(.vertical) \(.portada | split("/") | last) \(.lanzar)"' <<<"$salida")" == "true library_capsule.jpg steam://rungameid/236390" ]] &&
  bien "portada vertical del formato nuevo (carpeta con hash) y cómo lanzarlo" || mal "War Thunder: $(jq -c '.[0]' <<<"$salida")"
[[ "$(jq -r '.[] | select(.id == "steam:517630") | "\(.vertical) \(.portada | split("/") | last)"' <<<"$salida")" == "true library_600x900.jpg" ]] &&
  bien "portada vertical del formato viejo" || mal "Just Cause 4"
[[ "$(jq -r '.[] | select(.id == "steam:582500") | "\(.vertical) \(.portada | split("/") | last)"' <<<"$salida")" == "false header.jpg" ]] &&
  bien "sin vertical, la horizontal (y comillas en el nombre)" || mal "We Were Here: $(jq -c '.[] | select(.id == "steam:582500")' <<<"$salida")"
[[ "$(jq -r '.[] | select(.origen == "heroic") | "\(.portada) \(.lanzar) \(.ultimo)"' <<<"$salida")" == "https://cdn.example/rocket.jpg heroic://launch?appName=Sugar&runner=legendary 1700000300" ]] &&
  bien "Heroic: portada de la tienda, enlace para lanzarlo y cuándo jugaste" || mal "Heroic: $(jq -c '.[] | select(.origen == "heroic")' <<<"$salida")"

vacio="$(HOME="$T/vacio" XDG_CONFIG_HOME="$T/vacio/.config" bash "$SCRIPT")"
[[ "$(jq -c . <<<"$vacio")" == "[]" ]] && bien "sin Steam ni Heroic: lista vacía" || mal "vacío: $vacio"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
