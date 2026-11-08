// La ventana de Vortexine's Local AI: a la izquierda la navegación y el estado de Ollama; al centro la
// página (Chat, Modelos, Vault o Ajustes). Ctrl+1…4 cambia de página, Ctrl+N empieza un chat nuevo.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    width: 1280
    height: 820
    minimumWidth: 980
    minimumHeight: 600
    visible: true
    title: "Vortexine's Local AI"
    color: Tema.fondo
    // Los enlaces del Markdown (chat y vista previa) toman el acento, no el azul puro de Qt
    palette.link: Tema.azul
    palette.linkVisited: Tema.violeta

    property string pagina: "chat"   // chat | modelos | vault | ajustes
    readonly property var paginas: ["chat", "modelos", "vault", "ajustes"]

    function irA(p) { if (paginas.indexOf(p) >= 0) pagina = p }

    Connections {
        target: ia
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
        function onAviso(texto, tipo) { aviso.mostrar(texto, tipo) }
        function onIrA(p) { ventana.irA(p) }
    }

    Shortcut { sequence: "Ctrl+1"; onActivated: ventana.pagina = "chat" }
    Shortcut { sequence: "Ctrl+2"; onActivated: ventana.pagina = "modelos" }
    Shortcut { sequence: "Ctrl+3"; onActivated: ventana.pagina = "vault" }
    Shortcut { sequence: "Ctrl+4"; onActivated: ventana.pagina = "ajustes" }
    Shortcut { sequence: "Ctrl+N"; onActivated: { ventana.pagina = "chat"; chat.nuevo() } }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }

    component Navegar: Rectangle {
        id: nav
        property string texto: ""
        property string icono: ""
        property string atajo: ""
        property bool elegido: false
        signal pulsado()
        Layout.fillWidth: true
        implicitHeight: 42
        radius: 12
        color: elegido ? Tema.barraSel : zonaNav.containsMouse ? Tema.barraHover : "transparent"
        Behavior on color { ColorAnimation { duration: 120 } }
        Rectangle {   // la rayita del elegido
            width: 3; height: nav.elegido ? 18 : 0; radius: 2
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.azul
            Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 12
            spacing: 12
            Icono { Layout.preferredWidth: 19; Layout.preferredHeight: 19; nombre: nav.icono; color: nav.elegido ? Tema.azul : Tema.suave }
            Text {
                Layout.fillWidth: true
                text: nav.texto
                color: nav.elegido ? Tema.texto : Tema.suave
                font.pixelSize: 14
                font.weight: nav.elegido ? Font.DemiBold : Font.Normal
            }
            Text { text: nav.atajo; color: Tema.tenue; font.pixelSize: 11; opacity: zonaNav.containsMouse ? 1 : 0 }
        }
        MouseArea { id: zonaNav; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: nav.pulsado() }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // --- lateral ---
        Rectangle {
            Layout.preferredWidth: Tema.anchoLateral
            Layout.fillHeight: true
            color: Tema.barra
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 4
                RowLayout {
                    Layout.leftMargin: 6
                    Layout.bottomMargin: 18
                    Layout.topMargin: 6
                    spacing: 10
                    Image { source: "logo.svg"; sourceSize.width: 64; sourceSize.height: 64; Layout.preferredWidth: 34; Layout.preferredHeight: 34 }
                    ColumnLayout {
                        spacing: 0
                        Text { text: "Vortexine's"; color: Tema.tenue; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 1.2; font.capitalization: Font.AllUppercase }
                        Text { text: "Local AI"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.Bold }
                    }
                }
                Navegar { texto: "Chat"; icono: "chat"; atajo: "Ctrl+1"; elegido: ventana.pagina === "chat"; onPulsado: ventana.pagina = "chat" }
                Navegar { texto: "Modelos"; icono: "modelos"; atajo: "Ctrl+2"; elegido: ventana.pagina === "modelos"; onPulsado: ventana.pagina = "modelos" }
                Navegar { texto: "Vault"; icono: "vault"; atajo: "Ctrl+3"; elegido: ventana.pagina === "vault"; onPulsado: ventana.pagina = "vault" }
                Navegar { texto: "Ajustes"; icono: "ajustes"; atajo: "Ctrl+4"; elegido: ventana.pagina === "ajustes"; onPulsado: ventana.pagina = "ajustes" }

                Item { Layout.fillHeight: true }

                // Estado de Ollama (el motor que corre los modelos)
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: estadoCol.implicitHeight + 24
                    radius: Tema.radioTarjeta - 6
                    color: Tema.tarjeta
                    border.width: 1
                    border.color: Tema.borde
                    ColumnLayout {
                        id: estadoCol
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 8
                        RowLayout {
                            spacing: 8
                            Punto { estado: ia.estadoOllama }
                            Text {
                                Layout.fillWidth: true
                                text: ia.estadoOllama === "corriendo" ? "Ollama corriendo"
                                    : ia.estadoOllama === "arrancando" ? "Arrancando Ollama…"
                                    : ia.estadoOllama === "falta" ? "Falta Ollama" : "Ollama detenido"
                                color: Tema.texto
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            visible: ia.gpu !== ""
                            text: ia.gpu
                            color: Tema.tenue
                            font.pixelSize: 11
                            elide: Text.ElideRight
                        }
                        Boton {
                            Layout.fillWidth: true
                            visible: ia.estadoOllama === "detenido"
                            tipo: "secundario"
                            icono: "play"
                            text: "Arrancar"
                            onClicked: ia.arrancarOllama()
                        }
                    }
                }
            }
        }

        // --- página ---
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: ventana.paginas.indexOf(ventana.pagina)
            PaginaChat { id: paginaChat; activa: ventana.pagina === "chat"; onIrA: (p) => ventana.irA(p) }
            PaginaModelos { activa: ventana.pagina === "modelos" }
            PaginaVault { id: paginaVault; activa: ventana.pagina === "vault" }
            PaginaAjustes { activa: ventana.pagina === "ajustes"; onIrA: (p) => ventana.irA(p) }
        }
    }

    // Aviso abajo al centro: «Listo, guardé el chat en tu vault.», errores amables…
    Rectangle {
        id: aviso
        property string texto: ""
        property string tipo: "info"     // info | error | exito
        function mostrar(t, k) { texto = t; tipo = k || "info"; opacity = 1; ocultar.restart() }
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 24
        anchors.horizontalCenterOffset: Tema.anchoLateral / 2
        width: Math.min(600, textoAviso.implicitWidth + 64)
        height: Math.max(46, textoAviso.implicitHeight + 24)
        radius: 23
        color: Tema.tarjeta
        border.width: 1
        border.color: tipo === "error" ? Tema.peligroBorde : Tema.bordeHover
        opacity: 0
        visible: opacity > 0
        z: 100
        Behavior on opacity { NumberAnimation { duration: 180 } }
        Row {
            anchors.centerIn: parent
            spacing: 10
            Icono {
                width: 18; height: 18
                anchors.verticalCenter: parent.verticalCenter
                nombre: aviso.tipo === "error" ? "alerta" : aviso.tipo === "exito" ? "visto" : "info"
                color: aviso.tipo === "error" ? Tema.peligro : aviso.tipo === "exito" ? Tema.exito : Tema.azul
            }
            Text {
                id: textoAviso
                width: Math.min(implicitWidth, 520)
                text: aviso.texto
                color: Tema.texto
                font.pixelSize: 13
                wrapMode: Text.Wrap
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        MouseArea { anchors.fill: parent; onClicked: aviso.opacity = 0 }
        Timer { id: ocultar; interval: 5000; onTriggered: aviso.opacity = 0 }
    }
}
