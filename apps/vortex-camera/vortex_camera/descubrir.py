"""Encontrar las cámaras en tu red, para no tener que saber su IP.

Las Imou/Dahua contestan a la búsqueda DHIP: un paquete UDP al puerto 37810 (a 239.255.255.251 y a
broadcast) con {"method": "DHDiscover.search"}. Cada cámara responde con su número de serie, IP, modelo
y MAC, así que con el número de serie de la etiqueta se sabe cuál es cuál aunque cambie de IP.

Si la red no deja pasar esa búsqueda (algunos módems la bloquean), `escanear` prueba el puerto 37777
(el de Dahua) y el 554 (RTSP) en tu red /24: encuentra la IP, pero no el número de serie.

Las cámaras de otras casas (por Tailscale) no salen en ninguna búsqueda: se escribe su IP a mano.
"""
from __future__ import annotations

import base64
import hashlib
import json
import re
import socket
import struct
import time
from concurrent.futures import ThreadPoolExecutor

from .camaras import limpiar_serie

PUERTO_DHIP = 37810
GRUPO_DHIP = "239.255.255.251"


def paquete_busqueda() -> bytes:
    """Cabecera DHIP de 32 bytes + la búsqueda en JSON."""
    datos = json.dumps({"method": "DHDiscover.search", "params": {"mac": "", "uni": 1}},
                       separators=(",", ":")).encode()
    n = len(datos)
    return b"\x20\x00\x00\x00DHIP" + bytes(8) + struct.pack("<IIII", n, 0, n, 0) + datos


def interpretar(datos: bytes, origen: str = "") -> dict | None:
    """La respuesta de una cámara → {serie, ip, modelo, mac, version}. None si no es una respuesta
    (por ejemplo, nuestra propia búsqueda, que también llega al grupo)."""
    inicio, fin = datos.find(b"{"), datos.rfind(b"}")
    if inicio < 0 or fin <= inicio:
        return None
    try:
        j = json.loads(datos[inicio:fin + 1].decode("utf-8", "replace"))
    except ValueError:
        return None
    if not isinstance(j, dict) or j.get("method") == "DHDiscover.search":
        return None
    params = j.get("params") if isinstance(j.get("params"), dict) else {}
    info = params.get("deviceInfo") if isinstance(params.get("deviceInfo"), dict) else params
    ipv4 = info.get("IPv4Address") if isinstance(info.get("IPv4Address"), dict) else {}
    ip = str(ipv4.get("IPAddress") or origen or "")
    serie = limpiar_serie(str(info.get("SerialNo") or ""))
    if not ip and not serie:
        return None
    return {
        "serie": serie,
        "ip": ip,
        "modelo": str(info.get("DeviceType") or info.get("DeviceClass") or ""),
        "mac": str(info.get("Mac") or "").lower(),
        "version": str(info.get("Version") or ""),
    }


def buscar(segundos: float = 2.5, destinos: list[tuple[str, int]] | None = None) -> list[dict]:
    """Pregunta en la red quién es cámara y junta las respuestas durante `segundos`."""
    destinos = destinos or [(GRUPO_DHIP, PUERTO_DHIP), ("255.255.255.255", PUERTO_DHIP)]
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
    try:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
        # Algunas cámaras contestan al grupo y no a quien preguntó: se escucha en su puerto si se puede.
        try:
            s.bind(("", PUERTO_DHIP))
            s.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP,
                         struct.pack("4s4s", socket.inet_aton(GRUPO_DHIP), socket.inet_aton("0.0.0.0")))
        except OSError:
            s.bind(("", 0))
        paquete = paquete_busqueda()
        for destino in destinos:
            try:
                s.sendto(paquete, destino)
            except OSError:
                pass
        encontradas: dict[str, dict] = {}
        limite = time.monotonic() + segundos
        while (falta := limite - time.monotonic()) > 0:
            s.settimeout(falta)
            try:
                datos, (origen, _puerto) = s.recvfrom(65535)
            except (socket.timeout, OSError):
                break
            camara = interpretar(datos, origen)
            if camara:
                encontradas[camara["serie"] or camara["ip"]] = camara
        return sorted(encontradas.values(), key=lambda c: tuple(int(p) if p.isdigit() else 0 for p in c["ip"].split(".")))
    finally:
        s.close()


def ip_local() -> str:
    """La IP de esta PC en tu red (sin mandar nada: connect en UDP solo elige la ruta)."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(("192.0.2.1", 9))
        return s.getsockname()[0]
    except OSError:
        return ""
    finally:
        s.close()


def abierto(ip: str, puerto: int, espera: float = 1.5) -> bool:
    try:
        with socket.create_connection((ip, puerto), timeout=espera):
            return True
    except OSError:
        return False


def _rtsp(s: socket.socket, metodo: str, url: str, cseq: int, extra: str = "") -> tuple[int, dict]:
    s.sendall(f"{metodo} {url} RTSP/1.0\r\nCSeq: {cseq}\r\nAccept: application/sdp\r\n{extra}\r\n".encode())
    datos = b""
    while b"\r\n\r\n" not in datos:
        trozo = s.recv(4096)
        if not trozo:
            break
        datos += trozo
    cabeza = datos.split(b"\r\n\r\n", 1)[0].decode("utf-8", "replace").split("\r\n")
    m = re.match(r"RTSP/1\.0 (\d{3})", cabeza[0] if cabeza else "")
    cabeceras: dict[str, str] = {}
    for linea in cabeza[1:]:
        k, _, v = linea.partition(":")
        cabeceras.setdefault(k.strip().lower(), v.strip())
    return (int(m.group(1)) if m else 0), cabeceras


def _autorizacion(desafio: str, metodo: str, url: str, usuario: str, clave: str) -> str:
    """La cabecera Authorization para el desafío de la cámara (Digest MD5 o Basic)."""
    if desafio.lower().startswith("digest"):
        campos = dict(re.findall(r'(\w+)="?([^",]*)"?', desafio))
        ha1 = hashlib.md5(f"{usuario}:{campos.get('realm', '')}:{clave}".encode()).hexdigest()
        ha2 = hashlib.md5(f"{metodo}:{url}".encode()).hexdigest()
        respuesta = hashlib.md5(f"{ha1}:{campos.get('nonce', '')}:{ha2}".encode()).hexdigest()
        return (f'Authorization: Digest username="{usuario}", realm="{campos.get("realm", "")}", '
                f'nonce="{campos.get("nonce", "")}", uri="{url}", response="{respuesta}"\r\n')
    return "Authorization: Basic " + base64.b64encode(f"{usuario}:{clave}".encode()).decode() + "\r\n"


def probar_rtsp(ip: str, puerto: int, ruta: str, usuario: str, clave: str, espera: float = 4) -> str:
    """¿Contesta la cámara y acepta el código? Un DESCRIBE, como el que hace el reproductor, sin
    traerse el video. «ok», «codigo» (no aceptó el código), «sin_conexion» o «error»."""
    host = f"[{ip}]" if ":" in ip else ip
    url = f"rtsp://{host}:{puerto}{ruta}"
    try:
        with socket.create_connection((ip, puerto), timeout=espera) as s:
            s.settimeout(espera)
            codigo, cab = _rtsp(s, "DESCRIBE", url, 1)
            if codigo == 401 and cab.get("www-authenticate"):
                codigo, cab = _rtsp(s, "DESCRIBE", url, 2,
                                    _autorizacion(cab["www-authenticate"], "DESCRIBE", url, usuario, clave))
    except OSError:
        return "sin_conexion"
    if codigo == 200:
        return "ok"
    if codigo in (401, 403):
        return "codigo"
    return "error" if codigo else "sin_conexion"


def escanear(puertos: tuple[int, ...] = (37777, 554), espera: float = 0.4) -> list[dict]:
    """Plan B: las IP de tu red /24 que tienen abierto el puerto de Dahua o el de RTSP."""
    propia = ip_local()
    if not propia:
        return []
    prefijo = propia.rsplit(".", 1)[0]
    ips = [f"{prefijo}.{n}" for n in range(1, 255) if f"{prefijo}.{n}" != propia]

    def probar(ip: str) -> dict | None:
        abiertos = [p for p in puertos if abierto(ip, p, espera)]
        return {"serie": "", "ip": ip, "modelo": "", "mac": "", "version": ""} if abiertos else None

    with ThreadPoolExecutor(max_workers=64) as pool:
        return [c for c in pool.map(probar, ips) if c]
