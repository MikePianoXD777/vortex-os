#!/usr/bin/env python3
"""Prueba de la cámara de Miku (2.2): camara.py, la herramienta ver_camara (acciones_mcp.py) y la decisión de la
ventana (asistente.py). NUNCA abre una cámara de verdad:

  - la «cámara» es un ffmpeg falso en el PATH (escribe 3 JPEG de mentira o falla como una cámara ocupada) y un
    dispositivo que no existe (/dev/video-falsa); el ffmpeg falso truena si le piden otro dispositivo;
  - HOME, XDG_CONFIG_HOME y XDG_RUNTIME_DIR son carpetas temporales: no lee ni cambia tu miku.json, tu pausa
    de Claude ni tus sockets;
  - la ventana corre sin pantalla (QCoreApplication, offscreen).

Qué cubre: la cámara falsa da la mejor de 3 fotos y borra el temporal (carpeta 700); cámara ocupada, sin
cámara y sin carpeta segura lo dicen claro; con el ajuste apagado no hay foto; con la pausa no hay foto (ni se
pregunta a la ventana); una foto por pregunta y tope por minuto; las frases que Miku dice sola no prenden la
cámara; lo que no suena a pedir la cámara pide un clic; «Autorizar todo» no vale para la cámara; el aviso de la
primera vez; apagarla en Ajustes vale al instante; el indicador se prende y se apaga.

  python3 -I apps/miku/pruebas/camara.py
"""

import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
TMP = Path(tempfile.mkdtemp(prefix="vortex-miku-camara."))
# Antes de importar miku: rutas.py lee estas variables al cargarse.
for nombre in ("home", "config", "run", "bin", "sysfs"):
    (TMP / nombre).mkdir()
os.chmod(TMP / "run", 0o700)
LOG = TMP / "log"
os.environ.update({"HOME": str(TMP / "home"), "XDG_CONFIG_HOME": str(TMP / "config"),
                   "XDG_RUNTIME_DIR": str(TMP / "run"), "VORTEX_MIKU_CAMARA": "/dev/video-falsa",
                   "VORTEX_MIKU_SYSFS_V4L": str(TMP / "sysfs"), "LOG": str(LOG),
                   "PATH": f"{TMP / 'bin'}:/usr/bin:/bin", "QT_QPA_PLATFORM": "offscreen",
                   "VORTEX_MIKU_RAIZ": str(RAIZ)})
sys.path.insert(0, str(RAIZ))
FALLOS = 0
PAUSA = TMP / "config/vortex/claude-pausa"

# El ffmpeg falso: con FFMPEG_MODO=ocupada falla como una cámara en uso; si no, deja 3 cuadros de tamaños
# distintos (el «más nítido» es el más pesado) y anota los permisos de la carpeta donde los dejó.
FFMPEG = r'''#!/bin/sh
echo "ffmpeg $*" >> "$LOG"
dev=""; prev=""
for a in "$@"; do [ "$prev" = "-i" ] && dev="$a"; prev="$a"; out="$a"; done
[ "$dev" = "/dev/video-falsa" ] || { echo "ffmpeg falso: me pidieron otra cámara ($dev)" >&2; exit 9; }
case "$FFMPEG_MODO" in
  ocupada) echo "[video4linux2,v4l2 @ 0x1] ioctl(VIDIOC_STREAMON): Device or resource busy" >&2; exit 1;;
  nada) echo "/dev/video-falsa: No such file or directory" >&2; exit 1;;
esac
d=$(dirname "$out")
stat -c 'carpeta %a' "$d" >> "$LOG"
printf 'JPEG-chico' > "$d/cuadro1.jpg"
printf 'JPEG-EL-MAS-NITIDO-DE-LOS-TRES' > "$d/cuadro2.jpg"
printf 'JPEG-medio-xx' > "$d/cuadro3.jpg"
'''


def probar(nombre: str, ok: bool, extra: str = "") -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}" + (f"   [{str(extra)[:300]}]" if not ok and extra else ""))
    FALLOS += 0 if ok else 1


def preparar_falsos() -> None:
    (TMP / "bin/ffmpeg").write_text(FFMPEG)
    (TMP / "bin/notify-send").write_text('#!/bin/sh\necho "notify-send $*" >> "$LOG"\n')
    for p in ("ffmpeg", "notify-send"):
        (TMP / "bin" / p).chmod(0o755)


def log() -> str:
    return LOG.read_text() if LOG.exists() else ""


def temporales_vivos() -> list:
    return sorted(p.name for p in (TMP / "run").glob("miku-camara.*"))


# ---- 1. camara.py ---------------------------------------------------------------------------------

def parte_modulo() -> None:
    from miku import camara
    print("== camara.py con el ffmpeg falso")
    for frase in ("Miku, mira el cuarto", "¿qué estoy haciendo?", "¿está ordenado mi cuarto?", "usa la cámara", "mírame",
                  "¿me ves por la cámara?", "échale un ojo al cuarto", "checa con la webcam si ya tendí la cama",
                  "toma una foto con la cámara"):
        probar(f"«{frase}» suena a pedir la cámara", camara.pidio_camara(frase))
    for frase in ("¿qué hora es?", "pon música", "abre el navegador", "", "mi cámara no sirve", "la webcam",
                  "¿cuánto cuesta una cámara?", "compré una webcam nueva"):
        probar(f"«{frase}» no", not camara.pidio_camara(frase))

    foto = camara.tomar_foto()
    probar("da la mejor de 3 fotos (la más nítida)", foto == b"JPEG-EL-MAS-NITIDO-DE-LOS-TRES", foto)
    probar("la carpeta temporal era 700 y estaba en XDG_RUNTIME_DIR", "carpeta 700" in log(), log())
    probar("…y se borró al terminar", temporales_vivos() == [], temporales_vivos())
    probar("solo pidió /dev/video-falsa (nunca una cámara real)", "-i /dev/video-falsa" in log() and "/dev/video0" not in log())
    probar("toma unos cuadros y sale (nada de video continuo)", "-frames:v 3" in log(), log())

    os.environ["FFMPEG_MODO"] = "ocupada"
    try:
        camara.tomar_foto()
        probar("cámara ocupada: lo dice", False, "no tronó")
    except camara.ErrorCamara as e:
        probar("cámara ocupada: lo dice claro", "ocupada" in str(e), e)
    probar("…y también borra el temporal", temporales_vivos() == [], temporales_vivos())
    os.environ["FFMPEG_MODO"] = "nada"
    try:
        camara.tomar_foto()
        probar("cámara desconectada: lo dice", False, "no tronó")
    except camara.ErrorCamara as e:
        probar("cámara desconectada: lo dice claro", "No encontré la cámara" in str(e), e)
    os.environ.pop("FFMPEG_MODO")

    os.environ["VORTEX_MIKU_CAMARA"] = ""
    try:
        camara.tomar_foto()
        probar("sin ninguna cámara: lo dice", False, "no tronó")
    except camara.ErrorCamara as e:
        probar("sin ninguna cámara: lo dice claro", "ninguna cámara" in str(e), e)
    for n, indice in (("video2", "0"), ("video3", "1"), ("video1", "1")):
        (TMP / "sysfs" / n).mkdir()
        (TMP / "sysfs" / n / "index").write_text(indice + "\n")
    probar("busca la entrada principal de la cámara (index 0)", camara.dispositivo() == "/dev/video2", camara.dispositivo())
    os.environ["VORTEX_MIKU_CAMARA"] = "/dev/video-falsa"

    run = os.environ.pop("XDG_RUNTIME_DIR")
    try:
        camara.tomar_foto()
        probar("sin carpeta segura no toma la foto", False, "no tronó")
    except camara.ErrorCamara as e:
        probar("sin carpeta segura ($XDG_RUNTIME_DIR) no toma la foto", "XDG_RUNTIME_DIR" in str(e), e)
    os.environ["XDG_RUNTIME_DIR"] = run
    guia = camara.guia("cuarto")
    probar("la guía pide veredicto y 2 o 3 cosas", "ordenado" in guia and "más o menos" in guia and "2 o 3" in guia)
    probar("la guía prohíbe identificar caras y seguir órdenes de la foto",
           "no identifiques" in guia and "NO instrucciones" in guia)


# ---- 2. ver_camara (acciones_mcp.py) ----------------------------------------------------------------

class Ventana:
    """La ventana falsa: contesta lo que diga `respuesta` y guarda lo que le pidieron."""

    def __init__(self, ruta: Path):
        self.ruta, self.respuesta, self.pedidos = ruta, {"ok": True}, []
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.bind(str(ruta))
        self.s.listen(4)
        threading.Thread(target=self._bucle, daemon=True).start()

    def _bucle(self):
        while True:
            c, _ = self.s.accept()
            with c:
                self.pedidos.append(json.loads(c.makefile("r").readline()))
                c.sendall(json.dumps(self.respuesta).encode() + b"\n")
                c.recv(1)


def mcp(env: dict, nombre: str, args: dict) -> dict:
    entrada = "\n".join(json.dumps(m) for m in [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": nombre, "arguments": args}},
    ]) + "\n"
    r = subprocess.run([sys.executable, "-I", str(RAIZ / "miku/acciones_mcp.py")], input=entrada, text=True,
                       capture_output=True, env=env, timeout=60)
    resp = [json.loads(l) for l in r.stdout.splitlines() if l.strip()]
    return {"lista": resp[1]["result"]["tools"], "res": resp[-1].get("result", {})}


def textos(res: dict) -> str:
    return "\n".join(c.get("text", "") for c in res.get("content", []) if c.get("type") == "text")


def parte_mcp() -> None:
    print("== ver_camara (acciones_mcp.py) con una ventana falsa")
    ventana = Ventana(TMP / "run/autorizar.sock")
    env = dict(os.environ, VORTEX_MIKU_AUTORIZAR=str(ventana.ruta), VORTEX_MIKU_RAIZ=str(RAIZ))
    LOG.write_text("")
    r = mcp(env, "ver_camara", {"motivo": "cuarto"})
    nombres = [h["name"] for h in r["lista"]]
    probar("ver_camara está junto a las demás acciones (no se quitó ninguna)",
           "ver_camara" in nombres and {"abrir", "correr_comando", "grabar", "redactar_mensaje"} <= set(nombres), nombres)
    imagenes = [c for c in r["res"].get("content", []) if c.get("type") == "image"]
    probar("con el sí de la ventana devuelve UNA imagen JPEG", len(imagenes) == 1 and imagenes[0]["mimeType"] == "image/jpeg", r["res"])
    probar("…con la guía del cuarto para la IA", "ordenado" in textos(r["res"]), textos(r["res"]))
    probar("le pidió la foto a la ventana como «camara»", ventana.pedidos[-1].get("tipo") == "camara"
           and ventana.pedidos[-1].get("motivo") == "cuarto", ventana.pedidos[-1:])
    probar("avisa con una notificación", "notify-send" in log() and "cámara" in log(), log())
    probar("no deja la foto en ningún lado", temporales_vivos() == [], temporales_vivos())

    LOG.write_text("")
    ventana.respuesta = {"ok": False, "mensaje": "La cámara está APAGADA en los ajustes de Miku."}
    r = mcp(env, "ver_camara", {"motivo": "actividad"})
    probar("ajuste apagado (la ventana dice que no): no hay foto", "APAGADA" in textos(r["res"])
           and not any(c.get("type") == "image" for c in r["res"].get("content", [])) and "ffmpeg" not in log(), r["res"])

    ventana.respuesta = {"ok": True}
    PAUSA.parent.mkdir(parents=True, exist_ok=True)
    PAUSA.write_text("")
    n = len(ventana.pedidos)
    LOG.write_text("")
    r = mcp(env, "ver_camara", {"motivo": "cuarto"})
    probar("con «vortex claude pausa» no hay foto", r["res"].get("isError") and "pausa" in textos(r["res"])
           and "ffmpeg" not in log(), r["res"])
    probar("…y ni siquiera se le pregunta a la ventana", len(ventana.pedidos) == n)
    PAUSA.unlink()

    env_ocupada = dict(env, FFMPEG_MODO="ocupada")
    r = mcp(env_ocupada, "ver_camara", {"motivo": "cuarto"})
    probar("cámara ocupada: la IA recibe el error claro (no falla en silencio)",
           r["res"].get("isError") and "ocupada" in textos(r["res"]), r["res"])

    sin_ventana = {k: v for k, v in env.items() if k != "VORTEX_MIKU_AUTORIZAR"}
    LOG.write_text("")
    r = mcp(sin_ventana, "ver_camara", {})
    probar("sin la ventana de Miku abierta no hay foto", r["res"].get("isError") and "ffmpeg" not in log(), r["res"])

    from miku import cerebro, cerebro_abierto
    cmd = cerebro.construir_comando("claude", "p", "hola", None, "/usr/bin/vortex-mcp", "/tmp/sock")
    probar("Claude tiene ver_camara permitida (solo con la ventana para decidir)", "mcp__miku__ver_camara" in cmd)
    cmd = cerebro.construir_comando("claude", "p", "hola", None, "/usr/bin/vortex-mcp", None)
    probar("…y sin la ventana, no", "mcp__miku__ver_camara" not in cmd)
    limpio = cerebro_abierto._sin_imagenes([{"role": "user", "content": [
        {"type": "text", "text": "La imagen que pediste"}, {"type": "image_url", "image_url": {"url": "data:x"}}]}])
    probar("la IA abierta no guarda la foto en el historial", "data:x" not in json.dumps(limpio) and "[imagen]" in limpio[0]["content"])


# ---- 3. La ventana decide (asistente.py) -------------------------------------------------------------

def parte_ventana() -> None:
    from PySide6.QtCore import QCoreApplication, QTimer
    from miku import asistente, camara, config
    app = QCoreApplication.instance() or QCoreApplication(sys.argv)
    print("== La ventana decide cada foto (asistente.py)")
    a = asistente.Asistente(config.cargar(), avatar=False)
    botones, avisos, indicador = [], [], []
    a.acciones.connect(botones.append)
    a.aviso.connect(avisos.append)
    a.camara.connect(indicador.append)
    ruta = a.cerebro.autorizar
    probar("de fábrica la cámara está apagada", config.cargar()["camara"] is False)

    def pedir(accion: str | None = None, esperar_ms: int = 300) -> dict:
        resultado = {}

        def cliente():
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(ruta)
            s.sendall(b'{"tipo": "camara", "titulo": "Tomar una foto", "motivo": "cuarto"}\n')
            resultado["r"] = json.loads(s.makefile("r").readline() or "{}")
            s.close()
        h = threading.Thread(target=cliente, daemon=True)
        h.start()
        if accion:
            QTimer.singleShot(esperar_ms, lambda: a.accion(accion))
        QTimer.singleShot(esperar_ms + 500, app.quit)
        app.exec()
        h.join(2)
        return resultado.get("r", {})

    def pregunta(texto: str, tuya: bool = True) -> None:
        a._pregunta_id += 1
        a._ultimo_texto, a._pregunta_tuya, a._cerebro_listo = (texto if tuya else ""), tuya, False

    pregunta("Miku, mira el cuarto")
    r = pedir()
    probar("ajuste apagado: no hay foto y lo explica", r.get("ok") is False and "APAGADA" in r.get("mensaje", ""), r)
    probar("…y ofrece «Activar la cámara»", ["activar_camara", "Activar la cámara"] in (botones[-1] if botones else []), botones[-1:])
    a.accion("activar_camara")
    probar("al activarla, primero el aviso: la foto va a la IA, no se guarda, nunca video",
           any("se manda a la IA" in t and "no se guarda" in t and "nunca graba video" in t for t in avisos), avisos[-1:])
    probar("…sin activarla todavía", config.cargar()["camara"] is False)
    a.accion("camara_si")
    probar("con «Sí» queda activada (y el aviso, visto)", config.cargar()["camara"] and config.cargar()["camara_aviso"])

    a._cerebro_listo = True
    r = pedir()
    probar("sin una pregunta en curso no hay foto (el cerebro no la prende solo)", r.get("ok") is False, r)
    pregunta("", tuya=False)
    r = pedir()
    probar("en lo que Miku dice sola (oculto) no hay foto", r.get("ok") is False, r)

    pregunta("Miku, ¿está ordenado mi cuarto?")
    r = pedir()
    probar("con la cámara activada y una pregunta clara: foto sin clic", r.get("ok") is True, r)
    probar("…y el indicador se prendió mientras tomaba la foto", indicador == [True, False], indicador)
    probar("…con el aviso «Miku está usando la cámara»", any("usando la cámara" in t for t in avisos), avisos[-1:])
    probar("al terminar la foto el indicador se apaga", indicador[-1:] == [False] and a._camara_socket is None, indicador)
    r = pedir()
    probar("una sola foto por pregunta", r.get("ok") is False and "esta pregunta" in r.get("mensaje", ""), r)
    for _ in range(camara.TOPE_POR_MINUTO - 1):
        pregunta("¿qué estoy haciendo?")
        pedir()
    pregunta("mira el cuarto otra vez")
    r = pedir()
    probar(f"tope de {camara.TOPE_POR_MINUTO} fotos por minuto", r.get("ok") is False and "minuto" in r.get("mensaje", ""), r)
    a._camara_fotos.clear()

    pregunta("¿y tú qué opinas?")
    r = pedir("cancelar")
    probar("si lo que dijo no suena a pedir la cámara, pide un clic", any(["autorizar", "Tomar la foto"] in b for b in botones), botones[-3:])
    probar("…y «No» no toma la foto", r.get("ok") is False, r)
    pregunta("¿y tú qué opinas?")
    r = pedir("autorizar_todo")
    probar("«Autorizar todo» vale solo para esta foto", r.get("ok") is True and a._autorizar_todo is False, r)

    PAUSA.parent.mkdir(parents=True, exist_ok=True)
    PAUSA.write_text("")
    pregunta("mira el cuarto")
    r = pedir()
    probar("con «vortex claude pausa» la ventana dice que no", r.get("ok") is False and "pausa" in r.get("mensaje", ""), r)
    PAUSA.unlink()

    config.guardar({"camara": False})
    pregunta("mira el cuarto")
    r = pedir()
    probar("apagarla en Ajustes vale al instante (sin reabrir a Miku)", r.get("ok") is False and "APAGADA" in r.get("mensaje", ""), r)

    config.guardar({"camara": True, "camara_aviso": False})
    pregunta("Miku, mira el cuarto")
    r = pedir("autorizar")
    probar("activada por comando sin ver el aviso: lo muestra y pide un clic antes de la primera foto",
           r.get("ok") is True and any(camara.AVISO_PRIMERA_VEZ in t for t in avisos), avisos[-1:])
    probar("…y desde ahí queda visto", config.cargar()["camara_aviso"] is True)
    a.cerrar_todo()


def parte_comando() -> None:
    print("== vortex-miku camara")
    env = dict(os.environ, VORTEX_MIKU_RAIZ=str(RAIZ))     # el código de esta rama, no el instalado
    r = subprocess.run([sys.executable, str(RAIZ / "vortex-miku"), "camara", "off"], capture_output=True, text=True, env=env, timeout=30)
    r2 = subprocess.run([sys.executable, str(RAIZ / "vortex-miku"), "camara", "estado"], capture_output=True, text=True, env=env, timeout=30)
    probar("«camara off» la apaga", r.returncode == 0 and "camara=off" in r2.stdout, r.stderr + r2.stdout)
    r = subprocess.run([sys.executable, str(RAIZ / "vortex-miku"), "camara", "on"], capture_output=True, text=True, env=env, timeout=30)
    probar("«camara on» la prende y dice qué pasa con la foto", r.returncode == 0 and "camara=on" in r.stdout
           and "no se guarda" in r.stdout, r.stdout + r.stderr)
    r = subprocess.run([sys.executable, str(RAIZ / "vortex-miku"), "camara", "talvez"], capture_output=True, text=True, env=env, timeout=30)
    probar("un valor raro no cambia nada", r.returncode == 2)


def main() -> int:
    preparar_falsos()
    try:
        parte_modulo()
        parte_mcp()
        parte_ventana()
        parte_comando()
    finally:
        import shutil
        shutil.rmtree(TMP, ignore_errors=True)
    print("\nTodo bien" if not FALLOS else f"\nFALLARON {FALLOS} pruebas")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
