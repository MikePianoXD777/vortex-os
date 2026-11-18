// Rendimiento: a la izquierda cada parte de la PC con su gráfica chiquita (CPU, memoria, cada disco,
// cada red, cada tarjeta de video); a la derecha la elegida en grande, con sus datos. La CPU se puede
// ver por núcleo.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: pagina
    property bool activa: false
    property string elegido: "cpu"
    property bool porNucleo: false
    readonly property var r: tm.rendimiento
    readonly property var h: tm.historia
    readonly property bool listo: r.cpu !== undefined

    function _hd(grupo, id, clave) { return h[grupo] && h[grupo][id] ? h[grupo][id][clave] : [] }

    // Cada parte de la PC, para la lista de la izquierda.
    readonly property var partes: {
        if (!listo) return []
        var l = [
            { clave: "cpu", titulo: "CPU", icono: "cpu", tinta: Tema.azul, serie: h.cpu, maximo: 100,
              valor: Fmt.pct(r.cpu.uso), detalle: Fmt.dec(r.cpu.ghz, 2) + " GHz" + (r.cpu.temp ? " · " + Math.round(r.cpu.temp) + " °C" : "") },
            { clave: "mem", titulo: "Memoria", icono: "memoria", tinta: Tema.violeta, serie: h.mem, maximo: 100,
              valor: Fmt.pct(r.mem.uso), detalle: Fmt.bytes(r.mem.usada) + " de " + Fmt.bytes(r.mem.total) }
        ]
        for (var i = 0; i < r.discos.length; i++) {
            var d = r.discos[i]
            l.push({ clave: "disco:" + d.id, titulo: "Disco " + i + " (" + d.id + ")", icono: "disco", tinta: Tema.exito,
                     serie: _hd("discos", d.id, "activo"), maximo: 100, valor: Fmt.pct(d.activo), detalle: d.tipo + " · " + Fmt.bytes(d.capacidad) })
        }
        for (var j = 0; j < r.redes.length; j++) {
            var n = r.redes[j]
            l.push({ clave: "red:" + n.id, titulo: n.tipo, icono: n.tipo === "Wi-Fi" ? "red" : "ethernet", tinta: Tema.azul,
                     serie: _hd("redes", n.id, "recepcion"), serie2: _hd("redes", n.id, "envio"), maximo: 0,
                     valor: "↓ " + Fmt.bits(n.recepcion), detalle: "↑ " + Fmt.bits(n.envio) })
        }
        for (var k = 0; k < r.gpus.length; k++) {
            var g = r.gpus[k]
            l.push({ clave: "gpu:" + g.id, titulo: "GPU " + k, icono: "gpu", tinta: Tema.exito,
                     serie: _hd("gpus", g.id, "uso"), maximo: 100, valor: g.uso >= 0 ? Fmt.pct(g.uso) : "",
                     detalle: g.nombre })
        }
        return l
    }

    // Lo que se ve a la derecha.
    readonly property var det: {
        if (!listo) return null
        var p = elegido.split(":"), tipo = p[0], id = p.slice(1).join(":")
        if (tipo === "cpu") {
            return { titulo: "CPU", modelo: r.cpu.nombre, tinta: Tema.azul, serie: h.cpu, maximo: 100, etiqueta: "% de uso en 60 segundos",
                     grandes: [["Uso", Fmt.pct(r.cpu.uso)], ["Velocidad", Fmt.dec(r.cpu.ghz, 2) + " GHz"],
                               ["Temperatura", r.cpu.temp ? Math.round(r.cpu.temp) + " °C" : "—"],
                               ["Procesos", r.cpu.procesos], ["Hilos", r.cpu.hilos], ["Encendida", Fmt.tiempo(r.cpu.encendido)]],
                     datos: [["Velocidad máxima", r.cpu.maxGhz ? Fmt.dec(r.cpu.maxGhz, 2) + " GHz" : "—"],
                             ["Núcleos", r.cpu.fisicos], ["Procesadores lógicos", r.cpu.logicos]] }
        }
        if (tipo === "mem") {
            return { titulo: "Memoria", modelo: Fmt.bytes(r.mem.total), tinta: Tema.violeta, serie: h.mem, maximo: 100, etiqueta: "% en uso en 60 segundos",
                     grandes: [["En uso", Fmt.bytes(r.mem.usada)], ["Disponible", Fmt.bytes(r.mem.disponible)],
                               ["En caché", Fmt.bytes(r.mem.cache)],
                               ["Swap", r.mem.swapTotal ? Fmt.bytes(r.mem.swapUsada) + " de " + Fmt.bytes(r.mem.swapTotal) : "Sin swap"]],
                     datos: [], memoria: true }
        }
        if (tipo === "disco") {
            var d = r.discos.find(x => x.id === id)
            if (!d) return null
            return { titulo: "Disco (" + d.id + ")", modelo: d.modelo, tinta: Tema.exito, serie: _hd("discos", id, "activo"), maximo: 100,
                     etiqueta: "Tiempo activo en 60 segundos",
                     extra: { serie: _hd("discos", id, "lectura"), serie2: _hd("discos", id, "escritura"), etiqueta: "Lectura y escritura (punteada)" },
                     grandes: [["Tiempo activo", Fmt.pct(d.activo)], ["Lectura", Fmt.mbs(d.lectura)], ["Escritura", Fmt.mbs(d.escritura)]],
                     datos: [["Tipo", d.tipo], ["Capacidad", Fmt.bytes(d.capacidad)],
                             ["Usado", d.usado ? Fmt.bytes(d.usado) + " de " + Fmt.bytes(d.total) : "Sin montar"]] }
        }
        if (tipo === "red") {
            var n = r.redes.find(x => x.id === id)
            if (!n) return null
            return { titulo: n.tipo, modelo: n.id, tinta: Tema.azul, serie: _hd("redes", id, "recepcion"), serie2: _hd("redes", id, "envio"),
                     maximo: 0, etiqueta: "Recepción y envío (punteada) en 60 segundos",
                     grandes: [["Recepción", Fmt.bits(n.recepcion)], ["Envío", Fmt.bits(n.envio)]],
                     datos: [["Dirección IPv4", n.ipv4 || "—"], ["Red", n.ssid || "—"],
                             ["Velocidad del enlace", n.velocidad ? n.velocidad + " Mbps" : "—"]].filter(x => x[1] !== "—" || x[0] !== "Red") }
        }
        if (tipo === "gpu") {
            var g = r.gpus.find(x => x.id === id)
            if (!g) return null
            return { titulo: "GPU", modelo: g.nombre, tinta: Tema.exito, serie: _hd("gpus", id, "uso"), maximo: 100, etiqueta: "% de uso en 60 segundos",
                     extra: g.vramTotal ? { serie: _hd("gpus", id, "vram"), maximo: 100, etiqueta: "Memoria de video (% en uso)" } : null,
                     grandes: [["Uso", g.uso >= 0 ? Fmt.pct(g.uso) : "—"], ["Memoria de video", g.vramTotal ? Fmt.bytes(g.vramUsada) + " de " + Fmt.bytes(g.vramTotal) : "—"],
                               ["Temperatura", g.temp ? Math.round(g.temp) + " °C" : "—"], ["Consumo", g.watts ? Math.round(g.watts) + " W" : "—"]],
                     datos: [] }
        }
        return null
    }
    onDetChanged: if (listo && det === null) elegido = "cpu"   // el disco o la red que estaba elegida ya no está

    component Parte: Rectangle {
        id: parte
        property var p
        readonly property bool elegida: pagina.elegido === p.clave
        Layout.fillWidth: true
        implicitHeight: 74
        radius: 16
        color: elegida ? Tema.barraSel : zonaParte.containsMouse ? Tema.realce : "transparent"
        border.width: elegida ? 1 : 0
        border.color: Tema.bordeHover
        Behavior on color { ColorAnimation { duration: 120 } }
        RowLayout {
            anchors.fill: parent
            anchors.margins: 10
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 78
                Layout.fillHeight: true
                radius: 8
                color: Tema.fondo
                border.width: 1
                border.color: Qt.rgba(parte.p.tinta.r, parte.p.tinta.g, parte.p.tinta.b, 0.5)
                clip: true
                Grafica {
                    anchors.fill: parent
                    anchors.margins: 1
                    valores: parte.p.serie || []
                    valores2: parte.p.serie2 || []
                    maximo: parte.p.maximo
                    color: parte.p.tinta
                    rejilla: false
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1
                Text { Layout.fillWidth: true; text: parte.p.titulo; color: Tema.texto; font.pixelSize: 14; font.weight: Font.DemiBold; elide: Text.ElideRight }
                Text { Layout.fillWidth: true; text: parte.p.valor; color: Tema.suave; font.pixelSize: 12; font.features: { "tnum": 1 }; elide: Text.ElideRight; visible: text !== "" }
                Text { Layout.fillWidth: true; text: parte.p.detalle; color: Tema.tenue; font.pixelSize: 11; elide: Text.ElideRight }
            }
        }
        MouseArea { id: zonaParte; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: pagina.elegido = parte.p.clave }
    }

    component Dato: ColumnLayout {
        property string t: ""
        property string v: ""
        property bool grande: true
        spacing: 2
        Text { text: parent.t; color: Tema.tenue; font.pixelSize: 12 }
        Text { text: parent.v; color: Tema.texto; font.pixelSize: parent.grande ? 22 : 14; font.weight: parent.grande ? Font.DemiBold : Font.Normal; font.features: { "tnum": 1 } }
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 24
        anchors.topMargin: 20
        spacing: 20

        ColumnLayout {
            Layout.fillWidth: false
            Layout.preferredWidth: 270
            Layout.fillHeight: true
            spacing: 14
            Text { text: "Rendimiento"; color: Tema.texto; font.pixelSize: 26; font.weight: Font.Bold }
            Flickable {
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentHeight: columnaPartes.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ColumnLayout {
                    id: columnaPartes
                    width: parent.width
                    spacing: 4
                    Repeater {
                        model: pagina.partes
                        Parte { required property var modelData; p: modelData }
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            visible: pagina.det !== null

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 24
                spacing: 14

                RowLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: false
                    spacing: 12
                    Text { text: pagina.det ? pagina.det.titulo : ""; color: Tema.texto; font.pixelSize: 24; font.weight: Font.Bold }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignRight
                        text: pagina.det ? pagina.det.modelo : ""
                        color: Tema.suave
                        font.pixelSize: 15
                        elide: Text.ElideLeft
                    }
                    Rectangle {   // Gráfica / Por núcleo
                        visible: pagina.elegido === "cpu"
                        implicitWidth: filaVista.implicitWidth + 8
                        implicitHeight: 34
                        radius: 17
                        color: Tema.campo
                        Row {
                            id: filaVista
                            anchors.centerIn: parent
                            spacing: 2
                            Repeater {
                                model: [["Total", "grafica", false], ["Por núcleo", "nucleos", true]]
                                Rectangle {
                                    required property var modelData
                                    readonly property bool sel: pagina.porNucleo === modelData[2]
                                    width: filaBoton.implicitWidth + 24
                                    height: 28
                                    radius: 14
                                    color: sel ? Tema.azul : zonaVista.containsMouse ? Tema.realce : "transparent"
                                    Row {
                                        id: filaBoton
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Icono { width: 14; height: 14; nombre: modelData[1]; color: sel ? Tema.sobreAcento : Tema.suave; anchors.verticalCenter: parent.verticalCenter }
                                        Text { text: modelData[0]; color: sel ? Tema.sobreAcento : Tema.suave; font.pixelSize: 12; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
                                    }
                                    MouseArea { id: zonaVista; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: pagina.porNucleo = modelData[2] }
                                }
                            }
                        }
                    }
                }

                Text { text: pagina.det ? pagina.det.etiqueta : ""; color: Tema.tenue; font.pixelSize: 12 }

                // Gráfica grande (o una por núcleo)
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumHeight: 160
                    Rectangle {
                        anchors.fill: parent
                        visible: !(pagina.elegido === "cpu" && pagina.porNucleo)
                        radius: 14
                        color: Tema.fondo
                        border.width: 1
                        border.color: pagina.det ? Qt.rgba(pagina.det.tinta.r, pagina.det.tinta.g, pagina.det.tinta.b, 0.45) : Tema.borde
                        clip: true
                        Grafica {
                            anchors.fill: parent
                            anchors.margins: 1
                            valores: pagina.det ? pagina.det.serie || [] : []
                            valores2: pagina.det ? pagina.det.serie2 || [] : []
                            maximo: pagina.det ? pagina.det.maximo : 100
                            color: pagina.det ? pagina.det.tinta : Tema.azul
                        }
                        Text {   // la escala de arriba, cuando se ajusta sola
                            visible: pagina.det && pagina.det.maximo === 0
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 8
                            text: pagina.det && pagina.det.maximo === 0 ? Fmt.bits(Math.max(1, Math.max.apply(null, (pagina.det.serie || [0]).concat(pagina.det.serie2 || [0]))) * 1.2) : ""
                            color: Tema.tenue
                            font.pixelSize: 11
                        }
                    }
                    GridLayout {
                        anchors.fill: parent
                        visible: pagina.elegido === "cpu" && pagina.porNucleo
                        readonly property int n: pagina.listo ? pagina.r.cpu.nucleos.length : 1
                        columns: Math.ceil(Math.sqrt(n * width / Math.max(1, height)))
                        rowSpacing: 6
                        columnSpacing: 6
                        Repeater {
                            model: pagina.listo && pagina.porNucleo ? pagina.r.cpu.nucleos.length : 0
                            Rectangle {
                                required property int index
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                radius: 8
                                color: Tema.fondo
                                border.width: 1
                                border.color: Qt.rgba(Tema.azul.r, Tema.azul.g, Tema.azul.b, 0.35)
                                clip: true
                                Grafica {
                                    anchors.fill: parent
                                    anchors.margins: 1
                                    rejilla: false
                                    valores: pagina.h.nucleos[index] || []
                                }
                                Text {
                                    anchors.left: parent.left; anchors.top: parent.top; anchors.margins: 6
                                    text: index + "  ·  " + Fmt.pct(pagina.r.cpu.nucleos[index] || 0)
                                    color: Tema.tenue
                                    font.pixelSize: 10
                                }
                            }
                        }
                    }
                }

                // Segunda gráfica (lectura/escritura del disco, memoria de video)
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 110
                    visible: !!(pagina.det && pagina.det.extra)
                    radius: 14
                    color: Tema.fondo
                    border.width: 1
                    border.color: Tema.borde
                    clip: true
                    Grafica {
                        anchors.fill: parent
                        anchors.margins: 1
                        valores: pagina.det && pagina.det.extra ? pagina.det.extra.serie || [] : []
                        valores2: pagina.det && pagina.det.extra ? pagina.det.extra.serie2 || [] : []
                        maximo: pagina.det && pagina.det.extra && pagina.det.extra.maximo ? pagina.det.extra.maximo : 0
                        color: Tema.azul
                    }
                    Text {
                        anchors.left: parent.left; anchors.top: parent.top; anchors.margins: 8
                        text: pagina.det && pagina.det.extra ? pagina.det.extra.etiqueta : ""
                        color: Tema.tenue
                        font.pixelSize: 11
                    }
                }

                // Cómo se reparte la memoria
                ColumnLayout {
                    id: reparto
                    Layout.fillWidth: true
                    Layout.fillHeight: false
                    visible: !!(pagina.det && pagina.det.memoria)
                    spacing: 6
                    readonly property real total: pagina.listo ? pagina.r.mem.total : 1
                    readonly property real usada: pagina.listo ? pagina.r.mem.usada : 0
                    readonly property real cache: pagina.listo ? Math.min(pagina.r.mem.cache, total - usada) : 0
                    Text { text: "Cómo se reparte"; color: Tema.tenue; font.pixelSize: 12 }
                    Rectangle {
                        id: barraMem
                        Layout.fillWidth: true
                        implicitHeight: 14
                        radius: 7
                        color: Tema.relleno
                        clip: true
                        Row {
                            anchors.fill: parent
                            Rectangle { width: barraMem.width * reparto.usada / reparto.total; height: parent.height; color: Tema.violeta }
                            Rectangle { width: barraMem.width * reparto.cache / reparto.total; height: parent.height; color: Qt.rgba(Tema.violeta.r, Tema.violeta.g, Tema.violeta.b, 0.4) }
                        }
                    }
                    Row {
                        spacing: 18
                        Repeater {
                            model: [["En uso", Tema.violeta], ["En caché (se suelta si hace falta)", Qt.rgba(Tema.violeta.r, Tema.violeta.g, Tema.violeta.b, 0.4)], ["Libre", Tema.relleno]]
                            Row {
                                required property var modelData
                                spacing: 6
                                Rectangle { width: 10; height: 10; radius: 3; color: modelData[1]; anchors.verticalCenter: parent.verticalCenter }
                                Text { text: modelData[0]; color: Tema.suave; font.pixelSize: 12 }
                            }
                        }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: 36
                    Repeater {
                        model: pagina.det ? pagina.det.grandes : []
                        Dato { required property var modelData; t: modelData[0]; v: modelData[1] }
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 36
                    visible: pagina.det && pagina.det.datos.length > 0
                    Repeater {
                        model: pagina.det ? pagina.det.datos : []
                        Dato { required property var modelData; t: modelData[0]; v: modelData[1]; grande: false }
                    }
                }
            }
        }
    }
}
