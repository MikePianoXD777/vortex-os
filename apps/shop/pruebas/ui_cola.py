"""Prueba de la lógica de la interfaz (puente + QML) con el motor falso: cola de instalaciones de una
en una, interfaz sin bloquearse, errores, búsqueda que descarta lo viejo y confirmación al cerrar.

    python apps/shop/pruebas/ui_cola.py        (sale con 0 si todo pasa)
"""
from __future__ import annotations

import os
import sys
import tempfile
import time
from pathlib import Path

os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ.setdefault("VORTEX_SHOP_COLORES", "/nonexistent/qs_colors.json")   # respaldo fijo, no el fondo de Mike
os.environ.setdefault("QT_QUICK_BACKEND", "software")
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from PySide6.QtCore import QEventLoop, QTimer, QtMsgType, qInstallMessageHandler  # noqa: E402

import falso_motor  # noqa: E402
from vortex_shop.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402

AVISOS_QT: list[str] = []
FALLOS: list[str] = []


def _mensajes(tipo, contexto, texto):
    if tipo in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg, QtMsgType.QtFatalMsg):
        AVISOS_QT.append(texto)
        print(f"[qt] {texto}", file=sys.stderr)


def esperar(ms: int) -> None:
    bucle = QEventLoop()
    QTimer.singleShot(ms, bucle.quit)
    bucle.exec()


def esperar_hasta(condicion, limite_s: float = 20.0) -> bool:
    fin = time.monotonic() + limite_s
    while time.monotonic() < fin:
        if condicion():
            return True
        esperar(30)
    return condicion()


def comprobar(condicion: bool, texto: str) -> None:
    print(("ok    " if condicion else "FALLA ") + texto)
    if not condicion:
        FALLOS.append(texto)


def main() -> int:
    qInstallMessageHandler(_mensajes)
    app = crear_aplicacion(sys.argv[:1])
    motor = falso_motor.crear_motor(tempfile.mkdtemp(prefix="vortex-shop-cola-"), duracion=1.2, latencia=0.1,
                                    instaladas=("org.mozilla.firefox", "org.audacityteam.Audacity"),
                                    actualizaciones=("org.mozilla.firefox",))
    ctl = motor.control
    ctl.del_sistema.add("org.audacityteam.Audacity")
    interfaz, puente = cargar_interfaz(app, motor)
    v = interfaz.rootObjects()[0]
    avisos: list[str] = []
    puente.aviso.connect(avisos.append)
    puente.iniciar()

    # --- Arranque: el inicio carga solo y las listas locales también ---
    comprobar(esperar_hasta(lambda: puente.inicioEstado == "listo"), "el inicio carga (estado «listo»)")
    comprobar(len(puente.recomendadas) == 6 and len(puente.populares) > 0, "recomendadas y populares llegan")
    comprobar(len(puente.categorias) == 8, "las categorías llegan")
    comprobar(esperar_hasta(lambda: len(puente.instaladas) == 2 and puente.numActualizaciones == 1),
              "instaladas y actualizaciones se cargan (contador = 1)")
    comprobar(puente.sistemaIds.get("org.audacityteam.Audacity") is True, "una app del sistema se marca (Quitar deshabilitado)")

    # --- Cola: tres instalaciones a la vez, de una en una, sin congelar la interfaz ---
    maximo = [0.0]
    ultimo = [time.monotonic()]

    def latido() -> None:
        ahora = time.monotonic()
        maximo[0] = max(maximo[0], ahora - ultimo[0])
        ultimo[0] = ahora
    reloj = QTimer()
    reloj.setInterval(20)
    reloj.timeout.connect(latido)
    reloj.start()

    ids = ["org.videolan.VLC", "org.gimp.GIMP", "org.kde.krita"]
    for i in ids:
        puente.instalar(i)
    comprobar(puente.ocupado, "ocupado = True al encolar")
    esperar(500)
    est = puente.estados
    comprobar(est[ids[0]]["fase"] == "instalando" and est[ids[0]]["pct"] > 0, "la primera se está instalando con porcentaje")
    comprobar(est[ids[1]]["fase"] == "cola" and est[ids[2]]["fase"] == "cola", "las otras dos esperan en cola")
    puente.instalar(ids[0])   # repetida: se ignora
    comprobar(esperar_hasta(lambda: not puente.ocupado, 30), "la cola termina")
    comprobar([r for r in ctl.registro if r[0] == "instalar"] == [("instalar", i) for i in ids],
              "se instalaron en el orden pedido, una por una")
    comprobar(all(puente.instaladasIds.get(i) for i in ids), "las tres quedan instaladas")
    comprobar(maximo[0] < 0.25, f"la interfaz no se bloqueó (mayor pausa del bucle: {maximo[0] * 1000:.0f} ms)")
    reloj.stop()

    # --- Error de flatpak: aviso en español, la app no queda instalada ---
    ctl.fallan.add("org.blender.Blender")
    puente.instalar("org.blender.Blender")
    comprobar(esperar_hasta(lambda: len(avisos) == 1 and not puente.ocupado, 15), "un error de flatpak lanza un aviso")
    comprobar("espacio en disco" in avisos[0], "el aviso lleva el mensaje en español del motor")
    comprobar(not puente.instaladasIds.get("org.blender.Blender"), "la app que falló no queda instalada")

    # --- Quitar y actualizar ---
    puente.quitar("org.videolan.VLC")
    comprobar(esperar_hasta(lambda: not puente.ocupado and not puente.instaladasIds.get("org.videolan.VLC")), "quitar funciona")
    puente.actualizarTodo()
    comprobar(esperar_hasta(lambda: not puente.ocupado and puente.numActualizaciones == 0), "«Actualizar todo» deja el contador en 0")

    # --- Búsqueda: una respuesta vieja no pisa a la nueva ---
    puente.buscar("a")
    puente.buscar("vlc")
    comprobar(esperar_hasta(lambda: puente.busquedaEstado == "listo" and puente.busquedaTexto == "vlc"), "la búsqueda responde")
    esperar(500)
    comprobar([a["id"] for a in puente.busquedaApps] == ["org.videolan.VLC"], "solo queda el resultado de la última búsqueda")
    puente.buscar("zzzz")
    esperar_hasta(lambda: puente.busquedaEstado == "listo" and puente.busquedaTexto == "zzzz")
    comprobar(puente.busquedaApps == [], "sin resultados: lista vacía")

    # --- Categoría con «Mostrar más» ---
    puente.abrirCategoria("Network")
    comprobar(esperar_hasta(lambda: puente.categoriaEstado == "listo"), "una categoría carga")
    primera = len(puente.categoriaApps)
    comprobar(primera == 6 and puente.categoriaHayMas, "página llena: hay más")
    puente.masDeCategoria()
    comprobar(esperar_hasta(lambda: len(puente.categoriaApps) > primera), "«Mostrar más» agrega la siguiente página")
    comprobar(esperar_hasta(lambda: not puente.categoriaHayMas), "la última página (incompleta) oculta «Mostrar más»")

    # --- Sin red: estado «sinred» y recuperación ---
    ctl.sin_red = True
    puente.reintentarTodo()
    comprobar(esperar_hasta(lambda: puente.inicioEstado == "sinred"), "sin red: el inicio pasa a «sinred»")
    ctl.sin_red = False
    puente.reintentarTodo()
    comprobar(esperar_hasta(lambda: puente.inicioEstado == "listo"), "con red otra vez: el inicio se recupera")

    # --- Cerrar con algo instalándose pide confirmación ---
    ctl.duracion = 3.0
    puente.instalar("com.obsproject.Studio")
    esperar(300)
    v.show()
    v.close()
    esperar(300)
    comprobar(v.isVisible(), "cerrar con una instalación en curso NO cierra la ventana (pregunta antes)")
    comprobar(esperar_hasta(lambda: not puente.ocupado, 15), "la instalación termina")
    v.close()
    esperar(200)
    comprobar(not v.isVisible(), "sin nada en curso, la ventana sí se cierra")

    comprobar(not AVISOS_QT, "el QML no dejó errores ni avisos")
    print("\nTODO BIEN" if not FALLOS else f"\nFALLARON {len(FALLOS)}: " + "; ".join(FALLOS))
    return 1 if FALLOS else 0


if __name__ == "__main__":
    codigo = main()
    sys.stdout.flush()
    os._exit(codigo)
