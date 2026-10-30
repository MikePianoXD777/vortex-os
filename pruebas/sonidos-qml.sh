#!/usr/bin/env bash
# Prueba del panel de sonidos (2.2, shell/src/quickshell/guide/notifications/NotificationsSoundSection.qml)
# con el quickshell de verdad, sin pantalla (QT_QPA_PLATFORM=offscreen), como pruebas/miku-qml.sh: HOME,
# XDG_* y QS_STATE_DIR temporales, los componentes del shell (Input, botones, SettingsRow, I18n, Sounds…)
# reemplazados por versiones mínimas y la pestaña de Notificaciones por una falsa que lista la carpeta.
# El archivo QML que se prueba es el real, y el «vortex sonidos» también (bin/vortex de este repo), contra
# un servidor web local: no baja nada de internet ni toca tus sonidos ni tu shell.
# Comprueba: la lista muestra solo tus sonidos (no los que trae el shell); pegar un enlace lo baja, lo
# elige, lo toca y lo agrega a la lista; mientras baja queda ocupado; un enlace que no es audio deja el
# error de vortex en la descripción, sin el «vortex:»; «Usar» lo elige y lo toca; la basura lo borra.
#
#   pruebas/sonidos-qml.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECCION="$AQUI/shell/src/quickshell/guide/notifications/NotificationsSoundSection.qml"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

if ! command -v quickshell >/dev/null || ! command -v ffmpeg >/dev/null; then
  echo "  --  falta quickshell o ffmpeg: se salta"
  exit 0
fi
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-sonidos-qml.XXXXXX")"
SERVIDOR=""
trap '[[ -n "$SERVIDOR" ]] && kill "$SERVIDOR" 2>/dev/null; rm -rf "$T"' EXIT

echo "== Armando el shell de prueba"
S="$T/stub"
mkdir -p "$S/guide/notifications" "$S/reusables/guide" "$T/bin" "$T/web" "$T/assets" "$T/estado/sounds/notifications"
cp "$SECCION" "$S/guide/notifications/"
ffmpeg -v error -f lavfi -i sine=frequency=880:duration=0.3 "$T/web/campana.wav"
cp "$T/web/campana.wav" "$T/estado/sounds/notifications/Viejo.wav"
cp "$T/web/campana.wav" "$T/assets/Del shell.wav"
echo "<html>no soy audio</html>" >"$T/web/pagina.html"

printf 'singleton I18n 1.0 I18n.qml\nsingleton ThemeBackend 1.0 ThemeBackend.qml\nsingleton Caching 1.0 Caching.qml\nsingleton Sounds 1.0 Sounds.qml\n' >"$S/qmldir"
printf 'pragma Singleton\nimport QtQuick\nQtObject { function t(k, f) { return f !== undefined ? f : k; } }\n' >"$S/I18n.qml"
printf 'pragma Singleton\nimport QtQuick\nimport Quickshell\nQtObject { property string stateDir: Quickshell.env("QS_STATE_DIR"); property string home: Quickshell.env("HOME") }\n' >"$S/Caching.qml"
printf 'pragma Singleton\nimport QtQuick\nQtObject { function play(p) { console.log("T suena " + p); } }\n' >"$S/Sounds.qml"
cat >"$S/ThemeBackend.qml" <<'EOF'
pragma Singleton
import QtQuick
QtObject {
    property color mauve: "#cba6f7"; property color surface0: "#313244"; property color surface1: "#45475a"
    property color surface2: "#585b70"; property color crust: "#11111b"; property color text: "#cdd6f4"
    property color subtext0: "#a6adc8"; property color overlay2: "#9399b2"; property color red: "#f38ba8"
    property real borderRadius: 8
}
EOF
printf 'Toggle 1.0 Toggle.qml\nIconButton 1.0 IconButton.qml\nClickButton 1.0 ClickButton.qml\nDropdown 1.0 Dropdown.qml\nInput 1.0 Input.qml\n' >"$S/reusables/qmldir"
printf 'import QtQuick\nItem { property bool checked; property color accentColor; property color baseColor; property color handleColor; property color handleOffColor; signal toggled(bool c) }\n' >"$S/reusables/Toggle.qml"
for b in IconButton ClickButton; do
  printf 'import QtQuick\nItem { readonly property string stubKind: "%s"; property real cornerRadius; property string buttonIcon; property string buttonText; property int iconFontSize; property int textFontSize; property color accentColor; property color textColor; property bool isHoveredOrHighlighted; signal clicked() }\n' "$b" >"$S/reusables/$b.qml"
done
printf 'import QtQuick\nItem { property var options; property int currentIndex; property color accentColor; property color baseColor; property color hoverColor; property color dropdownColor; property color borderColor; property color textColor; property color activeTextColor; property real cornerRadius; property int fontPixelSize; signal selected(int index, var value) }\n' >"$S/reusables/Dropdown.qml"
cat >"$S/reusables/Input.qml" <<'EOF'
import QtQuick
Item {
    readonly property string stubKind: "Input"
    property string text; property string placeholderText; property bool isBusy; property bool hasError
    property color baseColor; property color accentColor; property color textColor; property color subTextColor
    property color borderColor; property real cornerRadius; property int fontPixelSize
    signal textEdited(string newText)
    signal accepted(string finalText)
    function clear() { text = ""; }
}
EOF
printf 'SettingsBlob 1.0 SettingsBlob.qml\nSettingsRow 1.0 SettingsRow.qml\n' >"$S/reusables/guide/qmldir"
printf 'import QtQuick\nItem { default property alias content: col.data; Column { id: col } }\n' >"$S/reusables/guide/SettingsBlob.qml"
cat >"$S/reusables/guide/SettingsRow.qml" <<'EOF'
import QtQuick
Item {
    property var rootObj; property string icon; property string title; property string description
    property color descriptionColor; property bool wrapText; property bool searchable: true
    default property alias content: col.data
    Item { id: col }
}
EOF
# La pestaña falsa: lo que la sección usa de NotificationsTab, y reloadSounds lista las dos carpetas
# (la del shell y la tuya) como la de verdad.
cat >"$S/shell.qml" <<'EOF'
import QtQuick
import Quickshell
import Quickshell.Io
import "guide/notifications"

ShellRoot {
    id: root
    QtObject {
        id: fakeTab
        property var rootObj: ({ s: function(v) { return v; } })
        property bool soundEnabled: true
        property var soundList: []
        property var availableSounds: []
        property string selectedSound: ""
        property var soundPickerPopup: null
        function updateNotifSetting(k, v) { console.log("T ajuste " + k + "=" + v); }
        function reloadSounds() { lister.running = false; lister.running = true; }
    }
    Process {
        id: lister
        command: ["bash", "-c", "for f in \"$ASSETS\"/* \"$QS_STATE_DIR\"/sounds/notifications/*; do [ -f \"$f\" ] && echo \"$f\"; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                let l = [];
                for (let p of this.text.trim().split("\n")) if (p) l.push({ name: p.replace(/^.*\//, "").replace(/\.[^.]*$/, ""), path: p });
                fakeTab.soundList = l.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            }
        }
    }
    NotificationsSoundSection { id: sec; tab: fakeTab }

    function find(item, test, acc) {
        if (!item) return acc;
        if (test(item)) acc.push(item);
        for (let i = 0; i < item.children.length; i++) find(item.children[i], test, acc);
        return acc;
    }
    function rows() { return find(sec, i => i.searchable === false, []); }
    function input() { return find(sec, i => i.stubKind === "Input", [])[0]; }
    function snap(tag) {
        let linkRow = find(sec, i => i.icon === "󰌹", [])[0];
        console.log("T " + tag + " busy=" + sec.linkBusy + " error=" + sec.linkError + " filas=" + rows().map(r => r.title + (r.icon === "󰄬" ? "*" : "")).join(",")
            + " desc=" + linkRow.description + " sel=" + fakeTab.selectedSound.replace(/^.*\//, ""));
    }
    function pegar(url) { let i = input(); i.text = url; i.accepted(url); }
    function boton(titulo, clase, icono) {
        let r = rows().filter(r => r.title === titulo)[0];
        let b = find(r, i => i.stubKind === clase && (icono === "" || i.buttonIcon === icono) && i.visible, [])[0];
        if (b) b.clicked(); else console.log("T sin boton " + titulo + " " + clase);
    }
    Component.onCompleted: fakeTab.reloadSounds()
    property string web: Quickshell.env("WEB")
    Timer { interval: 400; running: true; onTriggered: { root.snap("inicio"); root.pegar(root.web + "/campana.wav"); root.snap("pegado"); console.log("T campo=" + root.input().text); } }
    Timer { interval: 2500; running: true; onTriggered: { root.snap("bajado"); root.pegar(root.web + "/pagina.html"); } }
    Timer { interval: 4500; running: true; onTriggered: { root.snap("error"); root.boton("Viejo", "ClickButton", ""); } }
    Timer { interval: 4800; running: true; onTriggered: { root.snap("usado"); root.boton("Viejo", "IconButton", "󰆴"); } }
    Timer { interval: 6000; running: true; onTriggered: { root.snap("fin"); Qt.quit(); } }
}
EOF
printf '#!/usr/bin/env bash\nexec bash "%s/bin/vortex" "$@"\n' "$AQUI" >"$T/bin/vortex"
chmod +x "$T/bin/vortex"

python3 -u -m http.server 0 --bind 127.0.0.1 --directory "$T/web" >"$T/servidor.log" 2>&1 &
SERVIDOR=$!
for _ in $(seq 50); do grep -q "port" "$T/servidor.log" && break; sleep 0.1; done
PUERTO="$(grep -oE 'port [0-9]+' "$T/servidor.log" | head -1 | cut -d' ' -f2)"
[[ -n "$PUERTO" ]] || { echo "No arrancó el servidor de prueba" >&2; exit 1; }

mkdir -p "$T/home" "$T/rt" "$T/cfg" "$T/cache"
chmod 700 "$T/rt"
env -i LANG=C.UTF-8 HOME="$T/home" PATH="$T/bin:$PATH" XDG_RUNTIME_DIR="$T/rt" XDG_CONFIG_HOME="$T/cfg" XDG_CACHE_HOME="$T/cache" \
  QS_STATE_DIR="$T/estado" ASSETS="$T/assets" WEB="http://127.0.0.1:$PUERTO" QT_QPA_PLATFORM=offscreen \
  timeout 30 quickshell -p "$S/shell.qml" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' >"$T/todo.log"
grep -oE ' T .*' "$T/todo.log" | sed 's/^ T //' >"$T/salida.log"
L="$T/salida.log"
D="$T/estado/sounds/notifications"

echo "== Panel de sonidos"
probar "la lista muestra solo tus sonidos, no los que trae el shell" "grep -q '^inicio busy=false error=false filas=Viejo desc=A myinstants.com' '$L'"
probar "al pegar el enlace queda ocupado («Downloading…») y el campo se vacía" \
  "grep -q '^pegado busy=true error=false .*desc=Downloading…' '$L' && grep -q '^campo=\$' '$L'"
probar "lo baja a tu carpeta" "[[ -f '$D/campana.wav' ]]"
probar "lo elige (ajuste soundFile) y lo toca" \
  "grep -q '^ajuste soundFile=$D/campana.wav' '$L' && grep -q '^suena $D/campana.wav' '$L'"
probar "aparece en la lista, marcado como el que suena, y la descripción dice «Added «campana»»" \
  "grep -q '^bajado busy=false error=false filas=campana\\*,Viejo desc=Added «campana» sel=campana.wav' '$L'"
probar "un enlace que no es audio: error con el mensaje de vortex, sin «vortex:» y con mayúscula" \
  "grep -q '^error busy=false error=true .*desc=Eso no es un sonido sel=campana.wav' '$L'"
probar "y no agrega nada" "grep -q '^error .*filas=campana\\*,Viejo desc' '$L'"
probar "«Usar» elige el otro y lo toca" \
  "grep -q '^usado .*filas=campana,Viejo\\* .*sel=Viejo.wav' '$L' && grep -q '^suena $D/Viejo.wav' '$L'"
probar "la basura lo borra de la carpeta y de la lista" \
  "[[ ! -e '$D/Viejo.wav' ]] && grep -q '^fin .*filas=campana desc' '$L'"
probar "el sonido del shell sigue en su lugar" "[[ -f '$T/assets/Del shell.wav' ]]"
probar "sin errores de QML" "! grep -qiE 'ReferenceError|TypeError|is not defined|Cannot assign|Type .* unavailable' '$T/todo.log'"

if ((FALLOS)); then
  echo "--- salida"; cat "$L"
  echo "--- errores"; grep -iE 'error|warn' "$T/todo.log" | head -20
fi
echo
((FALLOS == 0)) && echo "Todo bien" || { echo "FALLARON $FALLOS pruebas"; exit 1; }
