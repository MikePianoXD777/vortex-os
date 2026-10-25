import QtQuick
import QtQuick.Layouts
import "../../../reusables"
import "../../../"

Item {
    id: pillsFaceRoot
    property var widget: null

    readonly property bool isVertical: widget ? widget.orientation === "vertical" : false
    readonly property int activeIdx: widget ? Number(widget.activeIndex) : -1

    implicitWidth: wsLayout.implicitWidth
    implicitHeight: wsLayout.implicitHeight

    // Vortex: un escritorio con apps crece para que quepan sus íconos (WorkspacesFace.appIconsKey).
    readonly property real iconSize: widget ? widget.s(widget.isCompact ? 12 : 13) : 13
    readonly property real iconGap: widget ? widget.s(3) : 3
    readonly property real iconPad: widget ? widget.s(5) : 5
    function iconsKey(index) {
        return (widget && typeof widget.appIconsKey === "function") ? widget.appIconsKey(index) : "";
    }
    function pillLength(index, active) {
        let base = widget ? widget.s(widget.isCompact ? (active ? 34 : 16) : (active ? 36 : 18)) : (active ? 36 : 18);
        let key = iconsKey(index);
        let n = key === "" ? 0 : key.split("|").length;
        if (n === 0) return base;
        return Math.max(base, n * iconSize + (n - 1) * iconGap + 2 * iconPad);
    }

    Rectangle {
        id: activeHighlight
        z: 3
        radius: widget ? widget.s(widget.isCompact ? 7 : 8) : 8
        color: pillsGlass.active ? "transparent" : ((widget && widget.isCompact) ? Qt.lighter(ThemeBackend.mauve, 1.05) : ThemeBackend.mauve)

        GlassFill {
            id: pillsGlass
            anchors.fill: parent
            radius: parent.radius
            tintColor: ThemeBackend.mauve
            tintAmount: 0.5
            press: 0.3 + 0.7 * activeHighlight.lift
        }

        property int prevIdx: 0
        property int curIdx: widget ? widget.activeIndex : -1

        onCurIdxChanged: {
            if (curIdx >= 0 && prevIdx >= 0) {
                if (curIdx > prevIdx) {
                    startAnim.duration = 400;
                    endAnim.duration = 300;
                } else if (curIdx < prevIdx) {
                    startAnim.duration = 300;
                    endAnim.duration = 400;
                }
            }
            if (curIdx >= 0) {
                prevIdx = curIdx;
            }
        }

        function getPos(index, activeIndex) {
            if (index < 0 || !widget) return 0;
            let pos = 0;
            let spacing = widget.s(widget.isCompact ? 7 : 8);
            for (let i = 0; i < index; i++) {
                if (typeof widget.isShown === "function" && !widget.isShown(i))
                    continue;
                pos += pillsFaceRoot.pillLength(i, i === activeIndex) + spacing;
            }
            return pos;
        }

        property real targetStart: {
            if (widget) {
                widget.hideEmptyWorkspaces;
                widget.activeIndex;
                widget.niriOccupiedMap;
                widget.swayOccupiedMap;
                widget.mangoOccupiedMap;
            }
            return (curIdx >= 0 && widget) ? getPos(curIdx, curIdx) : 0;
        }
        property real targetEnd: (curIdx >= 0 && widget) ? targetStart + pillsFaceRoot.pillLength(curIdx, true) : 0
        property real actualStart: targetStart
        property real actualEnd: targetEnd

        Behavior on actualStart { NumberAnimation { id: startAnim; duration: 380; easing.type: Easing.OutQuint } }
        Behavior on actualEnd { NumberAnimation { id: endAnim; duration: 380; easing.type: Easing.OutQuint } }

        readonly property bool isMoving: Math.abs(actualStart - targetStart) > 0.5 || Math.abs(actualEnd - targetEnd) > 0.5
        property real lift: (pillsGlass.active && isMoving) ? 1.0 : 0.0
        Behavior on lift { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
        scale: 1.0 + 0.14 * lift
        transformOrigin: Item.Center

        readonly property real thickness: widget ? widget.s(widget.isCompact ? 16 : 18) : 18

        x: pillsFaceRoot.isVertical ? wsLayout.x + (wsLayout.width - width) / 2 : wsLayout.x + actualStart
        y: pillsFaceRoot.isVertical ? wsLayout.y + actualStart : wsLayout.y + (wsLayout.height - height) / 2
        width: pillsFaceRoot.isVertical ? thickness : actualEnd - actualStart
        height: pillsFaceRoot.isVertical ? actualEnd - actualStart : thickness
        opacity: (widget && widget.workspaceCount > 0 && widget.activeIndex >= 0) ? 1.0 : 0.0
        Behavior on opacity { NumberAnimation { duration: 180 } }
    }

    AxisFlow {
        id: wsLayout
        vertical: pillsFaceRoot.isVertical
        horizontalItemAlignment: Grid.AlignHCenter
        verticalItemAlignment: Grid.AlignVCenter
        z: 2
        anchors.centerIn: parent
        spacing: widget ? widget.s(widget.isCompact ? 7 : 8) : 8

        Repeater {
            model: widget ? widget.workspaceCount : 0

            delegate: Item {
                id: wsPill
                required property int index

                property bool isOccupied: widget ? widget.isOccupied(index) : false
                property bool isActive: pillsFaceRoot ? index === pillsFaceRoot.activeIdx : false
                property bool initAnimTrigger: false
                property bool shown: widget && typeof widget.isShown === "function" ? widget.isShown(index) : true

                visible: shown
                readonly property real length: shown ? pillsFaceRoot.pillLength(index, isActive) : 0
                readonly property real thickness: widget ? widget.s(widget.isCompact ? 16 : 18) : 18
                width: pillsFaceRoot.isVertical ? thickness : length
                height: pillsFaceRoot.isVertical ? length : thickness

                Behavior on width {
                    enabled: !pillsFaceRoot.isVertical
                    NumberAnimation { duration: 400; easing.type: Easing.OutQuint }
                }
                Behavior on height {
                    enabled: pillsFaceRoot.isVertical
                    NumberAnimation { duration: 400; easing.type: Easing.OutQuint }
                }

                Rectangle {
                    id: wsVisualShape
                    anchors.fill: parent
                    radius: widget ? widget.s(widget.isCompact ? 8 : 10) : 10
                    color: (wsPill.isActive || pillsShapeGlass.active) ? "transparent" : Qt.alpha(wsPill.isOccupied ? ThemeBackend.surface2 : ((widget && widget.isCompact) ? ThemeBackend.surface1 : ThemeBackend.surface0), ThemeBackend.elementOpacity)
                    border.width: 0

                    GlassFill {
                        id: pillsShapeGlass
                        anchors.fill: parent
                        radius: parent.radius
                        allowed: !wsPill.isActive
                        tintColor: wsPill.isOccupied ? ThemeBackend.surface2 : ThemeBackend.surface0
                        tintAmount: 0.3
                        press: wsPillMouse.pressed ? 0.8 : (wsPillMouse.containsMouse ? 0.25 : 0)
                    }

                    Behavior on color { ColorAnimation { duration: 250 } }

                    scale: wsPillMouse.pressed ? 0.88 : (wsPillMouse.containsMouse ? 1.08 : 1.0)
                    Behavior on scale { NumberAnimation { duration: 250; easing.type: Easing.OutQuint } }
                }

                opacity: initAnimTrigger ? 1.0 : 0.0
                transform: Translate {
                    x: (!pillsFaceRoot.isVertical || wsPill.initAnimTrigger) ? 0 : (widget ? widget.s(15) : 15)
                    y: (pillsFaceRoot.isVertical || wsPill.initAnimTrigger) ? 0 : (widget ? widget.s(15) : 15)
                    Behavior on x { NumberAnimation { duration: 650; easing.type: Easing.OutQuint } }
                    Behavior on y { NumberAnimation { duration: 650; easing.type: Easing.OutQuint } }
                }

                Component.onCompleted: {
                    if (widget && widget.barWindow && !widget.barWindow.startupCascadeFinished) {
                        animTimer.interval = index * 50 + 100;
                        if (widget.moduleActive) animTimer.start();
                    } else {
                        initAnimTrigger = true;
                    }
                }

                Timer {
                    id: animTimer
                    running: false
                    repeat: false
                    onTriggered: wsPill.initAnimTrigger = true
                }

                Behavior on opacity { NumberAnimation { duration: 450; easing.type: Easing.OutCubic } }

                MouseArea {
                    id: wsPillMouse
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    anchors.fill: parent
                    onClicked: {
                        if (widget) widget.focusWorkspace(wsPill.index);
                    }
                }
            }
        }
    }
    // Los íconos van en su propia capa, encima del resaltado del escritorio activo (que tapa a las pills),
    // con los mismos tamaños que las pills de abajo.
    AxisFlow {
        id: iconLayer
        vertical: pillsFaceRoot.isVertical
        horizontalItemAlignment: Grid.AlignHCenter
        verticalItemAlignment: Grid.AlignVCenter
        z: 4
        anchors.centerIn: wsLayout
        spacing: wsLayout.spacing
        visible: widget ? widget.showAppIcons === true : false

        Repeater {
            model: widget ? widget.workspaceCount : 0

            delegate: Item {
                id: iconSlot
                required property int index
                readonly property bool shown: widget && typeof widget.isShown === "function" ? widget.isShown(index) : true
                readonly property bool isActive: index === pillsFaceRoot.activeIdx
                readonly property string iconsKey: pillsFaceRoot.iconsKey(index)
                readonly property real length: shown ? pillsFaceRoot.pillLength(index, isActive) : 0
                readonly property real thickness: widget ? widget.s(widget.isCompact ? 16 : 18) : 18
                visible: shown
                width: pillsFaceRoot.isVertical ? thickness : length
                height: pillsFaceRoot.isVertical ? length : thickness
                Behavior on width {
                    enabled: !pillsFaceRoot.isVertical
                    NumberAnimation { duration: 400; easing.type: Easing.OutQuint }
                }
                Behavior on height {
                    enabled: pillsFaceRoot.isVertical
                    NumberAnimation { duration: 400; easing.type: Easing.OutQuint }
                }

                AxisFlow {
                    vertical: pillsFaceRoot.isVertical
                    anchors.centerIn: parent
                    spacing: pillsFaceRoot.iconGap
                    Repeater {
                        model: iconSlot.iconsKey === "" ? [] : iconSlot.iconsKey.split("|")
                        delegate: Image {
                            required property string modelData
                            width: pillsFaceRoot.iconSize
                            height: pillsFaceRoot.iconSize
                            sourceSize: Qt.size(pillsFaceRoot.iconSize * 2, pillsFaceRoot.iconSize * 2)
                            source: modelData
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                            smooth: true
                            mipmap: true
                            cache: true
                            opacity: status === Image.Ready ? (iconSlot.isActive ? 1.0 : 0.85) : 0.0
                            Behavior on opacity { NumberAnimation { duration: 200 } }
                        }
                    }
                }
            }
        }
    }
}
