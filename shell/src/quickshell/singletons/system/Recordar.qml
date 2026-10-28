pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../../"

// Vortex 1.10: los recordatorios del lanzador («recuérdame en 20 min sacar la pizza», ver
// launcher/Recordatorios.js). Viven aquí y no en el lanzador para sonar con el lanzador cerrado, y se
// guardan en un archivo: si la PC estaba apagada a esa hora, suenan al volver a entrar. El aviso
// trae «5 min más». Shell.qml lo despierta al arrancar.
QtObject {
    id: root

    property var lista: [] // [{id, cuando (ms), texto}], la más próxima primero
    property bool cargado: false

    function t(clave, porDefecto) {
        return I18n.t("recordatorios." + clave, porDefecto);
    }

    function iniciar() {
        // Solo con nombrarlo se crea el singleton y se lee el archivo
        return cargado;
    }

    function agregar(cuando, texto) {
        let nueva = lista.slice();
        nueva.push({ "id": Date.now().toString(36) + Math.floor(Math.random() * 1e6).toString(36),
                     "cuando": cuando, "texto": texto });
        nueva.sort((a, b) => a.cuando - b.cuando);
        lista = nueva;
        guardar();
    }

    function quitar(id) {
        lista = lista.filter(r => r.id !== id);
        guardar();
    }

    function guardar() {
        archivo.setText(JSON.stringify(lista, null, 2) + "\n");
    }

    function sonar(r) {
        let titulo = r.texto !== "" ? r.texto : t("default_title", "Reminder");
        let tarde = Date.now() - r.cuando > 120000;
        let cuerpo = tarde
            ? t("missed", "It was for {hora}: the PC was off.").replace("{hora}", Qt.formatDateTime(new Date(r.cuando), "dd/MM HH:mm"))
            : t("body", "Your reminder from the launcher");
        Sounds.playSfx("notifications/WakeUp.mp3", 1.0);
        let p = avisoProc.createObject(root, { "texto": r.texto });
        p.command = ["notify-send", "-a", t("app", "Reminder"), "-u", "critical", "-i", "alarm-symbolic",
                     "--action=posponer=" + t("snooze", "5 more min"), titulo, cuerpo];
        p.running = true;
    }

    // notify-send espera a que des clic: si es «5 min más», vuelve a quedar pendiente
    property Component avisoProc: Component {
        Process {
            id: aviso
            property string texto: ""
            stdout: StdioCollector {
                onStreamFinished: {
                    if (this.text.trim() === "posponer") root.agregar(Date.now() + 5 * 60000, aviso.texto);
                }
            }
            onExited: destroy(1000)
        }
    }

    property Timer reloj: Timer {
        interval: 5000
        repeat: true
        running: root.cargado && root.lista.length > 0
        triggeredOnStart: true
        onTriggered: {
            let ahora = Date.now();
            let toca = root.lista.filter(r => r.cuando <= ahora);
            if (toca.length === 0) return;
            root.lista = root.lista.filter(r => r.cuando > ahora);
            root.guardar();
            for (let i = 0; i < toca.length; i++) root.sonar(toca[i]);
        }
    }

    property FileView archivo: FileView {
        printErrors: false
        path: Caching.getStateDir("recordatorios") + "/lista.json"
        onLoaded: {
            try {
                let d = JSON.parse(text());
                root.lista = Array.isArray(d) ? d.filter(r => r && typeof r.cuando === "number") : [];
            } catch (e) {
                root.lista = [];
            }
            root.cargado = true;
        }
        onLoadFailed: root.cargado = true
    }
}
