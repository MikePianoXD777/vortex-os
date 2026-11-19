// Miku: con avatar (la página web en QtWebEngine) o solo el panel de voz. Si QtWebEngine no carga, baja al
// panel con un aviso (lo mismo que hacía `app.py`).
import QtQuick

QtObject {
    id: raiz

    readonly property var avatarComp: mk.avatar ? Qt.createComponent("Avatar.qml") : null
    readonly property bool avatarOk: avatarComp !== null && avatarComp.status === Component.Ready

    property var panel: Instantiator {
        active: !raiz.avatarOk
        delegate: Panel {}
    }
    property var avatar: Instantiator {
        active: raiz.avatarOk
        delegate: raiz.avatarComp
    }

    Component.onCompleted: {
        if (mk.avatar && !avatarOk) {
            console.warn("Miku: no cargó el avatar: " + (avatarComp ? avatarComp.errorString() : ""))
            mk.sinWebEngine()
        }
    }
}
