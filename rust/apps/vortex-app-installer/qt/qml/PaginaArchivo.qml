// Pestaña «Archivo»: arriba el ícono, el nombre, qué es, su tamaño y de dónde vino; después lo que va a
// pasar en palabras sencillas, los avisos y el botón grande con la acción. Abajo, lo que trae (si es
// un comprimido). Todo sale de `instalador.archivo` (motor.para_mostrar).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Flickable {
    id: raiz
    readonly property var t: instalador.t
    readonly property var a: instalador.archivo
    readonly property string estado: instalador.estado
    readonly property int margen: width < 640 ? 20 : 32
    readonly property bool conArchivo: estado !== "vacio"

    contentWidth: width
    contentHeight: columna.implicitHeight + 40
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: Barra {}

    ColumnLayout {
        id: columna
        x: raiz.margen
        y: 8
        width: raiz.width - raiz.margen * 2
        spacing: 16

        // ---------------------------------------------------------------- sin archivo
        Rectangle {
            visible: !raiz.conArchivo
            Layout.fillWidth: true
            Layout.topMargin: 12
            implicitHeight: Math.max(raiz.height - 60, vacio.implicitHeight + 80)
            radius: Tema.radioGrande
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.bordeCampo
            Column {
                id: vacio
                anchors.centerIn: parent
                width: Math.min(parent.width - 48, 460)
                spacing: 16
                Image {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 112
                    height: 112
                    sourceSize.width: 224
                    sourceSize.height: 224
                    source: "logo.svg"
                    smooth: true
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: raiz.t.vacio_titulo
                    color: Tema.texto
                    font.pixelSize: 24
                    font.weight: Font.DemiBold
                    wrapMode: Text.WordWrap
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: raiz.t.vacio_texto
                    color: Tema.suave
                    font.pixelSize: 15
                    lineHeight: 1.25
                    wrapMode: Text.WordWrap
                }
                Item { width: 1; height: 4 }
                Boton {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: raiz.t.boton_elegir_archivo
                    icono: "archivo"
                    grande: true
                    onClicked: ventana.elegirArchivo()
                }
            }
        }

        // ---------------------------------------------------------------- el archivo
        Rectangle {
            visible: raiz.conArchivo
            Layout.fillWidth: true
            implicitHeight: cabeza.implicitHeight + 44
            radius: Tema.radioGrande
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            RowLayout {
                id: cabeza
                x: 22
                y: 22
                width: parent.width - 44
                spacing: 18
                Mosaico {
                    Layout.alignment: Qt.AlignTop
                    tipo: raiz.estado === "leyendo" ? "" : (raiz.a.tipo || "")
                    lado: raiz.width < 560 ? 60 : 76
                    opacity: raiz.estado === "leyendo" ? 0.5 : 1
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text {
                        Layout.fillWidth: true
                        text: raiz.a.nombre || ""
                        color: Tema.texto
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                        maximumLineCount: 3
                        elide: Text.ElideRight
                    }
                    Rectangle {   // qué es
                        visible: raiz.estado !== "leyendo" && (raiz.a.tipoTexto || "") !== ""
                        Layout.maximumWidth: parent.width
                        implicitWidth: tipoTexto.implicitWidth + 24
                        implicitHeight: 28
                        radius: 14
                        color: Tema.realceFuerte
                        Text {
                            id: tipoTexto
                            anchors.centerIn: parent
                            width: Math.min(implicitWidth, parent.width - 24)
                            text: raiz.a.tipoTexto || ""
                            color: Tema.azul
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }
                    }
                    Text {
                        visible: raiz.estado !== "leyendo"
                        Layout.fillWidth: true
                        text: [raiz.a.tamano || "", raiz.a.origen || ""].filter(function (x) { return x !== "" }).join("  ·  ")
                        color: Tema.tenue
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                    }
                    Row {
                        visible: raiz.estado === "leyendo"
                        spacing: 10
                        Girando { width: 20; height: 20; anchors.verticalCenter: parent.verticalCenter }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: raiz.t.leyendo
                            color: Tema.suave
                            font.pixelSize: 14
                        }
                    }
                }
                Rectangle {   // cerrar este archivo
                    Layout.alignment: Qt.AlignTop
                    visible: !instalador.ocupado
                    implicitWidth: 36
                    implicitHeight: 36
                    radius: 18
                    color: cerrar.containsMouse ? Tema.realceFuerte : "transparent"
                    Icono { anchors.centerIn: parent; width: 16; height: 16; nombre: "cerrar"; color: Tema.suave }
                    MouseArea {
                        id: cerrar
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: instalador.cerrarArchivo()
                    }
                    Accessible.role: Accessible.Button
                    Accessible.name: raiz.t.boton_otro_archivo
                }
            }
        }

        // Aviso: viene de internet y corre código
        Rectangle {
            visible: raiz.conArchivo && raiz.estado !== "leyendo" && (raiz.a.aviso || "") !== ""
                     && raiz.estado !== "hecho"
            Layout.fillWidth: true
            implicitHeight: filaAviso.implicitHeight + 28
            radius: Tema.radioCampo
            color: Tema.peligroFondo
            border.width: 1
            border.color: Tema.peligroBorde
            RowLayout {
                id: filaAviso
                x: 16
                y: 14
                width: parent.width - 32
                spacing: 12
                Icono { Layout.alignment: Qt.AlignTop; Layout.preferredWidth: 22; Layout.preferredHeight: 22; width: 22; height: 22; nombre: "escudo"; color: Tema.peligro }
                Text {
                    Layout.fillWidth: true
                    text: raiz.a.aviso || ""
                    color: Tema.texto
                    font.pixelSize: 14
                    lineHeight: 1.2
                    wrapMode: Text.WordWrap
                }
            }
        }

        // Qué es y qué va a pasar, en palabras sencillas
        Text {
            visible: raiz.estado === "listo" || raiz.estado === "trabajando"
            Layout.fillWidth: true
            Layout.leftMargin: 4
            Layout.rightMargin: 4
            text: raiz.a.descripcion || ""
            color: Tema.suave
            font.pixelSize: 15
            lineHeight: 1.3
            wrapMode: Text.WordWrap
        }
        Row {
            visible: (raiz.estado === "listo" || raiz.estado === "trabajando") && (raiz.a.nota || "") !== ""
            Layout.fillWidth: true
            Layout.leftMargin: 4
            spacing: 8
            Icono { width: 16; height: 16; nombre: "info"; color: Tema.tenue; anchors.verticalCenter: parent.verticalCenter }
            Text {
                width: parent.width - 24
                text: raiz.a.nota || ""
                color: Tema.tenue
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
        }

        // Falta un programa (7z, unrar, wine…): el comando para copiarlo
        Comando {
            visible: raiz.estado === "listo" && (raiz.a.comando || "") !== ""
            Layout.fillWidth: true
            titulo: (raiz.a.error || "") !== "" ? raiz.a.error : raiz.t.falta_titulo
            texto: raiz.t.falta_texto
            comando: raiz.a.comando || ""
        }
        Text {   // no se pudo leer (dañado) y no es por un programa que falte
            visible: raiz.estado === "listo" && (raiz.a.error || "") !== "" && (raiz.a.comando || "") === ""
            Layout.fillWidth: true
            Layout.leftMargin: 4
            text: raiz.a.error || ""
            color: Tema.peligro
            font.pixelSize: 14
            wrapMode: Text.WordWrap
        }

        // El mismo programa en Flathub
        Rectangle {
            visible: raiz.estado === "listo" && raiz.a.flathub !== undefined && raiz.a.flathub.id !== undefined
            Layout.fillWidth: true
            implicitHeight: filaFlathub.implicitHeight + 32
            radius: Tema.radioCampo
            color: Tema.realce
            border.width: 1
            border.color: Tema.bordeHover
            RowLayout {
                id: filaFlathub
                x: 16
                y: 16
                width: parent.width - 32
                spacing: 14
                Mosaico {
                    lado: 52
                    tipo: "flatpak"
                    imagen: raiz.a.flathub ? (raiz.a.flathub.icono || "") : ""
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text {
                        text: raiz.t.flathub_titulo
                        color: Tema.azul
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                    }
                    Text {
                        Layout.fillWidth: true
                        text: raiz.a.flathub ? (raiz.a.flathub.nombre || "") : ""
                        color: Tema.texto
                        font.pixelSize: 16
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.fillWidth: true
                        visible: text !== ""
                        text: raiz.a.flathub ? (raiz.a.flathub.resumen || "") : ""
                        color: Tema.suave
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                    }
                }
            }
        }

        // Mods: las instancias de Minecraft de «vortex mods»
        Rectangle {
            visible: raiz.estado === "listo" && raiz.a.instancias !== undefined && raiz.a.instancias.length > 0
            Layout.fillWidth: true
            implicitHeight: listaInstancias.implicitHeight + 32
            radius: Tema.radioCampo
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            Column {
                id: listaInstancias
                x: 16
                y: 16
                width: parent.width - 32
                spacing: 6
                Text {
                    text: raiz.t.instancias_titulo
                    color: Tema.texto
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                    bottomPadding: 4
                }
                Repeater {
                    model: raiz.a.instancias || []
                    delegate: Rectangle {
                        required property var modelData
                        width: listaInstancias.width
                        height: 56
                        radius: 14
                        color: zonaInst.containsMouse ? Tema.tarjetaHover : "transparent"
                        opacity: modelData.compatible ? 1 : 0.55
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 6
                            spacing: 12
                            Icono { Layout.preferredWidth: 22; Layout.preferredHeight: 22; nombre: "cubo"; color: Tema.violeta }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    Layout.fillWidth: true
                                    text: modelData.nombre
                                    color: Tema.texto
                                    font.pixelSize: 14
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: [modelData.launcher, modelData.version, modelData.loader]
                                          .filter(function (x) { return x }).join(" · ")
                                          + (modelData.compatible ? "" : " — " + raiz.t.instancia_incompatible)
                                    color: Tema.tenue
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                }
                            }
                            Boton {
                                text: raiz.t.boton_mandar
                                tipo: modelData.compatible ? "primario" : "secundario"
                                onClicked: instalador.ejecutar("mod_instancia", { "ruta": modelData.ruta,
                                                                                  "instancia": modelData.nombre })
                            }
                        }
                        MouseArea { id: zonaInst; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
                    }
                }
            }
        }

        // Instaladores .sh/.run/.bin: así empiezan
        Rectangle {
            visible: raiz.estado === "listo" && raiz.a.tipo === "script"
            Layout.fillWidth: true
            implicitHeight: colVista.implicitHeight + 32
            radius: Tema.radioCampo
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            ColumnLayout {
                id: colVista
                x: 16
                y: 16
                width: parent.width - 32
                spacing: 10
                Text {
                    text: raiz.t.vista_titulo
                    color: Tema.texto
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                }
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: Math.min(240, textoVista.implicitHeight + 24)
                    radius: 12
                    color: Tema.campo
                    clip: true
                    Flickable {
                        id: deslizaVista
                        anchors.fill: parent
                        anchors.margins: 12
                        contentWidth: textoVista.implicitWidth
                        contentHeight: textoVista.implicitHeight
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: Barra {}
                        TextEdit {
                            id: textoVista
                            text: (raiz.a.vista || "") !== "" ? raiz.a.vista : raiz.t.vista_binario
                            readOnly: true
                            selectByMouse: true
                            color: (raiz.a.vista || "") !== "" ? Tema.texto : Tema.tenue
                            selectionColor: Tema.bordeHover
                            font.family: "monospace"
                            font.pixelSize: 12
                        }
                    }
                }
            }
        }
        Comando {
            visible: raiz.estado === "listo" && (raiz.a.comandoAdmin || "") !== ""
            Layout.fillWidth: true
            titulo: raiz.t.admin_titulo
            texto: raiz.t.admin_texto
            comando: raiz.a.comandoAdmin || ""
        }

        // ---------------------------------------------------------------- botones
        Flow {
            id: botones
            visible: raiz.estado === "listo"
            Layout.fillWidth: true
            Layout.topMargin: 4
            spacing: 10
            Repeater {
                model: raiz.a.acciones || []
                delegate: Boton {
                    required property var modelData
                    text: modelData.texto
                    icono: modelData.icono
                    tipo: modelData.tipo
                    grande: true
                    onClicked: ventana.pedir(modelData)
                }
            }
        }

        // ---------------------------------------------------------------- trabajando
        Rectangle {
            visible: raiz.estado === "trabajando"
            Layout.fillWidth: true
            implicitHeight: colTrabajo.implicitHeight + 40
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.bordeHover
            ColumnLayout {
                id: colTrabajo
                x: 20
                y: 20
                width: parent.width - 40
                spacing: 14
                RowLayout {
                    Layout.fillWidth: true
                    Girando { Layout.preferredWidth: 22; Layout.preferredHeight: 22 }
                    Text {
                        Layout.fillWidth: true
                        text: instalador.trabajo
                        color: Tema.texto
                        font.pixelSize: 16
                        font.weight: Font.DemiBold
                    }
                    Text {
                        visible: instalador.progreso >= 0
                        Layout.preferredWidth: 64
                        horizontalAlignment: Text.AlignRight
                        text: Math.round(instalador.progreso * 100) + " %"
                        color: Tema.azul
                        font.pixelSize: 16
                        font.weight: Font.DemiBold
                    }
                }
                Progreso {
                    Layout.fillWidth: true
                    valor: instalador.progreso
                }
                Boton {
                    Layout.alignment: Qt.AlignRight
                    text: raiz.t.boton_cancelar
                    tipo: "secundario"
                    onClicked: instalador.cancelar()
                }
            }
        }

        // ---------------------------------------------------------------- hecho
        Rectangle {
            visible: raiz.estado === "hecho"
            Layout.fillWidth: true
            implicitHeight: colHecho.implicitHeight + 48
            radius: Tema.radioGrande
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.bordeHover
            ColumnLayout {
                id: colHecho
                x: 24
                y: 24
                width: parent.width - 48
                spacing: 12
                Rectangle {
                    implicitWidth: 56
                    implicitHeight: 56
                    radius: 28
                    color: Tema.realceFuerte
                    Icono { anchors.centerIn: parent; width: 30; height: 30; nombre: "visto"; color: Tema.exito; trazo: 2.6 }
                }
                Text {
                    Layout.fillWidth: true
                    text: instalador.resultado.titulo || ""
                    color: Tema.texto
                    font.pixelSize: 21
                    font.weight: Font.DemiBold
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    text: instalador.resultado.texto || ""
                    color: Tema.suave
                    font.pixelSize: 15
                    lineHeight: 1.25
                    wrapMode: Text.WordWrap
                }
                Text {
                    visible: instalador.resultado.deshacible === false
                    Layout.fillWidth: true
                    text: raiz.t.sin_deshacer
                    color: Tema.tenue
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    spacing: 10
                    Boton {
                        visible: (instalador.resultado.abrir || "") !== "" && (instalador.resultado.id || "") !== ""
                                 || (instalador.resultado.flatpak || "") !== ""
                        text: raiz.t.boton_abrir_app
                        icono: "abrir"
                        grande: true
                        onClicked: instalador.abrirResultado()
                    }
                    Boton {
                        visible: (instalador.resultado.carpeta || "") !== ""
                        text: raiz.t.boton_ver_carpeta
                        icono: "carpeta"
                        grande: true
                        onClicked: instalador.abrirCarpeta(instalador.resultado.carpeta)
                    }
                    Boton {
                        text: raiz.t.boton_otro_archivo
                        icono: "archivo"
                        tipo: "secundario"
                        grande: true
                        onClicked: { instalador.cerrarArchivo(); ventana.elegirArchivo() }
                    }
                }
            }
        }

        // ---------------------------------------------------------------- error
        Rectangle {
            visible: raiz.estado === "error"
            Layout.fillWidth: true
            implicitHeight: colError.implicitHeight + 48
            radius: Tema.radioGrande
            color: Tema.peligroFondo
            border.width: 1
            border.color: Tema.peligroBorde
            ColumnLayout {
                id: colError
                x: 24
                y: 24
                width: parent.width - 48
                spacing: 12
                Icono { Layout.preferredWidth: 32; Layout.preferredHeight: 32; nombre: "alerta"; color: Tema.peligro }
                Text {
                    Layout.fillWidth: true
                    text: instalador.error.titulo || ""
                    color: Tema.texto
                    font.pixelSize: 20
                    font.weight: Font.DemiBold
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    text: instalador.error.texto || ""
                    color: Tema.suave
                    font.pixelSize: 15
                    wrapMode: Text.WordWrap
                }
                Comando {
                    visible: (instalador.error.comando || "") !== ""
                    Layout.fillWidth: true
                    texto: raiz.t.falta_texto
                    comando: instalador.error.comando || ""
                }
                Boton {
                    text: raiz.t.boton_reintentar
                    tipo: "secundario"
                    grande: true
                    onClicked: instalador.volver()
                }
            }
        }

        // ---------------------------------------------------------------- qué trae
        Rectangle {
            visible: (raiz.estado === "listo" || raiz.estado === "trabajando") && (raiz.a.cuantas || 0) > 0
            Layout.fillWidth: true
            implicitHeight: colContenido.implicitHeight + 32
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            ColumnLayout {
                id: colContenido
                x: 18
                y: 16
                width: parent.width - 36
                spacing: 8
                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: raiz.t.contenido_titulo
                        color: Tema.texto
                        font.pixelSize: 15
                        font.weight: Font.DemiBold
                    }
                    Item { Layout.fillWidth: true }
                    Text {
                        text: raiz.a.contenido || ""
                        color: Tema.tenue
                        font.pixelSize: 13
                    }
                }
                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(contentHeight, 300)
                    clip: true
                    interactive: contentHeight > height
                    boundsBehavior: Flickable.StopAtBounds
                    model: raiz.a.entradas || []
                    ScrollBar.vertical: Barra {}
                    delegate: Item {
                        required property var modelData
                        width: lista.width
                        height: 32
                        RowLayout {
                            anchors.fill: parent
                            anchors.rightMargin: 12
                            spacing: 10
                            Icono {
                                Layout.preferredWidth: 16
                                Layout.preferredHeight: 16
                                nombre: modelData.peligrosa ? "alerta" : modelData.carpeta ? "carpeta" : modelData.enlace ? "enlace" : "archivo"
                                color: modelData.peligrosa ? Tema.peligro : modelData.carpeta ? Tema.azul : Tema.tenue
                            }
                            Text {
                                Layout.fillWidth: true
                                text: modelData.nombre
                                color: modelData.peligrosa ? Tema.peligro : Tema.texto
                                font.pixelSize: 13
                                elide: Text.ElideMiddle
                            }
                            Text {
                                text: modelData.tamano
                                color: Tema.tenue
                                font.pixelSize: 12
                            }
                        }
                    }
                }
                Text {
                    visible: (raiz.a.cuantas || 0) > (raiz.a.entradas || []).length
                    text: raiz.t.contenido_y_mas.replace("{n}", (raiz.a.cuantas || 0) - (raiz.a.entradas || []).length)
                          .replace(/\{n\|([^|]*)\|([^}]*)\}/, "$2")
                    color: Tema.tenue
                    font.pixelSize: 12
                }
            }
        }
    }
}
