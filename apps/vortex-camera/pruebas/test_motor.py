"""Pruebas del motor de Vortexine's Camera, sin pantalla y sin cámaras de verdad: una cámara falsa por
RTSP (camara_falsa.py), una que contesta la búsqueda DHIP y un servidor ONVIF/CGI de mentiras en
127.0.0.1. Todo se guarda en carpetas temporales: no toca tu configuración ni tus fotos.

    python3 -m unittest discover -s apps/vortex-camera/pruebas
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from datetime import datetime
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from camara_falsa import CamaraFalsa  # noqa: E402
from vortex_camera import descubrir, grabar, ptz  # noqa: E402
from vortex_camera.camaras import Almacen, Camara, limpiar_serie  # noqa: E402


class PruebaCamaras(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.alm = Almacen(self.dir.name)

    def tearDown(self):
        self.dir.cleanup()

    def test_urls_de_imou(self):
        c = Camara.desde({"id": "a", "nombre": "Patio", "codigo": "L2C3 D4E5", "ip": "192.168.1.20"})
        self.assertEqual(c.url_vivo(), "rtsp://admin:L2C3%20D4E5@192.168.1.20:554/cam/realmonitor?channel=1&subtype=1")
        self.assertTrue(c.url_vivo(principal=True).endswith("subtype=0"))
        url = c.url_grabado(datetime(2026, 10, 30, 14, 5), datetime(2026, 10, 30, 15, 0))
        self.assertIn("/cam/playback?channel=1&starttime=2026_10_30_14_05_00&endtime=2026_10_30_15_00_00", url)
        self.assertEqual(Camara.desde({"id": "b", "nombre": "x"}).url_vivo(), "")    # sin IP no hay URL
        self.assertIn("@[fd7a::1]:554/", Camara.desde({"id": "c", "nombre": "x", "ip": "fd7a::1", "codigo": "a"}).url_vivo())

    def test_el_codigo_no_va_a_la_interfaz(self):
        c = Camara.desde({"id": "a", "nombre": "Patio", "codigo": "SECRETO1"})
        self.assertNotIn("SECRETO1", json.dumps(c.publica()))
        self.assertTrue(c.publica()["tieneCodigo"])

    def test_serie_y_datos_raros(self):
        self.assertEqual(limpiar_serie(" 9a0 b-1234 "), "9A0B1234")
        c = Camara.desde({"id": "a", "nombre": "  ", "puerto": "abc", "canal": -3, "otro": 1})
        self.assertEqual((c.nombre, c.puerto, c.canal), ("Cámara", 554, 1))

    def test_archivo_privado_y_recargable(self):
        self.alm.agregar({"nombre": "Patio", "codigo": "X", "ip": "10.0.0.2"})
        modo = stat.S_IMODE(os.stat(self.alm.ruta).st_mode)
        self.assertEqual(modo, 0o600)
        otro = Almacen(self.dir.name)
        self.assertEqual([c.nombre for c in otro.camaras], ["Patio"])
        self.assertEqual(otro.camaras[0].codigo, "X")

    def test_archivo_roto_no_tumba(self):
        Path(self.alm.ruta).write_text("{ esto no es json", encoding="utf-8")
        otro = Almacen(self.dir.name)
        self.assertEqual(otro.camaras, [])
        self.assertEqual(len(otro.ventanas), 1)

    def test_el_mosaico_crece_solo(self):
        ids = [self.alm.agregar({"nombre": f"C{n}", "codigo": "x", "ip": f"10.0.0.{n}"}).id for n in range(1, 27)]
        v = self.alm.ventanas[0]
        self.assertEqual(v["division"], 32)                   # 26 cámaras: de 4 a 9, 16, 25 y 32
        self.assertEqual(v["celdas"][:26], ids)
        for n in range(27, 34):
            self.alm.agregar({"nombre": f"C{n}", "codigo": "x", "ip": "10.0.1.1"})
        self.assertEqual(len(self.alm.ventanas), 2)            # la 33 ya no cabe: otra ventana
        self.assertEqual(self.alm.ventanas[0]["celdas"], [c.id for c in self.alm.camaras[:32]])
        self.assertEqual(self.alm.ventanas[1]["celdas"][0], self.alm.camaras[32].id)

    def test_poner_celda_intercambia(self):
        a = self.alm.agregar({"nombre": "A", "codigo": "x", "ip": "1.1.1.1"}).id
        b = self.alm.agregar({"nombre": "B", "codigo": "x", "ip": "1.1.1.2"}).id
        self.alm.poner_celda(0, 1, a)
        self.assertEqual(self.alm.ventanas[0]["celdas"][:2], [b, a])
        self.alm.poner_celda(0, 0, "")
        self.alm.poner_celda(0, 9, a)                          # fuera de la división: no hace nada
        self.assertEqual(self.alm.ventanas[0]["celdas"][:2], ["", a])
        self.alm.quitar(a)
        self.assertNotIn(a, self.alm.ventanas[0]["celdas"])

    def test_editar_codigo_olvida_ptz(self):
        c = self.alm.agregar({"nombre": "A", "codigo": "x", "ip": "1.1.1.1"})
        self.alm.actualizar(c.id, {"ptz": "onvif"})
        self.assertEqual(self.alm.camara(c.id).ptz, "onvif")
        self.alm.actualizar(c.id, {"nombre": "Otro"})
        self.assertEqual(self.alm.camara(c.id).ptz, "onvif")
        self.alm.actualizar(c.id, {"codigo": "y"})
        self.assertEqual(self.alm.camara(c.id).ptz, "")

    def test_ventanas(self):
        i = self.alm.nueva_ventana()
        self.alm.renombrar_ventana(i, "  Calle  ")
        self.alm.poner_division(i, 9)
        self.assertEqual(self.alm.ventanas[i]["nombre"], "Calle")
        self.assertEqual(len(self.alm.ventanas[i]["celdas"]), 9)
        self.alm.poner_division(i, 7)                          # no existe esa división
        self.assertEqual(self.alm.ventanas[i]["division"], 9)
        self.alm.quitar_ventana(0)
        self.alm.quitar_ventana(0)                             # la última nunca se quita
        self.assertEqual(len(self.alm.ventanas), 1)


class PruebaDescubrir(unittest.TestCase):
    def test_paquete(self):
        p = descubrir.paquete_busqueda()
        self.assertEqual(p[:8], b"\x20\x00\x00\x00DHIP")
        n = struct.unpack("<I", p[16:20])[0]
        self.assertEqual(len(p), 32 + n)
        self.assertEqual(json.loads(p[32:])["method"], "DHDiscover.search")

    def test_busqueda_con_una_camara_que_contesta(self):
        respondedor = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        respondedor.bind(("127.0.0.1", 0))
        puerto = respondedor.getsockname()[1]
        info = {"method": "client.notifyDevInfo", "params": {"deviceInfo": {
            "SerialNo": "9a0b1c2d", "DeviceType": "IPC-K42", "Mac": "AA:BB:CC:00:11:22",
            "IPv4Address": {"IPAddress": "192.168.1.77"}}}}

        def contestar():
            datos, origen = respondedor.recvfrom(4096)
            assert b"DHDiscover.search" in datos
            cuerpo = json.dumps(info).encode()
            respondedor.sendto(b"\x20\x00\x00\x00DHIP" + bytes(24) + cuerpo, origen)
            respondedor.sendto(b"basura", origen)

        hilo = threading.Thread(target=contestar, daemon=True)
        hilo.start()
        halladas = descubrir.buscar(1.0, destinos=[("127.0.0.1", puerto)])
        respondedor.close()
        self.assertEqual(halladas, [{"serie": "9A0B1C2D", "ip": "192.168.1.77", "modelo": "IPC-K42",
                                     "mac": "aa:bb:cc:00:11:22", "version": ""}])

    def test_interpretar_ignora_lo_propio(self):
        self.assertIsNone(descubrir.interpretar(descubrir.paquete_busqueda()))
        self.assertIsNone(descubrir.interpretar(b"DHIP{roto"))


class PruebaConCamaraFalsa(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cam = CamaraFalsa()

    @classmethod
    def tearDownClass(cls):
        cls.cam.cerrar()

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.dir.cleanup()

    def test_probar_rtsp(self):
        ruta = "/cam/realmonitor?channel=1&subtype=1"
        self.assertEqual(descubrir.probar_rtsp("127.0.0.1", self.cam.puerto, ruta, "admin", self.cam.codigo), "ok")
        self.assertEqual(descubrir.probar_rtsp("127.0.0.1", self.cam.puerto, ruta, "admin", "MALO"), "codigo")
        libre = socket.socket()
        libre.bind(("127.0.0.1", 0))
        puerto_libre = libre.getsockname()[1]
        libre.close()
        self.assertEqual(descubrir.probar_rtsp("127.0.0.1", puerto_libre, ruta, "admin", "x", 1), "sin_conexion")

    def test_autorizacion_digest(self):
        cab = descubrir._autorizacion('Digest realm="Login to 9A0B", nonce="abc"', "DESCRIBE", "rtsp://h/x", "admin", "L2C")
        ha1 = hashlib.md5(b"admin:Login to 9A0B:L2C").hexdigest()
        ha2 = hashlib.md5(b"DESCRIBE:rtsp://h/x").hexdigest()
        self.assertIn(f'response="{hashlib.md5(f"{ha1}:abc:{ha2}".encode()).hexdigest()}"', cab)

    def test_foto(self):
        ruta = grabar.foto(self.cam.url(), Path(self.dir.name), "Patio/Cochera")
        self.assertTrue(ruta.name.startswith("Patio Cochera "))
        self.assertGreater(ruta.stat().st_size, 1000)
        with self.assertRaises(RuntimeError) as e:
            grabar.foto(self.cam.url("MALO"), Path(self.dir.name), "Patio")
        self.assertIn("código", str(e.exception))
        self.assertEqual(len(list(Path(self.dir.name).iterdir())), 1)    # la fallida no deja archivo

    def test_grabar(self):
        g = grabar.Grabacion(self.cam.url(), Path(self.dir.name), "Patio")
        time.sleep(3)
        self.assertTrue(g.activa)
        ruta = g.detener()
        self.assertFalse(g.activa)
        duracion = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                                   str(ruta)], capture_output=True, text=True).stdout.strip()
        self.assertGreater(float(duracion), 1)

    def test_grabacion_con_codigo_malo_se_corta_sola(self):
        g = grabar.Grabacion(self.cam.url("MALO"), Path(self.dir.name), "Patio")
        limite = time.monotonic() + 10
        while g.activa and time.monotonic() < limite:
            time.sleep(0.1)
        self.assertFalse(g.activa)
        self.assertIn("código", g.error())
        self.assertIsNone(g.detener())


class _Onvif(BaseHTTPRequestHandler):
    vistos: list = []
    clave = "L2C3D4E5"
    cgi = False

    def log_message(self, *a):
        pass

    def do_POST(self):
        cuerpo = self.rfile.read(int(self.headers["Content-Length"])).decode()
        if _Onvif.cgi:
            self.send_response(404)
            self.end_headers()
            return
        import re
        nonce = base64.b64decode(re.search(r"<Nonce[^>]*>([^<]+)<", cuerpo).group(1))
        creado = re.search(r"<Created[^>]*>([^<]+)<", cuerpo).group(1)
        resumen = re.search(r"<Password[^>]*>([^<]+)<", cuerpo).group(1)
        bien = base64.b64encode(hashlib.sha1(nonce + creado.encode() + self.clave.encode()).digest()).decode()
        if resumen != bien:
            self.send_response(400)
            self.end_headers()
            return
        accion = re.search(r"<s:Body><(\w+)", cuerpo).group(1)
        _Onvif.vistos.append((self.path, accion, re.search(r'x="([^"]+)"', cuerpo).group(1) if "x=" in cuerpo else ""))
        respuesta = ('<Envelope><Body><GetProfilesResponse><trt:Profiles token="MediaProfile000" fixed="true">'
                     '</trt:Profiles></GetProfilesResponse></Body></Envelope>') if accion == "GetProfiles" else "<ok/>"
        self.send_response(200)
        self.end_headers()
        self.wfile.write(respuesta.encode())

    def do_GET(self):
        import re
        auth = self.headers.get("Authorization", "")
        campos = dict(re.findall(r'(\w+)="?([^",]*)"?', auth))
        ha1 = hashlib.md5(f"admin:Login to falsa:{self.clave}".encode()).hexdigest()
        ha2 = hashlib.md5(f"GET:{campos.get('uri', '')}".encode()).hexdigest()
        bien = hashlib.md5(f"{ha1}:n1:{campos.get('nc')}:{campos.get('cnonce')}:auth:{ha2}".encode()).hexdigest()
        if campos.get("response") != bien:
            self.send_response(401)
            self.send_header("WWW-Authenticate", 'Digest realm="Login to falsa", nonce="n1", qop="auth"')
            self.end_headers()
            return
        _Onvif.vistos.append((self.path.split("?")[0], self.path.split("action=")[1].split("&")[0],
                              self.path.split("code=")[1].split("&")[0]))
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"OK\r\n")


class PruebaPtz(unittest.TestCase):
    def setUp(self):
        _Onvif.vistos = []
        _Onvif.cgi = False
        ptz._PERFILES.clear()
        self.servidor = HTTPServer(("127.0.0.1", 0), _Onvif)
        threading.Thread(target=self.servidor.serve_forever, daemon=True).start()
        self.cam = Camara.desde({"id": "p", "nombre": "PTZ", "codigo": _Onvif.clave,
                                 "ip": f"127.0.0.1:{self.servidor.server_port}"})

    def tearDown(self):
        self.servidor.shutdown()
        self.servidor.server_close()

    def test_onvif(self):
        self.assertEqual(ptz.mover(self.cam, "izquierda"), "onvif")
        self.cam.ptz = "onvif"
        ptz.parar(self.cam)
        self.assertEqual(_Onvif.vistos, [("/onvif/media_service", "GetProfiles", ""),
                                         ("/onvif/ptz_service", "ContinuousMove", "-0.5"),
                                         ("/onvif/ptz_service", "Stop", "")])

    def test_cgi_si_no_hay_onvif(self):
        _Onvif.cgi = True
        self.assertEqual(ptz.mover(self.cam, "arriba"), "cgi")
        self.cam.ptz = "cgi"
        ptz.parar(self.cam, "arriba")
        self.assertEqual(_Onvif.vistos, [("/cgi-bin/ptz.cgi", "start", "Up"), ("/cgi-bin/ptz.cgi", "stop", "Up")])

    def test_clave_mala(self):
        self.cam.codigo = "MALO"
        _Onvif.vistos = []
        with self.assertRaises(ptz.SinPtz):
            ptz.mover(self.cam, "abajo")


if __name__ == "__main__":
    unittest.main()
