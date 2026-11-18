// La ventana de Vortexine's Runner: tu biblioteca de programas de Windows (.exe) que corren con Proton.
// Arriba el logo, la búsqueda y «Instalar programa» / «Agregar .exe»; abajo las tarjetas. Se puede
// arrastrar un .exe a la ventana. Ctrl+F busca, Ctrl+O agrega un .exe, Ctrl+I instala, Ctrl+W cierra.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    width: 1180
    height: 780
    minimumWidth: 760
    minimumHeight: 520
    visible: true
    title: "Vortexine's Runner"
    color: Tema.fondo

    property string modoArchivo: "agregar"   // agregar | instalar: qué hace el FileDialog al aceptar

    readonly property var filtrados: {
        var q = busqueda.text.trim().toLowerCase()
        var lista = runner.programas
        if (q === "") return lista
        return lista.filter(p => p.nombre.toLowerCase().indexOf(q) >= 0)
    }

    function elegirArchivo(modo) { modoArchivo = modo; archivo.open() }
    // Para las pruebas: abre «Configurar» o «Quitar» del programa n de la lista
    function configurar(n) { dialogoPrograma.abrir(filtrados[n]) }
    function pedirQuitar(n) { dialogoQuitar.abrir(filtrados[n]) }

    Connections {
        target: runner
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
        function onAviso(texto, tipo) { aviso.mostrar(texto, tipo) }
        function onElegirInstalado(lista) { dialogoInstalado.abrir(lista) }
        function onPreguntarArchivo(ruta, nombre, esInstalador) { dialogoArchivo.abrir(ruta, nombre, esInstalador) }
    }

    Shortcut { sequence: "Ctrl+F"; onActivated: busqueda.forceActiveFocus() }
    Shortcut { sequence: "Ctrl+O"; onActivated: ventana.elegirArchivo("agregar") }
    Shortcut { sequence: "Ctrl+I"; onActivated: ventana.elegirArchivo("instalar") }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }

    FileDialog {
        id: archivo
        title: ventana.modoArchivo === "instalar" ? "Elige el instalador" : "Elige el .exe del programa"
        nameFilters: ["Programas de Windows (*.exe *.EXE *.msi *.MSI *.bat)", "Todos los archivos (*)"]
        onAccepted: {
            var u = selectedFile.toString()
            if (ventana.modoArchivo === "instalar") runner.instalar(u)
            else runner.agregar(u)
        }
    }

    DialogoPrograma { id: dialogoPrograma; objectName: "dialogoPrograma" }
    DialogoInstalado { id: dialogoInstalado; objectName: "dialogoInstalado" }
    DialogoArchivo { id: dialogoArchivo; objectName: "dialogoArchivo" }
    Dialogo { id: dialogo }

    // Quitar: primero pregunta; luego, si también se borra su espacio (el disco C: con lo instalado).
    Popup {
        id: dialogoQuitar
        objectName: "dialogoQuitar"
        property var programa: null
        function abrir(p) { programa = p; open() }
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 470
        modal: true
        focus: true
        padding: 24
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        Overlay.modal: Rectangle { color: Tema.velo }
        background: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
        contentItem: ColumnLayout {
            spacing: 14
            Text {
                Layout.fillWidth: true
                text: dialogoQuitar.programa ? "¿Quitar «" + dialogoQuitar.programa.nombre + "»?" : ""
                color: Tema.texto
                font.pixelSize: 17
                font.weight: Font.Bold
                wrapMode: Text.Wrap
            }
            Text {
                Layout.fillWidth: true
                text: "«Solo de la biblioteca» deja sus archivos donde están. «Y borrar sus datos» también borra su espacio de Windows (lo que instaló, sus partidas guardadas y ajustes)."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.Wrap
                lineHeight: 1.2
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 10
                Item { Layout.fillWidth: true }
                Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: dialogoQuitar.close() }
                Boton {
                    tipo: "secundario"
                    text: "Solo de la biblioteca"
                    onClicked: { runner.quitar(dialogoQuitar.programa.id, false); dialogoQuitar.close() }
                }
                Boton {
                    tipo: "peligro"
                    text: "Y borrar sus datos"
                    onClicked: { runner.quitar(dialogoQuitar.programa.id, true); dialogoQuitar.close() }
                }
            }
        }
    }

    // Arrastrar un .exe (o varios) a la ventana
    DropArea {
        id: soltar
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: (ev) => {
            if (!ev.hasUrls) return
            for (var i = 0; i < ev.urls.length; i++) runner.abrirArchivo(ev.urls[i].toString())
            ev.accept()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        anchors.topMargin: 22
        spacing: 18

        // --- cabecera ---
        RowLayout {
            Layout.fillWidth: true
            spacing: 14
            Image { source: "logo.svg"; sourceSize.width: 96; sourceSize.height: 96; Layout.preferredWidth: 44; Layout.preferredHeight: 44 }
            ColumnLayout {
                spacing: 0
                Text { text: "Vortexine's"; color: Tema.tenue; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 1.2; font.capitalization: Font.AllUppercase }
                Text { text: "Runner"; color: Tema.texto; font.pixelSize: 22; font.weight: Font.Bold }
            }
            Item { Layout.fillWidth: true }
            Rectangle {
                Layout.preferredWidth: 280
                implicitHeight: 40
                radius: 20
                color: Tema.campo
                border.width: 1
                border.color: busqueda.activeFocus ? Tema.azul : Tema.bordeCampo
                visible: runner.programas.length > 0
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 8
                    spacing: 8
                    Icono { Layout.preferredWidth: 16; Layout.preferredHeight: 16; nombre: "buscar"; color: Tema.tenue }
                    TextField {
                        id: busqueda
                        objectName: "busqueda"
                        Layout.fillWidth: true
                        placeholderText: "Buscar en tu biblioteca"
                        placeholderTextColor: Tema.tenue
                        color: Tema.texto
                        font.pixelSize: 13
                        background: null
                        leftPadding: 0
                        Keys.onEscapePressed: text = ""
                    }
                    BotonIcono { visible: busqueda.text !== ""; icono: "cerrar"; tam: 26; onClicked: busqueda.text = "" }
                }
            }
            Boton { tipo: "secundario"; icono: "bajar"; text: "Instalar programa"; onClicked: ventana.elegirArchivo("instalar") }
            Boton { icono: "mas"; text: "Agregar .exe"; onClicked: ventana.elegirArchivo("agregar") }
        }

        // --- falta el motor (umu-launcher y el Proton de CachyOS) ---
        Rectangle {
            Layout.fillWidth: true
            visible: runner.faltaMotor.length > 0
            implicitHeight: motorFila.implicitHeight + 32
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.azul
            RowLayout {
                id: motorFila
                anchors.fill: parent
                anchors.margins: 16
                spacing: 14
                Icono { Layout.preferredWidth: 26; Layout.preferredHeight: 26; nombre: "mando"; color: Tema.azul }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text {
                        Layout.fillWidth: true
                        text: runner.instalandoMotor ? "Instalando lo que hace falta…" : "Falta instalar lo que corre los programas de Windows"
                        color: Tema.texto
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                        wrapMode: Text.Wrap
                    }
                    Text {
                        Layout.fillWidth: true
                        text: runner.instalandoMotor
                            ? "Puede tardar unos minutos (son unos 350 MB). Puedes seguir usando la PC."
                            : "Hay que instalar " + runner.faltaMotor.join(" y ") + " (una sola vez; te pide tu contraseña)."
                        color: Tema.suave
                        font.pixelSize: 12
                        wrapMode: Text.Wrap
                    }
                }
                BusyIndicator { visible: runner.instalandoMotor; running: visible; Layout.preferredWidth: 32; Layout.preferredHeight: 32 }
                Boton { visible: !runner.instalandoMotor; icono: "bajar"; text: "Instalar"; onClicked: runner.instalarMotor() }
            }
        }

        // --- un instalador en marcha ---
        Rectangle {
            Layout.fillWidth: true
            visible: runner.instalando !== ""
            implicitHeight: 58
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                spacing: 12
                BusyIndicator { running: parent.visible; Layout.preferredWidth: 28; Layout.preferredHeight: 28 }
                Text {
                    Layout.fillWidth: true
                    text: "Instalando «" + runner.instalando + "». Sigue los pasos en su ventana; al terminar, te pregunto cuál es el programa."
                    color: Tema.texto
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                }
            }
        }

        // --- la biblioteca ---
        GridView {
            id: rejilla
            objectName: "rejilla"
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: ventana.filtrados.length > 0
            clip: true
            readonly property int columnas: Math.max(1, Math.floor((width + 0) / 220))
            cellWidth: Math.floor(width / columnas)
            cellHeight: 262
            model: ventana.filtrados
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                required property var modelData
                width: rejilla.cellWidth
                height: rejilla.cellHeight
                Tarjeta {
                    anchors.fill: parent
                    anchors.margins: 8
                    programa: parent.modelData
                    onConfigurar: dialogoPrograma.abrir(programa)
                    onPedirQuitar: dialogoQuitar.abrir(programa)
                }
            }
        }

        // --- nada que coincida con la búsqueda ---
        Text {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: runner.programas.length > 0 && ventana.filtrados.length === 0
            text: "Nada en tu biblioteca se llama «" + busqueda.text.trim() + "»."
            color: Tema.tenue
            font.pixelSize: 14
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        // --- biblioteca vacía ---
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: runner.programas.length === 0
            Rectangle {
                anchors.fill: parent
                radius: Tema.radioTarjeta
                color: soltar.containsDrag ? Tema.realce : "transparent"
                border.width: 2
                border.color: soltar.containsDrag ? Tema.azul : Tema.borde
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 48, 520)
                spacing: 12
                Image {
                    Layout.alignment: Qt.AlignHCenter
                    source: "logo.svg"
                    sourceSize.width: 192; sourceSize.height: 192
                    Layout.preferredWidth: 96; Layout.preferredHeight: 96
                    opacity: 0.9
                }
                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    text: "Juega y usa programas de Windows"
                    color: Tema.texto
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                }
                Text {
                    Layout.fillWidth: true
                    text: "Arrastra aquí un .exe, o elige uno. Cada programa va en su propio espacio de Windows y corre con el Proton de CachyOS, como en Steam."
                    color: Tema.suave
                    font.pixelSize: 14
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    lineHeight: 1.2
                }
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                    spacing: 10
                    Boton { tipo: "secundario"; icono: "bajar"; text: "Instalar programa"; onClicked: ventana.elegirArchivo("instalar") }
                    Boton { icono: "mas"; text: "Agregar .exe"; onClicked: ventana.elegirArchivo("agregar") }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    text: "«Instalar» es para los instaladores (setup.exe); «Agregar» para un programa que ya está listo."
                    color: Tema.tenue
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    // Al arrastrar con la biblioteca llena: un velo que dice «Suelta para agregar»
    Rectangle {
        anchors.fill: parent
        visible: soltar.containsDrag && runner.programas.length > 0
        color: Tema.velo
        z: 50
        Rectangle {
            anchors.centerIn: parent
            width: 320; height: 120
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 2
            border.color: Tema.azul
            Text { anchors.centerIn: parent; text: "Suelta para agregarlo"; color: Tema.texto; font.pixelSize: 16; font.weight: Font.DemiBold }
        }
    }

    // Aviso abajo al centro
    Rectangle {
        id: aviso
        objectName: "aviso"
        property string texto: ""
        property string tipo: "info"     // info | error | exito
        function mostrar(t, k) { texto = t; tipo = k || "info"; opacity = 1; ocultar.restart() }
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 24
        width: Math.min(620, textoAviso.implicitWidth + 64)
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
                width: Math.min(implicitWidth, 540)
                text: aviso.texto
                color: Tema.texto
                font.pixelSize: 13
                wrapMode: Text.Wrap
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        MouseArea { anchors.fill: parent; onClicked: aviso.opacity = 0 }
        Timer { id: ocultar; interval: 6000; onTriggered: aviso.opacity = 0 }
    }
}
