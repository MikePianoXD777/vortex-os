#!/usr/bin/env bash
# Prueba de serpantinumd: intel_sin_ihd decide si los fondos de video se decodifican en el procesador
# (QT_FFMPEG_DECODING_HW_DEVICE_TYPES=none) porque la gráfica Intel no tiene un iHD que funcione.
# Arma un /sys/class/drm y un /usr/lib/dri falsos en una carpeta temporal; no toca nada del sistema.
set -uo pipefail

AQUI="$(cd "$(dirname "$0")/.." && pwd)"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT

sed -n '/^intel_viejo_para_ihd() {/,/^}/p; /^intel_sin_ihd() {/,/^}/p' "$AQUI/shell/bin/serpantinumd" >"$T/funciones.sh"
grep -q 'intel_sin_ihd' "$T/funciones.sh" || { echo "  MAL no encontré intel_sin_ihd en serpantinumd"; exit 1; }
# shellcheck source=/dev/null
source "$T/funciones.sh"

fallos=0
gpu() { # gpu NOMBRE VENDOR DEVICE [con-ihd]
    local d="$T/$1"
    mkdir -p "$d/drm/renderD128/device" "$d/dri"
    echo "$2" >"$d/drm/renderD128/device/vendor"
    echo "$3" >"$d/drm/renderD128/device/device"
    [ "${4:-}" = con-ihd ] && touch "$d/dri/iHD_drv_video.so"
    echo "$d"
}
caso() { # caso DESCRIPCION ESPERADO NOMBRE VENDOR DEVICE [con-ihd]
    local desc="$1" esperado="$2" d
    d="$(gpu "$3" "$4" "$5" "${6:-}")"
    local obtenido
    obtenido="$(intel_sin_ihd "$d/drm" "$d/dri")"
    if [ "$obtenido" = "$esperado" ]; then
        echo "  bien  $desc → $obtenido"
    else
        echo "  MAL   $desc → $obtenido (esperaba $esperado)"
        fallos=$((fallos + 1))
    fi
}

caso "Braswell (Pentium N3710) con iHD instalado" si braswell 0x8086 0x22b1 con-ihd
caso "Cherry Trail con iHD instalado" si cherry 0x8086 0x22b0 con-ihd
caso "Bay Trail con iHD instalado" si baytrail 0x8086 0x0f31 con-ihd
caso "Haswell con iHD instalado" si haswell 0x8086 0x0416 con-ihd
caso "Sandy Bridge con iHD instalado" si sandy 0x8086 0x0116 con-ihd
caso "Pineview con iHD instalado" si pineview 0x8086 0xa011 con-ihd
caso "Broadwell con iHD instalado" no broadwell 0x8086 0x1616 con-ihd
caso "Skylake con iHD instalado" no skylake 0x8086 0x1916 con-ihd
caso "Gemini Lake con iHD instalado" no geminilake 0x8086 0x3185 con-ihd
caso "Alder Lake con iHD instalado" no alder 0x8086 0x46a6 con-ihd
caso "Raptor Lake con iHD instalado" no raptor 0x8086 0xa7a0 con-ihd
caso "Skylake sin iHD (ISO 1.6.3)" si skylake-sin 0x8086 0x1916
caso "AMD" no amd 0x1002 0x73bf
caso "Intel sin archivo de device" si sin-device 0x8086 "" con-ihd

vacio="$T/vacio"; mkdir -p "$vacio/drm" "$vacio/dri"
if [ "$(intel_sin_ihd "$vacio/drm" "$vacio/dri")" = no ]; then
    echo "  bien  sin GPU de render → no"
else
    echo "  MAL   sin GPU de render"; fallos=$((fallos + 1))
fi

[ "$fallos" -eq 0 ] && echo "intel-ihd: todo bien" || { echo "intel-ihd: $fallos fallos"; exit 1; }
