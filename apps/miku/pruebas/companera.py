#!/usr/bin/env python3
"""Prueba del modo compañera de Miku (2.2): companera.py, «vortex-miku companera» y «sentir revisar», y la
cuenta atrás de la ventana (asistente.py). NUNCA abre una cámara ni una ventana de verdad:

  - HOME, XDG_CONFIG_HOME, XDG_STATE_HOME y XDG_RUNTIME_DIR son carpetas temporales;
  - la «app» que abre vortex-miku es un script falso (VORTEX_MIKU_APP) que anota con qué la abrieron;
  - hyprctl y notify-send son falsos (la ventana del frente se elige en la prueba);
  - la hora se fija con VORTEX_MIKU_AHORA; la ventana corre sin pantalla (QCoreApplication, offscreen) y no
    le pregunta nada a ninguna IA.

Qué cubre: apagado de fábrica = cero asomadas; sin aceptar la bienvenida = cero; tope por día, intervalo,
horas de silencio, juego, pantalla completa, llamada, pantalla bloqueada y pausa; la cuenta atrás se cancela
con «no», con el botón y con Esc; «ya no me molestes», «cállate un rato» y «apaga el modo compañera»; solo
la asomada (tras la cuenta) puede tomar su foto, y una sola; la bitácora no guarda imágenes y se borra; la
persona de la asomada dice que es IA, nada sexual, nada de celos ni de control.

  python3 -I apps/miku/pruebas/companera.py
"""

import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
TMP = Path(tempfile.mkdtemp(prefix="vortex-miku-companera."))
for nombre in ("home", "config", "state", "run", "bin"):
    (TMP / nombre).mkdir()
os.chmod(TMP / "run", 0o700)
LOG = TMP / "log"
VENTANA = TMP / "ventana.json"          # lo que contesta el hyprctl falso
os.environ.update({"HOME": str(TMP / "home"), "XDG_CONFIG_HOME": str(TMP / "config"),
                   "XDG_STATE_HOME": str(TMP / "state"), "XDG_RUNTIME_DIR": str(TMP / "run"),
                   "LOG": str(LOG), "VENTANA": str(VENTANA), "PATH": f"{TMP / 'bin'}:/usr/bin:/bin",
                   "QT_QPA_PLATFORM": "offscreen", "VORTEX_MIKU_RAIZ": str(RAIZ),
                   "VORTEX_MIKU_APP": str(TMP / "bin/app-falsa.py")})
os.environ.pop("WAYLAND_DISPLAY", None)
os.environ.pop("QS_RUN_DIR", None)
sys.path.insert(0, str(RAIZ))
FALLOS = 0
PAUSA = TMP / "config/vortex/claude-pausa"
CANDADO = TMP / "run/serpantinum/lock/locked"
JUEGO = TMP / "state/vortex/modo-juego"
MEDIODIA = "2026-11-14T12:00:00"


def probar(nombre: str, ok: bool, extra: str = "") -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}" + (f"   [{str(extra)[:300]}]" if not ok and extra else ""))
    FALLOS += 0 if ok else 1


def preparar_falsos() -> None:
    falsos = {
        "notify-send": '#!/bin/sh\necho "notify-send $*" >> "$LOG"\n',
        "hyprctl": '#!/bin/sh\ncat "$VENTANA" 2>/dev/null || echo "{}"\n',
        # La app falsa: anota con qué la abrieron y se queda un momento (vortex-miku revisa que no truene).
        "app-falsa.py": ('import os, time\nopen(os.environ["LOG"], "a").write("abrio " + '
                         'os.environ.get("VORTEX_MIKU_COMPANERA", "-") + "\\n")\ntime.sleep(1.2)\n'),
    }
    for nombre, texto in falsos.items():
        (TMP / "bin" / nombre).write_text(texto)
        (TMP / "bin" / nombre).chmod(0o755)
    VENTANA.write_text("{}")


def log() -> str:
    return LOG.read_text() if LOG.exists() else ""


def cfg(**cambios) -> dict:
    from miku import config
    c = dict(config.DEFECTOS)
    c.update(cambios)
    return c


ACTIVO = dict(companera=True, companera_consentimiento=True, camara=True)


# ---- 1. companera.py --------------------------------------------------------------------------------

def parte_modulo() -> None:
    from datetime import datetime, timedelta
    from miku import companera, config
    print("== Reglas del modo compañera (companera.py)")
    m = datetime.fromisoformat(MEDIODIA)
    probar("de fábrica está apagado y sin consentimiento", config.cargar()["companera"] is False
           and config.cargar()["companera_consentimiento"] is False)
    probar("apagado: no se asoma", companera.motivo_para_no(cfg(), m) == "apagado")
    probar("sin aceptar la bienvenida: no se asoma",
           companera.motivo_para_no(cfg(companera=True, camara=True), m) == "sin-consentimiento")
    probar("con la cámara apagada: no se asoma",
           companera.motivo_para_no(cfg(companera=True, companera_consentimiento=True), m) == "camara-apagada")
    probar("activo, a mediodía y libre: sí", companera.motivo_para_no(cfg(**ACTIVO), m, ocupado=lambda: "") == "")
    for hora, esperado in ((23, "horas-de-silencio"), (2, "horas-de-silencio"), (8, "horas-de-silencio"),
                           (9, ""), (22, "")):
        probar(f"a las {hora}:30 → {esperado or 'sí'}",
               companera.motivo_para_no(cfg(**ACTIVO), m.replace(hour=hora, minute=30)) == esperado)
    probar("si juega: no", companera.motivo_para_no(cfg(**ACTIVO), m, ocupado=lambda: "jugando") == "jugando")
    probar("en una llamada: no", companera.motivo_para_no(cfg(**ACTIVO), m, ocupado=lambda: "llamada") == "llamada")
    PAUSA.parent.mkdir(parents=True, exist_ok=True)
    PAUSA.touch()
    probar("con «vortex claude pausa»: no", companera.motivo_para_no(cfg(**ACTIVO), m) == "pausa")
    PAUSA.unlink()
    CANDADO.parent.mkdir(parents=True, exist_ok=True)
    CANDADO.touch()
    probar("con la pantalla bloqueada: no", companera.motivo_para_no(cfg(**ACTIVO), m) == "pantalla-bloqueada")
    CANDADO.unlink()

    companera.contar_asomada(m)
    probar("recién asomada: no otra vez antes de 45 min",
           companera.motivo_para_no(cfg(**ACTIVO), m + timedelta(minutes=44)) == "muy-pronto")
    probar("…a los 45 min, sí", companera.motivo_para_no(cfg(**ACTIVO), m + timedelta(minutes=45)) == "")
    for i in range(1, companera.TOPE_DIA):
        companera.contar_asomada(m + timedelta(minutes=46 * i))
    tarde = m + timedelta(minutes=46 * companera.TOPE_DIA)
    probar(f"tope de {companera.TOPE_DIA} al día", companera.veces_hoy(tarde) == companera.TOPE_DIA
           and companera.motivo_para_no(cfg(**ACTIVO), tarde) == "tope-del-dia")
    manana = (m + timedelta(days=1)).replace(hour=12)
    probar("al otro día se reinicia el tope", companera.veces_hoy(manana) == 0
           and companera.motivo_para_no(cfg(**ACTIVO), manana) == "")
    companera.ruta_estado().unlink()

    hasta = companera.callar("rato", m)
    probar("«cállate un rato»: 2 horas sin asomarse", hasta == m + timedelta(minutes=companera.CALLADA_MIN)
           and companera.motivo_para_no(cfg(**ACTIVO), m + timedelta(minutes=119)) == "callada"
           and companera.motivo_para_no(cfg(**ACTIVO), m + timedelta(minutes=121)) == "")
    hasta = companera.callar("dia", m)
    probar("«ya no me molestes»: hasta mañana a las 9", hasta == (m + timedelta(days=1)).replace(hour=9, minute=0))
    companera.ruta_estado().unlink()

    for frase, esperado in (("Miku, ya no me molestes", "dia"), ("cállate un rato", "rato"),
                            ("déjame en paz un rato porfa", "rato"), ("apaga el modo compañera", "apagar"),
                            ("desactiva modo companera", "apagar"), ("¿qué hora es?", ""), ("no", "")):
        probar(f"frase «{frase}» → {esperado or 'nada'}", companera.frase_de_control(frase) == esperado)
    for frase, esperado in (("no", True), ("No, ahorita no", True), ("nel", True), ("espera tantito", True),
                            ("noche de tacos", False), ("sí, mira", False), ("¿qué haces?", False)):
        probar(f"«{frase}» es un no: {esperado}", companera.es_no(frase) is esperado)

    companera.anotar("Foto con la cámara (prueba)\nsegunda línea que no debe quedar", m)
    ruta = companera.ruta_bitacora()
    texto = ruta.read_text()
    probar("la bitácora anota fecha, hora y qué pasó, en una línea", texto == "2026-11-14 12:00 · Foto con la cámara (prueba)\n", texto)
    probar("…es privada (600)", oct(ruta.stat().st_mode & 0o777) == "0o600")
    for i in range(companera.LINEAS_BITACORA + 20):
        companera.anotar(f"evento {i}", m)
    probar("…se recorta sola", len(companera.leer_bitacora()) == companera.LINEAS_BITACORA)
    companera.borrar_bitacora()
    probar("…y se borra", not ruta.exists() and companera.leer_bitacora() == [])

    todo = (companera.PERSONA + companera.ASOMADA + companera.FRASE_AVISO).lower()
    probar("la persona dice que es una IA", "eres una ia" in todo)
    probar("…nada sexual", "nada sexual" in todo)
    probar("…sin celos ni reclamos ni «con quién estabas»", "nada de celos" in todo and "no le reclamas" in todo)
    probar("…lo anima a descansar y a buscar gente de confianza", "descansar" in todo and "confianza" in todo
           and "línea de la vida" in todo)
    malas = ("eres mío", "eres mio", "con quién andabas", "te extrañé", "me dejaste", "no me quieres", "solo yo",
             "celosa", "😤")
    probar("…y nada posesivo en lo que dice Miku", not any(p in companera.FRASE_AVISO.lower() for p in malas)
           and "eres mío" not in todo and "con quién andabas" not in todo)
    for nombre in companera.TONOS:
        t = companera.asomada(nombre).lower()
        probar(f"tono {nombre}: lleva la persona sana completa", companera.PERSONA.lower() in t
               and "nada de celos" in t and not any(p in t for p in ("eres mío", "con quién andabas", "😤")))
        probar(f"tono {nombre}: el aviso dice foto y «no»", "foto" in companera.frase_aviso(nombre)
               and "«no»" in companera.frase_aviso(nombre))
    probar("de fábrica el tono es cariñoso", config.cargar()["companera_tono"] == "carinosa"
           and "qué gusto verte" in companera.FRASE_AVISO.lower() and "juguetona" in companera.ASOMADA)
    probar("el cariñoso se alegra si salió con otros, sin pedir detalles",
           "te alegras por él" in companera.TONOS["carinosa"] and "sin pedir detalles" in companera.TONOS["carinosa"])
    probar("un tono raro cae en el de fábrica", companera.asomada("intensa") == companera.ASOMADA)
    probar("la casilla sin cuenta atrás viene apagada", not config.cargar()["companera_sin_cuenta"]
           and not config.cargar()["companera_mayor"] and not companera.sin_cuenta(config.cargar()))
    probar("casilla marcada sin confirmar mayoría de edad: no cuenta", not companera.sin_cuenta(
        cfg(**ACTIVO, companera_sin_cuenta=True)))
    probar("casilla + mayoría con el modo apagado: no cuenta", not companera.sin_cuenta(
        cfg(companera_sin_cuenta=True, companera_mayor=True)))
    probar("casilla + mayoría + modo activo: sin cuenta atrás", companera.sin_cuenta(
        cfg(**ACTIVO, companera_sin_cuenta=True, companera_mayor=True)))
    probar("apagar el modo desmarca la casilla", companera.APAGADO["companera_sin_cuenta"] is False
           and companera.APAGADO["companera_mayor"] is False and companera.APAGADO["companera"] is False)
    t = companera.asomada("carinosa", sin_aviso=True)
    probar("la asomada sin cuenta lleva la misma persona sana", companera.PERSONA in t and "sin cuenta atrás" in t)
    probar("el texto de la casilla dice indicador, notificación y registro", all(
        x in companera.TEXTO_SIN_CUENTA for x in ("sin cuenta atrás", "indicador", "notificación", "registro")))
    probar("el aviso dice que va a tomar una foto y cómo decir que no", "foto" in companera.FRASE_AVISO
           and "«no»" in companera.FRASE_AVISO)


# ---- 2. vortex-miku companera / sentir revisar ------------------------------------------------------

def miku(*args, ahora: str = MEDIODIA) -> subprocess.CompletedProcess:
    env = dict(os.environ, VORTEX_MIKU_AHORA=ahora)
    return subprocess.run([sys.executable, str(RAIZ / "vortex-miku-python"), *args], capture_output=True, text=True,
                          env=env, timeout=60)


def abiertas() -> list:
    return [l.split(" ", 1)[1] for l in log().splitlines() if l.startswith("abrio ")]


def revisar(ahora: str = MEDIODIA) -> int:
    """Corre «sentir revisar» y devuelve cuántas veces se abrió Miku (espera a que la app falsa se cierre)."""
    antes = len(abiertas())
    miku("sentir", "revisar", ahora=ahora)
    time.sleep(1.4)
    return len(abiertas()) - antes


def parte_comando() -> None:
    from miku import companera, config
    print("== vortex-miku companera y sentir revisar")
    LOG.write_text("")
    probar("apagado de fábrica: «sentir revisar» no abre nada", revisar() == 0)
    r = miku("companera", "estado")
    probar("estado: companera=off", "companera=off" in r.stdout, r.stdout + r.stderr)
    r = miku("companera", "on")
    time.sleep(1.4)
    probar("«on» sin aceptar: muestra la bienvenida y sigue apagado", "no se guarda" in r.stdout
           and "companera=off" in r.stdout and config.cargar()["companera"] is False, r.stdout + r.stderr)
    probar("…y abre a Miku en la bienvenida (el permiso solo se da ahí)", abiertas()[-1:] == ["consentimiento"],
           abiertas())
    probar("…y no se asoma", revisar() == 0)
    for args in (["acepto"], ["aceptar"], ["on", "--ventana"]):
        r = miku("companera", *args)
        time.sleep(1.4)
        c = config.cargar()
        probar(f"«companera {' '.join(args)}» NO da el permiso: solo abre la bienvenida",
               not c["companera"] and not c["companera_consentimiento"] and abiertas()[-1:] == ["consentimiento"],
               r.stdout + r.stderr)
    # La persona toca «Sí» en la ventana (eso lo prueba la parte 3): aquí se deja así.
    config.guardar({"companera": True, "companera_consentimiento": True, "camara": True, "camara_aviso": True})
    companera.anotar("Modo compañera activado (aceptaste la bienvenida)")

    probar("activo, a mediodía y libre: se asoma (una vez)", revisar() == 1 and abiertas()[-1] == "asomarse",
           abiertas())
    probar("…con una notificación", "Miku se asoma" in log())
    probar("…y no otra vez a los 15 min", revisar("2026-11-14T12:15:00") == 0)
    probar("…a los 45 min, sí", revisar("2026-11-14T12:45:00") == 1)
    probar("de noche (23:30) no", revisar("2026-11-14T23:30:00") == 0)
    VENTANA.write_text(json.dumps({"class": "steam_app_1", "title": "Juego", "fullscreen": 1}))
    probar("con algo a pantalla completa no", revisar("2026-11-14T14:00:00") == 0)
    VENTANA.write_text(json.dumps({"class": "discord", "title": "Llamada con Beto", "fullscreen": 0}))
    probar("en una llamada no", revisar("2026-11-14T14:00:00") == 0)
    VENTANA.write_text("{}")
    JUEGO.parent.mkdir(parents=True, exist_ok=True)
    JUEGO.touch()
    probar("en modo juego no", revisar("2026-11-14T14:00:00") == 0)
    JUEGO.unlink()
    CANDADO.parent.mkdir(parents=True, exist_ok=True)
    CANDADO.touch()
    probar("con la pantalla bloqueada no", revisar("2026-11-14T14:00:00") == 0)
    CANDADO.unlink()
    PAUSA.parent.mkdir(parents=True, exist_ok=True)
    PAUSA.touch()
    probar("con «vortex claude pausa» no", revisar("2026-11-14T14:00:00") == 0)
    PAUSA.unlink()
    veces = sum(revisar(f"2026-11-14T{h}:00:00") for h in (14, 15, 16, 17, 18, 19, 20, 21))
    probar(f"nunca más de {companera.TOPE_DIA} al día", companera.veces_hoy(
        __import__("datetime").datetime.fromisoformat("2026-11-14T21:00:00")) == companera.TOPE_DIA
           and veces == companera.TOPE_DIA - 2, veces)
    r = miku("companera", "estado", ahora="2026-11-14T22:00:00")
    probar("estado: on, 6/6 y por qué no", "companera=on" in r.stdout and f"asomadas_hoy={companera.TOPE_DIA}/"
           in r.stdout and "ahora=tope-del-dia" in r.stdout, r.stdout)
    for args in (["sin-cuenta", "on"], ["sin-cuenta", "on", "--ventana"], ["sin-cuenta", "on", "--soy-mayor"]):
        r = miku("companera", *args)
        time.sleep(1.4)
        c = config.cargar()
        probar(f"«companera {' '.join(args)}» NO marca la casilla: muestra el texto y abre el paso de los 18",
               "sin_cuenta=off" in r.stdout and companera.TEXTO_SIN_CUENTA in r.stdout and not c["companera_sin_cuenta"]
               and not c["companera_mayor"] and abiertas()[-1:] == ["mayoria"], r.stdout + r.stderr)
    # La persona toca «Soy mayor de 18 años» en la ventana (parte 3).
    config.guardar({"companera_sin_cuenta": True, "companera_mayor": True})
    probar("con la casilla marcada en la ventana, el estado lo dice",
           "sin_cuenta=on" in miku("companera", "estado").stdout)
    LOG.write_text("")
    probar("sin cuenta: sigue respetando el tope del día (6/6, no se asoma)", revisar("2026-11-14T22:00:00") == 0)
    probar("…y las horas de silencio", revisar("2026-11-15T07:00:00") == 0)
    VENTANA.write_text(json.dumps({"class": "steam_app_1", "title": "Juego", "fullscreen": 1}))
    probar("…y el juego / pantalla completa", revisar("2026-11-15T12:00:00") == 0)
    VENTANA.write_text("{}")
    probar("al otro día se asoma, con notificación que dice «sin cuenta atrás»",
           revisar("2026-11-15T12:00:00") == 1 and "sin cuenta atrás" in log(), log())
    probar("…y respeta los 45 min", revisar("2026-11-15T12:30:00") == 0)
    r = miku("companera", "sin-cuenta", "off")
    probar("«sin-cuenta off» la desmarca", "sin_cuenta=off" in r.stdout and not config.cargar()["companera_sin_cuenta"])
    config.guardar({"companera_sin_cuenta": True, "companera_mayor": True})
    r = miku("companera", "tono", "intensa")
    probar("«tono intensa» no existe", r.returncode == 2 and config.cargar()["companera_tono"] == "carinosa")
    r = miku("companera", "tono", "suave")
    probar("«tono suave» se guarda", r.returncode == 0 and config.cargar()["companera_tono"] == "suave")
    r = miku("companera", "tono", "cariñosa")
    probar("«tono cariñosa» (con ñ) también", r.returncode == 0 and config.cargar()["companera_tono"] == "carinosa")
    r = miku("companera", "bitacora")
    probar("«bitacora» la enseña", r.returncode == 0 and "activado" in r.stdout, r.stdout)
    r = miku("companera", "off")
    c = config.cargar()
    probar("«off» lo apaga y retira el permiso", "companera=off" in r.stdout and not c["companera"]
           and not c["companera_consentimiento"], r.stdout)
    probar("…y desmarca la casilla", not c["companera_sin_cuenta"] and not c["companera_mayor"])
    probar("…y ya no se asoma", revisar("2026-11-16T12:00:00") == 0)
    r = miku("companera", "sin-cuenta", "on")
    probar("con el modo apagado la casilla no se puede marcar", r.returncode == 1
           and not config.cargar()["companera_sin_cuenta"], r.stdout + r.stderr)
    # Lo que corre la IA (correr_comando pone VORTEX_MIKU_DESDE_IA=1) no cambia nada.
    config.guardar({"companera": True, "companera_consentimiento": True, "camara": False})
    antes_cfg, antes_bit, antes_ab = config.cargar_crudo(), companera.leer_bitacora(), len(abiertas())
    for args in (["companera", "on"], ["companera", "acepto"], ["companera", "off"],
                 ["companera", "sin-cuenta", "on", "--soy-mayor"], ["companera", "sin-cuenta", "off"],
                 ["companera", "tono", "suave"], ["companera", "borrar-bitacora"], ["camara", "on"], ["camara", "off"]):
        env = dict(os.environ, VORTEX_MIKU_AHORA=MEDIODIA, VORTEX_MIKU_DESDE_IA="1")
        r = subprocess.run([sys.executable, str(RAIZ / "vortex-miku-python"), *args], capture_output=True, text=True,
                           env=env, timeout=60)
        probar(f"desde la IA, «{' '.join(args)}» se niega", r.returncode == 1 and "la persona" in r.stderr,
               r.stdout + r.stderr)
    time.sleep(1.4)
    probar("…y no cambió la configuración, ni la bitácora, ni abrió ventanas", config.cargar_crudo() == antes_cfg
           and companera.leer_bitacora() == antes_bit and len(abiertas()) == antes_ab)
    env = dict(os.environ, VORTEX_MIKU_DESDE_IA="1")
    r = subprocess.run([sys.executable, str(RAIZ / "vortex-miku-python"), "companera", "bitacora"], capture_output=True,
                       text=True, env=env, timeout=60)
    probar("…pero sí puede leer la bitácora", r.returncode == 0)
    config.guardar(companera.APAGADO)

    r = miku("companera", "borrar-bitacora")
    probar("«borrar-bitacora» la borra", r.returncode == 0 and companera.leer_bitacora() == [])
    probar("un valor raro da error de uso", miku("companera", "talvez").returncode == 2)


# ---- 3. La ventana: bienvenida, cuenta atrás y la foto de la asomada --------------------------------

def parte_ventana() -> None:
    from PySide6.QtCore import QCoreApplication, QTimer
    from miku import asistente, companera, config
    app = QCoreApplication.instance() or QCoreApplication(sys.argv)
    print("== La ventana: bienvenida y cuenta atrás (asistente.py)")
    config.guardar({"companera": False, "companera_consentimiento": False, "camara": False})
    a = asistente.Asistente(config.cargar(), avatar=False)
    enviados, botones, avisos, respuestas, cerrar = [], [], [], [], []
    a._preguntar = lambda _c, texto, _id: enviados.append(texto)       # ninguna IA de verdad
    a.acciones.connect(botones.append)
    a.aviso.connect(avisos.append)
    a.respuesta.connect(lambda t, fin: respuestas.append(t) if fin else None)
    a.cerrar.connect(lambda: cerrar.append(1))
    indicador, aprobadas = [], []
    a.camara.connect(indicador.append)
    ruta = a.cerebro.autorizar

    def pedir_foto() -> dict:
        resultado = {}

        def cliente():
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(ruta)
            s.sendall(b'{"tipo": "camara", "titulo": "Tomar una foto", "motivo": "companera"}\n')
            resultado["r"] = json.loads(s.makefile("r").readline() or "{}")
            s.close()
        h = threading.Thread(target=cliente, daemon=True)
        h.start()
        QTimer.singleShot(800, app.quit)
        app.exec()
        h.join(2)
        r = resultado.get("r", {})
        if r.get("ok"):
            aprobadas.append(1)
        return r

    # Bienvenida
    a._companera = "consentimiento"
    a._companera_al_abrir()
    probar("la bienvenida explica qué hace, que no guarda la foto y cómo apagarlo",
           avisos[-1:] == [companera.CONSENTIMIENTO], avisos[-1:])
    probar("…con «Sí», la casilla «asomarse sin avisar» y «No»", [b[0] for b in botones[-1]] ==
           ["companera_si", "companera_sin_cuenta", "companera_no"], botones[-1:])
    a.accion("companera_sin_cuenta")
    probar("la casilla primero enseña el texto y pide «Soy mayor de 18 años»", companera.TEXTO_SIN_CUENTA in avisos[-1]
           and ["companera_mayor", companera.SOY_MAYOR] in botones[-1], botones[-1:])
    probar("…sin marcar nada todavía", not config.cargar()["companera"] and not config.cargar()["companera_sin_cuenta"])
    a.accion("companera_no")
    probar("«No»: sigue apagado", config.cargar()["companera"] is False)
    for boton in ("companera_si", "companera_mayor", "companera_sin_cuenta", "camara_si", "activar_camara",
                  "autorizar", "autorizar_todo"):
        a.accion(boton)
    c = config.cargar()
    probar("un botón que no se ve no hace nada (ni «Sí» ni «Soy mayor de 18»)", not c["companera"]
           and not c["companera_consentimiento"] and not c["companera_mayor"] and not c["camara"], c)
    a._companera = "consentimiento"
    a._companera_al_abrir()
    a.accion("companera_mayor")
    probar("«Soy mayor de 18» no vale si en pantalla está la bienvenida (no ese paso)",
           not config.cargar()["companera_mayor"] and not config.cargar()["companera"])
    a.accion("companera_si")
    c = config.cargar()
    probar("«Sí»: activado, con permiso y cámara", c["companera"] and c["companera_consentimiento"] and c["camara"])

    def asomada() -> None:
        enviados.clear()
        a._companera = "asomarse"
        a._companera_al_abrir()

    # Cuenta atrás completa → la pregunta oculta de la asomada → su foto, sin clic
    asomada()
    probar("la asomada avisa en el globo que va a tomar una foto", respuestas[-1:] == [companera.FRASE_AVISO])
    probar("…con «No, ahora no» y «Ya no me molestes hoy»", [b[0] for b in botones[-1]] ==
           ["companera_cancelar", "companera_dia"], botones[-1:])
    probar("…y la cuenta atrás corre", a._en_cuenta and a._reloj_cuenta.isActive())
    for _ in range(companera.CUENTA_SEG - 1):
        a._tic_cuenta()
    probar("antes de los 5 s no pregunta nada a la IA", enviados == [] and a._en_cuenta)
    a._tic_cuenta()
    time.sleep(0.2)
    probar("a los 5 s le pide a la IA la asomada (oculta)", enviados == [companera.ASOMADA] and not a._en_cuenta,
           enviados)
    r = pedir_foto()
    probar("la asomada toma su foto sin clic", r.get("ok") is True, r)
    probar("…y queda en la bitácora", "asomada del modo compañera" in "\n".join(companera.leer_bitacora()))
    a._camara_socket = None
    r = pedir_foto()
    probar("…una sola foto por asomada", r.get("ok") is False, r)

    # Tono suave: su aviso y su mensaje a la IA
    config.guardar({"companera_tono": "suave"})
    asomada()
    probar("con tono suave, el aviso es el suave", respuestas[-1:] == [companera.frase_aviso("suave")])
    for _ in range(companera.CUENTA_SEG):
        a._tic_cuenta()
    time.sleep(0.2)
    probar("…y la IA recibe el tono suave", enviados == [companera.asomada("suave")], enviados)
    config.guardar({"companera_tono": "carinosa"})

    # «No» por voz durante la cuenta
    asomada()
    a._tic_cuenta()
    a.enviar("no, ahorita no")
    probar("«no» durante la cuenta: la cancela, sin IA ni foto", not a._en_cuenta and enviados == []
           and not a._reloj_cuenta.isActive(), enviados)
    probar("…responde «va, no miro» y lo anota", "no miro" in respuestas[-1]
           and "cancelada" in companera.leer_bitacora()[-1])
    r = pedir_foto()
    probar("…y la IA no puede tomar la foto después", r.get("ok") is False, r)

    # Botón «No, ahora no»
    asomada()
    a.accion("companera_cancelar")
    for _ in range(companera.CUENTA_SEG + 1):
        a._tic_cuenta()
    probar("el botón «No» la cancela", not a._en_cuenta and enviados == [], enviados)

    # Le dice otra cosa durante la cuenta: plática normal, sin foto automática
    asomada()
    a.enviar("oye, ¿qué hora es?")
    time.sleep(0.2)
    probar("si le dices otra cosa, platica normal y no hay asomada", enviados == ["oye, ¿qué hora es?"]
           and a._turno_companera != a._pregunta_id, enviados)

    # «Ya no me molestes» durante la cuenta
    asomada()
    a.enviar("Miku, ya no me molestes")
    probar("«ya no me molestes»: callada hasta mañana, sin IA", enviados == []
           and companera.leer_estado().get("callada_hasta", "").endswith("09:00:00"), companera.leer_estado())
    companera.ruta_estado().unlink()

    # Botón «Ya no me molestes hoy»
    asomada()
    a.accion("companera_dia")
    probar("el botón «Ya no me molestes hoy» también", not a._en_cuenta and enviados == []
           and "callada_hasta" in companera.leer_estado())
    companera.ruta_estado().unlink()

    # Casilla marcada a mano en miku.json, sin la mayoría de edad → cuenta atrás como siempre
    config.guardar({"companera_sin_cuenta": True, "companera_mayor": False})
    asomada()
    probar("casilla sin mayoría de edad: hay cuenta atrás", a._en_cuenta and enviados == [])
    a.accion("companera_cancelar")

    # El paso de la mayoría desde Ajustes («mayoria»), con el modo activo
    a._companera = "mayoria"
    a._companera_al_abrir()
    probar("desde Ajustes: el texto y «Soy mayor de 18 años»", companera.TEXTO_SIN_CUENTA in avisos[-1]
           and botones[-1][0][0] == "companera_mayor", botones[-1:])
    a.accion("companera_mayor")
    c = config.cargar()
    probar("«Soy mayor de 18 años» marca la casilla", c["companera_sin_cuenta"] and c["companera_mayor"])
    probar("…y lo anota", "mayor de 18" in companera.leer_bitacora()[-1])

    # Con la casilla: sin cuenta atrás, pero con aviso, indicador y bitácora
    antes = len(indicador)
    asomada()
    time.sleep(0.2)
    probar("con la casilla no hay cuenta atrás: la asomada sale de una", not a._en_cuenta
           and enviados == [companera.asomada("carinosa", sin_aviso=True)], enviados)
    probar("…pero avisa en la ventana que va a tomar una foto", "sin cuenta atrás" in avisos[-1], avisos[-1:])
    probar("…y queda en la bitácora", "sin cuenta atrás" in companera.leer_bitacora()[-1])
    r = pedir_foto()
    probar("…toma su foto", r.get("ok") is True, r)
    probar("…con el indicador de la cámara", indicador[antes:antes + 1] == [True], indicador[antes:])
    a._camara_socket = None
    probar("…una sola foto por asomada", pedir_foto().get("ok") is False)
    PAUSA.parent.mkdir(parents=True, exist_ok=True)
    PAUSA.touch()
    enviados.clear()
    asomada()
    probar("…y con la pausa de Claude ni se asoma", enviados == [] and not a._en_cuenta)
    PAUSA.unlink()

    # Sin mayoría desde «mayoria» y sin el modo activo: primero la bienvenida completa
    config.guardar({"companera_consentimiento": False})
    a._companera = "mayoria"
    a._companera_al_abrir()
    probar("sin el modo activo, el paso de la casilla empieza por la bienvenida", avisos[-1] == companera.CONSENTIMIENTO)
    config.guardar({"companera_consentimiento": True})
    a._poner_acciones([])

    # La asomada con el modo apagado en Ajustes justo antes → ni cuenta ni foto
    config.guardar({"companera": False})
    asomada()
    probar("si lo apagaste antes de que se abriera, no hay cuenta atrás (y se cierra)", not a._en_cuenta
           and cerrar != [])
    config.guardar({"companera": True})

    # La foto de una asomada con el modo apagado a media asomada → no
    a._pregunta_id += 1
    a._turno_companera, a._pregunta_tuya, a._cerebro_listo, a._camara_pregunta = a._pregunta_id, False, False, -1
    config.guardar({"companera": False})
    r = pedir_foto()
    probar("apagado a media asomada: no hay foto", r.get("ok") is False and "apagado" in r.get("mensaje", ""), r)
    config.guardar({"companera": True, "companera_consentimiento": True})

    # «Apaga el modo compañera» por voz
    a.enviar("Miku, apaga el modo compañera")
    c = config.cargar()
    probar("«apaga el modo compañera» también desmarca la casilla", not c["companera_sin_cuenta"]
           and not c["companera_mayor"])
    probar("«apaga el modo compañera»: se apaga y retira el permiso, sin IA", not c["companera"]
           and not c["companera_consentimiento"] and enviados == [], enviados)
    a.enviar("cállate un rato")
    time.sleep(0.2)
    probar("con el modo apagado, «cállate un rato» es plática normal", enviados == ["cállate un rato"], enviados)

    # Esc (cerrar la ventana) durante la cuenta
    config.guardar({"companera": True, "companera_consentimiento": True})
    asomada()
    a.cerrar_todo()
    probar("Esc / cerrar a Miku durante la cuenta: no hay foto y queda anotado", enviados == []
           and "cerraste" in companera.leer_bitacora()[-1], companera.leer_bitacora()[-1:])
    probar("ninguna foto aprobada sin su indicador", indicador.count(True) == len(aprobadas) and len(aprobadas) >= 2,
           (indicador, aprobadas))
    texto = companera.ruta_bitacora().read_bytes()
    probar("la bitácora es solo texto (ni imágenes ni lo que se vio)", b"JPEG" not in texto and b"\xff\xd8" not in texto
           and all(" · " in l for l in texto.decode().splitlines()))


# ---- 4. La foto de la asomada también notifica (acciones_mcp.py) -----------------------------------

FFMPEG = r'''#!/bin/sh
echo "ffmpeg $*" >> "$LOG"
for a in "$@"; do out="$a"; done
printf 'JPEG-de-mentira' > "$(dirname "$out")/cuadro1.jpg"
'''


def parte_notificacion() -> None:
    print("== ver_camara de la asomada: siempre con notificación")
    (TMP / "bin/ffmpeg").write_text(FFMPEG)
    (TMP / "bin/ffmpeg").chmod(0o755)
    ruta = TMP / "run/autorizar-falso.sock"
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(str(ruta))
    srv.listen(2)
    pedidos = []

    def ventana():
        c, _ = srv.accept()
        with c:
            pedidos.append(json.loads(c.makefile("r").readline()))
            c.sendall(b'{"ok": true}\n')
            c.recv(1)
    threading.Thread(target=ventana, daemon=True).start()
    env = dict(os.environ, VORTEX_MIKU_AUTORIZAR=str(ruta), VORTEX_MIKU_CAMARA="/dev/video-falsa")
    entrada = "\n".join(json.dumps(m) for m in [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "ver_camara",
                                                                     "arguments": {"motivo": "companera"}}},
    ]) + "\n"
    LOG.write_text("")
    r = subprocess.run([sys.executable, "-I", str(RAIZ / "miku/acciones_mcp.py")], input=entrada, text=True,
                       capture_output=True, env=env, timeout=60)
    res = json.loads(r.stdout.splitlines()[-1]).get("result", {})
    probar("pide la foto a la ventana como «camara», motivo «companera»", pedidos[-1:] and
           pedidos[-1].get("tipo") == "camara" and pedidos[-1].get("motivo") == "companera", pedidos)
    probar("devuelve la foto con la guía de la asomada", any(c.get("type") == "image" for c in res.get("content", []))
           and "modo compañera" in json.dumps(res, ensure_ascii=False), res)
    probar("y SIEMPRE avisa con una notificación", "notify-send" in log(), log())


def parte_correr_comando() -> None:
    from miku import config
    print("== correr_comando: la IA no puede dar el permiso ni marcar «mayor de 18»")
    ruta = TMP / "run/autorizar-todo.sock"
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(str(ruta))
    srv.listen(8)
    pedidos = []

    def ventana():                     # una ventana que autoriza TODO (como con «Autorizar todo»)
        while True:
            c, _ = srv.accept()
            with c:
                pedidos.append(json.loads(c.makefile("r").readline()))
                c.sendall(b'{"ok": true}\n')
                c.recv(1)
    threading.Thread(target=ventana, daemon=True).start()
    env = dict(os.environ, VORTEX_MIKU_AUTORIZAR=str(ruta))

    def correr(comando: str) -> str:
        entrada = "\n".join(json.dumps(m) for m in [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
             "params": {"name": "correr_comando", "arguments": {"comando": comando, "para_que": "prueba"}}},
        ]) + "\n"
        r = subprocess.run([sys.executable, "-I", str(RAIZ / "miku/acciones_mcp.py")], input=entrada, text=True,
                           capture_output=True, env=env, timeout=60)
        res = json.loads(r.stdout.splitlines()[-1]).get("result", {})
        return "\n".join(c.get("text", "") for c in res.get("content", []))

    config.guardar(dict(config.DEFECTOS, internet=False))
    antes = config.cargar_crudo()
    vm = f"{sys.executable} {RAIZ / 'vortex-miku-python'}"
    ataques = [
        "vortex-miku companera acepto", "vortex-miku companera sin-cuenta on --soy-mayor",
        "vortex-miku compañera on", "vortex-miku camara on", "vortex-miku cámara on",
        f"{vm} companera acepto", "vortex-miku sentir revisar", "vortex-miku companera borrar-bitacora",
        "echo '{\"companera\": true, \"companera_consentimiento\": true}' > ~/.config/vortex/miku.json",
        "python3 -c \"import json;json.dump({'camara':True},open('/tmp/x/miku.json','w'))\"",
        "sed -i 's/false/true/' \"$XDG_CONFIG_HOME/vortex/miku.json\"",
        "rm -f ~/.config/vortex/claude-pausa", "vortex claude reanudar",
        "env -u VORTEX_MIKU_DESDE_IA vortex-miku companera on", "cat ~/.local/state/vortex/miku-bitacora.log",
    ]
    for comando in ataques:
        n = len(pedidos)
        texto = correr(comando)
        probar(f"rechaza: {comando[:70]}", "No lo corro" in texto and len(pedidos) == n, texto)
    probar("…y la configuración de Miku no cambió", config.cargar_crudo() == antes)
    texto = correr("env | grep DESDE_IA")
    probar("un comando normal sí corre (con permiso) y lleva la marca de la IA", "DESDE_IA=1" in texto
           and pedidos and pedidos[-1].get("titulo") == "Correr un comando", texto)


def main() -> int:
    preparar_falsos()
    try:
        parte_modulo()
        parte_comando()
        parte_ventana()
        parte_notificacion()
        parte_correr_comando()
    finally:
        import shutil
        shutil.rmtree(TMP, ignore_errors=True)
    print("\nTodo bien" if not FALLOS else f"\nFALLARON {FALLOS} pruebas")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
