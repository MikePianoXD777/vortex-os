pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../../"

// 4.5: «Se fue el internet». Avisa cuando la PC se queda sin internet estando conectada a una red, y cuando
// vuelve, con cuánto tiempo estuvo caído. Lee el estado de NetworkManager con `serpantinum-core watch network`
// (el mismo que usa el panel de red; solo escribe cuando algo cambia, así que no gasta nada esperando).
// Caídas de menos de 10 s no se avisan, y si tú te desconectaste (sin cable ni Wi-Fi conectado) tampoco.
// (Primer borrador de Codex; Claude le quitó el reloj de cada segundo y lo de contar lo/docker como red.)
Item {
    id: root

    property bool iniciado: false
    property bool conInternet: false
    property real caidaDesde: 0
    property bool avisoSeFue: false

    function iniciar() {} // lo llama Shell.qml para que el singleton se cree al arrancar

    // Una red de verdad: con IPv4 y que no sea la de la propia PC ni las de Docker o las VMs.
    function hayRed(estado) {
        let ifs = estado.interfaces || {};
        for (let n in ifs) {
            if (/^(lo|docker|br-|veth|virbr|vnet|tun|tap|wg|tailscale|zt)/.test(n)) continue;
            if (ifs[n] && ifs[n].ipv4) return true;
        }
        return false;
    }

    function duracion(ms) {
        let min = Math.floor(ms / 60000);
        if (min >= 1) return I18n.t("sysnotif.network.back_min", { "n": String(min) });
        return I18n.t("sysnotif.network.back_sec", { "n": String(Math.max(1, Math.floor(ms / 1000))) });
    }

    function avisar(icono, titulo, texto) {
        Quickshell.execDetached(["notify-send", "-a", "Vortexine's OS", "-i", icono, titulo, texto]);
    }

    function cambio(estado) {
        let c = estado.connectivity || "unknown";
        if (c === "unknown") return; // NetworkManager sin revisar la conexión: no se sabe
        let hay = c === "full";
        if (!root.iniciado) { // al arrancar solo se anota cómo está
            root.iniciado = true;
            root.conInternet = hay;
            return;
        }
        if (root.conInternet && !hay) {
            root.conInternet = false;
            if (hayRed(estado)) { // conectado a una red, pero sin internet
                root.caidaDesde = Date.now();
                root.avisoSeFue = false;
                esperaCaida.restart();
            }
        } else if (!root.conInternet && hay) {
            root.conInternet = true;
            esperaCaida.stop();
            if (root.caidaDesde > 0 && root.avisoSeFue) {
                avisar("network-wired", I18n.t("sysnotif.network.back_title", "Volvió el internet"),
                    duracion(Date.now() - root.caidaDesde));
            }
            root.caidaDesde = 0;
            root.avisoSeFue = false;
        }
    }

    Process {
        running: true
        command: ["serpantinum-core", "watch", "network"]
        stdout: SplitParser {
            onRead: data => {
                let e;
                try { e = JSON.parse(data); } catch (err) { return; }
                root.cambio(e);
            }
        }
    }

    // Los microcortes no cuentan: si a los 10 s sigue sin internet, se avisa.
    Timer {
        id: esperaCaida
        interval: 10000
        onTriggered: {
            if (root.conInternet || root.caidaDesde <= 0) return;
            root.avisoSeFue = true;
            root.avisar("network-offline", I18n.t("sysnotif.network.gone_title", "Se fue el internet"),
                I18n.t("sysnotif.network.gone_body", "Estás conectado a la red, pero no llega internet. Te aviso cuando vuelva."));
        }
    }
}
