#!/usr/bin/env python3
"""Prueba de los casos de Claude (apagado, sin instalar, sin sesión, listo) con un `vortex` y un `claude` FALSOS
en un PATH de prueba: no se toca la cuenta de nadie ni se gasta nada. Sin pantalla ni sonido.

    QT_QPA_PLATFORM=offscreen python3 -I pruebas/claude_estados.py CARPETA_DE_TRABAJO [CARPETA_DE_CAPTURAS]

El `vortex` falso responde a «claude activado» según el archivo activado.txt (0 = activado) y anota cada
orden que recibe en vortex.log; el `claude` falso responde a «auth status» según sesion.txt y, con -p,
contesta con un flujo mínimo en el mismo formato que el real. Se comprueba, para cada caso:
  - el mensaje en español y el botón que aparece (Activar Claude / Iniciar sesión);
  - que el micrófono NO se abre mientras Claude no esté listo;
  - que cada botón lanza lo que debe (`vortex claude activar` y luego `vortex claude login`, o solo `vortex claude login`),
    aparte de la app (setsid -f), sin pasar por una shell con texto de nadie.
"""

import os
import shutil
import sys
import time
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

trabajo = Path(sys.argv[1]).resolve()
capturas = Path(sys.argv[2]) if len(sys.argv) > 2 else None
shutil.rmtree(trabajo, ignore_errors=True)
bin_falso = trabajo / "bin"
bin_falso.mkdir(parents=True)
(trabajo / "activado.txt").write_text("1")           # 1 = apagado (sale con 1), 0 = activado
(trabajo / "sesion.txt").write_text("1")             # 1 = sin sesión, 0 = con sesión
(trabajo / "vortex.log").write_text("")

VORTEX = f'''#!/bin/bash
echo "vortex $*" >> "{trabajo}/vortex.log"
if [ "$1 $2" = "claude activado" ]; then exit "$(cat "{trabajo}/activado.txt")"; fi
exit 0
'''
CLAUDE = f'''#!/bin/bash
echo "claude $*" >> "{trabajo}/claude.log"
if [ "$1 $2" = "auth status" ]; then exit "$(cat "{trabajo}/sesion.txt")"; fi
echo '{{"type":"system","subtype":"init","session_id":"falso-1","tools":[],"mcp_servers":[],"model":"falso"}}'
echo '{{"type":"stream_event","event":{{"type":"content_block_delta","delta":{{"type":"text_delta","text":"Hola. Soy yo."}}}}}}'
echo '{{"type":"stream_event","event":{{"type":"content_block_stop"}}}}'
echo '{{"type":"result","is_error":false,"result":"Hola. Soy yo.","session_id":"falso-1"}}'
'''
for nombre, texto in (("vortex", VORTEX), ("claude", CLAUDE)):
    ruta = bin_falso / nombre
    ruta.write_text(texto)
    ruta.chmod(0o755)

# El PATH de la prueba: solo lo falso y lo básico (que no encuentre el claude ni el vortex reales).
os.environ["PATH"] = f"{bin_falso}:/usr/bin:/bin"
os.environ["VORTEX_MIKU_SALIDA"] = str(trabajo / "altavoz")
os.environ["VORTEX_MIKU_SALIDA_VELOCIDAD"] = "8"
os.environ.pop("VORTEX_BIN", None)
# El «micrófono» es un WAV de silencio: en estas pruebas nunca se abre el micrófono de verdad.
import wave  # noqa: E402

with wave.open(str(trabajo / "silencio.wav"), "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(16000)
    w.writeframes(b"\0\0" * 16000 * 12)
os.environ["VORTEX_MIKU_ENTRADA"] = str(trabajo / "silencio.wav")

from PySide6.QtWidgets import QApplication  # noqa: E402

from miku import cerebro, config, descargas, rutas  # noqa: E402
from miku.asistente import Asistente  # noqa: E402
from miku.ui_panel import PanelVoz  # noqa: E402

fallos = []


def comprobar(nombre, cond, extra=""):
    print(("  ok  " if cond else "  MAL ") + nombre + (f"  {extra}" if extra else ""), flush=True)
    if not cond:
        fallos.append(nombre)


def esperar(app, cond, limite=8.0):
    fin = time.time() + limite
    while time.time() < fin:
        app.processEvents()
        if cond():
            return True
        time.sleep(0.02)
    return False


def caso(app, nombre, activado, sesion, con_claude=True):
    """Abre un asistente nuevo con ese estado de Claude y devuelve (asistente, panel, registro)."""
    print(f"== {nombre}")
    (trabajo / "activado.txt").write_text("0" if activado else "1")
    (trabajo / "sesion.txt").write_text("0" if sesion else "1")
    claude_dest = Path.home() / ".local" / "bin" / "claude"
    if claude_dest.exists() or claude_dest.is_symlink():
        claude_dest.unlink()
    if not con_claude:
        os.environ["PATH"] = f"{trabajo / 'bin-sin-claude'}:/usr/bin:/bin"
    else:
        os.environ["PATH"] = f"{bin_falso}:/usr/bin:/bin"
    cfg = config.cargar()
    cfg["seguir_escuchando"] = False
    a = Asistente(cfg, False)
    registro = {"estados": [], "respuestas": [], "acciones": [], "avisos": []}
    a.estado.connect(lambda e, t: registro["estados"].append(e))
    a.respuesta.connect(lambda t, fin: registro["respuestas"].append(t) if fin else None)
    a.acciones.connect(lambda lista: registro["acciones"].append(lista))
    a.aviso.connect(lambda t: registro["avisos"].append(t))
    panel = PanelVoz(a, lambda: None)
    panel.show()
    a.iniciar()
    return a, panel, registro


def main() -> int:
    app = QApplication(sys.argv[:1])
    # El oído y la voz ya están en el HOME de prueba, así que no hay descargas que esperar.
    comprobar("los modelos de voz y de oído están (no hay descarga)", not descargas.pendientes(), str(descargas.pendientes()))

    (trabajo / "bin-sin-claude").mkdir()
    shutil.copy(bin_falso / "vortex", trabajo / "bin-sin-claude" / "vortex")

    # 1. Claude apagado (de fábrica en Vortexine's)
    a, panel, r = caso(app, "Claude apagado", activado=False, sesion=False)
    comprobar("dice «Para hablar conmigo necesito tu cuenta de Claude.»", esperar(app, lambda: r["respuestas"] and "Para hablar conmigo necesito tu cuenta de Claude." in r["respuestas"][-1]))
    comprobar("trae el botón «Activar Claude»", r["acciones"] and r["acciones"][-1] == [["activar", "Activar Claude"]], str(r["acciones"]))
    comprobar("el micrófono no se abrió (nadie escucha)", "escuchando" not in r["estados"])
    app.processEvents()
    if capturas:
        capturas.mkdir(parents=True, exist_ok=True)
        panel.grab().save(str(capturas / "panel-claude-apagado.png"))
    comprobar("lo dijo también en voz alta (audio generado)", esperar(app, lambda: list((trabajo / "altavoz").glob("*.wav")), 20))
    (trabajo / "vortex.log").write_text("")
    a.accion("activar")
    comprobar("«Activar Claude» corre `vortex claude activar` y luego `vortex claude login`",
              esperar(app, lambda: (trabajo / "vortex.log").read_text().splitlines() == ["vortex claude activar", "vortex claude login"], 5),
              repr((trabajo / "vortex.log").read_text()))
    comprobar("el botón desaparece y avisa qué hacer", r["acciones"][-1] == [] and any("inicies sesión" in x for x in r["avisos"]))
    a.cerrar_todo()
    panel.deleteLater()

    # 2. Activado, pero el programa claude no está
    a, panel, r = caso(app, "Activado, pero Claude todavía no está instalado", activado=True, sesion=True, con_claude=False)
    comprobar("avisa que falta instalarlo con el botón «Activar Claude»",
              esperar(app, lambda: r["acciones"] and r["acciones"][-1] == [["activar", "Activar Claude"]]) and "instalado" in r["respuestas"][-1], r["respuestas"][-1:] and r["respuestas"][-1])
    a.cerrar_todo()
    panel.deleteLater()

    # 3. Activado y con programa, pero sin sesión
    a, panel, r = caso(app, "Activado, sin sesión iniciada", activado=True, sesion=False)
    comprobar("dice que no hay sesión con el botón «Iniciar sesión»",
              esperar(app, lambda: r["acciones"] and r["acciones"][-1] == [["sesion", "Iniciar sesión"]]) and "sesión" in r["respuestas"][-1], r["respuestas"][-1:] and r["respuestas"][-1])
    comprobar("el micrófono no se abrió", "escuchando" not in r["estados"])
    if capturas:
        panel.grab().save(str(capturas / "panel-sin-sesion.png"))
    (trabajo / "vortex.log").write_text("")
    a.accion("sesion")
    comprobar("«Iniciar sesión» abre `vortex claude login` (solo el inicio de sesión, que se cierra solo)",
              esperar(app, lambda: (trabajo / "vortex.log").read_text().splitlines() == ["vortex claude login"], 5), repr((trabajo / "vortex.log").read_text()))
    # Escribir una pregunta con Claude sin sesión vuelve a comprobar y vuelve a decirlo (no revienta).
    r["respuestas"].clear()
    a.enviar("hola")
    comprobar("una pregunta sin sesión también recibe el aviso y el botón",
              esperar(app, lambda: r["respuestas"] and "sesión" in r["respuestas"][-1]) and r["acciones"][-1] == [["sesion", "Iniciar sesión"]])
    a.cerrar_todo()
    panel.deleteLater()

    # 4. Todo listo: Claude contesta y el micrófono se abre (la entrada es un WAV de silencio)
    a, panel, r = caso(app, "Claude listo (activado, instalado y con sesión)", activado=True, sesion=True)
    comprobar("se pone a escuchar enseguida", esperar(app, lambda: "escuchando" in r["estados"], 10), str(r["estados"]))
    a.enviar("hola")
    comprobar("contesta con el flujo de Claude", esperar(app, lambda: r["respuestas"] and r["respuestas"][-1] == "Hola. Soy yo.", 15), str(r["respuestas"]))
    comprobar("no hay botones", not r["acciones"] or r["acciones"][-1] == [])
    argv = (trabajo / "claude.log").read_text()
    comprobar("claude -p va con las herramientas restringidas (--tools vacío y --restricted)", "--restricted" in argv and "--tools" in argv and "--strict-mcp-config" in argv)
    a.cerrar_todo()
    panel.deleteLater()

    print("FALLOS:", len(fallos), fallos if fallos else "")
    return 1 if fallos else 0


if __name__ == "__main__":
    codigo = main()
    sys.stdout.flush()
    os._exit(codigo)
