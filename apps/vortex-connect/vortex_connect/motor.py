"""Lo que hace Connect, sin Qt: entrar a una red, pasar el portal y proteger. La ventana y el modo viaje usan esto.

El orden importa: una red con portal no da internet hasta aceptar, y WARP no puede salir antes. Por eso primero se
entra, luego se pregunta si hay internet o portal, y solo al final se cifra.
"""
from __future__ import annotations

import json
import os
import threading
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

from . import carpeta_config
from .proteccion import ErrorProteccion, Warp
from .redes import Actual, ErrorRed, crear as crear_redes

# Da 204 si hay internet. Va sin https a propósito: un portal solo puede interceptar lo que no va cifrado.
PRUEBA = os.environ.get("VORTEX_CONNECT_PRUEBA", "http://cp.cloudflare.com/generate_204")
ESPERA_PORTAL_S = 300
ENFRIAR_FALLIDA_S = 600


class _SinRedirigir(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


def probar_internet(timeout: float = 6) -> tuple[str, str]:
    """(«internet», ""), («portal», dirección del portal) o («nada», "")."""
    abridor = urllib.request.build_opener(_SinRedirigir, urllib.request.ProxyHandler({}))
    try:
        with abridor.open(PRUEBA, timeout=timeout) as r:
            return ("internet", "") if r.status == 204 else ("portal", PRUEBA)
    except urllib.error.HTTPError as e:
        destino = e.headers.get("Location") or PRUEBA
        e.close()
        return "portal", destino if destino.startswith(("http://", "https://")) else PRUEBA
    except (OSError, urllib.error.URLError, ValueError):
        return "nada", ""


# --- Ajustes ---

@dataclass
class Ajustes:
    condiciones: bool = False   # la persona aceptó las condiciones de Cloudflare en la ventana
    modo_viaje: bool = False
    protegio_connect: bool = False   # si la prendió Connect, Connect la apaga al salir de la red abierta

    @staticmethod
    def ruta() -> Path:
        return carpeta_config() / "connect.json"

    @classmethod
    def leer(cls) -> "Ajustes":
        try:
            d = json.loads(cls.ruta().read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return cls()
        if not isinstance(d, dict):
            return cls()
        return cls(d.get("condiciones") is True, d.get("modo_viaje") is True, d.get("protegio_connect") is True)

    def guardar(self) -> None:
        ruta = self.ruta()
        try:
            ruta.parent.mkdir(parents=True, exist_ok=True)
            temporal = ruta.with_suffix(".tmp")
            temporal.write_text(json.dumps(self.__dict__), encoding="utf-8")
            os.replace(temporal, ruta)
        except OSError:
            pass


# --- Un solo trabajo a la vez, entre la ventana y el modo viaje ---

class Candado:
    def __init__(self):
        self._ruta = carpeta_config() / "connect.candado"
        self._f = None

    def tomar(self) -> bool:
        try:
            self._ruta.parent.mkdir(parents=True, exist_ok=True)
            self._f = open(self._ruta, "a+b")
            if os.name == "nt":
                import msvcrt
                self._f.seek(0)
                msvcrt.locking(self._f.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self._f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except OSError:
            self.soltar()
            return False

    def soltar(self) -> None:
        if self._f:
            self._f.close()     # cerrar suelta el candado en los dos sistemas
            self._f = None


# --- El estado ---

SIN_RED, PORTAL, SIN_PROTECCION, SOLO_DNS, PROTEGIDO, RED_DE_CONFIANZA = (
    "sin_red", "portal", "sin_proteccion", "solo_dns", "protegido", "confianza")


@dataclass
class Estado:
    estado: str = SIN_RED
    red: Actual = field(default_factory=Actual)
    portal: str = ""


class Connect:
    """`avisar(texto)` cuenta lo que va pasando; `abrir_portal(url)` le enseña el portal a la persona."""

    def __init__(self, redes=None, warp=None, probar=probar_internet, dormir=time.sleep, ajustes: Ajustes | None = None,
                 avisar=lambda _t: None, abrir_portal=lambda _u: None):
        self.redes = redes or crear_redes()
        self.warp = warp or Warp()
        self.probar = probar
        self.dormir = dormir
        self.ajustes = ajustes or Ajustes.leer()
        self.avisar = avisar
        self.abrir_portal = abrir_portal
        self._fallidas: dict[str, float] = {}

    def estado(self) -> Estado:
        red = self.redes.actual()
        modo = self.warp.modo() if self.ajustes.condiciones else ""
        if modo == "cifrado":
            return Estado(PROTEGIDO, red)
        if not red.tipo:
            return Estado(SIN_RED, red)
        if modo == "dns":
            return Estado(SOLO_DNS, red)
        conexion, portal = self.probar()
        if conexion == "portal":
            return Estado(PORTAL, red, portal)
        if conexion == "nada":
            return Estado(SIN_RED, red)
        return Estado(SIN_PROTECCION if red.abierta else RED_DE_CONFIANZA, red)

    def conectar(self, ssid: str, cancelar: threading.Event | None = None) -> Estado:
        """Entra a esa red y la protege. Lanza ErrorRed si no se puede entrar."""
        cancelar = cancelar or threading.Event()
        if self.ajustes.condiciones and self.warp.conectado():
            self.avisar("Pausando la protección para entrar…")   # un túnel prendido no deja ver el portal
            self.warp.apagar()
        self.avisar(f"Entrando a «{ssid}»…")
        self.redes.conectar(ssid)
        return self.asegurar(cancelar)

    def asegurar(self, cancelar: threading.Event | None = None) -> Estado:
        """Ya dentro de una red: pasa el portal si lo hay y luego protege."""
        cancelar = cancelar or threading.Event()
        if not self._pasar_portal(cancelar):
            return self.estado()
        return self.proteger()

    def _pasar_portal(self, cancelar: threading.Event) -> bool:
        conexion, portal = self.probar()
        if conexion == "internet":
            return True
        if conexion == "nada":
            self.dormir(3)      # el DHCP puede tardar tras entrar
            conexion, portal = self.probar()
            if conexion == "internet":
                return True
            if conexion == "nada":
                self.avisar("Esta red no da internet.")
                return False
        self.avisar("Esta red pide aceptar algo. Lo que escribas en esa página no va cifrado.")
        self.abrir_portal(portal)
        fin = time.monotonic() + ESPERA_PORTAL_S
        while time.monotonic() < fin and not cancelar.is_set():
            self.dormir(3)
            if self.probar()[0] == "internet":
                return True
        self.avisar("Sigue sin internet: la página de la red no se aceptó.")
        return False

    def proteger(self) -> Estado:
        if not self.ajustes.condiciones:
            self.avisar("Falta aceptar las condiciones de Cloudflare para cifrar.")
            return self.estado()
        self.avisar("Cifrando tu conexión…")
        try:
            self.warp.preparar()
        except ErrorProteccion as e:
            self.avisar(str(e))
            return self.estado()
        if self.warp.activar_cifrado():
            self._marcar(True)
            self.avisar("Protegido: lo que mandas va cifrado." if self.warp.blindar_dns() else
                        "Lo que mandas va cifrado, pero esta red aún puede ver los nombres de los sitios que abres.")
            return Estado(PROTEGIDO, self.redes.actual())
        self.avisar("Esta red bloquea el cifrado completo. Probando con el DNS cifrado…")
        if self.warp.activar_solo_dns() and self.warp.blindar_dns():
            self._marcar(True)
            self.avisar("Solo el DNS va cifrado: esta red bloquea lo demás. Evita entrar a tu banco aquí.")
            return Estado(SOLO_DNS, self.redes.actual())
        self.warp.apagar()
        self._marcar(False)
        self.avisar("No se pudo cifrar en esta red. Lo que mandas va sin protección.")
        return self.estado()

    def quitar(self) -> Estado:
        self.warp.apagar()
        self._marcar(False)
        self.avisar("Protección apagada.")
        return self.estado()

    def _marcar(self, si: bool) -> None:
        if self.ajustes.protegio_connect != si:
            self.ajustes.protegio_connect = si
            self.ajustes.guardar()

    def revisar(self, cancelar: threading.Event | None = None) -> Estado | None:
        """Una vuelta del modo viaje. Solo actúa si hace falta; devuelve None si no hizo nada."""
        red = self.redes.actual()
        if red.tipo == "cable" or (red.tipo == "wifi" and not red.abierta):
            # De vuelta en casa: la protección que prendió Connect ya no toca (y deja en paz a Tailscale).
            if self.ajustes.protegio_connect:
                return self.quitar()
            return None
        if red.tipo == "wifi":
            if self.ajustes.condiciones and self.warp.modo():
                self.warp.blindar_dns()     # si el wifi se reconectó, NetworkManager le devolvió su DNS al enlace
                return None
            return self.asegurar(cancelar)
        if self.ajustes.condiciones and self.warp.conectado():
            self.warp.apagar()      # sin red, un túnel prendido solo estorba para la siguiente
        ahora = time.monotonic()
        for candidata in self.redes.lista():
            if ahora - self._fallidas.get(candidata.ssid, -ENFRIAR_FALLIDA_S) < ENFRIAR_FALLIDA_S:
                continue
            try:
                return self.conectar(candidata.ssid, cancelar)
            except ErrorRed:
                self._fallidas[candidata.ssid] = ahora
        return None
