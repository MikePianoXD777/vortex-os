import QtQuick
import Quickshell
import Quickshell.Services.UPower
import "../../../"

// 4.4: la batería de tus audífonos, controles, mouse y teclado inalámbricos, junto al reloj. Sale sola cuando
// conectas uno que diga su batería (UPower: Bluetooth, el control de Switch o de Xbox, mouse Logitech…) y se
// va cuando lo desconectas. Con 15 % o menos avisa una vez (y otra si vuelve a bajar después de cargarlo).
// Lo bajo no se distingue solo por el color: lleva el icono de batería baja.
Row {
    id: root

    property real tam: 12
    property real tamIcono: 14
    property color colorTexto: ThemeBackend.text

    spacing: tam * 0.6
    visible: lista.length > 0

    // Los que tienen batería propia y no son la de la laptop ni el cargador.
    readonly property var tiposConBateria: [
        UPowerDeviceType.Mouse, UPowerDeviceType.Keyboard, UPowerDeviceType.GamingInput, UPowerDeviceType.Pen,
        UPowerDeviceType.Headset, UPowerDeviceType.Headphones, UPowerDeviceType.Speakers,
        UPowerDeviceType.OtherAudio, UPowerDeviceType.RemoteControl, UPowerDeviceType.Wearable,
        UPowerDeviceType.Toy, UPowerDeviceType.Touchpad, UPowerDeviceType.BluetoothGeneric
    ]

    property int revision: 0
    readonly property var lista: {
        let r = revision;
        let res = [];
        let ds = UPower.devices ? (UPower.devices.values || []) : [];
        for (let i = 0; i < ds.length; i++) {
            let d = ds[i];
            if (!d || !d.ready || d.isLaptopBattery || !d.isPresent) continue;
            if (tiposConBateria.indexOf(d.type) === -1) continue;
            if (!(d.percentage > 0)) continue;
            res.push(d);
        }
        return res;
    }

    function icono(d) {
        switch (d.type) {
        case UPowerDeviceType.GamingInput: return "󰊴";
        case UPowerDeviceType.Mouse: return "󰍽";
        case UPowerDeviceType.Keyboard: return "󰌌";
        case UPowerDeviceType.Headset: return "󰋎";
        case UPowerDeviceType.Headphones: return "󰋋";
        case UPowerDeviceType.Speakers: return "󰓃";
        case UPowerDeviceType.Pen: return "󰏪";
        case UPowerDeviceType.Wearable: return "󰖉";
        default: return "󰂯";
        }
    }

    function cargando(d) {
        return d.state === UPowerDeviceState.Charging || d.state === UPowerDeviceState.FullyCharged;
    }

    // Avisos de batería baja: una vez por dispositivo hasta que se cargue arriba del 20 %.
    property var avisados: ({})
    function revisarBajas() {
        for (let i = 0; i < lista.length; i++) {
            let d = lista[i];
            let clave = d.nativePath || d.model || String(i);
            let pct = Math.round(d.percentage * 100);
            if (pct <= 15 && !cargando(d) && !avisados[clave]) {
                avisados[clave] = true;
                let nombre = d.model || "Tu dispositivo";
                Quickshell.execDetached(["notify-send", "-a", "Vortexine's OS", "-i", "battery-caution",
                    "-u", "critical",
                    nombre + ": batería baja (" + pct + " %)",
                    "Ponlo a cargar pronto para que no se apague a la mitad."]);
            } else if (pct > 20 && avisados[clave]) {
                delete avisados[clave];
            }
        }
    }

    // UPower agrega y quita dispositivos, y cada uno cambia de porcentaje: se rehace la lista.
    Connections {
        target: UPower.devices || null
        ignoreUnknownSignals: true
        function onObjectInsertedPost() { root.revision++; }
        function onObjectRemovedPost() { root.revision++; }
        function onValuesChanged() { root.revision++; }
    }
    Instantiator {
        model: UPower.devices
        delegate: Connections {
            required property var modelData
            target: modelData
            ignoreUnknownSignals: true
            function onPercentageChanged() { root.revision++; root.revisarBajas(); }
            function onStateChanged() { root.revision++; }
            function onReadyChanged() { root.revision++; }
            function onIsPresentChanged() { root.revision++; }
        }
    }
    onListaChanged: revisarBajas()

    Repeater {
        model: root.lista
        Row {
            required property var modelData
            readonly property int pct: Math.round(modelData.percentage * 100)
            readonly property bool baja: pct <= 15 && !root.cargando(modelData)
            spacing: root.tam * 0.25
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.icono(modelData)
                font.family: ThemeBackend.iconFont
                font.pixelSize: root.tamIcono
                color: root.colorTexto
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                // Baja: el icono de batería baja además del color (que no dependa solo del color).
                text: parent.baja ? "󰂃" : (root.cargando(modelData) ? "󰂄" : "")
                visible: text !== ""
                font.family: ThemeBackend.iconFont
                font.pixelSize: root.tamIcono
                color: parent.baja ? ThemeBackend.red : root.colorTexto
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: parent.pct + "%"
                font.family: ThemeBackend.fontFamily
                font.pixelSize: root.tam
                font.bold: true
                color: parent.baja ? ThemeBackend.red : root.colorTexto
            }
        }
    }
}
