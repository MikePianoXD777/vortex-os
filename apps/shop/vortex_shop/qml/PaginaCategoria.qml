// Una categoría: sus apps en cuadrícula, con «Mostrar más» para las siguientes páginas. Las categorías
// de Vortexine's (IA) traen un texto de arriba y, a veces, un comando para la terminal que se puede copiar.
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property string estado: tienda.categoriaEstado
    readonly property var cat: tienda.categoriaActual

    Seccion {
        Layout.fillWidth: true
        etiqueta: raiz.cat.destacada === true ? "Vortexine's" : "Categoría"
        titulo: raiz.cat.nombre ? raiz.cat.nombre : ""
        tamTitulo: 28
        subtitulo: raiz.estado === "listo" && tienda.categoriaApps.length > 0
                   ? tienda.categoriaApps.length + (tienda.categoriaApps.length === 1 ? " app" : " apps") + (tienda.categoriaHayMas ? " y más" : "")
                   : ""

        Rectangle {   // texto de arriba (solo las categorías que lo traen)
            Layout.fillWidth: true
            visible: (raiz.cat.intro ? raiz.cat.intro : "") !== ""
            // Sin Layouts por dentro: texto que se ajusta + alto que manda en el padre = bucle de polish().
            implicitHeight: intro.implicitHeight + 40
            radius: Tema.radioTarjeta
            border.width: 1
            border.color: Tema.borde
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: Tema.realce }
                GradientStop { position: 1; color: Tema.tarjeta }
            }
            Column {
                id: intro
                x: 24
                y: 20
                width: parent.width - 48
                spacing: 12
                Text {
                    width: parent.width
                    text: raiz.cat.intro ? raiz.cat.intro : ""
                    color: Tema.texto
                    font.pixelSize: 15
                    wrapMode: Text.WordWrap
                }
                Row {
                    visible: (raiz.cat.comando ? raiz.cat.comando : "") !== ""
                    spacing: 10
                    Rectangle {
                        width: comando.implicitWidth + 28
                        height: 38
                        radius: 12
                        color: Tema.campo
                        border.width: 1
                        border.color: Tema.bordeCampo
                        TextEdit {
                            id: comando
                            anchors.centerIn: parent
                            text: raiz.cat.comando ? raiz.cat.comando : ""
                            readOnly: true
                            selectByMouse: true
                            color: Tema.azul
                            selectionColor: Tema.bordeHover
                            font.family: "monospace"
                            font.pixelSize: 15
                        }
                    }
                    Boton {
                        id: copiar
                        property bool copiado: false
                        anchors.verticalCenter: parent.verticalCenter
                        text: copiado ? "Copiado" : "Copiar"
                        icono: copiado ? "visto" : ""
                        tipo: "secundario"
                        onClicked: {
                            comando.selectAll()
                            comando.copy()
                            comando.deselect()
                            copiado = true
                            volver.restart()
                        }
                        Timer { id: volver; interval: 2000; onTriggered: copiar.copiado = false }
                    }
                }
            }
        }

        Rejilla {
            Layout.fillWidth: true
            visible: raiz.estado === "cargando" || (raiz.estado === "listo" && tienda.categoriaApps.length > 0)
            cargando: raiz.estado === "cargando"
            huecos: 9
            apps: tienda.categoriaApps
        }
        Boton {
            Layout.alignment: Qt.AlignHCenter
            visible: raiz.estado === "listo" && tienda.categoriaHayMas
            enabled: !tienda.categoriaCargandoMas
            text: tienda.categoriaCargandoMas ? "Cargando…" : "Mostrar más"
            tipo: "secundario"
            grande: true
            onClicked: tienda.masDeCategoria()
        }
        EstadoPantalla {
            Layout.fillWidth: true
            visible: raiz.estado === "listo" && tienda.categoriaApps.length === 0
            icono: "categorias"
            titulo: "Esta categoría está vacía por ahora"
            texto: "Prueba con otra categoría o busca por nombre."
            boton: "Ir al inicio"
            onAccion: Nav.seccion("inicio", "")
        }
        EstadoPantalla {
            Layout.fillWidth: true
            visible: raiz.estado === "sinred" || raiz.estado === "error"
            icono: raiz.estado === "sinred" ? "sinred" : "alerta"
            titulo: raiz.estado === "sinred" ? "Sin conexión a internet" : "No pudimos cargar esta categoría"
            texto: raiz.estado === "sinred" ? "Revisa tu red y vuelve a intentarlo."
                                            : "Algo salió mal. Vuelve a intentarlo en un momento."
            boton: "Reintentar"
            onAccion: tienda.reintentarCategoria()
        }
    }
}
