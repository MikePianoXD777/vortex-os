#!/usr/bin/env bash
# Prueba de «Tus tareas en la barra» (2.5): bin/vortex-tareas con una bóveda de Obsidian temporal y la
# hora fija (VORTEX_TAREAS_AHORA), y el panel de verdad (shell/src/quickshell/tareas/TareasPopup.qml con
# el singleton Tareas.qml) en quickshell sin pantalla, con el resto del shell reemplazado por versiones
# mínimas. HOME, XDG_* y la bóveda son temporales: no toca tu Obsidian, ni avisa en tu escritorio
# (notify-send, xdg-open y qs_manager.sh son falsos y anotan lo que les piden).
#
#   pruebas/tareas.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-tareas.XXXXXX")"
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/home" "$T/cfg" "$T/estado" "$T/bin" "$T/serp/scripts" "$T/b/Escuela" "$T/b/.obsidian"
export HOME="$T/home" XDG_CONFIG_HOME="$T/cfg" XDG_STATE_HOME="$T/estado" SERPANTINUM_DIR="$T/serp"
export PATH="$T/bin:$PATH"
# notify-send falso: anota el título y contesta lo que diga RESPUESTA (el botón elegido)
cat >"$T/bin/notify-send" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${@: -2:1}" >>"$AVISOS"
printf '%s\n' "${RESPUESTA:-}"
EOF
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$1" >>"%s/abiertos"\n' "$T" >"$T/bin/xdg-open"
printf 'printf "%%s\\n" "$*" >>"%s/paneles"\n' "$T" >"$T/serp/scripts/qs_manager.sh"
ln -s "$AQUI/bin/vortex-tareas" "$T/bin/vortex-tareas"
chmod +x "$T/bin/"*
export AVISOS="$T/avisos"
: >"$AVISOS"
VT="$AQUI/bin/vortex-tareas"

# La bóveda: el lunes 2/11/2026 a las 10 am
cat >"$T/b/Escuela/Mate.md" <<'EOF'
# Mate
- [ ] Examen de fracciones 📅 2026-11-02
- [ ] Maqueta de ciencias (3/11)
- [ ] Problemas pág. 40 para el 6/11
- [ ] Tarea vieja [due:: 2026-10-30]
- [ ] Sin fecha en otra nota
- [x] Ya hecha 📅 2026-11-02
- [ ] Receta: 1/2 taza de azúcar
EOF
printf -- '---\ntags:\n  - tareas\n---\n- [ ] Comprar cartulina\n' >"$T/b/Tareas.md"
mkdir -p "$T/b/.trash"
printf -- '- [ ] En la basura 📅 2026-11-02\n' >"$T/b/.trash/Vieja.md"
export VORTEX_TAREAS_BOVEDA="$T/b" VORTEX_TAREAS_AHORA="2026-11-02T10:00"
lista() { "$VT" listar | python3 -c 'import json,sys; d=json.load(sys.stdin); print("\n".join(f"{t["texto"]}|{t["fecha"]}|{t["dias"]}|{t["nota"]}" for t in d["tareas"]))'; }

echo "== Leer las tareas"
L="$(lista)"
probar "las de las notas con fecha, por fecha, y todas las de Tareas.md al final" \
  "[[ \"\$L\" == \"Tarea vieja|2026-10-30|-3|Mate
Examen de fracciones|2026-11-02|0|Mate
Maqueta de ciencias|2026-11-03|1|Mate
Problemas pág. 40|2026-11-06|4|Mate
Comprar cartulina||None|Tareas\" ]]"
probar "sin las hechas, las sin fecha de otras notas, «1/2 taza» ni la basura" \
  "! grep -qE 'Ya hecha|Sin fecha|taza|basura' <<<\"\$L\""
probar "dice dónde están la bóveda y Tareas.md" \
  "\"$VT\" listar | grep -q '\"archivo\": \"$T/b/Tareas.md\"'"

echo "== Agregar y marcar hecha"
"$VT" agregar "  Leer   el cap. 3 " 2026-11-04
probar "la nueva va a Tareas.md con su 📅" "tail -1 '$T/b/Tareas.md' | grep -qx -- '- \[ \] Leer el cap. 3 📅 2026-11-04'"
probar "una fecha que no existe no se agrega" "! \"$VT\" agregar 'Mal' 2026-13-40 2>/dev/null && ! grep -q Mal '$T/b/Tareas.md'"
"$VT" hecha "$T/b/Escuela/Mate.md" 2 "- [ ] Examen de fracciones 📅 2026-11-02"
probar "hecha: queda [x] con ✅ y la fecha, en su nota" \
  "sed -n 2p '$T/b/Escuela/Mate.md' | grep -qx -- '- \[x\] Examen de fracciones 📅 2026-11-02 ✅ 2026-11-02'"
antes="$(md5sum <"$T/b/Escuela/Mate.md")"
probar "si la línea cambió, no toca nada" \
  "! \"$VT\" hecha '$T/b/Escuela/Mate.md' 3 '- [ ] Otra cosa' 2>/dev/null && [[ \"\$(md5sum <'$T/b/Escuela/Mate.md')\" == \"$antes\" ]]"

echo "== Avisos"
"$VT" avisar
probar "a las 10 am avisa de las de hoy y vencidas, no de la de mañana" \
  "grep -qx 'Para hoy: Tarea vieja' '$AVISOS' && ! grep -q Maqueta '$AVISOS'"
n="$(wc -l <"$AVISOS")"
"$VT" avisar
probar "no repite el aviso" "[[ \$(wc -l <'$AVISOS') == $n ]]"
VORTEX_TAREAS_AHORA="2026-11-02T18:00" RESPUESTA=ver "$VT" avisar
sleep 0.3
probar "a las 6 pm avisa de la de mañana" "tail -1 '$AVISOS' | grep -qx 'Para mañana: Maqueta de ciencias'"
probar "«Ver mis tareas» abre el panel" "grep -qx 'toggle tareas' '$T/paneles'"
"$VT" agregar "Otra para hoy" 2026-11-02
mkdir -p "$T/estado/vortex" && touch "$T/estado/vortex/modo-juego"
n="$(wc -l <"$AVISOS")"
"$VT" avisar
probar "en modo juego no avisa" "[[ \$(wc -l <'$AVISOS') == $n ]]"
rm "$T/estado/vortex/modo-juego"

echo "== La bóveda de Obsidian y abrir"
unset VORTEX_TAREAS_BOVEDA
mkdir -p "$T/otra" "$T/cfg/obsidian"
printf '{"vaults":{"a":{"path":"%s","ts":1},"b":{"path":"%s","ts":2,"open":true},"c":{"path":"/no/existe","ts":3}}}\n' "$T/b" "$T/otra" \
  >"$T/cfg/obsidian/obsidian.json"
probar "usa la bóveda que Obsidian tiene abierta" "\"$VT\" listar | grep -q '\"boveda\": \"$T/otra\"'"
"$VT" abrir
sleep 0.3
probar "abrir crea Tareas.md (con tags) si falta" "grep -qx '  - tareas' '$T/otra/Tareas.md'"
probar "y la abre en Obsidian" "grep -qx 'obsidian://open?path=%2F.*%2Fotra%2FTareas.md' '$T/abiertos'"
rm -rf "$T/cfg/obsidian"
probar "sin Obsidian, usa Documentos/Tareas.md" "\"$VT\" listar | grep -q '\"archivo\": \"$T/home/Documentos/Tareas.md\"'"

echo "== El panel (quickshell)"
if ! command -v quickshell >/dev/null; then
  echo "  --  sin quickshell: se salta el panel"
else
  # Con las fechas de verdad: el panel calcula «Mañana» con la fecha de hoy
  unset VORTEX_TAREAS_AHORA
  hoy="$(date +%F)"; manana="$(date -d tomorrow +%F)"; en5="$(date -d '+5 days' +%F)"
  mkdir -p "$T/p/Escuela"
  printf -- '- [ ] Examen 📅 %s\n- [ ] Exposición 📅 %s\n- [ ] Ensayo 📅 %s\n' "$hoy" "$manana" "$en5" >"$T/p/Escuela/Mate.md"
  printf -- '- [ ] Comprar cartulina\n' >"$T/p/Tareas.md"
  S="$T/stub"
  mkdir -p "$S/tareas" "$S/singletons/system" "$S/reusables"
  cp "$AQUI/shell/src/quickshell/tareas/TareasPopup.qml" "$S/tareas/"
  cp "$AQUI/shell/src/quickshell/singletons/system/Tareas.qml" "$S/singletons/system/"
  printf 'singleton I18n 1.0 I18n.qml\nsingleton ThemeBackend 1.0 ThemeBackend.qml\nsingleton Scaler 1.0 Scaler.qml\nsingleton Sounds 1.0 Sounds.qml\nsingleton Tareas 1.0 singletons/system/Tareas.qml\n' >"$S/qmldir"
  printf 'pragma Singleton\nimport QtQuick\nQtObject { property string currentLang: "es"; function t(k, f) { return f !== undefined ? f : k; } }\n' >"$S/I18n.qml"
  printf 'pragma Singleton\nimport QtQuick\nQtObject { function s(v) { return v; } }\n' >"$S/Scaler.qml"
  printf 'pragma Singleton\nimport QtQuick\nQtObject { function playSfx(p, v) { console.log("T suena " + p); } }\n' >"$S/Sounds.qml"
  cat >"$S/ThemeBackend.qml" <<'EOF'
pragma Singleton
import QtQuick
QtObject {
    property color mauve: "#cba6f7"; property color green: "#a6e3a1"; property color red: "#f38ba8"; property color peach: "#fab387"
    property color surface0: "#313244"; property color surface1: "#45475a"; property color crust: "#11111b"; property color base: "#1e1e2e"
    property color text: "#cdd6f4"; property color subtext0: "#a6adc8"; property color overlay1: "#7f849c"
    property real borderRadius: 16; property real elementOpacity: 0.9; property string iconFont: "Symbols Nerd Font"; property string fontFamily: "sans"
}
EOF
  printf 'WallpaperBlur 1.0 WallpaperBlur.qml\n' >"$S/reusables/qmldir"
  printf 'import QtQuick\nItem { property real topLeftRadius; property real topRightRadius; property real bottomLeftRadius; property real bottomRightRadius }\n' >"$S/reusables/WallpaperBlur.qml"
  cat >"$S/shell.qml" <<'EOF'
import QtQuick
import Quickshell
import "tareas"
ShellRoot {
    id: root
    FloatingWindow {
        implicitWidth: 540; implicitHeight: 680; visible: true
        TareasPopup { id: pop; anchors.fill: parent; visible: true }
    }
    function find(item, name) {
        if (!item) return null;
        if (item.objectName === name) return item;
        for (let i = 0; i < item.children.length; i++) { let r = find(item.children[i], name); if (r) return r; }
        return null;
    }
    function snap(tag) {
        let l = find(pop, "lista");
        console.log("T " + tag + " barra=" + JSON.stringify(Tareas.barra.map(b => b.time + "/" + b.tareas))
            + " filas=" + l.count + " tareas=" + Tareas.tareas.map(t => t.texto + "@" + Tareas.cuando(t)).join(","));
    }
    Timer { interval: 1500; running: true; onTriggered: {
        root.snap("inicio");
        let n = find(pop, "nueva");
        console.log("T foco=" + n.activeFocus);
        pop.dentroDe = 1; n.text = "Llevar el compás"; n.accepted();
        console.log("T campo=" + n.text);
    } }
    Timer { interval: 3500; running: true; onTriggered: {
        root.snap("agregada");
        // CAPTURA=archivo.png guarda cómo se ve el panel (para revisarlo a ojo)
        if (Quickshell.env("CAPTURA")) pop.grabToImage(r => r.saveToFile(Quickshell.env("CAPTURA")));
        let l = find(pop, "lista");
        l.currentIndex = 0;
        find(l.currentItem, "hecha").clicked(null);
    } }
    Timer { interval: 5500; running: true; onTriggered: { root.snap("hecha"); Qt.quit(); } }
}
EOF
  mkdir -p "$T/rt" "$T/cache"
  chmod 700 "$T/rt"
  env -i LANG=C.UTF-8 HOME="$T/home" PATH="$T/bin:$PATH" XDG_RUNTIME_DIR="$T/rt" XDG_CONFIG_HOME="$T/cfg" \
    XDG_CACHE_HOME="$T/cache" XDG_STATE_HOME="$T/estado" VORTEX_TAREAS_BOVEDA="$T/p" AVISOS="$AVISOS" CAPTURA="${CAPTURA:-}" \
    QT_QPA_PLATFORM=offscreen timeout 30 quickshell -p "$S/shell.qml" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' >"$T/todo.log"
  grep -oE ' T .*' "$T/todo.log" | sed 's/^ T //' >"$T/salida.log"
  L="$T/salida.log"
  probar "la barra cuenta las de hoy y mañana (2), con clic al panel" "grep -q '^inicio barra=\[\"2 tasks/true\"\]' '$L'"
  probar "el panel las lista con Hoy / Mañana / la fecha, y la de Tareas.md sin fecha" \
    "grep -qE '^inicio .*filas=4 tareas=Examen@Today,Exposición@Tomorrow,Ensayo@[^,]+,Comprar cartulina@\$' '$L'"
  probar "el campo de la nueva tarea tiene el foco al abrir" "grep -qx 'foco=true' '$L'"
  probar "Enter la agrega para mañana y vacía el campo" \
    "grep -qx 'campo=' '$L' && grep -qx -- '- \[ \] Llevar el compás 📅 $manana' '$T/p/Tareas.md'"
  probar "y aparece en la lista y en la cuenta de la barra (3)" \
    "grep -q '^agregada barra=\[\"3 tasks/true\"\] filas=5 .*Llevar el compás@Tomorrow' '$L'"
  probar "el círculo la marca hecha en su nota, suena y sale de la lista" \
    "grep -qx -- '- \[x\] Examen 📅 $hoy ✅ $hoy' '$T/p/Escuela/Mate.md' && grep -q '^suena ' '$L' && grep -q '^hecha barra=\[\"2 tasks/true\"\] filas=4 tareas=Exposición' '$L'"
  probar "sin errores de QML" "! grep -qiE 'ReferenceError|TypeError|is not defined|Cannot assign|Type .* unavailable|is not a type' '$T/todo.log'"
  if ((FALLOS)); then
    echo "--- salida"; cat "$L"
    echo "--- errores"; grep -iE 'error|warn' "$T/todo.log" | head -20
  fi
fi

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "FALLARON $FALLOS pruebas"; exit 1; }
