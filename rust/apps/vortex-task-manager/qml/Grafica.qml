// Gráfica de área de los últimos 60 segundos (la más nueva a la derecha, como en Windows). `valores2`
// es una segunda serie opcional (escritura, envío…) en línea punteada. Con `maximo: 0` la escala se
// ajusta sola a lo más alto que se ve.
import QtQuick

Canvas {
    id: raiz
    property var valores: []
    property var valores2: []
    property real maximo: 100
    property color color: Tema.azul
    property color color2: Tema.violeta
    property bool rejilla: true
    property int puntos: 60
    readonly property real tope: {
        if (maximo > 0) return maximo
        var m = 0
        for (var i = 0; i < valores.length; i++) m = Math.max(m, valores[i])
        for (var j = 0; j < valores2.length; j++) m = Math.max(m, valores2[j])
        return Math.max(1, m * 1.2)
    }

    onValoresChanged: requestPaint()
    onValores2Changed: requestPaint()
    onColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    function _serie(ctx, vals, col, relleno, punteada) {
        if (!vals || vals.length < 2) return
        var w = width, h = height, paso = w / (puntos - 1)
        var x0 = w - (vals.length - 1) * paso
        ctx.beginPath()
        for (var i = 0; i < vals.length; i++) {
            var x = x0 + i * paso
            var y = h - Math.min(1, Math.max(0, vals[i] / tope)) * (h - 2) - 1
            if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y)
        }
        ctx.lineWidth = punteada ? 1.5 : 2
        ctx.strokeStyle = col
        ctx.setLineDash(punteada ? [4, 3] : [])
        ctx.stroke()
        if (relleno) {
            ctx.lineTo(w, h)
            ctx.lineTo(x0, h)
            ctx.closePath()
            var g = ctx.createLinearGradient(0, 0, 0, h)
            g.addColorStop(0, Qt.rgba(col.r, col.g, col.b, 0.38))
            g.addColorStop(1, Qt.rgba(col.r, col.g, col.b, 0.03))
            ctx.fillStyle = g
            ctx.fill()
        }
    }

    onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        if (rejilla) {
            ctx.strokeStyle = Qt.rgba(Tema.borde.r, Tema.borde.g, Tema.borde.b, 0.7)
            ctx.lineWidth = 1
            ctx.setLineDash([])
            ctx.beginPath()
            for (var f = 1; f < 4; f++) {
                var y = Math.round(height * f / 4) + 0.5
                ctx.moveTo(0, y); ctx.lineTo(width, y)
            }
            for (var c = 1; c < 6; c++) {
                var x = Math.round(width * c / 6) + 0.5
                ctx.moveTo(x, 0); ctx.lineTo(x, height)
            }
            ctx.stroke()
        }
        _serie(ctx, valores, color, true, false)
        _serie(ctx, valores2, color2, false, true)
    }
}
