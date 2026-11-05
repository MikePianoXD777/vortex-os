#!/usr/bin/env python3
"""Prueba de bin/vortex-bt-agente, sin tocar tu Bluetooth ni tu escritorio: un bus de D-Bus propio, un BlueZ falso en
él y un notify-send falso que contesta lo que le pida cada prueba.

    python3 -I pruebas/bt-agente.py
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

AGENTE = Path(__file__).resolve().parent.parent / "bin" / "vortex-bt-agente"
RUTA = "/org/vortexine/bt_agente"
DISPOSITIVO = "/org/bluez/hci0/dev_98_80_BB_41_01_D5"
XML = Gio.DBusNodeInfo.new_for_xml("""
<node>
  <interface name="org.bluez.AgentManager1">
    <method name="RegisterAgent"><arg type="o" direction="in"/><arg type="s" direction="in"/></method>
    <method name="RequestDefaultAgent"><arg type="o" direction="in"/></method>
  </interface>
  <interface name="org.freedesktop.DBus.Properties">
    <method name="Get">
      <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/>
    </method>
  </interface>
</node>""")


class BluezFalso:
    def __init__(self, direccion: str, alias="SOUNDPEATS H3"):
        self.alias = alias
        self.llamadas: list[tuple] = []
        self.bus = Gio.DBusConnection.new_for_address_sync(
            direccion, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        self.registros = [self.bus.register_object("/org/bluez", XML.interfaces[0], self.manejar, None, None),
                          self.bus.register_object(DISPOSITIVO, XML.interfaces[1], self.manejar, None, None)]
        self.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName",
                           GLib.Variant("(su)", ("org.bluez", 0)), None, Gio.DBusCallFlags.NONE, 3000, None)

    def manejar(self, _b, _e, _r, _i, metodo, parametros, invocacion):
        if metodo == "Get":
            invocacion.return_value(GLib.Variant("(v)", (GLib.Variant("s", self.alias),)))
            return
        self.llamadas.append((metodo, *parametros.unpack()))
        invocacion.return_value(None)

    def cerrar(self):
        if not self.bus.is_closed():
            self.bus.close_sync(None)


class Prueba(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        t = Path(cls.tmp.name)
        cls.t = t
        cls.conf = t / "bus.conf"
        cls.conf.write_text(f"""<busconfig><type>session</type><listen>unix:path={t}/bus</listen>
          <policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/>
          <allow own="*"/><allow user="*"/></policy></busconfig>""")
        cls.dbus = subprocess.Popen(["dbus-daemon", f"--config-file={cls.conf}", "--nofork"], stderr=subprocess.DEVNULL)
        cls.direccion = f"unix:path={t}/bus"
        for _ in range(50):
            if (t / "bus").exists():
                break
            time.sleep(0.05)
        (t / "bin").mkdir()
        falso = t / "bin" / "notify-send"
        falso.write_text(f"""#!/bin/sh
for a in "$@"; do printf '%s\\n' "$a"; done >>"{t}/aviso"
respuesta=$(cat "{t}/respuesta" 2>/dev/null)
case "$respuesta" in colgar) exec sleep 60;; esac
case "$*" in *"-A"*) printf '%s\\n' "$respuesta";; esac
""")
        falso.chmod(0o755)

    @classmethod
    def tearDownClass(cls):
        cls.dbus.terminate()
        cls.dbus.wait()
        cls.tmp.cleanup()

    def setUp(self):
        (self.t / "aviso").unlink(missing_ok=True)
        (self.t / "respuesta").write_text("")
        self.bluez = BluezFalso(self.direccion)
        self.addCleanup(self.bluez.cerrar)
        self.agente = self.arrancar_agente()
        self.addCleanup(self.parar_agente)
        self.cliente = Gio.DBusConnection.new_for_address_sync(
            self.direccion, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        self.addCleanup(self.cliente.close_sync, None)
        self.esperar(lambda: len(self.bluez.llamadas) >= 2)

    def arrancar_agente(self, path=None):
        env = dict(os.environ, DBUS_SYSTEM_BUS_ADDRESS=self.direccion, PATH=path or f"{self.t}/bin:{os.environ['PATH']}")
        return subprocess.Popen([sys.executable, "-I", str(AGENTE)], env=env, stderr=subprocess.DEVNULL)

    def parar_agente(self):
        self.agente.terminate()
        self.agente.wait()

    def esperar(self, condicion, segundos=5):
        contexto = GLib.MainContext.default()
        fin = time.monotonic() + segundos
        while time.monotonic() < fin:
            while contexto.pending():
                contexto.iteration(False)
            if condicion():
                return True
            time.sleep(0.02)
        self.fail("no pasó a tiempo")

    def llamar(self, metodo, firma=None, args=(), espera=5000):
        """Llama al agente como BlueZ, sin atascar lo que hace BlueZ falso en este mismo proceso."""
        salida = {}

        def listo(bus, resultado):
            try:
                salida["ok"] = bus.call_finish(resultado).unpack()
            except GLib.Error as e:
                salida["error"] = Gio.DBusError.get_remote_error(e)
        self.cliente.call(self.nombre_agente(), RUTA, "org.bluez.Agent1", metodo,
                          GLib.Variant(firma, args) if firma else None, None, Gio.DBusCallFlags.NONE, espera, None, listo)
        self.esperar(lambda: salida, espera / 1000 + 1)
        return salida

    def nombre_agente(self):
        # dbus-daemon reparte nombres en orden: se pregunta quién tiene el objeto de entre los conectados.
        nombres = self.cliente.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames",
                                         None, None, Gio.DBusCallFlags.NONE, 3000, None).unpack()[0]
        for n in sorted((x for x in nombres if x.startswith(":")), key=lambda x: float(x[1:]), reverse=True):
            try:
                texto = self.cliente.call_sync(n, RUTA, "org.freedesktop.DBus.Introspectable", "Introspect", None, None,
                                               Gio.DBusCallFlags.NONE, 800, None).unpack()[0]
            except GLib.Error:
                continue
            if "org.bluez.Agent1" in texto:
                return n
        self.fail("el agente no publicó su objeto")

    def aviso(self):
        ruta = self.t / "aviso"
        return ruta.read_text() if ruta.exists() else ""

    # --- Las pruebas

    def test_se_registra_como_el_agente_de_siempre(self):
        self.assertEqual(self.bluez.llamadas[0], ("RegisterAgent", RUTA, "DisplayYesNo"))
        self.assertEqual(self.bluez.llamadas[1], ("RequestDefaultAgent", RUTA))

    def test_se_registra_otra_vez_si_bluez_reinicia(self):
        self.bluez.cerrar()
        time.sleep(0.2)
        otro = BluezFalso(self.direccion)
        self.addCleanup(otro.cerrar)
        self.esperar(lambda: len(otro.llamadas) >= 2)
        self.assertEqual(otro.llamadas[1], ("RequestDefaultAgent", RUTA))

    def test_aceptar_con_el_boton_empareja(self):
        (self.t / "respuesta").write_text("aceptar")
        self.assertEqual(self.llamar("RequestConfirmation", "(ou)", (DISPOSITIVO, 123456)), {"ok": ()})
        self.assertIn("123456", self.aviso())
        self.assertIn("SOUNDPEATS H3 (98:80:BB:41:01:D5)", self.aviso())
        self.assertIn("critical", self.aviso())

    def test_rechazar_no_empareja(self):
        (self.t / "respuesta").write_text("rechazar")
        self.assertEqual(self.llamar("RequestAuthorization", "(o)", (DISPOSITIVO,)), {"error": "org.bluez.Error.Rejected"})

    def test_tocar_la_tarjeta_no_acepta_nada(self):
        (self.t / "respuesta").write_text("default")
        self.assertEqual(self.llamar("RequestAuthorization", "(o)", (DISPOSITIVO,)), {"error": "org.bluez.Error.Rejected"})

    def test_cerrar_el_aviso_no_acepta_nada(self):
        self.assertEqual(self.llamar("AuthorizeService", "(os)", (DISPOSITIVO, "0000110b")), {"error": "org.bluez.Error.Rejected"})

    def test_si_bluez_cancela_se_quita_la_pregunta(self):
        (self.t / "respuesta").write_text("colgar")
        salida = {}

        def listo(bus, resultado):
            try:
                salida["ok"] = bus.call_finish(resultado).unpack()
            except GLib.Error as e:
                salida["error"] = Gio.DBusError.get_remote_error(e)
        nombre = self.nombre_agente()
        self.cliente.call(nombre, RUTA, "org.bluez.Agent1", "RequestAuthorization", GLib.Variant("(o)", (DISPOSITIVO,)),
                          None, Gio.DBusCallFlags.NONE, 8000, None, listo)
        self.esperar(self.aviso)
        self.cliente.call_sync(nombre, RUTA, "org.bluez.Agent1", "Cancel", None, None, Gio.DBusCallFlags.NONE, 3000, None)
        self.esperar(lambda: salida)
        self.assertEqual(salida, {"error": "org.bluez.Error.Canceled"})

    def test_un_nombre_con_formato_no_se_interpreta(self):
        self.bluez.alias = "<b>Teclado</b>"
        (self.t / "respuesta").write_text("rechazar")
        self.llamar("RequestAuthorization", "(o)", (DISPOSITIVO,))
        self.assertIn("&lt;b&gt;Teclado", self.aviso())
        self.assertNotIn("<b>", self.aviso())

    def test_el_pin_solo_con_permiso(self):
        (self.t / "respuesta").write_text("aceptar")
        self.assertEqual(self.llamar("RequestPinCode", "(o)", (DISPOSITIVO,)), {"ok": ("0000",)})
        (self.t / "respuesta").write_text("rechazar")
        self.assertIn("error", self.llamar("RequestPinCode", "(o)", (DISPOSITIVO,)))

    def test_no_inventa_un_numero(self):
        self.assertEqual(self.llamar("RequestPasskey", "(o)", (DISPOSITIVO,)), {"error": "org.bluez.Error.Rejected"})
        self.assertEqual(self.aviso(), "")

    def test_ensena_el_numero_que_hay_que_escribir(self):
        self.assertEqual(self.llamar("DisplayPasskey", "(ouq)", (DISPOSITIVO, 42, 0)), {"ok": ()})
        self.esperar(lambda: "000042" in self.aviso())

    def test_sigue_vivo_despues_de_contestar(self):
        (self.t / "respuesta").write_text("rechazar")
        self.llamar("RequestAuthorization", "(o)", (DISPOSITIVO,))
        (self.t / "respuesta").write_text("aceptar")
        self.assertEqual(self.llamar("RequestAuthorization", "(o)", (DISPOSITIVO,)), {"ok": ()})
        self.assertIsNone(self.agente.poll())


class SinAvisos(Prueba):
    """Sin notify-send no hay cómo preguntar: se dice que no."""

    def setUp(self):
        self._path_vacio = self.t / "vacio"
        self._path_vacio.mkdir(exist_ok=True)
        super().setUp()

    def arrancar_agente(self, path=None):
        return super().arrancar_agente(str(self._path_vacio))

    def test_aceptar_con_el_boton_empareja(self):
        self.assertEqual(self.llamar("RequestConfirmation", "(ou)", (DISPOSITIVO, 1)), {"error": "org.bluez.Error.Rejected"})

    test_rechazar_no_empareja = test_tocar_la_tarjeta_no_acepta_nada = test_cerrar_el_aviso_no_acepta_nada = None
    test_si_bluez_cancela_se_quita_la_pregunta = test_un_nombre_con_formato_no_se_interpreta = None
    test_el_pin_solo_con_permiso = test_ensena_el_numero_que_hay_que_escribir = test_sigue_vivo_despues_de_contestar = None


if __name__ == "__main__":
    unittest.main(verbosity=0)
