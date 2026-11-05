"""Las redes wifi: cuáles abiertas hay cerca, en cuál estás y cómo entrar a una. Sin Qt.

Linux usa nmcli; Windows, netsh. Las dos dan lo mismo al motor. Nunca se tocan redes con contraseña: solo se
listan las abiertas, y de la red actual solo se pregunta si es abierta.
"""
from __future__ import annotations

import getpass
import os
import re
import subprocess
import tempfile
from dataclasses import dataclass
from xml.sax.saxutils import escape

from . import ES_WINDOWS

PREFIJO = "Vortexine Connect: "
_SIN_VENTANA = subprocess.CREATE_NO_WINDOW if ES_WINDOWS else 0


class ErrorRed(Exception):
    """Algo que se le puede decir a la persona tal cual."""


@dataclass(frozen=True)
class Red:
    ssid: str
    senal: int          # 0-100
    conectada: bool = False


@dataclass(frozen=True)
class Actual:
    tipo: str = ""      # «wifi», «cable» o «» si no hay nada
    ssid: str = ""
    abierta: bool = False


def _correr(orden: list[str], timeout: float) -> tuple[int, str, str]:
    try:
        r = subprocess.run(orden, capture_output=True, timeout=timeout, creationflags=_SIN_VENTANA)
    except OSError as e:
        return 127, "", str(e)
    except subprocess.TimeoutExpired:
        return 124, "", "tardó demasiado"
    # netsh escribe en la página de códigos de la consola; lo que no se entienda no debe romper la lista.
    codec = "oem" if ES_WINDOWS else "utf-8"
    return r.returncode, r.stdout.decode(codec, errors="replace"), r.stderr.decode(codec, errors="replace")


def partir_nmcli(linea: str) -> list[str]:
    """Una línea de «nmcli -t»: «:» separa y «\\:» es un dos puntos de verdad (un SSID puede traerlo)."""
    campos, actual, i = [], "", 0
    while i < len(linea):
        c = linea[i]
        if c == "\\" and i + 1 < len(linea):
            actual += linea[i + 1]
            i += 2
            continue
        if c == ":":
            campos.append(actual)
            actual = ""
        else:
            actual += c
        i += 1
    campos.append(actual)
    return campos


def _unir(filas: list[Red]) -> list[Red]:
    """Una fila por nombre (la de mejor señal: un hotel tiene muchas antenas) y las más fuertes primero."""
    mejores: dict[str, Red] = {}
    for f in filas:
        previa = mejores.get(f.ssid)
        if previa is None or f.senal > previa.senal:
            mejores[f.ssid] = Red(f.ssid, f.senal, f.conectada or bool(previa and previa.conectada))
        elif f.conectada:
            mejores[f.ssid] = Red(previa.ssid, previa.senal, True)
    return sorted(mejores.values(), key=lambda r: (-r.senal, r.ssid.lower()))


class RedesLinux:
    def __init__(self, correr=_correr):
        self._correr = correr

    def _filas(self, buscar: bool) -> list[tuple[bool, str, int, str]]:
        orden = ["nmcli", "-t", "-e", "yes", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"]
        if buscar:
            orden += ["--rescan", "yes"]
        else:
            orden += ["--rescan", "no"]
        codigo, salida, _ = self._correr(orden, 30)
        if codigo != 0:
            return []
        filas = []
        for linea in salida.splitlines():
            campos = partir_nmcli(linea)
            if len(campos) < 4 or not campos[1]:
                continue    # sin nombre: red oculta
            try:
                senal = int(campos[2])
            except ValueError:
                senal = 0
            filas.append((campos[0].strip() == "*", campos[1], senal, campos[3].strip()))
        return filas

    def lista(self, buscar: bool = True) -> list[Red]:
        return _unir([Red(ssid, senal, usa) for usa, ssid, senal, seg in self._filas(buscar) if seg in ("", "--")])

    def actual(self) -> Actual:
        codigo, salida, _ = self._correr(["nmcli", "-t", "-f", "TYPE,STATE", "device"], 10)
        if codigo != 0:
            return Actual()
        conectados = [partir_nmcli(l) for l in salida.splitlines()]
        tipos = {c[0] for c in conectados if len(c) >= 2 and c[1] == "connected"}
        if "ethernet" in tipos:
            return Actual("cable")
        if "wifi" not in tipos:
            return Actual()
        for usa, ssid, _, seg in self._filas(False):
            if usa:
                return Actual("wifi", ssid, seg in ("", "--"))
        return Actual("wifi")

    def hay_wifi(self) -> bool:
        codigo, salida, _ = self._correr(["nmcli", "-t", "-f", "TYPE", "device"], 10)
        return codigo == 0 and "wifi" in salida.splitlines()

    def conectar(self, ssid: str) -> None:
        """Con un perfil propio: MAC al azar y sin volver a entrar solo, para que otra red que se llame igual
        no te atrape después. Si ya existe, se reutiliza."""
        perfil = PREFIJO + ssid
        codigo, salida, _ = self._correr(["nmcli", "-t", "-f", "NAME", "connection", "show"], 10)
        existentes = {partir_nmcli(l)[0] for l in salida.splitlines()} if codigo == 0 else set()
        if perfil not in existentes:
            codigo, _, error = self._correr([
                "nmcli", "connection", "add", "type", "wifi", "con-name", perfil, "ssid", ssid,
                "connection.autoconnect", "no", "wifi.cloned-mac-address", "random",
                "connection.permissions", f"user:{getpass.getuser()}",
            ], 15)
            if codigo != 0:
                raise ErrorRed(f"No pude preparar «{ssid}»: {error.strip() or 'nmcli falló'}")
        codigo, _, error = self._correr(["nmcli", "--wait", "40", "connection", "up", perfil], 45)
        if codigo != 0:
            self._correr(["nmcli", "connection", "delete", perfil], 10)
            raise ErrorRed(f"No se pudo entrar a «{ssid}». Puede estar muy lejos.")


# --- Windows ---

_AUTENTICACION = re.compile(r"^\s*(?:Authentication|Autenticaci\S*n)\s*:\s*(.+?)\s*$", re.I | re.M)
_ABIERTA = re.compile(r"^(?:Open|Abierta|Abierto)$", re.I)
_SIN_CIFRAR = re.compile(r"^\s*(?:Encryption|Cifrado)\s*:\s*(?:None|Ninguno)\s*$", re.I | re.M)


def bloques_netsh(salida: str) -> list[tuple[str, str]]:
    """«SSID 1 : nombre» y lo que sigue hasta el próximo. La salida viene traducida; el SSID es igual siempre."""
    partes = re.split(r"^\s*SSID\s+\d+\s*:[ \t]*(.*)$", salida, flags=re.M)
    return [(partes[i].strip(), partes[i + 1]) for i in range(1, len(partes) - 1, 2)]


def es_abierta_netsh(bloque: str) -> bool:
    auth = _AUTENTICACION.search(bloque)
    return bool(auth and _ABIERTA.match(auth.group(1)) and _SIN_CIFRAR.search(bloque))


def perfil_windows(ssid: str) -> str:
    """Perfil de red abierta, en manual: Windows no vuelve a entrar solo a otra que se llame igual."""
    nombre = escape(ssid)
    return f"""<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{nombre}</name>
  <SSIDConfig><SSID><name>{nombre}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>manual</connectionMode>
  <MSM><security>
    <authEncryption><authentication>open</authentication><encryption>none</encryption><useOneX>false</useOneX></authEncryption>
  </security></MSM>
</WLANProfile>
"""


class RedesWindows:
    def __init__(self, correr=_correr):
        self._correr = correr

    def _interfaz(self) -> str:
        codigo, salida, _ = self._correr(["netsh", "wlan", "show", "interfaces"], 10)
        return salida if codigo == 0 else ""

    def _ssid_actual(self) -> str:
        m = re.search(r"^\s*SSID\s*:[ \t]*(.+?)\s*$", self._interfaz(), re.M)
        return m.group(1) if m else ""

    def lista(self, buscar: bool = True) -> list[Red]:
        codigo, salida, error = self._correr(["netsh", "wlan", "show", "networks", "mode=bssid"], 20)
        if codigo != 0:
            # Windows 11 pide el permiso de ubicación para ver redes wifi.
            if "ubicaci" in (salida + error).lower() or "location" in (salida + error).lower():
                raise ErrorRed("Windows pide activar Ubicación para ver las redes wifi (Configuración → Privacidad).")
            return []
        actual = self._ssid_actual()
        filas = []
        for ssid, bloque in bloques_netsh(salida):
            if not ssid or not es_abierta_netsh(bloque):
                continue
            senales = [int(s) for s in re.findall(r":\s*(\d{1,3})\s*%", bloque)]
            filas.append(Red(ssid, max(senales, default=0), ssid == actual))
        return _unir(filas)

    def actual(self) -> Actual:
        texto = self._interfaz()
        ssid = self._ssid_actual()
        if ssid:
            auth = _AUTENTICACION.search(texto)
            return Actual("wifi", ssid, bool(auth and _ABIERTA.match(auth.group(1))))
        # Solo adaptadores físicos: el loopback y los virtuales de WARP o Tailscale siempre están arriba.
        codigo, salida, _ = self._correr(["powershell", "-NoProfile", "-Command",
                                          "@(Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' -and $_.PhysicalMediaType -ne 'Native 802.11' }).Count"], 15)
        return Actual("cable") if codigo == 0 and salida.strip() not in ("", "0") else Actual()

    def hay_wifi(self) -> bool:
        # Sin antena o sin el servicio de wifi, netsh falla o no enseña ninguna interfaz con nombre.
        return bool(re.search(r"^\s*(?:Name|Nombre)\s*:", self._interfaz(), re.I | re.M))

    def conectar(self, ssid: str) -> None:
        with tempfile.NamedTemporaryFile("w", suffix=".xml", delete=False, encoding="utf-8") as f:
            f.write(perfil_windows(ssid))
        try:
            codigo, _, _ = self._correr(["netsh", "wlan", "add", "profile", f"filename={f.name}", "user=current"], 15)
            if codigo != 0:
                raise ErrorRed(f"No pude preparar «{ssid}».")
            codigo, _, _ = self._correr(["netsh", "wlan", "connect", f"name={ssid}", f"ssid={ssid}"], 20)
            if codigo != 0:
                raise ErrorRed(f"No se pudo entrar a «{ssid}». Puede estar muy lejos.")
        finally:
            try:
                os.unlink(f.name)
            except OSError:
                pass


def crear():
    return RedesWindows() if ES_WINDOWS else RedesLinux()
