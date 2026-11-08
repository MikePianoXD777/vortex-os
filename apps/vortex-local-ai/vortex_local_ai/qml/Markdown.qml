// Texto en Markdown, solo para leer: el chat y la vista previa de las notas. Los enlaces [[nota]] llegan
// ya convertidos (boveda.vista) a «nota:Nombre»; al tocarlos se avisa con notaPedida.
import QtQuick
import QtQuick.Controls.Basic

TextEdit {
    id: raiz
    property string markdown: ""
    signal notaPedida(string nombre)
    textFormat: TextEdit.MarkdownText
    // TextEdit pinta los enlaces de azul puro (no hace caso a palette.link): el texto de cada enlace va
    // dentro de un <span> del color del tema. Las imágenes (![...]) se quedan igual.
    text: markdown.replace(/(^|[^!])\[([^\]\n]+)\]\(([^)\s]+)\)/g,
        (_, antes, texto, url) => antes + "[<span style=\"color:" + Tema.azul + "\">" + texto + "</span>](" + url + ")")
    readOnly: true
    selectByMouse: true
    wrapMode: TextEdit.Wrap
    color: Tema.texto
    selectionColor: Tema.realceFuerte
    selectedTextColor: Tema.texto
    font.pixelSize: 14
    onLinkActivated: (enlace) => {
        if (enlace.indexOf("nota:") === 0)
            raiz.notaPedida(decodeURIComponent(enlace.substring(5)))
        else
            Qt.openUrlExternally(enlace)
    }
    HoverHandler { cursorShape: raiz.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.IBeamCursor }
}
