"""Pruebas de la ventana de Vortexine's Local AI, sin pantalla (offscreen) y con un Ollama falso: no
arranca Ollama, no baja nada y no toca tus chats ni tu vault (todo va a una carpeta temporal con
VORTEX_LOCAL_AI_HOME).

Cubre: la QML carga sin errores ni advertencias, cada página abre, un chat con trozos que llegan poco a
poco (con pensamiento, detener y regenerar), «Usar mi vault» con citas, bajar un modelo con progreso
falso (y cancelar), borrar un modelo, el vault (crear, editar en el editor, guardar solo, enlaces,
retroenlaces, [[ autocompletado, renombrar, borrar), «Guardar en el vault», el grafo con datos y que
Ollama se arranca solo con un aviso amable.

    cd apps/vortex-local-ai && QT_QPA_PLATFORM=offscreen python3 -m unittest discover -s pruebas -p 'test_interfaz.py'

Con VORTEX_LOCAL_AI_CAPTURAS=/carpeta guarda una captura de cada página (QQuickWindow.grabWindow).
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

# Un Ollama falso con la misma API que vortex_local_ai.ollama.Ollama (API.md).
FALSO = textwrap.dedent('''
    import threading, time
    from vortex_local_ai.ollama import Cancelado, ErrorOllama

    class FalsoOllama:
        url = "http://127.0.0.1:1"
        def __init__(self, corriendo=True):
            self.corriendo = corriendo
            self.ultimo_error = ""
            self.ultimas_estadisticas = {}
            self.arranques = 0
            self.chats = []
            self.mods = [{"nombre": "qwen3:8b", "bytes": 5.2e9, "familia": "qwen3", "parametros": "8.2B",
                          "cuantizacion": "Q4_K_M", "modificado": "", "embeddings": False}]
        def instalado(self): return True
        def disponible(self): return self.corriendo
        def asegurar_servidor(self, timeout=20):
            self.arranques += 1
            time.sleep(0.2)
            self.corriendo = True
            return True
        def modelos(self):
            if not self.corriendo:
                raise ErrorOllama("Ollama no contesta.", "conexion")
            return [dict(m) for m in self.mods]
        def cargados(self): return []
        def bajar(self, nombre, al_progreso=None, cancelar=None):
            total = 4_000_000_000
            for i in range(0, 21):
                if cancelar is not None and cancelar.is_set():
                    raise Cancelado()
                if al_progreso: al_progreso(total * i // 20, total, "Bajando" if i < 20 else "Listo")
                time.sleep(0.04)
            self.mods.append({"nombre": nombre, "bytes": float(total), "familia": "falsa", "parametros": "3B",
                              "cuantizacion": "Q4_K_M", "modificado": "", "embeddings": False})
        def borrar(self, nombre):
            self.mods = [m for m in self.mods if m["nombre"] != nombre]
        def chat(self, modelo, mensajes, al_trozo=None, cancelar=None, opciones=None, al_pensar=None):
            self.chats.append({"modelo": modelo, "mensajes": mensajes})
            if al_pensar:
                for t in ("Veamos, ", "lo pienso."):
                    al_pensar(t); time.sleep(0.03)
            salida = ""
            for t in ("Hola, ", "esto ", "es ", "**Markdown** ", "que ", "llega ", "por ", "trozos. ", "Mira [[Ideas]]."):
                if cancelar is not None and cancelar.is_set():
                    break
                salida += t
                if al_trozo: al_trozo(t)
                time.sleep(0.07)
            self.ultimas_estadisticas = {"tokens": 9, "segundos": 0.6, "tokens_por_segundo": 15.0}
            return salida
        def embeber(self, modelo, textos):
            return [[1.0, 0.0, 0.0] for _ in ([textos] if isinstance(textos, str) else textos)]
        def parar_propio(self): pass
        def descargar_de_gpu(self, modelo): pass
''')


def _guion(cuerpo: str, corriendo: bool = True) -> str:
    return textwrap.dedent("""
        import os, sys, json, time
        sys.path.insert(0, %r)
        from PySide6.QtCore import QTimer, QObject
    """ % str(RAIZ)) + FALSO + textwrap.dedent("""
        from vortex_local_ai.__main__ import crear_aplicacion, cargar_interfaz
        from vortex_local_ai import catalogo
        from vortex_local_ai.puente import Puente
        # Sin leer el vortex-ia de la máquina: si no, la ventanita del motor taparía las demás pruebas.
        catalogo.motor_pendiente = lambda: None
        app = crear_aplicacion(["la"])
        falso = FalsoOllama(corriendo=%r)
        puente = Puente(app, ollama=falso, vram={"nombre": "NVIDIA GeForce RTX 3080", "total_mb": 10240,
                                                 "libre_mb": 9500, "tipo": "nvidia", "ram_mb": 32000})
        motor, _ = cargar_interfaz(app, puente)
        assert motor.rootObjects(), "la ventana no cargó"
        ventana = motor.rootObjects()[0]
        avisos = []
        puente.general.aviso.connect(lambda t, k: avisos.append([t, k]))
        capturas = os.environ.get("VORTEX_LOCAL_AI_CAPTURAS", "")
        def capturar(nombre):
            if capturas:
                ventana.grabWindow().save(os.path.join(capturas, nombre + ".png"))
        def hijo(nombre):
            return ventana.findChild(QObject, nombre)
        tiempo = [0]
        def paso(ms, f):
            tiempo[0] += ms
            QTimer.singleShot(tiempo[0], f)
        datos = {}
        def fin():
            datos["avisos"] = avisos
            print("RESULTADO " + json.dumps(datos, ensure_ascii=False), flush=True)
            puente.cerrar()
            # Como la app (__main__.main): se sale sin desarmar la QML, que si no se queja de los objetos
            # que ya se borraron.
            sys.stdout.flush()
            sys.stderr.flush()
            os._exit(0)
    """ % corriendo) + textwrap.dedent(cuerpo) + "\napp.exec()\n"


class Interfaz(unittest.TestCase):
    def setUp(self) -> None:
        self.t = Path(tempfile.mkdtemp(prefix="vla-"))

    def tearDown(self) -> None:
        shutil.rmtree(self.t, ignore_errors=True)

    def correr(self, cuerpo: str, corriendo: bool = True) -> dict:
        guion = self.t / "guion.py"
        guion.write_text(_guion(cuerpo, corriendo), encoding="utf-8")
        entorno = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "DISPLAY")}
        # QT_FORCE_STDERR_LOGGING: sin terminal, Qt manda las advertencias al journal y no se verían aquí.
        entorno.update({"QT_QPA_PLATFORM": "offscreen", "QT_FORCE_STDERR_LOGGING": "1", "VORTEX_LOCAL_AI_HOME": str(self.t / "casa"),
                        "VORTEX_LOCAL_AI_COLORES": "/no/existe", "VORTEX_LOCAL_AI_NO_ARRANCAR": "1",
                        "VORTEX_LOCAL_AI_OLLAMA": "http://127.0.0.1:1", "XDG_RUNTIME_DIR": str(self.t),
                        "XDG_CONFIG_HOME": str(self.t / "config")})
        r = subprocess.run([sys.executable, str(guion)], env=entorno, capture_output=True, text=True, timeout=90)
        linea = next((x for x in r.stdout.splitlines() if x.startswith("RESULTADO ")), None)
        self.assertIsNotNone(linea, r.stdout[-3000:] + r.stderr[-4000:])
        d = json.loads(linea[len("RESULTADO "):])
        # Qt escribe en stderr cada advertencia o error de QML («file:///…/Main.qml:12: …», «qml: …»).
        # (Un manejador de mensajes en Python se traba si la advertencia sale del hilo que carga la QML.)
        quejas = [x for x in r.stderr.splitlines() if x.strip()]
        self.assertEqual(quejas, [], "La QML o Qt se quejaron")
        return d

    def test_paginas_y_modelos(self) -> None:
        d = self.correr("""
            def inicio():
                datos["estado"] = puente.general.estadoOllama
                datos["gpu"] = puente.modelos.gpu
                datos["instalados"] = list(puente.modelos.nombresChat)
                filas = puente.modelos.catalogo.filas()
                datos["catalogo"] = len(filas)
                datos["cabe"] = sorted({f["cabe"] for f in filas})
                datos["recomendado"] = sum(f["recomendado"] for f in filas)
                datos["modelo_chat"] = puente.chat.modelo
                capturar("chat-vacio")
            def paginas():
                datos["paginas"] = []
                for p in ("modelos", "vault", "ajustes", "chat"):
                    ventana.setProperty("pagina", p)
                    app.processEvents()
                    datos["paginas"].append(ventana.property("pagina"))
                    capturar("pagina-" + p)
                ventana.setProperty("pagina", "modelos")
            def filtrar():
                puente.modelos.filtro = "codigo"
                datos["filtrado"] = all("codigo" in f["para"] for f in puente.modelos.catalogo.filas())
                datos["n_codigo"] = puente.modelos.catalogo.count
                puente.modelos.filtro = "todos"
                libres = [f for f in puente.modelos.catalogo.filas() if not f["instalado"] and f["cabe"] == "si"]
                datos["a_bajar"] = libres[0]["ident"]
                datos["a_cancelar"] = libres[1]["ident"]
                puente.modelos.bajar(datos["a_bajar"])
                datos["progresos"] = []
            def mirar():
                i = puente.modelos.catalogo.buscar("ident", datos["a_bajar"])
                f = puente.modelos.catalogo.fila(i)
                datos["progresos"].append([f["bajando"], round(f["progreso"], 2), f["estado"]])
                capturar("bajando")
            def bajado():
                i = puente.modelos.catalogo.buscar("ident", datos["a_bajar"])
                f = puente.modelos.catalogo.fila(i)
                datos["bajado"] = [f["bajando"], f["instalado"]]
                datos["instalados_despues"] = list(puente.modelos.nombresChat)
                puente.modelos.bajar(datos["a_cancelar"])
            def cancelar():
                puente.modelos.cancelar(datos["a_cancelar"])
            def cancelado():
                i = puente.modelos.catalogo.buscar("ident", datos["a_cancelar"])
                f = puente.modelos.catalogo.fila(i)
                datos["cancelado"] = [f["bajando"], f["instalado"]]
                puente.modelos.borrar("qwen3:8b")
            def borrado():
                datos["tras_borrar"] = list(puente.modelos.nombresChat)
                datos["modelo_chat_tras_borrar"] = puente.chat.modelo
                fin()
            paso(600, inicio)
            paso(100, paginas)
            paso(200, filtrar)
            for _ in range(4):
                paso(150, mirar)
            paso(900, bajado)
            paso(150, cancelar)
            paso(400, cancelado)
            paso(400, borrado)
        """)
        self.assertEqual(d["estado"], "corriendo")
        self.assertEqual(d["gpu"], "Tu gráfica: RTX 3080 · 10 GB")
        self.assertEqual(d["instalados"], ["qwen3:8b"])
        self.assertGreater(d["catalogo"], 5)
        self.assertIn("si", d["cabe"])
        self.assertIn("no", d["cabe"])
        self.assertEqual(d["recomendado"], 1)
        self.assertEqual(d["modelo_chat"], "qwen3:8b")
        self.assertEqual(d["paginas"], ["modelos", "vault", "ajustes", "chat"])
        self.assertTrue(d["filtrado"])
        self.assertGreater(d["n_codigo"], 0)
        en_curso = [p for p in d["progresos"] if p[0]]
        self.assertTrue(en_curso, d["progresos"])
        self.assertTrue(any(0 < p[1] < 1 for p in en_curso), d["progresos"])
        self.assertEqual(d["bajado"], [False, True])
        self.assertIn(d["a_bajar"], d["instalados_despues"])
        self.assertEqual(d["cancelado"], [False, False])
        self.assertNotIn("qwen3:8b", d["tras_borrar"])
        self.assertEqual(d["modelo_chat_tras_borrar"], d["a_bajar"])
        textos = [a[0] for a in d["avisos"]]
        self.assertTrue(any("ya está instalado" in t for t in textos), textos)
        self.assertTrue(any("Cancelé la descarga" in t for t in textos), textos)

    def test_chat(self) -> None:
        d = self.correr("""
            from vortex_local_ai import conversaciones
            m = puente.chat.mensajes
            def mandar():
                puente.chat.enviar("Hola, ¿qué tal?")
                datos["parciales"] = []
            def mirar():
                f = m.fila(m.count - 1)
                datos["parciales"].append([f["escribiendo"], f["contenido"], f["pensamiento"]])
                capturar("chat-escribiendo")
            def termino():
                f = m.fila(m.count - 1)
                datos["final"] = [f["rol"], f["contenido"], f["pensamiento"], f["escribiendo"], f["datos"]]
                datos["generando"] = puente.chat.generando
                datos["lista"] = puente.chat.conversaciones.count
                datos["titulo"] = puente.chat.titulo
                datos["vista"] = puente.boveda.vista(f["contenido"])
                capturar("chat-respuesta")
                puente.chat.enviar("Otra pregunta")
            def detener():
                puente.chat.detener()
            def detenido():
                f = m.fila(m.count - 1)
                datos["detenido"] = [f["escribiendo"], f["datos"], len(f["contenido"]) < 40]
                puente.chat.regenerar()
            def regenerado():
                datos["tras_regenerar"] = [x["rol"] for x in m.filas()]
                datos["ultimo"] = m.fila(m.count - 1)["contenido"]
                datos["historial_mandado"] = [x["role"] for x in falso.chats[-1]["mensajes"]]
                conv = conversaciones.abrir(puente.chat.actual)
                datos["guardados"] = [x["role"] for x in conv["mensajes"]]
                # Usar mi vault: la nota se cita como chip
                v = puente.boveda.vault
                v.guardar("Ideas.md", "Quiero aprender piano y hacer juegos.\\n")
                puente.boveda.recargar()
                puente.chat.nuevo()
                puente.chat.usarVault = True
                puente.chat.enviar("¿Qué quiero aprender de piano?")
            def con_vault():
                f = m.fila(m.count - 1)
                datos["citas"] = f["citas"]
                datos["system"] = falso.chats[-1]["mensajes"][0]["role"]
                puente.chat.guardarEnVault()
            def guardado():
                datos["chats_en_vault"] = sorted(p.name for p in (puente.boveda.raiz / "Chats").glob("*.md"))
                # abrir uno viejo
                primero = puente.chat.conversaciones.fila(puente.chat.conversaciones.count - 1)["clave"]
                puente.chat.abrir(primero)
                datos["abierto"] = [x["rol"] for x in m.filas()]
                capturar("chat-abierto")
                fin()
            paso(500, mandar)
            for _ in range(3):
                paso(180, mirar)
            paso(900, termino)
            paso(250, detener)
            paso(400, detenido)
            paso(1500, regenerado)
            paso(1500, con_vault)
            paso(300, guardado)
        """)
        en_curso = [p for p in d["parciales"] if p[0]]
        self.assertTrue(en_curso, d["parciales"])
        self.assertTrue(any(0 < len(p[1]) < 60 for p in en_curso), "los trozos no llegaron poco a poco")
        rol, contenido, pensamiento, escribiendo, extra = d["final"]
        self.assertEqual(rol, "assistant")
        self.assertTrue(contenido.startswith("Hola, esto es **Markdown**"), contenido)
        self.assertEqual(pensamiento, "Veamos, lo pienso.")
        self.assertFalse(escribiendo)
        self.assertIn("palabras/s", extra)
        self.assertFalse(d["generando"])
        self.assertEqual(d["lista"], 1)
        self.assertTrue(d["titulo"])
        self.assertIn("[Ideas](nota:Ideas)", d["vista"])
        self.assertFalse(d["detenido"][0])
        self.assertTrue(d["detenido"][1].startswith("Detenido"), d["detenido"])
        self.assertTrue(d["detenido"][2])
        self.assertEqual(d["tras_regenerar"], ["user", "assistant", "user", "assistant"])
        self.assertTrue(d["ultimo"].endswith("[[Ideas]]."))
        self.assertEqual(d["historial_mandado"], ["user", "assistant", "user"])
        self.assertEqual(d["guardados"], ["user", "assistant", "user", "assistant"])
        self.assertIn("Ideas.md", [c["ruta"] for c in d["citas"]])
        self.assertEqual(d["system"], "system")
        self.assertEqual(len(d["chats_en_vault"]), 1)
        self.assertEqual(d["abierto"], ["user", "assistant", "user", "assistant"])

    def test_vault_y_grafo(self) -> None:
        d = self.correr("""
            b = puente.boveda
            def crear():
                ventana.setProperty("pagina", "vault")
                datos["ruta1"] = b.nuevaNota("Ideas", "")
                b.nuevaCarpeta("Proyectos", "")
                datos["ruta2"] = b.nuevaNota("Juegos", "Proyectos")
                b.abrir(datos["ruta1"])
                editor = hijo("editor")
                editor.setProperty("text", "Me gustan los [[Juegos]] y el piano. #musica\\n\\n[[Nota que falta]]")
            def guardado():
                datos["en_disco"] = (b.raiz / "Ideas.md").read_text(encoding="utf-8")
                datos["enlaces"] = [[e["nombre"], e["existe"]] for e in b.enlaces]
                datos["etiquetas"] = list(b.etiquetasNota)
                datos["sugerir"] = list(b.sugerir("jue"))
                capturar("vault-editor")
                hijo("paginaVault").setProperty("modo", "lado")
                b.abrir(datos["ruta2"])
            def retro():
                datos["retro"] = [r["titulo"] for r in b.retroenlaces]
                datos["arbol"] = [[f["tipo"], f["nombre"], f["nivel"]] for f in b.arbol.filas()]
                capturar("vault-lado-a-lado")
                b.busqueda = "piano"
                datos["busqueda"] = [r["titulo"] for r in b.resultados]
                b.busqueda = ""
                b.abrirEnlace("Nota que falta")
            def creada():
                datos["creada"] = b.nota
                b.pedirGrafo()
            def grafo():
                g = b.grafo
                datos["nodos"] = sorted(n["titulo"] for n in g["nodos"])
                datos["aristas"] = len(g["aristas"])
                hijo("paginaVault").setProperty("verGrafo", True)
            def grafo_mueve():
                gr = hijo("grafo")
                datos["grafo_cuantos"] = gr.property("cuantos")
                datos["energia"] = gr.property("energia")
                capturar("vault-grafo")
                hijo("paginaVault").setProperty("verGrafo", False)
                b.renombrar(datos["ruta2"], "Videojuegos")
            def renombrado():
                datos["tras_renombrar"] = (b.raiz / "Ideas.md").read_text(encoding="utf-8")
                b.borrar("Nota que falta.md")
            def borrado():
                datos["papelera"] = sorted(p.name for p in (b.raiz / ".trash").iterdir())
                datos["nota_tras_borrar"] = b.nota
                b.actualizarMemoria()
            def memoria():
                datos["memoria"] = [b.memoriaProgreso, b.memoriaTexto]
                fin()
            paso(500, crear)
            paso(1300, guardado)
            paso(300, retro)
            paso(300, creada)
            paso(500, grafo)
            paso(700, grafo_mueve)
            paso(300, renombrado)
            paso(300, borrado)
            paso(1200, memoria)
        """)
        self.assertEqual(d["ruta1"], "Ideas.md")
        self.assertEqual(d["ruta2"], "Proyectos/Juegos.md")
        self.assertIn("[[Juegos]]", d["en_disco"], "el editor no guardó solo")
        self.assertIn(["Juegos", True], d["enlaces"])
        self.assertIn(["Nota que falta", False], d["enlaces"])
        self.assertIn("musica", d["etiquetas"])
        self.assertEqual(d["sugerir"], ["Juegos"])
        self.assertEqual(d["retro"], ["Ideas"])
        self.assertIn(["carpeta", "Proyectos", 0], d["arbol"])
        self.assertIn(["nota", "Juegos", 1], d["arbol"])
        self.assertEqual(d["busqueda"], ["Ideas"])
        self.assertEqual(d["creada"], "Nota que falta.md")
        self.assertEqual(d["nodos"], ["Ideas", "Juegos", "Nota que falta"])
        self.assertEqual(d["aristas"], 2)
        self.assertEqual(d["grafo_cuantos"], 3)
        self.assertGreaterEqual(d["energia"], 0)
        self.assertIn("[[Videojuegos]]", d["tras_renombrar"])
        self.assertIn("Nota que falta.md", d["papelera"])
        self.assertEqual(d["nota_tras_borrar"], "")
        self.assertLess(d["memoria"][0], 0)
        self.assertTrue(d["memoria"][1], d["memoria"])

    def test_arranca_ollama_solo(self) -> None:
        d = self.correr("""
            def mirar():
                datos["estado"] = puente.general.estadoOllama
                datos["arranques"] = falso.arranques
                datos["instalados"] = list(puente.modelos.nombresChat)
                fin()
            paso(1500, mirar)
        """, corriendo=False)
        self.assertEqual(d["estado"], "corriendo")
        self.assertEqual(d["arranques"], 1)
        self.assertEqual(d["instalados"], ["qwen3:8b"])
        self.assertTrue(any("Arranqué Ollama" in a[0] and a[1] == "exito" for a in d["avisos"]), d["avisos"])

    def test_pregunta_del_motor_al_abrir(self) -> None:
        d = self.correr("""
            from PySide6.QtCore import QMetaObject, Qt
            elegidos = []
            catalogo.elegir_motor = lambda si: elegidos.append(si) or "ok"
            dialogo = hijo("dialogoMotor")
            def preguntar():
                datos["cerrado_al_inicio"] = not dialogo.property("visible")
                puente.general._motor_listo("¿Lo cambio?")
                app.processEvents()
                datos["abierto"] = dialogo.property("visible")
                datos["mensaje"] = dialogo.property("mensaje")
                datos["boton_no"] = dialogo.property("textoCancelar")
                capturar("pregunta-motor")
                QMetaObject.invokeMethod(dialogo, "_rechazar", Qt.DirectConnection)
            def aceptar():
                puente.general._motor_listo("¿Lo cambio?")
                app.processEvents()
                QMetaObject.invokeMethod(dialogo, "_aceptar", Qt.DirectConnection)
            def mirar():
                datos["elegidos"] = elegidos
                fin()
            paso(800, preguntar)
            paso(800, aceptar)
            paso(800, mirar)
        """)
        self.assertTrue(d["cerrado_al_inicio"])
        self.assertTrue(d["abierto"])
        self.assertEqual(d["mensaje"], "¿Lo cambio?")
        self.assertEqual(d["boton_no"], "No, gracias")
        self.assertEqual(d["elegidos"], [False, True])
        self.assertTrue(any("Listo: Ollama usará CUDA" in a[0] for a in d["avisos"]), d["avisos"])


if __name__ == "__main__":
    unittest.main()
