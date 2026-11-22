#!/usr/bin/env bash
# Prueba del interruptor «Compañera de escritorio» de Ajustes → General
# (shell/src/quickshell/guide/general/GeneralMascotaSection.qml), 4.1.
#
# Solo Qt (qmltestrunner, sin pantalla): no abre quickshell ni toca tu escritorio. El archivo QML es el
# real, copiado (solo su import de Quickshell.Io cambia); Quickshell.Io (Process, StdioCollector) y las piezas del shell (SettingsBlob,
# SettingsRow, Toggle, ThemeBackend, I18n) van de mentira. El Process falso anota cada comando y
# contesta lo que le diga la prueba. Comprueba: lee «vortex mascota estado» al abrir; «encendida …»
# deja el interruptor prendido y «apagada …» apagado; prender y apagar llaman a «vortex mascota on|off»
# y vuelven a leer el estado; sin vortex-mascota (no contesta) lo dice y el interruptor queda quieto.
#
#   pruebas/mascota-ajustes.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECCION="$AQUI/shell/src/quickshell/guide/general/GeneralMascotaSection.qml"
RUNNER="$(command -v qmltestrunner6 || command -v qmltestrunner || echo /usr/lib/qt6/bin/qmltestrunner)"
if [[ ! -x "$RUNNER" ]]; then
  echo "  --  falta qmltestrunner (qt6-declarative): se salta"
  exit 0
fi
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-mascota-ajustes.XXXXXX")"
trap 'rm -rf "$T"' EXIT

Q="$T/q"
mkdir -p "$Q/guide/general" "$Q/reusables/guide" "$T/imports/FalsoQs/Io"
# El Quickshell de verdad le ganaría al de mentira: en la copia, sus imports van a «FalsoQs».
sed -e '/^import Quickshell$/d' -e 's/^import Quickshell\.Io$/import FalsoQs.Io/' "$SECCION" >"$Q/guide/general/GeneralMascotaSection.qml"
grep -q '^import FalsoQs.Io$' "$Q/guide/general/GeneralMascotaSection.qml" || { echo "  MAL la sección ya no importa Quickshell.Io como antes"; exit 1; }

printf 'singleton I18n 1.0 I18n.qml\nsingleton ThemeBackend 1.0 ThemeBackend.qml\n' >"$Q/qmldir"
printf 'pragma Singleton\nimport QtQuick\nQtObject { function t(k, d) { return d || k; } }\n' >"$Q/I18n.qml"
printf 'pragma Singleton\nimport QtQuick\nQtObject { property color mauve: "#fff"; property color surface1: "#333"; property color crust: "#000"; property color text: "#ddd" }\n' >"$Q/ThemeBackend.qml"
printf 'Toggle 1.0 Toggle.qml\n' >"$Q/reusables/qmldir"
cat >"$Q/reusables/Toggle.qml" <<'EOF'
import QtQuick
Item {
    property bool checked: false
    property color accentColor
    property color baseColor
    property color handleColor
    property color handleOffColor
    signal toggled(bool on)
}
EOF
printf 'SettingsBlob 1.0 SettingsBlob.qml\nSettingsRow 1.0 SettingsRow.qml\n' >"$Q/reusables/guide/qmldir"
printf 'import QtQuick\nItem {}\n' >"$Q/reusables/guide/SettingsBlob.qml"
cat >"$Q/reusables/guide/SettingsRow.qml" <<'EOF'
import QtQuick
Item {
    property var rootObj
    property string settingId
    property string icon
    property bool wrapText
    property string title
    property string description
    property string searchKeywords
}
EOF

# Quickshell.Io de mentira (como FalsoQs.Io). Registro (singleton) guarda los comandos y la respuesta del estado.
printf 'module FalsoQs.Io\nProcess 1.0 Process.qml\nStdioCollector 1.0 StdioCollector.qml\nsingleton Registro 1.0 Registro.qml\n' >"$T/imports/FalsoQs/Io/qmldir"
cat >"$T/imports/FalsoQs/Io/Registro.qml" <<'EOF'
pragma Singleton
import QtQuick
QtObject {
    property var comandos: []
    property string respuesta: ""
}
EOF
printf 'import QtQuick\nQtObject { property string text: ""; signal streamFinished() }\n' >"$T/imports/FalsoQs/Io/StdioCollector.qml"
cat >"$T/imports/FalsoQs/Io/Process.qml" <<'EOF'
import QtQuick
QtObject {
    id: p
    property var command: []
    property bool running: false
    property var stdout: null
    onRunningChanged: {
        if (!running) return;
        Registro.comandos = Registro.comandos.concat([p.command.join(" ")]);
        Qt.callLater(function() {
            if (p.stdout) {
                p.stdout.text = Registro.respuesta;
                p.stdout.streamFinished();
            }
            p.running = false;
        });
    }
}
EOF

cat >"$Q/guide/general/tst_mascota.qml" <<'EOF'
import QtQuick
import QtTest
import FalsoQs.Io

Item {
    id: raiz
    width: 400; height: 200
    QtObject { id: pestana; property var rootObj: null; property bool visible: true }

    Component { id: hacer; GeneralMascotaSection { tab: pestana } }

    function interruptor(s) {
        // El Toggle es el único hijo del SettingsRow.
        const fila = s.children[0];
        for (let i = 0; i < fila.children.length; i++) if (fila.children[i].toggled !== undefined) return fila.children[i];
        return null;
    }

    TestCase {
        name: "mascota"
        function init() { Registro.comandos = []; }

        function test_lee_el_estado_al_abrir() {
            Registro.respuesta = "encendida (PID 42), aspecto: mascota\n";
            const s = createTemporaryObject(hacer, raiz);
            tryCompare(s, "mascotaOn", true);
            verify(s.available);
            compare(Registro.comandos[0], "bash -c vortex mascota estado 2>/dev/null");
            verify(interruptor(s).checked, "el interruptor queda prendido");
            Registro.respuesta = "apagada, aspecto: original\n";
            const t = createTemporaryObject(hacer, raiz);
            wait(50);
            verify(!t.mascotaOn && t.available && !interruptor(t).checked, "apagada: interruptor apagado");
        }

        function test_prender_y_apagar() {
            Registro.respuesta = "apagada, aspecto: original\n";
            const s = createTemporaryObject(hacer, raiz);
            wait(50);
            Registro.comandos = [];
            Registro.respuesta = "encendida (PID 7), aspecto: original\n";
            interruptor(s).toggled(true);
            tryCompare(s, "mascotaOn", true);
            compare(Registro.comandos[0], "vortex mascota on");
            compare(Registro.comandos[1], "bash -c vortex mascota estado 2>/dev/null", "vuelve a leer el estado");
            Registro.comandos = [];
            Registro.respuesta = "apagada, aspecto: original\n";
            interruptor(s).toggled(false);
            tryCompare(s, "mascotaOn", false);
            compare(Registro.comandos[0], "vortex mascota off");
        }

        function test_sin_vortex_mascota() {
            Registro.respuesta = "";
            const s = createTemporaryObject(hacer, raiz);
            tryCompare(s, "available", false);
            verify(!interruptor(s).enabled, "el interruptor queda quieto");
            verify(s.children[0].description.indexOf("vortex actualizar") >= 0, "dice qué hacer");
        }
    }
}
EOF

salida="$(cd "$Q/guide/general" && env -u WAYLAND_DISPLAY -u DISPLAY QT_QPA_PLATFORM=offscreen \
  timeout 60 "$RUNNER" -import "$T/imports" -input tst_mascota.qml 2>&1)"
rc=$?
printf '%s\n' "$salida" | grep -E '^(PASS|FAIL|   Actual|   Expected|   Loc|QWARN|Totals|    /)' | grep -v 'initTestCase\|cleanupTestCase'
if ((rc == 0)); then echo "Todo bien"; else echo "FALLARON pruebas (rc=$rc)"; exit 1; fi
