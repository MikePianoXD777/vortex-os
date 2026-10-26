#!/usr/bin/env bash
# Vortex 1.8: los juegos instalados de Steam y Heroic para la biblioteca (JuegosPopup.qml), en JSON:
#
#   [{"id": "steam:236390", "nombre": "War Thunder", "origen": "steam", "portada": RUTA o URL o "",
#     "vertical": true, "lanzar": "steam://rungameid/236390", "ultimo": 1791761627}, ...]
#
# Lo último que jugaste va primero. Steam: cada biblioteca de libraryfolders.vdf (también la de Steam
# en Flatpak), sus appmanifest_*.acf y las portadas de appcache/librarycache (la vertical si existe;
# si no, la horizontal). Se saltan Proton, los Steam Linux Runtime y los redistribuibles. Heroic: lo
# que su caché marca como instalado (Epic, GOG y Amazon), con la portada de la tienda. Los demás
# juegos (Flatpak, Minecraft, los .desktop con Categories=Game) los agrega el panel por su cuenta.
set -uo pipefail

steam_raices() {
  local r
  for r in "$HOME/.local/share/Steam" "$HOME/.steam/steam" "$HOME/.var/app/com.valvesoftware.Steam/.local/share/Steam"; do
    [[ -d "$r/steamapps" ]] && realpath "$r"
  done | sort -u
}

# «"path"  "/mnt/juegos/SteamLibrary"» → /mnt/juegos/SteamLibrary
steam_bibliotecas() {
  local raiz
  while IFS= read -r raiz; do
    printf '%s\n' "$raiz"
    sed -nE 's/^[[:space:]]*"path"[[:space:]]+"(.*)"[[:space:]]*$/\1/p' "$raiz/steamapps/libraryfolders.vdf" 2>/dev/null
  done < <(steam_raices) | while IFS= read -r b; do [[ -d "$b/steamapps" ]] && realpath "$b"; done | sort -u
}

# Valor de una clave de primer nivel de un .acf: «"name"  "War Thunder"».
acf() { sed -nE "s/^[[:space:]]*\"$2\"[[:space:]]+\"(.*)\"[[:space:]]*$/\1/p" "$1" | head -n1; }

portada_steam() { # APPID → «RUTA vertical» (vertical = true/false), o nada
  local raiz dir nombre f
  while IFS= read -r raiz; do
    dir="$raiz/appcache/librarycache/$1"
    [[ -d "$dir" ]] || continue
    for nombre in library_600x900.jpg library_capsule.jpg library_header.jpg header.jpg; do
      f="$(find "$dir" -maxdepth 2 -name "$nombre" -size +1k -print -quit 2>/dev/null)"
      if [[ -n "$f" ]]; then
        case "$nombre" in library_600x900.jpg | library_capsule.jpg) printf '%s true\n' "$f" ;; *) printf '%s false\n' "$f" ;; esac
        return 0
      fi
    done
  done < <(steam_raices)
}

steam() {
  local bib m appid nombre ultimo p ruta vertical
  while IFS= read -r bib; do
    for m in "$bib"/steamapps/appmanifest_*.acf; do
      [[ -f "$m" ]] || continue
      appid="$(acf "$m" appid)"
      nombre="$(acf "$m" name)"
      [[ "$appid" =~ ^[0-9]+$ && -n "$nombre" ]] || continue
      [[ "$nombre" =~ ^(Proton|Steam\ Linux\ Runtime|Steamworks\ Common) ]] && continue
      ultimo="$(acf "$m" LastPlayed)"
      p="$(portada_steam "$appid")"
      ruta="${p% *}"
      vertical="${p##* }"
      jq -cn --arg id "steam:$appid" --arg nombre "$nombre" --arg portada "$ruta" \
        --argjson vertical "$([[ "$vertical" == true ]] && echo true || echo false)" \
        --arg lanzar "steam://rungameid/$appid" --argjson ultimo "$([[ "$ultimo" =~ ^[0-9]+$ ]] && echo "$ultimo" || echo 0)" \
        '{id: $id, nombre: $nombre, origen: "steam", portada: $portada, vertical: $vertical, lanzar: $lanzar, ultimo: $ultimo}'
    done
  done < <(steam_bibliotecas)
}

heroic() {
  local base="${XDG_CONFIG_HOME:-$HOME/.config}/heroic" f runner fechas
  # Cuándo jugaste: store/timestamp.json, {"APP": {"lastPlayed": "2026-10-12T01:07:00.219Z", ...}}.
  fechas="$(cat "$base/store/timestamp.json" 2>/dev/null)"
  jq -e 'type == "object"' >/dev/null 2>&1 <<<"$fechas" || fechas='{}'
  for f in legendary_library.json gog_library.json nile_library.json; do
    [[ -f "$base/store_cache/$f" ]] || continue
    runner="${f%%_*}"
    jq -c --arg runner "$runner" --argjson fechas "$fechas" '(.library // .games // [])[]
      | select(.is_installed == true and (.app_name // "") != "")
      | {id: ("heroic:" + .app_name), nombre: (.title // .app_name), origen: "heroic",
         portada: (.art_square // .art_cover // ""), vertical: ((.art_square // "") != ""),
         lanzar: ("heroic://launch?appName=" + (.app_name | @uri) + "&runner=" + $runner),
         ultimo: (($fechas[.app_name] // {}) | (.lastPlayed // .firstPlayed // "")
                  | (try (sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601 | floor) catch 0))}' \
      "$base/store_cache/$f" 2>/dev/null
  done
}

{ steam; heroic; } | jq -s 'unique_by(.id) | sort_by(-.ultimo, (.nombre | ascii_downcase))'
