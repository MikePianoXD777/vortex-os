#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex mods» (1.10, botones Modrinth y CurseForge del calendario) en un HOME temporal con
# instancias falsas de Prism, TLauncher y una carpeta agregada a mano: no toca tu Minecraft. Lo de
# Modrinth (buscar e instalar) solo se prueba si hay internet, y baja a la carpeta temporal.
#
#   pruebas/mods.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-mods.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

H="$T/home"
mkdir -p "$T/bin" "$H/Descargas"
printf '#!/bin/sh\necho "%s"\n' "$H/Descargas" >"$T/bin/xdg-user-dir"
chmod +x "$T/bin/xdg-user-dir"
mods() {
  HOME="$H" XDG_CONFIG_HOME="$H/.config" XDG_DATA_HOME="$H/.local/share" PATH="$T/bin:$PATH" \
    bash "$AQUI/bin/vortex" mods "$@"
}

# Prism (Flatpak): una con NeoForge y otra con Fabric (carpeta «.minecraft», como las viejas)
P="$H/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances"
mkdir -p "$P/Mi pack/minecraft" "$P/Fabricosa/.minecraft"
printf 'name=Mi pack\n' >"$P/Mi pack/instance.cfg"
echo '{"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.neoforged","version":"21.1.77"}]}' >"$P/Mi pack/mmc-pack.json"
printf 'name=Fabricosa\n' >"$P/Fabricosa/instance.cfg"
echo '{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16"}]}' >"$P/Fabricosa/mmc-pack.json"
# TLauncher: .minecraft con su versión de Fabric (la más nueva manda)
mkdir -p "$H/.tlauncher" "$H/.minecraft/versions/1.20.4" "$H/.minecraft/versions/fabric-loader-0.19.5-1.21.3"
touch -d '2 days ago' "$H/.minecraft/versions/1.20.4"
# Otro launcher, agregado a mano
mkdir -p "$T/otro launcher/juego/versions/neoforge-21.4.10"

echo "== vortex mods instancias"
mods instancias >"$T/inst.json"
probar "salen las 3 (2 de Prism y TLauncher)" "[[ \$(jq length '$T/inst.json') == 3 ]]"
probar "Prism con NeoForge: versión y loader del mmc-pack" \
  "jq -e '.[] | select(.nombre == \"Mi pack\" and .launcher == \"Prism\" and .version == \"1.21.1\" and .loader == \"neoforge\" and (.ruta | endswith(\"Mi pack/minecraft\")))' '$T/inst.json' >/dev/null"
probar "Prism con carpeta .minecraft y Fabric" \
  "jq -e '.[] | select(.nombre == \"Fabricosa\" and .loader == \"fabric\" and (.ruta | endswith(\"Fabricosa/.minecraft\")))' '$T/inst.json' >/dev/null"
probar "TLauncher: adivina 1.21.3 con Fabric de versions/" \
  "jq -e '.[] | select(.launcher == \"TLauncher\" and .version == \"1.21.3\" and .loader == \"fabric\")' '$T/inst.json' >/dev/null"

echo "== vortex mods agregar / quitar"
mods agregar "$T/otro launcher/juego" >/dev/null
mods agregar "$T/otro launcher/juego" >/dev/null
probar "se guarda una sola vez" "[[ \$(grep -c . '$H/.config/vortex/minecraft-carpetas') == 1 ]]"
mods instancias >"$T/inst.json"
probar "la tuya sale al final, con NeoForge 21.4 = 1.21.4" \
  "jq -e '.[-1] | select(.launcher == \"Tuya\" and .nombre == \"juego\" and .version == \"1.21.4\" and .loader == \"neoforge\")' '$T/inst.json' >/dev/null"
probar "una carpeta que no existe no se agrega" "! mods agregar '$T/no-existe' 2>/dev/null"
mods quitar "$T/otro launcher/juego"
probar "quitar la saca de la lista (sin borrar nada)" "[[ \$(mods instancias | jq length) == 3 && -d '$T/otro launcher/juego' ]]"

echo "== vortex mods descargas / mover (lo de CurseForge)"
echo jar >"$H/Descargas/jei-1.21.1.jar"
mkdir -p "$T/zip/shaders" "$T/zip2"
echo x >"$T/zip/shaders/final.fsh"
echo '{}' >"$T/zip2/pack.mcmeta"
(cd "$T/zip" && python3 -I -m zipfile -c "$H/Descargas/Complementary.zip" shaders)
(cd "$T/zip2" && python3 -I -m zipfile -c "$H/Descargas/Faithful.zip" pack.mcmeta)
echo viejo >"$H/Descargas/viejo.jar"
touch -d '10 days ago' "$H/Descargas/viejo.jar"
touch "$H/Descargas/nota.txt"
mods descargas >"$T/desc.json"
probar "salen los .jar y .zip recientes (no el viejo ni el .txt)" "[[ \$(jq -r '.[].nombre' '$T/desc.json' | sort | tr '\n' ' ') == 'Complementary.zip Faithful.zip jei-1.21.1.jar ' ]]"
DEST="$P/Mi pack/minecraft"
mods mover "$H/Descargas/jei-1.21.1.jar" "$DEST" >/dev/null
probar "el .jar va a mods/" "[[ -f '$DEST/mods/jei-1.21.1.jar' && ! -e '$H/Descargas/jei-1.21.1.jar' ]]"
mods mover "$H/Descargas/Complementary.zip" "$DEST" >/dev/null
probar "un .zip con shaders/ va a shaderpacks/" "[[ -f '$DEST/shaderpacks/Complementary.zip' ]]"
mods mover "$H/Descargas/Faithful.zip" "$DEST" >/dev/null
probar "un .zip con pack.mcmeta va a resourcepacks/" "[[ -f '$DEST/resourcepacks/Faithful.zip' ]]"
echo otro >"$H/Descargas/jei-1.21.1.jar"
probar "no pisa uno con el mismo nombre" "! mods mover '$H/Descargas/jei-1.21.1.jar' '$DEST' 2>/dev/null && [[ \$(cat '$DEST/mods/jei-1.21.1.jar') == jar ]]"

echo "== vortex mods buscar / instalar (Modrinth)"
if curl -fsS --max-time 5 -o /dev/null https://api.modrinth.com/v2/tag/loader; then
  mods buscar --version 1.20.1 --loader fabric sodium >"$T/busca.json"
  probar "busca Sodium para 1.20.1 Fabric" "jq -e '.[] | select(.slug == \"sodium\" and .id == \"AANobbMI\")' '$T/busca.json' >/dev/null"
  probar "los filtros de mods traen Optimización con su ícono" "bash '$AQUI/bin/vortex-mods' categorias | jq -e '.[] | select(.nombre == \"optimization\" and .grupo == \"categories\" and (.icono | startswith(\"<svg\")))' >/dev/null"
  probar "los de shaders traen rendimiento (PC lenta)" "bash '$AQUI/bin/vortex-mods' categorias --tipo shader | jq -e '.[-1].grupo == \"performance impact\" and any(.[]; .nombre == \"potato\")' >/dev/null"
  probar "filtrar por categoría cambia lo que sale" "[[ \$(mods buscar --version 1.20.1 --loader fabric --categoria optimization | jq -r '.[0].id') != \$(mods buscar --version 1.20.1 --loader fabric --categoria magic | jq -r '.[0].id') ]]"
  F="$P/Fabricosa/.minecraft"
  mods instalar AANobbMI "$F" --version 1.20.1 --loader fabric >/dev/null
  probar "instala Sodium en la instancia elegida" "compgen -G '$F/mods/sodium-fabric-*.jar' >/dev/null"
  probar "la marca como instalada" "mods instalados '$F' | jq -e 'index(\"AANobbMI\")' >/dev/null"
  probar "si no hay versión, lo dice y no baja nada" "! mods instalar AANobbMI '$F' --version 1.2 --loader fabric 2>/dev/null"
else
  echo "  --  sin internet: no se prueba Modrinth"
fi

echo
if ((FALLOS)); then echo "$FALLOS fallaron"; exit 1; fi
echo "Todo bien"
