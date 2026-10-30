// Agregar o editar una cámara: nombre, número de serie y código de seguridad (los dos vienen en la
// etiqueta), y la IP solo si hace falta (la de tu red se busca sola; la de otra casa por Tailscale se
// escribe). «Buscar en mi red» enseña las cámaras que contestaron, para elegir una con un clic.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: raiz
    property string camId: ""          // vacío: agregar
    property bool masOpciones: false
    readonly property bool editando: camId !== ""

    function abrir(id) {
        camId = id || ""
        var c = editando ? cam.camara(camId) : {}
        nombre.text = c.nombre || ""
        serie.text = c.serie || ""
        codigo.text = ""
        codigo.mostrar = false
        ip.text = c.ip || ""
        puerto.text = String(c.puerto || 554)
        canal.text = String(c.canal || 1)
        error.text = ""
        masOpciones = editando && ((c.puerto && c.puerto !== 554) || (c.canal && c.canal !== 1))
        open()
        nombre.campo.forceActiveFocus()
    }
    function guardar() {
        var datos = { nombre: nombre.text, serie: serie.text, codigo: codigo.text, ip: ip.text,
                      puerto: parseInt(puerto.text) || 554, canal: parseInt(canal.text) || 1 }
        var e = editando ? cam.editarCamara(camId, datos) : cam.agregarCamara(datos)
        if (e) {
            error.text = e
            return
        }
        close()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(560, parent.width - 48)
    height: Math.min(contenido.implicitHeight + 56, parent.height - 48)
    modal: true
    focus: true
    padding: 28
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Overlay.modal: Rectangle { color: Tema.velo }
    background: Rectangle { radius: Tema.radioGrande; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 140 } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 } }

    Flickable {
        anchors.fill: parent
        contentHeight: contenido.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        ColumnLayout {
            id: contenido
            width: parent.width
            spacing: 16

            ColumnLayout {
                spacing: 4
                Text {
                    text: raiz.editando ? "Editar cámara" : "Agregar cámara"
                    color: Tema.texto
                    font.pixelSize: 22
                    font.weight: Font.DemiBold
                }
                Text {
                    Layout.fillWidth: true
                    text: "El número de serie (SN) y el código de seguridad (Safety Code) vienen en la etiqueta "
                        + "de la cámara o en su caja."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                }
            }

            Entrada {
                id: nombre
                Layout.fillWidth: true
                etiqueta: "Nombre"
                placeholderText: "Patio, Cochera, Calle…"
                onAceptado: raiz.guardar()
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Entrada {
                    id: serie
                    Layout.fillWidth: true
                    etiqueta: "Número de serie (SN)"
                    placeholderText: "9A0B1C2D3E4F5"
                    onAceptado: raiz.guardar()
                }
                Entrada {
                    id: codigo
                    Layout.fillWidth: true
                    etiqueta: "Código de seguridad"
                    placeholderText: raiz.editando ? "Igual que antes" : "L2C3D4E5"
                    secreto: true
                    onAceptado: raiz.guardar()
                }
            }
            Entrada {
                id: ip
                Layout.fillWidth: true
                etiqueta: "IP (si no está en tu red)"
                placeholderText: "Déjalo vacío y la busco yo"
                ayuda: "Para una cámara de otra casa conectada por Tailscale, escribe aquí su IP (la de esa casa, "
                     + "como 192.168.0.30)."
                onAceptado: raiz.guardar()
            }

            Text {
                text: raiz.masOpciones ? "Menos opciones" : "Más opciones"
                color: Tema.azul
                font.pixelSize: 12
                font.weight: Font.DemiBold
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: raiz.masOpciones = !raiz.masOpciones }
            }
            RowLayout {
                visible: raiz.masOpciones
                Layout.fillWidth: true
                spacing: 12
                Entrada {
                    id: puerto
                    Layout.preferredWidth: 140
                    etiqueta: "Puerto RTSP"
                    validator: IntValidator { bottom: 1; top: 65535 }
                }
                Entrada {
                    id: canal
                    Layout.preferredWidth: 140
                    etiqueta: "Canal"
                    ayuda: "1, salvo en grabadores (NVR)"
                    validator: IntValidator { bottom: 1; top: 64 }
                }
                Item { Layout.fillWidth: true }
            }

            // --- buscar en la red ---
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: red.implicitHeight + 28
                radius: Tema.radioTarjeta - 6
                color: Tema.realce
                border.width: 1
                border.color: Tema.borde
                ColumnLayout {
                    id: red
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 10
                    RowLayout {
                        Layout.fillWidth: true
                        Icono { width: 18; height: 18; nombre: "red"; color: Tema.azul }
                        Text {
                            Layout.fillWidth: true
                            text: cam.buscando ? "Buscando cámaras en tu red…"
                                : cam.encontradas.length ? cam.encontradas.length + (cam.encontradas.length === 1 ? " cámara en tu red" : " cámaras en tu red")
                                : "¿No sabes su número de serie? Búscala en tu red."
                            color: Tema.texto
                            font.pixelSize: 13
                            wrapMode: Text.Wrap
                        }
                        Boton {
                            tipo: "secundario"
                            icono: "buscar"
                            text: cam.buscando ? "Buscando…" : "Buscar"
                            enabled: !cam.buscando
                            onClicked: cam.buscarEnRed()
                        }
                    }
                    Repeater {
                        model: cam.encontradas
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 10
                            Icono { width: 18; height: 18; nombre: "camara"; color: Tema.suave }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                Text {
                                    text: modelData.modelo || "Cámara"
                                    color: Tema.texto
                                    font.pixelSize: 13
                                    font.weight: Font.DemiBold
                                }
                                Text {
                                    text: (modelData.serie ? "SN " + modelData.serie + " · " : "") + modelData.ip
                                    color: Tema.tenue
                                    font.pixelSize: 12
                                }
                            }
                            Boton {
                                tipo: modelData.agregada ? "fantasma" : "secundario"
                                text: modelData.agregada ? "Ya está" : "Usar"
                                enabled: !modelData.agregada
                                onClicked: {
                                    if (modelData.serie) serie.text = modelData.serie
                                    ip.text = modelData.ip
                                    if (!nombre.text) nombre.text = modelData.modelo || "Cámara"
                                    codigo.campo.forceActiveFocus()
                                }
                            }
                        }
                    }
                }
            }

            Text {
                id: error
                Layout.fillWidth: true
                visible: text !== ""
                color: Tema.peligro
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                Item { Layout.fillWidth: true }
                Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: raiz.close() }
                Boton { icono: raiz.editando ? "visto" : "mas"; text: raiz.editando ? "Guardar" : "Agregar"; onClicked: raiz.guardar() }
            }
        }
    }
}
