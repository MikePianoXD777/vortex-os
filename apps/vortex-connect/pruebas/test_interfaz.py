"""La ventana sin pantalla (offscreen), con nmcli, warp-cli y systemctl falsos: carga sin errores de QML, pide las
condiciones antes de cifrar y recorre el flujo. No toca tu wifi ni tu WARP.

    python3 -m unittest discover -s apps/vortex-connect/pruebas
"""
from __future__ import annotations

import http.server
import json
import os
import stat
import subprocess
import sys
import tempfile
import textwrap
import threading
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

NMCLI = r"""#!/bin/sh
case "$*" in
  *"TYPE,STATE"*) echo "wifi:connected" ;;
  *"-f TYPE device"*) echo "wifi" ;;
  *"wifi list"*) printf '*:Cafe\\: Centro:70:\n :Aeropuerto:40:\n :Casa:99:WPA2\n' ;;
  *) ;;
esac
"""
WARP = """#!/bin/sh
echo "$*" >> "$REGISTRO"
case "$*" in
  *status*) cat "$ESTADO" ;;
  *" connect"*) echo "Status update: Connected" > "$ESTADO" ;;
  *disconnect*) echo "Status update: Disconnected" > "$ESTADO" ;;
esac
"""
# Sin estos, «Cifrar» y «Quitar» tocarían el DNS de verdad del equipo donde corren las pruebas.
SIN_EFECTO = "#!/bin/sh\nexit 0\n"
SYSTEMCTL = """#!/bin/sh
echo "$*" >> "$REGISTRO_SVC"
case "$*" in
  *is-active*) [ -f "$SVC" ] ;;
  *" start"*) : > "$SVC" ;;
  *" stop"*) rm -f "$SVC" ;;
esac
"""

GUION = textwrap.dedent("""
    import json, sys
    sys.path.insert(0, %r)
    from PySide6.QtCore import QObject, QTimer
    from vortex_connect.ventana import crear_aplicacion, cargar_interfaz
    app = crear_aplicacion(["vc"])
    motor, puente = cargar_interfaz(app)
    errores = []
    motor.warnings.connect(lambda ws: errores.extend(w.toString() for w in ws))
    v = motor.rootObjects()[0]
    velo = v.findChild(QObject, "veloCondiciones")
    datos = {}
    def texto(nombre):
        return v.findChild(QObject, nombre).property("text")
    def paso1():
        datos["inicio"] = [puente.estado, puente.redActual, puente.redAbierta, texto("tituloEstado")]
        datos["redes"] = [r["ssid"] for r in puente.redes]
        v.setProperty("pidiendoCondiciones", False)
        v.findChild(QObject, "botonProteger").clicked.emit()
        datos["velo"] = velo.property("visible")
        datos["sin_aceptar"] = puente.condiciones
        v.findChild(QObject, "botonAceptar").clicked.emit()
        datos["aceptadas"] = puente.condiciones
        datos["velo_tras"] = velo.property("visible")
        QTimer.singleShot(2500, paso2)
    def paso2():
        datos["protegido"] = [puente.estado, texto("tituloEstado"), puente.ocupado, puente.mensaje]
        puente.quitarProteccion()
        QTimer.singleShot(1500, paso3)
    def paso3():
        datos["sin_proteger"] = [puente.estado, puente.ocupado]
        interruptor = v.findChild(QObject, "interruptorViaje")
        interruptor.setProperty("checked", True)   # lo que hace un clic de verdad antes de avisar
        interruptor.clicked.emit()
        datos["viaje"] = puente.modoViaje
        datos["errores"] = errores
        print("DATOS" + json.dumps(datos))
        puente.cerrar()
        app.quit()
    QTimer.singleShot(1500, paso1)
    app.exec()
""")


class _Pagina(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/traza":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"warp=on\n" if Path(self.server.estado).read_text().find("Connected") >= 0 and "Disc" not in Path(self.server.estado).read_text() else b"warp=off\n")
            return
        self.send_response(204)
        self.end_headers()

    def log_message(self, *_a):
        pass


class TestInterfaz(unittest.TestCase):
    def test_flujo(self):
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            for nombre, texto in (("nmcli", NMCLI), ("warp-cli", WARP), ("systemctl", SYSTEMCTL),
                                  ("resolvectl", SIN_EFECTO), ("ip", SIN_EFECTO)):
                (d / nombre).write_text(texto)
                (d / nombre).chmod((d / nombre).stat().st_mode | stat.S_IXUSR)
            estado = d / "estado"
            estado.write_text("Status update: Disconnected\n")
            servidor = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Pagina)
            servidor.estado = str(estado)
            threading.Thread(target=servidor.serve_forever, daemon=True).start()
            base = f"http://127.0.0.1:{servidor.server_address[1]}"
            env = dict(os.environ, PATH=f"{d}:{os.environ['PATH']}", QT_QPA_PLATFORM="offscreen", BROWSER="true",
                       XDG_CONFIG_HOME=str(d / "config"), REGISTRO=str(d / "reg"), REGISTRO_SVC=str(d / "reg-svc"),
                       ESTADO=str(estado), SVC=str(d / "svc"), VORTEX_CONNECT_PRUEBA=base + "/204",
                       VORTEX_CONNECT_TRAZA=base + "/traza", VORTEX_CONNECT_COLORES=str(d / "no-hay.json"))
            try:
                r = subprocess.run([sys.executable, "-c", GUION % str(RAIZ)], env=env, capture_output=True, text=True, timeout=60)
            finally:
                servidor.shutdown()
                servidor.server_close()
            linea = next((l for l in r.stdout.splitlines() if l.startswith("DATOS")), None)
            self.assertIsNotNone(linea, r.stderr[-1500:])
            datos = json.loads(linea[5:])
            ordenes = (d / "reg").read_text().splitlines()
            guardado = json.loads((d / "config" / "vortex" / "connect.json").read_text())
        self.assertEqual(datos["errores"], [])
        self.assertEqual(datos["inicio"], ["sin_proteccion", "Cafe: Centro", True, "Sin protección"])
        self.assertEqual(datos["redes"], ["Cafe: Centro", "Aeropuerto"])
        self.assertTrue(datos["velo"], "pide las condiciones antes de cifrar")
        self.assertFalse(datos["sin_aceptar"])
        self.assertTrue(datos["aceptadas"])
        self.assertFalse(datos["velo_tras"])
        self.assertEqual(datos["protegido"][:3], ["protegido", "Protegido", False], datos["protegido"])
        self.assertEqual(datos["sin_proteger"], ["sin_proteccion", False])
        self.assertTrue(datos["viaje"])
        self.assertTrue(guardado["condiciones"] and guardado["modo_viaje"])
        self.assertTrue(ordenes, "llegó a warp-cli después de aceptar")


class TestAntesDeAceptar(unittest.TestCase):
    def test_estado_sin_aceptar_no_toca_warp(self):
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            for nombre, texto in (("nmcli", NMCLI), ("warp-cli", WARP), ("systemctl", SYSTEMCTL),
                                  ("resolvectl", SIN_EFECTO), ("ip", SIN_EFECTO)):
                (d / nombre).write_text(texto)
                (d / nombre).chmod((d / nombre).stat().st_mode | stat.S_IXUSR)
            env = dict(os.environ, PATH=f"{d}:{os.environ['PATH']}", XDG_CONFIG_HOME=str(d / "config"),
                       REGISTRO=str(d / "reg"), REGISTRO_SVC=str(d / "reg-svc"), ESTADO=str(d / "e"), SVC=str(d / "svc"),
                       VORTEX_CONNECT_PRUEBA="http://127.0.0.1:9/", PYTHONPATH=str(RAIZ))
            r = subprocess.run([sys.executable, "-m", "vortex_connect", "estado"], env=env, capture_output=True, text=True, timeout=30)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertFalse((d / "reg").exists(), "warp-cli no se llama sin aceptar")
            self.assertFalse((d / "reg-svc").exists() and "start" in (d / "reg-svc").read_text())


if __name__ == "__main__":
    unittest.main()
