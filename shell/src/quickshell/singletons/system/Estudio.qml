pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../../"

// Vortex 1.10: el temporizador de estudio (pomodoro). Lo maneja el panel estudio/EstudioPopup.qml y se
// ve en la barra (bar/faces/info/InfoFace.qml) mientras corre. Vive aquí y no en el panel para que
// siga contando con el panel cerrado; guarda su estado en un archivo, así sobrevive a que el shell se
// reinicie (vortex actualizar lo hace). Al terminar cada bloque suena y avisa; el siguiente empieza
// solo, hasta que lo paras. Mientras estudias pone «No molestar» y al descansar lo deja como estaba.
QtObject {
    id: root

    property string fase: "" // "", "estudio" o "descanso"
    property real fin: 0 // ms (epoch) en que termina el bloque; 0 si está en pausa
    property real restante: 0 // ms que faltan; se pone al día cada segundo
    property int minutosEstudio: 25
    property int minutosDescanso: 5
    property bool noMolestar: true
    property int sesionesHoy: 0
    property string dia: ""
    property bool dndPuesto: false // lo puso el temporizador (y lo quita); si ya estaba, no lo toca

    readonly property bool activo: fase !== ""
    readonly property bool pausado: activo && fin === 0
    readonly property int minutosDescansoLargo: Math.max(15, minutosDescanso * 3)
    property bool cargado: false

    function hoy() {
        return Qt.formatDate(new Date(), "yyyy-MM-dd");
    }

    function texto(ms) {
        let seg = Math.max(0, Math.ceil(ms / 1000));
        let m = Math.floor(seg / 60), s = seg % 60;
        return m + ":" + String(s).padStart(2, "0");
    }

    function t(clave, porDefecto) {
        return I18n.t("estudio." + clave, porDefecto);
    }

    function empezar() {
        if (dia !== hoy()) { dia = hoy(); sesionesHoy = 0; }
        entrar("estudio", minutosEstudio);
        Sounds.playSfx("notifications/Progress.wav", 0.8);
    }

    function pausar() {
        if (!activo || pausado) return;
        restante = Math.max(0, fin - Date.now());
        fin = 0;
        guardar();
    }

    function seguir() {
        if (!pausado) return;
        fin = Date.now() + restante;
        guardar();
    }

    function saltar() {
        if (activo) terminarBloque(false);
    }

    function parar() {
        fase = "";
        fin = 0;
        restante = 0;
        ponerDnd(false);
        guardar();
    }

    function ajustar(estudio, descanso) {
        minutosEstudio = estudio;
        minutosDescanso = descanso; // el bloque en curso no cambia: cuenta desde el siguiente
        guardar();
    }

    function entrar(nuevaFase, minutos) {
        fase = nuevaFase;
        restante = minutos * 60000;
        fin = Date.now() + restante;
        // «No molestar» un momento después: que el aviso de «a estudiar» sí se vea.
        if (nuevaFase === "estudio") dndTimer.restart(); else ponerDnd(false);
        guardar();
    }

    function terminarBloque(avisar) {
        if (fase === "estudio") {
            if (dia !== hoy()) { dia = hoy(); sesionesHoy = 0; }
            sesionesHoy += 1;
            let largo = sesionesHoy % 4 === 0;
            let min = largo ? minutosDescansoLargo : minutosDescanso;
            entrar("descanso", min);
            if (avisar) {
                avisarCon(t("break_title", "Time for a break"),
                          (largo ? t("long_break_body", "You studied {n} blocks today. Rest {m} minutes: stand up, drink water.")
                                 : t("break_body", "Rest {m} minutes. You studied {n} blocks today."))
                              .replace("{n}", sesionesHoy).replace("{m}", min),
                          "notifications/Botanica.wav");
            }
        } else {
            entrar("estudio", minutosEstudio);
            if (avisar) {
                avisarCon(t("study_title", "Back to studying"),
                          t("study_body", "{m} minutes. Stop it from the bar or the panel when you're done.").replace("{m}", minutosEstudio),
                          "notifications/Progress.wav");
            }
        }
    }

    function avisarCon(titulo, cuerpo, sonido) {
        Sounds.playSfx(sonido, 1.0);
        Quickshell.execDetached(["notify-send", "-a", t("app", "Study timer"), "-i", "chronometer", titulo, cuerpo]);
    }

    function ponerDnd(encender) {
        let n = Config.getSetting("notifications", {}) || {};
        let yaEsta = Boolean(n.dnd);
        if (encender && noMolestar && !yaEsta) {
            Config.setSetting("notifications.dnd", true);
            dndPuesto = true;
        } else if (!encender && dndPuesto) {
            if (yaEsta) Config.setSetting("notifications.dnd", false);
            dndPuesto = false;
        }
    }

    property Timer dndTimer: Timer {
        interval: 4000
        onTriggered: if (root.fase === "estudio" && !root.pausado) root.ponerDnd(true)
    }

    property Timer reloj: Timer {
        interval: 1000
        repeat: true
        running: root.activo && !root.pausado
        triggeredOnStart: true
        onTriggered: {
            root.restante = Math.max(0, root.fin - Date.now());
            if (root.restante <= 0) root.terminarBloque(true);
            root.exportar();
        }
    }

    onFaseChanged: exportar()
    onPausadoChanged: exportar()

    // Lo que se ve en la barra (InfoFace junta esto con los temporizadores de las acciones rápidas).
    property var barra: []
    function exportar() {
        if (!activo) { barra = []; return; }
        barra = [{
            icon: pausado ? "" : (fase === "estudio" ? "" : ""),
            time: texto(restante),
            colorType: fase === "estudio" ? "mauve" : "green",
            estudio: true
        }];
    }

    function guardar() {
        if (!cargado) return;
        archivo.setText(JSON.stringify({
            fase: fase, fin: fin, restante: restante, minutosEstudio: minutosEstudio, minutosDescanso: minutosDescanso,
            noMolestar: noMolestar, sesionesHoy: sesionesHoy, dia: dia, dndPuesto: dndPuesto
        }) + "\n");
    }

    property FileView archivo: FileView {
        printErrors: false
        path: Caching.getStateDir("estudio") + "/estado.json"
        onLoaded: root.restaurar(text())
        onLoadFailed: root.restaurar("")
    }

    function restaurar(txt) {
        try {
            let e = JSON.parse(txt);
            minutosEstudio = e.minutosEstudio || 25;
            minutosDescanso = e.minutosDescanso || 5;
            noMolestar = e.noMolestar !== false;
            sesionesHoy = e.dia === hoy() ? (e.sesionesHoy || 0) : 0;
            dia = hoy();
            dndPuesto = Boolean(e.dndPuesto);
            cargado = true;
            if (e.fase) {
                fase = e.fase;
                fin = e.fin || 0;
                restante = fin > 0 ? Math.max(0, fin - Date.now()) : (e.restante || 0);
                // Si el bloque terminó con el shell apagado, sigue con el siguiente sin avisar de más.
                if (fin > 0 && restante <= 0) terminarBloque(true);
            }
        } catch (err) {}
        cargado = true;
        exportar();
    }

    onNoMolestarChanged: {
        if (!cargado) return;
        if (fase === "estudio" && !pausado) ponerDnd(noMolestar);
        guardar();
    }
}
