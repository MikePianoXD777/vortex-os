// Vault: tus notas en Markdown, como Obsidian. A la izquierda el árbol (o lo que buscas) y la memoria de
// la IA; al centro el editor con vista previa (o el grafo); a la derecha los retroenlaces y las etiquetas.
// Escribe [[ para enlazar otra nota; en el editor, Ctrl+clic en un enlace lo abre. Se guarda solo.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: raiz
    objectName: "paginaVault"
    property bool activa: false
    property string modo: "editar"          // editar | vista | lado
    property bool verGrafo: false
    readonly property bool hayNota: boveda.nota !== ""

    // Lo que se edita es de esta nota (por si cambias de nota antes de que se guarde)
    property string notaEditada: ""
    property bool cargando: false

    function guardarYa() {
        if (raiz.notaEditada !== "" && editor.text !== raiz.textoGuardado)
            boveda.guardarEn(raiz.notaEditada, editor.text)
        raiz.textoGuardado = editor.text
        autoguardar.stop()
    }
    property string textoGuardado: ""

    function cargarNota() {
        raiz.cargando = true
        editor.text = boveda.texto
        raiz.textoGuardado = boveda.texto
        raiz.notaEditada = boveda.nota
        raiz.textoVista = boveda.vista(boveda.texto)
        raiz.cargando = false
        sugerencias.close()
    }
    property string textoVista: ""

    component Titulo: Text {
        color: Tema.tenue
        font.pixelSize: 11
        font.weight: Font.DemiBold
        font.letterSpacing: 1
        font.capitalization: Font.AllUppercase
    }

    function abrir(ruta) { guardarYa(); verGrafo = false; boveda.abrir(ruta) }
    function abrirEnlace(nombre) { guardarYa(); verGrafo = false; boveda.abrirEnlace(nombre) }
    function nuevaNota(carpeta) {
        dialogo.pedir("Nota nueva", carpeta !== "" ? "Dentro de «" + carpeta + "»." : "", "", "Crear",
                      function (t) { raiz.guardarYa(); raiz.verGrafo = false; boveda.nuevaNota(t, carpeta); editor.forceActiveFocus() })
    }
    function nuevaCarpeta(padre) {
        dialogo.pedir("Carpeta nueva", padre !== "" ? "Dentro de «" + padre + "»." : "", "", "Crear",
                      function (t) { boveda.nuevaCarpeta(t, padre) })
    }
    function renombrar(ruta, nombre) {
        dialogo.pedir("Renombrar", "Los [[enlaces]] que apuntan aquí se actualizan solos.", nombre, "Renombrar",
                      function (t) { raiz.guardarYa(); boveda.renombrar(ruta, t) })
    }
    function borrar(ruta, nombre, esCarpeta) {
        dialogo.confirmar("¿Borrar «" + nombre + "»?",
            (esCarpeta ? "La carpeta y todo lo que tiene van" : "Va") + " a la papelera del vault (.trash): la puedes recuperar desde ahí.",
            "Borrar", true, function () { if (ruta === raiz.notaEditada) raiz.notaEditada = ""; boveda.borrar(ruta) })
    }

    Connections {
        target: boveda
        function onNotaCambio() {
            if (boveda.nota !== raiz.notaEditada || boveda.texto !== editor.text) raiz.cargarNota()
        }
    }
    Component.onCompleted: cargarNota()
    onActivaChanged: if (!activa) guardarYa()

    Shortcut { enabled: raiz.activa; sequence: "Ctrl+S"; onActivated: raiz.guardarYa() }
    Shortcut { enabled: raiz.activa; sequence: "Ctrl+E"; onActivated: raiz.modo = raiz.modo === "editar" ? "vista" : "editar" }
    Shortcut { enabled: raiz.activa; sequence: "Ctrl+G"; onActivated: raiz.verGrafo = !raiz.verGrafo }
    Shortcut { enabled: raiz.activa; sequence: "Ctrl+Shift+F"; onActivated: buscador.forceActiveFocus() }

    Timer { id: autoguardar; interval: 800; onTriggered: raiz.guardarYa() }
    Timer { id: refrescarVista; interval: 250; onTriggered: raiz.textoVista = boveda.vista(editor.text) }

    Dialogo { id: dialogo }
    FolderDialog {
        id: dialogoCarpeta
        title: "Abrir otra carpeta como vault"
        onAccepted: { raiz.guardarYa(); ia.abrirCarpetaVault(selectedFolder) }
    }
    MenuVortex {
        id: menuFila
        property string ruta: ""
        property string nombre: ""
        property bool carpeta: false
        OpcionMenu { text: "Nota nueva aquí"; icono: "notaNueva"; visible: menuFila.carpeta; onTriggered: raiz.nuevaNota(menuFila.ruta) }
        OpcionMenu { text: "Carpeta nueva aquí"; icono: "carpetaNueva"; visible: menuFila.carpeta; onTriggered: raiz.nuevaCarpeta(menuFila.ruta) }
        OpcionMenu { text: "Renombrar"; icono: "lapiz"; onTriggered: raiz.renombrar(menuFila.ruta, menuFila.nombre) }
        OpcionMenu { text: "Borrar"; icono: "basura"; peligro: true; onTriggered: raiz.borrar(menuFila.ruta, menuFila.nombre, menuFila.carpeta) }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // --- izquierda: árbol, búsqueda y memoria ---
        Rectangle {
            Layout.preferredWidth: 270
            Layout.fillHeight: true
            color: Tema.fondo
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 10
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Text { text: "Vault"; color: Tema.tenue; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 1; font.capitalization: Font.AllUppercase }
                        Text { Layout.fillWidth: true; text: boveda.nombre; color: Tema.texto; font.pixelSize: 16; font.weight: Font.Bold; elide: Text.ElideRight }
                    }
                    BotonIcono { icono: "carpeta"; tam: 32; ayuda: "Abrir otra carpeta"; onClicked: dialogoCarpeta.open() }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    BotonIcono { icono: "notaNueva"; tam: 34; ayuda: "Nota nueva (en la raíz)"; onClicked: raiz.nuevaNota("") }
                    BotonIcono { icono: "carpetaNueva"; tam: 34; ayuda: "Carpeta nueva"; onClicked: raiz.nuevaCarpeta("") }
                    BotonIcono { icono: "grafo"; tam: 34; activo: raiz.verGrafo; ayuda: "Ver el grafo (Ctrl+G)"; onClicked: { raiz.guardarYa(); raiz.verGrafo = !raiz.verGrafo } }
                    Item { Layout.fillWidth: true }
                    Text { text: boveda.totalNotas + (boveda.totalNotas === 1 ? " nota" : " notas"); color: Tema.tenue; font.pixelSize: 11 }
                }
                Campo {
                    id: buscador
                    Layout.fillWidth: true
                    placeholderText: "Buscar en tus notas o #etiqueta"
                    onTextChanged: esperaBusqueda.restart()
                    Timer { id: esperaBusqueda; interval: 200; onTriggered: boveda.busqueda = buscador.text }
                }

                // Árbol
                ListView {
                    id: arbol
                    visible: buscador.text.trim() === ""
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: boveda.lista
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                    delegate: Rectangle {
                        id: filaArbol
                        required property string tipo
                        required property string nombre
                        required property string ruta
                        required property int nivel
                        required property bool abierta
                        readonly property bool elegida: tipo === "nota" && ruta === boveda.nota && !raiz.verGrafo
                        width: ListView.view.width
                        height: 32
                        radius: 9
                        color: elegida ? Tema.barraSel : zonaArbol.containsMouse ? Tema.barraHover : "transparent"
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 8 + filaArbol.nivel * 14
                            anchors.rightMargin: 6
                            spacing: 6
                            Icono {
                                Layout.preferredWidth: 12; Layout.preferredHeight: 12
                                visible: filaArbol.tipo === "carpeta"
                                nombre: "derecha"
                                rotation: filaArbol.abierta ? 90 : 0
                                color: Tema.tenue
                            }
                            Icono {
                                Layout.preferredWidth: 15; Layout.preferredHeight: 15
                                nombre: filaArbol.tipo === "carpeta" ? "carpeta" : "nota"
                                color: filaArbol.elegida ? Tema.azul : filaArbol.tipo === "carpeta" ? Tema.suave : Tema.tenue
                            }
                            Text {
                                Layout.fillWidth: true
                                text: filaArbol.nombre
                                color: filaArbol.elegida ? Tema.texto : Tema.suave
                                font.pixelSize: 13
                                font.weight: filaArbol.elegida || filaArbol.tipo === "carpeta" ? Font.DemiBold : Font.Normal
                                elide: Text.ElideRight
                            }
                        }
                        MouseArea {
                            id: zonaArbol
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            cursorShape: Qt.PointingHandCursor
                            onClicked: (m) => {
                                if (m.button === Qt.RightButton) {
                                    menuFila.ruta = filaArbol.ruta
                                    menuFila.nombre = filaArbol.nombre
                                    menuFila.carpeta = filaArbol.tipo === "carpeta"
                                    menuFila.popup()
                                } else if (filaArbol.tipo === "carpeta")
                                    boveda.alternar(filaArbol.ruta)
                                else
                                    raiz.abrir(filaArbol.ruta)
                            }
                        }
                    }
                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 20
                        visible: arbol.count === 0
                        text: "Tu vault está vacío. Crea tu primera nota con el botón de arriba."
                        color: Tema.tenue
                        font.pixelSize: 12
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                    }
                }

                // Resultados de la búsqueda
                ListView {
                    id: resultados
                    visible: buscador.text.trim() !== ""
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: boveda.resultados
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                    delegate: Rectangle {
                        id: filaRes
                        required property var modelData
                        width: ListView.view.width
                        height: colRes.implicitHeight + 16
                        radius: 10
                        color: zonaRes.containsMouse ? Tema.barraHover : "transparent"
                        ColumnLayout {
                            id: colRes
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 2
                            Text { Layout.fillWidth: true; text: filaRes.modelData.titulo; color: Tema.texto; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                            Text {
                                Layout.fillWidth: true
                                visible: text !== ""
                                text: filaRes.modelData.fragmento
                                color: Tema.tenue
                                font.pixelSize: 11
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                textFormat: Text.PlainText
                            }
                        }
                        MouseArea { id: zonaRes; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: raiz.abrir(filaRes.modelData.ruta) }
                    }
                    Text {
                        anchors.centerIn: parent
                        visible: resultados.count === 0
                        text: "Nada con eso."
                        color: Tema.tenue
                        font.pixelSize: 12
                    }
                }

                // Memoria de la IA
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: colMem.implicitHeight + 24
                    radius: Tema.radioTarjeta - 6
                    color: Tema.tarjeta
                    border.width: 1
                    border.color: Tema.borde
                    ColumnLayout {
                        id: colMem
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 8
                        RowLayout {
                            spacing: 8
                            Icono { Layout.preferredWidth: 16; Layout.preferredHeight: 16; nombre: "memoria"; color: Tema.violeta }
                            Text { text: "Memoria de la IA"; color: Tema.texto; font.pixelSize: 13; font.weight: Font.DemiBold }
                        }
                        Text {
                            Layout.fillWidth: true
                            text: boveda.memoriaTexto !== "" ? boveda.memoriaTexto
                                : "Para «Usar mi vault» en el chat: la IA lee tus notas y las recuerda."
                            color: Tema.tenue
                            font.pixelSize: 11
                            wrapMode: Text.Wrap
                        }
                        Barrita { Layout.fillWidth: true; visible: boveda.memoriaProgreso >= 0; valor: boveda.memoriaProgreso; tinta: Tema.violeta }
                        Boton {
                            Layout.fillWidth: true
                            tipo: boveda.memoriaProgreso >= 0 ? "fantasma" : "secundario"
                            icono: boveda.memoriaProgreso >= 0 ? "detener" : "girar"
                            text: boveda.memoriaProgreso >= 0 ? "Detener" : "Actualizar memoria"
                            onClicked: boveda.memoriaProgreso >= 0 ? boveda.cancelarMemoria() : (raiz.guardarYa(), boveda.actualizarMemoria())
                        }
                    }
                }
            }
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Tema.borde }
        }

        // --- centro: el grafo o la nota ---
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Grafo {
                id: grafo
                objectName: "grafo"
                anchors.fill: parent
                visible: raiz.verGrafo
                datos: boveda.grafo
                actual: boveda.nota
                onAbrirNodo: (id, titulo, existe) => existe ? raiz.abrir(id) : raiz.abrirEnlace(titulo)
                Text {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.margins: 20
                    text: "Grafo · " + grafo.cuantos + (grafo.cuantos === 1 ? " nota" : " notas")
                    color: Tema.texto
                    font.pixelSize: 18
                    font.weight: Font.Bold
                }
                Text {
                    anchors.left: parent.left
                    anchors.bottom: parent.bottom
                    anchors.margins: 20
                    text: "Arrastra para mover · rueda para acercar · clic para abrir"
                    color: Tema.tenue
                    font.pixelSize: 11
                }
                BotonIcono {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 14
                    icono: "cerrar"
                    ayuda: "Volver a la nota"
                    onClicked: raiz.verGrafo = false
                }
            }

            // Sin nota abierta
            ColumnLayout {
                anchors.centerIn: parent
                visible: !raiz.verGrafo && !raiz.hayNota
                width: Math.min(parent.width - 60, 420)
                spacing: 14
                Icono { Layout.alignment: Qt.AlignHCenter; Layout.preferredWidth: 46; Layout.preferredHeight: 46; nombre: "vault"; color: Tema.tenue; trazo: 1.5 }
                Text { Layout.fillWidth: true; text: "Elige una nota o crea una nueva"; color: Tema.texto; font.pixelSize: 20; font.weight: Font.Bold; horizontalAlignment: Text.AlignHCenter }
                Text {
                    Layout.fillWidth: true
                    text: "Son archivos Markdown normales en tu carpeta: también los abre Obsidian. Enlaza notas con [[nombre]]."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                    horizontalAlignment: Text.AlignHCenter
                }
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 10
                    Boton { icono: "notaNueva"; text: "Nota nueva"; onClicked: raiz.nuevaNota("") }
                    Boton { tipo: "secundario"; icono: "grafo"; text: "Ver el grafo"; onClicked: raiz.verGrafo = true }
                }
            }

            // La nota
            ColumnLayout {
                anchors.fill: parent
                visible: !raiz.verGrafo && raiz.hayNota
                spacing: 0
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: 14
                    Layout.leftMargin: 24
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Text {
                            Layout.fillWidth: true
                            visible: boveda.carpetaNota !== ""
                            text: boveda.carpetaNota
                            color: Tema.tenue
                            font.pixelSize: 11
                            elide: Text.ElideMiddle
                        }
                        Text {
                            Layout.fillWidth: true
                            text: boveda.titulo
                            color: Tema.texto
                            font.pixelSize: 20
                            font.weight: Font.Bold
                            elide: Text.ElideRight
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onDoubleClicked: raiz.renombrar(boveda.nota, boveda.titulo) }
                        }
                    }
                    Text {
                        text: editor.text !== raiz.textoGuardado ? "Sin guardar" : "Guardado"
                        color: Tema.tenue
                        font.pixelSize: 11
                    }
                    // Editar / Vista / Lado a lado
                    Rectangle {
                        implicitWidth: filaModos.implicitWidth + 8
                        implicitHeight: 38
                        radius: 19
                        color: Tema.campo
                        border.width: 1
                        border.color: Tema.bordeCampo
                        Row {
                            id: filaModos
                            anchors.centerIn: parent
                            spacing: 2
                            Repeater {
                                model: [{ "id": "editar", "icono": "lapiz", "ayuda": "Editar (Ctrl+E)" },
                                        { "id": "vista", "icono": "ojo", "ayuda": "Vista previa (Ctrl+E)" },
                                        { "id": "lado", "icono": "columnas", "ayuda": "Lado a lado" }]
                                BotonIcono {
                                    required property var modelData
                                    tam: 30
                                    icono: modelData.icono
                                    activo: raiz.modo === modelData.id
                                    ayuda: modelData.ayuda
                                    onClicked: { raiz.modo = modelData.id; raiz.textoVista = boveda.vista(editor.text) }
                                }
                            }
                        }
                    }
                    BotonIcono { icono: "lapiz"; tam: 36; ayuda: "Renombrar"; onClicked: raiz.renombrar(boveda.nota, boveda.titulo) }
                    BotonIcono { icono: "basura"; tam: 36; ayuda: "Borrar"; onClicked: raiz.borrar(boveda.nota, boveda.titulo, false) }
                }
                Rectangle { Layout.fillWidth: true; height: 1; color: Tema.borde }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: 0

                    // Editor
                    ScrollView {
                        id: marcoEditor
                        visible: raiz.modo !== "vista"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredWidth: 1
                        TextArea {
                            id: editor
                            objectName: "editor"
                            leftPadding: 28
                            rightPadding: 28
                            topPadding: 20
                            bottomPadding: 40
                            color: Tema.texto
                            selectionColor: Tema.realceFuerte
                            selectedTextColor: Tema.texto
                            placeholderText: "Escribe aquí… Usa [[ para enlazar otra nota y # para etiquetas."
                            placeholderTextColor: Tema.tenue
                            font.pixelSize: 15
                            font.family: "monospace"
                            wrapMode: TextArea.Wrap
                            background: null
                            textFormat: TextEdit.PlainText

                            onTextChanged: {
                                if (raiz.cargando) return
                                autoguardar.restart()
                                if (raiz.modo !== "editar") refrescarVista.restart()
                                revisarSugerencias()
                            }
                            onCursorPositionChanged: if (!raiz.cargando) revisarSugerencias()

                            property string prefijo: ""
                            function revisarSugerencias() {
                                var antes = text.substring(Math.max(0, cursorPosition - 80), cursorPosition)
                                var m = antes.match(/\[\[([^\[\]\n|#]*)$/)
                                if (!m || !activeFocus) { sugerencias.close(); return }
                                prefijo = m[1]
                                var lista = boveda.sugerir(prefijo)
                                listaSug.model = lista
                                listaSug.currentIndex = 0
                                if (lista.length > 0) sugerencias.open()
                                else sugerencias.close()
                            }
                            function completar(nombre) {
                                var inicio = cursorPosition - prefijo.length
                                var cierra = text.substring(cursorPosition, cursorPosition + 2) === "]]"
                                remove(inicio, cursorPosition)
                                insert(inicio, nombre + (cierra ? "" : "]]"))
                                cursorPosition = inicio + nombre.length + 2
                                sugerencias.close()
                            }
                            // El [[enlace]] en una posición del texto, o ""
                            function enlaceEn(pos) {
                                var ini = text.lastIndexOf("[[", pos)
                                if (ini < 0) return ""
                                var fin = text.indexOf("]]", ini)
                                if (fin < 0 || fin + 2 < pos) return ""
                                var cuerpo = text.substring(ini + 2, fin)
                                if (cuerpo.indexOf("\n") >= 0) return ""
                                return cuerpo.split("|")[0]
                            }

                            Keys.onPressed: (e) => {
                                if (!sugerencias.visible) return
                                if (e.key === Qt.Key_Down) { listaSug.incrementCurrentIndex(); e.accepted = true }
                                else if (e.key === Qt.Key_Up) { listaSug.decrementCurrentIndex(); e.accepted = true }
                                else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Tab) {
                                    if (listaSug.currentIndex >= 0) editor.completar(listaSug.model[listaSug.currentIndex])
                                    e.accepted = true
                                } else if (e.key === Qt.Key_Escape) { sugerencias.close(); e.accepted = true }
                            }
                            TapHandler {
                                acceptedModifiers: Qt.ControlModifier
                                onTapped: (punto) => {
                                    var nombre = editor.enlaceEn(editor.positionAt(punto.position.x, punto.position.y))
                                    if (nombre !== "") raiz.abrirEnlace(nombre)
                                }
                            }
                            HoverHandler { acceptedModifiers: Qt.ControlModifier; cursorShape: Qt.PointingHandCursor }

                            // Autocompletado de [[
                            Popup {
                                id: sugerencias
                                x: Math.min(editor.cursorRectangle.x, editor.width - width - 10)
                                y: editor.cursorRectangle.y + editor.cursorRectangle.height + 6
                                width: 280
                                padding: 4
                                focus: false
                                closePolicy: Popup.CloseOnPressOutsideParent
                                background: Rectangle { radius: 14; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
                                contentItem: ListView {
                                    id: listaSug
                                    implicitHeight: Math.min(contentHeight, 8 * 34)
                                    clip: true
                                    model: []
                                    delegate: Rectangle {
                                        id: sug
                                        required property int index
                                        required property string modelData
                                        width: ListView.view.width
                                        height: 34
                                        radius: 10
                                        color: ListView.isCurrentItem ? Tema.realceFuerte : zonaSug.containsMouse ? Tema.realce : "transparent"
                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 10
                                            spacing: 8
                                            Icono { Layout.preferredWidth: 14; Layout.preferredHeight: 14; nombre: "nota"; color: Tema.tenue }
                                            Text { Layout.fillWidth: true; text: sug.modelData; color: Tema.texto; font.pixelSize: 13; elide: Text.ElideRight }
                                        }
                                        MouseArea { id: zonaSug; anchors.fill: parent; hoverEnabled: true; onClicked: editor.completar(sug.modelData) }
                                    }
                                }
                            }
                        }
                    }
                    Rectangle { visible: raiz.modo === "lado"; Layout.fillHeight: true; width: 1; color: Tema.borde }
                    // Vista previa
                    ScrollView {
                        visible: raiz.modo !== "editar"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredWidth: 1
                        contentWidth: availableWidth
                        Markdown {
                            id: vistaPrevia
                            width: parent.width
                            leftPadding: 28
                            rightPadding: 28
                            topPadding: 20
                            bottomPadding: 40
                            font.pixelSize: 15
                            markdown: raiz.textoVista
                            onNotaPedida: (nombre) => raiz.abrirEnlace(nombre)
                        }
                    }
                }
            }
        }

        // --- derecha: retroenlaces y etiquetas ---
        Rectangle {
            visible: raiz.hayNota && !raiz.verGrafo
            Layout.preferredWidth: 250
            Layout.fillHeight: true
            color: Tema.fondo
            Rectangle { anchors.left: parent.left; width: 1; height: parent.height; color: Tema.borde }
            Flickable {
                anchors.fill: parent
                anchors.margins: 16
                contentHeight: panel.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ColumnLayout {
                    id: panel
                    width: parent.width
                    spacing: 10


                    Titulo { text: "Retroenlaces · " + boveda.retroenlaces.length }
                    Text {
                        visible: boveda.retroenlaces.length === 0
                        Layout.fillWidth: true
                        text: "Ninguna nota enlaza aquí todavía."
                        color: Tema.tenue
                        font.pixelSize: 12
                        wrapMode: Text.Wrap
                    }
                    Repeater {
                        model: boveda.retroenlaces
                        Rectangle {
                            id: retro
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: colRetro.implicitHeight + 14
                            radius: 10
                            color: zonaRetro.containsMouse ? Tema.realce : Tema.tarjeta
                            ColumnLayout {
                                id: colRetro
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.margins: 10
                                spacing: 2
                                Text { Layout.fillWidth: true; text: retro.modelData.titulo; color: Tema.azul; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                Text {
                                    Layout.fillWidth: true
                                    visible: text !== ""
                                    text: retro.modelData.fragmento
                                    color: Tema.tenue
                                    font.pixelSize: 11
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                    textFormat: Text.PlainText
                                }
                            }
                            MouseArea { id: zonaRetro; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: raiz.abrir(retro.modelData.ruta) }
                        }
                    }

                    Titulo { Layout.topMargin: 10; text: "Enlaces · " + boveda.enlaces.length; visible: boveda.enlaces.length > 0 }
                    Flow {
                        Layout.fillWidth: true
                        visible: boveda.enlaces.length > 0
                        spacing: 6
                        Repeater {
                            model: boveda.enlaces
                            Chip {
                                required property var modelData
                                icono: "enlace"
                                texto: modelData.nombre
                                tinta: modelData.existe ? Tema.suave : Tema.tenue
                                opacity: modelData.existe ? 1 : 0.7
                                onPulsado: modelData.existe && modelData.ruta !== "" ? raiz.abrir(modelData.ruta) : raiz.abrirEnlace(modelData.nombre)
                            }
                        }
                    }

                    Titulo { Layout.topMargin: 10; text: "Etiquetas de esta nota"; visible: boveda.etiquetasNota.length > 0 }
                    Flow {
                        Layout.fillWidth: true
                        visible: boveda.etiquetasNota.length > 0
                        spacing: 6
                        Repeater {
                            model: boveda.etiquetasNota
                            Chip {
                                required property string modelData
                                icono: "etiqueta"
                                texto: "#" + modelData
                                onPulsado: buscador.text = "#" + modelData
                            }
                        }
                    }

                    Titulo { Layout.topMargin: 10; text: "Todas las etiquetas"; visible: boveda.etiquetas.length > 0 }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: boveda.etiquetas
                            Chip {
                                required property var modelData
                                texto: "#" + modelData.nombre + "  " + modelData.n
                                onPulsado: buscador.text = "#" + modelData.nombre
                            }
                        }
                    }
                }
            }
        }
    }
}
