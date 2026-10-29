// 2.1.1: escuchar 30 s de una canción de YouTube Music antes de bajarla (otra vez, lo para).
// Mientras YouTube da el audio (1-2 s), el ícono gira; la fila pinta el título del acento mientras suena.
import QtQuick

BotonIcono {
    id: raiz
    property string vid: ""
    readonly property bool mio: vid !== "" && vp.avance.idActual === vid
    readonly property bool cargando: mio && !vp.avance.sonando
    tam: 30
    icono: cargando ? "girar" : mio ? "pausa" : "play"
    ayuda: mio ? "Parar" : "Escuchar 30 s"
    onCargandoChanged: if (!cargando) rotation = 0
    RotationAnimation on rotation {
        running: raiz.cargando
        from: 0
        to: 360
        duration: 900
        loops: Animation.Infinite
    }
}
