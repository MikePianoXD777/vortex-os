import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.Pipewire
import "../"
import "../reusables"

Item {
    required property var panel
    property alias nightLight: nightLightBtn
    property alias coffee: coffeeBtn

    component QuickActionBtn : Rectangle {
        id: qaBtn
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: view.panel.cardRadius

        property bool isActive: false
        property string iconText: ""
        property color activeColor: ThemeBackend.blue
        readonly property bool isHovered: qaMa.containsMouse
        property real customFontSize: 0
        property string customFontFamily: ""
        // Vortex 4.0 (diseño nuevo, islas): fichas grandes con el ícono en un círculo, el nombre y el estado.
        property string etiqueta: ""
        property string estado: isActive ? I18n.t("syspanel.tiles.on", "On") : I18n.t("syspanel.tiles.off", "Off")
        readonly property bool ficha: view.fichas

        property real togglePulse: 0.0

        onIsActiveChanged: {
            if (qaGlass.active) togglePulseAnim.restart();
        }

        SequentialAnimation {
            id: togglePulseAnim
            NumberAnimation { target: qaBtn; property: "togglePulse"; to: 1.0; duration: 90; easing.type: Easing.OutQuad }
            NumberAnimation { target: qaBtn; property: "togglePulse"; to: 0.0; duration: 560; easing.type: Easing.OutCubic }
        }

        signal leftClicked()
        signal rightClicked()

        color: qaGlass.active ? "transparent" : (isActive ? activeColor : ThemeBackend.faded(qaMa.containsMouse ? ThemeBackend.surface1 : Qt.darker(ThemeBackend.surface0, 1.04)))

        GlassFill {
            id: qaGlass
            anchors.fill: parent
            radius: parent.radius
            tintColor: qaBtn.isActive ? qaBtn.activeColor : (qaMa.containsMouse ? ThemeBackend.surface1 : ThemeBackend.surface0)
            tintAmount: qaBtn.isActive ? 0.6 : 0.3
            shadow: 1
            press: qaMa.pressed ? 1 : (qaMa.containsMouse ? 0.25 : 0)
            pulse: qaBtn.togglePulse
            Behavior on press { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }
            Behavior on tintAmount { NumberAnimation { duration: 150 } }
        }
        Behavior on color {
            enabled: view.panel.visible
            ColorAnimation { duration: 150 }
        }

        scale: qaMa.pressed ? 0.95 : (qaMa.containsMouse ? 1.01 : 1.0)
        Behavior on scale {
            enabled: view.panel.visible
            NumberAnimation { duration: 250; easing.type: Easing.OutQuart }
        }

        Rectangle {
            id: qaCirculo
            visible: qaBtn.ficha
            anchors.left: parent.left
            anchors.leftMargin: view.panel.s(10)
            anchors.verticalCenter: parent.verticalCenter
            width: view.panel.s(32)
            height: width
            radius: width / 2
            color: qaBtn.isActive ? Qt.alpha(ThemeBackend.crust, 0.12) : ThemeBackend.surface1
        }

        Column {
            visible: qaBtn.ficha
            anchors.left: qaCirculo.right
            anchors.leftMargin: view.panel.s(10)
            anchors.right: parent.right
            anchors.rightMargin: view.panel.s(8)
            anchors.verticalCenter: parent.verticalCenter
            spacing: view.panel.s(1)
            Text {
                width: parent.width
                text: qaBtn.etiqueta
                elide: Text.ElideRight
                font.family: ThemeBackend.fontFamily
                font.pixelSize: view.panel.s(12)
                font.weight: Font.DemiBold
                color: (qaBtn.isActive && !qaGlass.active) ? ThemeBackend.crust : ThemeBackend.text
            }
            Text {
                width: parent.width
                text: qaBtn.estado
                elide: Text.ElideRight
                font.family: ThemeBackend.fontFamily
                font.pixelSize: view.panel.s(10)
                color: (qaBtn.isActive && !qaGlass.active) ? Qt.alpha(ThemeBackend.crust, 0.7) : ThemeBackend.subtext0
            }
        }

        Text {
            anchors.centerIn: qaBtn.ficha ? qaCirculo : parent
            font.family: qaBtn.customFontFamily !== "" ? qaBtn.customFontFamily : ThemeBackend.iconFont
            font.pixelSize: qaBtn.customFontSize > 0 ? qaBtn.customFontSize : view.panel.s(qaBtn.ficha ? 16 : 18)
            font.weight: qaBtn.customFontSize > 0 ? Font.Bold : Font.Normal
            color: (qaBtn.isActive && !qaGlass.active) ? ThemeBackend.crust : ((qaBtn.isActive || qaMa.containsMouse) ? ThemeBackend.text : ThemeBackend.subtext0)
            text: qaBtn.iconText
            Behavior on color {
                enabled: view.panel.visible
                ColorAnimation { duration: 150 }
            }
        }

        MouseArea {
            id: qaMa
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: (mouse) => {
                if (mouse.button === Qt.RightButton) qaBtn.rightClicked();
                else qaBtn.leftClicked();
            }
        }
    }

    id: view
    z: 5
    readonly property bool fichas: ThemeBackend.barIslas
    readonly property real altoFicha: view.panel.s(50)
    readonly property real altoTotal: fichas ? altoFicha * 3 + view.panel.s(6) * 2 : view.panel.s(73)
    Layout.fillWidth: true
    Layout.preferredHeight: altoTotal
    Layout.maximumHeight: altoTotal
    opacity: view.panel.introQuickActions
    transform: Translate { x: (view.panel.isLeftAnchored ? -view.panel.rowSlideDistance : view.panel.rowSlideDistance) * (1.0 - view.panel.introQuickActions) }

    GridLayout {
        anchors.fill: parent
        columns: view.fichas ? 2 : 5
        rowSpacing: view.panel.s(6)
        columnSpacing: view.panel.s(6)

        QuickActionBtn {
            id: nightLightBtn
            etiqueta: I18n.t("syspanel.tiles.night_light", "Night light")
            iconText: "󰖔"
            activeColor: ThemeBackend.peach

            function updateState() {
                let anyEnabled = false;
                if (typeof Gamma !== "undefined" && typeof Gamma.isAnyEnabled === "function") {
                    anyEnabled = Gamma.isAnyEnabled();
                } else if (typeof Config !== "undefined") {
                    let ds = Config.getSetting("display", {"monitors": {}});
                    let mons = (ds && ds.monitors) ? ds.monitors : {};
                    for (let mName in mons) {
                        if (mons[mName] && mons[mName].enabled) {
                            anyEnabled = true;
                            break;
                        }
                    }
                }
                isActive = anyEnabled;
            }

            Component.onCompleted: updateState()

            Connections {
                target: typeof Config !== "undefined" ? Config : null
                enabled: view.panel.visible
                ignoreUnknownSignals: true
                function onSettingsLoaded() {
                    nightLightBtn.updateState();
                }
            }

            Connections {
                target: typeof Gamma !== "undefined" ? Gamma : null
                enabled: view.panel.visible
                ignoreUnknownSignals: true
                function onSettingsChanged() {
                    nightLightBtn.updateState();
                }
            }

            onLeftClicked: {
                Sounds.playSfx("system/quick_click.wav");
                let target = !nightLightBtn.isActive;
                nightLightBtn.isActive = target;

                let monNames = [];
                let ds = typeof Config !== "undefined" ? Config.getSetting("display", {"monitors": {}}) : {"monitors": {}};
                let mons = (ds && ds.monitors) ? ds.monitors : {};
                for (let m in mons) {
                    if (monNames.indexOf(m) === -1) {
                        monNames.push(m);
                    }
                }
                if (typeof Quickshell !== "undefined" && Quickshell.screens) {
                    for (let i = 0; i < Quickshell.screens.length; i++) {
                        let scr = Quickshell.screens[i];
                        if (scr && scr.name && monNames.indexOf(scr.name) === -1) {
                            monNames.push(scr.name);
                        }
                    }
                }

                if (monNames.length > 0) {
                    for (let i = 0; i < monNames.length; i++) {
                        Gamma.setEnabled(monNames[i], target);
                    }
                } else {
                    Gamma.setEnabled("", target);
                }
                nightLightBtn.updateState();
            }

            onRightClicked: {
                view.panel.closeAnim.start();
                Quickshell.execDetached(["bash", Caching.serpantinumDir + "/scripts/qs_manager.sh", "toggle", "settings", "display"]);
            }
        }

        QuickActionBtn {
            id: coffeeBtn
            etiqueta: I18n.t("syspanel.tiles.caffeine", "Caffeine")
            estado: isActive ? remainingTimeString : I18n.t("syspanel.tiles.off", "Off")
            activeColor: Qt.tint(ThemeBackend.peach, "#5c3016")

            property double coffeeStartTime: 0
            property double coffeeEndTime: 0
            property int remainingSeconds: 0

            readonly property string remainingTimeString: {
                if (isActive && coffeeEndTime <= 0) return "∞";
                let secs = remainingSeconds;
                if (secs <= 0) return "< 1m";
                let h = Math.floor(secs / 3600);
                let m = Math.floor((secs % 3600) / 60);
                if (h > 0) {
                    return h + "h " + (m < 10 ? "0" + m : m) + "m";
                }
                return Math.max(1, m) + "m";
            }

            iconText: (isActive && isHovered && !ficha) ? remainingTimeString : "󰅶"
            customFontSize: (isActive && isHovered && !ficha) ? view.panel.s(12) : 0
            customFontFamily: (isActive && isHovered) ? ThemeBackend.fontFamily : ""

            function updateRemaining() {
                if (!isActive || coffeeEndTime <= 0) {
                    remainingSeconds = 0;
                    return;
                }
                let now = Date.now();
                let diff = Math.round((coffeeEndTime - now) / 1000);
                if (diff <= 0) {
                    remainingSeconds = 0;
                    disableCaffeine();
                } else {
                    remainingSeconds = diff;
                }
            }

            function syncConfig(enabled) {
                if (typeof Config === "undefined") return;
                let idleObj = Object.assign({}, Config.getSetting("idle", {}));
                idleObj.manualInhibit = enabled;
                idleObj.caffeineStartTime = enabled ? coffeeStartTime : 0;
                idleObj.caffeineEndTime = enabled ? coffeeEndTime : 0;
                Config.setSetting("idle", idleObj);
            }

            function enableCaffeine(durationMs) {
                let now = Date.now();
                coffeeStartTime = now;
                coffeeEndTime = now + durationMs;
                isActive = true;
                updateRemaining();
                syncConfig(true);
            }

            function increaseCaffeine(addMs) {
                let now = Date.now();
                if (!isActive || coffeeEndTime <= now) {
                    enableCaffeine(addMs);
                } else {
                    coffeeEndTime += addMs;
                    updateRemaining();
                    syncConfig(true);
                }
            }

            function disableCaffeine() {
                isActive = false;
                coffeeStartTime = 0;
                coffeeEndTime = 0;
                remainingSeconds = 0;
                syncConfig(false);
            }

            function updateState() {
                if (typeof Config === "undefined") return;
                let idleObj = Config.getSetting("idle", {}) || {};
                let endTime = Number(idleObj.caffeineEndTime) || 0;

                if (!idleObj.manualInhibit) {
                    isActive = false;
                    coffeeStartTime = 0;
                    coffeeEndTime = 0;
                    remainingSeconds = 0;
                } else if (endTime > 0 && endTime <= Date.now()) {
                    disableCaffeine();
                } else {
                    coffeeStartTime = Number(idleObj.caffeineStartTime) || 0;
                    coffeeEndTime = endTime;
                    isActive = true;
                    updateRemaining();
                }
            }

            Timer {
                id: coffeeTicker
                interval: view.panel.visible ? 1000 : 15000
                repeat: true
                running: coffeeBtn.isActive
                triggeredOnStart: true
                onTriggered: {
                    coffeeBtn.updateRemaining();
                }
            }

            Component.onCompleted: updateState()

            Connections {
                target: typeof Config !== "undefined" ? Config : null
                enabled: true
                ignoreUnknownSignals: true
                function onSettingsLoaded() {
                    coffeeBtn.updateState();
                }
            }

            onLeftClicked: {
                Sounds.playSfx("system/quick_click.wav");
                if (isActive) {
                    disableCaffeine();
                } else {
                    enableCaffeine(3600 * 1000);
                }
            }

            onRightClicked: {
                Sounds.playSfx("system/quick_click.wav");
                increaseCaffeine(3600 * 1000);
            }
        }

        QuickActionBtn {
            etiqueta: I18n.t("syspanel.tiles.wifi", "Wi-Fi")
            iconText: isActive ? "󰤨" : "󰤮"
            activeColor: ThemeBackend.blue
            isActive: view.panel.wifiRadioEnabled
            onLeftClicked: {
                Sounds.playSfx("system/quick_click.wav");
                Networking.wifiEnabled = !Networking.wifiEnabled;
            }
            onRightClicked: {
                view.panel.closeAnim.start();
                Quickshell.execDetached(["bash", Caching.serpantinumDir + "/scripts/qs_manager.sh", "toggle", "network", "wifi"]);
            }
        }

        QuickActionBtn {
            etiqueta: I18n.t("syspanel.tiles.bluetooth", "Bluetooth")
            iconText: isActive ? "󰂯" : "󰂲"
            activeColor: ThemeBackend.mauve
            isActive: view.panel.btRadioEnabled
            onLeftClicked: {
                Sounds.playSfx("system/quick_click.wav");
                if (Bluetooth.defaultAdapter) {
                    Bluetooth.defaultAdapter.enabled = !Bluetooth.defaultAdapter.enabled;
                }
            }
            onRightClicked: {
                view.panel.closeAnim.start();
                Quickshell.execDetached(["bash", Caching.serpantinumDir + "/scripts/qs_manager.sh", "toggle", "network", "bt"]);
            }
        }

        QuickActionBtn {
            etiqueta: I18n.t("syspanel.tiles.airplane", "Airplane mode")
            Layout.columnSpan: view.fichas ? 2 : 1
            iconText: "󰀝"
            activeColor: ThemeBackend.red
            isActive: !view.panel.wifiRadioEnabled && !view.panel.btRadioEnabled
            onLeftClicked: {
                Sounds.playSfx("system/quick_click.wav");
                let enableAirplane = !isActive;
                if (enableAirplane) {
                    view.panel.btStateBeforeAirplane = view.panel.btRadioEnabled;
                    view.panel.wifiStateBeforeAirplane = view.panel.wifiRadioEnabled;
                    Networking.wifiEnabled = false;
                    if (Bluetooth.defaultAdapter) Bluetooth.defaultAdapter.enabled = false;
                } else {
                    let restoreWifi = view.panel.wifiStateBeforeAirplane;
                    let restoreBt = view.panel.btStateBeforeAirplane;
                    if (!restoreWifi && !restoreBt) {
                        restoreWifi = true;
                    }
                    Networking.wifiEnabled = restoreWifi;
                    if (Bluetooth.defaultAdapter) Bluetooth.defaultAdapter.enabled = restoreBt;
                }
            }
        }
    }
}
