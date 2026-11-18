// Juegos de Android (4.0): la Play Store dentro de Vortexine's OS, con Waydroid.
// Tres momentos: sin preparar (qué es y un botón), preparando (los pasos) y listo (buscar en la Play
// Store, tus juegos y los populares). El registro en Google es un paso único que se muestra hasta
// que la persona dice que ya lo hizo.
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls.Basic

Pagina {
    id: raiz
    readonly property var est: tienda.androidEstado
    readonly property bool listo: est.preparado === true
    readonly property bool ocupado: tienda.androidFase !== ""
    readonly property bool preparando: tienda.androidFase === "preparando"

    Component.onCompleted: tienda.cargarAndroid()

    Seccion {
        Layout.fillWidth: true
        etiqueta: "Juegos de Android"
        titulo: "La Play Store, en tu PC"
        tamTitulo: 28
        subtitulo: raiz.listo
            ? "Busca un juego, instálalo en la Play Store y aparece en tu lanzador como cualquier app. Android se prende solo cuando juegas y se apaga al cerrar."
            : "Juega los juegos de tu celular aquí, en su propia ventana, con teclado, mouse o control. Android solo corre mientras juegas: no gasta memoria el resto del tiempo."
    }

    // --- Cargando ---
    Esqueleto {
        Layout.fillWidth: true
        Layout.preferredHeight: 180
        visible: !tienda.androidCargado
    }

    // --- Aviso de NVIDIA ---
    Rectangle {
        Layout.fillWidth: true
        visible: tienda.androidCargado && raiz.est.nvidia === true
        implicitHeight: filaNv.implicitHeight + 32
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.borde
        RowLayout {
            id: filaNv
            anchors.fill: parent
            anchors.margins: 16
            spacing: 14
            Icono { Layout.preferredWidth: 22; Layout.preferredHeight: 22; nombre: "info"; color: Tema.suave; trazo: 1.8 }
            Text {
                Layout.fillWidth: true
                text: "Tu tarjeta es NVIDIA: Android todavía no puede usarla, así que dibuja con el procesador. Los juegos ligeros (Subway Surfers, Clash Royale, Among Us…) van bien; los muy pesados pueden ir lentos."
                color: Tema.suave
                font.pixelSize: 13
                lineHeight: 1.2
                wrapMode: Text.WordWrap
            }
        }
    }

    // --- Sin preparar ---
    Rectangle {
        Layout.fillWidth: true
        visible: tienda.androidCargado && !raiz.listo
        implicitHeight: colPrep.implicitHeight + 56
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.borde

        ColumnLayout {
            id: colPrep
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 28
            spacing: 18

            RowLayout {
                spacing: 18
                Rectangle {
                    Layout.preferredWidth: 64
                    Layout.preferredHeight: 64
                    radius: 18
                    color: Tema.realce
                    Icono { anchors.centerIn: parent; width: 32; height: 32; nombre: "android"; color: Tema.texto; trazo: 1.6 }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text {
                        text: raiz.preparando ? "Preparando Android…" : "Preparar Android"
                        color: Tema.texto
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                    }
                    Text {
                        Layout.fillWidth: true
                        text: raiz.preparando ? tienda.androidPaso
                            : "Se hace una sola vez: baja Android con la Play Store (unos 1,5 GB) y lo deja listo. Te pide tu contraseña al empezar."
                        color: Tema.suave
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                    }
                }
            }

            // Barra de progreso
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 6
                visible: raiz.preparando
                radius: 3
                color: Tema.realce
                Rectangle {
                    width: parent.width * Math.max(0.03, tienda.androidPct / 100)
                    height: parent.height
                    radius: 3
                    color: Tema.azul
                    Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                }
            }

            Boton {
                visible: !raiz.preparando
                text: "Preparar Android"
                grande: true
                enabled: !raiz.ocupado
                onClicked: tienda.prepararAndroid()
            }
        }
    }

    // --- Listo: buscar en la Play Store ---
    RowLayout {
        Layout.fillWidth: true
        visible: raiz.listo
        spacing: 12

        TextField {
            id: buscador
            Layout.fillWidth: true
            Layout.maximumWidth: 720
            Layout.preferredHeight: 48
            leftPadding: 50
            rightPadding: 18
            placeholderText: "Busca un juego en la Play Store…"
            placeholderTextColor: Tema.tenue
            color: Tema.texto
            selectionColor: Tema.bordeHover
            selectedTextColor: Tema.texto
            font.pixelSize: 16
            verticalAlignment: TextInput.AlignVCenter
            inputMethodHints: Qt.ImhNoPredictiveText
            enabled: !raiz.ocupado
            background: Rectangle {
                radius: 24
                color: Tema.campo
                border.width: buscador.activeFocus ? 2 : 1
                border.color: buscador.activeFocus ? Tema.azul : Tema.bordeCampo
            }
            Icono {
                x: 18
                anchors.verticalCenter: parent.verticalCenter
                width: 20
                height: 20
                nombre: "buscar"
                color: buscador.activeFocus ? Tema.azul : Tema.tenue
            }
            onAccepted: if (text.trim() !== "") tienda.androidBuscar(text)
        }
        Boton {
            text: "Abrir Play Store"
            tipo: "secundario"
            grande: true
            enabled: !raiz.ocupado
            onClicked: tienda.androidTienda("")
        }
    }

    // Lo que está pasando (abrir Android tarda unos segundos la primera vez)
    Text {
        Layout.fillWidth: true
        visible: raiz.listo && raiz.ocupado
        text: tienda.androidPaso + "  La primera vez Android tarda unos segundos en prender."
        color: Tema.suave
        font.pixelSize: 13
        wrapMode: Text.WordWrap
        SequentialAnimation on opacity {
            running: raiz.listo && raiz.ocupado
            loops: Animation.Infinite
            NumberAnimation { from: 1; to: 0.45; duration: 700; easing.type: Easing.InOutSine }
            NumberAnimation { from: 0.45; to: 1; duration: 700; easing.type: Easing.InOutSine }
        }
    }

    // --- Paso único: registrar en Google ---
    Rectangle {
        Layout.fillWidth: true
        visible: raiz.listo && raiz.est.registrado !== true
        implicitHeight: colReg.implicitHeight + 40
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.bordeHover

        ColumnLayout {
            id: colReg
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 20
            spacing: 12
            Text {
                text: "Un paso más para la Play Store"
                color: Tema.texto
                font.pixelSize: 16
                font.weight: Font.DemiBold
            }
            Text {
                Layout.fillWidth: true
                text: "Google pide registrar este Android una vez. 1) Abre la Play Store e inicia sesión. 2) Pide tu número aquí. 3) Pégalo en la página de Google y dale a «Registrar». Puede tardar unos minutos en funcionar."
                color: Tema.suave
                font.pixelSize: 13
                lineHeight: 1.25
                wrapMode: Text.WordWrap
            }
            RowLayout {
                visible: tienda.androidId !== ""
                spacing: 10
                Text {
                    text: tienda.androidId
                    color: Tema.texto
                    font.pixelSize: 18
                    font.family: "monospace"
                    font.weight: Font.DemiBold
                }
                Boton {
                    text: "Copiar"
                    tipo: "fantasma"
                    onClicked: { copiador.text = tienda.androidId; copiador.selectAll(); copiador.copy(); tienda.aviso("Número copiado.") }
                }
                TextEdit { id: copiador; visible: false }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 10
                Boton { text: tienda.androidId === "" ? "Pedir mi número" : "Volver a pedir"; tipo: "secundario"; enabled: !raiz.ocupado; onClicked: tienda.androidPedirId() }
                Boton { text: "Abrir la página de Google"; tipo: "secundario"; onClicked: tienda.androidAbrirRegistro() }
                Boton { text: "Ya lo registré"; enabled: !raiz.ocupado; onClicked: tienda.androidYaRegistre() }
            }
        }
    }

    // --- Tus juegos ---
    Seccion {
        Layout.fillWidth: true
        visible: raiz.listo && tienda.androidApps.length > 0
        titulo: "Tus juegos de Android"

        Repeater {
            model: tienda.androidApps
            Rectangle {
                required property var modelData
                Layout.fillWidth: true
                implicitHeight: 72
                radius: Tema.radioTarjeta
                color: zonaJ.containsMouse ? Tema.tarjetaHover : Tema.tarjeta
                border.width: 1
                border.color: zonaJ.containsMouse ? Tema.bordeHover : Tema.borde
                Behavior on color { ColorAnimation { duration: 120 } }
                MouseArea { id: zonaJ; anchors.fill: parent; hoverEnabled: true }
                IconoApp {
                    id: iconoJ
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    tam: 44
                    url: modelData.icono
                    nombre: modelData.nombre
                }
                Text {
                    anchors.left: iconoJ.right
                    anchors.leftMargin: 14
                    anchors.right: botonesJ.left
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.nombre
                    color: Tema.texto
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }
                Row {
                    id: botonesJ
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8
                    Boton { text: "Quitar"; tipo: "peligro"; enabled: !raiz.ocupado; onClicked: tienda.androidQuitar(modelData.paquete, modelData.nombre) }
                    Boton { text: "Jugar"; enabled: !raiz.ocupado; onClicked: tienda.androidAbrir(modelData.paquete) }
                }
            }
        }
    }

    // --- Populares ---
    Seccion {
        Layout.fillWidth: true
        visible: tienda.androidCargado
        titulo: "Populares que van bien"
        subtitulo: "Los juegos con anti-trampas muy estrictos (como algunos shooters en línea) a veces no dejan jugar en PC."

        GridLayout {
            Layout.fillWidth: true
            columns: Math.max(1, Math.floor(width / 300))
            columnSpacing: 12
            rowSpacing: 12
            Repeater {
                model: tienda.androidPopulares
                Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: 76
                    radius: Tema.radioTarjeta
                    color: zonaP.containsMouse ? Tema.tarjetaHover : Tema.tarjeta
                    border.width: 1
                    border.color: zonaP.containsMouse ? Tema.bordeHover : Tema.borde
                    Behavior on color { ColorAnimation { duration: 120 } }
                    MouseArea {
                        id: zonaP
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: raiz.listo ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: if (raiz.listo && !raiz.ocupado) tienda.androidTienda(modelData.paquete)
                    }
                    IconoApp {
                        id: iconoP
                        x: 14
                        anchors.verticalCenter: parent.verticalCenter
                        tam: 46
                        nombre: modelData.nombre
                    }
                    Column {
                        anchors.left: iconoP.right
                        anchors.leftMargin: 14
                        anchors.right: flechaP.left
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2
                        Text { width: parent.width; text: modelData.nombre; color: Tema.texto; font.pixelSize: 15; font.weight: Font.DemiBold; elide: Text.ElideRight }
                        Text { width: parent.width; text: modelData.genero; color: Tema.suave; font.pixelSize: 13; elide: Text.ElideRight }
                    }
                    Icono {
                        id: flechaP
                        anchors.right: parent.right
                        anchors.rightMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        width: 18
                        height: 18
                        visible: raiz.listo
                        nombre: "derecha"
                        color: zonaP.containsMouse ? Tema.texto : Tema.tenue
                    }
                }
            }
        }
    }
}
