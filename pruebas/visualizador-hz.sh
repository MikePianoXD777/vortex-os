#!/usr/bin/env bash
# Prueba del visualizador de música a la frecuencia del monitor (4.3,
# shell/src/quickshell/reusables/media/Visualizer.qml) con qml6 sin pantalla (QT_QPA_PLATFORM=offscreen):
# no abre quickshell ni ventanas. El Visualizer.qml es el real; Config y ThemeBackend son versiones mínimas.
# Comprueba: el paso dura un cuadro del monitor (200 Hz → 5 ms, 144 → 7, 60 → 16), nunca baja de 4 ms ni
# pasa de 16, el perfil «bajo» se queda en 16 ms, y el Timer usa ese intervalo.
#
#   pruebas/visualizador-hz.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VIS="$AQUI/shell/src/quickshell/reusables/media/Visualizer.qml"
QML="$(command -v qml6 || echo /usr/lib/qt6/bin/qml)"
if [[ ! -x "$QML" ]]; then
  echo "  --  falta qml6: se salta"
  exit 0
fi
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-visualizador.XXXXXX")"
trap '[[ -n "${GUARDAR:-}" ]] || rm -rf "$T"' EXIT

mkdir -p "$T/reusables/media"
cp "$VIS" "$T/reusables/media/"
printf 'singleton Config 1.0 Config.qml\nsingleton ThemeBackend 1.0 ThemeBackend.qml\nsingleton Spectrum 1.0 Spectrum.qml\nsingleton Caching 1.0 Caching.qml\n' >"$T/qmldir"
cat >"$T/Config.qml" <<'EOF'
pragma Singleton
import QtQuick
QtObject {
    property string rendimiento: "ultra"
    function getSetting(k, d) { return k === "rendimiento" ? rendimiento : d; }
}
EOF
printf 'pragma Singleton\nimport QtQuick\nQtObject { property color mauve: "white" }\n' >"$T/ThemeBackend.qml"
printf 'pragma Singleton\nimport QtQuick\nQtObject { property int barCount: 32; property var barLevels: [] }\n' >"$T/Spectrum.qml"
printf 'pragma Singleton\nimport QtQuick\nQtObject { property string serpantinumDir: "/nonexistent" }\n' >"$T/Caching.qml"
cat >"$T/prueba.qml" <<'EOF'
import QtQuick
import "reusables/media"
Item {
    Visualizer { id: v; width: 100; height: 40 }
    Component.onCompleted: {
        let casos = [[200, false, 5], [144, false, 7], [165, false, 6], [60, false, 16], [30, false, 16],
                     [500, false, 4], [0, false, 16], [NaN, false, 16], [200, true, 16], [144, true, 16]];
        let fallos = 0;
        for (let c of casos) {
            let r = v.intervaloPaso(c[0], c[1]);
            let ok = r === c[2];
            if (!ok) fallos++;
            console.log((ok ? "  ok  " : "  MAL ") + c[0] + " Hz" + (c[1] ? " (bajo)" : "") + " → " + r + " ms (esperado " + c[2] + ")");
        }
        let esperado = v.intervaloPaso(v.refrescoMonitor, v.perfilBajo);
        let timer = null;
        for (let i = 0; i < v.data.length; i++)
            if (v.data[i] instanceof Timer && v.data[i].interval === esperado && v.data[i].repeat) timer = v.data[i];
        if (timer === null) fallos++;
        console.log((timer !== null ? "  ok  " : "  MAL ") + "el Timer de las barras va a " + esperado + " ms (pantalla de " + v.refrescoMonitor + " Hz)");
        console.log("FALLOS=" + fallos);
        Qt.exit(fallos === 0 ? 0 : 1);
    }
}
EOF

echo "== Visualizador a la frecuencia del monitor"
SALIDA="$(cd "$T" && env -u WAYLAND_DISPLAY -u DISPLAY QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen timeout 30 "$QML" prueba.qml 2>&1)"
CODIGO=$?
printf '%s\n' "$SALIDA" | sed -n 's/^qml: \(  ok  \|  MAL \)/\1/p'
if [[ $CODIGO -ne 0 ]] || ! grep -q 'FALLOS=0' <<<"$SALIDA"; then
  printf '%s\n' "$SALIDA" | grep -v '^qml: ' | head -5
  echo "FALLÓ (código $CODIGO)"
  exit 1
fi
echo "Todo bien."
