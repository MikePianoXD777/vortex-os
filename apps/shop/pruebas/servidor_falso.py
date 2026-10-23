#!/usr/bin/env python3
"""Un Flathub FALSO para las pruebas: la API de /api/v2 servida en 127.0.0.1 con las respuestas reales
guardadas en datos/ (ver bajar_datos.py). Nunca sale a internet.

    servidor_falso.py CARPETA_DE_DATOS ARCHIVO_DEL_PUERTO [CARPETA_DE_CONTROL]

Se queda corriendo hasta que lo matan. Imita lo que importa de la API real:
  - las listas exigen «page» y «per_page» juntos (si no, 400);
  - una categoría que no existe responde 422, una ficha que no existe 404;
  - cada petición se anota en CONTROL/pedidos.log («GET /ruta?consulta» o «POST /ruta cuerpo»);
  - si existe CONTROL/modo con «500» (o «lento»), todo responde 500 (o tarda 3 s).
"""
import http.server
import json
import os
import sys
import time
import urllib.parse


class Crudo(str):
    """Un archivo de datos que no es JSON válido: se sirve tal cual (para probar respuestas rotas)."""


class Falso(http.server.BaseHTTPRequestHandler):
    datos = ""
    control = ""

    def log_message(self, *args):
        pass

    def _modo(self):
        try:
            with open(os.path.join(self.control, "modo")) as f:
                return f.read().strip()
        except OSError:
            return ""

    def _cargar(self, nombre):
        try:
            with open(os.path.join(self.datos, nombre), encoding="utf-8") as f:
                texto = f.read()
        except OSError:
            return None
        try:
            return json.loads(texto)
        except ValueError:
            return Crudo(texto)

    def _responder(self, codigo, cuerpo):
        crudo = (cuerpo if isinstance(cuerpo, Crudo) else json.dumps(cuerpo, ensure_ascii=False)).encode()
        self.send_response(codigo)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(crudo)))
        self.end_headers()
        self.wfile.write(crudo)

    def _anotar(self, texto):
        with open(os.path.join(self.control, "pedidos.log"), "a") as f:
            f.write(texto + "\n")

    def _atender(self, metodo, cuerpo):
        url = urllib.parse.urlsplit(self.path)
        consulta = urllib.parse.parse_qs(url.query)
        self._anotar("%s %s%s%s" % (metodo, url.path, "?" + url.query if url.query else "",
                                    " " + json.dumps(cuerpo, sort_keys=True) if cuerpo is not None else ""))
        modo = self._modo()
        if modo == "500":
            return self._responder(500, {"detail": "Internal Server Error"})
        if modo == "lento":
            time.sleep(3)
        ruta = url.path
        if not (self.headers.get("User-Agent") or "").startswith("vortexines-shop/"):
            return self._responder(403, {"detail": "User-Agent no esperado: %s" % self.headers.get("User-Agent")})

        if ruta.startswith("/collection/"):
            resto = ruta[len("/collection/"):]
            if resto != "category" and ("page" not in consulta or "per_page" not in consulta):
                return self._responder(400, {"detail": "Bad Request"})
            if resto == "category":
                return self._responder(200, self._cargar("categorias.json"))
            if resto in ("trending", "popular"):
                return self._responder(200, self._cargar("trending.json"))
            if resto.startswith("category/"):
                categoria = resto[len("category/"):]
                validas = self._cargar("categorias.json")
                if categoria not in validas:
                    return self._responder(422, {"detail": "categoría inválida"})
                pagina = int(consulta["page"][0])
                if categoria == "game" and pagina == 1:
                    return self._responder(200, self._cargar("categoria_game.json"))
                return self._responder(200, {"hits": [], "page": pagina, "totalHits": 0})
        elif ruta == "/search" and metodo == "POST":
            consulta_texto = (cuerpo or {}).get("query", "").lower()
            if "minecraft" in consulta_texto:
                return self._responder(200, self._cargar("busqueda_minecraft.json"))
            return self._responder(200, {"hits": [], "query": consulta_texto, "totalHits": 0})
        elif ruta.startswith("/appstream/"):
            ficha = self._cargar("appstream_%s.json" % ruta[len("/appstream/"):])
            return self._responder(200, ficha) if ficha else self._responder(404, {"detail": "App not found"})
        elif ruta.startswith("/summary/"):
            resumen = self._cargar("resumen_%s.json" % ruta[len("/summary/"):])
            return self._responder(200, resumen) if resumen else self._responder(404, {"detail": "App not found"})
        elif ruta.startswith("/app-picks/app-of-the-day/"):
            return self._responder(200, self._cargar("app_del_dia.json"))
        elif ruta.startswith("/app-picks/apps-of-the-week/"):
            return self._responder(200, self._cargar("apps_de_la_semana.json"))
        return self._responder(404, {"detail": "Not Found"})

    def do_GET(self):
        self._atender("GET", None)

    def do_POST(self):
        largo = int(self.headers.get("Content-Length") or 0)
        self._atender("POST", json.loads(self.rfile.read(largo) or b"{}"))


def main():
    datos, archivo_puerto = sys.argv[1], sys.argv[2]
    Falso.datos = datos
    Falso.control = sys.argv[3] if len(sys.argv) > 3 else os.path.dirname(os.path.abspath(archivo_puerto))
    servidor = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Falso)
    with open(archivo_puerto, "w") as f:
        f.write(str(servidor.server_address[1]))
    servidor.serve_forever()


if __name__ == "__main__":
    main()
