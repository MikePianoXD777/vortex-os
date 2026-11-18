// «Configurar»: el nombre, qué Proton usa, los argumentos y el .exe de un programa.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Popup {
    id: raiz
    property var programa: null

    function abrir(p) {
        programa = p
        nombre.text = p.nombre
        argumentos.text = p.argumentos
        var lista = runner.protones
        proton.currentIndex = 0
        for (var i = 0; i < lista.length; i++)
            if (lista[i].id === p.proton) proton.currentIndex = i
        open()
        nombre.forceActiveFocus()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 520
    modal: true
    focus: true
    padding: 26
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Overlay.modal: Rectangle { color: Tema.velo }
    background: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120 } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 } }

    FileDialog {
        id: elegirExe
        title: "Elige el .exe del programa"
        nameFilters: ["Programas de Windows (*.exe *.EXE *.msi *.bat)", "Todos los archivos (*)"]
        onAccepted: if (raiz.programa) runner.cambiarExe(raiz.programa.id, selectedFile.toString())
    }

    contentItem: ColumnLayout {
        spacing: 8
        Text {
            text: "Configurar"
            color: Tema.texto
            font.pixelSize: 18
            font.weight: Font.Bold
        }
        Text { Layout.topMargin: 8; text: "Nombre"; color: Tema.suave; font.pixelSize: 12; font.weight: Font.DemiBold }
        CampoTexto { id: nombre; Layout.fillWidth: true; onAccepted: guardar.clicked() }

        Text { Layout.topMargin: 8; text: "Proton"; color: Tema.suave; font.pixelSize: 12; font.weight: Font.DemiBold }
        Selector {
            id: proton
            Layout.fillWidth: true
            model: runner.protones
            textRole: "nombre"
        }
        Text {
            Layout.fillWidth: true
            text: "El Proton de CachyOS va bien con casi todo. Si algo no abre, prueba GE-Proton: trae arreglos para muchos juegos (se baja solo la primera vez)."
            color: Tema.tenue
            font.pixelSize: 12
            wrapMode: Text.Wrap
        }

        Text { Layout.topMargin: 8; text: "Argumentos (opcional)"; color: Tema.suave; font.pixelSize: 12; font.weight: Font.DemiBold }
        CampoTexto { id: argumentos; Layout.fillWidth: true; placeholderText: "Por ejemplo: -windowed -dx11"; onAccepted: guardar.clicked() }

        Text { Layout.topMargin: 8; text: "Programa"; color: Tema.suave; font.pixelSize: 12; font.weight: Font.DemiBold }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Text {
                Layout.fillWidth: true
                text: raiz.programa ? raiz.programa.exe : ""
                color: Tema.tenue
                font.pixelSize: 12
                elide: Text.ElideMiddle
            }
            Boton { tipo: "secundario"; text: "Cambiar"; onClicked: elegirExe.open() }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 14
            spacing: 10
            Item { Layout.fillWidth: true }
            Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: raiz.close() }
            Boton {
                id: guardar
                text: "Guardar"
                onClicked: {
                    var lista = runner.protones
                    runner.guardar(raiz.programa.id, nombre.text, lista[proton.currentIndex].id, argumentos.text)
                    raiz.close()
                }
            }
        }
    }
}
