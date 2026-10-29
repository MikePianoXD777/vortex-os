#!/usr/bin/env python3
"""Prueba sin pantalla (QT_QPA_PLATFORM=offscreen) de la ventana con avatar: carga el VRM en QtWebEngine,
mueve la interfaz como lo haría el asistente y guarda imágenes en una carpeta.

    QT_QPA_PLATFORM=offscreen python3 -I pruebas/pagina.py MODELO.vrm CARPETA_SALIDA [sin-modelo]

Comprueba: que el modelo cargue y dibuje cuadros; que el fondo sea transparente (alfa 0 en las esquinas,
tanto en el WebGL como en lo que ve Qt); que la boca (vocales), la emoción, el sonrojo, el globo, el estado y el
crédito cambien lo que se ve; y que el puente de la página hacia Python funcione (escribir, micrófono, cerrar).
"""

import base64
import json
import os
import sys
import time
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from PySide6 import QtWebEngineWidgets  # noqa: E402,F401  (antes de crear la QApplication)
from PySide6.QtGui import QImage  # noqa: E402
from PySide6.QtGui import QGuiApplication  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402,F401

from miku import app as miku_app  # noqa: E402
from miku import rutas  # noqa: E402
from miku.ui_avatar import VentanaAvatar  # noqa: E402
from pruebas.falso import AsistenteFalso  # noqa: E402


def main() -> int:
    modelo = Path(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1] != "-" else None
    salida = Path(sys.argv[2])
    salida.mkdir(parents=True, exist_ok=True)
    sin_modelo = "sin-modelo" in sys.argv[3:]
    fallos = []

    def comprobar(nombre, cond, extra=""):
        print(("  ok  " if cond else "  MAL ") + nombre + (f"  {extra}" if extra else ""), flush=True)
        if not cond:
            fallos.append(nombre)

    app = miku_app.crear_aplicacion()
    comprobar("el app_id de Wayland es «vortex-miku» (lo que casan las reglas de Hyprland)", QGuiApplication.desktopFileName() == "vortex-miku", QGuiApplication.desktopFileName())
    asistente = AsistenteFalso()
    cerrado = []
    # En el modo «sin-modelo» se prueba además la escala de la interfaz (general.uiScale = 0.85).
    ventana = VentanaAvatar(asistente, None if sin_modelo else modelo, lambda: cerrado.append(1), escala=0.85 if sin_modelo else None)
    ventana.show()
    pagina = ventana.pag
    t0 = time.time()

    def esperar(cond_js, limite=60.0):
        """Procesa eventos hasta que la expresión JS sea verdadera."""
        fin = time.time() + limite
        while time.time() < fin:
            r = []
            pagina.runJavaScript(cond_js, lambda v: r.append(v))
            while not r and time.time() < fin:
                app.processEvents()
                time.sleep(0.01)
            if r and r[0]:
                return True
            for _ in range(10):
                app.processEvents()
                time.sleep(0.05)
        return False

    def js(codigo):
        r = []
        pagina.runJavaScript(codigo, lambda v: r.append(v))
        fin = time.time() + 10
        while not r and time.time() < fin:
            app.processEvents()
            time.sleep(0.01)
        return r[0] if r else None

    def pasar(segundos):
        fin = time.time() + segundos
        while time.time() < fin:
            app.processEvents()
            time.sleep(0.01)

    def foto(nombre):
        img = ventana.grab().toImage()
        img.save(str(salida / f"{nombre}.png"))
        return img

    print("== Carga de la página")
    comprobar("el puente QWebChannel conectó", esperar("typeof miku !== 'undefined' && !!miku.puente", 30))
    if sin_modelo:
        pasar(1.0)
        comprobar("avisa que falta el modelo con el comando para ponerlo",
                  "vortex-miku modelo RUTA.vrm" in (js("document.getElementById('sin-modelo').textContent") or ""))
        comprobar("con uiScale 0.85 la ventana mide 391x646 y la página se escala igual",
                  (ventana.width(), ventana.height()) == (391, 646) and abs(ventana.zoomFactor() - 0.85) < 1e-6,
                  f"{ventana.width()}x{ventana.height()} zoom {ventana.zoomFactor()}")
        foto("sin-modelo")
        ventana.close()
        return 1 if fallos else 0

    comprobar("el modelo 3D cargó", esperar("!!(miku.escena && miku.escena.listo)", 90), f"en {time.time() - t0:.1f}s")
    comprobar("sin error de carga", not js("miku.escena.error"), str(js("miku.escena.error")))
    comprobar("dibuja cuadros", esperar("miku.escena.cuadros >= 10", 30))
    info = json.loads(js("JSON.stringify(miku.info())") or "{}")
    print("      WebGL:", info.get("gl"), "|", info.get("gpu"), "| three", info.get("three"), "| tris", info.get("tris"), "| llamadas", info.get("calls"))
    comprobar("trae las expresiones del VRM (a, i, u, e, o, parpadeo, alegre, triste, relajada)",
              {"aa", "ih", "ou", "ee", "oh", "blink", "happy", "sad", "relaxed"} <= set(info.get("expresiones", [])))

    print("== Transparencia")
    png = js("miku.captura()")
    crudo = base64.b64decode(png.split(",", 1)[1])
    (salida / "webgl.png").write_bytes(crudo)
    img = QImage.fromData(crudo)
    esquinas = [img.pixelColor(2, 2).alpha(), img.pixelColor(img.width() - 3, 2).alpha(), img.pixelColor(2, img.height() - 3).alpha()]
    opacos = sum(1 for x in range(0, img.width(), 4) for y in range(0, img.height(), 4) if img.pixelColor(x, y).alpha() > 200)
    comprobar("WebGL: fondo transparente en las esquinas", esquinas == [0, 0, 0], f"alfa={esquinas}")
    comprobar("WebGL: el cuerpo de Miku sí es opaco", opacos > 300, f"{opacos} muestras opacas")
    g = foto("ventana-reposo")
    esq = [g.pixelColor(2, 2).alpha(), g.pixelColor(g.width() - 3, 2).alpha(), g.pixelColor(2, g.height() // 2).alpha()]
    comprobar("Qt: la ventana tiene alfa (transparente fuera del personaje)", g.hasAlphaChannel() and esq == [0, 0, 0], f"alfa={esq}")

    print("== Interfaz")
    comprobar("tamaño fijo 460x760", (ventana.width(), ventana.height()) == (460, 760))
    comprobar("título «Miku»", ventana.windowTitle() == "Miku")
    asistente.estado.emit("escuchando", "Escuchando...")
    asistente.nivel.emit(0.8)
    pasar(0.4)
    comprobar("indicador de escucha visible", "Escuchando" in (js("document.getElementById('estado-texto').textContent") or "")
              and js("document.getElementById('mic').classList.contains('activo')"))
    foto("escuchando")

    print("== Colores del fondo, primera vez y Claude apagado")
    P = ventana._vigilante.paleta
    comprobar("los colores de la página salen de la paleta del fondo",
              (js("getComputedStyle(document.documentElement).getPropertyValue('--acento').trim()") or "").lower() == P["azul"].lower(),
              f"acento {P['azul']}")
    comprobar("las esquinas son de 18 px (como en la 1.2)", js("getComputedStyle(document.documentElement).getPropertyValue('--radio').trim()") == "18px")
    asistente.progreso.emit("Preparando la voz de Miku (una sola vez, ~560 MB)…", "Bajando la voz 120 MB de 563 MB", 0.21)
    pasar(0.4)
    comprobar("muestra «Preparando la voz de Miku (una sola vez, ~560 MB)…» con su progreso",
              "~560 MB" in (js("document.getElementById('prep-titulo').textContent") or "")
              and js("document.getElementById('prep-relleno').style.width") == "21%" and js("!document.getElementById('globo').hidden"))
    foto("preparando")
    asistente.progreso.emit("", "", -1.0)
    asistente.respuesta.emit("Para hablar conmigo necesito tu cuenta de Claude.", True)
    asistente.acciones.emit([["activar", "Activar Claude"]])
    pasar(0.4)
    comprobar("botón «Activar Claude» en el globo y la descarga ya no se ve",
              js("document.querySelector('#acciones button') && document.querySelector('#acciones button').textContent") == "Activar Claude"
              and js("document.getElementById('prep').hidden"))
    foto("activar-claude")
    js("document.querySelector('#acciones button').click()")
    pasar(0.3)
    comprobar("el botón le pide a Python la acción «activar»", ("accion", "activar") in asistente.registro)
    asistente.acciones.emit([["sesion", "Iniciar sesión"]])
    asistente.respuesta.emit("No tengo sesión en Claude. Inicia sesión con tu cuenta para que pueda hablar contigo.", True)
    pasar(0.3)
    comprobar("botón «Iniciar sesión»", js("document.querySelector('#acciones button').textContent") == "Iniciar sesión")
    foto("iniciar-sesion")
    asistente.acciones.emit([])
    pasar(0.2)
    comprobar("sin botones se esconden", js("document.getElementById('acciones').hidden"))
    asistente.usuario.emit("¿Qué ventanas tengo abiertas?")
    asistente.estado.emit("pensando", "Pensando...")
    pasar(0.5)
    foto("pensando")
    asistente.estado.emit("viendo", "Mirando tu pantalla...")
    asistente.respuesta.emit("Déjame ver. Tienes la terminal y el navegador abiertos, y el editor detrás.", False)
    pasar(0.4)
    comprobar("el globo muestra la respuesta", "terminal" in (js("document.getElementById('texto').textContent") or ""))
    asistente.estado.emit("hablando", "")
    asistente.respuesta.emit("Déjame ver. Tienes la terminal y el navegador abiertos, y el editor detrás. ¿Quieres que mire algo en especial?", True)
    asistente.emocion.emit("happy", 0.35)

    print("== Sonrojo (2.0)")
    comprobar("sabe cómo sonrojarse (expresión del modelo o mejillas)", js("miku.escena.sonrojo") in ("expresion", "mejillas"),
              str(js("miku.escena.sonrojo")))
    antes = js("miku.captura()")
    asistente.sonrojo.emit(1.0)
    pasar(1.2)
    comprobar("se sonroja al recibir la señal", (js("miku.escena.sonrojoValor") or 0) > 0.6, str(js("miku.escena.sonrojoValor")))
    despues = js("miku.captura()")
    (salida / "sonrojo.png").write_bytes(base64.b64decode(despues.split(",", 1)[1]))
    comprobar("el sonrojo cambia lo que se ve", antes != despues)
    js("miku.sonrojoHasta = 0")
    comprobar("se le baja sola", esperar("miku.escena.sonrojoValor === 0", 25))

    print("== Boca: vocales con la voz")
    # Una pista de prueba: a, i, u, e, o, cada una 0,5 s al máximo.
    pista = []
    for v in range(5):
        pista += [[100 if k == v else 0 for k in range(5)]] * 15
    asistente.boca.emit(pista, 30)
    maximos = [0.0] * 5
    con_foto = set()
    fin = time.time() + 3.4
    while time.time() < fin:
        v = js("JSON.stringify(miku.bocaActual())")
        vals = json.loads(v) if v and v != "null" else None
        if vals:
            for i in range(5):
                maximos[i] = max(maximos[i], vals[i])
                if vals[i] > 0.9 and i not in con_foto:        # una foto de cada forma de la boca
                    con_foto.add(i)
                    foto(f"boca-{'aiueo'[i]}")
        pasar(0.03)
    for i, nombre in enumerate("aiueo"):
        comprobar(f"la vocal {nombre} abre su forma de boca (aa ih ou ee oh)", maximos[i] > 0.9, f"máximo {maximos[i]:.2f}")
    pasar(1.0)
    cierre = js("JSON.stringify(miku.bocaActual())")
    comprobar("al terminar la pista la boca vuelve a cerrarse", cierre in (None, "null", "[0,0,0,0,0]"), str(cierre))
    asistente.boca_fin.emit()

    print("== Crédito y puente a Python")
    credito = js("document.getElementById('credito').textContent") or ""
    esperado = rutas.leer_credito(modelo)
    comprobar("el crédito del modelo está en la página", bool(esperado) and credito.strip() == esperado, credito[:60])
    pie = js("document.getElementById('pie').textContent") or ""
    comprobar("crédito pequeño siempre visible", len(pie) > 5, pie)
    js("document.getElementById('b-info').click()")
    pasar(0.3)
    comprobar("el botón de información abre el crédito completo", js("!document.getElementById('credito').hidden"))
    foto("credito")
    js("document.getElementById('b-info').click()")
    js("document.getElementById('entrada').value = 'hola desde la caja'; document.getElementById('form').requestSubmit()")
    js("document.getElementById('mic').click()")
    asistente.estado_actual = "escuchando"
    js("document.getElementById('mic').click()")
    pasar(0.5)
    comprobar("escribir en la caja manda el texto a Python", ("enviar", "hola desde la caja") in asistente.registro)
    comprobar("clic en el micrófono pide escuchar", ("escuchar",) in asistente.registro)
    comprobar("segundo clic con la escucha abierta la corta", ("cortar",) in asistente.registro)
    js("document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape'}))")
    pasar(0.5)
    comprobar("Esc pide cerrar todo", bool(cerrado))
    foto("final")

    print("== Pausa")
    js("miku.pausar()")
    c1 = js("miku.escena.cuadros")
    pasar(0.8)
    c2 = js("miku.escena.cuadros")
    comprobar("al pausar no dibuja cuadros (CPU ~0)", c1 == c2, f"{c1} -> {c2}")

    ventana.deleteLater()
    print("FALLOS:", len(fallos), fallos if fallos else "")
    return 1 if fallos else 0


if __name__ == "__main__":
    codigo = main()
    sys.stdout.flush()
    os._exit(codigo)   # QtWebEngine tarda en cerrar sus procesos; la prueba ya terminó
