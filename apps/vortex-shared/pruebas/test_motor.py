"""Pruebas de Vortexine Shared con un sendme falso (copia archivos): el motor y el servicio de punta a punta,
por un socket de verdad en este equipo. No abre puertos en tu red ni saca avisos en tu escritorio.

    python3 -m unittest discover -s apps/vortex-shared/pruebas
"""
from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ))

T = tempfile.TemporaryDirectory()
BASE = Path(T.name)
(BASE / "bin").mkdir()
os.environ.update(PATH=f"{BASE / 'bin'}:{os.environ['PATH']}", VORTEX_SHARED_CARPETA=str(BASE / "recibidos"),
                  XDG_CACHE_HOME=str(BASE / "cache"), XDG_CONFIG_HOME=str(BASE / "conf"), XDG_STATE_HOME=str(BASE / "estado"),
                  XDG_RUNTIME_DIR=str(BASE / "run"), VORTEX_SHARED_SIN_AVANCE="2", VORTEX_SHARED_ESPERA="2",
                  VORTEX_SHARED_INACTIVO="1")
SENDME = BASE / "bin" / "sendme"
SENDME.write_text(f"""#!/usr/bin/env python3
import base64, os, shutil, sys, time
o = sys.argv[1]
if o == "send":
    ruta = sys.argv[2]
    ticket = "blob" + base64.b32encode(ruta.encode()).decode().lower().rstrip("=") + "aaaaaaaaaa"
    open("{BASE}/ultimo-cwd", "w").write(os.getcwd())
    print("imported", ruta); print("to get this data, use"); print("sendme receive " + ticket, flush=True)
    time.sleep(600)
elif o == "receive":
    t = sys.argv[2][4:-10]
    if t.startswith("colgado"[:4]) or os.environ.get("PRUEBA_COLGADO"):
        os.mkdir(".sendme-recv-x"); time.sleep(600)
    ruta = base64.b32decode(t.upper() + "=" * (-len(t) % 8)).decode()
    os.mkdir(".sendme-recv-x"); open(".sendme-recv-x/blob", "w").write("x")
    nombre = os.path.basename(ruta)
    if os.path.isdir(ruta): shutil.copytree(ruta, nombre)
    else: shutil.copy(ruta, nombre)
    shutil.rmtree(".sendme-recv-x")
""")
NOTIFY = BASE / "bin" / "notify-send"
NOTIFY.write_text(f"""#!/bin/sh
for a in "$@"; do printf '%s\\n' "$a"; done >>"{BASE}/avisos"
case "$*" in
  *aceptar=*)
    [ -e "{BASE}/colgar-aviso" ] && {{ echo 7; exec sleep 60; }}
    echo 7; cat "{BASE}/respuesta" 2>/dev/null
    [ -e "{BASE}/respuesta2" ] && mv "{BASE}/respuesta2" "{BASE}/respuesta" ;;
esac
""")
GDBUS = BASE / "bin" / "gdbus"
GDBUS.write_text(f'#!/bin/sh\necho cerrado >>"{BASE}/avisos"\n')
for f in (SENDME, NOTIFY, GDBUS):
    f.chmod(0o755)

from vortex_shared import motor, servicio, thunar  # noqa: E402


def origen(*archivos: str) -> list[Path]:
    d = Path(tempfile.mkdtemp(dir=BASE))
    rutas = []
    for a in archivos:
        r = d / a
        r.parent.mkdir(parents=True, exist_ok=True)
        r.write_text(f"contenido de {a}")
        rutas.append(r)
    return rutas


class Base(unittest.TestCase):
    def setUp(self):
        import shutil
        shutil.rmtree(BASE / "recibidos", ignore_errors=True)
        for f in ("avisos", "respuesta", "respuesta2", "colgar-aviso"):
            (BASE / f).unlink(missing_ok=True)
        estado = BASE / "estado" / "vortex" / "modo-juego"
        estado.unlink(missing_ok=True)

    def recibidos(self) -> list[str]:
        d = BASE / "recibidos"
        return sorted(x.name for x in d.iterdir()) if d.exists() else []


class Reglas(unittest.TestCase):
    def test_quien_puede_avisar(self):
        for ip in ("192.168.1.5", "10.0.0.2", "172.16.1.1", "100.94.20.35", "127.0.0.1", "::1", "fd7a:115c::1", "::ffff:192.168.1.9", "fe80::1%enp3s0"):
            self.assertTrue(motor.ip_permitida(ip), ip)
        for ip in ("8.8.8.8", "138.186.28.249", "2001:4860::1", "::ffff:8.8.8.8", "basura", ""):
            self.assertFalse(motor.ip_permitida(ip), ip)

    def test_el_ticket_no_puede_ser_una_opcion_ni_llevar_nada_raro(self):
        base = {"v": 1, "usuario": "mike", "tam": 1, "nombres": ["a"]}
        for ticket in ("--help", "blob; rm -rf ~", "blob$(x)", "BLOBAAAAAAAAAAAA", "", None, 5, "blob"):
            with self.assertRaises(ValueError):
                motor.validar_peticion({**base, "ticket": ticket})
        self.assertTrue(motor.validar_peticion({**base, "ticket": "blob" + "a" * 20}))

    def test_peticiones_rotas(self):
        ok = {"v": 1, "ticket": "blob" + "a" * 20, "usuario": "mike", "tam": 1, "nombres": ["a"]}
        for malo in ([], "x", {**ok, "v": 9}, {**ok, "usuario": "a/b"}, {**ok, "tam": -1}, {**ok, "tam": True}, {**ok, "nombres": []}, {**ok, "nombres": "a"}):
            with self.assertRaises(ValueError):
                motor.validar_peticion(malo)

    def test_los_nombres_se_limpian(self):
        p = motor.validar_peticion({"v": 1, "ticket": "blob" + "a" * 20, "usuario": "m", "tam": 0,
                                    "nombres": ["<b>hola</b>\n&x", "z" * 300] + ["y"] * 50, "n": 52, "de": "a\nb"})
        self.assertEqual(p["nombres"][0], "bhola/bx")
        self.assertEqual(len(p["nombres"][1]), 80)
        self.assertEqual(len(p["nombres"]), motor.MAX_NOMBRES)
        self.assertEqual(p["de"], "ab")
        self.assertIn("y 51 más", motor.texto_peticion(p))

    def test_una_mac_lo_explica(self):
        with self.assertRaisesRegex(motor.ErrorShared, "MAC"):
            motor._destino_de("04:42:1a:08:6b:62")
        with self.assertRaises(motor.ErrorShared):
            motor._destino_de("")

    def test_tailscale(self):
        estado = {"Self": {"TailscaleIPs": ["100.1.1.1", "fd7a::1"]}, "Peer": {
            "a": {"HostName": "zeta", "Online": True, "OS": "linux", "TailscaleIPs": ["fd7a::2", "100.1.1.2"]},
            "b": {"HostName": "cel", "Online": True, "OS": "android", "TailscaleIPs": ["100.1.1.3"]},
            "c": {"HostName": "apagada", "Online": False, "OS": "linux", "TailscaleIPs": ["100.1.1.4"]},
            "d": {"HostName": "alfa", "Online": True, "OS": "linux", "TailscaleIPs": ["100.1.1.5"]}}}
        self.assertEqual(motor.compus_tailscale(estado), [{"nombre": "alfa", "ip": "100.1.1.5"}, {"nombre": "zeta", "ip": "100.1.1.2"}])
        self.assertEqual(motor.mis_direcciones(estado)[0]["ip"], "100.1.1.1")
        self.assertEqual(motor.compus_tailscale({}), [])

    def test_thunar_no_toca_lo_tuyo(self):
        uca = BASE / "conf" / "Thunar" / "uca.xml"
        uca.parent.mkdir(parents=True, exist_ok=True)
        uca.write_text("<actions><action><unique-id>mia</unique-id></action></actions>")
        thunar.poner_accion()
        thunar.poner_accion()
        xml = uca.read_text()
        self.assertIn("mia", xml)
        self.assertEqual(xml.count(thunar.ID_ACCION), 1)


class Envios(Base):
    def _ida_y_vuelta(self, rutas):
        envio = motor.Envio(rutas)
        try:
            return motor.recibir(envio.preparar()), envio
        finally:
            envio.terminar()

    def test_un_archivo(self):
        r, = origen("foto ñ.jpg")
        quedaron, envio = self._ida_y_vuelta([r])
        self.assertEqual([q.name for q in quedaron], ["foto ñ.jpg"])
        self.assertEqual(quedaron[0].read_text(), "contenido de foto ñ.jpg")
        self.assertFalse(envio._cwd.exists(), "no deja basura de sendme")

    def test_una_carpeta(self):
        r = origen("album/a.jpg", "album/sub/b.jpg")
        quedaron, _ = self._ida_y_vuelta([r[0].parent])
        self.assertEqual(quedaron[0].name, "album")
        self.assertTrue((quedaron[0] / "sub" / "b.jpg").exists())

    def test_varios_llegan_sueltos_sin_el_lote(self):
        a, b = origen("a.txt", "b.txt")
        quedaron, envio = self._ida_y_vuelta([a, b])
        self.assertEqual(sorted(q.name for q in quedaron), ["a.txt", "b.txt"])
        self.assertEqual(self.recibidos(), ["a.txt", "b.txt"])
        self.assertFalse(envio._lote.exists())
        self.assertTrue(a.exists(), "el original sigue")

    def test_varios_del_mismo_nombre(self):
        a, = origen("foto.jpg")
        b, = origen("foto.jpg")
        quedaron, _ = self._ida_y_vuelta([a, b])
        self.assertEqual(sorted(q.name for q in quedaron), ["foto (2).jpg", "foto.jpg"])

    def test_no_pisa_lo_que_tienes(self):
        a, = origen("nota.txt")
        (BASE / "recibidos").mkdir(parents=True, exist_ok=True)
        (BASE / "recibidos" / "nota.txt").write_text("mia")
        quedaron, _ = self._ida_y_vuelta([a])
        self.assertEqual(quedaron[0].name, "nota (2).txt")
        self.assertEqual((BASE / "recibidos" / "nota.txt").read_text(), "mia")

    def test_si_el_otro_se_apago_corta_y_limpia(self):
        os.environ["PRUEBA_COLGADO"] = "1"
        try:
            with self.assertRaisesRegex(motor.ErrorShared, "No llega nada"):
                motor.recibir("blob" + "a" * 20)
        finally:
            del os.environ["PRUEBA_COLGADO"]
        self.assertEqual(self.recibidos(), [])
        self.assertEqual(list((BASE / "recibidos").glob(".*")), [])

    def test_sendme_no_sobrevive_si_se_cae_quien_manda(self):
        r, = origen("secreto.txt")
        guion = (f"import sys; sys.path.insert(0, {str(RAIZ)!r})\nfrom vortex_shared import motor\n"
                 f"e = motor.Envio([{str(r)!r}]); print(e.preparar(), flush=True)\nimport time; time.sleep(60)\n")
        hijo = subprocess.Popen([sys.executable, "-c", guion], stdout=subprocess.PIPE, text=True)
        ticket = hijo.stdout.readline().strip()
        self.assertTrue(ticket.startswith("blob"))
        marca = str(r)
        vivos = lambda: subprocess.run(["pgrep", "-f", f"sendme send {marca}"], capture_output=True).returncode == 0  # noqa: E731
        self.assertTrue(vivos())
        hijo.kill()
        hijo.wait()
        hijo.stdout.close()
        for _ in range(30):
            if not vivos():
                break
            time.sleep(0.1)
        self.assertFalse(vivos(), "sigue compartiendo el archivo")

    def test_codigo_malo(self):
        with self.assertRaisesRegex(motor.ErrorShared, "código"):
            motor.recibir("--help")

    def test_falta_el_archivo(self):
        with self.assertRaisesRegex(motor.ErrorShared, "No encuentro"):
            motor.Envio([BASE / "no-existe"])


class DePuntaAPunta(Base):
    def setUp(self):
        super().setUp()
        self.oyente = socket.socket()
        self.oyente.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.oyente.bind(("127.0.0.1", 0))
        self.oyente.listen()
        self.puerto = self.oyente.getsockname()[1]
        self.servicio = servicio.Servicio(self.oyente)
        self.hilo = threading.Thread(target=self.servicio.correr, daemon=True)
        self.hilo.start()
        self.puerto_antes = motor.PUERTO
        motor.PUERTO = self.puerto
        self.addCleanup(self._apagar)

    def _apagar(self):
        motor.PUERTO = self.puerto_antes
        self.servicio.ultimo = 0
        self.hilo.join(5)
        self.oyente.close()

    def _destino(self):
        return motor._destino_de("127.0.0.1")

    def _mandar(self, rutas, usuario=None):
        import getpass
        envio = motor.Envio(rutas)
        estados = []
        try:
            envio.preparar()
            motor.mandar("127.0.0.1", usuario or getpass.getuser(), envio, lambda e, b: estados.append(e))
        finally:
            envio.terminar()
        return estados

    def test_aceptado(self):
        (BASE / "respuesta").write_text("aceptar\n")
        a, b = origen("uno.txt", "dos.txt")
        estados = self._mandar([a, b])
        self.assertEqual(self.recibidos(), ["dos.txt", "uno.txt"])
        self.assertIn("mandando", estados)
        avisos = (BASE / "avisos").read_text()
        self.assertIn("te quiere mandar archivos", avisos)
        self.assertIn("uno.txt y 1 más", avisos)
        self.assertIn("Desde 127.0.0.1", avisos)
        self.assertIn("critical", avisos)

    def test_picarle_a_la_tarjeta_no_acepta(self):
        (BASE / "respuesta").write_text("default\n")
        (BASE / "respuesta2").write_text("rechazar\n")
        a, = origen("uno.txt")
        with self.assertRaisesRegex(motor.ErrorShared, "dijeron que no"):
            self._mandar([a])
        self.assertEqual(self.recibidos(), [])
        self.assertEqual((BASE / "avisos").read_text().count("te quiere mandar"), 2, "el aviso vuelve a salir")

    def test_rechazado_no_baja_nada(self):
        (BASE / "respuesta").write_text("rechazar\n")
        a, = origen("uno.txt")
        with self.assertRaisesRegex(motor.ErrorShared, "dijeron que no"):
            self._mandar([a])
        self.assertEqual(self.recibidos(), [])

    def test_nadie_contesta_y_se_cierra_el_aviso(self):
        (BASE / "colgar-aviso").touch()
        a, = origen("uno.txt")
        with self.assertRaisesRegex(motor.ErrorShared, "dijeron que no"):
            self._mandar([a])
        self.assertIn("cerrado", (BASE / "avisos").read_text())

    def test_otro_usuario(self):
        a, = origen("uno.txt")
        with self.assertRaisesRegex(motor.ErrorShared, "no tiene ese usuario"):
            self._mandar([a], "alguien-que-no-existe")
        self.assertFalse((BASE / "avisos").exists(), "ni siquiera sale el aviso")

    def test_jugando_no_molesta(self):
        marca = BASE / "estado" / "vortex" / "modo-juego"
        marca.parent.mkdir(parents=True, exist_ok=True)
        marca.touch()
        a, = origen("uno.txt")
        with self.assertRaisesRegex(motor.ErrorShared, "ocupada"):
            self._mandar([a])
        self.assertFalse((BASE / "avisos").exists())

    def test_uno_a_la_vez(self):
        self.assertTrue(self.servicio.ocupado.acquire(blocking=False))
        try:
            a, = origen("uno.txt")
            with self.assertRaisesRegex(motor.ErrorShared, "ocupada"):
                self._mandar([a])
        finally:
            self.servicio.ocupado.release()

    def test_basura_no_lo_tira(self):
        with socket.create_connection(self._destino(), timeout=3) as c:
            c.sendall(b"esto no es json\n")
            self.assertIn(b"error", c.recv(100))
        with socket.create_connection(self._destino(), timeout=3) as c:
            c.sendall(b"x" * 200000)
        with socket.create_connection(self._destino(), timeout=3) as c:
            c.sendall(json.dumps({"v": 1, "ticket": "--help", "usuario": "a", "tam": 0, "nombres": ["a"]}).encode() + b"\n")
            self.assertIn(b"error", c.recv(100))
        self.assertFalse((BASE / "avisos").exists())

    def test_de_internet_ni_contesta(self):
        servidor, cliente = socket.socketpair()
        with cliente:
            self.servicio.atender(servidor, "8.8.8.8")
            cliente.settimeout(1)
            self.assertEqual(cliente.recv(10), b"")

    def test_se_apaga_solo(self):
        self.servicio.ultimo = time.monotonic() - 100
        self.hilo.join(4)
        self.assertFalse(self.hilo.is_alive(), "no se queda corriendo")


def tearDownModule():
    T.cleanup()


if __name__ == "__main__":
    unittest.main()
