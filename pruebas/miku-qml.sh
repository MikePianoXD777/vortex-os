#!/usr/bin/env bash
# Prueba del interruptor «Miku en 3D» (shell/src/quickshell/guide/general/GeneralAssistantSection.qml)
# con el quickshell de verdad, sin pantalla (QT_QPA_PLATFORM=offscreen) y sin tocar tu escritorio:
# HOME, XDG_* y XDG_RUNTIME_DIR temporales (así no se mete en el IPC de tu shell), un vortex-miku
# falso que cumple el contrato del comando (estado, avatar on|off que guarda "avatar" en
# ~/.config/vortex/miku.json y tarda 1 s) y los componentes del shell (Toggle, SettingsRow,
# ThemeBackend, I18n) reemplazados por versiones mínimas; el archivo QML que se prueba es el real,
# copiado tal cual. Comprueba: de fábrica está encendido (Miku en 3D); apagar y encender cambian
# el archivo y el interruptor; mientras corre queda quieto («Cambiando…») y un segundo clic no hace
# nada; con "avatar": false guardado arranca apagado; cerrar Ajustes a mitad no mata al comando;
# y, si falta vortex-miku o no contesta bien, lo dice en la descripción y avisa con una
# notificación en vez de callar.
#
#   pruebas/miku-qml.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECCION="$AQUI/shell/src/quickshell/guide/general/GeneralAssistantSection.qml"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

if ! command -v quickshell >/dev/null; then
  echo "  --  quickshell no está instalado: se salta"
  exit 0
fi
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-miku-qml.XXXXXX")"
trap 'rm -rf "$T"' EXIT

echo "== Armando el shell de prueba"
mkdir -p "$T/stub/guide/general" "$T/stub/reusables/guide" "$T/bin" "$T/bin-roto" "$T/bin-notif"
cp "$SECCION" "$T/stub/guide/general/GeneralAssistantSection.qml"
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
import "guide/general"

ShellRoot {
    id: root
    property string scen: Quickshell.env("SCEN")
    property real t0: Date.now()
    QtObject {
        id: fakeTab
        property var rootObj: ({ s: function(v) { return v; } })
        property bool visible: true
    }
    Loader { id: ld; active: true; sourceComponent: Component { GeneralAssistantSection { tab: fakeTab } } }
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
        let row = find(sec, function(i) { return i.settingId === "miku_avatar"; });
        console.log("T " + tag + " avatarOn=" + sec.avatarOn + " available=" + sec.available + " busy=" + sec.busy
            + " checked=" + tg.checked + " enabled=" + tg.enabled + " desc=" + row.description);
    }
    function clic(tag) {
        let tg = find(ld.item, function(i) { return i.isStubToggle === true; });
        console.log("T " + tag + " clic=" + tg.click());
    }
    Timer { interval: 100; running: true; repeat: true; onTriggered: root.snap("tick") }
    Timer { interval: 700; running: true; onTriggered: root.clic("primero") }
    // Un doble clic de verdad (150 ms después): el interruptor está quieto y no hace nada.
    Timer { interval: 850; running: true; onTriggered: root.clic("segundo") }
    // Normal: se vuelve a encender ya terminado. Destruir: se cierra Ajustes con el comando corriendo.
    Timer { interval: 2800; running: root.scen === "normal"; onTriggered: root.clic("encender") }
    Timer { interval: 1100; running: root.scen === "destruir"; onTriggered: { console.log("T cierro Ajustes"); ld.active = false; } }
    Timer { interval: 4900; running: true; onTriggered: { root.snap("fin"); Qt.quit(); } }
}
EOF
# vortex-miku falso: el contrato del comando (estado: dos líneas; avatar on|off guarda "avatar" en
# miku.json y tarda 1 s, como si cerrara y reabriera a Miku).
cat >"$T/bin/vortex-miku" <<'EOF'
#!/usr/bin/env bash
cfg="$XDG_CONFIG_HOME/vortex/miku.json"
leer() { if [[ -f "$cfg" ]] && grep -q '"avatar": *false' "$cfg"; then echo off; else echo on; fi; }
case "$1" in
  estado) echo "avatar=$(leer)"; echo "abierta=no" ;;
  avatar)
    echo "arranca: $*" >>"$QS_LOG"
    sleep 1
    mkdir -p "$(dirname "$cfg")"
    if [[ "$2" == on ]]; then echo '{"avatar": true}' >"$cfg"; else echo '{"avatar": false}' >"$cfg"; fi
    echo "termina: $*" >>"$QS_LOG"
    ;;
  *) exit 2 ;;
esac
EOF
# Uno que existe pero falla (y dice cosas raras): el interruptor no debe inventar un estado.
cat >"$T/bin-roto/vortex-miku" <<'EOF'
#!/usr/bin/env bash
echo "avatar=quizá"
exit 1
EOF
printf '#!/usr/bin/env bash\necho "notify-send: $*" >>"$QS_LOG"\n' >"$T/bin-notif/notify-send"
chmod +x "$T/bin/vortex-miku" "$T/bin-roto/vortex-miku" "$T/bin-notif/notify-send"

# corre ESCENARIO [json-inicial]; deja su salida en $T/ESCENARIO/{salida,cmd}.log
correr() {
  local esc="$1" r="$T/$1" ruta="$T/bin:$T/bin-notif:$PATH"
  [[ "$esc" == falta ]] && ruta="$T/bin-notif:$PATH"
  [[ "$esc" == roto ]] && ruta="$T/bin-roto:$T/bin-notif:$PATH"
  mkdir -p "$r/home" "$r/rt" "$r/cfg" "$r/cache" "$r/state"
  chmod 700 "$r/rt"
  if [[ -n "${2:-}" ]]; then
    mkdir -p "$r/cfg/vortex"
    printf '%s\n' "$2" >"$r/cfg/vortex/miku.json"
  fi
  env -i HOME="$r/home" PATH="$ruta" XDG_RUNTIME_DIR="$r/rt" XDG_CONFIG_HOME="$r/cfg" XDG_CACHE_HOME="$r/cache" \
    XDG_STATE_HOME="$r/state" QT_QPA_PLATFORM=offscreen SCEN="$esc" QS_LOG="$r/cmd.log" \
    timeout 30 quickshell -p "$T/stub/shell.qml" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep ' T ' >"$r/salida.log"
  touch "$r/cmd.log"
}

echo "== Apagar y encender con el interruptor"
correr normal
S="$T/normal/salida.log"
probar "al arrancar está encendido (Miku en 3D de fábrica), sin trabajar" \
  "grep -q 'T tick avatarOn=true available=true busy=false checked=true enabled=true desc=guide.general.assistant.desc' '$S'"
probar "mientras corre: ocupado, interruptor deshabilitado y «Cambiando…»" \
  "grep -q 'busy=true checked=false enabled=false desc=guide.general.assistant.working' '$S'"
probar "el segundo clic (doble clic) no hace nada" \
  "grep -q 'T segundo clic=false' '$S' && [[ \"\$(grep -c 'arranca: avatar off' '$T/normal/cmd.log')\" == 1 ]]"
probar "al terminar lee el estado: apagado y el interruptor sin marcar" \
  "grep -q 'avatarOn=false available=true busy=false checked=false enabled=true desc=guide.general.assistant.desc' '$S'"
probar "apagar llamó a «vortex-miku avatar off» y lo dejó guardado" \
  "grep -q 'arranca: avatar off' '$T/normal/cmd.log' && grep -q 'termina: avatar off' '$T/normal/cmd.log'"
probar "encender llama a «avatar on» y vuelve a encendido" \
  "grep -q 'arranca: avatar on' '$T/normal/cmd.log' && grep -q 'T fin avatarOn=true available=true busy=false checked=true enabled=true' '$S'"

echo "== Con \"avatar\": false guardado arranca apagado"
correr inicial-off '{"voz": "x", "avatar": false}'
probar "el interruptor arranca sin marcar" \
  "grep -q 'T tick avatarOn=false available=true busy=false checked=false enabled=true' '$T/inicial-off/salida.log'"

echo "== Cerrar Ajustes con el comando corriendo"
correr destruir
probar "se destruyó la sección" "grep -q 'T fin destruida' '$T/destruir/salida.log'"
probar "el comando termina igual y deja el archivo (no lo mata el cierre)" \
  "grep -q 'termina: avatar off' '$T/destruir/cmd.log' && grep -q '\"avatar\": false' '$T/destruir/cfg/vortex/miku.json'"

echo "== Falta vortex-miku (el paquete no está instalado)"
correr falta
probar "la descripción dice que falta el asistente" "grep -q 'T tick avatarOn=true available=false busy=false .* desc=guide.general.assistant.missing' '$T/falta/salida.log'"
probar "un clic avisa con una notificación y el interruptor vuelve a como estaba" \
  "grep -q 'notify-send: -a Vortex OS Miku Falta el asistente (paquete vortex-miku)' '$T/falta/cmd.log' && grep -q 'T fin avatarOn=true available=false busy=false checked=true enabled=true' '$T/falta/salida.log'"

echo "== vortex-miku existe pero estado falla"
correr roto
probar "no inventa un estado: queda encendido (de fábrica) y dice que no está disponible" \
  "grep -q 'T fin avatarOn=true available=false busy=false checked=true' '$T/roto/salida.log' && grep -q 'desc=guide.general.assistant.missing' '$T/roto/salida.log'"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS pruebas"
  exit 1
fi
echo "Todo bien"
