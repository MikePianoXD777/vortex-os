// Una categoría: sus apps en cuadrícula, con «Mostrar más» para las siguientes páginas.
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property string estado: tienda.categoriaEstado
    readonly property var cat: tienda.categoriaActual

    Seccion {
        Layout.fillWidth: true
        etiqueta: "Categoría"
        titulo: raiz.cat.nombre ? raiz.cat.nombre : ""
        tamTitulo: 28
        subtitulo: raiz.estado === "listo" && tienda.categoriaApps.length > 0
                   ? tienda.categoriaApps.length + (tienda.categoriaApps.length === 1 ? " app" : " apps") + (tienda.categoriaHayMas ? " y más" : "")
                   : ""

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
