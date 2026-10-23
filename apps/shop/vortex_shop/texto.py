"""De la descripción de AppStream (un HTML mínimo: <p>, <ul>, <ol>, <li>, <em>, <code>) a texto plano.

Párrafos separados por una línea en blanco, viñetas «• » en las listas, números en las ordenadas.
También acepta texto que ya viene plano (las búsquedas de Flathub lo devuelven así): solo le
quita la sangría de cada renglón y las líneas en blanco de más.
"""
import re
from html.parser import HTMLParser

_BLOQUES = {"p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre"}


class _Convertidor(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.bloques = []        # lista de (tipo, texto): "p" o "li"
        self.actual = None       # texto del bloque que se está armando
        self.tipo = "p"
        self.listas = []         # pila de listas abiertas: [tipo, contador, número de lista]
        self.n_listas = 0

    def _cerrar(self):
        if self.actual is not None:
            texto = re.sub(r"[ \t\r\f\v]*\n[ \t\r\f\v]*", "\n", self.actual)
            texto = re.sub(r"[ \t\r\f\v]+", " ", texto).strip()
            if texto:
                self.bloques.append((self.tipo, texto, self.listas[-1][2] if self.tipo == "li" and self.listas else 0))
        self.actual = None

    def _abrir(self, tipo):
        self._cerrar()
        self.tipo = tipo
        self.actual = ""

    def handle_starttag(self, tag, attrs):
        if tag in ("ul", "ol"):
            self._cerrar()
            self.n_listas += 1
            self.listas.append([tag, 0, self.n_listas])
        elif tag == "li":
            prefijo = "• "
            if self.listas and self.listas[-1][0] == "ol":
                self.listas[-1][1] += 1
                prefijo = "%d. " % self.listas[-1][1]
            self._abrir("li")
            self.actual = prefijo
        elif tag in _BLOQUES:
            self._abrir("p")
        elif tag == "br":
            if self.actual is None:
                self.actual = ""
            self.actual += "\n"

    def handle_endtag(self, tag):
        if tag in ("ul", "ol"):
            self._cerrar()
            if self.listas:
                self.listas.pop()
        elif tag == "li" or tag in _BLOQUES:
            self._cerrar()

    def handle_data(self, data):
        if self.actual is None:
            if not data.strip():
                return
            self.tipo = "p"
            self.actual = ""
        self.actual += data

    def texto(self):
        self._cerrar()
        salida = []
        previo = None
        for tipo, texto, lista in self.bloques:
            if previo is not None:
                # Entre elementos de una misma lista, un solo salto; una introducción que termina
                # en «:» queda pegada a su lista; lo demás, una línea en blanco.
                pegado = (tipo == "li" and previo[0] == "li" and previo[2] == lista) or \
                         (tipo == "li" and previo[0] == "p" and previo[1].endswith(":"))
                salida.append("\n" if pegado else "\n\n")
            salida.append(texto)
            previo = (tipo, texto, lista)
        return "".join(salida)


def html_a_texto(contenido):
    """Convierte una descripción de AppStream (HTML o texto) en texto plano."""
    if not contenido:
        return ""
    if "<" not in contenido:
        lineas = [renglon.strip() for renglon in str(contenido).splitlines()]
        return re.sub(r"\n{3,}", "\n\n", "\n".join(lineas)).strip()
    conversor = _Convertidor()
    conversor.feed(str(contenido))
    conversor.close()
    return conversor.texto()
