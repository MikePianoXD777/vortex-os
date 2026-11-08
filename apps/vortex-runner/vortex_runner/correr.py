"""Correr un programa de Windows con umu-run y Proton.

umu-run es el lanzador de Heroic y Lutris: mete a Proton en el runtime de Steam (el mismo contenedor
que usa Steam), así que los juegos corren igual que en Steam aunque no estén en Steam. La primera vez
baja el runtime (unos 500 MB) a ~/.local/share/umu.

Variables que recibe umu-run: WINEPREFIX (el prefijo del programa), PROTONPATH (la carpeta del Proton
o «GE-Proton» para que lo baje solo), GAMEID (umu-default: sin arreglos de juegos específicos) y
STORE=none.

Para saber si un programa sigue abierto se buscan los procesos tuyos con su WINEPREFIX (lee
/proc/<pid>/environ): así se ve también el que abriste desde el lanzador, y «Detener» los cierra todos.

Para pruebas: VORTEX_RUNNER_UMU=/ruta/a/un/umu-run falso.
"""
from __future__ import annotations

import os
import shlex
import shutil
import signal
import subprocess
import time
from pathlib import Path

from . import protones, rutas

UMU = "umu-run"
PAQUETES = ("umu-launcher", "proton-cachyos-slr")


def umu() -> str | None:
    propio = os.environ.get("VORTEX_RUNNER_UMU")
    if propio:
        return propio if os.access(propio, os.X_OK) else None
    return shutil.which(UMU)


def falta_motor() -> list[str]:
    """Los paquetes del sistema que faltan para correr programas (vacío si está todo)."""
    falta = []
    if not umu():
        falta.append("umu-launcher")
    if not any(p["id"].startswith("proton-cachyos") for p in protones.lista()):
        falta.append("proton-cachyos-slr")
    return falta


def instalar_motor() -> subprocess.CompletedProcess:
    """Instala lo que falte con pacman (pide tu contraseña en una ventana: pkexec)."""
    falta = falta_motor()
    if not falta:
        return subprocess.CompletedProcess([], 0, "", "")
    orden = [os.environ.get("VORTEX_RUNNER_PKEXEC", "pkexec"), "pacman", "-S", "--needed", "--noconfirm", *falta]
    return subprocess.run(orden, capture_output=True, text=True, timeout=3600)


def entorno(e: dict) -> dict[str, str]:
    p = protones.buscar(e.get("proton") or None)
    env = dict(os.environ)
    env.update({
        "WINEPREFIX": str(e["prefijo"]),
        "PROTONPATH": p["ruta"],
        "GAMEID": env.get("VORTEX_RUNNER_GAMEID", "umu-default"),
        "STORE": "none",
        # Para que el gestor de Vortexine sepa qué programa es (modo juego, Task Manager)
        "VORTEX_RUNNER_ID": e["id"],
    })
    return env


def orden(e: dict, exe: str | None = None, extra: list[str] | None = None) -> list[str]:
    ejecutable = umu() or UMU
    exe = exe or e["exe"]
    if exe.lower().endswith(".msi"):
        return [ejecutable, "msiexec", "/i", exe]
    args = shlex.split(e.get("argumentos") or "") if exe == e["exe"] else []
    return [ejecutable, exe, *args, *(extra or [])]


def _registro(e: dict) -> Path:
    r = rutas.registros()
    r.mkdir(parents=True, exist_ok=True)
    return r / f"{e['id']}.log"


def lanzar(e: dict, exe: str | None = None, extra: list[str] | None = None) -> subprocess.Popen:
    """Arranca el programa (o otro .exe en su mismo prefijo, como un instalador o winecfg)."""
    if not umu():
        raise FileNotFoundError("Falta umu-run (el paquete umu-launcher).")
    Path(e["prefijo"]).mkdir(parents=True, exist_ok=True)
    destino = exe or e["exe"]
    carpeta = str(Path(destino).parent) if Path(destino).is_absolute() and Path(destino).parent.is_dir() else str(Path(e["exe"]).parent)
    registro = open(_registro(e), "wb")
    try:
        return subprocess.Popen(orden(e, exe, extra), env=entorno(e), cwd=carpeta, stdin=subprocess.DEVNULL,
                                stdout=registro, stderr=subprocess.STDOUT, start_new_session=True)
    finally:
        registro.close()


def _prefijo_de(pid: int) -> str | None:
    try:
        crudo = Path(f"/proc/{pid}/environ").read_bytes()
    except OSError:
        return None
    for var in crudo.split(b"\0"):
        if var.startswith(b"WINEPREFIX="):
            return var[11:].decode("utf-8", "replace")
    return None


def _todos() -> dict[str, list[int]]:
    """{prefijo: [pids]} de todos tus procesos con WINEPREFIX (una sola pasada por /proc)."""
    yo, mio = os.getpid(), os.getuid()
    salida: dict[str, list[int]] = {}
    for d in os.listdir("/proc"):
        if not d.isdigit() or int(d) == yo:
            continue
        try:
            if os.stat(f"/proc/{d}").st_uid != mio:
                continue
        except OSError:
            continue
        pref = _prefijo_de(int(d))
        if pref:
            salida.setdefault(os.path.normpath(pref), []).append(int(d))
    return salida


def procesos(prefijo: str | Path) -> list[int]:
    """Los procesos tuyos que corren en ese prefijo."""
    return _todos().get(os.path.normpath(str(prefijo)), [])


def prefijos_abiertos() -> set[str]:
    return set(_todos())


def corriendo(e: dict) -> bool:
    return bool(procesos(e["prefijo"]))


def detener(e: dict, espera: float = 4.0) -> None:
    """Cierra el programa: primero amable (SIGTERM) y, si sigue, a la fuerza."""
    for senal in (signal.SIGTERM, signal.SIGKILL):
        pids = procesos(e["prefijo"])
        if not pids:
            return
        for pid in pids:
            try:
                os.kill(pid, senal)
            except OSError:
                pass
        limite = time.monotonic() + espera
        while time.monotonic() < limite and procesos(e["prefijo"]):
            time.sleep(0.2)


def correr_y_esperar(e: dict) -> int:
    """Para el lanzador («vortex-runner --correr ID»): lo abre, espera a que se cierre y suma el tiempo."""
    from . import biblioteca
    inicio = time.time()
    proc = lanzar(e)
    codigo = proc.wait()
    # umu-run puede salir antes que el juego (si el juego relanza su propio proceso): se espera al prefijo
    while corriendo(e):
        time.sleep(2)
    biblioteca.sumar_tiempo(e["id"], time.time() - inicio, inicio)
    return codigo
