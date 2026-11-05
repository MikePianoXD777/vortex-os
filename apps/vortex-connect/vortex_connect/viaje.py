"""El modo viaje: corre aparte de la ventana, sin Qt. Se despierta cuando cambia la red y hace una vuelta de
`Connect.revisar`. Solo corre si la persona lo prendió.

En Linux lo despierta «nmcli monitor», así que no gasta nada mientras no cambie la red; Windows no tiene eso y
revisa cada 20 s.
"""
from __future__ import annotations

import shutil
import subprocess
import sys
import threading
import time
import webbrowser

from . import APP_ID, ES_WINDOWS, NOMBRE
from .motor import Ajustes, Candado, Connect, PROTEGIDO, SOLO_DNS

SERVICIO = "vortex-connect-viaje.service"
CALMA_S = 3             # una conexión dispara varios cambios seguidos
REPASO_S = 60
ENFRIAR_INTENTO_S = 300
_SIN_VENTANA = subprocess.CREATE_NO_WINDOW if ES_WINDOWS else 0


class _AvisoPortal:
    """El aviso del portal se cierra solo al pasar el portal: si no, seguiría pidiendo algo que ya está hecho."""
    id = ""

    @classmethod
    def cerrar(cls) -> None:
        previo, cls.id = cls.id, ""
        if not previo or not shutil.which("gdbus"):
            return
        # Cerrarlo también termina al notify-send que espera el botón.
        subprocess.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.Notifications",
                        "--object-path", "/org/freedesktop/Notifications",
                        "--method", "org.freedesktop.Notifications.CloseNotification", previo],
                       capture_output=True, timeout=5, check=False)


def notificar(texto: str, urgente: bool = False) -> None:
    _AvisoPortal.cerrar()
    if ES_WINDOWS or not shutil.which("notify-send"):
        return
    subprocess.run(["notify-send", "-a", NOMBRE, "-i", APP_ID, "-u", "critical" if urgente else "normal", NOMBRE, texto],
                   capture_output=True, timeout=10, check=False)


def ofrecer_portal(url: str) -> None:
    """Nunca abre el navegador por su cuenta en Linux: quizá no estás viendo, y el botón está en el aviso."""
    if ES_WINDOWS or not shutil.which("notify-send"):
        webbrowser.open(url)
        return
    _AvisoPortal.cerrar()

    def preguntar() -> None:
        try:
            p = subprocess.Popen(["notify-send", "-p", "-a", NOMBRE, "-i", APP_ID, "-u", "critical",
                                  "-A", "default=Ver", "-A", "abrir=Abrir la página",
                                  "Esta red pide aceptar algo",
                                  "Ábrela para tener internet. Lo que escribas ahí no va cifrado."],
                                 stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, errors="replace")
        except OSError:
            return
        with p:
            _AvisoPortal.id = p.stdout.readline().strip()   # con -p, lo primero es el número del aviso
            accion = p.stdout.readline().strip()
        if accion in ("abrir", "default"):
            webbrowser.open(url)

    threading.Thread(target=preguntar, daemon=True).start()


def _despertar_con_nmcli(despierta: threading.Event) -> subprocess.Popen | None:
    try:
        p = subprocess.Popen(["nmcli", "monitor"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, errors="replace")
    except OSError:
        return None

    def leer() -> None:
        for _ in p.stdout:
            despierta.set()

    threading.Thread(target=leer, daemon=True).start()
    return p


def main() -> int:
    ajustes = Ajustes.leer()
    if not ajustes.modo_viaje or not ajustes.condiciones:
        return 0
    dicho = ""

    def avisar(texto: str) -> None:
        nonlocal dicho
        if texto != dicho:
            dicho = texto
            if sys.stdout:      # en Windows, sin consola, no hay
                print(texto, flush=True)

    connect = Connect(ajustes=ajustes, avisar=avisar, abrir_portal=ofrecer_portal)
    despierta = threading.Event()
    despierta.set()     # la primera vuelta, de una
    monitor = None if ES_WINDOWS else _despertar_con_nmcli(despierta)
    candado = Candado()
    ultimo_intento = -ENFRIAR_INTENTO_S
    ultimo_final = ""
    try:
        while True:
            despierta.wait(20 if monitor is None else REPASO_S)
            despierta.clear()
            time.sleep(CALMA_S)
            despierta.clear()
            connect.ajustes = Ajustes.leer()
            if not connect.ajustes.modo_viaje:
                return 0
            if time.monotonic() - ultimo_intento < ENFRIAR_INTENTO_S and ultimo_final not in (PROTEGIDO, SOLO_DNS):
                continue    # el intento anterior no pudo: no insistir con cada cambio de la red
            if not candado.tomar():
                continue    # la ventana está trabajando
            try:
                resultado = connect.revisar()
            finally:
                candado.soltar()
            if resultado is None:
                continue
            ultimo_intento, ultimo_final = time.monotonic(), resultado.estado
            nombre = resultado.red.ssid or "esta red"
            if resultado.estado == PROTEGIDO:
                notificar(f"Entraste a «{nombre}» y lo que mandas va cifrado.")
            elif resultado.estado == SOLO_DNS:
                notificar(f"En «{nombre}» solo va cifrado el DNS: esa red bloquea lo demás.", urgente=True)
    finally:
        if monitor:
            monitor.terminate()


def prender(si: bool) -> bool:
    """Lo deja corriendo y que arranque solo al entrar a tu sesión, o lo apaga del todo."""
    if ES_WINDOWS:
        return _prender_windows(si)
    orden = ["systemctl", "--user", "enable" if si else "disable", "--now", SERVICIO]
    try:
        return subprocess.run(orden, capture_output=True, timeout=20, check=False).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def _prender_windows(si: bool) -> bool:
    import winreg
    clave_ruta = r"Software\Microsoft\Windows\CurrentVersion\Run"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, clave_ruta, 0, winreg.KEY_SET_VALUE) as clave:
            if si:
                winreg.SetValueEx(clave, "VortexineConnect", 0, winreg.REG_SZ, f'"{sys.executable}" vigilar')
            else:
                try:
                    winreg.DeleteValue(clave, "VortexineConnect")
                except FileNotFoundError:
                    pass
    except OSError:
        return False
    if si:
        subprocess.Popen([sys.executable, "vigilar"], creationflags=_SIN_VENTANA | subprocess.DETACHED_PROCESS)
    return True      # al apagarlo, el que corre se entera solo: lee los ajustes en cada vuelta
