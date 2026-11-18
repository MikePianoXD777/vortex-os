pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Polkit
import "."

Item {
    id: root

    PolkitAgent {
        id: agent

        onAuthenticationRequestStarted: {
            root.errorMessage = "";
            root.requestStarted();
        }
    }

    readonly property bool isActive: agent.isActive
    readonly property bool isRegistered: agent.isRegistered
    readonly property var flow: agent.flow
    property string errorMessage: ""

    // Vortex 4.0: los permisos que pide pkexec llegan como «Authentication is needed to run `comando' as the
    // super user» y con un id técnico debajo. Aquí se dicen en palabras: qué se va a hacer, y el comando real
    // en chiquito (para que se vea qué corre). Los mensajes que ya vienen traducidos (redes, discos) se quedan.
    readonly property string comandoPedido: {
        let m = agent.flow && agent.flow.message ? String(agent.flow.message) : "";
        let r = m.match(/`([^']+)'/) || m.match(/[«"]([^»"]+)[»"]/);
        return r ? r[1] : "";
    }
    readonly property var tareas: [
        [/timedatectl set-ntp/, "time", "Poner la hora automática"],
        [/systemctl enable --now bluetooth/, "bluetooth", "Encender el Bluetooth"],
        [/systemctl disable/, "boot", "Quitar un servicio que traba el arranque"],
        [/\/rm -f .*\.lck|rm -f \/var\/lib\/pacman/, "updates_lock", "Destrabar las actualizaciones"],
        [/paccache|journalctl --vacuum/, "disk", "Liberar espacio en el disco"],
        [/vortex-android/, "android", "Preparar Android para los juegos"],
        [/pacman (-S|-U|-Syu)/, "install", "Instalar o actualizar programas"],
        [/pacman -R/, "remove", "Quitar programas"],
        [/ufw /, "firewall", "Dejar pasar una app en el cortafuegos"],
        [/vortex/, "vortex", "Hacer un ajuste de Vortexine's OS"]
    ]
    function tr(clave, porDefecto) {
        let v = typeof I18n !== "undefined" ? I18n.t(clave, porDefecto) : porDefecto;
        return (v && v !== clave) ? v : porDefecto;
    }
    readonly property string titulo: {
        let c = comandoPedido;
        if (c === "") return agent.flow && agent.flow.message ? String(agent.flow.message) : tr("polkit.default_message", "Una aplicación está solicitando privilegios administrativos.");
        for (let i = 0; i < tareas.length; i++)
            if (tareas[i][0].test(c)) return tr("polkit.task." + tareas[i][1], tareas[i][2]);
        return tr("polkit.task.generic", "Hacer un cambio en el sistema");
    }
    readonly property string detalle: comandoPedido !== ""
        ? tr("polkit.enter_password", "Escribe tu contraseña para continuar.")
        : tr("polkit.default_description", "Se requiere autenticación para ejecutar esta acción como superusuario.")
    // El comando, corto: sin rutas (/usr/bin/timedatectl → timedatectl) y sin pasar de ~90 letras.
    readonly property string comandoCorto: {
        let c = comandoPedido.replace(/(^|\s)\/(?:usr\/)?(?:local\/)?s?bin\//g, "$1");
        return c.length > 90 ? c.slice(0, 88) + "…" : c;
    }

    signal requestStarted()
    signal requestCancelled()
    signal authenticationFailed()
    signal authenticationSucceeded()

    function submit(response) {
        if (agent.flow && typeof agent.flow.submit === "function") {
            agent.flow.submit(response);
        }
    }

    function cancel() {
        if (agent.flow && typeof agent.flow.cancelAuthenticationRequest === "function") {
            agent.flow.cancelAuthenticationRequest();
        }
        root.errorMessage = "";
    }

    Connections {
        target: agent.flow

        function onAuthenticationFailed() {
            if (agent.flow && agent.flow.supplementaryMessage) {
                root.errorMessage = agent.flow.supplementaryMessage;
            } else {
                root.errorMessage = typeof I18n !== "undefined" ? I18n.t("polkit.error_failed") : "Authentication failed. Please try again.";
            }
            root.authenticationFailed();
        }

        function onAuthenticationSucceeded() {
            root.errorMessage = "";
            root.authenticationSucceeded();
        }

        function onAuthenticationRequestCancelled() {
            root.errorMessage = "";
            root.requestCancelled();
        }

        function onSupplementaryMessageChanged() {
            if (agent.flow && agent.flow.supplementaryIsError && agent.flow.supplementaryMessage) {
                root.errorMessage = agent.flow.supplementaryMessage;
            }
        }
    }
}
