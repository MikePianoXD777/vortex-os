#!/usr/bin/env python3
"""Prueba de punta a punta del asistente, sin pantalla ni sonido: oír → Claude → voz.

El «micrófono» es un WAV (VORTEX_MIKU_ENTRADA) y el «altavoz» una carpeta (VORTEX_MIKU_SALIDA: guarda
los WAV que sonarían y espera lo que durarían). Claude es el real (`claude -p`, gasta de la suscripción:
una llamada por turno) y su vortex-mcp debe ser uno con grim y hyprctl de mentira (VORTEX_MCP).

    QT_QPA_PLATFORM=offscreen python3 -I pruebas/integracion.py panel|avatar CARPETA turno [turno ...]

Cada turno es  voz:ARCHIVO.wav  (lo oye el oído) o  texto:Lo que se escribe.
Termina con un resumen de tiempos y de lo que pasó. Con «avatar» necesita además VORTEX_MIKU_MODELO_SISTEMA
o el modelo en ~/.local/share/vortex/miku/.
"""

import json
import os
import sys
import time
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

modo, carpeta = sys.argv[1], Path(sys.argv[2])
turnos = sys.argv[3:]
carpeta.mkdir(parents=True, exist_ok=True)
os.environ["VORTEX_MIKU_SALIDA"] = str(carpeta / "altavoz")
os.environ.setdefault("VORTEX_MIKU_SALIDA_VELOCIDAD", "3")

if modo == "avatar":
    from PySide6 import QtWebEngineWidgets  # noqa: F401  (antes de la QApplication)
from PySide6.QtCore import QTimer  # noqa: E402
from PySide6.QtGui import QGuiApplication  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from miku import app as miku_app  # noqa: E402
from miku import config, rutas  # noqa: E402


def main() -> int:
    cfg = config.cargar()
    cfg["seguir_escuchando"] = False
    avatar = modo == "avatar"
    QGuiApplication.setDesktopFileName("vortex-miku")
    app = QApplication(sys.argv[:1])
    modelo = rutas.ruta_modelo(cfg) if avatar else None
    asistente, ventana, cerrar = miku_app.crear(cfg, avatar, "", modelo)
    t0 = time.monotonic()
    eventos = []

    def marca(nombre):
        def f(*a):
            eventos.append((round(time.monotonic() - t0, 2), nombre, a))
        return f

    asistente.estado.connect(lambda e, t: eventos.append((round(time.monotonic() - t0, 2), "estado", (e,))))
    asistente.usuario.connect(marca("usuario"))
    asistente.respuesta.connect(lambda t, fin: eventos.append((round(time.monotonic() - t0, 2), "respuesta_final" if fin else "respuesta", (t,))))
    asistente.aviso.connect(marca("aviso"))
    asistente.boca.connect(lambda p, hz: eventos.append((round(time.monotonic() - t0, 2), "boca", (len(p),))))
    asistente.emocion.connect(marca("emocion"))
    original = asistente._al_evento_cerebro

    def espia(tipo, datos):
        if tipo == "herramienta":
            eventos.append((round(time.monotonic() - t0, 2), "herramienta", (datos["nombre"],)))
        original(tipo, datos)
    asistente._evento_cerebro.disconnect(original)
    asistente._evento_cerebro.connect(espia)

    ventana.show()
    pendientes = list(turnos)
    estado = {"turno": 0, "fin": None, "cuenta": 0}

    def foto(nombre):
        try:
            ventana.grab().save(str(carpeta / f"{nombre}.png"))
        except Exception as e:  # noqa: BLE001
            print("sin foto", e)

    def lanzar_turno():
        if not pendientes:
            QTimer.singleShot(300, terminar)
            return
        t = pendientes.pop(0)
        estado["turno"] += 1
        estado["fin"] = None
        eventos.append((round(time.monotonic() - t0, 2), "TURNO", (estado["turno"], t)))
        if t.startswith("voz:"):
            os.environ["VORTEX_MIKU_ENTRADA"] = t[4:]
            asistente.escuchar()
        else:
            os.environ.pop("VORTEX_MIKU_ENTRADA", None)
            asistente.enviar(t[6:])

    def vigilar():
        if estado["turno"] == 0:
            return
        # ¿terminó el turno? respuesta final + reposo + sin audio pendiente (durante 1 s seguido)
        ultimo_turno = max(i for i, e in enumerate(eventos) if e[1] == "TURNO")
        listo = (asistente.estado_actual == "reposo" and asistente._sonando is None and not asistente._cola
                 and not asistente._pedidos and asistente._cerebro_listo
                 and any(e[1] == "respuesta_final" for e in eventos[ultimo_turno:]))
        if listo:
            estado["cuenta"] += 1
            if estado["cuenta"] >= 5:
                estado["cuenta"] = 0
                lanzar_turno()
        else:
            estado["cuenta"] = 0
        if time.monotonic() - t0 > 240:
            print("TIEMPO AGOTADO")
            terminar()

    def terminar():
        foto("final")
        reloj.stop()
        print("\n== Línea de tiempo")
        for t, nombre, a in eventos:
            if nombre in ("respuesta", "boca", "emocion"):
                continue
            print(f"  {t:6.2f}s  {nombre:15s} {str(a)[:150]}")
        print("\n== Respuestas completas")
        for t, nombre, a in eventos:
            if nombre == "respuesta_final":
                print(f"  [{t:6.2f}s] {a[0]}")
        print("\n== Claude dijo tener estas herramientas:", json.dumps(asistente.cerebro.ultimo_init.get("tools"), ensure_ascii=False))
        print("== Servidores MCP:", asistente.cerebro.ultimo_init.get("mcp_servers"))
        print("== Modelo:", asistente.cerebro.ultimo_init.get("model"))
        wavs = sorted((carpeta / "altavoz").glob("*.wav")) if (carpeta / "altavoz").exists() else []
        print(f"== Audios que habrían sonado: {len(wavs)}")
        sesiones = list(asistente.cerebro.sesiones_creadas)
        cerrar()
        # Tras cerrar: ¿quedó el rastro de la conversación en ~/.claude?
        base = Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude") / "projects"
        quedan = [str(p) for s in sesiones for p in base.glob(f"*/{s}*")]
        print("== Rastro de la conversación en ~/.claude tras cerrar:", quedan or "ninguno")
        print("== Audios temporales tras cerrar:", [p.name for p in rutas.temporales().glob("*.wav")] or "ninguno")

    reloj = QTimer()
    reloj.timeout.connect(vigilar)
    reloj.start(200)
    # La primera acción es el primer turno, tras iniciar la voz.
    asistente._arrancar_voz()
    QTimer.singleShot(300, lanzar_turno)
    QTimer.singleShot(1500, lambda: foto("inicio"))
    app.aboutToQuit.connect(asistente.cerrar_todo)
    app.exec()
    return 0


if __name__ == "__main__":
    code = main()
    sys.stdout.flush()
    os._exit(code)
