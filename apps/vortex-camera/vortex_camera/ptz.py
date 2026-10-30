"""Mover las cámaras que giran (PTZ).

Dos caminos, porque no todas las Imou dejan los dos:
  - ONVIF (SOAP en el puerto 80): ContinuousMove mientras dejas apretada la flecha, y Stop al soltarla.
    La contraseña va con WS-Security (resumen SHA-1 con nonce): nunca viaja tal cual.
  - El CGI de Dahua: /cgi-bin/ptz.cgi?action=start&code=Left…, con autenticación digest.
El primero que funcione se guarda en la cámara («onvif» o «cgi») para no volver a probar; si ninguno
funciona, «no», y la interfaz esconde las flechas.
"""
from __future__ import annotations

import base64
import hashlib
import os
import re
import urllib.error
import urllib.request
from datetime import datetime, timezone

from .camaras import Camara

VELOCIDAD = 0.5
# dirección → (x, y) para ONVIF y código para el CGI de Dahua
DIRECCIONES = {
    "arriba": ((0.0, VELOCIDAD), "Up"),
    "abajo": ((0.0, -VELOCIDAD), "Down"),
    "izquierda": ((-VELOCIDAD, 0.0), "Left"),
    "derecha": ((VELOCIDAD, 0.0), "Right"),
}
ESPERA = 4


class SinPtz(Exception):
    """La cámara no se deja mover desde la PC (no gira, o no abre ni ONVIF ni el CGI)."""


# --- ONVIF ---

def _seguridad(usuario: str, clave: str) -> str:
    nonce = os.urandom(16)
    creado = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")
    resumen = base64.b64encode(hashlib.sha1(nonce + creado.encode() + clave.encode()).digest()).decode()
    usuario = usuario.replace("&", "&amp;").replace("<", "&lt;")
    return (
        '<Security s:mustUnderstand="1" xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd">'
        f"<UsernameToken><Username>{usuario}</Username>"
        '<Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">'
        f"{resumen}</Password>"
        '<Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">'
        f"{base64.b64encode(nonce).decode()}</Nonce>"
        '<Created xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">'
        f"{creado}</Created></UsernameToken></Security>"
    )


def sobre(cuerpo: str, usuario: str, clave: str) -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">'
        f"<s:Header>{_seguridad(usuario, clave)}</s:Header><s:Body>{cuerpo}</s:Body></s:Envelope>"
    ).encode()


def _soap(url: str, cuerpo: str, cam: Camara) -> str:
    peticion = urllib.request.Request(url, data=sobre(cuerpo, cam.usuario, cam.codigo),
                                      headers={"Content-Type": "application/soap+xml; charset=utf-8"})
    try:
        with urllib.request.urlopen(peticion, timeout=ESPERA) as r:
            return r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        e.close()
        raise SinPtz(f"ONVIF respondió {e.code}") from None
    except (urllib.error.URLError, OSError) as e:
        raise SinPtz(f"ONVIF no contesta: {e}") from e


_PERFILES: dict[str, str] = {}


def _perfil(cam: Camara) -> str:
    """El token del primer perfil de video (Dahua suele llamarlo «MediaProfile000»)."""
    if cam.id in _PERFILES:
        return _PERFILES[cam.id]
    xml = _soap(f"http://{cam.ip}/onvif/media_service",
                '<GetProfiles xmlns="http://www.onvif.org/ver10/media/wsdl"/>', cam)
    m = re.search(r'<[^>]*Profiles[^>]*\btoken="([^"]+)"', xml)
    if not m:
        raise SinPtz("ONVIF no dio ningún perfil")
    _PERFILES[cam.id] = m.group(1)
    return m.group(1)


def onvif_mover(cam: Camara, direccion: str) -> None:
    (x, y), _ = DIRECCIONES[direccion]
    _soap(f"http://{cam.ip}/onvif/ptz_service",
          '<ContinuousMove xmlns="http://www.onvif.org/ver20/ptz/wsdl">'
          f"<ProfileToken>{_perfil(cam)}</ProfileToken><Velocity>"
          f'<PanTilt x="{x}" y="{y}" xmlns="http://www.onvif.org/ver10/schema"/>'
          "</Velocity></ContinuousMove>", cam)


def onvif_parar(cam: Camara) -> None:
    _soap(f"http://{cam.ip}/onvif/ptz_service",
          '<Stop xmlns="http://www.onvif.org/ver20/ptz/wsdl">'
          f"<ProfileToken>{_perfil(cam)}</ProfileToken><PanTilt>true</PanTilt><Zoom>true</Zoom></Stop>", cam)


# --- CGI de Dahua ---

def _cgi(cam: Camara, accion: str, codigo: str) -> None:
    url = (f"http://{cam.ip}/cgi-bin/ptz.cgi?action={accion}&channel={cam.canal}"
           f"&code={codigo}&arg1=0&arg2=4&arg3=0")
    claves = urllib.request.HTTPPasswordMgrWithDefaultRealm()
    claves.add_password(None, f"http://{cam.ip}/", cam.usuario, cam.codigo)
    abridor = urllib.request.build_opener(urllib.request.HTTPDigestAuthHandler(claves))
    try:
        with abridor.open(url, timeout=ESPERA) as r:
            texto = r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        e.close()
        raise SinPtz(f"el CGI respondió {e.code}") from None
    except (urllib.error.URLError, OSError) as e:
        raise SinPtz(f"el CGI no contesta: {e}") from e
    if "OK" not in texto:
        raise SinPtz(f"el CGI no lo hizo: {texto.strip()[:80]}")


# --- lo que usa la interfaz ---

def mover(cam: Camara, direccion: str) -> str:
    """Empieza a girar. Devuelve el método que funcionó («onvif» o «cgi»); SinPtz si ninguno."""
    if direccion not in DIRECCIONES:
        raise ValueError(direccion)
    if cam.ptz == "no":
        raise SinPtz("esta cámara no se deja mover")
    metodos = [cam.ptz] if cam.ptz in ("onvif", "cgi") else ["onvif", "cgi"]
    ultimo: SinPtz | None = None
    for metodo in metodos:
        try:
            if metodo == "onvif":
                onvif_mover(cam, direccion)
            else:
                _cgi(cam, "start", DIRECCIONES[direccion][1])
            return metodo
        except SinPtz as e:
            ultimo = e
    raise ultimo or SinPtz("esta cámara no se deja mover")


def parar(cam: Camara, direccion: str = "") -> None:
    if cam.ptz == "onvif":
        onvif_parar(cam)
    elif cam.ptz == "cgi":
        for _xy, codigo in ([DIRECCIONES[direccion]] if direccion in DIRECCIONES else DIRECCIONES.values()):
            _cgi(cam, "stop", codigo)
