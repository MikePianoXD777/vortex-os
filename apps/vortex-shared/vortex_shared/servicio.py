"""Escucha los avisos de otras compus. Lo arranca el socket de systemd (vortex-shared.socket) cuando alguien
se conecta y se apaga solo al terminar: en reposo no hay ningún proceso.

Sale un aviso para aceptar o rechazar. Nada se baja sin que aceptes, y nunca sale con un juego abierto.
"""
from __future__ import annotations

import getpass
import json
import os
import select
import shutil
import socket
import subprocess
import threading
import time
from pathlib import Path

from . import motor

ESPERA_RESPUESTA_S = float(os.environ.get("VORTEX_SHARED_ESPERA", "120"))
INACTIVO_S = float(os.environ.get("VORTEX_SHARED_INACTIVO", "45"))
APP = "Vortexine Shared"


def _run() -> Path:
    return Path(os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}")


def _socket_hyprland() -> Path | None:
    """El servicio no hereda la sesión: el Hyprland vivo es el del socket más nuevo."""
    candidatos = list((_run() / "hypr").glob("*/.socket2.sock"))
    return max(candidatos, key=lambda p: p.stat().st_mtime) if candidatos else None


def jugando() -> bool:
    """La marca de «vortex modo-juego», si es de esta sesión de Hyprland."""
    estado = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state") / "vortex" / "modo-juego"
    try:
        marca = estado.stat().st_mtime
    except OSError:
        return False
    hypr = _socket_hyprland()
    return hypr is None or marca > hypr.stat().st_mtime


def preguntar(titulo: str, cuerpo: str) -> bool:
    """El aviso con sus botones. Si nadie contesta a tiempo, se cierra y cuenta como no. Picarle a la tarjeta
    («default») no acepta nada: el aviso vuelve a salir."""
    if not shutil.which("notify-send"):
        return False
    fin = time.monotonic() + ESPERA_RESPUESTA_S
    while True:
        proc = subprocess.Popen(["notify-send", "-a", APP, "-i", "folder-download", "-u", "critical", "-p",
                                 "-A", "default=Detalles", "-A", "aceptar=Aceptar", "-A", "rechazar=Rechazar", titulo, cuerpo],
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        try:
            salida, _ = proc.communicate(timeout=max(0.1, fin - time.monotonic()))
        except subprocess.TimeoutExpired:
            proc.kill()
            salida, _ = proc.communicate()
            lineas = salida.split()
            if lineas and lineas[0].isdigit():
                try:
                    subprocess.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.Notifications",
                                    "--object-path", "/org/freedesktop/Notifications", "--method",
                                    "org.freedesktop.Notifications.CloseNotification", lineas[0]],
                                   capture_output=True, timeout=5)
                except (OSError, subprocess.TimeoutExpired):
                    pass
            return False
        palabras = salida.split()
        if "default" in palabras and time.monotonic() < fin:
            continue
        return "aceptar" in palabras


def avisar_listo(carpeta: Path, texto: str) -> None:
    if not shutil.which("notify-send"):
        return
    try:
        accion = subprocess.run(["notify-send", "-a", APP, "-i", "folder-download", "-A", "abrir=Abrir carpeta",
                                 "Ya llegó", texto], capture_output=True, text=True, timeout=300).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return
    if accion == "abrir":
        hypr = _socket_hyprland()
        entorno = dict(os.environ, HYPRLAND_INSTANCE_SIGNATURE=hypr.parent.name) if hypr else None
        ruta = str(carpeta).replace('"', '')
        for orden in (f'hl.dsp.exec_cmd("xdg-open \'{ruta}\'")', f"exec xdg-open '{ruta}'"):
            try:
                if subprocess.run(["hyprctl", "dispatch", orden], capture_output=True, timeout=3, env=entorno).returncode == 0:
                    return
            except (OSError, subprocess.TimeoutExpired):
                return


def _responder(conexion: socket.socket, **datos) -> bool:
    try:
        conexion.sendall((json.dumps(datos) + "\n").encode())
        return True
    except OSError:
        return False


def _leer_linea(conexion: socket.socket, espera_s: float = 10) -> bytes:
    conexion.settimeout(espera_s)
    datos = b""
    fin = time.monotonic() + espera_s
    while b"\n" not in datos and len(datos) <= motor.MAX_LINEA and time.monotonic() < fin:
        trozo = conexion.recv(4096)
        if not trozo:
            break
        datos += trozo
    return datos.split(b"\n", 1)[0] if b"\n" in datos else b""


class Servicio:
    def __init__(self, oyente: socket.socket):
        self.oyente = oyente
        self.ocupado = threading.Lock()
        self.ultimo = time.monotonic()
        self.hilos: list[threading.Thread] = []

    def atender(self, conexion: socket.socket, ip: str) -> None:
        with conexion:
            try:
                self._atender(conexion, ip)
            except OSError:
                pass
            finally:
                self.ultimo = time.monotonic()

    def _atender(self, conexion: socket.socket, ip: str) -> None:
        if not motor.ip_permitida(ip):
            return
        try:
            peticion = motor.validar_peticion(json.loads(_leer_linea(conexion)))
        except (ValueError, UnicodeDecodeError):
            _responder(conexion, r="error")
            return
        if peticion["usuario"] != getpass.getuser():
            _responder(conexion, r="usuario")
            return
        if jugando() or not self.ocupado.acquire(blocking=False):
            _responder(conexion, r="ocupado")
            return
        try:
            self._recibir(conexion, peticion, ip)
        finally:
            self.ocupado.release()

    def _recibir(self, conexion: socket.socket, peticion: dict, ip: str) -> None:
        carpeta = motor.carpeta_recibidos()
        try:
            carpeta.mkdir(parents=True, exist_ok=True)
            libre = shutil.disk_usage(carpeta).free
        except OSError:
            libre = 0
        if libre < peticion["tam"] * 2:   # mientras baja, sendme tiene su copia y la que deja
            _responder(conexion, r="espacio")
            return
        if not preguntar(f"{peticion['de']} te quiere mandar archivos", f"{motor.texto_peticion(peticion)}. Desde {ip.removeprefix('::ffff:')}."):
            _responder(conexion, r="rechazado")
            return
        if not _responder(conexion, r="aceptado"):
            return
        conexion.settimeout(5)
        vivo = {"ok": True, "ultimo": 0.0}

        def avance(n: int) -> None:
            ahora = time.monotonic()
            if ahora - vivo["ultimo"] >= 1:
                vivo["ultimo"] = ahora
                vivo["ok"] = _responder(conexion, r="avance", bytes=n) and vivo["ok"]

        try:
            quedaron = motor.recibir(peticion["ticket"], avance, lambda: not vivo["ok"], peticion["tam"])
        except motor.ErrorShared:
            _responder(conexion, r="error")
            return
        _responder(conexion, r="listo")
        conexion.close()
        nombre = quedaron[0].name if len(quedaron) == 1 else f"{quedaron[0].name} y {len(quedaron) - 1} más"
        avisar_listo(carpeta, motor.limpiar_nombre(f"{nombre}, en Descargas/{carpeta.name}"))

    def correr(self) -> None:
        self.oyente.setblocking(False)
        while True:
            self.hilos = [h for h in self.hilos if h.is_alive()]
            listos, _, _ = select.select([self.oyente], [], [], 1)
            if listos:
                try:
                    conexion, direccion = self.oyente.accept()
                except OSError:
                    continue
                conexion.setblocking(True)
                self.ultimo = time.monotonic()
                hilo = threading.Thread(target=self.atender, args=(conexion, direccion[0]), daemon=True)
                hilo.start()
                self.hilos.append(hilo)
            elif not self.hilos and time.monotonic() - self.ultimo > INACTIVO_S:
                return


def oyente_de_systemd() -> socket.socket:
    """El socket que abrió systemd (LISTEN_FDS, el descriptor 3)."""
    if os.environ.get("LISTEN_PID") == str(os.getpid()) and os.environ.get("LISTEN_FDS") == "1":
        return socket.socket(fileno=3)
    raise SystemExit("vortex-shared escuchar lo arranca vortex-shared.socket (systemd).")


def main() -> int:
    Servicio(oyente_de_systemd()).correr()
    return 0
