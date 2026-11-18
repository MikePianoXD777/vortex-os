import QtQuick
import "../../../"
import "../../../reusables"

Item {
    id: el

    property string variant: "round"
    property var ctx: null

    readonly property real side: Math.min(width, height)
    readonly property string avatarSource: {
        if (el.ctx && el.ctx.faceIconPath !== undefined && el.ctx.faceIconPath !== "") return el.ctx.faceIconPath;
        if (SystemInfo.avatarPath === "") return "";
        return SystemInfo.avatarPath.startsWith("file://") ? SystemInfo.avatarPath : "file://" + SystemInfo.avatarPath;
    }

    readonly property string iniciales: {
        let u = (el.ctx && el.ctx.currentUser) ? String(el.ctx.currentUser) : String(SystemInfo.username || "");
        let partes = u.trim().split(/[\s._-]+/).filter(p => p.length > 0);
        if (partes.length === 0) return "?";
        if (partes.length === 1) return partes[0].substring(0, 2).toUpperCase();
        return (partes[0][0] + partes[1][0]).toUpperCase();
    }

    ImageBox {
        anchors.centerIn: parent
        size: Math.round(el.side)
        cornerRadius: el.variant === "rounded" ? Math.round(ThemeBackend.borderRadius * 1.5) : Math.round(el.side / 2)
        imageRadius: cornerRadius
        source: el.avatarSource
        // Vortex (rediseño «Mezcla»): sin foto, un círculo del color de acento con las iniciales del usuario.
        backgroundColor: el.avatarSource === "" ? ThemeBackend.blue : "transparent"

        Text {
            anchors.centerIn: parent
            visible: el.avatarSource === ""
            text: el.iniciales
            font.family: ThemeBackend.titleFont
            font.weight: ThemeBackend.pesoTitulo(Font.Bold)
            font.pixelSize: el.side / 3
            color: ThemeBackend.crust
        }
    }
}
