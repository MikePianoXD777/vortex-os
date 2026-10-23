"""Prueba de integración: la interfaz REAL (QML offscreen) con el motor REAL (catalogo.py + flatpak.py),
contra un Flathub FALSO en 127.0.0.1 (servidor_falso.py, vía VORTEX_SHOP_API) y un «flatpak» FALSO en el
PATH. Nada sale a internet ni toca tu flatpak. Recorre lo que hace una persona, con clics de verdad en los
botones: inicio → buscar → detalle → Instalar (con progreso) → Abrir → Quitar (con confirmación).

    python apps/shop/pruebas/ui_integracion.py        (sale con 0 si todo pasa)
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
TIENDA = AQUI.parent

# Entorno aislado ANTES de importar nada de la tienda: casa, caché, flatpak falso y Flathub falso.
RAIZ = Path(tempfile.mkdtemp(prefix="vortex-shop-integracion-"))
BIN = RAIZ / "bin"
ESTADO = RAIZ / "flatpak"
CONTROL = RAIZ / "control"
for carpeta in (BIN, ESTADO, CONTROL):
    carpeta.mkdir()
shutil.copy(AQUI / "flatpak_falso", BIN / "flatpak")
os.chmod(BIN / "flatpak", 0o755)
shutil.copytree(AQUI / "datos", RAIZ / "datos")
servidor = subprocess.Popen([sys.executable, "-I", str(AQUI / "servidor_falso.py"), str(RAIZ / "datos"),
                             str(CONTROL / "puerto"), str(CONTROL)])
for _ in range(200):
    if (CONTROL / "puerto").exists() and (CONTROL / "puerto").read_text().strip():
        break
    time.sleep(0.025)
PUERTO = int((CONTROL / "puerto").read_text())

os.environ.update({
    "QT_QPA_PLATFORM": "offscreen", "QT_QUICK_BACKEND": "software",
    "VORTEX_SHOP_COLORES": "/nonexistent/qs_colors.json",
    "VORTEX_SHOP_API": f"http://127.0.0.1:{PUERTO}",
    "VORTEX_SHOP_FLATPAK": "cli",                 # sin libflatpak: todo pasa por el flatpak falso
    "FAKE_FLATPAK_DIR": str(ESTADO),
    "HOME": str(RAIZ), "XDG_CACHE_HOME": str(RAIZ / "cache"),
    "PATH": f"{BIN}{os.pathsep}{os.environ['PATH']}",
})
sys.path.insert(0, str(TIENDA))

from PySide6.QtCore import (Q_ARG, QEventLoop, QMetaObject, QPoint, QPointF, Qt, QTimer,  # noqa: E402
                            QtMsgType, qInstallMessageHandler)
from PySide6.QtTest import QTest  # noqa: E402

from vortex_shop.__main__ import _motor_real, cargar_interfaz, crear_aplicacion  # noqa: E402

PRISM = "org.prismlauncher.PrismLauncher"
AVISOS_QT: list[str] = []
FALLOS: list[str] = []


# Qt Quick Controls busca su qtquickcontrols2.conf en la casa de la persona; aquí la casa es temporal.
RUIDO_DE_ENTORNO = ("No file name specified", "No config file path available")


def _mensajes(tipo, contexto, texto):
    if any(r in texto for r in RUIDO_DE_ENTORNO):
        return
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


def llamar(v, nombre, *args):
    QMetaObject.invokeMethod(v, nombre, Qt.ConnectionType.DirectConnection, *[Q_ARG("QVariant", a) for a in args])


def items(v):
    pendientes, todos = [v.contentItem()], []
    while pendientes:   # los hijos visuales de QML no son hijos QObject: se recorre childItems()
        item = pendientes.pop()
        todos.append(item)
        pendientes.extend(item.childItems())
    return todos


def es_boton(item) -> bool:
    return item.metaObject().className().startswith("Boton")


def en_dialogo(item) -> bool:
    """True si el item cuelga de un cuadro de confirmación (Dialogo, z = 100)."""
    while item is not None:
        if item.z() >= 100:
            return True
        item = item.parentItem()
    return False


def boton(v, texto: str):
    """El botón visible y habilitado con ese texto (o None)."""
    for item in items(v):
        if es_boton(item) and item.property("text") == texto and item.isVisible() and item.isEnabled():
            return item
    return None


def clic(v, item) -> None:
    centro = item.mapToScene(QPointF(item.width() / 2, item.height() / 2))
    QTest.mouseClick(v, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier, QPoint(int(centro.x()), int(centro.y())))


def clic_en(v, texto: str, limite_s: float = 10.0) -> bool:
    if not esperar_hasta(lambda: boton(v, texto) is not None, limite_s):
        return False
    esperar(700)   # que termine la animación de la página antes de apuntar
    if boton(v, texto) is None:
        return False
    clic(v, boton(v, texto))
    return True


def textos(v) -> list[str]:
    return [str(i.property("text")) for i in items(v) if i.metaObject().className().startswith("QQuickText")]


def main() -> int:
    qInstallMessageHandler(_mensajes)
    app = crear_aplicacion(sys.argv[:1])
    motor = _motor_real()
    interfaz, puente = cargar_interfaz(app, motor)
    v = interfaz.rootObjects()[0]
    v.resize(1180, 760)
    avisos: list[str] = []
    puente.aviso.connect(avisos.append)
    puente.iniciar()

    # --- Inicio: sale del Flathub falso por la API real del motor ---
    comprobar(esperar_hasta(lambda: puente.inicioEstado == "listo"), "el inicio carga con el catálogo real")
    comprobar(len(puente.recomendadas) > 0, f"hay recomendadas ({len(puente.recomendadas)})")
    comprobar(len(puente.categorias) > 0, f"hay categorías ({len(puente.categorias)})")
    comprobar(esperar_hasta(lambda: (ESTADO / "remotos.txt").exists() and "flathub" in (ESTADO / "remotos.txt").read_text()),
              "al arrancar deja Flathub como fuente del usuario (flatpak remote-add --user)")

    # --- Buscar ---
    llamar(v, "escribirBusqueda", "minecraft")
    comprobar(esperar_hasta(lambda: puente.busquedaEstado == "listo" and len(puente.busquedaApps) > 0), "buscar «minecraft» da resultados")
    comprobar(any(a["id"] == PRISM for a in puente.busquedaApps), "Prism Launcher está en los resultados")
    prism = next((a for a in puente.busquedaApps if a["id"] == PRISM), {})
    comprobar(bool(prism.get("nombre")) and prism.get("icono", "").startswith("http"), "el resultado trae nombre e ícono")
    llamar(v, "escribirBusqueda", "zzzninguna")
    comprobar(esperar_hasta(lambda: puente.busquedaEstado == "listo" and puente.busquedaApps == []), "una búsqueda sin coincidencias queda vacía")
    llamar(v, "escribirBusqueda", "minecraft")
    comprobar(esperar_hasta(lambda: any(a["id"] == PRISM for a in puente.busquedaApps)), "volver a buscar lo anterior funciona")

    # --- Detalle ---
    llamar(v, "verDetalle", PRISM)
    comprobar(esperar_hasta(lambda: puente.detalleEstado == "listo"), "el detalle carga")
    d = puente.detalle
    comprobar(d.get("id") == PRISM and bool(d.get("descripcion")) and len(d.get("capturas", [])) > 0,
              "el detalle trae descripción y capturas")
    comprobar(bool(d.get("licencia")) and bool(d.get("version")) and bool(d.get("tamanoTexto")), "el detalle trae licencia, versión y tamaño")
    esperar(500)
    comprobar(boton(v, "Instalar") is not None, "hay un botón «Instalar»")

    # --- Instalar con un clic y ver el progreso ---
    (ESTADO / "llamadas.log").write_text("")
    comprobar(clic_en(v, "Instalar"), "clic en «Instalar»")
    comprobar(esperar_hasta(lambda: PRISM in puente.estados), "la app pasa a la cola/instalación")
    vistos = set()
    esperar_hasta(lambda: (vistos.add(puente.estados.get(PRISM, {}).get("pct", -1)), PRISM not in puente.estados)[1], 30)
    comprobar(max(vistos) > 0, f"el progreso avanza (porcentajes vistos: {sorted(p for p in vistos if p >= 0)})")
    comprobar(esperar_hasta(lambda: PRISM not in puente.estados and puente.instaladasIds.get(PRISM) is True, 30),
              "al terminar queda instalada")
    comprobar(PRISM in (ESTADO / "usuario.txt").read_text().split(), "flatpak (falso) la instaló en el usuario")
    llamadas = (ESTADO / "llamadas.log").read_text()
    escrituras = [l for l in llamadas.splitlines() if l.split()[:1] in (["install"], ["uninstall"], ["remote-add"], ["update"])]
    comprobar(bool(escrituras) and all("--user" in l and "--system" not in l for l in escrituras),
              f"todo lo que escribe va con «--user» ({escrituras})")
    comprobar(avisos == [], f"sin avisos de error ({avisos})")
    comprobar(any(i["id"] == PRISM for i in puente.instaladas), "aparece en «Instaladas»")

    # --- Instaladas en la interfaz ---
    llamar(v, "navegar", "instaladas", "")
    comprobar(esperar_hasta(lambda: "Prism Launcher" in textos(v)), "la página «Instaladas» muestra la app con su nombre")

    # --- Abrir ---
    llamar(v, "verDetalle", PRISM)
    comprobar(esperar_hasta(lambda: boton(v, "Abrir") is not None), "en el detalle ahora hay «Abrir»")
    comprobar(clic_en(v, "Abrir"), "clic en «Abrir»")
    abierta = esperar_hasta(lambda: (ESTADO / f"abierta-{PRISM}").exists(), 10)
    if not abierta:
        print((ESTADO / "llamadas.log").read_text())
    comprobar(abierta, "flatpak (falso) recibió «run» de la app")
    comprobar(esperar_hasta(lambda: PRISM not in puente.estados, 10), "el botón vuelve a su estado tras abrir")

    # --- Quitar (con confirmación) ---
    comprobar(clic_en(v, "Quitar"), "clic en «Quitar» (en el detalle)")
    comprobar(esperar_hasta(lambda: any("¿Quitar" in t for t in textos(v)), 5), "pide confirmar antes de quitar")
    esperar(400)
    comprobar(PRISM in (ESTADO / "usuario.txt").read_text().split(), "todavía no se quitó nada")
    quitar = [b for b in items(v) if es_boton(b) and b.property("text") == "Quitar" and b.isVisible() and en_dialogo(b)]
    comprobar(len(quitar) == 1, "el diálogo tiene su botón «Quitar»")
    clic(v, quitar[0])
    comprobar(esperar_hasta(lambda: PRISM not in puente.instaladasIds and PRISM not in puente.estados, 30), "se quita")
    comprobar(PRISM not in (ESTADO / "usuario.txt").read_text().split(), "flatpak (falso) ya no la tiene")
    comprobar(esperar_hasta(lambda: boton(v, "Instalar") is not None), "el botón vuelve a ser «Instalar»")
    comprobar(avisos == [], f"sin avisos de error al final ({avisos})")

    # --- Sin red: el motor real lo traduce a un estado de la interfaz ---
    (CONTROL / "modo").write_text("500")
    puente.buscar("nunca antes buscado")
    comprobar(esperar_hasta(lambda: puente.busquedaEstado in ("sinred", "error")), "si Flathub falla, la búsqueda lo dice (no se cuelga)")

    comprobar(AVISOS_QT == [], f"el QML no dejó errores ni avisos ({AVISOS_QT[:3]})")
    print("\nTODO BIEN" if not FALLOS else f"\nFALLÓ ({len(FALLOS)})")
    return 0 if not FALLOS else 1


if __name__ == "__main__":
    try:
        codigo = main()
    finally:
        servidor.terminate()
        shutil.rmtree(RAIZ, ignore_errors=True)
    sys.stdout.flush()
    os._exit(codigo)
