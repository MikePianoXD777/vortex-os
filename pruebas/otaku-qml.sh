#!/usr/bin/env bash
# Prueba del interruptor «Modo otaku» (shell/src/quickshell/guide/theme/ThemeOtakuSection.qml)
# con el quickshell de verdad, sin pantalla (QT_QPA_PLATFORM=offscreen) y sin tocar tu escritorio:
# HOME, XDG_* y XDG_RUNTIME_DIR temporales (así no se mete en el IPC de tu shell), un vortex-otaku
# falso que tarda 1 s y los componentes del shell (Toggle, SettingsRow, ThemeBackend, I18n)
# reemplazados por versiones mínimas; el archivo QML que se prueba es el real, copiado tal cual.
# Comprueba: el interruptor queda quieto («Trabajando…») mientras corre y un segundo clic no hace
# nada, al terminar refleja la marca, apagar funciona, cerrar Ajustes a mitad no mata al comando
# y, si falta vortex-otaku, avisa con una notificación en vez de callar.
#
#   pruebas/otaku-qml.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECCION="$AQUI/shell/src/quickshell/guide/theme/ThemeOtakuSection.qml"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

if ! command -v quickshell >/dev/null; then
  echo "  --  quickshell no está instalado: se salta"
  exit 0
fi
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-otaku-qml.XXXXXX")"
trap 'rm -rf "$T"' EXIT

echo "== Armando el shell de prueba"
mkdir -p "$T/stub/guide/theme" "$T/stub/reusables/guide" "$T/bin" "$T/bin-notif"
cp "$SECCION" "$T/stub/guide/theme/ThemeOtakuSection.qml"
printf 'singleton I18n 1.0 I18n.qml\nsingleton ThemeBackend 1.0 ThemeBackend.qml\n' >"$T/stub/qmldir"
printf 'pragma Singleton\nimport QtQuick\nQtObject { function t(k) { return k; } }\n' >"$T/stub/I18n.qml"
cat >"$T/stub/ThemeBackend.qml" <<'EOF'
pragma Singleton
import QtQuick
QtObject {
    property color mauve: "#cba6f7"
    property color surface1: "#45475a"
    property color crust: "#11111b"
    property color text: "#cdd6f4"
}
EOF
printf 'Toggle 1.0 Toggle.qml\n' >"$T/stub/reusables/qmldir"
# Igual que reusables/inputs/Toggle.qml: el clic voltea «checked» (rompe el enlace) y emite toggled.
cat >"$T/stub/reusables/Toggle.qml" <<'EOF'
import QtQuick
Item {
    id: root
    readonly property bool isStubToggle: true
    property bool checked: false
    property color accentColor
    property color baseColor
    property color handleColor
    property color handleOffColor
    signal toggled(bool checked)
    function click() {
        if (!root.enabled) return false;
        root.checked = !root.checked;
        root.toggled(root.checked);
        return true;
    }
}
EOF
printf 'SettingsBlob 1.0 SettingsBlob.qml\nSettingsRow 1.0 SettingsRow.qml\n' >"$T/stub/reusables/guide/qmldir"
printf 'import QtQuick\nItem { default property alias content: col.data; Item { id: col } }\n' >"$T/stub/reusables/guide/SettingsBlob.qml"
cat >"$T/stub/reusables/guide/SettingsRow.qml" <<'EOF'
import QtQuick
Item {
    property var rootObj
    property string settingId
    property string icon
    property bool wrapText
    property string title
    property string description
    property string searchKeywords
    default property alias content: col.data
    Item { id: col }
}
EOF
cat >"$T/stub/shell.qml" <<'EOF'
import QtQuick
import Quickshell
import "guide/theme"

ShellRoot {
    id: root
    property string scen: Quickshell.env("SCEN")
    property real t0: Date.now()
    function ms() { return Math.round(Date.now() - t0); }
    QtObject {
        id: fakeTab
        property var rootObj: ({ s: function(v) { return v; } })
        property bool visible: true
    }
    Loader { id: ld; active: true; sourceComponent: Component { ThemeOtakuSection { tab: fakeTab } } }
    function find(item, test) {
        if (!item) return null;
        if (test(item)) return item;
        for (let i = 0; i < item.children.length; i++) {
            let r = find(item.children[i], test);
            if (r) return r;
        }
        return null;
    }
    function snap(tag) {
        let sec = ld.item;
        if (!sec) { console.log("T " + tag + " destruida"); return; }
        let tg = find(sec, function(i) { return i.isStubToggle === true; });
        let row = find(sec, function(i) { return i.settingId === "otaku_mode"; });
        console.log("T " + tag + " otakuOn=" + sec.otakuOn + " busy=" + sec.busy + " checked=" + tg.checked
            + " enabled=" + tg.enabled + " desc=" + row.description);
    }
    function clic(tag) {
        let tg = find(ld.item, function(i) { return i.isStubToggle === true; });
        console.log("T " + tag + " clic=" + tg.click());
    }
    Timer { interval: 100; running: true; repeat: true; onTriggered: root.snap("tick") }
    Timer { interval: 400; running: true; onTriggered: root.clic("primero") }
    // Un doble clic de verdad (150 ms después): el interruptor está quieto y no hace nada.
    Timer { interval: 550; running: true; onTriggered: root.clic("segundo") }
    // Normal: se apaga ya terminado. Destruir: se cierra Ajustes con el comando corriendo.
    Timer { interval: 2500; running: root.scen === "normal"; onTriggered: root.clic("apagar") }
    Timer { interval: 800; running: root.scen === "destruir"; onTriggered: { console.log("T cierro Ajustes"); ld.active = false; } }
    Timer { interval: 4600; running: true; onTriggered: { root.snap("fin"); Qt.quit(); } }
}
EOF
# vortex-otaku falso: tarda 1 s y deja o quita la marca, como el de verdad.
cat >"$T/bin/vortex-otaku" <<'EOF'
#!/usr/bin/env bash
echo "arranca: $*" >>"$QS_LOG"
sleep 1
case "$1" in
  on) mkdir -p "$XDG_CONFIG_HOME/vortex"; touch "$XDG_CONFIG_HOME/vortex/otaku" ;;
  off) rm -f "$XDG_CONFIG_HOME/vortex/otaku" ;;
esac
echo "termina: $*" >>"$QS_LOG"
EOF
printf '#!/usr/bin/env bash\necho "notify-send: $*" >>"$QS_LOG"\n' >"$T/bin-notif/notify-send"
chmod +x "$T/bin/vortex-otaku" "$T/bin-notif/notify-send"

# corre ESCENARIO; deja su salida en $T/ESCENARIO/{salida,cmd}.log
correr() {
  local esc="$1" r="$T/$1" ruta="$T/bin:$T/bin-notif:$PATH"
  [[ "$esc" == falta ]] && ruta="$T/bin-notif:$PATH"
  mkdir -p "$r/home" "$r/rt" "$r/cfg" "$r/cache" "$r/state"
  chmod 700 "$r/rt"
  env -i HOME="$r/home" PATH="$ruta" XDG_RUNTIME_DIR="$r/rt" XDG_CONFIG_HOME="$r/cfg" XDG_CACHE_HOME="$r/cache" \
    XDG_STATE_HOME="$r/state" QT_QPA_PLATFORM=offscreen SCEN="$esc" QS_LOG="$r/cmd.log" \
    timeout 30 quickshell -p "$T/stub/shell.qml" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep ' T ' >"$r/salida.log"
  touch "$r/cmd.log"
}

echo "== Encender y apagar con el interruptor"
correr normal
S="$T/normal/salida.log"
probar "al arrancar está apagado, sin trabajar" "grep -q 'T tick otakuOn=false busy=false checked=false enabled=true desc=guide.theme.otaku.desc' '$S'"
probar "mientras corre: ocupado, interruptor deshabilitado y «Trabajando…»" \
  "grep -q 'busy=true checked=false enabled=false desc=guide.theme.otaku.working' '$S'"
probar "el segundo clic (doble clic) no hace nada" "grep -q 'T segundo clic=false' '$S' && [[ \"\$(grep -c 'arranca: on' '$T/normal/cmd.log')\" == 1 ]]"
probar "al terminar lee la marca: encendido y el interruptor puesto" "grep -q 'busy=false checked=true enabled=true desc=guide.theme.otaku.desc' '$S' && grep -q 'otakuOn=true busy=false' '$S'"
probar "apagar llama a «off» y vuelve a apagado" "grep -q 'arranca: off' '$T/normal/cmd.log' && grep -q 'T fin otakuOn=false busy=false checked=false enabled=true' '$S'"

echo "== Cerrar Ajustes con el comando corriendo"
correr destruir
probar "se destruyó la sección" "grep -q 'T fin destruida' '$T/destruir/salida.log'"
probar "el comando termina igual y deja la marca (no lo mata el cierre)" \
  "grep -q 'termina: on' '$T/destruir/cmd.log' && [[ -f '$T/destruir/cfg/vortex/otaku' ]]"

echo "== Falta vortex-otaku (sistema actualizado sin reinstalar /usr/local/bin)"
correr falta
probar "avisa con una notificación y el interruptor vuelve a apagado" \
  "grep -q 'notify-send: -a Vortex OS Modo otaku Falta el comando vortex-otaku' '$T/falta/cmd.log' && grep -q 'T fin otakuOn=false busy=false checked=false enabled=true' '$T/falta/salida.log'"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS pruebas"
  exit 1
fi
echo "Todo bien"
