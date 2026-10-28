import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Effects
import "../"
import "../reusables"

Item {
    id: eqView
    required property var root
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    width: parent.width
    x: root.bottomSectionSwitch.currentIndex === 0 ? 0 : -parent.width - root.s(24)
    opacity: root.bottomSectionSwitch.currentIndex === 0 ? 1.0 : 0.0
    visible: opacity > 0.0

    Behavior on x { NumberAnimation { duration: 420; easing.type: Easing.OutQuart } }
    Behavior on opacity { NumberAnimation { duration: 320; easing.type: Easing.OutCubic } }

    ColumnLayout {
        anchors.fill: parent
        spacing: root.s(12)

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: root.s(155)

            Row {
                id: eqSliderRow
                anchors.fill: parent
                z: 1

                // Vortex 1.10: una columna por banda (de 2 a 16, ver MusicEqualizerState) y el «+» al final
                Repeater {
                    model: root.eqBandas.length
                    delegate: Item {
                        id: sliderDelegate
                        width: (eqSliderRow.width - masBanda.width) / root.eqBandas.length
                        height: eqSliderRow.height
                        readonly property var banda: root.eqBandas[index] || { "f": 1000, "g": 0 }

                        HoverHandler { id: hoverBanda }

                        opacity: root.introEqSliders
                        transform: Translate {
                            y: root.s(25) * (1 - root.introEqSliders) + (index * root.s(7) * (1 - root.introEqSliders))
                        }

                        property real dist: root.eqLightningProgress - index
                        property real hitPulse: dist >= 0 && dist < 1.0 ? Math.sin((dist) * Math.PI) : 0.0
                        
                        property real trackPulse: 0.0
                        property real ringPulse: 0.0
                        property real flashFade: 0.0
                        property bool hasFired: false

                        onDistChanged: {
                            if (dist <= 0.05) {
                                hasFired = false;
                            } else if (dist > 0.4 && !hasFired) {
                                hasFired = true;
                                trackPulseAnim.restart();
                                ringPulseAnim.restart();
                                flashFadeAnim.restart();
                            }
                        }

                        SequentialAnimation {
                            id: trackPulseAnim
                            NumberAnimation { target: sliderDelegate; property: "trackPulse"; from: 0.0; to: 1.0; duration: 1000; easing.type: Easing.OutQuart }
                        }
                        SequentialAnimation {
                            id: ringPulseAnim
                            NumberAnimation { target: sliderDelegate; property: "ringPulse"; from: 1.0; to: 0.0; duration: 1500; easing.type: Easing.OutExpo }
                        }
                        SequentialAnimation {
                            id: flashFadeAnim
                            NumberAnimation { target: sliderDelegate; property: "flashFade"; from: 1.0; to: 0.0; duration: 1500; easing.type: Easing.OutSine }
                        }

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: root.s(4)
                            Slider {
                                id: eqSlider
                                Layout.fillHeight: true
                                Layout.alignment: Qt.AlignHCenter
                                orientation: Qt.Vertical
                                from: -12; to: 12
                                stepSize: 1
                                hoverEnabled: true

                                Component.onCompleted: value = Number(sliderDelegate.banda.g) || 0

                                Connections {
                                    target: root
                                    function onEqDataChanged() {
                                        if (!eqSlider.pressed) {
                                            var p = Number(sliderDelegate.banda.g);
                                            if (!isNaN(p)) eqSlider.value = p;
                                        }
                                    }
                                }

                                Behavior on value {
                                    enabled: !eqSlider.pressed
                                    NumberAnimation {
                                        duration: 350
                                        easing.type: Easing.OutQuart
                                    }
                                }

                                onPressedChanged: {
                                    if (!pressed && Math.round(value) !== Math.round(Number(sliderDelegate.banda.g))) {
                                        root.ponerGanancia(index, Math.round(value));
                                    }
                                }

                                background: Rectangle {
                                    id: trackBg
                                    x: eqSlider.leftPadding + (eqSlider.availableWidth - width) / 2
                                    y: eqSlider.topPadding
                                    implicitWidth: root.s(12)
                                    implicitHeight: root.s(130)
                                    width: root.s(12); height: eqSlider.availableHeight
                                    radius: ThemeBackend.borderRadius
                                    
                                    color: trackGlass.active ? "transparent" : Qt.rgba((ThemeBackend.surface0 ? ThemeBackend.surface0.r : 0.2), (ThemeBackend.surface0 ? ThemeBackend.surface0.g : 0.2), (ThemeBackend.surface0 ? ThemeBackend.surface0.b : 0.3), 0.7 * ThemeBackend.elementOpacity)

                                    GlassFill {
                                        id: trackGlass
                                        anchors.fill: parent
                                        radius: parent.radius
                                        tintColor: ThemeBackend.surface0
                                        tintAmount: 0.3
                                    }

                                    layer.enabled: true
                                    layer.effect: MultiEffect {
                                        id: trackEffect
                                        shadowEnabled: true
                                        shadowColor: "#000000"
                                        shadowOpacity: 0.9
                                        shadowBlur: 0.5
                                        shadowVerticalOffset: 1
                                    }

                                    Rectangle {
                                        z: -1
                                        anchors.centerIn: parent
                                        width: parent.width + root.s(17) + sliderDelegate.ringPulse * root.s(34)
                                        height: parent.height + root.s(17) + sliderDelegate.ringPulse * root.s(51)
                                        radius: parent.radius + root.s(8) + sliderDelegate.ringPulse * root.s(17)
                                        color: "transparent"
                                        border.color: root.eqAccentColor
                                        border.width: root.s(2) + sliderDelegate.ringPulse * root.s(4)
                                        opacity: sliderDelegate.ringPulse * 0.8 * (1.0 - root.eqLightningFade)
                                        
                                        layer.enabled: true
                                        layer.effect: MultiEffect { blurEnabled: true; blurMax: 32; blur: 1.0 }
                                    }

                                    Item {
                                        width: parent.width
                                        height: (1 - eqSlider.visualPosition) * parent.height
                                        y: eqSlider.visualPosition * parent.height
                                        
                                        layer.enabled: true
                                        layer.effect: MultiEffect {
                                            maskEnabled: true
                                            maskSource: eqFillMask
                                        }

                                        Rectangle {
                                            id: eqFillMask
                                            anchors.fill: parent
                                            radius: ThemeBackend.borderRadius
                                            visible: false
                                            layer.enabled: true 
                                        }

                                        GlassFill {
                                            id: eqFillGlass
                                            anchors.fill: parent
                                            radius: ThemeBackend.borderRadius
                                            forceClear: true
                                            tintColor: (root ? root.eqAccentColor : ThemeBackend.mauve)
                                            tintAmount: 0.6
                                            tintStrength: 1.0
                                            press: eqSlider.pressed ? 0.5 : (eqSlider.hovered ? 0.2 : 0)
                                            Behavior on press { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                                        }

                                        Rectangle {
                                            anchors.fill: parent
                                            color: eqFillGlass.active ? "transparent" : (root ? root.eqAccentColor : ThemeBackend.mauve)

                                            Rectangle {
                                                anchors.fill: parent
                                                opacity: sliderDelegate.flashFade
                                                gradient: Gradient {
                                                    orientation: Gradient.Vertical
                                                    GradientStop { position: 0.0; color: Qt.lighter(root.eqAccentColor, 1.3) }
                                                    GradientStop { position: 0.5; color: root.eqAccentColor }
                                                    GradientStop { position: 1.0; color: "transparent" }
                                                }
                                            }

                                            Rectangle {
                                                width: parent.width
                                                height: root.s(70)
                                                y: (sliderDelegate.trackPulse * (parent.height + height)) - height
                                                opacity: Math.sin(sliderDelegate.trackPulse * Math.PI) * 2.0 * (1.0 - root.eqLightningFade)
                                                
                                                gradient: Gradient {
                                                    orientation: Gradient.Vertical
                                                    GradientStop { position: 0.0; color: "transparent" }
                                                    GradientStop { position: 0.2; color: root.eqAccentColor }
                                                    GradientStop { position: 0.5; color: ThemeBackend.text || "#ffffff" }
                                                    GradientStop { position: 0.8; color: Qt.lighter(root.eqAccentColor, 1.2) }
                                                    GradientStop { position: 1.0; color: "transparent" }
                                                }
                                                
                                                layer.enabled: true
                                                layer.effect: MultiEffect {
                                                    shadowEnabled: true; shadowColor: root.eqAccentColor; shadowBlur: 1.0; shadowOpacity: 1.0
                                                }
                                            }
                                        }
                                    }
                                }

                                handle: Rectangle {
                                    id: eqHandle
                                    x: eqSlider.leftPadding + (eqSlider.availableWidth - width) / 2
                                    y: eqSlider.topPadding + eqSlider.visualPosition * (eqSlider.availableHeight - height)
                                    implicitWidth: root.s(16)
                                    implicitHeight: root.s(16)
                                    width: root.s(16); height: root.s(16)
                                    radius: root.s(8)

                                    property color handleColor: ThemeBackend.text || "#cdd6f4"
                                    property color handleHoverColor: Qt.lighter(root.eqAccentColor, 1.15)
                                    property color handleDragColor: Qt.lighter(root.eqAccentColor, 1.30)

                                    color: eqHandleGlass.active ? "transparent" : (eqSlider.pressed ? handleDragColor : (eqSlider.hovered ? handleHoverColor : handleColor))

                                    GlassFill {
                                        id: eqHandleGlass
                                        anchors.fill: parent
                                        radius: parent.radius
                                        tintColor: parent.handleColor
                                        tintAmount: 0.4
                                        shadow: 1
                                        press: eqSlider.pressed ? 1 : (eqSlider.hovered ? 0.3 : 0)
                                        Behavior on press { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                                    }
                                    Behavior on color { ColorAnimation { duration: 150 } }

                                    property real targetScale: {
                                        if (eqSlider.pressed) return 1.25;
                                        if (eqSlider.hovered) return 1.08;
                                        return 1.0;
                                    }

                                    Behavior on targetScale {
                                        NumberAnimation {
                                            duration: 150
                                            easing.type: Easing.OutBack
                                        }
                                    }

                                    property real popScale: 1.0
                                    property real flashOpacity: 0.0

                                    SequentialAnimation {
                                        id: eqHandlePopAnim
                                        NumberAnimation { target: eqHandle; property: "popScale"; to: 1.12; duration: 100; easing.type: Easing.OutQuad }
                                        NumberAnimation { target: eqHandle; property: "popScale"; to: 1.0; duration: 200; easing.type: Easing.OutCubic }
                                    }

                                    Connections {
                                        target: eqSlider
                                        function onMoved() {
                                            eqHandlePopAnim.restart();
                                        }
                                        function onPressedChanged() {
                                            if (!eqSlider.pressed) {
                                                eqHandle.flashOpacity = 0.35;
                                                eqHandleFlashAnim.restart();
                                            }
                                        }
                                    }

                                    property var catColors: [
                                        root.eqAccentColor,
                                        ThemeBackend.pink || "#f5c2e7",
                                        ThemeBackend.lavender || "#b4befe",
                                        root.eqAccentColor,
                                        ThemeBackend.blue || "#89b4fa"
                                    ]

                                    Rectangle {
                                        anchors.centerIn: parent
                                        width: parent.width + root.s(31) * sliderDelegate.hitPulse
                                        height: width
                                        radius: width / 2
                                        color: (parent.catColors && parent.catColors.length > 0) ? (parent.catColors[(typeof index !== "undefined" ? index : 0) % parent.catColors.length] || root.eqAccentColor) : root.eqAccentColor
                                        opacity: sliderDelegate.hitPulse * (1.0 - root.eqLightningFade)
                                        layer.enabled: true
                                        layer.effect: MultiEffect { blurEnabled: true; blurMax: 32; blur: 1.0 }
                                    }

                                    Rectangle {
                                        anchors.fill: parent
                                        radius: parent.radius
                                        color: "#ffffff"
                                        opacity: eqHandle.flashOpacity
                                        PropertyAnimation on opacity { id: eqHandleFlashAnim; to: 0; duration: 300; easing.type: Easing.OutCubic }
                                    }

                                    scale: (targetScale * popScale) + (sliderDelegate.hitPulse * 0.4 * (1.0 - root.eqLightningFade))
                                }
                            }
                            // La frecuencia: clic para escribirla («1.5k», «800»), rueda para moverla
                            // poquito a poquito, clic derecho para quitar la banda
                            Item {
                                Layout.fillWidth: true
                                Layout.preferredHeight: root.s(14)

                                Text {
                                    anchors.centerIn: parent
                                    visible: !freqInput.visible
                                    text: root.formatoFrecuencia(sliderDelegate.banda.f)
                                    color: freqZona.containsMouse ? root.eqAccentColor : (ThemeBackend.overlay1 || "#7f849c")
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: root.s(root.eqBandas.length > 12 ? 8.5 : 9.5)
                                    font.bold: true
                                    font.underline: freqZona.containsMouse
                                }

                                TextInput {
                                    id: freqInput
                                    anchors.centerIn: parent
                                    width: Math.max(parent.width, root.s(34))
                                    visible: false
                                    horizontalAlignment: TextInput.AlignHCenter
                                    color: root.eqAccentColor
                                    selectionColor: Qt.alpha(root.eqAccentColor, 0.4)
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: root.s(10)
                                    font.bold: true
                                    maximumLength: 8
                                    function terminar(guardar) {
                                        if (!visible) return;
                                        visible = false;
                                        if (guardar) root.ponerFrecuencia(index, root.leerFrecuencia(text));
                                    }
                                    onAccepted: terminar(true)
                                    onActiveFocusChanged: if (!activeFocus) terminar(true)
                                    Keys.onEscapePressed: (event) => { terminar(false); event.accepted = true; }
                                    Rectangle {
                                        z: -1
                                        anchors.fill: parent
                                        anchors.margins: -root.s(2)
                                        radius: root.s(4)
                                        color: Qt.alpha(ThemeBackend.surface0 || "#313244", 0.95)
                                        border.width: 1
                                        border.color: root.eqAccentColor
                                    }
                                }

                                MouseArea {
                                    id: freqZona
                                    anchors.fill: parent
                                    visible: !freqInput.visible
                                    hoverEnabled: true
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    cursorShape: Qt.PointingHandCursor
                                    onContainsMouseChanged: root.eqAyuda = containsMouse ? I18n.t("music.eq.freq_hint") : ""
                                    onClicked: (mouse) => {
                                        if (mouse.button === Qt.RightButton) {
                                            root.quitarBanda(index);
                                            return;
                                        }
                                        freqInput.text = root.formatoFrecuencia(sliderDelegate.banda.f);
                                        freqInput.visible = true;
                                        freqInput.forceActiveFocus();
                                        freqInput.selectAll();
                                    }
                                    onWheel: (wheel) => {
                                        // Un «clic» de la rueda = 1/12 de octava (el touchpad va más fino)
                                        let paso = Math.pow(2, wheel.angleDelta.y / 120 / 12);
                                        root.ponerFrecuencia(index, sliderDelegate.banda.f * paso);
                                    }
                                }
                            }
                        }

                        // Quitar la banda (aparece al pasar el mouse; siempre quedan al menos 2)
                        Rectangle {
                            z: 3
                            anchors.top: parent.top
                            anchors.right: parent.right
                            anchors.rightMargin: Math.max(0, (parent.width - root.s(16)) / 2 - root.s(14))
                            width: root.s(15)
                            height: width
                            radius: width / 2
                            visible: opacity > 0
                            opacity: hoverBanda.hovered && root.eqBandas.length > root.minBandas && !eqSlider.pressed ? 1 : 0
                            Behavior on opacity { NumberAnimation { duration: 150 } }
                            color: quitarZona.containsMouse ? (ThemeBackend.red || "#f38ba8") : Qt.alpha(ThemeBackend.surface1 || "#45475a", 0.95)
                            Text {
                                anchors.centerIn: parent
                                text: "×"
                                color: quitarZona.containsMouse ? (ThemeBackend.base || "#1e1e2e") : (ThemeBackend.text || "#cdd6f4")
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: root.s(11)
                                font.bold: true
                            }
                            MouseArea {
                                id: quitarZona
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onContainsMouseChanged: root.eqAyuda = containsMouse ? I18n.t("music.eq.remove_band") : ""
                                onClicked: root.quitarBanda(index)
                            }
                        }
                    }
                }

                // Agregar una banda (va en el hueco más grande)
                Item {
                    id: masBanda
                    width: root.eqBandas.length < root.maxBandas ? root.s(30) : 0
                    height: eqSliderRow.height
                    visible: width > 0
                    opacity: root.introEqSliders

                    Rectangle {
                        anchors.centerIn: parent
                        anchors.verticalCenterOffset: -root.s(9)
                        width: root.s(24)
                        height: width
                        radius: width / 2
                        color: masZona.containsMouse ? root.eqAccentColor : Qt.alpha(ThemeBackend.surface0 || "#313244", 0.8)
                        border.width: 1
                        border.color: Qt.alpha(root.eqAccentColor, 0.6)
                        Behavior on color { ColorAnimation { duration: 150 } }
                        Text {
                            anchors.centerIn: parent
                            text: "+"
                            color: masZona.containsMouse ? (ThemeBackend.base || "#1e1e2e") : (ThemeBackend.text || "#cdd6f4")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: root.s(16)
                            font.bold: true
                        }
                        MouseArea {
                            id: masZona
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onContainsMouseChanged: root.eqAyuda = containsMouse ? I18n.t("music.eq.add_band") : ""
                            onClicked: root.agregarBanda()
                        }
                    }
                }
            }

            // Qué hace lo que tienes bajo el mouse
            Text {
                z: 4
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.top
                anchors.topMargin: -root.s(14)
                text: root.eqAyuda
                visible: opacity > 0
                opacity: root.eqAyuda !== "" ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 150 } }
                color: ThemeBackend.subtext0 || "#a6adc8"
                font.family: ThemeBackend.fontFamily
                font.pixelSize: root.s(10)
            }

            ShaderEffect {
                id: lightningCanvas
                anchors.fill: parent
                // El rayo de los presets está hecho para las 10 bandas de siempre
                opacity: root.eqBandas.length === 10 ? 1.0 - root.eqLightningFade : 0
                z: 0

                property real animTime: 0.0
                NumberAnimation on animTime {
                    running: root.active
                    loops: Animation.Infinite
                    from: 0; to: 1000
                    duration: 1000000
                }

                property vector2d itemSize: Qt.vector2d(width, height)
                property real time: animTime
                property real progress: root.eqLightningProgress
                property real fade: root.eqLightningFade
                property real scale: root.s(1.0)
                property color accentColor: root.eqAccentColor

                property vector4d bands0: Qt.vector4d(
                    root.eqBandas.length > 0 ? Number(root.eqBandas[0].g) : 0,
                    root.eqBandas.length > 1 ? Number(root.eqBandas[1].g) : 0,
                    root.eqBandas.length > 2 ? Number(root.eqBandas[2].g) : 0,
                    root.eqBandas.length > 3 ? Number(root.eqBandas[3].g) : 0
                )
                property vector4d bands1: Qt.vector4d(
                    root.eqBandas.length > 4 ? Number(root.eqBandas[4].g) : 0,
                    root.eqBandas.length > 5 ? Number(root.eqBandas[5].g) : 0,
                    root.eqBandas.length > 6 ? Number(root.eqBandas[6].g) : 0,
                    root.eqBandas.length > 7 ? Number(root.eqBandas[7].g) : 0
                )
                property vector4d bands2: Qt.vector4d(
                    root.eqBandas.length > 8 ? Number(root.eqBandas[8].g) : 0,
                    root.eqBandas.length > 9 ? Number(root.eqBandas[9].g) : 0,
                    root.s(10),
                    root.s(30)
                )

                fragmentShader: "file://" + Caching.serpantinumDir + "/assets/shaders/vfx/eq_lightning.frag.qsb"
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: root.s(7)
            
            opacity: root.introPresets
            transform: Translate { y: root.s(18) * (1 - root.introPresets) }

            RowLayout {
                Layout.fillWidth: true
                spacing: root.s(8)
                Repeater {
                    model: ["Flat", "Bass", "Treble", "Vocal"]
                    delegate: MusicPresetButton { required property var modelData; root: eqView.root; name: modelData }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: root.s(8)
                Repeater {
                    model: ["Pop", "Rock", "Jazz", "Classic"]
                    delegate: MusicPresetButton { required property var modelData; root: eqView.root; name: modelData }
                }
            }
        }
    }
}
