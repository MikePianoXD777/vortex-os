// Lo que se puede hacer con una app, en el mismo sitio: Instalar / Abrir / Quitar / Actualizar,
// o su barra de progreso (con su turno en la cola) mientras se trabaja en ella.
// modo: «tarjeta» | «detalle» | «instalada» (fila de Instaladas) | «actualizacion» (fila de Actualizaciones)
import QtQuick

Item {
    id: raiz
    property string appId: ""
    property string nombre: ""
    property string modo: "tarjeta"

    readonly property var est: tienda.estados[appId]
    readonly property bool enCurso: est !== undefined && est.fase !== "abriendo"
    readonly property bool abriendo: est !== undefined && est.fase === "abriendo"
    readonly property bool instalada: tienda.instaladasIds[appId] === true
    readonly property bool hayAct: tienda.actualizablesIds[appId] === true
    readonly property bool delSistema: tienda.sistemaIds[appId] === true
    readonly property bool grande: modo === "detalle"
    readonly property int pct: enCurso ? est.pct : 0
    readonly property bool enCola: enCurso && est.fase === "cola"
    readonly property string verbo: !enCurso ? "" : est.fase === "cola" ? "En cola"
        : est.fase === "instalando" ? "Instalando" : est.fase === "quitando" ? "Quitando" : "Actualizando"

    implicitWidth: enCurso ? 220 : botones.implicitWidth
    implicitHeight: enCurso ? progreso.implicitHeight : botones.implicitHeight

    Row {
        id: botones
        visible: !raiz.enCurso
        spacing: 8
        Boton {
            visible: !raiz.instalada && raiz.modo !== "instalada"
            text: "Instalar"
            icono: "descargar"
            grande: raiz.grande
            onClicked: tienda.instalar(raiz.appId)
        }
        Boton {
            visible: raiz.instalada && raiz.modo !== "actualizacion"
            enabled: !raiz.abriendo
            text: raiz.abriendo ? "Abriendo…" : "Abrir"
            icono: "abrir"
            grande: raiz.grande
            onClicked: tienda.abrir(raiz.appId)
        }
        Boton {
            visible: (raiz.instalada && raiz.hayAct && raiz.modo !== "instalada") || raiz.modo === "actualizacion"
            text: "Actualizar"
            icono: "actualizaciones"
            tipo: raiz.modo === "actualizacion" ? "primario" : "secundario"
            grande: raiz.grande
            onClicked: tienda.actualizar(raiz.appId)
        }
        Boton {
            visible: raiz.instalada && (raiz.modo === "detalle" || raiz.modo === "instalada")
            enabled: !raiz.delSistema
            text: raiz.delSistema ? "Instalada por el sistema" : "Quitar"
            icono: raiz.delSistema ? "" : "papelera"
            tipo: "peligro"
            grande: raiz.grande
            onClicked: Nav.quitarPedido(raiz.appId, raiz.nombre)
        }
    }

    Column {
        id: progreso
        visible: raiz.enCurso
        width: parent.width
        spacing: 6
        Row {
            width: parent.width
            spacing: 10
            Item {
                id: pista
                width: parent.width - etiqueta.implicitWidth - parent.spacing
                height: raiz.grande ? 10 : 8
                anchors.verticalCenter: parent.verticalCenter
                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Tema.borde
                }
                Rectangle {   // avance real
                    visible: raiz.pct > 0
                    height: parent.height
                    width: Math.max(height, parent.width * raiz.pct / 100)
                    radius: height / 2
                    color: Tema.azul
                    Behavior on width { NumberAnimation { duration: 180 } }
                }
                Rectangle {   // sin porcentaje todavía (preparando o en cola): bloque que va y viene
                    id: bloque
                    visible: raiz.pct <= 0
                    height: parent.height
                    width: parent.width * 0.3
                    radius: height / 2
                    color: raiz.enCola ? Tema.bordeHover : Tema.azul
                    SequentialAnimation on x {
                        running: bloque.visible && !raiz.enCola
                        loops: Animation.Infinite
                        NumberAnimation { from: 0; to: pista.width * 0.7; duration: 900; easing.type: Easing.InOutSine }
                        NumberAnimation { from: pista.width * 0.7; to: 0; duration: 900; easing.type: Easing.InOutSine }
                    }
                }
            }
            Text {
                id: etiqueta
                anchors.verticalCenter: parent.verticalCenter
                text: raiz.enCola ? "En cola" : raiz.pct > 0 ? raiz.pct + " %" : "…"
                color: Tema.texto
                font.pixelSize: raiz.grande ? 14 : 12
                font.weight: Font.DemiBold
            }
        }
        Text {
            width: parent.width
            text: raiz.enCola ? "Espera su turno (de una en una)"
                  : (raiz.est !== undefined && raiz.est.texto ? raiz.est.texto : raiz.verbo + "…")
            color: Tema.tenue
            font.pixelSize: raiz.grande ? 13 : 11
            elide: Text.ElideRight
        }
    }
}
