// Redondea las esquinas de lo que haya debajo pintando encima, en el color del fondo, el «negativo»
// de cada esquina. Así no hace falta ningún efecto de shader (que no existe con el render por
// software) para tener capturas con esquinas redondeadas.
import QtQuick

Canvas {
    id: raiz
    property color relleno: Tema.tarjeta
    property real radio: 16
    renderTarget: Canvas.Image
    onRellenoChanged: requestPaint()
    onRadioChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        ctx.fillStyle = relleno
        var r = radio
        function esquina() {
            ctx.beginPath()
            ctx.moveTo(0, 0)
            ctx.lineTo(r, 0)
            ctx.arc(r, r, r, -Math.PI / 2, Math.PI, true)
            ctx.closePath()
            ctx.fill()
        }
        esquina()
        ctx.save(); ctx.translate(width, 0); ctx.scale(-1, 1); esquina(); ctx.restore()
        ctx.save(); ctx.translate(0, height); ctx.scale(1, -1); esquina(); ctx.restore()
        ctx.save(); ctx.translate(width, height); ctx.scale(-1, -1); esquina(); ctx.restore()
    }
}
