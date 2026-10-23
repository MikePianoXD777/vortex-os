// Resultados de la búsqueda.
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property string estado: tienda.busquedaEstado
    readonly property int cuantas: tienda.busquedaApps.length

    Seccion {
        Layout.fillWidth: true
        etiqueta: "Búsqueda"
        titulo: "Resultados para “" + tienda.busquedaTexto + "”"
        tamTitulo: 26
        subtitulo: raiz.estado === "listo" && raiz.cuantas > 0 ? raiz.cuantas + (raiz.cuantas === 1 ? " app" : " apps") : ""

        Rejilla {
            Layout.fillWidth: true
            visible: raiz.estado === "cargando" || (raiz.estado === "listo" && raiz.cuantas > 0)
            cargando: raiz.estado === "cargando"
            huecos: 6
            apps: tienda.busquedaApps
        }
        EstadoPantalla {
            Layout.fillWidth: true
            visible: raiz.estado === "listo" && raiz.cuantas === 0
            icono: "buscar"
            titulo: "No encontramos apps con “" + tienda.busquedaTexto + "”"
            texto: "Revisa cómo está escrito o prueba con otra palabra, por ejemplo el tipo de app («editor», «música»)."
        }
        EstadoPantalla {
            Layout.fillWidth: true
            visible: raiz.estado === "sinred" || raiz.estado === "error"
            icono: raiz.estado === "sinred" ? "sinred" : "alerta"
            titulo: raiz.estado === "sinred" ? "Sin conexión a internet" : "No pudimos buscar"
            texto: raiz.estado === "sinred" ? "Revisa tu red y vuelve a intentarlo."
                                            : "Algo salió mal. Vuelve a intentarlo en un momento."
            boton: "Reintentar"
            onAccion: tienda.reintentarBusqueda()
        }
    }
}
