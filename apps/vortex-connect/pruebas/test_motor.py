"""El motor con redes y WARP falsos: no toca tu wifi, tu WARP ni tu internet.

    python3 -m unittest discover -s apps/vortex-connect/pruebas
"""
from __future__ import annotations

import http.server
import os
import stat
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from vortex_connect import motor, proteccion, redes  # noqa: E402
from vortex_connect.motor import Ajustes, Candado, Connect  # noqa: E402
from vortex_connect.redes import Actual, ErrorRed, Red, RedesLinux, RedesWindows  # noqa: E402


class RedesFalsas:
    def __init__(self, actual=Actual(), lista=(), fallan=()):
        self._actual, self._lista, self.fallan = actual, list(lista), set(fallan)
        self.entradas = []

    def actual(self):
        return self._actual

    def lista(self, buscar=True):
        return self._lista

    def hay_wifi(self):
        return True

    def conectar(self, ssid):
        self.entradas.append(ssid)
        if ssid in self.fallan:
            raise ErrorRed(f"No se pudo entrar a «{ssid}».")
        self._actual = Actual("wifi", ssid, True)


class WarpFalso:
    def __init__(self, cifra=True, dns=True, prendido="", blinda=True):
        self.cifra, self.dns, self.prendido, self.blinda = cifra, dns, prendido, blinda
        self.llamadas = []
        self.blindajes = 0

    def blindar_dns(self):
        self.blindajes += 1
        return self.blinda

    def instalado(self):
        return True

    def conectado(self):
        return bool(self.prendido)

    def modo(self):
        return self.prendido

    def preparar(self):
        self.llamadas.append("preparar")

    def activar_cifrado(self):
        self.llamadas.append("cifrado")
        if self.cifra:
            self.prendido = "cifrado"
        return self.cifra

    def activar_solo_dns(self):
        self.llamadas.append("dns")
        if self.dns:
            self.prendido = "dns"
        return self.dns

    def apagar(self):
        self.llamadas.append("apagar")
        self.prendido = ""


def respuestas(*lista):
    """Cada llamada da la siguiente respuesta; la última se repite."""
    pendientes = list(lista)

    def probar():
        return pendientes.pop(0) if len(pendientes) > 1 else pendientes[0]

    return probar


INTERNET = ("internet", "")
PORTAL = ("portal", "http://portal.test/")
NADA = ("nada", "")


class ConConfig(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self._env = mock.patch.dict(os.environ, {"XDG_CONFIG_HOME": self._tmp.name})
        self._env.start()
        self.addCleanup(self._tmp.cleanup)
        self.addCleanup(self._env.stop)

    def connect(self, redes_f=None, warp=None, probar=None, condiciones=True, protegio=False):
        self.dichos, self.portales = [], []
        return Connect(redes_f or RedesFalsas(), warp or WarpFalso(), probar or respuestas(INTERNET),
                       dormir=lambda _s: None, ajustes=Ajustes(condiciones=condiciones, protegio_connect=protegio),
                       avisar=self.dichos.append, abrir_portal=self.portales.append)


class TestFlujo(ConConfig):
    def test_entra_y_cifra(self):
        r, w = RedesFalsas(), WarpFalso()
        e = self.connect(r, w).conectar("Cafe")
        self.assertEqual(r.entradas, ["Cafe"])
        self.assertEqual(e.estado, motor.PROTEGIDO)
        self.assertEqual(self.portales, [])

    def test_con_portal_abre_espera_y_luego_cifra(self):
        w = WarpFalso()
        c = self.connect(RedesFalsas(), w, respuestas(PORTAL, PORTAL, INTERNET))
        e = c.conectar("Aeropuerto")
        self.assertEqual(self.portales, ["http://portal.test/"])
        self.assertTrue(any("no va cifrado" in d for d in self.dichos))
        self.assertEqual(e.estado, motor.PROTEGIDO)

    def test_cancelar_el_portal_no_cifra(self):
        w = WarpFalso()
        c = self.connect(RedesFalsas(), w, respuestas(PORTAL))
        cancelar = threading.Event()
        cancelar.set()
        e = c.conectar("Aeropuerto", cancelar)
        self.assertEqual(w.llamadas, [])
        self.assertEqual(e.estado, motor.PORTAL)

    def test_sin_internet_lo_dice_y_no_cifra(self):
        w = WarpFalso()
        e = self.connect(RedesFalsas(), w, respuestas(NADA)).conectar("Rota")
        self.assertEqual(w.llamadas, [])
        self.assertEqual(e.estado, motor.SIN_RED)
        self.assertTrue(any("no da internet" in d for d in self.dichos))

    def test_red_que_bloquea_cae_a_dns(self):
        w = WarpFalso(cifra=False)
        e = self.connect(RedesFalsas(), w).conectar("Hotel")
        self.assertEqual(w.llamadas, ["preparar", "cifrado", "dns"])
        self.assertEqual(e.estado, motor.SOLO_DNS)

    def test_al_cifrar_le_quita_al_wifi_su_dns(self):
        w = WarpFalso()
        self.connect(RedesFalsas(), w).conectar("Hotel")
        self.assertEqual(w.blindajes, 1)
        self.assertTrue(any(d.startswith("Protegido") for d in self.dichos))

    def test_si_el_dns_sigue_expuesto_lo_dice(self):
        w = WarpFalso(blinda=False)
        e = self.connect(RedesFalsas(), w).conectar("Hotel")
        self.assertEqual(e.estado, motor.PROTEGIDO)
        self.assertFalse(any(d.startswith("Protegido") for d in self.dichos))
        self.assertTrue(any("nombres de los sitios" in d for d in self.dichos))

    def test_solo_dns_sin_blindar_no_cuenta_como_protegido(self):
        # En este modo los nombres son lo único que se cifra: si siguen saliendo en claro, no hay protección.
        w = WarpFalso(cifra=False, blinda=False)
        e = self.connect(RedesFalsas(), w).conectar("Hotel")
        self.assertEqual(e.estado, motor.SIN_PROTECCION)
        self.assertIn("apagar", w.llamadas)

    def test_si_nada_funciona_lo_dice_y_no_finge(self):
        w = WarpFalso(cifra=False, dns=False)
        e = self.connect(RedesFalsas(), w).conectar("Hotel")
        self.assertEqual(e.estado, motor.SIN_PROTECCION)
        self.assertTrue(any("sin protección" in d for d in self.dichos))

    def test_sin_aceptar_condiciones_warp_ni_se_toca(self):
        w = WarpFalso()
        e = self.connect(RedesFalsas(), w, condiciones=False).conectar("Cafe")
        self.assertEqual(w.llamadas, [])
        self.assertNotEqual(e.estado, motor.PROTEGIDO)

    def test_sin_aceptar_ni_se_pregunta_si_esta_prendido(self):
        w = WarpFalso()
        w.conectado = mock.Mock(return_value=True)
        self.connect(RedesFalsas(), w, condiciones=False).conectar("Cafe")
        self.connect(RedesFalsas(lista=[]), w, condiciones=False).revisar()
        w.conectado.assert_not_called()

    def test_pausa_el_tunel_antes_de_entrar(self):
        w = WarpFalso(prendido="cifrado")
        self.connect(RedesFalsas(), w).conectar("Cafe")
        self.assertEqual(w.llamadas[0], "apagar")

    def test_no_poder_entrar_sube_como_error(self):
        with self.assertRaises(ErrorRed):
            self.connect(RedesFalsas(fallan=["Lejos"])).conectar("Lejos")


class TestEstado(ConConfig):
    def test_estados(self):
        abierta = Actual("wifi", "Cafe", True)
        cerrada = Actual("wifi", "Casa", False)
        casos = [
            (abierta, "", INTERNET, motor.SIN_PROTECCION),
            (abierta, "cifrado", INTERNET, motor.PROTEGIDO),
            (abierta, "dns", INTERNET, motor.SOLO_DNS),
            (abierta, "", PORTAL, motor.PORTAL),
            (abierta, "", NADA, motor.SIN_RED),
            (cerrada, "", INTERNET, motor.RED_DE_CONFIANZA),
            (Actual("cable"), "", INTERNET, motor.RED_DE_CONFIANZA),
            (Actual(), "", NADA, motor.SIN_RED),
        ]
        for red, modo, internet, esperado in casos:
            with self.subTest(red=red, modo=modo):
                c = self.connect(RedesFalsas(red), WarpFalso(prendido=modo), respuestas(internet))
                self.assertEqual(c.estado().estado, esperado)


class TestModoViaje(ConConfig):
    def test_sin_red_entra_a_la_mas_fuerte(self):
        r = RedesFalsas(lista=[Red("Fuerte", 90), Red("Debil", 20)])
        e = self.connect(r).revisar()
        self.assertEqual(r.entradas, ["Fuerte"])
        self.assertEqual(e.estado, motor.PROTEGIDO)

    def test_si_falla_prueba_la_siguiente_y_no_insiste(self):
        r = RedesFalsas(lista=[Red("Fuerte", 90), Red("Debil", 20)], fallan=["Fuerte"])
        c = self.connect(r)
        c.revisar()
        self.assertEqual(r.entradas, ["Fuerte", "Debil"])
        r._actual = Actual()
        r.entradas.clear()
        c.revisar()
        self.assertEqual(r.entradas, ["Debil"])

    def test_sin_redes_no_hace_nada(self):
        self.assertIsNone(self.connect(RedesFalsas()).revisar())

    def test_red_abierta_sin_cifrar_se_cifra(self):
        w = WarpFalso()
        e = self.connect(RedesFalsas(Actual("wifi", "Cafe", True)), w).revisar()
        self.assertEqual(e.estado, motor.PROTEGIDO)

    def test_ya_cifrada_no_hace_nada(self):
        w = WarpFalso(prendido="cifrado")
        self.assertIsNone(self.connect(RedesFalsas(Actual("wifi", "Cafe", True)), w).revisar())
        self.assertEqual(w.llamadas, [])

    def test_tu_red_con_contrasena_no_se_toca(self):
        r, w = RedesFalsas(Actual("wifi", "Casa", False), [Red("Cafe", 99)]), WarpFalso()
        self.assertIsNone(self.connect(r, w).revisar())
        self.assertEqual(r.entradas, [])
        self.assertEqual(w.llamadas, [])

    def test_con_cable_no_busca_wifi(self):
        r = RedesFalsas(Actual("cable"), [Red("Cafe", 99)])
        self.assertIsNone(self.connect(r).revisar())
        self.assertEqual(r.entradas, [])

    def test_de_vuelta_en_casa_apaga_lo_que_prendio(self):
        w = WarpFalso(prendido="cifrado")
        self.connect(RedesFalsas(Actual("wifi", "Casa", False)), w, protegio=True).revisar()
        self.assertIn("apagar", w.llamadas)

    def test_en_casa_no_apaga_lo_que_prendiste_tu(self):
        w = WarpFalso(prendido="cifrado")
        self.assertIsNone(self.connect(RedesFalsas(Actual("wifi", "Casa", False)), w, protegio=False).revisar())
        self.assertEqual(w.llamadas, [])

    def test_si_el_wifi_se_reconecto_vuelve_a_quitarle_su_dns(self):
        w = WarpFalso(prendido="cifrado")
        self.assertIsNone(self.connect(RedesFalsas(Actual("wifi", "Cafe", True)), w).revisar())
        self.assertEqual(w.blindajes, 1)


class TestAjustes(ConConfig):
    def test_ida_y_vuelta(self):
        Ajustes(condiciones=True, modo_viaje=True).guardar()
        a = Ajustes.leer()
        self.assertTrue(a.condiciones and a.modo_viaje)
        self.assertFalse(a.protegio_connect)

    def test_archivo_roto_o_con_tipos_raros_no_acepta_nada(self):
        ruta = Ajustes.ruta()
        ruta.parent.mkdir(parents=True, exist_ok=True)
        for texto in ("{roto", "[]", '{"condiciones": "si", "modo_viaje": 1}'):
            ruta.write_text(texto)
            self.assertFalse(Ajustes.leer().condiciones, texto)
            self.assertFalse(Ajustes.leer().modo_viaje, texto)

    def test_candado_es_de_uno_en_uno(self):
        a, b = Candado(), Candado()
        self.assertTrue(a.tomar())
        self.assertFalse(b.tomar())
        a.soltar()
        self.assertTrue(b.tomar())
        b.soltar()


class _Pagina(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/204":
            self.send_response(204)
        elif self.path == "/portal":
            self.send_response(302)
            self.send_header("Location", "http://portal.test/entra")
        elif self.path == "/raro":
            self.send_response(302)
            self.send_header("Location", "javascript:alert(1)")
        else:
            self.send_response(200)
        self.end_headers()

    def log_message(self, *_a):
        pass


class TestProbarInternet(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.servidor = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Pagina)
        threading.Thread(target=cls.servidor.serve_forever, daemon=True).start()
        cls.base = f"http://127.0.0.1:{cls.servidor.server_address[1]}"

    @classmethod
    def tearDownClass(cls):
        cls.servidor.shutdown()
        cls.servidor.server_close()

    def probar(self, ruta):
        with mock.patch.object(motor, "PRUEBA", self.base + ruta):
            return motor.probar_internet(2)

    def test_204_es_internet(self):
        self.assertEqual(self.probar("/204"), ("internet", ""))

    def test_redireccion_es_portal_y_da_su_pagina(self):
        self.assertEqual(self.probar("/portal"), ("portal", "http://portal.test/entra"))

    def test_una_pagina_en_vez_de_204_es_portal(self):
        self.assertEqual(self.probar("/otra")[0], "portal")

    def test_un_portal_no_te_manda_a_cualquier_esquema(self):
        self.assertEqual(self.probar("/raro"), ("portal", self.base + "/raro"))

    def test_sin_red_es_nada(self):
        with mock.patch.object(motor, "PRUEBA", "http://127.0.0.1:9/"):
            self.assertEqual(motor.probar_internet(1), ("nada", ""))


LISTA_NMCLI = "\n".join([
    "*:Cafe\\: Centro:70:",
    " :Cafe\\: Centro:90:",
    " :Aeropuerto:40:--",
    " :Casa:99:WPA2",
    " :Vieja:80:WEP",
    " ::50:",
])


def correr_falso(respuestas_por_orden: dict[str, tuple], registro: list):
    def correr(orden, _timeout):
        registro.append(orden)
        texto = " ".join(orden)
        for clave, resp in respuestas_por_orden.items():
            if clave in texto:
                return resp
        return 0, "", ""
    return correr


class TestLinux(unittest.TestCase):
    def test_partir(self):
        self.assertEqual(redes.partir_nmcli("a\\:b:c\\\\d::"), ["a:b", "c\\d", "", ""])

    def test_lista_solo_abiertas_sin_repetir_y_las_fuertes_primero(self):
        r = RedesLinux(correr_falso({"wifi list": (0, LISTA_NMCLI, "")}, []))
        self.assertEqual(r.lista(), [Red("Cafe: Centro", 90, True), Red("Aeropuerto", 40)])

    def test_actual(self):
        lista = (0, LISTA_NMCLI, "")
        casos = [
            ("ethernet:connected\nwifi:connected\n", lista, Actual("cable")),
            ("wifi:connected\n", lista, Actual("wifi", "Cafe: Centro", True)),
            ("wifi:connected\n", (0, " :Cafe:70:\n*:Casa:99:WPA2", ""), Actual("wifi", "Casa", False)),
            ("wifi:disconnected\n", lista, Actual()),
        ]
        for dispositivos, filas, esperado in casos:
            with self.subTest(dispositivos):
                r = RedesLinux(correr_falso({"TYPE,STATE": (0, dispositivos, ""), "wifi list": filas}, []))
                self.assertEqual(r.actual(), esperado)

    def test_conectar_con_mac_al_azar_y_sin_volver_solo(self):
        registro = []
        RedesLinux(correr_falso({}, registro)).conectar("Cafe; rm -rf")
        alta = next(o for o in registro if "add" in o)
        self.assertIn("Cafe; rm -rf", alta)     # un solo argumento: el nombre nunca pasa por un shell
        self.assertEqual(alta[alta.index("connection.autoconnect") + 1], "no")
        self.assertEqual(alta[alta.index("wifi.cloned-mac-address") + 1], "random")
        self.assertNotIn("wifi-sec.key-mgmt", alta)
        self.assertEqual(registro[-1][-1], "Vortexine Connect: Cafe; rm -rf")

    def test_reutiliza_el_perfil_que_ya_existe(self):
        registro = []
        RedesLinux(correr_falso({"NAME": (0, "Vortexine Connect\\: Cafe\n", "")}, registro)).conectar("Cafe")
        self.assertFalse(any("add" in o for o in registro))

    def test_si_no_entra_borra_el_perfil_y_lo_dice(self):
        registro = []
        with self.assertRaises(ErrorRed):
            RedesLinux(correr_falso({"connection up": (4, "", "no")}, registro)).conectar("Lejos")
        self.assertEqual(registro[-1][1:3], ["connection", "delete"])

    def test_nmcli_que_no_esta_no_rompe(self):
        r = RedesLinux(lambda *_: (127, "", ""))
        self.assertEqual(r.lista(), [])
        self.assertEqual(r.actual(), Actual())
        self.assertFalse(r.hay_wifi())


NETSH_ES = """
Nombre de interfaz : Wi-Fi
Hay 3 redes visibles actualmente.

SSID 1 : Cafe Centro
    Tipo de red             : Infraestructura
    Autenticación           : Abierta
    Cifrado                 : Ninguno
    BSSID 1                 : aa:bb:cc:dd:ee:01
         Señal              : 80%
    BSSID 2                 : aa:bb:cc:dd:ee:02
         Señal              : 35%

SSID 2 : Casa
    Tipo de red             : Infraestructura
    Autenticación           : WPA2-Personal
    Cifrado                 : CCMP
    BSSID 1                 : aa:bb:cc:dd:ee:03
         Señal              : 99%

SSID 3 : Con portal
    Tipo de red             : Infraestructura
    Autenticación           : Abierta
    Cifrado                 : Ninguno
    BSSID 1                 : aa:bb:cc:dd:ee:04
         Señal              : 60%
"""

NETSH_EN = """
SSID 1 : Airport
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : None
    BSSID 1                 : aa:bb:cc:dd:ee:01
         Signal             : 50%

SSID 2 : Enhanced
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : WEP
    BSSID 1                 : aa:bb:cc:dd:ee:02
         Signal             : 90%
"""


class TestWindows(unittest.TestCase):
    def test_lista_en_espanol(self):
        r = RedesWindows(correr_falso({"show networks": (0, NETSH_ES, "")}, []))
        self.assertEqual(r.lista(), [Red("Cafe Centro", 80), Red("Con portal", 60)])

    def test_lista_en_ingles_sin_las_que_cifran(self):
        r = RedesWindows(correr_falso({"show networks": (0, NETSH_EN, "")}, []))
        self.assertEqual(r.lista(), [Red("Airport", 50)])

    def test_pide_ubicacion(self):
        r = RedesWindows(correr_falso({"show networks": (1, "La ubicación está desactivada", "")}, []))
        with self.assertRaisesRegex(ErrorRed, "Ubicación"):
            r.lista()

    def test_actual(self):
        interfaz = "Nombre : Wi-Fi\n    SSID                   : Cafe\n    BSSID                  : aa\n    Autenticación          : Abierta\n"
        r = RedesWindows(correr_falso({"show interfaces": (0, interfaz, "")}, []))
        self.assertEqual(r.actual(), Actual("wifi", "Cafe", True))
        self.assertTrue(r.hay_wifi())

    def test_cable_solo_con_adaptador_fisico(self):
        r = RedesWindows(correr_falso({"Get-NetAdapter": (0, "1\r\n", "")}, []))
        self.assertEqual(r.actual(), Actual("cable"))
        r = RedesWindows(correr_falso({"Get-NetAdapter": (0, "0\r\n", "")}, []))
        self.assertEqual(r.actual(), Actual())

    def test_perfil_escapa_el_nombre_y_no_entra_solo(self):
        perfil = redes.perfil_windows("A&B <c>")
        self.assertIn("A&amp;B &lt;c&gt;", perfil)
        self.assertIn("<connectionMode>manual</connectionMode>", perfil)
        self.assertIn("<authentication>open</authentication>", perfil)

    def test_conectar_borra_el_perfil_temporal(self):
        registro = []
        RedesWindows(correr_falso({}, registro)).conectar("Cafe")
        archivo = registro[0][registro[0].index("add") + 2].split("=", 1)[1]
        self.assertFalse(Path(archivo).exists())
        self.assertIn("name=Cafe", registro[1])


CLI_FALSO = """#!/bin/sh
echo "$*" >> "$REGISTRO"
case "$*" in
  *status*) cat "$ESTADO" ;;
  *"registration show"*) if [ -f "$REGISTRADO" ]; then echo "Account"; else echo "Missing registration"; exit 1; fi ;;
  *"registration new"*) : > "$REGISTRADO"; echo Success ;;
  *" connect"*) echo "Status update: Connected" > "$ESTADO"; echo Success ;;
  *disconnect*) echo "Status update: Disconnected" > "$ESTADO"; echo Success ;;
  *"settings list"*) cat "$AJUSTES" 2>/dev/null ;;
  *) echo Success ;;
esac
"""
# Lo que imprime de verdad warp-cli 2026.8 en «settings list» (recortado).
AJUSTES_REALES = """Merged configuration:
(not set)\tCompliance Environment: Normal
(user set)\tMode: {modo}
(network policy)\tWARP tunnel protocol: MASQUE
(default)\tDoH MSS clamp enabled: true
"""
IP_FALSO = """#!/bin/sh
case "$1" in
  -4) echo "default via 10.77.0.1 dev wlan0 proto dhcp src 10.77.0.34 metric 600" ;;
  -6) echo "default via fe80::1 dev wlan0 proto ra metric 600 pref medium" ;;
esac
"""
RESOLVECTL_FALSO = """#!/bin/sh
echo "$*" >> "$REGISTRO_DNS"
[ ! -f "$DNS_FALLA" ]
"""
SYSTEMCTL_FALSO = """#!/bin/sh
echo "$*" >> "$REGISTRO_SVC"
case "$*" in
  *is-active*) [ -f "$SVC" ] ;;
  *start*) : > "$SVC" ;;
  *stop*) rm -f "$SVC" ;;
esac
"""


@unittest.skipIf(os.name == "nt", "usa programas falsos de shell")
class TestWarp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        d = Path(self.tmp.name)
        for nombre, texto in (("warp-cli", CLI_FALSO), ("systemctl", SYSTEMCTL_FALSO), ("ip", IP_FALSO),
                              ("resolvectl", RESOLVECTL_FALSO)):
            (d / nombre).write_text(texto)
            (d / nombre).chmod((d / nombre).stat().st_mode | stat.S_IXUSR)
        (d / "estado").write_text("Status update: Disconnected\n")
        self.registro, self.registro_svc = d / "reg", d / "reg-svc"
        env = mock.patch.dict(os.environ, {
            "PATH": f"{d}:{os.environ['PATH']}", "REGISTRO": str(self.registro), "REGISTRO_SVC": str(self.registro_svc),
            "ESTADO": str(d / "estado"), "REGISTRADO": str(d / "registrado"), "SVC": str(d / "svc"),
            "AJUSTES": str(d / "ajustes"), "REGISTRO_DNS": str(d / "reg-dns"), "DNS_FALLA": str(d / "dns-falla"),
        })
        env.start()
        self.addCleanup(env.stop)
        self.d = d

    def ordenes(self):
        return self.registro.read_text().splitlines() if self.registro.exists() else []

    def test_preguntar_en_reposo_no_prende_el_servicio(self):
        w = proteccion.Warp(lambda: False)
        self.assertFalse(w.conectado())
        self.assertEqual(w.modo(), "")
        self.assertEqual(self.ordenes(), [])
        self.assertNotIn("start", self.registro_svc.read_text())

    def test_preparar_prende_y_registra_una_vez(self):
        w = proteccion.Warp(lambda: True)
        w.preparar()
        w.preparar()
        self.assertEqual(sum("registration new" in o for o in self.ordenes()), 1)
        self.assertTrue(all(o.startswith("--accept-tos") for o in self.ordenes()))
        self.assertIn("--no-ask-password start warp-svc.service", self.registro_svc.read_text())

    def test_cifrado_pide_ver_warp_on(self):
        w = proteccion.Warp(lambda: True)
        w.preparar()
        self.assertTrue(w.activar_cifrado())
        self.assertIn("--accept-tos mode warp", self.ordenes())
        self.assertEqual(w.modo(), "cifrado")

    def test_el_protocolo_lo_elige_warp(self):
        # Forzar MASQUE y luego WireGuard dejó al cliente real sin poder conectar en los intentos siguientes.
        w = proteccion.Warp(lambda: True)
        w.preparar()
        w.activar_cifrado()
        self.assertFalse([o for o in self.ordenes() if "protocol" in o or "rotate-keys" in o])

    @mock.patch.object(proteccion.time, "sleep")
    def test_si_cloudflare_no_ve_warp_no_cuenta(self, _dormir):
        w = proteccion.Warp(lambda: False)
        w.preparar()
        self.assertFalse(w.activar_cifrado())

    @mock.patch.object(proteccion.time, "sleep")
    def test_conectado_antes_de_dejar_pasar_trafico_se_le_da_tiempo(self, _dormir):
        respuestas = iter([False, False, True])
        w = proteccion.Warp(lambda: next(respuestas))
        w.preparar()
        self.assertTrue(w.activar_cifrado())

    @mock.patch.object(proteccion.time, "sleep")
    def test_una_respuesta_perdida_no_quita_el_cifrado(self, _dormir):
        w = proteccion.Warp(lambda: True)
        w.preparar()
        w.activar_cifrado()
        respuestas = iter([False, True])
        w._traza = lambda: next(respuestas)
        self.assertEqual(w.modo(), "cifrado")

    def test_solo_dns(self):
        w = proteccion.Warp(lambda: False)
        w.preparar()
        self.assertTrue(w.activar_solo_dns())
        self.assertIn("--accept-tos mode doh", self.ordenes())
        (self.d / "ajustes").write_text(AJUSTES_REALES.format(modo="DnsOverHttps"))
        self.assertEqual(w.modo(), "dns")

    def test_conectado_sin_warp_ni_doh_no_dice_que_protege(self):
        # «DoH MSS clamp» sale en todos los modos: no debe confundirse con el modo DNS.
        w = proteccion.Warp(lambda: False)
        w.preparar()
        w.activar_solo_dns()
        (self.d / "ajustes").write_text(AJUSTES_REALES.format(modo="Warp"))
        self.assertEqual(w.modo(), "")

    def test_avisa_de_tailscale_solo_si_esta_en_este_equipo(self):
        with mock.patch.object(proteccion.os.path, "exists", return_value=True) as existe, \
                mock.patch.object(proteccion, "ES_WINDOWS", False):
            self.assertTrue(proteccion.hay_tailscale())
            existe.assert_called_with("/sys/class/net/tailscale0")
            existe.return_value = False
            self.assertFalse(proteccion.hay_tailscale())

    def test_blindar_le_quita_el_dns_al_enlace_una_sola_vez(self):
        self.assertTrue(proteccion.Warp(lambda: True).blindar_dns())
        self.assertEqual((self.d / "reg-dns").read_text().splitlines(), ["--no-ask-password default-route wlan0 false"])

    def test_blindar_avisa_si_no_pudo(self):
        (self.d / "dns-falla").touch()
        self.assertFalse(proteccion.Warp(lambda: True).blindar_dns())

    def test_apagar_le_devuelve_el_dns_al_enlace(self):
        w = proteccion.Warp(lambda: True)
        w.preparar()
        w.apagar()
        self.assertEqual((self.d / "reg-dns").read_text().splitlines()[-1], "--no-ask-password default-route wlan0 true")

    def test_apagar_corta_y_apaga_el_servicio(self):
        w = proteccion.Warp(lambda: True)
        w.preparar()
        w.activar_cifrado()
        w.apagar()
        self.assertEqual(self.ordenes()[-1], "--accept-tos disconnect")
        self.assertFalse((self.d / "svc").exists())


if __name__ == "__main__":
    unittest.main()
