#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex rendimiento» (3.3, los perfiles Ultra, Alto, Medio y Bajo) con un escritorio falso: hyprctl,
# quickshell, notify-send y systemd-run falsos en un PATH temporal y un HOME temporal. No toca la sesión de verdad.
#
#   pruebas/rendimiento.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-rendimiento-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config/serpantinum" "$T/home/.config/vortex" "$T/home/.local/state/vortex" "$T/run"
for c in hyprctl quickshell notify-send systemd-run vortex-pantallas powerprofilesctl; do
  printf '#!/bin/sh\necho "%s $*" >>"%s/registro"\nexit 0\n' "$c" "$T" >"$T/bin/$c"
done
chmod +x "$T/bin/"*
: >"$T/registro"
E="$T/home/.local/state/vortex"
A="$T/home/.config/serpantinum/settings.json"
M="$T/home/.config/vortex/rendimiento"
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_STATE_HOME="$T/home/.local/state" XDG_RUNTIME_DIR="$T/run" \
    HYPRLAND_INSTANCE_SIGNATURE=prueba VORTEX_SHELL_QML="$T/Shell.qml" PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}
ajuste() { jq -r '.rendimiento // "nada"' "$A"; }

echo "== vortex rendimiento"
[[ "$(vortex rendimiento)" == ultra ]] && bien "de fábrica: ultra" || mal "de fábrica"
[[ ! -e "$M" ]] && bien "ultra no deja archivo" || mal "ultra dejó archivo"
vortex rendimiento turbo >/dev/null 2>&1 && mal "aceptó un perfil que no existe" || bien "rechaza un perfil que no existe"
[[ ! -e "$M" ]] && bien "el perfil malo no cambia nada" || mal "el perfil malo escribió"

echo '{"otra": 1}' >"$A"
: >"$T/registro"
[[ "$(vortex rendimiento alto)" == alto ]] && bien "alto: lo dice" || mal "alto"
[[ "$(cat "$M")" == alto ]] && bien "alto: lo guarda para hyprland.lua" || mal "alto marca"
[[ "$(ajuste)" == alto && "$(jq -r .otra "$A")" == 1 ]] && bien "alto: lo guarda en settings.json sin perder lo demás" || mal "alto ajustes: $(cat "$A")"
grep -q "hyprctl reload" "$T/registro" && bien "alto: recarga Hyprland" || mal "alto recarga"
grep -q "vortex-pantallas" "$T/registro" && bien "alto: devuelve los Hz de las pantallas" || mal "alto pantallas"
! grep -q "serpantinum kill" "$T/registro" && bien "alto: no reinicia el shell" || mal "alto reinició"

: >"$T/registro"
vortex rendimiento alto >/dev/null
! grep -q "hyprctl reload" "$T/registro" && bien "el mismo perfil no recarga" || mal "recargó sin cambio"

# Con el modo juego puesto, la recarga no le devuelve los efectos al juego.
touch "$E/modo-juego"
: >"$T/registro"
vortex rendimiento medio >/dev/null
grep -q "hyprctl eval .*animations = { enabled = false }" "$T/registro" && bien "con modo juego: sigue sin efectos" || mal "modo juego: $(cat "$T/registro")"
rm -f "$E/modo-juego"

# Los clips: graban al jugar, menos en el perfil bajo.
[[ "$(vortex rendimiento)" == medio ]] && bien "estado: medio" || mal "estado medio"
: >"$T/registro"
vortex modo-juego on >/dev/null 2>&1
sleep 0.3
grep -q "systemd-run .*vortex-clips servicio" "$T/registro" && bien "medio: los clips graban al jugar" || mal "medio clips: $(cat "$T/registro")"
vortex modo-juego off >/dev/null 2>&1
vortex rendimiento bajo >/dev/null
: >"$T/registro"
vortex modo-juego on >/dev/null 2>&1
sleep 0.3
! grep -q "vortex-clips servicio" "$T/registro" && bien "bajo: los clips no graban al jugar" || mal "bajo clips"
vortex modo-juego off >/dev/null 2>&1
vortex rendimiento ultra >/dev/null
[[ ! -e "$M" && "$(ajuste)" == ultra ]] && bien "volver a ultra borra el archivo" || mal "ultra: $(cat "$A")"

rm -f "$A"
vortex rendimiento medio >/dev/null
[[ "$(ajuste)" == medio ]] && bien "sin settings.json lo crea" || mal "sin ajustes"

# hyprland.lua con un hl falso: qué efectos apaga cada perfil.
echo "== hyprland.lua"
if command -v lua >/dev/null; then
  cat >"$T/perfil.lua" <<'L'
-- Solo el bloque del perfil de hyprland.lua, con un hl que anota lo que se le pide.
local fuente = io.open(arg[1]):read("a")
local bloque = fuente:match("local f_perfil = .-\nend\n\npcall")
bloque = bloque:gsub("pcall$", "")
local dijo = {}
hl = { config = function(c)
  if c.animations then dijo[#dijo + 1] = "animaciones=" .. tostring(c.animations.enabled) end
  local d = c.decoration or {}
  if d.blur then dijo[#dijo + 1] = "desenfoque=" .. tostring(d.blur.enabled) end
  if d.shadow then dijo[#dijo + 1] = "sombras=" .. tostring(d.shadow.enabled) end
end }
home = arg[2]
assert(load(bloque))()
table.sort(dijo)
print(table.concat(dijo, " "))
L
  perfil_lua() { lua "$T/perfil.lua" "$AQUI/config/hypr/hyprland.lua" "$T/home"; }
  rm -f "$M"
  [[ -z "$(perfil_lua)" ]] && bien "ultra no apaga nada" || mal "ultra: $(perfil_lua)"
  echo alto >"$M"
  [[ "$(perfil_lua)" == "desenfoque=false" ]] && bien "alto: solo sin desenfoque" || mal "alto: $(perfil_lua)"
  for p in medio bajo; do
    echo "$p" >"$M"
    [[ "$(perfil_lua)" == "animaciones=false desenfoque=false" ]] && bien "$p: sin animaciones ni desenfoque" || mal "$p: $(perfil_lua)"
  done
else
  bien "(sin lua: se salta esta parte)"
fi

((FALLOS == 0)) && { echo; echo "Todas las pruebas pasaron."; exit 0; }
echo; echo "Fallaron $FALLOS."; exit 1
