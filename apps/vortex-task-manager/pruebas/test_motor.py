"""Pruebas del motor de Vortexine Task Manager (sin ventana). Solo toca procesos que crea ella misma.

    python3 -m unittest discover -s apps/vortex-task-manager/pruebas
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from vortex_task_manager import servicios, sistema  # noqa: E402


def proc(pid, ppid, nombre, uid=1000, cpu=0.0, mem=0, disco=0.0, estado="sleeping"):
    return {"pid": pid, "ppid": ppid, "nombre": nombre, "comando": nombre, "uid": uid, "usuario": "mike" if uid == 1000 else "root",
            "cpu": cpu, "mem": mem, "hilos": 1, "estado": estado, "disco": disco}


class Nombres(unittest.TestCase):
    def test_interpretes_muestran_su_script(self):
        self.assertEqual(sistema.nombre_de("python3", ["python3", "-m", "vortex_player"]), "vortex_player")
        self.assertEqual(sistema.nombre_de("python3", ["/usr/bin/python3", "-u", "/opt/x/focus_daemon.py"]), "focus_daemon.py")
        self.assertEqual(sistema.nombre_de("bash", ["bash", "/usr/bin/vortex-clips", "servicio"]), "vortex-clips")
        self.assertEqual(sistema.nombre_de("python3", ["python3", "-c", "print(1)"]), "python3")
        self.assertEqual(sistema.nombre_de("fish", ["fish"]), "fish")

    def test_comm_cortado_usa_la_linea_de_comando(self):
        self.assertEqual(sistema.nombre_de("chrome_crashpad", ["/opt/chrome/chrome_crashpad_handler", "--x"]), "chrome_crashpad_handler")


class Lanzadores(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        (self.dir / "org.mozilla.firefox.desktop").write_text(
            "[Desktop Entry]\nType=Application\nName=Firefox\nName[es]=Firefox ES\nIcon=org.mozilla.firefox\nExec=firefox %u\n")
        (self.dir / "kitty-open.desktop").write_text("[Desktop Entry]\nType=Application\nName=kitty URL Launcher\nExec=kitty +open %U\nNoDisplay=true\n")
        (self.dir / "kitty.desktop").write_text("[Desktop Entry]\nType=Application\nName=kitty\nIcon=kitty\nExec=kitty\n")
        (self.dir / "steam.desktop").write_text("[Desktop Entry]\nType=Application\nName=Steam\nIcon=steam\nExec=env X=1 /usr/bin/steam %U\nStartupWMClass=Steam\n"
                                                "[Desktop Action x]\nName=Otra cosa\n")

    def test_busca_por_clase_id_y_ejecutable(self):
        a = sistema.Aplicaciones([self.dir])
        self.assertEqual(a.de_clase("firefox")["icono"], "org.mozilla.firefox")        # último pedazo del id
        self.assertEqual(a.de_clase("org.mozilla.firefox")["id"], "org.mozilla.firefox")
        self.assertEqual(a.de_clase("Steam")["nombre"], "Steam")                         # StartupWMClass, sin la acción
        self.assertEqual(a.de_ejecutable("steam")["nombre"], "Steam")                    # Exec con env y ruta
        self.assertEqual(a.de_ejecutable("kitty")["nombre"], "kitty")                    # el visible, no el oculto
        self.assertIsNone(a.de_clase("nada"))


class Agrupar(unittest.TestCase):
    def setUp(self):
        self.filas = [
            proc(1, 0, "systemd", uid=0), proc(100, 1, "kitty", cpu=1, mem=50), proc(101, 100, "fish", mem=5),
            proc(102, 101, "claude", cpu=4, mem=300), proc(200, 1, "firefox", cpu=10, mem=900),
            proc(201, 200, "firefox", cpu=2, mem=400), proc(300, 1, "pipewire", mem=20), proc(301, 1, "pipewire", mem=10),
            proc(400, 1, "sshd", uid=0, mem=8),
        ]
        self.ventanas = [{"pid": 100, "clase": "kitty", "titulo": "~", "direccion": "0x1"},
                         {"pid": 200, "clase": "firefox", "titulo": "YouTube", "direccion": "0x2"},
                         {"pid": 200, "clase": "firefox", "titulo": "Otra", "direccion": "0x3"}]
        self.apps = sistema.Aplicaciones([])

    def test_apps_fondo_y_sistema(self):
        g = sistema.agrupar(self.filas, self.ventanas, self.apps, uid=1000)
        apps = {x["nombre"]: x for x in g["apps"]}
        self.assertEqual(sorted(apps["kitty"]["pids"]), [100, 101, 102])     # la terminal con lo que corre dentro
        self.assertEqual(apps["kitty"]["cpu"], 5)
        self.assertEqual(apps["firefox"]["subtitulo"], "2 ventanas")
        self.assertEqual(apps["firefox"]["ventanas"], ["0x2", "0x3"])
        self.assertEqual(apps["firefox"]["mem"], 1300)
        fondo = {x["nombre"]: x for x in g["fondo"]}
        self.assertEqual(sorted(fondo["pipewire"]["pids"]), [300, 301])       # juntos por nombre
        self.assertEqual({x["nombre"] for x in g["sistema"]}, {"systemd", "sshd"})

    def test_ordenar_y_buscar(self):
        g = sistema.agrupar(self.filas, self.ventanas, self.apps, uid=1000)
        self.assertEqual([x["nombre"] for x in sistema.ordenar(g["apps"], "mem")], ["firefox", "kitty"])
        self.assertEqual([x["nombre"] for x in sistema.ordenar(g["apps"], "nombre")], ["firefox", "kitty"])
        kitty = next(x for x in g["apps"] if x["nombre"] == "kitty")
        self.assertTrue(sistema.coincide(kitty, "claude"))       # por un proceso de adentro
        self.assertTrue(sistema.coincide(kitty, "102"))          # por PID
        self.assertFalse(sistema.coincide(kitty, "firefox"))

    def test_suspendido_solo_si_todos(self):
        filas = [proc(10, 1, "a", estado="stopped"), proc(11, 10, "b", estado="stopped")]
        g = sistema.agrupar(filas, [{"pid": 10, "clase": "a", "titulo": "", "direccion": "0x9"}], self.apps, uid=1000)
        self.assertTrue(g["apps"][0]["suspendido"])


class ProcesosReales(unittest.TestCase):
    def test_muestrea_y_controla_un_proceso_propio(self):
        hijo = subprocess.Popen(["sleep", "30"])
        try:
            p = sistema.Procesos()
            p.muestrear()
            filas = {f["pid"]: f for f in p.muestrear()}
            self.assertIn(hijo.pid, filas)
            self.assertEqual(filas[hijo.pid]["nombre"], "sleep")
            self.assertEqual(filas[os.getpid()]["usuario"], filas[hijo.pid]["usuario"])
            self.assertIsNone(sistema.suspender([hijo.pid], True))
            time.sleep(0.2)
            self.assertEqual({f["pid"]: f for f in p.muestrear()}[hijo.pid]["estado"], "stopped")
            self.assertIsNone(sistema.suspender([hijo.pid], False))
            self.assertIsNone(sistema.prioridad([hijo.pid], "baja"))
            self.assertEqual(sistema.prioridad_de(hijo.pid), "baja")
            self.assertIsNone(sistema.terminar([hijo.pid]))
            self.assertEqual(hijo.wait(5), -15)
        finally:
            hijo.kill()
            hijo.wait()

    def test_rendimiento(self):
        r = sistema.Rendimiento()
        r.muestrear()
        time.sleep(0.3)
        d = r.muestrear(10, 20)
        self.assertGreater(d["mem"]["total"], 0)
        self.assertTrue(0 <= d["cpu"]["uso"] <= 100)
        self.assertEqual(len(d["cpu"]["nucleos"]), sistema.CPUS)
        self.assertEqual((d["cpu"]["procesos"], d["cpu"]["hilos"]), (10, 20))
        for disco in d["discos"]:
            self.assertGreater(disco["capacidad"], 0)


class Servicios(unittest.TestCase):
    def test_unir(self):
        unidades = [{"unit": "a.service", "load": "loaded", "active": "active", "sub": "running", "description": "A"},
                    {"unit": "b.service", "load": "loaded", "active": "failed", "sub": "failed", "description": "B"},
                    {"unit": "dbus-:1.14-x@0.service", "load": "loaded", "active": "active", "sub": "running", "description": "dbus-:1.14-x@0.service"},
                    {"unit": "c.socket", "load": "loaded", "active": "active", "sub": "listening", "description": "C"},
                    {"unit": "z.service", "load": "not-found", "active": "inactive", "sub": "dead", "description": "z"}]
        archivos = [{"unit_file": "a.service", "state": "enabled"}, {"unit_file": "d.service", "state": "disabled"},
                    {"unit_file": "b.service", "state": "static"}, {"unit_file": "t@.service", "state": "disabled"}]
        filas = servicios.unir(unidades, archivos)
        self.assertEqual([f["nombre"] for f in filas], ["b", "a", "d"])     # falló, corriendo, detenido
        self.assertTrue(filas[1]["cambiable"])
        self.assertFalse(filas[0]["cambiable"])
        self.assertEqual(filas[2]["activo"], "inactive")

    def test_accion_rechaza_cosas_raras(self):
        self.assertEqual(servicios.accion("--help", "start", True), "Acción no válida.")
        self.assertEqual(servicios.accion("a.service", "mask", True), "Acción no válida.")


class Modelo(unittest.TestCase):
    def test_solo_cambia_lo_que_cambio(self):
        from PySide6.QtCore import QCoreApplication
        from vortex_task_manager.modelo import ModeloFilas
        QCoreApplication.instance() or QCoreApplication([])
        m = ModeloFilas()
        eventos = []
        m.rowsInserted.connect(lambda _p, a, b: eventos.append(("+", a, b)))
        m.rowsRemoved.connect(lambda _p, a, b: eventos.append(("-", a, b)))
        m.modelReset.connect(lambda: eventos.append(("reset",)))
        m.actualizar([{"clave": k} for k in "abcd"])
        eventos.clear()
        m.actualizar([{"clave": k, "x": 1} for k in "abxcd"])
        self.assertEqual(eventos, [("+", 2, 2)])
        eventos.clear()
        m.actualizar([{"clave": k} for k in "bxd"])
        self.assertEqual(sorted(e[0] for e in eventos), ["-", "-"])
        self.assertEqual([f["clave"] for f in m.filas()], list("bxd"))
        self.assertNotIn(("reset",), eventos)


if __name__ == "__main__":
    unittest.main()
