#!/usr/bin/env python3
"""Pruebas del motor de la tienda (catalogo.py, texto.py, ids.py y flatpak.py), sin internet y sin
tocar el sistema. Las corre pruebas/tienda.sh; también se pueden correr solas:

    python3 -I -m unittest -v apps/shop/pruebas/test_motor.py     (desde la raíz del repo)

  - El catálogo habla con un Flathub FALSO en 127.0.0.1 (servidor_falso.py) que sirve respuestas
    reales guardadas en datos/. La caché va a una carpeta temporal.
  - flatpak.py habla con un «flatpak» FALSO (flatpak_falso) puesto al principio del PATH; la ruta de
    libflatpak, con un módulo Flatpak falso. Nunca se llama al flatpak de verdad.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest

AQUI = os.path.dirname(os.path.abspath(__file__))


def leer_archivo(ruta):
    with open(ruta, encoding="utf-8") as f:
        return f.read()


sys.path.insert(0, os.path.dirname(AQUI))

from vortex_shop import catalogo as catalogo_mod                     # noqa: E402
from vortex_shop import flatpak                                       # noqa: E402
from vortex_shop.catalogo import App, AppDetalle, Catalogo, Categoria, RECOMENDADAS  # noqa: E402
from vortex_shop.errores import AppNoEncontrada, ErrorFlatpak, FlatpakSinRed, SinRed  # noqa: E402
from vortex_shop.ids import es_id_valido, validar_id                  # noqa: E402
from vortex_shop.texto import html_a_texto                            # noqa: E402
from vortex_shop import propias                                       # noqa: E402

PRISM = "org.prismlauncher.PrismLauncher"


# ====================================================================== el Flathub falso

class Servidor:
    """El servidor_falso.py corriendo en un puerto libre, con su carpeta de datos y de control."""

    def __init__(self, raiz):
        self.raiz = raiz
        self.datos = os.path.join(raiz, "datos")
        self.control = os.path.join(raiz, "control")
        shutil.copytree(os.path.join(AQUI, "datos"), self.datos)
        os.makedirs(self.control)
        archivo_puerto = os.path.join(self.control, "puerto")
        self.proceso = subprocess.Popen(
            [sys.executable, "-I", os.path.join(AQUI, "servidor_falso.py"), self.datos, archivo_puerto, self.control])
        for _ in range(200):
            if os.path.exists(archivo_puerto) and leer_archivo(archivo_puerto).strip():
                break
            time.sleep(0.025)
        self.puerto = int(leer_archivo(archivo_puerto))
        self.url = "http://127.0.0.1:%d" % self.puerto

    def pedidos(self):
        try:
            return leer_archivo(os.path.join(self.control, "pedidos.log")).splitlines()
        except OSError:
            return []

    def modo(self, modo):
        with open(os.path.join(self.control, "modo"), "w") as f:
            f.write(modo)

    def datos_escribir(self, nombre, contenido):
        with open(os.path.join(self.datos, nombre), "w") as f:
            json.dump(contenido, f)

    def parar(self):
        if self.proceso.poll() is None:
            self.proceso.terminate()
            self.proceso.wait(5)


def puerto_cerrado():
    """Un puerto donde no escucha nadie (se abre y se cierra)."""
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    puerto = s.getsockname()[1]
    s.close()
    return puerto


class BaseCatalogo(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="vortex-tienda-prueba.")
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.srv = Servidor(os.path.join(self.tmp, "srv"))
        self.addCleanup(self.srv.parar)
        self.cache = os.path.join(self.tmp, "cache")
        self.cat = Catalogo(cache_dir=self.cache, api=self.srv.url)


# ====================================================================== catálogo

class TestCatalogo(BaseCatalogo):
    def test_categorias(self):
        cats = self.cat.categorias()
        self.assertTrue(all(isinstance(c, Categoria) for c in cats))
        self.assertEqual(len(cats), 11)
        self.assertEqual((cats[0].id, cats[0].nombre), ("game", "Juegos"))
        self.assertTrue(all(c.icono.endswith("-symbolic") and c.nombre for c in cats))
        self.assertIn("audiovideo", [c.id for c in cats])

    def test_categoria_nueva_de_flathub_sale_al_final(self):
        self.srv.datos_escribir("categorias.json", ["game", "robotica", "../x"])
        cats = self.cat.categorias()
        self.assertEqual([c.id for c in cats], ["game", "robotica"])
        self.assertEqual(cats[1].nombre, "Robotica")

    def test_destacados_completa_con_tendencias(self):
        apps = self.cat.destacados()
        self.assertEqual(len(apps), 6)
        self.assertTrue(all(isinstance(a, App) and es_id_valido(a.id) and a.nombre for a in apps))
        self.assertEqual(len({a.id for a in apps}), 6)
        pedidos = "\n".join(self.srv.pedidos())
        self.assertIn("GET /app-picks/app-of-the-day/", pedidos)
        self.assertIn("GET /collection/trending?page=1&per_page=12&locale=es", pedidos)

    def test_destacados_primero_las_selecciones_del_dia(self):
        self.srv.datos_escribir("app_del_dia.json", {"app_id": "com.valvesoftware.Steam"})
        self.srv.datos_escribir("apps_de_la_semana.json", {"apps": [
            {"app_id": "org.mozilla.firefox", "position": 2}, {"app_id": PRISM, "position": 1},
            {"app_id": "com.valvesoftware.Steam", "position": 3}, {"app_id": "no.existe.Nada", "position": 4}]})
        apps = self.cat.destacados()
        self.assertEqual([a.id for a in apps[:3]], ["com.valvesoftware.Steam", PRISM, "org.mozilla.firefox"])
        self.assertEqual(len(apps), 6)
        self.assertEqual(len({a.id for a in apps}), 6)

    def test_recomendadas_descarta_las_que_no_existen(self):
        apps = self.cat.recomendadas()
        ids = [a.id for a in apps]
        existen = {"com.valvesoftware.Steam", PRISM, "org.vinegarhq.Sober", "org.mozilla.firefox", "org.videolan.VLC",
                   "com.spotify.Client"}
        self.assertEqual(set(ids), existen)
        self.assertEqual(ids, [i for i in RECOMENDADAS if i in existen])      # en el orden de la lista
        steam = apps[0]
        self.assertEqual((steam.nombre, steam.desarrollador), ("Steam", "Valve Corporation"))
        self.assertTrue(steam.icono_url.startswith("https://"))
        self.assertTrue(next(a for a in apps if a.id == PRISM).verificado)

    def test_recomendadas_ids_validos_y_sin_repetir(self):
        self.assertEqual(len(RECOMENDADAS), len(set(RECOMENDADAS)))
        self.assertTrue(all(es_id_valido(i) for i in RECOMENDADAS))
        for nombre in ("Steam", "PrismLauncher", "Sober", "hgl", "bottles", "Lutris", "pupgui2", "Discord", "Vesktop",
                       "Browser", "firefox", "Client", "Studio", "VLC", "LibreOffice", "siyuan", "easyeffects",
                       "Pegasus", "osu", "Zoom"):
            self.assertTrue(any(i.endswith(nombre) for i in RECOMENDADAS), nombre)

    def test_por_categoria(self):
        apps = self.cat.por_categoria("game")
        self.assertEqual(len(apps), 8)
        self.assertIsInstance(apps[0].descargas_mes, int)
        self.assertIn("GET /collection/category/game?page=1&per_page=30&sort_by=installs_last_month&locale=es",
                      self.srv.pedidos())
        self.assertEqual(self.cat.por_categoria("game", 2), [])           # página sin nada
        self.assertEqual(self.cat.por_categoria("GAME", 0)[0].id, apps[0].id)   # mayúsculas y página < 1
        self.assertEqual(self.cat.por_categoria("noexiste"), [])          # la API responde 422

    def test_por_categoria_no_deja_pasar_rutas_raras(self):
        antes = len(self.srv.pedidos())
        for malo in ("../etc/passwd", "game/../x", "a b", "game?x=1", "", None, "x" * 80):
            self.assertEqual(self.cat.por_categoria(malo), [])
        self.assertEqual(len(self.srv.pedidos()), antes)

    def test_buscar(self):
        apps = self.cat.buscar("minecraft")
        self.assertEqual(len(apps), 8)
        self.assertTrue(all(isinstance(a, App) for a in apps))
        self.assertIn('POST /search?locale=es {"hits_per_page": 30, "page": 1, "query": "minecraft"}',
                      self.srv.pedidos())
        self.assertEqual(self.cat.buscar("zzzzqq"), [])

    def test_buscar_vacio_no_pide_nada(self):
        for vacio in ("", "   ", None):
            self.assertEqual(self.cat.buscar(vacio), [])
        self.assertEqual(self.srv.pedidos(), [])

    def test_buscar_texto_raro_viaja_como_json(self):
        self.cat.buscar('"; DROP TABLE apps; -- ñ <script>')
        self.assertTrue(any(p.startswith("POST /search") for p in self.srv.pedidos()))

    def test_detalle(self):
        d = self.cat.detalle(PRISM)
        self.assertIsInstance(d, AppDetalle)
        self.assertIsInstance(d, App)
        self.assertEqual((d.id, d.nombre), (PRISM, "Prism Launcher"))
        self.assertTrue(d.verificado)
        self.assertEqual(d.licencia, "GPL-3.0-only")
        self.assertEqual(d.web, "https://prismlauncher.org/")
        self.assertRegex(d.version, r"^\d+\.\d+")
        resumen = json.loads(leer_archivo(os.path.join(AQUI, "datos", "resumen_%s.json" % PRISM)))
        self.assertEqual(d.tamano_descarga, resumen["download_size"])
        self.assertIn("Juegos", d.categorias)
        self.assertGreaterEqual(len(d.capturas), 1)
        self.assertTrue(all(c.startswith("https://") for c in d.capturas))
        self.assertEqual(len(d.capturas), len(set(d.capturas)))
        self.assertNotIn("<", d.descripcion)
        self.assertIn("• ", d.descripcion)
        self.assertIn("Features:\n• Easily install game modifications", d.descripcion)
        self.assertEqual(self.cat.detalle("com.valvesoftware.Steam").tamano_descarga, None)   # sin /summary: sigue la ficha

    def test_detalle_de_lo_que_no_existe(self):
        with self.assertRaises(AppNoEncontrada):
            self.cat.detalle("org.no.Existe")
        antes = len(self.srv.pedidos())
        for malo in ("; rm -rf ~", "a b.c.d", "../x", "", None, "org.x.Y\n", "http://evil/x.y"):
            with self.assertRaises(AppNoEncontrada):
                self.cat.detalle(malo)
        self.assertEqual(len(self.srv.pedidos()), antes)
        self.assertTrue(issubclass(AppNoEncontrada, ErrorFlatpak))

    def test_el_idioma_viaja_como_locale(self):
        Catalogo(cache_dir=self.cache + "2", idioma="en", api=self.srv.url).categorias()
        self.assertIsInstance(Catalogo(idioma="de; rm -rf").idioma, str)
        self.assertEqual(Catalogo(idioma="de; rm -rf").idioma, "es")
        self.assertEqual(Catalogo(idioma="es_MX").idioma, "es_MX")

    def test_api_por_variable_de_entorno(self):
        os.environ["VORTEX_SHOP_API"] = self.srv.url + "/"
        try:
            c = Catalogo(cache_dir=self.cache + "3")
            self.assertEqual(c.api, self.srv.url)
            self.assertEqual(len(c.categorias()), 11)
        finally:
            del os.environ["VORTEX_SHOP_API"]
        self.assertEqual(Catalogo(api="file:///etc/passwd").api, catalogo_mod.API_POR_DEFECTO)


class TestCache(BaseCatalogo):
    def test_segunda_vez_sale_de_la_cache(self):
        self.cat.categorias()
        self.cat.buscar("minecraft")
        n = len(self.srv.pedidos())
        otra = Catalogo(cache_dir=self.cache, api=self.srv.url)           # otro objeto, misma carpeta
        self.assertEqual(len(otra.categorias()), 11)
        self.assertEqual(len(otra.buscar("minecraft")), 8)
        self.assertEqual(len(self.srv.pedidos()), n)
        self.assertTrue(os.listdir(self.cache))
        self.assertTrue(all(f.endswith(".json") for f in os.listdir(self.cache)))

    def test_las_listas_vencen_a_la_hora_y_las_fichas_a_las_24(self):
        reloj = [1_800_000_000.0]
        self.cat._ahora = lambda: reloj[0]
        self.cat.por_categoria("game")
        self.cat.detalle(PRISM)
        n = len(self.srv.pedidos())
        reloj[0] += 3500                                                  # < 1 h: todo de la caché
        self.cat.por_categoria("game")
        self.cat.detalle(PRISM)
        self.assertEqual(len(self.srv.pedidos()), n)
        reloj[0] += 200                                                   # > 1 h: la lista se vuelve a pedir
        self.cat.por_categoria("game")
        self.assertEqual(len(self.srv.pedidos()), n + 1)
        self.cat.detalle(PRISM)                                           # la ficha aún no vence
        self.assertEqual(len(self.srv.pedidos()), n + 1)
        reloj[0] += 24 * 3600                                             # > 24 h: la ficha también
        self.cat.detalle(PRISM)
        self.assertEqual(len(self.srv.pedidos()), n + 3)                  # ficha + tamaño

    def test_con_el_servidor_caido_usa_la_cache_vieja(self):
        reloj = [1_800_000_000.0]
        self.cat._ahora = lambda: reloj[0]
        antes = self.cat.buscar("minecraft")
        self.cat.detalle(PRISM)
        reloj[0] += 10 * 24 * 3600                                        # todo vencido
        self.srv.modo("500")
        self.assertEqual([a.id for a in self.cat.buscar("minecraft")], [a.id for a in antes])
        self.assertEqual(self.cat.detalle(PRISM).nombre, "Prism Launcher")

    def test_con_la_red_caida_usa_la_cache_vieja(self):
        reloj = [1_800_000_000.0]
        self.cat._ahora = lambda: reloj[0]
        self.cat.categorias()
        reloj[0] += 5 * 24 * 3600
        self.srv.parar()                                                  # conexión rechazada
        self.assertEqual(len(self.cat.categorias()), 11)

    def test_sin_red_y_sin_cache_es_sin_red(self):
        c = Catalogo(cache_dir=self.cache + "-vacia", api="http://127.0.0.1:%d" % puerto_cerrado())
        for llamada in (c.destacados, c.recomendadas, c.categorias, lambda: c.por_categoria("game"),
                        lambda: c.buscar("minecraft"), lambda: c.detalle(PRISM)):
            with self.assertRaises(SinRed) as cm:
                llamada()
            self.assertIn("conexión", str(cm.exception))
        self.assertFalse(os.path.exists(self.cache + "-vacia") and os.listdir(self.cache + "-vacia"))

    def test_error_del_servidor_sin_cache_es_sin_red(self):
        self.srv.modo("500")
        with self.assertRaises(SinRed):
            self.cat.buscar("minecraft")

    def test_timeout(self):
        self.srv.modo("lento")
        guardado, catalogo_mod.ESPERA = catalogo_mod.ESPERA, 0.4
        try:
            inicio = time.time()
            with self.assertRaises(SinRed):
                self.cat.categorias()
            self.assertLess(time.time() - inicio, 2.5)
        finally:
            catalogo_mod.ESPERA = guardado

    def test_cache_danada_se_ignora(self):
        self.cat.categorias()
        for f in os.listdir(self.cache):
            with open(os.path.join(self.cache, f), "w") as fh:
                fh.write("{esto no es json")
        self.assertEqual(len(self.cat.categorias()), 11)
        for f in os.listdir(self.cache):
            self.assertEqual(json.loads(leer_archivo(os.path.join(self.cache, f)))["datos"],
                             json.loads(leer_archivo(os.path.join(AQUI, "datos", "categorias.json"))))

    def test_respuesta_que_no_es_json_cuenta_como_fallo(self):
        with open(os.path.join(self.srv.datos, "categorias.json"), "w") as f:
            f.write("<html>no soy json</html>")
        with self.assertRaises(SinRed):
            self.cat.categorias()

    def test_cache_en_una_carpeta_que_no_se_puede_escribir(self):
        c = Catalogo(cache_dir="/proc/no-se-puede-escribir/aqui", api=self.srv.url)
        self.assertEqual(len(c.categorias()), 11)                         # funciona igual, solo sin caché


class TestTexto(unittest.TestCase):
    def test_parrafos_y_vinetas(self):
        html = "<p>Hola <em>mundo</em> &amp; todos.</p>\n   <p>Funciones:</p><ul><li>Una</li>\n<li>Dos   cosas</li></ul>\n<p>Fin.</p>"
        self.assertEqual(html_a_texto(html), "Hola mundo & todos.\n\nFunciones:\n• Una\n• Dos cosas\n\nFin.")

    def test_lista_ordenada_y_anidada(self):
        self.assertEqual(html_a_texto("<ol><li>Uno</li><li>Dos</li></ol><ul><li>X</li></ul>"),
                         "1. Uno\n2. Dos\n\n• X")

    def test_salto_de_linea_y_entidades(self):
        self.assertEqual(html_a_texto("<p>a<br/>b &lt;c&gt; &quot;d&quot; &#233;</p>"), 'a\nb <c> "d" é')

    def test_texto_plano_pasa_casi_igual(self):
        self.assertEqual(html_a_texto("  Linea uno\n      linea dos\n\n\n\n   otra  "), "Linea uno\nlinea dos\n\notra")

    def test_vacios(self):
        for v in (None, "", "   ", "<p></p>", "<ul></ul>"):
            self.assertEqual(html_a_texto(v), "")

    def test_no_hay_etiquetas_ni_scripts_sueltos(self):
        t = html_a_texto("<p>Hola</p><script>alert(1)</script><img src=x onerror=y><a href='http://x'>enlace</a>")
        self.assertNotIn("<", t)
        self.assertIn("Hola", t)
        self.assertIn("enlace", t)


class TestIds(unittest.TestCase):
    def test_validos(self):
        for i in ("org.mozilla.firefox", "com.valvesoftware.Steam", "net.davidotek.pupgui2", "sh.ppy.osu",
                  "io.github.ilya-zlobintsev.LACT", "org.pegasus_frontend.Pegasus", "a.b", "com.0x.App1"):
            self.assertTrue(es_id_valido(i), i)
            self.assertEqual(validar_id(i), i)

    def test_raros(self):
        for i in ("; rm -rf ~", "org.x.y; rm -rf ~", "a b.c", "$(touch x).y.z", "`id`.a.b", "../../x.y", "a/b.c",
                  "-rf.a.b", "--help", "org..x", ".org.x", "org.x.", "org.x\n", "org.x y", "sin_puntos", "", " ",
                  "a" * 300 + ".b", "org.x|y", "org.x&y", "org.x'y", 'org.x"y', "org.x\\y", "ñ.ñ", None, 7, b"a.b", ["a.b"]):
            self.assertFalse(es_id_valido(i), repr(i))
            with self.assertRaises(ValueError):
                validar_id(i)


# ====================================================================== flatpak (CLI falsa)

class BaseFlatpak(unittest.TestCase):
    """Un PATH con un «flatpak» falso al principio y su carpeta de estado."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="vortex-flatpak-prueba.")
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.estado = os.path.join(self.tmp, "estado")
        os.makedirs(self.estado)
        binarios = os.path.join(self.tmp, "bin")
        os.makedirs(binarios)
        destino = os.path.join(binarios, "flatpak")
        shutil.copy(os.path.join(AQUI, "flatpak_falso"), destino)
        os.chmod(destino, 0o755)
        self.entorno_antes = dict(os.environ)
        os.environ["PATH"] = binarios + os.pathsep + os.environ["PATH"]
        os.environ["FAKE_FLATPAK_DIR"] = self.estado
        os.environ.pop("FAKE_FLATPAK_FALLA", None)
        os.environ["HOME"] = self.tmp                  # nada escribe en tu casa de verdad
        self.addCleanup(self._restaurar)
        self.gi_antes = flatpak._gi
        flatpak._gi = None                              # por defecto, la CLI
        self.addCleanup(setattr, flatpak, "_gi", self.gi_antes)

    def _restaurar(self):
        os.environ.clear()
        os.environ.update(self.entorno_antes)

    def escribir(self, nombre, lineas):
        with open(os.path.join(self.estado, nombre), "w") as f:
            f.write("".join(l + "\n" for l in lineas))

    def leer(self, nombre):
        try:
            return leer_archivo(os.path.join(self.estado, nombre)).split()
        except OSError:
            return []

    def llamadas(self):
        try:
            return leer_archivo(os.path.join(self.estado, "llamadas.log")).splitlines()
        except OSError:
            return []

    def avance(self):
        eventos = []
        return eventos, lambda p, t: eventos.append((p, t))


class TestFlatpakCli(BaseFlatpak):
    def test_instaladas_junta_usuario_y_sistema(self):
        self.escribir("usuario.txt", ["org.mozilla.firefox", "io.qt.qtwebengine.BaseApp"])
        self.escribir("sistema.txt", ["org.vinegarhq.Sober", "org.mozilla.firefox", "Application ID", "; rm -rf ~"])
        self.assertEqual(flatpak.instaladas(), ["org.mozilla.firefox", "org.vinegarhq.Sober"])

    def test_es_del_sistema(self):
        self.escribir("usuario.txt", ["org.mozilla.firefox"])
        self.escribir("sistema.txt", ["org.vinegarhq.Sober", "org.mozilla.firefox"])
        self.assertTrue(flatpak.es_del_sistema("org.vinegarhq.Sober"))
        self.assertFalse(flatpak.es_del_sistema("org.mozilla.firefox"))
        self.assertFalse(flatpak.es_del_sistema("com.spotify.Client"))

    def test_preparar_agrega_flathub_al_usuario_una_vez(self):
        self.assertTrue(flatpak.preparar())
        self.assertEqual(self.leer("remotos.txt"), ["flathub"])
        self.assertFalse(flatpak.preparar())
        agregados = [l for l in self.llamadas() if l.startswith("remote-add")]
        self.assertEqual(agregados, ["remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo"])

    def test_instalar_con_progreso(self):
        eventos, cb = self.avance()
        flatpak.instalar("com.spotify.Client", cb)
        self.assertIn("com.spotify.Client", self.leer("usuario.txt"))
        self.assertEqual(eventos[-1], (100, "Listo"))
        porcentajes = [p for p, _ in eventos]
        self.assertEqual(porcentajes, sorted(porcentajes), "el progreso nunca retrocede")
        self.assertTrue(all(0 <= p <= 100 for p in porcentajes))
        descargas = [(p, t) for p, t in eventos if t.startswith("Descargando…")]
        self.assertGreaterEqual(len(descargas), 4)
        self.assertTrue(all(t == "Descargando… %d %%" % int(t.split()[1]) for _, t in descargas), descargas)
        self.assertTrue(any(t == "Instalando…" for _, t in eventos))
        self.assertTrue(any(0 < p < 100 for p, _ in eventos))
        self.assertIn("install --user --noninteractive -y flathub com.spotify.Client", self.llamadas())
        self.assertEqual(self.leer("remotos.txt"), ["flathub"])           # preparó Flathub antes
        self.assertFalse([l for l in self.llamadas() if "--system" in l])

    def test_instalar_algo_que_ya_esta_no_repite(self):
        self.escribir("usuario.txt", ["com.spotify.Client"])
        self.escribir("sistema.txt", ["org.vinegarhq.Sober"])
        for app_id in ("com.spotify.Client", "org.vinegarhq.Sober"):
            eventos, cb = self.avance()
            flatpak.instalar(app_id, cb)
            self.assertEqual(eventos, [(100, "Ya estaba instalada")])
        self.assertFalse([l for l in self.llamadas() if l.startswith("install")])

    def test_instalar_sin_callback(self):
        flatpak.instalar("com.spotify.Client")
        flatpak.instalar("org.videolan.VLC", None)
        self.assertEqual(self.leer("usuario.txt"), ["com.spotify.Client", "org.videolan.VLC"])

    def test_un_callback_que_falla_no_rompe_la_instalacion(self):
        def malo(p, t):
            raise RuntimeError("la interfaz se rompió")
        flatpak.instalar("com.spotify.Client", malo)
        self.assertIn("com.spotify.Client", self.leer("usuario.txt"))

    def test_quitar(self):
        self.escribir("usuario.txt", ["com.spotify.Client", "org.videolan.VLC"])
        eventos, cb = self.avance()
        flatpak.quitar("com.spotify.Client", cb)
        self.assertEqual(self.leer("usuario.txt"), ["org.videolan.VLC"])
        self.assertEqual(eventos[0], (0, "Quitando…"))
        self.assertEqual(eventos[-1], (100, "Listo"))
        self.assertIn("uninstall --user --noninteractive -y com.spotify.Client", self.llamadas())

    def test_quitar_una_del_sistema_lo_dice_claro(self):
        self.escribir("sistema.txt", ["org.vinegarhq.Sober"])
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.quitar("org.vinegarhq.Sober")
        self.assertIn("parte del sistema", str(cm.exception))
        self.assertIn("no se puede quitar", str(cm.exception))
        self.assertFalse([l for l in self.llamadas() if l.startswith("uninstall")])

    def test_quitar_una_que_no_esta(self):
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.quitar("com.spotify.Client")
        self.assertIn("no está instalada", str(cm.exception))

    def test_actualizaciones_y_actualizar_todas(self):
        self.escribir("actualizaciones.txt", ["org.videolan.VLC", "com.spotify.Client"])
        self.assertEqual(flatpak.actualizaciones(), ["com.spotify.Client", "org.videolan.VLC"])
        eventos, cb = self.avance()
        flatpak.actualizar(None, cb)
        self.assertEqual(self.leer("actualizaciones.txt"), [])
        self.assertEqual(eventos[-1], (100, "Listo"))
        porcentajes = [p for p, _ in eventos]
        self.assertEqual(porcentajes, sorted(porcentajes))
        self.assertTrue(any(t.startswith("Descargando…") for _, t in eventos))
        self.assertIn("update --user --noninteractive -y", self.llamadas())

    def test_actualizar_solo_unas(self):
        self.escribir("actualizaciones.txt", ["org.videolan.VLC", "com.spotify.Client"])
        flatpak.actualizar(["com.spotify.Client", "org.mozilla.firefox"])        # firefox no tiene nada pendiente
        self.assertEqual(self.leer("actualizaciones.txt"), ["org.videolan.VLC"])
        self.assertIn("update --user --noninteractive -y com.spotify.Client", self.llamadas())

    def test_actualizar_sin_nada_pendiente(self):
        eventos, cb = self.avance()
        flatpak.actualizar(["org.videolan.VLC"], cb)
        self.assertEqual(eventos[-1], (100, "Todo está al día"))
        self.assertFalse([l for l in self.llamadas() if l.startswith("update")])

    # -- errores en español
    def test_sin_internet(self):
        os.environ["FAKE_FLATPAK_FALLA"] = "red"
        for llamada in (lambda: flatpak.instalar("com.spotify.Client"), flatpak.preparar):
            with self.assertRaises(FlatpakSinRed) as cm:
                llamada()
            self.assertIsInstance(cm.exception, SinRed)
            self.assertIsInstance(cm.exception, ErrorFlatpak)
            self.assertIn("conexión a internet", str(cm.exception))

    def test_sin_espacio(self):
        self.escribir("remotos.txt", ["flathub"])
        os.environ["FAKE_FLATPAK_FALLA"] = "espacio"
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.instalar("com.spotify.Client")
        self.assertIn("No hay espacio suficiente", str(cm.exception))
        self.assertIn("«com.spotify.Client»", str(cm.exception))

    def test_la_app_no_existe_en_flathub(self):
        self.escribir("remotos.txt", ["flathub"])
        os.environ["FAKE_FLATPAK_FALLA"] = "noexiste"
        with self.assertRaises(AppNoEncontrada) as cm:
            flatpak.instalar("org.no.Existe")
        self.assertIn("Flathub no tiene", str(cm.exception))

    def test_error_desconocido_trae_el_motivo(self):
        self.escribir("remotos.txt", ["flathub"])
        os.environ["FAKE_FLATPAK_FALLA"] = "sistema"
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.instalar("com.spotify.Client")
        self.assertTrue(str(cm.exception).startswith("No hay permiso para instalar"), str(cm.exception))

    def test_sin_flatpak_instalado(self):
        os.environ["PATH"] = "/nonexistent-vortex"
        for llamada in (flatpak.instaladas, flatpak.preparar, lambda: flatpak.instalar("a.b.c"),
                        lambda: flatpak.abrir("a.b.c")):
            with self.assertRaises(ErrorFlatpak) as cm:
                llamada()
            self.assertIn("Flatpak no está instalado", str(cm.exception))

    # -- ids raros: se rechazan antes de llamar a nada
    def test_ids_raros_se_rechazan_sin_tocar_flatpak(self):
        marca = os.path.join(self.tmp, "pwned")
        raros = ["; rm -rf ~", "com.x.y; touch %s" % marca, "$(touch %s).a.b" % marca, "a b.c.d", "`touch %s`.a.b" % marca,
                 "--system.a.b", "../../x.y", "org.x\n", "", None, 5, "org.x|y"]
        antes = self.llamadas()
        for malo in raros:
            for llamada in (flatpak.instalar, flatpak.quitar, flatpak.abrir, flatpak.es_del_sistema,
                            lambda i: flatpak.actualizar([i])):
                with self.assertRaises(ErrorFlatpak):
                    llamada(malo)
        self.assertEqual(self.llamadas(), antes)
        self.assertFalse(os.path.exists(marca))

    # -- abrir
    def test_abrir_desacoplada(self):
        self.escribir("usuario.txt", ["org.videolan.VLC"])
        inicio = time.time()
        flatpak.abrir("org.videolan.VLC")
        self.assertLess(time.time() - inicio, 0.3, "abrir() no espera a que la app termine")
        with self.assertRaises(ChildProcessError):                       # no queda ningún hijo (ni zombi)
            os.waitpid(-1, os.WNOHANG)
        marca = os.path.join(self.estado, "abierta-org.videolan.VLC")
        for _ in range(100):
            if os.path.exists(marca):
                break
            time.sleep(0.05)
        self.assertTrue(os.path.exists(marca), "la app abierta siguió corriendo sola")

    def test_abrir_una_que_no_esta_instalada(self):
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.abrir("org.videolan.VLC")
        self.assertIn("no está instalada", str(cm.exception))
        self.assertFalse([l for l in self.llamadas() if l.startswith("run")])

    def test_abrir_una_del_sistema(self):
        self.escribir("sistema.txt", ["org.vinegarhq.Sober"])
        flatpak.abrir("org.vinegarhq.Sober")
        marca = os.path.join(self.estado, "abierta-org.vinegarhq.Sober")
        for _ in range(100):
            if os.path.exists(marca):
                break
            time.sleep(0.05)
        self.assertTrue(os.path.exists(marca))

    # -- una operación a la vez
    def test_dos_operaciones_hacen_fila(self):
        self.escribir("remotos.txt", ["flathub"])
        eventos_b, cb_b = self.avance()
        a = threading.Thread(target=flatpak.instalar, args=("com.spotify.Client",))
        a.start()
        time.sleep(0.12)
        flatpak.instalar("org.videolan.VLC", cb_b)
        a.join()
        self.assertEqual(eventos_b[0], (0, "Esperando a que termine otra operación…"))
        self.assertEqual(eventos_b[-1], (100, "Listo"))
        self.assertEqual(sorted(self.leer("usuario.txt")), ["com.spotify.Client", "org.videolan.VLC"])


# ====================================================================== flatpak (libflatpak falsa)

class _Error(Exception):
    def __init__(self, message):
        super().__init__(message)
        self.message = message


class _Progreso:
    def __init__(self):
        self.manejadores = []
        self.valor = 0
        self.estimando = False

    def connect(self, nombre, f, *a):
        assert nombre == "changed"
        self.manejadores.append(f)

    def set_update_frequency(self, ms):
        pass

    def get_progress(self):
        return self.valor

    def get_is_estimating(self):
        return self.estimando

    def mover(self, valor):
        self.valor = valor
        for f in self.manejadores:
            f(self)


class _Op:
    def __init__(self, ref, tam, tipo):
        self.ref, self.tam, self.tipo = ref, tam, tipo

    def get_ref(self):
        return self.ref

    def get_download_size(self):
        return self.tam

    def get_operation_type(self):
        return self.tipo


class _Ref:
    def __init__(self, nombre, tipo="app", titulo=None):
        self.nombre, self.tipo, self.titulo = nombre, tipo, titulo

    def get_name(self):
        return self.nombre

    def get_kind(self):
        return self.tipo

    def get_appdata_name(self):
        return self.titulo

    def format_ref(self):
        return "%s/%s/x86_64/stable" % (self.tipo, self.nombre)


class _Instalacion:
    def __init__(self, instaladas=(), para_actualizar=()):
        self.instaladas = list(instaladas)
        self.para_actualizar = list(para_actualizar)

    def list_installed_refs_by_kind(self, tipo, cancelable):
        return [r for r in self.instaladas if r.tipo == tipo]

    def list_installed_refs_for_update(self, cancelable):
        return list(self.para_actualizar)


class _Transaccion:
    """Una Transaction que, al correr, dispara las señales como libflatpak: ready, new-operation (con su
    progreso), operation-done... Lo que debe pasar lo fija la prueba en `guion`."""
    ultima = None
    guion = None

    def __init__(self, instalacion):
        self.instalacion = instalacion
        self.señales = {}
        self.pendientes, self.quitar, self.actualizar = [], [], []
        self.sin_interaccion = None
        self.sin_usar = None
        self.ops = []
        _Transaccion.ultima = self

    @classmethod
    def new_for_installation(cls, instalacion, cancelable):
        return cls(instalacion)

    def set_no_interaction(self, v):
        self.sin_interaccion = v

    def set_include_unused_uninstall_ops(self, v):
        self.sin_usar = v

    def connect(self, nombre, f):
        self.señales[nombre] = f

    def add_install(self, remoto, ref, subrutas):
        self.pendientes.append((remoto, ref))

    def add_uninstall(self, ref):
        self.quitar.append(ref)

    def add_update(self, ref, subrutas, commit):
        self.actualizar.append(ref)

    def get_operations(self):
        return self.ops

    def run(self, cancelable):
        guion = _Transaccion.guion
        self.ops = [_Op(ref, tam, tipo) for ref, tam, tipo in guion["ops"]]
        assert self.señales["ready"](self) is True
        for op, pasos in zip(self.ops, guion["avance"]):
            progreso = _Progreso()
            self.señales["new-operation"](self, op, progreso)
            for paso in pasos:
                progreso.mover(paso)
            self.señales["operation-done"](self, op, "abc", 0)
        if guion.get("error"):
            op = self.ops[-1]
            seguir = self.señales["operation-error"](self, op, _Error(guion["error"]), 0)
            assert seguir is False
            raise _Error("Aborted due to failure")


class _FlatpakFalso:
    class RefKind:
        APP = "app"
        RUNTIME = "runtime"

    class TransactionOperationType:
        INSTALL = 0
        UNINSTALL = 3

    class TransactionErrorDetails:
        NON_FATAL = 1

    Transaction = _Transaccion
    usuario = _Instalacion()
    sistema = _Instalacion()

    class Installation:
        @staticmethod
        def new_user(c):
            return _FlatpakFalso.usuario

        @staticmethod
        def new_system(c):
            return _FlatpakFalso.sistema

    @staticmethod
    def get_default_arch():
        return "x86_64"


class _GLibFalso:
    Error = _Error


class TestFlatpakGi(BaseFlatpak):
    def setUp(self):
        super().setUp()
        _FlatpakFalso.usuario = _Instalacion()
        _FlatpakFalso.sistema = _Instalacion()
        flatpak._gi = (_FlatpakFalso, _GLibFalso)
        _Transaccion.guion = None
        _Transaccion.ultima = None

    def test_instaladas_y_del_sistema(self):
        _FlatpakFalso.usuario = _Instalacion([_Ref("org.mozilla.firefox"), _Ref("org.gnome.Platform", "runtime")])
        _FlatpakFalso.sistema = _Instalacion([_Ref("org.vinegarhq.Sober"), _Ref("io.qt.qtwebengine.BaseApp"),
                                              _Ref("org.mozilla.firefox")])
        self.assertEqual(flatpak.instaladas(), ["org.mozilla.firefox", "org.vinegarhq.Sober"])
        self.assertTrue(flatpak.es_del_sistema("org.vinegarhq.Sober"))
        self.assertFalse(flatpak.es_del_sistema("org.mozilla.firefox"))

    def test_instalar_pondera_el_progreso_por_tamano(self):
        _Transaccion.guion = {"ops": [("runtime/org.kde.Platform/x86_64/6.10", 900, 0),
                                      ("app/com.spotify.Client/x86_64/stable", 100, 0)],
                              "avance": [[0, 50, 100], [0, 50, 100]]}
        eventos, cb = self.avance()
        flatpak.instalar("com.spotify.Client", cb)
        t = _Transaccion.ultima
        self.assertEqual(t.pendientes, [("flathub", "app/com.spotify.Client/x86_64/stable")])
        self.assertTrue(t.sin_interaccion)
        self.assertEqual(eventos[-1], (100, "Listo"))
        porcentajes = [p for p, _ in eventos]
        self.assertEqual(porcentajes, sorted(porcentajes))
        self.assertTrue(all(p < 100 for p in porcentajes[:-1]))
        # el runtime (900 de 1000) hecho: ya va por ~90 %, y a la mitad de la app por ~95 %
        self.assertTrue(any(88 <= p <= 91 for p in porcentajes), porcentajes)
        self.assertTrue(any(94 <= p <= 96 for p in porcentajes), porcentajes)
        self.assertIn(("Descargando… 50 %"), [t for _, t in eventos])
        self.assertIn("Instalando…", [t for _, t in eventos])

    def test_instalar_dispara_las_senales_con_los_tipos_de_libflatpak(self):
        _Transaccion.guion = {"ops": [], "avance": []}
        flatpak.instalar("com.spotify.Client")
        t = _Transaccion.ultima
        self.assertIs(t.señales["add-new-remote"](t, 0, "x", "y", "z"), False)
        self.assertEqual(t.señales["choose-remote-for-ref"](t, "ref", "rt", ["otro", "flathub"]), 1)
        self.assertEqual(t.señales["choose-remote-for-ref"](t, "ref", "rt", ["otro"]), 0)
        # los fallos de extras opcionales no frenan la transacción; los demás sí
        op = _Op("x", 1, 0)
        self.assertTrue(t.señales["operation-error"](t, op, _Error("no pasa nada"), 1))
        self.assertFalse(t.señales["operation-error"](t, op, _Error("grave"), 0))

    def test_instalar_sin_internet(self):
        _Transaccion.guion = {"ops": [("app/com.spotify.Client/x86_64/stable", 100, 0)], "avance": [[0, 10]],
                              "error": "While fetching https://dl.flathub.org/repo/summary: Could not resolve host"}
        with self.assertRaises(FlatpakSinRed):
            flatpak.instalar("com.spotify.Client")

    def test_instalar_con_otro_error_trae_el_motivo(self):
        _Transaccion.guion = {"ops": [("app/com.spotify.Client/x86_64/stable", 100, 0)], "avance": [[0]],
                              "error": "Commit abc no firmado"}
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.instalar("com.spotify.Client")
        self.assertIn("No se pudo instalar «com.spotify.Client»", str(cm.exception))
        self.assertIn("no firmado", str(cm.exception))

    def test_instalar_ya_instalada(self):
        _FlatpakFalso.sistema = _Instalacion([_Ref("org.vinegarhq.Sober")])
        eventos, cb = self.avance()
        flatpak.instalar("org.vinegarhq.Sober", cb)
        self.assertEqual(eventos, [(100, "Ya estaba instalada")])
        self.assertIsNone(_Transaccion.ultima)

    def test_quitar(self):
        _FlatpakFalso.usuario = _Instalacion([_Ref("com.spotify.Client", titulo="Spotify")])
        _Transaccion.guion = {"ops": [("app/com.spotify.Client/x86_64/stable", 0, _FlatpakFalso.TransactionOperationType.UNINSTALL)],
                              "avance": [[0, 100]]}
        eventos, cb = self.avance()
        flatpak.quitar("com.spotify.Client", cb)
        t = _Transaccion.ultima
        self.assertEqual(t.quitar, ["app/com.spotify.Client/x86_64/stable"])
        self.assertTrue(t.sin_usar)
        self.assertEqual(eventos[0], (0, "Quitando…"))
        self.assertEqual(eventos[-1], (100, "Listo"))
        self.assertFalse([t for _, t in eventos if t.startswith("Descargando")])

    def test_quitar_una_del_sistema(self):
        _FlatpakFalso.sistema = _Instalacion([_Ref("org.vinegarhq.Sober", titulo="Sober")])
        with self.assertRaises(ErrorFlatpak) as cm:
            flatpak.quitar("org.vinegarhq.Sober")
        self.assertIn("«Sober» es parte del sistema", str(cm.exception))
        self.assertIsNone(_Transaccion.ultima)

    def test_actualizar(self):
        app, otra = _Ref("com.spotify.Client"), _Ref("org.videolan.VLC")
        runtime = _Ref("org.kde.Platform", "runtime")
        _FlatpakFalso.usuario = _Instalacion([app, otra], [app, otra, runtime])
        self.assertEqual(flatpak.actualizaciones(), ["com.spotify.Client", "org.videolan.VLC"])
        _Transaccion.guion = {"ops": [(r.format_ref(), 10, 0) for r in (app, otra, runtime)], "avance": [[0, 100]] * 3}
        eventos, cb = self.avance()
        flatpak.actualizar(None, cb)
        self.assertEqual(_Transaccion.ultima.actualizar, [r.format_ref() for r in (app, otra, runtime)])
        self.assertEqual(eventos[-1], (100, "Listo"))
        _Transaccion.guion = {"ops": [(app.format_ref(), 10, 0)], "avance": [[0, 100]]}
        flatpak.actualizar(["com.spotify.Client", "org.kde.Platform"])
        self.assertEqual(_Transaccion.ultima.actualizar, [app.format_ref()])     # solo apps pedidas
        eventos, cb = self.avance()
        flatpak.actualizar(["org.mozilla.firefox"], cb)                           # nada para ella
        self.assertEqual(eventos[-1], (100, "Todo está al día"))


class TestPropias(unittest.TestCase):
    """Las apps de Vortexine's (Vortexine's Camera): la tienda llama a «vortex camara …». Aquí con un
    vortex falso que deja la carpeta como el de verdad, o que falla como cuando falta algo."""

    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.datos = os.path.join(self.dir, "datos")
        falso = os.path.join(self.dir, "vortex")
        with open(falso, "w", encoding="utf-8") as f:
            f.write("#!/usr/bin/env bash\n"
                    'echo "$*" >>"$(dirname "$0")/llamadas"\n'
                    'd="$XDG_DATA_HOME/vortex-camera/vortex_camera"\n'
                    'case "$2" in\n'
                    '  instalar) [[ -e "$(dirname "$0")/fallar" ]] && { echo "vortex camara: le falta: ffmpeg" >&2; exit 2; }\n'
                    '            mkdir -p "$d" && : >"$d/__init__.py" ;;\n'
                    '  quitar) rm -rf "$XDG_DATA_HOME/vortex-camera" ;;\n'
                    "esac\n")
        os.chmod(falso, 0o755)
        self.antes = {k: os.environ.get(k) for k in ("VORTEX_SHOP_VORTEX", "XDG_DATA_HOME")}
        os.environ["VORTEX_SHOP_VORTEX"] = falso
        os.environ["XDG_DATA_HOME"] = self.datos

    def tearDown(self):
        for k, v in self.antes.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        shutil.rmtree(self.dir)

    def test_instalar_y_quitar(self):
        self.assertEqual([a["id"] for a in propias.lista()], ["vortex-camera"])
        self.assertFalse(propias.lista()[0]["instalada"])
        self.assertTrue(propias.lista()[0]["icono"].startswith("file://"))
        from urllib.parse import unquote, urlparse
        self.assertTrue(os.path.isfile(unquote(urlparse(propias.lista()[0]["icono"]).path)))
        propias.instalar("vortex-camera")
        self.assertTrue(propias.instalada("vortex-camera"))
        propias.quitar("vortex-camera")
        self.assertFalse(propias.instalada("vortex-camera"))
        with open(os.path.join(self.dir, "llamadas"), encoding="utf-8") as f:
            self.assertEqual(f.read().split("\n")[:2], ["camara instalar", "camara quitar"])

    def test_error_entendible(self):
        open(os.path.join(self.dir, "fallar"), "w").close()
        with self.assertRaises(propias.ErrorPropia) as e:
            propias.instalar("vortex-camera")
        self.assertEqual(str(e.exception), "No se pudo instalar Vortexine's Camera: le falta: ffmpeg")
        with self.assertRaises(propias.ErrorPropia):
            propias.instalar("otra-cosa")


if __name__ == "__main__":
    unittest.main(verbosity=2)
