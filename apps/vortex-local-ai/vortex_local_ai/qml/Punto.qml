// El puntito de estado de Ollama: verde si corre, del acento latiendo si arranca, rojo si no.
import QtQuick

Rectangle {
    id: raiz
    property string estado: "detenido"
    width: 9; height: 9; radius: 4.5
    color: estado === "corriendo" ? Tema.exito : estado === "arrancando" ? Tema.azul : Tema.peligro
    SequentialAnimation on opacity {
        running: raiz.estado === "arrancando"
        loops: Animation.Infinite
        alwaysRunToEnd: true
        NumberAnimation { from: 1; to: 0.3; duration: 600 }
        NumberAnimation { from: 0.3; to: 1; duration: 600 }
    }
}
