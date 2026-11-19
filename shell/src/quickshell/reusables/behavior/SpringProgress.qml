import QtQuick

QtObject {
    id: root

    property bool open: false
    // 4.0: los paneles que se cargan al abrirlos (BajoDemanda.qml) nacen ya abiertos; con esto animan
    // igual que si hubieran estado cargados, en vez de aparecer de golpe.
    property bool animarAlCrear: false
    property bool animated: true
    property int openDuration: 700
    property int closeDuration: 180

    property real value: 0.0
    property bool active: false

    property real time: 0.0
    property real origin: 0.0

    function springValue(t) {
        return 1.0 - Math.exp(-7.0 * t) * (Math.cos(5.62 * t) + 1.2456 * Math.sin(5.62 * t));
    }

    onTimeChanged: {
        if (openAnim.running) value = origin + (1.0 - origin) * springValue(time);
    }

    onValueChanged: {
        if (!open && active && value <= 0.001 && !linger.running) linger.restart();
    }

    onOpenChanged: {
        openAnim.stop();
        closeAnim.stop();
        if (open) {
            linger.stop();
            active = true;
            if (!animated) {
                value = 1.0;
                return;
            }
            origin = value;
            openAnim.restart();
        } else if (!animated || value <= 0.001) {
            value = 0.0;
            linger.restart();
        } else {
            closeAnim.restart();
        }
    }

    Component.onCompleted: {
        if (open && !active) {
            active = true;
            if (animarAlCrear && animated) {
                origin = 0.0;
                openAnim.restart();
            } else {
                value = 1.0;
            }
        }
    }

    property NumberAnimation openAnim: NumberAnimation {
        target: root
        property: "time"
        from: 0.0
        to: 1.0
        duration: root.openDuration
        onFinished: root.value = 1.0
    }

    property NumberAnimation closeAnim: NumberAnimation {
        target: root
        property: "value"
        to: 0.0
        duration: root.closeDuration
        easing.type: Easing.InQuad
    }

    property Timer linger: Timer {
        interval: 60
        onTriggered: {
            if (!root.open) root.active = false;
        }
    }
}
