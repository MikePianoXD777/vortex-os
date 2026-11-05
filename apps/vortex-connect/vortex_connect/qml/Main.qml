// La ventana de Vortexine Connect: arriba cómo vas (protegido o no), luego el modo viaje y las redes abiertas cerca.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    width: 500
    height: 720
    minimumWidth: 420
    minimumHeight: 600
    visible: true
    title: "Vortexine Connect"
    color: Tema.fondo

    readonly property var aspecto: ({
        "protegido": { "titulo": "Protegido", "detalle": "Lo que mandas va cifrado hasta Cloudflare.", "icono": "escudoBien", "color": Tema.exito },
        "solo_dns": { "titulo": "Solo DNS cifrado", "detalle": "Esta red bloquea lo demás. Evita entrar a tu banco aquí.", "icono": "escudoRoto", "color": Tema.violeta },
        "sin_proteccion": { "titulo": "Sin protección", "detalle": "Esta red es abierta: cualquiera cerca podría ver lo que no vaya cifrado.", "icono": "escudoRoto", "color": Tema.peligro },
        "portal": { "titulo": "La red pide aceptar algo", "detalle": "Ábrela para tener internet. Lo que escribas ahí no va cifrado.", "icono": "abrir", "color": Tema.violeta },
        "confianza": { "titulo": "Tu red", "detalle": "Esta red tiene contraseña o es por cable: no hace falta cifrar.", "icono": "escudo", "color": Tema.azul },
        "sin_red": { "titulo": "Sin internet", "detalle": vc.hayWifi ? "Elige una red abierta de abajo." : "Este equipo no tiene wifi.", "icono": "sinWifi", "color": Tema.tenue }
    })
    // Mientras entra a una red, lo de antes ya no aplica: «Tu red» con un portal abierto confunde.
    readonly property var trabajando: ({ "titulo": "Un momento…", "detalle": "", "icono": "wifi", "color": Tema.azul })
    readonly property var actual: vc.ocupado ? trabajando : (aspecto[vc.estado] || aspecto["sin_red"])
    property bool pidiendoCondiciones: false
    property var alAceptar: null

    // Nada se cifra sin que la persona haya aceptado las condiciones de Cloudflare aquí.
    function conCondiciones(accion) {
        if (vc.condiciones) { accion(); return }
        alAceptar = accion
        pidiendoCondiciones = true
    }

    component Tarjeta: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
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
    component Enlace: Text {
        signal pulsado()
        color: Tema.azul
        font.pixelSize: 13
        font.underline: true
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: parent.pulsado() }
    }

    Connections {
        target: vc
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
    }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }
    onActiveChanged: if (active) vc.actualizar()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 14

        // --- Cómo vas ---
        Tarjeta {
            Layout.fillWidth: true
            Layout.preferredHeight: columnaEstado.implicitHeight + 40
            ColumnLayout {
                id: columnaEstado
                anchors.fill: parent
                anchors.margins: 20
                spacing: 10
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 16
                    Rectangle {
                        Layout.preferredWidth: 64
                        Layout.preferredHeight: 64
                        radius: 32
                        color: Tema.campo
                        Icono { anchors.centerIn: parent; width: 36; height: 36; nombre: ventana.actual.icono; color: ventana.actual.color }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Text {
                            objectName: "tituloEstado"
                            Layout.fillWidth: true
                            text: ventana.actual.titulo
                            color: ventana.actual.color
                            font.pixelSize: 22
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }
                        Text {
                            objectName: "nombreRed"
                            Layout.fillWidth: true
                            visible: vc.tipoRed !== ""
                            text: vc.tipoRed === "cable" ? "Por cable" : (vc.redActual || "Wifi")
                            color: Tema.suave
                            font.pixelSize: 13
                            elide: Text.ElideMiddle
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: ventana.actual.detalle
                    color: Tema.tenue
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Text {
                    objectName: "mensaje"
                    Layout.fillWidth: true
                    visible: vc.mensaje !== ""
                    text: vc.mensaje
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Text {
                    objectName: "avisoTailscale"
                    Layout.fillWidth: true
                    visible: vc.hayTailscale && (vc.estado === "protegido" || vc.estado === "sin_proteccion" || vc.estado === "confianza")
                    text: "Mientras esté cifrado, Tailscale no conecta: lo que lo usa, como Shared a distancia, espera hasta que quites la protección."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    visible: !vc.warpInstalado
                    text: "Falta el cliente de Cloudflare WARP: sin él no se puede cifrar."
                    color: Tema.peligro
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Boton {
                        visible: !vc.warpInstalado && vc.esWindows
                        text: "Bajar WARP de Cloudflare"
                        icono: "abrir"
                        principal: true
                        onClicked: vc.bajarWarp()
                    }
                    Boton {
                        objectName: "botonPortal"
                        visible: vc.portal !== "" && (vc.estado === "portal" || vc.ocupado)
                        text: "Abrir la página"
                        icono: "abrir"
                        principal: true
                        onClicked: vc.abrirPortal()
                    }
                    Boton {
                        objectName: "botonProteger"
                        visible: !vc.ocupado && (vc.estado === "sin_proteccion" || vc.estado === "confianza" || vc.estado === "portal")
                        enabled: vc.warpInstalado
                        text: "Cifrar mi conexión"
                        icono: "escudo"
                        principal: vc.estado === "sin_proteccion"
                        onClicked: ventana.conCondiciones(function () { vc.proteger() })
                    }
                    Boton {
                        objectName: "botonQuitar"
                        visible: !vc.ocupado && (vc.estado === "protegido" || vc.estado === "solo_dns")
                        text: "Quitar protección"
                        onClicked: vc.quitarProteccion()
                    }
                    Boton {
                        objectName: "botonCancelar"
                        visible: vc.ocupado && vc.portal !== ""
                        text: "Cancelar"
                        onClicked: vc.cancelar()
                    }
                    BusyIndicator {
                        visible: vc.ocupado
                        running: vc.ocupado
                        Layout.preferredWidth: 28
                        Layout.preferredHeight: 28
                    }
                    Item { Layout.fillWidth: true }
                }
            }
        }

        // --- Modo viaje ---
        Tarjeta {
            Layout.fillWidth: true
            Layout.preferredHeight: filaViaje.implicitHeight + 32
            RowLayout {
                id: filaViaje
                anchors.fill: parent
                anchors.margins: 16
                spacing: 12
                Icono { Layout.preferredWidth: 24; Layout.preferredHeight: 24; nombre: "viaje"; color: Tema.azul }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text { text: "Modo viaje"; color: Tema.texto; font.pixelSize: 15; font.weight: Font.DemiBold }
                    Text {
                        Layout.fillWidth: true
                        text: "Sin internet, entra sola a la red abierta con mejor señal y la cifra. Una red abierta puede ser falsa: déjalo apagado si no lo necesitas."
                        color: Tema.tenue
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                    }
                }
                AbstractButton {
                    id: interruptor
                    objectName: "interruptorViaje"
                    implicitWidth: 44
                    implicitHeight: 24
                    checkable: true
                    checked: vc.modoViaje
                    enabled: vc.warpInstalado
                    opacity: enabled ? 1 : 0.45
                    onClicked: {
                        var quiere = checked
                        checked = Qt.binding(function () { return vc.modoViaje })
                        if (quiere) ventana.conCondiciones(function () { vc.cambiarModoViaje(true) })
                        else vc.cambiarModoViaje(false)
                    }
                    background: Rectangle {
                        radius: 12
                        color: interruptor.checked ? Tema.azul : Tema.relleno
                        border.width: 1
                        border.color: interruptor.checked ? Tema.azul : Tema.bordeCampo
                        Rectangle {
                            width: 18; height: 18; radius: 9
                            y: 3
                            x: interruptor.checked ? parent.width - width - 3 : 3
                            color: interruptor.checked ? Tema.sobreAcento : Tema.suave
                            Behavior on x { NumberAnimation { duration: 120 } }
                        }
                    }
                }
            }
        }

        // --- Redes abiertas ---
        Tarjeta {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 8
                RowLayout {
                    Layout.fillWidth: true
                    Text { Layout.fillWidth: true; text: "Redes abiertas cerca"; color: Tema.texto; font.pixelSize: 15; font.weight: Font.DemiBold }
                    Boton {
                        objectName: "botonBuscar"
                        text: vc.buscando ? "Buscando…" : "Buscar"
                        icono: "recargar"
                        enabled: !vc.buscando && !vc.ocupado
                        implicitHeight: 34
                        onClicked: vc.buscar()
                    }
                }
                ListView {
                    id: lista
                    objectName: "listaRedes"
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: vc.redes
                    delegate: Rectangle {
                        required property var modelData
                        width: ListView.view.width
                        height: 50
                        radius: 14
                        color: Tema.campo
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 8
                            spacing: 10
                            Icono {
                                Layout.preferredWidth: 20
                                Layout.preferredHeight: 20
                                nombre: "wifi"
                                color: modelData.senal >= 40 ? Tema.suave : Tema.tenue
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                Text { Layout.fillWidth: true; text: modelData.ssid; color: Tema.texto; font.pixelSize: 14; elide: Text.ElideRight }
                                Text { text: modelData.conectada ? "Estás aquí" : "Señal " + modelData.senal + " %"; color: Tema.tenue; font.pixelSize: 11 }
                            }
                            Boton {
                                visible: !modelData.conectada
                                enabled: !vc.ocupado
                                implicitHeight: 34
                                text: "Conectar"
                                onClicked: {
                                    var red = modelData.ssid
                                    ventana.conCondiciones(function () { vc.conectar(red) })
                                }
                            }
                        }
                    }
                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 32
                        visible: lista.count === 0
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        text: vc.buscando ? "Buscando…" : (vc.hayWifi ? "No hay redes abiertas cerca." : "Este equipo no tiene wifi.")
                        color: Tema.tenue
                        font.pixelSize: 13
                    }
                }
            }
        }
    }

    // --- Las condiciones de Cloudflare: las acepta la persona, una vez ---
    Rectangle {
        objectName: "veloCondiciones"
        anchors.fill: parent
        visible: ventana.pidiendoCondiciones
        color: Tema.velo
        MouseArea { anchors.fill: parent }
        Tarjeta {
            anchors.centerIn: parent
            width: Math.min(parent.width - 48, 420)
            height: columnaCondiciones.implicitHeight + 44
            ColumnLayout {
                id: columnaCondiciones
                anchors.fill: parent
                anchors.margins: 22
                spacing: 10
                Text { text: "Antes de cifrar"; color: Tema.texto; font.pixelSize: 18; font.weight: Font.DemiBold }
                Text {
                    Layout.fillWidth: true
                    text: "Connect cifra con Cloudflare WARP, gratis y sin cuenta. Tu tráfico pasa por los servidores de Cloudflare: la red donde estás ya no lo ve, pero Cloudflare recibe la conexión. No oculta lo que haces como una VPN de pago.\n\nPara usarlo hay que aceptar las condiciones de Cloudflare."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Row {
                    spacing: 16
                    Enlace { text: "Condiciones"; onPulsado: vc.verCondiciones() }
                    Enlace { text: "Privacidad"; onPulsado: vc.verPrivacidad() }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    spacing: 8
                    Item { Layout.fillWidth: true }
                    Boton { text: "Ahora no"; onClicked: ventana.pidiendoCondiciones = false }
                    Boton {
                        objectName: "botonAceptar"
                        text: "Aceptar y seguir"
                        principal: true
                        onClicked: {
                            ventana.pidiendoCondiciones = false
                            vc.aceptarCondiciones()
                            if (ventana.alAceptar) ventana.alAceptar()
                            ventana.alAceptar = null
                        }
                    }
                }
            }
        }
    }
}
