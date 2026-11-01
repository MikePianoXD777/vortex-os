pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../../"

// Vortex 2.5: tus tareas en la barra. Lee las tareas con `vortex-tareas listar` (las que tienen fecha en tu
// bóveda de Obsidian y todas las de Tareas.md); la barra cuenta las de hoy, mañana y las vencidas, y con
// clic abre el panel tareas/TareasPopup.qml. Cada 10 min corre `vortex-tareas avisar`, que avisa una vez
// de las de hoy y (desde las 5 pm) de las de mañana. Shell.qml lo despierta al arrancar.
QtObject {
    id: root

    property var tareas: [] // [{texto, fecha, dias, archivo, nota, linea, original}], por fecha
    property string boveda: ""
    property string archivo: "" // Tareas.md, donde caen las que agregas
    property bool cargado: false
    property bool pendiente: false // pidieron leer mientras ya se leía

    readonly property var proximas: tareas.filter(t => t.dias !== null && t.dias <= 1)

    function t(clave, porDefecto) {
        return I18n.t("tareas." + clave, porDefecto);
    }

    function iniciar() {
        // Solo con nombrarlo se crea el singleton y se leen las tareas
        return cargado;
    }

    function refrescar() {
        if (lector.running) { pendiente = true; return; }
        lector.running = true;
    }

    function correr(args) {
        let p = accion.createObject(root);
        p.command = ["vortex-tareas"].concat(args);
        p.running = true;
    }

    function agregar(texto, fecha) {
        texto = texto.trim();
        if (texto === "") return;
        correr(["agregar", texto, fecha || ""]);
    }

    function hecha(tarea) {
        tareas = tareas.filter(x => !(x.archivo === tarea.archivo && x.linea === tarea.linea));
        correr(["hecha", tarea.archivo, String(tarea.linea), tarea.original]);
    }

    // AAAA-MM-DD de dentro de n días
    function fechaEn(n) {
        let d = new Date();
        d.setDate(d.getDate() + n);
        return Qt.formatDate(d, "yyyy-MM-dd");
    }

    function cuando(tarea) {
        if (tarea.dias === null) return "";
        if (tarea.dias < 0) return tarea.dias === -1 ? t("yesterday", "Yesterday") : t("overdue", "Overdue");
        if (tarea.dias === 0) return t("today", "Today");
        if (tarea.dias === 1) return t("tomorrow", "Tomorrow");
        let d = new Date(tarea.fecha + "T12:00:00");
        if (tarea.dias < 7) {
            let n = Qt.locale(I18n.currentLang || "es").dayName(d.getDay(), Locale.LongFormat);
            return n.charAt(0).toUpperCase() + n.slice(1);
        }
        return Qt.formatDate(d, "d/M");
    }

    // Lo que se ve en la barra (InfoFace lo junta con el temporizador de estudio)
    readonly property var barra: proximas.length === 0 ? [] : [{
        icon: "",
        time: proximas.length === 1 ? t("bar_one", "1 task") : t("bar", "{n} tasks").replace("{n}", proximas.length),
        colorType: "mauve",
        tareas: true
    }]

    property Process lector: Process {
        command: ["vortex-tareas", "listar"]
        stdout: StdioCollector {
            id: salida
            onStreamFinished: {
                try {
                    let d = JSON.parse(salida.text);
                    root.boveda = d.boveda || "";
                    root.archivo = d.archivo || "";
                    root.tareas = d.tareas || [];
                } catch (e) {
                    root.tareas = [];
                }
                root.cargado = true;
            }
        }
        onExited: {
            if (root.pendiente) { root.pendiente = false; root.refrescar(); }
        }
    }

    // agregar y marcar hecha: al terminar se vuelve a leer
    property Component accion: Component {
        Process {
            id: p
            onExited: { root.refrescar(); p.destroy(); }
        }
    }

    property Timer reloj: Timer {
        interval: 120000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: root.refrescar()
    }

    property Process aviso: Process {
        command: ["vortex-tareas", "avisar"]
    }

    property Timer avisos: Timer {
        interval: 600000
        repeat: true
        running: true
        onTriggered: if (!root.aviso.running) root.aviso.running = true
    }

    // El primer aviso, un minuto después de entrar (que el escritorio ya esté listo)
    property Timer primerAviso: Timer {
        interval: 60000
        running: true
        onTriggered: if (!root.aviso.running) root.aviso.running = true
    }
}
