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
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from vortex_task_manager import servicios, sistema  # noqa: E402


def proc(pid, ppid, nombre, uid=1000, cpu=0.0, mem=0, disco=0.0, estado="sleeping", vram=0):
    return {"pid": pid, "ppid": ppid, "nombre": nombre, "comando": nombre, "uid": uid, "usuario": "mike" if uid == 1000 else "root",
            "cpu": cpu, "mem": mem, "hilos": 1, "estado": estado, "disco": disco, "vram": vram}


MIB = 1024 ** 2

# Como la da `nvidia-smi -q -d PIDS` (driver 615, RTX 3080), con una segunda tarjeta y un proceso sin dato.
NVIDIA_PIDS = """
==============NVSMI LOG==============

Timestamp                                              : Sat Nov  7 19:04:28 2026
Driver Version                                         : 615.78.08

Attached GPUs                                          : 2
GPU 00000000:01:00.0
    Processes
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 1226
            Type                          : G
            Name                          : Hyprland
            Used GPU Memory               : 107 MiB
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 2874
            Type                          : C+G
            Name                          : /usr/lib/claude/claude --type=gpu-process --x=a:b
            Used GPU Memory               : 90 MiB
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 5000
            Type                          : C
            Name                          : ollama
            Used GPU Memory               : 6144 MiB
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 5001
            Type                          : G
            Name                          : raro
            Used GPU Memory               : Not available
GPU 00000000:02:00.0
    Processes
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 5000
            Type                          : C
            Name                          : ollama
            Used GPU Memory               : 2048 MiB
"""

FDINFO_AMD = """pos:	0
flags:	02100002
mnt_id:	26
ino:	1046
drm-driver:	amdgpu
drm-client-id:	17
drm-pdev:	0000:03:00.0
pasid:	32790
drm-memory-vram:	524288 KiB
drm-memory-gtt: 	2048 KiB
drm-memory-cpu: 	0 KiB
drm-engine-gfx:	123456 ns
"""

FDINFO_AMD_NUEVO = """drm-driver:	amdgpu
drm-client-id:	18
drm-pdev:	0000:03:00.0
drm-memory-vram:	999 KiB
drm-total-vram:	300 MiB
drm-resident-vram:	256 MiB
drm-total-gtt:	8 MiB
"""

FDINFO_I915_INTEGRADA = """drm-driver:	i915
drm-client-id:	5
drm-pdev:	0000:00:02.0
drm-total-system0:	64 MiB
drm-resident-system0:	48 MiB
drm-total-stolen-system0:	0
"""

FDINFO_XE = """drm-driver:	xe
drm-client-id:	9
drm-pdev:	0000:03:00.0
drm-total-system:	10 MiB
drm-resident-system:	10 MiB
drm-total-vram0:	200 MiB
drm-resident-vram0:	128 MiB
"""

FDINFO_NVIDIA_DRM = """pos:	0
flags:	02100002
drm-driver:	nvidia-drm
drm-client-id:	3
drm-pdev:	0000:01:00.0
"""


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

    def test_vram_se_suma_por_app_y_se_ordena(self):
        filas = [proc(100, 1, "kitty", mem=50), proc(101, 100, "llama-server", vram=6000 * MIB),
                 proc(200, 1, "firefox", mem=900, vram=300 * MIB), proc(201, 200, "firefox", vram=100 * MIB),
                 proc(300, 1, "pipewire")]
        ventanas = [{"pid": 100, "clase": "kitty", "titulo": "~", "direccion": "0x1"},
                    {"pid": 200, "clase": "firefox", "titulo": "YouTube", "direccion": "0x2"}]
        g = sistema.agrupar(filas, ventanas, self.apps, uid=1000)
        apps = {x["nombre"]: x for x in g["apps"]}
        self.assertEqual(apps["kitty"]["vram"], 6000 * MIB)       # la IA que corre dentro de la terminal
        self.assertEqual(apps["firefox"]["vram"], 400 * MIB)
        self.assertEqual(g["fondo"][0]["vram"], 0)
        self.assertEqual([x["nombre"] for x in sistema.ordenar(g["apps"], "vram")], ["kitty", "firefox"])
        self.assertEqual([x["nombre"] for x in sistema.ordenar(g["apps"], "mem")], ["firefox", "kitty"])
        self.assertIn("vram", sistema.ORDENES)


class Vram(unittest.TestCase):
    def test_a_bytes(self):
        self.assertEqual(sistema._a_bytes("865 MiB"), 865 * MIB)
        self.assertEqual(sistema._a_bytes("524288 KiB"), 512 * MIB)
        self.assertEqual(sistema._a_bytes("4096"), 4096)
        self.assertEqual(sistema._a_bytes("2 GiB"), 2048 * MIB)
        self.assertIsNone(sistema._a_bytes("N/A"))
        self.assertIsNone(sistema._a_bytes("Not available"))

    def test_nvidia_trae_graficos_y_computo_y_suma_tarjetas(self):
        vram = sistema.parsear_pids_nvidia(NVIDIA_PIDS)
        self.assertEqual(vram, {1226: 107 * MIB, 2874: 90 * MIB, 5000: 8192 * MIB})   # sin «Not available»
        self.assertEqual(sistema.parsear_pids_nvidia("GPU 00000000:01:00.0\n    Processes : None\n"), {})

    def test_vram_nvidia_sin_nvidia_smi_o_si_falla(self):
        with mock.patch.object(sistema.shutil, "which", return_value=None):
            self.assertIsNone(sistema.vram_nvidia())
        roto = subprocess.CompletedProcess([], 9, "NVIDIA-SMI has failed because it couldn't communicate with the driver", "")
        with mock.patch.object(sistema.shutil, "which", return_value="/usr/bin/nvidia-smi"), \
             mock.patch.object(sistema.subprocess, "run", return_value=roto):
            self.assertIsNone(sistema.vram_nvidia())
        bien = subprocess.CompletedProcess([], 0, NVIDIA_PIDS, "")
        with mock.patch.object(sistema.shutil, "which", return_value="/usr/bin/nvidia-smi"), \
             mock.patch.object(sistema.subprocess, "run", return_value=bien) as run:
            self.assertEqual(sistema.vram_nvidia()[5000], 8192 * MIB)
            self.assertEqual(run.call_args[0][0], ["nvidia-smi", "-q", "-d", "PIDS"])

    def test_fdinfo_segun_el_driver(self):
        self.assertEqual(sistema.parsear_fdinfo_drm(FDINFO_AMD), ("0000:03:00.0/17", 512 * MIB))
        self.assertEqual(sistema.parsear_fdinfo_drm(FDINFO_AMD_NUEVO), ("0000:03:00.0/18", 256 * MIB))  # resident gana
        self.assertEqual(sistema.parsear_fdinfo_drm(FDINFO_XE), ("0000:03:00.0/9", 128 * MIB))          # vram, no system
        self.assertEqual(sistema.parsear_fdinfo_drm(FDINFO_I915_INTEGRADA), ("0000:00:02.0/5", 48 * MIB))
        self.assertEqual(sistema.parsear_fdinfo_drm(FDINFO_NVIDIA_DRM), ("0000:01:00.0/3", None))       # no dice
        self.assertIsNone(sistema.parsear_fdinfo_drm("pos:\t0\nflags:\t02\nmnt_id:\t1\n"))              # no es de la gráfica

    def _proc_falso(self) -> Path:
        raiz = Path(tempfile.mkdtemp())

        def abrir(pid, fd, destino, info):
            (raiz / str(pid) / "fd").mkdir(parents=True, exist_ok=True)
            (raiz / str(pid) / "fdinfo").mkdir(parents=True, exist_ok=True)
            os.symlink(destino, raiz / str(pid) / "fd" / str(fd))
            (raiz / str(pid) / "fdinfo" / str(fd)).write_text(info)
        abrir(10, 3, "/dev/dri/renderD128", FDINFO_AMD)
        abrir(10, 4, "/dev/dri/renderD128", FDINFO_AMD)          # el mismo cliente duplicado (dup)
        abrir(10, 5, "/dev/dri/card1", FDINFO_AMD_NUEVO)         # otro cliente del mismo proceso
        abrir(10, 6, "/home/mike/x.txt", FDINFO_AMD)             # no es de /dev/dri: no cuenta
        abrir(11, 3, "/dev/dri/renderD128", FDINFO_AMD)          # hijo de un fork con el fd heredado
        abrir(12, 3, "/dev/dri/renderD128", FDINFO_NVIDIA_DRM)
        (raiz / "13").mkdir()                                   # sin fd/: de otro usuario
        return raiz

    def test_fdinfo_suma_por_proceso_sin_duplicados(self):
        raiz = self._proc_falso()
        vram, supo = sistema.vram_fdinfo([13, 12, 11, 10, 99], raiz)
        self.assertTrue(supo)
        self.assertEqual(vram, {10: 512 * MIB + 256 * MIB})
        vram, supo = sistema.vram_fdinfo([12, 13], raiz)          # solo nvidia-drm: no se sabe
        self.assertEqual((vram, supo), ({}, False))

    def test_memoria_grafica_mide_cada_3_segundos(self):
        llamadas = []

        def falso():
            llamadas.append(1)
            return {5000: 6144 * MIB}
        with mock.patch.object(sistema, "vram_nvidia", side_effect=falso):
            m = sistema.MemoriaGrafica(nvidia=True, drm=False)
            self.assertFalse(m.disponible)
            self.assertEqual(m.medir([5000], ahora=100.0), {5000: 6144 * MIB})
            self.assertTrue(m.disponible)
            m.medir([5000], ahora=101.0)
            m.medir([5000], ahora=102.9)
            self.assertEqual(len(llamadas), 1)                    # entre medidas, la última
            m.medir([5000], ahora=103.0)
            self.assertEqual(len(llamadas), 2)

    def test_sin_forma_de_saber_no_hay_columna(self):
        with mock.patch.object(sistema, "vram_nvidia", return_value=None):
            m = sistema.MemoriaGrafica(nvidia=True, drm=False)
            self.assertEqual(m.medir([1], ahora=0.0), {})
            self.assertFalse(m.disponible)
        m = sistema.MemoriaGrafica(nvidia=False, drm=False)
        self.assertEqual(m.medir([1], ahora=0.0), {})
        self.assertFalse(m.disponible)

    def test_nvidia_y_amd_juntas(self):
        raiz = self._proc_falso()
        real = sistema.vram_fdinfo
        with mock.patch.object(sistema, "vram_nvidia", return_value={10: 100 * MIB, 5000: 6144 * MIB}), \
             mock.patch.object(sistema, "vram_fdinfo", side_effect=lambda pids: real(pids, raiz)):
            m = sistema.MemoriaGrafica(nvidia=True, drm=True)
            self.assertEqual(m.medir([10, 5000], ahora=0.0), {10: 868 * MIB, 5000: 6144 * MIB})


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
            self.assertIsNone(sistema.suspender([hijo.pid], True))     # en pausa también termina
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
