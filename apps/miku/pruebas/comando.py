#!/usr/bin/env python3
"""Prueba del comando `vortex-miku` con un HOME de prueba: estado, avatar, modelo, voz y alternar (abrir, cerrar
del todo, candado, PID viejo, cierre brusco). Sin ventanas visibles (QT_QPA_PLATFORM=offscreen) ni sonido ni micrófono.

    python3 -I pruebas/comando.py CARPETA_DE_TRABAJO [MODELOS_VOZ_Y_OIDO]

MODELOS_VOZ_Y_OIDO es una carpeta con voz/ y oido/ ya bajados (se enlazan, no se copian) para que, al abrir,
se levante también el proceso de la voz y se compruebe que se cierra con todo lo demás.
"""

import hashlib
import http.server
import json
import os
import shutil
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

trabajo = Path(sys.argv[1]).resolve()
modelos = Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else None
raiz = Path(__file__).resolve().parent.parent
COMANDO = str(raiz / "vortex-miku")

shutil.rmtree(trabajo, ignore_errors=True)
home = trabajo / "home"
runtime = trabajo / "run"
runtime.mkdir(parents=True, mode=0o700)
(home / ".local/share/vortex/miku").mkdir(parents=True)
if modelos:
    # Se enlazan los archivos (nunca la carpeta): la prueba de «voz ID» escribe en voice_styles/ y no debe tocar lo original.
    d = home / ".local/share/vortex/miku"
    (d / "oido").symlink_to(modelos / "oido")
    (d / "voz" / "voice_styles").mkdir(parents=True)
    (d / "voz" / "onnx").symlink_to((modelos / "voz" / "onnx").resolve())
    (d / "voz" / "voice_styles" / "F1.json").symlink_to((modelos / "voz" / "voice_styles" / "F1.json").resolve())
    (d / "voz" / "LICENSE").symlink_to((modelos / "voz" / "LICENSE").resolve())

# Un silencio de «micrófono»: en la prueba nunca se abre el micrófono de verdad.
import wave  # noqa: E402

with wave.open(str(trabajo / "silencio.wav"), "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(16000)
    w.writeframes(b"\0\0" * 16000 * 30)

ENV = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / ".config"), XDG_DATA_HOME=str(home / ".local/share"),
           XDG_STATE_HOME=str(home / ".local/state"), XDG_RUNTIME_DIR=str(runtime), QT_QPA_PLATFORM="offscreen",
           VORTEX_MIKU_ENTRADA=str(trabajo / "silencio.wav"), PATH="/usr/bin:/bin")
ENV.pop("VORTEX_MIKU_MODELO_SISTEMA", None)
CONFIG = home / ".config/vortex/miku.json"
fallos = []


def comprobar(nombre, cond, extra=""):
    print(("  ok  " if cond else "  MAL ") + nombre + (f"  {extra}" if extra else ""), flush=True)
    if not cond:
        fallos.append(nombre)


def correr(*args, env=None):
    return subprocess.run([sys.executable, "-I", COMANDO, *args], capture_output=True, text=True, env=env or ENV, timeout=60)


def procesos_de_miku() -> list[tuple[int, str]]:
    """Todo proceso vivo cuya línea de órdenes sea de ESTA copia de la app (la app, la voz, QtWebEngine…)."""
    marca = str(raiz)
    salida = []
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                cmd = Path(f"/proc/{d}/cmdline").read_text().replace("\0", " ")
                estado = Path(f"/proc/{d}/stat").read_text().rsplit(")", 1)[1].split()[0]
            except OSError:
                continue
            if marca in cmd and estado != "Z" and int(d) != os.getpid() and "comando.py" not in cmd:
                salida.append((int(d), cmd[-80:]))
    return salida


def hijos_de(pid: int) -> list[int]:
    todos = {}
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                todos[int(d)] = int(Path(f"/proc/{d}/stat").read_text().rsplit(")", 1)[1].split()[1])
            except OSError:
                pass
    fuera, pend = [], [pid]
    while pend:
        p = pend.pop()
        for h, pp in todos.items():
            if pp == p and h not in fuera:
                fuera.append(h)
                pend.append(h)
    return fuera


def esperar(cond, limite=30.0):
    fin = time.time() + limite
    while time.time() < fin:
        if cond():
            return True
        time.sleep(0.1)
    return False


def main() -> int:
    print("== estado")
    r = correr("estado")
    comprobar("sin configuración: exactamente dos líneas avatar=on y abierta=no, sale con 0", r.returncode == 0 and r.stdout == "avatar=on\nabierta=no\n", repr(r.stdout))

    print("== avatar on|off")
    r = correr("avatar", "off")
    comprobar("avatar off sale con 0 y crea ~/.config/vortex/miku.json", r.returncode == 0 and json.loads(CONFIG.read_text()) == {"avatar": False}, CONFIG.read_text())
    comprobar("estado ahora dice avatar=off", correr("estado").stdout == "avatar=off\nabierta=no\n")
    CONFIG.write_text(json.dumps({"voz": "supertonic-f2", "otra": 1, "avatar": False}))
    correr("avatar", "on")
    comprobar("avatar on no borra las demás claves", json.loads(CONFIG.read_text()) == {"voz": "supertonic-f2", "otra": 1, "avatar": True}, CONFIG.read_text())
    CONFIG.write_text("{esto no es json")
    r = correr("estado")
    comprobar("un JSON inválido no truena: valores por defecto", r.returncode == 0 and r.stdout == "avatar=on\nabierta=no\n", repr(r.stdout))
    r = correr("avatar", "off")
    comprobar("y se reescribe bien al guardar", r.returncode == 0 and json.loads(CONFIG.read_text()) == {"avatar": False})
    comprobar("avatar con argumento malo falla con un mensaje", correr("avatar", "quizas").returncode == 2)

    print("== modelo")
    malo = trabajo / "malo.vrm"
    malo.write_bytes(b"no soy un modelo" * 10)
    r = correr("modelo", str(malo))
    comprobar("rechaza un archivo que no es glTF", r.returncode != 0 and "VRM" in r.stderr, r.stderr.strip())
    comprobar("y no deja nada copiado", not (home / ".local/share/vortex/miku/modelo.vrm").exists())
    bueno = trabajo / "con espacios" / "otro modelo.vrm"
    bueno.parent.mkdir()
    bueno.write_bytes(b"glTF" + b"\2\0\0\0" + b"\0" * 100)
    (bueno.parent / "CREDITO.txt").write_text("Modelo de prueba por nadie\n")
    r = correr("modelo", str(bueno))
    destino = home / ".local/share/vortex/miku/modelo.vrm"
    comprobar("copia un glTF (ruta con espacios) a ~/.local/share/vortex/miku/modelo.vrm", r.returncode == 0 and destino.read_bytes() == bueno.read_bytes(), r.stderr)
    comprobar("y su CREDITO.txt", (destino.parent / "CREDITO.txt").read_text() == "Modelo de prueba por nadie\n")
    comprobar("un archivo que no existe falla", correr("modelo", str(trabajo / "no-existe.vrm")).returncode != 0)

    print("== voz")
    r = correr("voz")
    comprobar("sin ID lista las voces y marca la elegida", r.returncode == 0 and "supertonic-f1" in r.stdout and "*" in r.stdout and "OpenRAIL-M" in r.stdout, r.stdout[:200])
    comprobar("elegir la de fábrica funciona", correr("voz", "supertonic-f1").returncode == 0 and json.loads(CONFIG.read_text())["voz"] == "supertonic-f1")
    r = correr("voz", "no-existe")
    comprobar("una voz que no existe falla con un mensaje", r.returncode != 0 and "no existe" in r.stderr)
    # Una voz opcional se baja al elegirla (aquí desde un servidor falso, con su sha256).
    falso = b'{"style_ttl": {}, "style_dp": {}}' * 20

    class H(http.server.BaseHTTPRequestHandler):
        def log_message(self, *a): pass

        def do_GET(self):
            self.send_response(200)
            self.send_header("Content-Length", str(len(falso)))
            self.end_headers()
            self.wfile.write(falso)
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    cat = json.loads((raiz / "voces.json").read_text())
    for v in cat["voces"]:
        if v["id"] == "supertonic-f2":
            v["archivo"].update(url=f"http://127.0.0.1:{srv.server_address[1]}/F2.json", bytes=len(falso), sha256=hashlib.sha256(falso).hexdigest())
    (trabajo / "voces.json").write_text(json.dumps(cat))
    env2 = dict(ENV, VORTEX_MIKU_CATALOGO=str(trabajo / "voces.json"))
    r = correr("voz", "supertonic-f2", env=env2)
    f2 = home / ".local/share/vortex/miku/voz/voice_styles/F2.json"
    comprobar("una voz opcional baja su archivito, lo comprueba y la elige", r.returncode == 0 and f2.read_bytes() == falso and json.loads(CONFIG.read_text())["voz"] == "supertonic-f2", r.stderr)
    f2.unlink()
    cat["voces"][1]["archivo"]["sha256"] = "0" * 64
    (trabajo / "voces.json").write_text(json.dumps(cat))
    r = correr("voz", "supertonic-f2", env=env2)
    comprobar("si llega distinta (sha256) no se instala ni se elige", r.returncode != 0 and not f2.exists() and "sha256" in r.stderr, r.stderr.strip()[:100])
    srv.shutdown()
    CONFIG.write_text(json.dumps({"avatar": False}))

    for modo in ("off", "on"):
        print(f"== alternar con avatar {modo}")
        correr("avatar", modo)
        antes = procesos_de_miku()
        comprobar("antes de abrir no hay ningún proceso de Miku", not antes, str(antes))
        r = correr("alternar")
        comprobar("«vortex-miku» abre la app", r.returncode == 0, r.stderr.strip())
        comprobar("estado dice abierta=si", esperar(lambda: correr("estado").stdout.endswith("abierta=si\n"), 20))
        time.sleep(4 if modo == "off" else 8)
        vivos = procesos_de_miku()
        pid = int((runtime / "vortex-miku.pid").read_text())
        arbol = hijos_de(pid)
        print(f"      procesos: app {pid}, {len(arbol)} hijos")
        comprobar("la app corre en su propia sesión (para poder cerrarla entera)", os.getsid(pid) == pid)
        if modelos:
            comprobar("al abrir levanta también el proceso de la voz", any("voz_worker" in c for _, c in vivos), str([c for _, c in vivos]))
        if modo == "on":
            comprobar("con avatar hay procesos de QtWebEngine", len(arbol) >= 3, f"{len(arbol)} hijos")
        else:
            comprobar("sin avatar no se carga QtWebEngine (ningún QtWebEngineProcess)", not any("QtWebEngineProcess" in c for _, c in vivos))
        r = correr("alternar")
        comprobar("otra vez «vortex-miku» la cierra", r.returncode == 0)
        comprobar("estado dice abierta=no", correr("estado").stdout.endswith("abierta=no\n"))
        comprobar("no queda NINGÚN proceso (ni la voz, ni el oído, ni claude, ni QtWebEngine)", esperar(lambda: not procesos_de_miku(), 6), str(procesos_de_miku()))
        comprobar("no quedan PID ni temporales", not (runtime / "vortex-miku.pid").exists() and not list((runtime / "vortex-miku").glob("*")) if (runtime / "vortex-miku").exists() else True)

    print("== PID viejo (cierre brusco anterior)")
    (runtime / "vortex-miku.pid").write_text("999999")
    comprobar("un PID que ya no existe cuenta como cerrada", correr("estado").stdout.endswith("abierta=no\n"))
    (runtime / "vortex-miku.pid").write_text("1")      # un PID vivo que no es Miku
    comprobar("un PID vivo que no es Miku tampoco cuenta", correr("estado").stdout.endswith("abierta=no\n"))
    r = correr("alternar")
    comprobar("y se abre sin problema", r.returncode == 0 and esperar(lambda: correr("estado").stdout.endswith("abierta=si\n"), 20))

    print("== Cierre brusco (kill -9 a la app): la voz muere con ella")
    time.sleep(3)
    pid = int((runtime / "vortex-miku.pid").read_text())
    os.kill(pid, signal.SIGKILL)
    comprobar("sin la app, no queda nada corriendo", esperar(lambda: not procesos_de_miku(), 8), str(procesos_de_miku()))
    comprobar("y estado dice abierta=no", correr("estado").stdout.endswith("abierta=no\n"))
    r = correr("alternar")
    comprobar("se puede abrir de nuevo", r.returncode == 0 and esperar(lambda: correr("estado").stdout.endswith("abierta=si\n"), 20))

    print("== avatar con la app abierta la reabre en el modo nuevo")
    antes = int((runtime / "vortex-miku.pid").read_text())
    r = correr("avatar", "on")
    comprobar("cambia de modo con la app abierta", r.returncode == 0 and "avatar=on" in r.stdout)
    comprobar("la app nueva tiene otro PID y la vieja ya no existe", esperar(lambda: (runtime / "vortex-miku.pid").exists() and int((runtime / "vortex-miku.pid").read_text()) != antes, 15)
              and not Path(f"/proc/{antes}").exists())
    correr("alternar")
    comprobar("al cerrar no queda nada", esperar(lambda: not procesos_de_miku(), 8), str(procesos_de_miku()))

    print("FALLOS:", len(fallos), fallos if fallos else "")
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
