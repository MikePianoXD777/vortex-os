// Ventana de Vortex App Installer: el logo y dos pestañas («Archivo» e «Instalado»). Se puede soltar
// un archivo en cualquier parte de la ventana. Los botones de cada archivo los decide el motor
// (motor.py → instalador.archivo.acciones); aquí solo se confirma, se elige carpeta y se pide.
import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    visible: true
    width: 760
    height: 720
    minimumWidth: 520
    minimumHeight: 540
    title: "Vortex App Installer"
    color: Tema.fondo

    readonly property var t: instalador.t
    property string pestana: "archivo"

    // --- acciones (también las usan las pruebas) ---

    function elegirArchivo() { selectorArchivo.open() }

    // `accion` es una de instalador.archivo.acciones: {id, texto, confirmar, elegirCarpeta, datos}
    function pedir(accion, extra) {
        var opciones = Object.assign({}, accion.datos || {}, extra || {})
        if (accion.elegirCarpeta) {
            selectorCarpeta.accion = accion.id
            selectorCarpeta.opciones = opciones
            selectorCarpeta.open()
            return
        }
        if (accion.confirmar && accion.confirmar.titulo) {
            confirmar.accion = accion.id
            confirmar.opciones = opciones
            confirmar.titulo = accion.confirmar.titulo
            confirmar.texto = accion.confirmar.texto
            confirmar.textoAceptar = accion.confirmar.aceptar
            confirmar.abierto = true
            return
        }
        instalador.ejecutar(accion.id, opciones)
    }

    function pedirQuitar(item) {
        dialogoQuitar.item = item
        dialogoQuitar.abierto = true
    }

    function cerrarDialogos() {
        confirmar.abierto = false
        dialogoQuitar.abierto = false
        instalador.descartarPregunta()
    }

    Connections {
        target: instalador
        function onAviso(texto, tipo) { aviso.mostrar(texto, tipo) }
        function onMostrarVentana() {
            ventana.pestana = "archivo"
            ventana.show()
            ventana.raise()
            ventana.requestActivate()
        }
        function onCambio() {
            if (instalador.estado === "leyendo")
                ventana.pestana = "archivo"
        }
    }

    Shortcut { sequence: "Ctrl+O"; onActivated: ventana.elegirArchivo() }
    Shortcut { sequence: "Ctrl+Q"; onActivated: ventana.close() }

    // --- cabecera: logo, nombre y pestañas ---
    Item {
        id: cabecera
        anchors.left: parent.left
        anchors.right: parent.right
        height: 76
        readonly property int margen: width < 640 ? 20 : 32

        Row {
            x: cabecera.margen
            anchors.verticalCenter: parent.verticalCenter
            spacing: 12
            Image {
                anchors.verticalCenter: parent.verticalCenter
                width: 38
                height: 38
                sourceSize.width: 76
                sourceSize.height: 76
                source: "logo.svg"
                smooth: true
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: cabecera.width > 600
                text: "Vortex App Installer"
                color: Tema.texto
                font.pixelSize: 18
                font.weight: Font.DemiBold
            }
        }

        Rectangle {   // pestañas en píldora
            id: pestanas
            anchors.right: parent.right
            anchors.rightMargin: cabecera.margen
            anchors.verticalCenter: parent.verticalCenter
            width: filaPestanas.implicitWidth + 8
            height: 44
            radius: 22
            color: Tema.barra
            border.width: 1
            border.color: Tema.borde
            Row {
                id: filaPestanas
                anchors.centerIn: parent
                spacing: 4
                Repeater {
                    model: [{ "id": "archivo", "texto": ventana.t.pestana_archivo, "icono": "archivo" },
                            { "id": "instalado", "texto": ventana.t.pestana_instalado, "icono": "paquete" }]
                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool activa: ventana.pestana === modelData.id
                        width: contenido.implicitWidth + 32
                        height: 36
                        radius: 18
                        color: activa ? Tema.barraSel : zona.containsMouse ? Tema.barraHover : "transparent"
                        Behavior on color { ColorAnimation { duration: 120 } }
                        Row {
                            id: contenido
                            anchors.centerIn: parent
                            spacing: 8
                            Icono {
                                anchors.verticalCenter: parent.verticalCenter
                                width: 17
                                height: 17
                                nombre: modelData.icono
                                color: activa ? Tema.azul : Tema.suave
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: modelData.texto
                                color: activa ? Tema.texto : Tema.suave
                                font.pixelSize: 14
                                font.weight: activa ? Font.DemiBold : Font.Normal
                            }
                            Rectangle {   // cuántas cosas hay instaladas
                                visible: modelData.id === "instalado" && instalador.instalados.length > 0
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.max(22, cuenta.implicitWidth + 12)
                                height: 22
                                radius: 11
                                color: activa ? Tema.azul : Tema.realceFuerte
                                Text {
                                    id: cuenta
                                    anchors.centerIn: parent
                                    text: instalador.instalados.length
                                    color: activa ? Tema.sobreAcento : Tema.texto
                                    font.pixelSize: 12
                                    font.weight: Font.DemiBold
                                }
                            }
                        }
                        MouseArea {
                            id: zona
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: ventana.pestana = modelData.id
                        }
                        Accessible.role: Accessible.PageTab
                        Accessible.name: modelData.texto
                    }
                }
            }
        }
    }

    StackLayout {
        anchors.top: cabecera.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        currentIndex: ventana.pestana === "archivo" ? 0 : 1
        PaginaArchivo { id: paginaArchivo }
        PaginaInstalado { id: paginaInstalado }
    }

    // --- soltar archivos en cualquier parte ---
    DropArea {
        id: soltar
        anchors.fill: parent
        onEntered: function (arrastre) {
            if (arrastre.hasUrls && !instalador.ocupado)
                arrastre.accept(Qt.CopyAction)
            else
                arrastre.accepted = false
        }
        onDropped: function (soltado) {
            if (!soltado.hasUrls || instalador.ocupado)
                return
            ventana.pestana = "archivo"
            instalador.abrirVarios(soltado.urls)
            soltado.accept(Qt.CopyAction)
        }
    }
    Rectangle {
        anchors.fill: parent
        anchors.margins: 14
        visible: soltar.containsDrag
        z: 80
        radius: Tema.radioGrande
        color: Tema.velo
        border.width: 2
        border.color: Tema.azul
        Column {
            anchors.centerIn: parent
            spacing: 14
            Icono {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 56
                height: 56
                nombre: "descargar"
                color: Tema.azul
            }
            Text {
                text: ventana.t.soltar_aqui
                color: Tema.texto
                font.pixelSize: 20
                font.weight: Font.DemiBold
            }
        }
    }

    Aviso { id: aviso }

    // --- selectores de archivos y carpetas ---
    FileDialog {
        id: selectorArchivo
        title: ventana.t.seleccionar_archivo
        currentFolder: StandardPaths.writableLocation(StandardPaths.DownloadLocation)
        onAccepted: { ventana.pestana = "archivo"; instalador.abrir(selectedFile.toString()) }
    }
    FolderDialog {
        id: selectorCarpeta
        property string accion: ""
        property var opciones: ({})
        title: ventana.t.seleccionar_carpeta
        currentFolder: StandardPaths.writableLocation(StandardPaths.DownloadLocation)
        onAccepted: instalador.ejecutar(accion, Object.assign({}, opciones, { "carpeta": selectedFolder.toString() }))
    }

    // --- confirmaciones ---
    Dialogo {
        id: confirmar
        property string accion: ""
        property var opciones: ({})
        onAceptado: { abierto = false; instalador.ejecutar(accion, opciones) }
        onCancelado: abierto = false
    }

    Dialogo {   // el destino ya existe: otro nombre o reemplazar
        id: dialogoPregunta
        abierto: instalador.pregunta.titulo !== undefined
        titulo: instalador.pregunta.titulo || ""
        texto: instalador.pregunta.texto || ""
        textoAceptar: instalador.pregunta.aceptar || ""
        tipoAceptar: instalador.pregunta.tipo || "primario"
        onAceptado: instalador.responder()
        onCancelado: instalador.descartarPregunta()
    }

    Dialogo {   // el comprimido tiene contraseña: la escribe la persona, no se guarda
        id: dialogoContrasena
        abierto: instalador.contrasena.accion !== undefined
        titulo: ventana.t.contrasena_titulo
        texto: instalador.contrasena.incorrecta ? ventana.t.contrasena_mala : ventana.t.contrasena_texto
        textoAceptar: ventana.t.boton_descomprimir_con
        aceptarActivo: campoContrasena.text.length > 0
        onAbiertoChanged: if (abierto) { campoContrasena.text = ""; campoContrasena.forceActiveFocus() }
        onAceptado: instalador.enviarContrasena(campoContrasena.text)
        onCancelado: instalador.descartarPregunta()
        TextField {
            id: campoContrasena
            width: parent.width
            height: 46
            echoMode: TextInput.Password
            placeholderText: ventana.t.contrasena_campo
            placeholderTextColor: Tema.tenue
            color: Tema.texto
            selectionColor: Tema.bordeHover
            font.pixelSize: 15
            leftPadding: 16
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
            background: Rectangle {
                radius: 23
                color: Tema.campo
                border.width: campoContrasena.activeFocus ? 2 : 1
                border.color: instalador.contrasena.incorrecta ? Tema.peligro
                            : campoContrasena.activeFocus ? Tema.azul : Tema.bordeCampo
            }
            onAccepted: if (text.length > 0) instalador.enviarContrasena(text)
        }
    }

    Dialogo {
        id: dialogoQuitar
        property var item: ({})
        titulo: ventana.t.quitar_titulo.replace("{nombre}", item.nombre || "")
        texto: item.tipo === "flatpak" ? ventana.t.quitar_texto_flatpak
             : (item.tipo === "appimage" || item.tipo === "programa") ? ventana.t.quitar_texto_app : ventana.t.quitar_texto_otro
        textoAceptar: item.boton || ""
        tipoAceptar: "peligro"
        onAceptado: { abierto = false; instalador.deshacer(item.id) }
        onCancelado: abierto = false
    }
}
