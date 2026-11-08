// El grafo del vault, como el de Obsidian: cada nota es un punto y cada [[enlace]] una línea. Física
// sencilla (se repelen, los enlaces jalan, todo tiende al centro) que se detiene sola cuando se calma.
// Arrastra un punto para moverlo, el fondo para moverte, la rueda para acercarte; un clic abre la nota.
import QtQuick

Item {
    id: raiz
    property var datos: ({ "nodos": [], "aristas": [] })
    property string actual: ""          // la ruta de la nota abierta: sale resaltada
    signal abrirNodo(string id, string titulo, bool existe)

    property var pos: []                // [{x, y, vx, vy}] en coordenadas del mundo
    property var vecinos: []            // índice → lista de índices
    property real zoom: 1
    property real despX: 0
    property real despY: 0
    property int sobre: -1
    property int arrastrado: -1
    property real energia: 0
    readonly property int cuantos: datos && datos.nodos ? datos.nodos.length : 0

    onDatosChanged: reiniciar()
    onVisibleChanged: if (visible && cuantos > 0) { energia = 1; reloj.start() }
    onActualChanged: lienzo.requestPaint()

    function radio(i) {
        var g = raiz.datos.nodos[i].grado || 0
        return 4 + Math.min(10, Math.sqrt(g) * 2.2)
    }

    function reiniciar() {
        var viejas = {}
        var nodosViejos = raiz._ids || []
        for (var k = 0; k < nodosViejos.length && k < pos.length; k++)
            viejas[nodosViejos[k]] = pos[k]
        var nodos = datos && datos.nodos ? datos.nodos : []
        var nuevas = []
        var ids = []
        for (var i = 0; i < nodos.length; i++) {
            ids.push(nodos[i].id)
            var v = viejas[nodos[i].id]
            if (v) nuevas.push({ x: v.x, y: v.y, vx: 0, vy: 0 })
            else {
                var ang = i * 2.399963   // ángulo dorado: arrancan repartidos en espiral
                var r = 30 * Math.sqrt(i + 1)
                nuevas.push({ x: Math.cos(ang) * r, y: Math.sin(ang) * r, vx: 0, vy: 0 })
            }
        }
        var vec = []
        for (i = 0; i < nodos.length; i++) vec.push([])
        var aristas = datos && datos.aristas ? datos.aristas : []
        for (i = 0; i < aristas.length; i++) {
            var a = aristas[i][0], b = aristas[i][1]
            if (a < nodos.length && b < nodos.length) { vec[a].push(b); vec[b].push(a) }
        }
        _ids = ids
        pos = nuevas
        vecinos = vec
        sobre = -1
        arrastrado = -1
        energia = 1
        if (nodos.length > 0) reloj.start()
        lienzo.requestPaint()
    }
    property var _ids: []

    function paso() {
        var n = pos.length
        if (n === 0) { reloj.stop(); return }
        var aristas = datos.aristas || []
        var fx = new Array(n).fill(0), fy = new Array(n).fill(0)
        var repulsion = 2400, largo = 70, resorte = 0.04, gravedad = 0.012
        for (var i = 0; i < n; i++) {
            var pi = pos[i]
            for (var j = i + 1; j < n; j++) {
                var dx = pi.x - pos[j].x, dy = pi.y - pos[j].y
                var d2 = dx * dx + dy * dy + 0.01
                if (d2 > 250000) continue          // lejos: no se empujan
                var f = repulsion / d2
                var d = Math.sqrt(d2)
                fx[i] += f * dx / d; fy[i] += f * dy / d
                fx[j] -= f * dx / d; fy[j] -= f * dy / d
            }
            fx[i] -= pi.x * gravedad
            fy[i] -= pi.y * gravedad
        }
        for (var k = 0; k < aristas.length; k++) {
            var a = aristas[k][0], b = aristas[k][1]
            if (a >= n || b >= n) continue
            var ex = pos[b].x - pos[a].x, ey = pos[b].y - pos[a].y
            var dist = Math.sqrt(ex * ex + ey * ey) + 0.01
            var t = (dist - largo) * resorte
            fx[a] += t * ex / dist; fy[a] += t * ey / dist
            fx[b] -= t * ex / dist; fy[b] -= t * ey / dist
        }
        var e = 0
        for (i = 0; i < n; i++) {
            if (i === arrastrado) continue
            var p = pos[i]
            p.vx = (p.vx + fx[i]) * 0.82
            p.vy = (p.vy + fy[i]) * 0.82
            var vel = Math.sqrt(p.vx * p.vx + p.vy * p.vy)
            if (vel > 25) { p.vx *= 25 / vel; p.vy *= 25 / vel }
            p.x += p.vx
            p.y += p.vy
            e += p.vx * p.vx + p.vy * p.vy
        }
        energia = e / n
        if (energia < 0.004 && arrastrado < 0) reloj.stop()
        lienzo.requestPaint()
    }

    function aPantalla(p) { return { x: p.x * zoom + width / 2 + despX, y: p.y * zoom + height / 2 + despY } }
    function aMundo(x, y) { return { x: (x - width / 2 - despX) / zoom, y: (y - height / 2 - despY) / zoom } }
    function nodoEn(x, y) {
        var m = aMundo(x, y)
        var mejor = -1, mejorD = 1e9
        for (var i = 0; i < pos.length; i++) {
            var dx = pos[i].x - m.x, dy = pos[i].y - m.y
            var d = Math.sqrt(dx * dx + dy * dy)
            var limite = radio(i) + 6 / zoom
            if (d < limite && d < mejorD) { mejor = i; mejorD = d }
        }
        return mejor
    }

    Timer {
        id: reloj
        interval: raiz.cuantos > 400 ? 40 : 16
        repeat: true
        running: false
        onTriggered: raiz.paso()
    }

    Canvas {
        id: lienzo
        anchors.fill: parent
        renderStrategy: Canvas.Cooperative
        onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            var nodos = raiz.datos.nodos || []
            var aristas = raiz.datos.aristas || []
            var n = Math.min(nodos.length, raiz.pos.length)
            if (n === 0) return
            var foco = raiz.sobre >= 0 ? raiz.sobre : -1
            var cerca = {}
            if (foco >= 0) { cerca[foco] = true; var vs = raiz.vecinos[foco] || []; for (var v = 0; v < vs.length; v++) cerca[vs[v]] = true }
            // líneas
            ctx.lineWidth = Math.max(0.6, 1 * raiz.zoom)
            for (var k = 0; k < aristas.length; k++) {
                var a = aristas[k][0], b = aristas[k][1]
                if (a >= n || b >= n) continue
                var pa = raiz.aPantalla(raiz.pos[a]), pb = raiz.aPantalla(raiz.pos[b])
                var resaltada = foco >= 0 && (a === foco || b === foco)
                ctx.globalAlpha = foco >= 0 && !resaltada ? 0.12 : resaltada ? 0.95 : 0.45
                ctx.strokeStyle = resaltada ? Tema.azul : Tema.tenue
                ctx.beginPath()
                ctx.moveTo(pa.x, pa.y)
                ctx.lineTo(pb.x, pb.y)
                ctx.stroke()
            }
            // puntos
            for (var i = 0; i < n; i++) {
                var p = raiz.aPantalla(raiz.pos[i])
                var r = raiz.radio(i) * Math.max(0.6, raiz.zoom)
                var esActual = nodos[i].id === raiz.actual
                ctx.globalAlpha = foco >= 0 && !cerca[i] ? 0.25 : 1
                ctx.fillStyle = esActual ? Tema.violeta : i === foco ? Tema.azul : nodos[i].existe ? Tema.suave : Tema.borde
                ctx.beginPath()
                ctx.arc(p.x, p.y, r, 0, Math.PI * 2)
                ctx.fill()
                if (esActual) {
                    ctx.strokeStyle = Tema.violeta
                    ctx.lineWidth = 2
                    ctx.beginPath()
                    ctx.arc(p.x, p.y, r + 4, 0, Math.PI * 2)
                    ctx.stroke()
                }
            }
            // nombres: de cerca, o el que tocas y sus vecinos
            ctx.font = "12px sans-serif"
            ctx.textAlign = "center"
            for (i = 0; i < n; i++) {
                if (!(raiz.zoom > 0.85 || cerca[i] || nodos[i].id === raiz.actual)) continue
                p = raiz.aPantalla(raiz.pos[i])
                ctx.globalAlpha = foco >= 0 && !cerca[i] ? 0.25 : 1
                ctx.fillStyle = i === foco ? Tema.texto : Tema.tenue
                ctx.fillText(nodos[i].titulo, p.x, p.y + raiz.radio(i) * Math.max(0.6, raiz.zoom) + 15)
            }
            ctx.globalAlpha = 1
        }
    }

    MouseArea {
        id: zona
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        property real inicioX: 0
        property real inicioY: 0
        property real ultimoX: 0
        property real ultimoY: 0
        property bool movio: false
        cursorShape: raiz.sobre >= 0 ? Qt.PointingHandCursor : pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
        onPositionChanged: (m) => {
            if (pressed) {
                if (Math.abs(m.x - inicioX) + Math.abs(m.y - inicioY) > 4) movio = true
                if (raiz.arrastrado >= 0) {
                    var w = raiz.aMundo(m.x, m.y)
                    raiz.pos[raiz.arrastrado].x = w.x
                    raiz.pos[raiz.arrastrado].y = w.y
                    raiz.energia = 1
                    reloj.start()
                } else {
                    raiz.despX += m.x - ultimoX
                    raiz.despY += m.y - ultimoY
                }
                ultimoX = m.x; ultimoY = m.y
                lienzo.requestPaint()
            } else {
                var s = raiz.nodoEn(m.x, m.y)
                if (s !== raiz.sobre) { raiz.sobre = s; lienzo.requestPaint() }
            }
        }
        onPressed: (m) => {
            inicioX = ultimoX = m.x
            inicioY = ultimoY = m.y
            movio = false
            raiz.arrastrado = raiz.nodoEn(m.x, m.y)
        }
        onReleased: (m) => {
            var i = raiz.arrastrado
            raiz.arrastrado = -1
            if (!movio && i >= 0) {
                var nodo = raiz.datos.nodos[i]
                raiz.abrirNodo(nodo.id, nodo.titulo, nodo.existe)
            }
        }
        onExited: if (raiz.sobre >= 0) { raiz.sobre = -1; lienzo.requestPaint() }
        onWheel: (r) => {
            var antes = raiz.aMundo(r.x, r.y)
            raiz.zoom = Math.max(0.2, Math.min(4, raiz.zoom * (r.angleDelta.y > 0 ? 1.12 : 1 / 1.12)))
            var despues = raiz.aPantalla(antes)
            raiz.despX += r.x - despues.x
            raiz.despY += r.y - despues.y
            lienzo.requestPaint()
        }
    }

    Text {
        anchors.centerIn: parent
        visible: raiz.cuantos === 0
        text: "Tu grafo sale aquí cuando tengas notas enlazadas con [[…]]."
        color: Tema.tenue
        font.pixelSize: 13
    }
}
