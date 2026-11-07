// 2.4: el mini reproductor (Ctrl+M o el botón de la barra): una ventanita del color de la portada, encima de
// todo, con la canción, ❤️ y los controles. En Vortexine's sale flotando en la esquina (regla de hyprland.lua
// por su título); en otro escritorio, donde la pongas.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: raiz
    property var principal: null
    property string colorPortada: ""
    title: "Vortex Player · Mini"
    width: 400
    height: 108
    minimumWidth: 320
    minimumHeight: 108
    maximumHeight: 108
    flags: Qt.Window | Qt.WindowStaysOnTopHint
    color: Tema.fondo
    visible: false

    function alternar() {
        if (visible) { hide(); return }
        refrescarColor()
        show(); raise()
    }
    function refrescarColor() { colorPortada = vp.colorDe(rep.cancion.ruta || "") }
    Connections {
        target: rep
        function onCambioCancion() { if (raiz.visible) raiz.refrescarColor() }
    }

    Rectangle {
        anchors.fill: parent
        color: raiz.colorPortada || Tema.tarjeta
        Behavior on color { ColorAnimation { duration: 600 } }

        RowLayout {
            anchors.fill: parent
            anchors.margins: 14
            anchors.bottomMargin: 18
            spacing: 12
            Portada {
                Layout.preferredWidth: 72
                Layout.preferredHeight: 72
                ruta: rep.cancion.ruta || ""
                radio: 8
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: if (raiz.principal) { raiz.principal.show(); raiz.principal.raise(); raiz.principal.requestActivate() }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { Layout.fillWidth: true; text: rep.cancion.titulo || "Nada sonando"; color: "white"; font.pixelSize: 15; font.weight: Font.Bold; elide: Text.ElideRight }
                Text {   // 1.5: la línea de la letra que va sonando (si la letra va con tiempos); si no, el artista
                    readonly property string linea: {
                        const l = vp.letra
                        if (!raiz.visible || !l.sincronizada || l.ruta !== rep.cancion.ruta) return ""
                        const i = vp.lineaActual(rep.posicion)
                        return i >= 0 && l.lineas[i] ? (l.lineas[i].texto || "♪") : ""
                    }
                    Layout.fillWidth: true
                    text: linea || rep.cancion.artista || ""
                    color: linea ? "white" : Qt.rgba(1, 1, 1, 0.75)
                    font.pixelSize: 12
                    font.weight: linea ? Font.DemiBold : Font.Normal
                    elide: Text.ElideRight
                }
                RowLayout {
                    spacing: 2
                    Layout.topMargin: 2
                    BotonIcono {   // 1.4: la de la radio se baja
                        visible: !!rep.cancion.radio
                        tam: 30
                        icono: vp.bajandoRadio.indexOf(rep.cancion.ruta || "") >= 0 ? "visto" : "descargar"
                        ayuda: "Bajar a tu biblioteca"
                        onClicked: vp.bajarDeLaRadio(rep.cancion.ruta)
                    }
                    BotonIcono {
                        readonly property bool gusta: vp.leGusta(rep.cancion.ruta || "", vp.versionGustos)
                        visible: !rep.cancion.radio
                        tam: 30; enabled: !!rep.cancion.ruta
                        icono: gusta ? "corazonLleno" : "corazon"; activo: gusta
                        ayuda: gusta ? "Quitar de Tus me gusta" : "Guardar en Tus me gusta"
                        onClicked: vp.alternarMeGusta(rep.cancion.ruta)
                    }
                    BotonIcono { icono: "anterior"; tam: 30; enabled: rep.hayAnterior; onClicked: rep.anterior() }
                    BotonIcono {
                        icono: rep.pausado ? "play" : "pausa"; relleno: true; tam: 34
                        onClicked: rep.cancion.ruta ? rep.alternar() : vp.tocar(0)
                    }
                    BotonIcono { icono: "siguiente"; tam: 30; enabled: rep.haySiguiente; onClicked: rep.siguiente() }
                }
            }
        }

        // la barra de avance, delgada abajo como en la tarjeta de Spotify
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 10
            height: 3
            radius: 1.5
            color: Qt.rgba(1, 1, 1, 0.25)
            Rectangle {
                width: rep.duracion > 0 ? parent.width * Math.min(1, rep.posicion / rep.duracion) : 0
                height: parent.height
                radius: 1.5
                color: "white"
            }
        }
    }
}
