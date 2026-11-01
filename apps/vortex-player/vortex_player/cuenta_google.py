"""La cuenta de Google con la que se sincroniza con el teléfono.

Se inicia sesión en el navegador (Google regresa a un puerto local de esta PC) y solo se pide permiso para la
carpeta oculta de la app en Drive: Vortex Player no ve nada más de tu Drive. La sesión se guarda en
~/.config/vortex-player/google.json, que solo puede leer tu usuario.
"""
from __future__ import annotations

import base64
import hashlib
import http.server
import json
import os
import secrets
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import webbrowser
from pathlib import Path
from typing import Callable

from . import rutas

# El cliente «App de escritorio» del proyecto Vortex Player en Google Cloud. El secreto no va en el repo
# (2.4.0): se lee de ~/.config/vortex-player/cliente-google.json o de VORTEX_PLAYER_GOOGLE_SECRETO; sin él,
# Ajustes dice que esta versión todavía no está registrada con Google.
CLIENTE_ID = "986706412005-cn9dc33hf03a6ock3f7mdm1j15a4mndt.apps.googleusercontent.com"
SECRETO_AL_REVES = "rVPV39OdxANwk3mpxOAg3GG35wGN-XPSCOG"

ALCANCE = "https://www.googleapis.com/auth/drive.appdata"
AUTORIZAR = "https://accounts.google.com/o/oauth2/v2/auth"
TOKEN = "https://oauth2.googleapis.com/token"
REVOCAR = "https://oauth2.googleapis.com/revoke"
API = "https://www.googleapis.com/drive/v3"
SUBIR = "https://www.googleapis.com/upload/drive/v3"
ARCHIVO = "vortex-player.json"


class SinPermiso(Exception):
    """Google ya no reconoce la sesión (la cerraste en tu cuenta o caducó)."""


def _local() -> dict:
    """El cliente de ~/.config/vortex-player/cliente-google.json ({"id": …, "secreto": …}), fuera del repo
    (google.json es tu sesión)."""
    try:
        return json.loads((rutas.config() / "cliente-google.json").read_text())
    except (OSError, ValueError):
        return {}


def cliente() -> tuple[str, str]:
    local = _local()
    return (os.environ.get("VORTEX_PLAYER_GOOGLE_ID") or local.get("id") or CLIENTE_ID,
            os.environ.get("VORTEX_PLAYER_GOOGLE_SECRETO") or local.get("secreto") or SECRETO_AL_REVES[::-1])


def configurada() -> bool:
    return all(cliente())


# --- HTTP (se cambia en las pruebas) -------------------------------------------------------------------

def _http(metodo: str, url: str, cuerpo: bytes | None = None, cabeceras: dict | None = None) -> tuple[int, bytes]:
    req = urllib.request.Request(url, data=cuerpo, method=metodo, headers=cabeceras or {})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


pedir: Callable[[str, str, bytes | None, dict | None], tuple[int, bytes]] = _http


def _formulario(datos: dict) -> bytes:
    return urllib.parse.urlencode(datos).encode()


# --- la sesión guardada --------------------------------------------------------------------------------

def _archivo() -> Path:
    return rutas.config() / "google.json"


def sesion() -> dict:
    """{refresh_token, correo, nombre} o {} si no has iniciado sesión."""
    try:
        d = json.loads(_archivo().read_text())
        return d if isinstance(d, dict) and d.get("refresh_token") else {}
    except (OSError, ValueError):
        return {}


def _guardar(d: dict) -> None:
    _archivo().parent.mkdir(parents=True, exist_ok=True)
    tmp = _archivo().with_suffix(".tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump(d, f)
    tmp.replace(_archivo())


_token: dict = {"valor": "", "vence": 0.0}


def token() -> str:
    """Un token para Drive (se renueva solo con la sesión guardada)."""
    if _token["valor"] and time.time() < _token["vence"] - 60:
        return _token["valor"]
    s = sesion()
    if not s:
        raise SinPermiso("No has iniciado sesión")
    cid, secreto = cliente()
    estado, cuerpo = pedir("POST", TOKEN, _formulario({"client_id": cid, "client_secret": secreto,
                                                       "refresh_token": s["refresh_token"], "grant_type": "refresh_token"}),
                           {"Content-Type": "application/x-www-form-urlencoded"})
    if estado in (400, 401):
        raise SinPermiso("Google ya no reconoce la sesión: vuelve a iniciarla")
    if estado != 200:
        raise OSError(f"Google contestó {estado}")
    d = json.loads(cuerpo)
    _token.update(valor=d["access_token"], vence=time.time() + int(d.get("expires_in", 3600)))
    return _token["valor"]


def olvidar_token() -> None:
    _token.update(valor="", vence=0.0)


# --- iniciar y cerrar sesión ---------------------------------------------------------------------------

def _pkce() -> tuple[str, str]:
    verificador = secrets.token_urlsafe(64)
    reto = base64.urlsafe_b64encode(hashlib.sha256(verificador.encode()).digest()).rstrip(b"=").decode()
    return verificador, reto


PAGINA = """<!doctype html><meta charset="utf-8"><title>Vortex Player</title>
<body style="background:#0B0D1A;color:#E8ECFF;font-family:sans-serif;display:grid;place-items:center;height:90vh">
<div style="text-align:center"><h1 style="color:#9EC5FF">{titulo}</h1><p>{texto}</p></div>"""


def iniciar_sesion(abrir: Callable[[str], object] = webbrowser.open, esperar: float = 300) -> dict:
    """Abre el navegador para elegir la cuenta; regresa {correo, nombre} cuando Google contesta."""
    if not configurada():
        raise RuntimeError("Esta versión todavía no está registrada con Google")
    cid, secreto = cliente()
    verificador, reto = _pkce()
    estado_esperado = secrets.token_urlsafe(16)
    respuesta: dict = {}
    listo = threading.Event()

    class Manejador(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:  # noqa: N802 (nombre de http.server)
            q = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
            if "code" not in q and "error" not in q:
                self.send_response(404)
                self.end_headers()
                return
            respuesta.update({k: v[0] for k, v in q.items()})
            ok = "code" in q and q.get("state", [""])[0] == estado_esperado
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(PAGINA.format(
                titulo="¡Listo!" if ok else "No se inició sesión",
                texto="Ya puedes cerrar esta pestaña y volver a Vortex Player." if ok
                else "Vuelve a Vortex Player e inténtalo otra vez.").encode())
            listo.set()

        def log_message(self, *args) -> None:
            pass

    servidor = http.server.HTTPServer(("127.0.0.1", 0), Manejador)
    redireccion = f"http://127.0.0.1:{servidor.server_port}"
    hilo = threading.Thread(target=servidor.serve_forever, daemon=True)
    hilo.start()
    try:
        abrir(AUTORIZAR + "?" + urllib.parse.urlencode({
            "client_id": cid, "redirect_uri": redireccion, "response_type": "code", "scope": ALCANCE,
            "code_challenge": reto, "code_challenge_method": "S256", "state": estado_esperado,
            "access_type": "offline", "prompt": "consent"}))
        if not listo.wait(esperar):
            raise TimeoutError("No llegó la respuesta de Google (¿cerraste la pestaña?)")
    finally:
        servidor.shutdown()
        servidor.server_close()
    if respuesta.get("state") != estado_esperado or "code" not in respuesta:
        raise RuntimeError("No se inició sesión" + (f" ({respuesta['error']})" if respuesta.get("error") else ""))
    estado, cuerpo = pedir("POST", TOKEN, _formulario({
        "client_id": cid, "client_secret": secreto, "code": respuesta["code"], "code_verifier": verificador,
        "grant_type": "authorization_code", "redirect_uri": redireccion}),
        {"Content-Type": "application/x-www-form-urlencoded"})
    if estado != 200:
        raise RuntimeError(f"Google no dio la sesión ({estado})")
    d = json.loads(cuerpo)
    if not d.get("refresh_token"):
        raise RuntimeError("Google no dio una sesión que dure")
    _token.update(valor=d["access_token"], vence=time.time() + int(d.get("expires_in", 3600)))
    nombre, correo = Drive(d["access_token"]).usuario()
    datos = {"refresh_token": d["refresh_token"], "correo": correo, "nombre": nombre}
    _guardar(datos)
    return {"correo": correo, "nombre": nombre}


def cerrar_sesion() -> None:
    s = sesion()
    if s:
        try:
            pedir("POST", REVOCAR, _formulario({"token": s["refresh_token"]}),
                  {"Content-Type": "application/x-www-form-urlencoded"})
        except OSError:
            pass   # sin internet: igual se olvida aquí
    olvidar_token()
    _archivo().unlink(missing_ok=True)


# --- Drive (solo la carpeta oculta de la app) -------------------------------------------------------------

class Drive:
    def __init__(self, token_acceso: str):
        self.token = token_acceso

    def _pedir(self, metodo: str, url: str, cuerpo: bytes | None = None, tipo: str = "") -> bytes:
        cab = {"Authorization": f"Bearer {self.token}"}
        if tipo:
            cab["Content-Type"] = tipo
        estado, datos = pedir(metodo, url, cuerpo, cab)
        if estado == 401:
            olvidar_token()
            raise SinPermiso("Google ya no reconoce la sesión")
        if estado >= 400:
            raise OSError(f"Google Drive contestó {estado}")
        return datos

    def buscar(self) -> str | None:
        q = urllib.parse.quote(f"name = '{ARCHIVO}'")
        r = json.loads(self._pedir("GET", f"{API}/files?spaces=appDataFolder&q={q}&fields=files(id)&orderBy=modifiedTime+desc"))
        archivos = r.get("files") or []
        return archivos[0]["id"] if archivos else None

    def leer(self, fid: str) -> bytes:
        return self._pedir("GET", f"{API}/files/{fid}?alt=media")

    def crear(self, texto: str) -> str:
        frontera = "vortex" + secrets.token_hex(8)
        datos = json.dumps({"name": ARCHIVO, "parents": ["appDataFolder"]})
        cuerpo = (f"--{frontera}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{datos}\r\n"
                  f"--{frontera}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{texto}\r\n--{frontera}--\r\n").encode()
        r = self._pedir("POST", f"{SUBIR}/files?uploadType=multipart&fields=id", cuerpo, f"multipart/related; boundary={frontera}")
        return json.loads(r)["id"]

    def escribir(self, fid: str, texto: str) -> None:
        self._pedir("PATCH", f"{SUBIR}/files/{fid}?uploadType=media", texto.encode(), "application/json; charset=UTF-8")

    def usuario(self) -> tuple[str, str]:
        u = json.loads(self._pedir("GET", f"{API}/about?fields=user(displayName,emailAddress)")).get("user", {})
        return u.get("displayName", ""), u.get("emailAddress", "")
