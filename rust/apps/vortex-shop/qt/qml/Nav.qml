pragma Singleton
// Señales de navegación: cualquier componente pide ir a un sitio y Main.qml lo hace.
import QtQuick

QtObject {
    signal detalle(string appId)
    signal seccion(string nombre, string arg)   // «inicio», «categoria» (arg = id), «instaladas», «actualizaciones»
    signal atras()
    signal quitarPedido(string appId, string nombre)
}
