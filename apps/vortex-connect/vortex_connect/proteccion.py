"""La protección: habla con el cliente oficial de Cloudflare WARP (warp-cli). Sin Qt.

--accept-tos dice a warp-cli que ya se aceptaron las condiciones de Cloudflare, así que solo se usa si la persona
las aceptó en la ventana (`ajustes.condiciones`); sin eso, ningún comando llega a warp-cli.

En Linux el servicio de WARP no corre en reposo: se prende cuando hace falta y se apaga al quitar la protección.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import time
import urllib.error
import urllib.request

from . import ES_WINDOWS

TRAZA = os.environ.get("VORTEX_CONNECT_TRAZA", "https://www.cloudflare.com/cdn-cgi/trace")
_SIN_VENTANA = subprocess.CREATE_NO_WINDOW if ES_WINDOWS else 0
_WARP_WINDOWS = r"C:\Program Files\Cloudflare\Cloudflare WARP\warp-cli.exe"
ESPERA_CIFRADO_S = 40


class ErrorProteccion(Exception):
    """Algo que se le puede decir a la persona tal cual."""


def _ruta_warp() -> str:
    if ES_WINDOWS and os.path.exists(_WARP_WINDOWS):
        return _WARP_WINDOWS
    return shutil.which("warp-cli") or ""


def hay_tailscale() -> bool:
    """Con el túnel prendido, el firewall de WARP en Linux solo deja salir lo suyo y las redes privadas: lo que
    Tailscale manda hacia afuera no pasa. En Windows no se ha comprobado, así que ahí no se avisa."""
    return not ES_WINDOWS and os.path.exists("/sys/class/net/tailscale0")


def traza_dice_warp(timeout: float = 8) -> bool:
    """Lo que ve Cloudflare es la prueba de verdad: «warp=on» o no va cifrado."""
    try:
        with urllib.request.urlopen(TRAZA, timeout=timeout) as r:
            texto = r.read(4096).decode(errors="replace")
    except (OSError, urllib.error.URLError, ValueError):
        return False
    return any(l.strip() in ("warp=on", "warp=plus") for l in texto.splitlines())


class Warp:
    def __init__(self, traza=traza_dice_warp):
        self._traza = traza

    def instalado(self) -> bool:
        return bool(_ruta_warp())

    def _cli(self, *args: str, timeout: float = 25) -> tuple[int, str]:
        ruta = _ruta_warp()
        if not ruta:
            return 127, ""
        try:
            r = subprocess.run([ruta, "--accept-tos", *args], capture_output=True, text=True, errors="replace",
                               timeout=timeout, stdin=subprocess.DEVNULL, creationflags=_SIN_VENTANA)
        except OSError:
            return 127, ""
        except subprocess.TimeoutExpired:
            return 124, ""
        return r.returncode, (r.stdout + r.stderr).strip()

    def _servicio(self, verbo: str) -> bool:
        if ES_WINDOWS:
            return True     # en Windows es un servicio de Cloudflare que ya corre
        try:
            return subprocess.run(["systemctl", "--no-ask-password", verbo, "warp-svc.service"],
                                  capture_output=True, timeout=30).returncode == 0
        except (OSError, subprocess.TimeoutExpired):
            return False

    def _servicio_activo(self) -> bool:
        if ES_WINDOWS:
            return True
        try:
            return subprocess.run(["systemctl", "is-active", "--quiet", "warp-svc.service"], timeout=10).returncode == 0
        except (OSError, subprocess.TimeoutExpired):
            return False

    def conectado(self) -> bool:
        """Sin tocar nada: en reposo el servicio está apagado y preguntar no debe prenderlo."""
        if not self.instalado() or not self._servicio_activo():
            return False
        return "Connected" in self._cli("status", timeout=8)[1]

    def _esperar_servicio(self) -> None:
        if not self._servicio_activo() and not self._servicio("start"):
            raise ErrorProteccion("No se pudo prender el servicio de WARP.")
        for _ in range(20):     # recién prendido, tarda en contestar
            if self._cli("status", timeout=5)[0] == 0:
                return
            time.sleep(0.5)
        raise ErrorProteccion("El servicio de WARP no contesta.")

    def preparar(self) -> None:
        """Prende el servicio y registra este equipo la primera vez (gratis y sin cuenta)."""
        if not self.instalado():
            raise ErrorProteccion("Falta el cliente de Cloudflare WARP.")
        self._esperar_servicio()
        codigo, salida = self._cli("registration", "show")
        if codigo != 0 or "Missing" in salida:
            codigo, salida = self._cli("registration", "new", timeout=40)
            if codigo != 0:
                raise ErrorProteccion("No se pudo registrar en WARP. ¿Ya hay internet?")

    def _esperar_estado(self, timeout_s: float = 20) -> bool:
        fin = time.monotonic() + timeout_s
        while time.monotonic() < fin:
            salida = self._cli("status", timeout=5)[1]
            if "Connected" in salida:
                return True
            time.sleep(1)
        return False

    def _traza_confirma(self, intentos: int = 4) -> bool:
        """Recién conectado, «Connected» puede llegar antes de que el túnel deje pasar tráfico: se le da tiempo."""
        for intento in range(intentos):
            if intento:
                time.sleep(2)
            if self._traza():
                return True
        return False

    def activar_cifrado(self) -> bool:
        """Todo cifrado hasta Cloudflare. El protocolo lo elige WARP: forzarlo probando MASQUE y luego WireGuard
        tarda en aplicarse y, con el cliente real, dejó el túnel sin poder conectar en los intentos siguientes."""
        self._cli("disconnect")
        self._cli("mode", "warp")
        self._cli("connect")
        return self._esperar_estado(ESPERA_CIFRADO_S) and self._traza_confirma()

    def activar_solo_dns(self) -> bool:
        """El respaldo: solo van cifradas las búsquedas de nombres (qué sitios abres), no lo demás."""
        self._cli("disconnect")
        self._cli("mode", "doh")
        self._cli("connect")
        return self._esperar_estado(10)

    def modo(self) -> str:
        """«cifrado», «dns» o «». Mira lo que ya está prendido."""
        if not self.conectado():
            return ""
        if self._traza_confirma(2):     # una respuesta perdida no debe hacer creer que dejó de cifrar
            return "cifrado"
        codigo, salida = self._cli("settings", "list", timeout=8)
        # El texto real es «(user set)  Mode: DnsOverHttps». Hay otra línea, «DoH MSS clamp…», que sale en todos los modos.
        return "dns" if codigo == 0 and re.search(r"\bMode:\s*DnsOverHttps\b", salida) else ""

    def _enlaces_de_salida(self) -> list[str]:
        enlaces: dict[str, None] = {}
        for familia in ("-4", "-6"):
            try:
                salida = subprocess.run(["ip", familia, "-o", "route", "show", "default"], capture_output=True,
                                        text=True, errors="replace", timeout=5).stdout
            except (OSError, subprocess.TimeoutExpired):
                continue
            enlaces.update(dict.fromkeys(re.findall(r"\bdev (\S+)", salida)))
        return list(enlaces)

    def _dns_del_enlace(self, si: bool) -> bool:
        if ES_WINDOWS or not shutil.which("resolvectl"):
            return True
        bien = True
        for enlace in self._enlaces_de_salida():
            try:
                bien = subprocess.run(["resolvectl", "--no-ask-password", "default-route", enlace, "true" if si else "false"],
                                      capture_output=True, timeout=10).returncode == 0 and bien
            except (OSError, subprocess.TimeoutExpired):
                bien = False
        return bien

    def blindar_dns(self) -> bool:
        """WARP cambia resolv.conf, pero systemd-resolved le sigue preguntando también al DNS del wifi, y eso va en
        claro: quien controla la red vería el nombre de cada sitio. Se le quita al enlace mientras dure la protección."""
        return self._dns_del_enlace(False)

    def apagar(self) -> None:
        if self.instalado() and self._servicio_activo():
            self._cli("disconnect")
            self._servicio("stop")
        self._dns_del_enlace(True)
