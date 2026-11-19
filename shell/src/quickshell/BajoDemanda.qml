import QtQuick

// 4.0: un panel que se carga solo mientras se usa. Se carga al abrirlo y se suelta `soltarTras` ms después
// de cerrarlo (para que su animación de cierre termine y reabrirlo enseguida no lo vuelva a cargar).
// El panel nace ya abierto: su animación de apertura la cuidan SpringProgress.animarAlCrear o su
// Component.onCompleted.
Item {
    id: raiz

    property bool abierto: false
    property url fuente
    property int soltarTras: 20000
    property bool mantener: false

    // La carga va después (Qt.callLater), fuera del cambio de «abierto»: si el panel toca su controlador al
    // crearse, no se mezcla con la misma vinculación.
    property bool cargar: false
    onAbiertoChanged: {
        if (abierto) {
            soltar.stop();
            mantener = true;
            Qt.callLater(() => { raiz.cargar = true; });
        } else {
            soltar.restart();
        }
    }

    Timer {
        id: soltar
        interval: raiz.soltarTras
        onTriggered: {
            raiz.mantener = false;
            raiz.cargar = false;
        }
    }

    Loader {
        active: raiz.cargar && (raiz.abierto || raiz.mantener)
        source: active ? raiz.fuente : ""
    }
}
