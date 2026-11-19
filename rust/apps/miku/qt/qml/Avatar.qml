// La ventana con avatar: una página local (three.js + three-vrm) en QtWebEngine, sin borde y transparente.
// La página habla con el asistente por QWebChannel (objeto «puente» = mk) y el asistente con la página por
// runJavaScript. Solo se carga cuando el 3D está encendido: sin avatar no entra QtWebEngine (ahorra RAM).
import QtQuick
import QtWebEngine
import QtWebChannel

Window {
    id: w

    readonly property real s: mk.escala

    objectName: "ventanaAvatar"
    title: "Miku"
    width: Math.round(460 * s)
    height: Math.round(760 * s)
    minimumWidth: width
    maximumWidth: width
    minimumHeight: height
    maximumHeight: height
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"
    visible: true

    WebEngineProfile {
        id: perfil
        offTheRecord: true      // en memoria: no deja caché, cookies ni historial en el disco
    }

    WebChannel {
        id: canal
        Component.onCompleted: registerObjects({"puente": mk})
    }

    WebEngineView {
        id: web
        objectName: "web"
        anchors.fill: parent
        profile: perfil
        webChannel: canal
        backgroundColor: "transparent"
        zoomFactor: w.s     // la página se arma para 460x760: con otra escala, todo crece o se encoge igual
        url: mk.pagina
        settings.localContentCanAccessFileUrls: true     // imports y VRM por file://
        settings.localContentCanAccessRemoteUrls: false
        settings.javascriptCanOpenWindows: false
        settings.errorPageEnabled: false
        settings.pluginsEnabled: false
        settings.showScrollBars: false
        settings.webGLEnabled: true

        // Solo deja abrir archivos locales: la página es nuestra y nunca sale a internet.
        onNavigationRequested: function (request) {
            if (request.url.toString().indexOf("file:") !== 0)
                request.action = WebEngineNavigationRequest.IgnoreRequest
        }
        onContextMenuRequested: function (request) { request.accepted = true }
        onJavaScriptConsoleMessage: function (nivel, mensaje, linea, origen) {
            if (nivel !== WebEngineView.InfoMessageLevel)
                console.warn("miku[js]: " + mensaje + " (" + origen.split("/").pop() + ":" + linea + ")")
        }
    }

    Connections {
        target: mk
        function onJs(codigo) { web.runJavaScript(codigo) }
    }
}
