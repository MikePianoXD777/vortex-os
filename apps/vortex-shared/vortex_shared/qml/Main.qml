// La ventana de Vortexine Shared: a la izquierda qué mandas y a qué compu; a la derecha lo que le das a otra
// persona para que te mande, y dónde pegar un código.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    width: 980
    height: 660
    minimumWidth: 760
    minimumHeight: 560
    visible: true
    title: "Vortexine Shared"
    color: Tema.fondo

    function elegirDestino(host, usuario) {
        campoHost.text = host
        if (usuario) campoUsuario.text = usuario
        else {
            for (var i = 0; i < vs.recientes.length; i++)
                if (vs.recientes[i].host === host) campoUsuario.text = vs.recientes[i].usuario
        }
        if (campoUsuario.text === "") campoUsuario.forceActiveFocus()
    }

    component Tarjeta: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
    component Titulo: Text { color: Tema.texto; font.pixelSize: 18; font.weight: Font.DemiBold }
    component Etiqueta: Text { color: Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold }
    component Campo: TextField {
        implicitHeight: 40
        color: Tema.texto
        placeholderTextColor: Tema.tenue
        selectionColor: Tema.azul
        selectedTextColor: Tema.sobreAcento
        font.pixelSize: 14
        leftPadding: 14
        rightPadding: 14
        background: Rectangle { radius: 20; color: Tema.campo; border.width: parent.activeFocus ? 2 : 1; border.color: parent.activeFocus ? Tema.azul : Tema.bordeCampo }
    }
    component Boton: AbstractButton {
        id: b
        property bool principal: false
        property string icono: ""
        implicitHeight: 40
        implicitWidth: filaB.implicitWidth + 32
        hoverEnabled: true
        opacity: enabled ? 1 : 0.45
        background: Rectangle {
            radius: 20
            color: b.principal ? (b.hovered ? Tema.azulHover : Tema.azul) : (b.hovered ? Tema.realceFuerte : Tema.campo)
            border.width: b.principal ? 0 : 1
            border.color: Tema.bordeCampo
        }
        contentItem: Item {
            Row {
                id: filaB
                anchors.centerIn: parent
                spacing: 8
                Icono {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: b.icono !== ""
                    width: 17; height: 17
                    nombre: b.icono
                    color: b.principal ? Tema.sobreAcento : Tema.suave
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: b.text
                    color: b.principal ? Tema.sobreAcento : Tema.texto
                    font.pixelSize: 14
                    font.weight: b.principal ? Font.DemiBold : Font.Normal
                }
            }
        }
    }
    component Ficha: AbstractButton {
        id: f
        implicitHeight: 32
        implicitWidth: textoF.implicitWidth + 26
        hoverEnabled: true
        background: Rectangle { radius: 16; color: f.hovered ? Tema.realceFuerte : Tema.campo; border.width: 1; border.color: Tema.bordeCampo }
        contentItem: Text { id: textoF; text: f.text; color: Tema.suave; font.pixelSize: 13; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
    }

    FileDialog {
        id: dialogoArchivos
        title: "Elige qué mandar"
        fileMode: FileDialog.OpenFiles
        onAccepted: vs.agregar(selectedFiles.map((u) => u.toString()))
    }
    FolderDialog {
        id: dialogoCarpeta
        title: "Elige una carpeta"
        onAccepted: vs.agregar([selectedFolder.toString()])
    }
    Connections {
        target: vs
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
    }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }
    onActiveChanged: if (active && !vs.ocupado) vs.actualizarRed()

    RowLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 18

        // --- Mandar ---
        Tarjeta {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 20
                spacing: 12
                Titulo { text: "Mandar" }
                Rectangle {   // suelta aquí
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumHeight: 130
                    radius: 18
                    color: suelta.containsDrag ? Tema.realceFuerte : Tema.campo
                    border.width: 1
                    border.color: suelta.containsDrag ? Tema.azul : Tema.bordeCampo
                    DropArea {
                        id: suelta
                        anchors.fill: parent
                        onDropped: (d) => { if (d.hasUrls) vs.agregar(d.urls.map((u) => u.toString())); d.accepted = true }
                    }
                    ColumnLayout {
                        anchors.centerIn: parent
                        visible: vs.archivos.length === 0
                        spacing: 8
                        Icono { Layout.alignment: Qt.AlignHCenter; Layout.preferredWidth: 34; Layout.preferredHeight: 34; nombre: "subir"; color: Tema.suave }
                        Text { Layout.alignment: Qt.AlignHCenter; text: "Suelta aquí lo que quieres mandar"; color: Tema.suave; font.pixelSize: 14 }
                    }
                    ListView {
                        objectName: "listaArchivos"
                        anchors.fill: parent
                        anchors.margins: 8
                        clip: true
                        visible: vs.archivos.length > 0
                        model: vs.archivos
                        spacing: 2
                        delegate: RowLayout {
                            required property var modelData
                            required property int index
                            width: ListView.view.width
                            height: 36
                            spacing: 10
                            Icono { Layout.leftMargin: 6; nombre: parent.modelData.carpeta ? "carpeta" : "archivo"; color: Tema.suave }
                            Text { Layout.fillWidth: true; text: parent.modelData.nombre; color: Tema.texto; font.pixelSize: 14; elide: Text.ElideMiddle }
                            Text { text: parent.modelData.texto; color: Tema.tenue; font.pixelSize: 13 }
                            AbstractButton {
                                implicitWidth: 28; implicitHeight: 28
                                hoverEnabled: true
                                enabled: !vs.ocupado
                                Accessible.name: "Quitar"
                                onClicked: vs.quitar(parent.index)
                                background: Rectangle { radius: 14; color: parent.hovered ? Tema.realce : "transparent" }
                                contentItem: Item { Icono { anchors.centerIn: parent; width: 14; height: 14; nombre: "cerrar"; color: Tema.suave } }
                            }
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Boton { text: "Archivos"; icono: "archivo"; enabled: !vs.ocupado; onClicked: dialogoArchivos.open() }
                    Boton { text: "Carpeta"; icono: "carpeta"; enabled: !vs.ocupado; onClicked: dialogoCarpeta.open() }
                    Item { Layout.fillWidth: true }
                    Text { visible: vs.archivos.length > 0; text: "Total: " + vs.total; color: Tema.tenue; font.pixelSize: 13 }
                }

                Etiqueta { text: "A QUÉ COMPU"; Layout.topMargin: 4 }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    visible: vs.compus.length > 0 || vs.recientes.length > 0
                    Repeater {
                        model: vs.compus
                        Ficha { required property var modelData; text: modelData.nombre; onClicked: ventana.elegirDestino(modelData.ip, "") }
                    }
                    Repeater {
                        model: vs.recientes.filter((r) => !vs.compus.some((c) => c.ip === r.host))
                        Ficha { required property var modelData; text: modelData.usuario + " @ " + modelData.host; onClicked: ventana.elegirDestino(modelData.host, modelData.usuario) }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Campo { id: campoHost; objectName: "campoHost"; Layout.fillWidth: true; Layout.preferredWidth: 300; placeholderText: "IP o nombre en Tailscale" }
                    Campo { id: campoUsuario; objectName: "campoUsuario"; Layout.fillWidth: true; Layout.preferredWidth: 180; placeholderText: "Usuario"; onAccepted: botonMandar.clicked() }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Boton {
                        id: botonMandar
                        text: "Mandar"
                        icono: "enviar"
                        principal: true
                        enabled: !vs.ocupado && campoHost.text.trim() !== "" && campoUsuario.text.trim() !== "" && vs.archivos.length > 0
                        onClicked: vs.mandar(campoHost.text, campoUsuario.text)
                    }
                    Boton {
                        text: "Crear un código"
                        enabled: !vs.ocupado && vs.archivos.length > 0
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: "Para mandar sin Tailscale: un código que se pega en la otra compu"
                        onClicked: vs.crearCodigo()
                    }
                    Item { Layout.fillWidth: true }
                    Boton { text: "Cancelar"; visible: vs.ocupado; onClicked: vs.cancelar() }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    visible: vs.estado !== "" || vs.codigo !== ""
                    spacing: 6
                    Text {
                        objectName: "estado"
                        Layout.fillWidth: true
                        text: vs.estado
                        wrapMode: Text.WordWrap
                        color: vs.tono === "error" ? Tema.peligro : vs.tono === "exito" ? Tema.exito : Tema.suave
                        font.pixelSize: 13
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        visible: vs.progreso >= 0 && vs.ocupado
                        height: 6
                        radius: 3
                        color: Tema.campo
                        Rectangle { width: parent.width * Math.max(0, vs.progreso); height: parent.height; radius: 3; color: Tema.azul }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        visible: vs.codigo !== ""
                        spacing: 8
                        Campo { Layout.fillWidth: true; readOnly: true; text: vs.codigo; font.pixelSize: 12 }
                        Boton { text: "Copiar"; icono: "copiar"; onClicked: vs.copiar(vs.codigo) }
                    }
                }
            }
        }

        // --- Recibir ---
        Tarjeta {
            Layout.fillHeight: true
            Layout.preferredWidth: 330
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 20
                spacing: 10
                Titulo { text: "Recibir" }
                Text {
                    Layout.fillWidth: true
                    text: "Pasa estos datos a quien te va a mandar algo. Te sale un aviso y nada baja hasta que aceptes."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Etiqueta { text: "TU USUARIO"; Layout.topMargin: 6 }
                Text { objectName: "miUsuario"; text: vs.usuario; color: Tema.texto; font.pixelSize: 15; font.weight: Font.DemiBold }
                Repeater {
                    model: vs.direcciones
                    ColumnLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 4
                        Etiqueta { text: parent.modelData.tipo.toUpperCase(); Layout.topMargin: 6 }
                        RowLayout {
                            spacing: 6
                            Text { Layout.fillWidth: true; text: parent.parent.modelData.ip; color: Tema.texto; font.pixelSize: 15; font.weight: Font.DemiBold }
                            AbstractButton {
                                implicitWidth: 32; implicitHeight: 32
                                hoverEnabled: true
                                Accessible.name: "Copiar"
                                onClicked: vs.copiar(parent.parent.modelData.ip)
                                background: Rectangle { radius: 16; color: parent.hovered ? Tema.realce : "transparent" }
                                contentItem: Item { Icono { anchors.centerIn: parent; width: 16; height: 16; nombre: "copiar"; color: Tema.suave } }
                            }
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: !vs.direcciones.some((d) => d.tipo.startsWith("Tailscale"))
                    text: "Para recibir de lejos sin un código, prende Tailscale."
                    color: Tema.tenue
                    font.pixelSize: 12
                    wrapMode: Text.WordWrap
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    visible: vs.puertoCerrado
                    spacing: 6
                    Text { Layout.fillWidth: true; text: "Tu cortafuegos no deja llegar los avisos."; color: Tema.peligro; font.pixelSize: 13; wrapMode: Text.WordWrap }
                    Boton { text: "Dejarlos pasar"; onClicked: vs.abrirPuerto() }
                }
                Etiqueta { text: "¿TE PASARON UN CÓDIGO?"; Layout.topMargin: 10 }
                Campo { id: campoCodigo; objectName: "campoCodigo"; Layout.fillWidth: true; placeholderText: "Pégalo aquí"; onAccepted: if (text.trim() !== "") vs.recibirCodigo(text) }
                Boton {
                    text: "Recibir"
                    icono: "subir"
                    enabled: !vs.ocupado && campoCodigo.text.trim() !== ""
                    onClicked: vs.recibirCodigo(campoCodigo.text)
                }
                Item { Layout.fillHeight: true }
                Boton { Layout.fillWidth: true; text: "Abrir lo que me han mandado"; icono: "carpeta"; onClicked: vs.abrirCarpeta() }
            }
        }
    }
}
