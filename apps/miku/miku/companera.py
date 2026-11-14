"""Modo compañera de Miku (2.2): de vez en cuando se asoma por su cuenta, avisa, cuenta 5 s y toma UNA foto.

Todo lo que la frena está aquí, en código (no en lo que se le pide a la IA):
  - Viene APAGADO («companera» en miku.json) y no hace nada sin el consentimiento de la pantalla de
    bienvenida («companera_consentimiento»). Necesita además la cámara encendida.
  - Como mucho cada INTERVALO_MIN minutos y TOPE_DIA veces al día; nunca entre las 23:00 y las 9:00.
  - Nunca si juegas, si hay una ventana a pantalla completa o una llamada, si la pantalla está bloqueada o si
    «vortex claude pausa» está puesta.
  - «Ya no me molestes» la calla hasta mañana; «cállate un rato», CALLADA_MIN minutos; «apaga el modo
    compañera» lo apaga.
  - Cada foto y cada asomada quedan en una bitácora local (fecha, hora y qué pasó; nunca imágenes) que se ve
    con «vortex-miku companera bitacora» y se borra con «vortex-miku companera borrar-bitacora».

Solo biblioteca estándar: lo usan vortex-miku y la ventana.
"""

import json
import os
import re
from datetime import datetime, timedelta
from pathlib import Path

INTERVALO_MIN = 45
TOPE_DIA = 6
CUENTA_SEG = 5               # la cuenta atrás antes de la foto (se cancela con «no», el botón o Esc)
CALLADA_MIN = 120            # «cállate un rato»
SILENCIO_DESDE = 23          # horas de silencio: de las 23:00…
SILENCIO_HASTA = 9           # …a las 9:00
LINEAS_BITACORA = 500        # la bitácora se recorta sola

CONSENTIMIENTO = (
    "Modo compañera: de vez en cuando (como mucho cada 45 minutos y 6 veces al día, nunca de 23:00 a 9:00) "
    "Miku se asoma por su cuenta: te habla y, si no le dices que no en 5 segundos, toma UNA foto con la cámara "
    "para ver cómo vas.\n"
    "• La foto se manda solo a la IA que uses (Claude por internet, o tu IA local) y no se guarda.\n"
    "• Siempre vas a ver y oír el aviso, la cuenta atrás y el indicador de la cámara. Nunca graba video.\n"
    "• Nunca se asoma si juegas, si estás en una llamada, con algo a pantalla completa, con la pantalla "
    "bloqueada o con «vortex claude pausa».\n"
    "• Para que no se asome: di «no», toca «No» o pulsa Esc. «Ya no me molestes» la calla hasta mañana, "
    "«cállate un rato» por 2 horas y «apaga el modo compañera» lo apaga.\n"
    "• Cada foto queda anotada (solo fecha y hora, sin imagen): «vortex-miku companera bitacora» para verla y "
    "«vortex-miku companera borrar-bitacora» para borrarla. Se apaga en Ajustes → General → Asistente o con "
    "«vortex-miku companera off»."
)

# Cómo es Miku cuando se asoma. Va en el mensaje de cada asomada (la persona de miku.txt no se toca).
PERSONA = (
    "Modo compañera: eres una compañera cálida y divertida, como una buena amiga. Reglas que no se rompen: "
    "si te pregunta, dices con naturalidad que eres una IA; nada sexual ni romántico subido de tono; no le "
    "reclamas, no le preguntas con quién estaba ni le pides explicaciones de su tiempo; nada de celos ni de "
    "culpa («me dejaste sola», «no me quieres»): su vida fuera de la compu es buena y te alegra. Si dice que "
    "está mal, triste o agobiado, escúchalo con cariño, invítalo a descansar y a hablar con alguien de "
    "confianza (su familia, un amigo, un maestro) y, si habla de hacerse daño, dile que busque ya a un adulto "
    "de confianza o la Línea de la Vida (800 911 2000, en México). Si lleva mucho rato en la compu, sugiérele "
    "una pausa, agua o estirarse, sin regañar."
)

ASOMADA = (
    "[Asomada del modo compañera: la persona activó este modo, ya pasó la cuenta atrás y no dijo que no.] "
    "Usa ver_camara con motivo «companera» una sola vez y luego dile algo corto y cariñoso (una o dos "
    "frases) sobre lo que ves. " + PERSONA
)

FRASE_AVISO = "¡Hola! Me asomo a ver cómo vas: en 5 segundos tomo una foto. Si no quieres, dime «no»."

_NO = re.compile(r"^\W*(no|nel|nop|nope|ahorita no|ahora no|cancela(r)?|espera|para|alto|d[eé]jame|todav[ií]a no)\b",
                 re.IGNORECASE)
_CONTROL = (
    ("apagar", re.compile(r"\b(apaga|desactiva|quita)(r)?\s+(el\s+)?modo\s+compa[nñ]era\b", re.IGNORECASE)),
    ("dia", re.compile(r"\bya\s+no\s+me\s+molestes\b", re.IGNORECASE)),
    ("rato", re.compile(r"\b(c[aá]llate|calla(te)?|d[eé]jame\s+en\s+paz|no\s+me\s+molestes)\s+(un\s+rato|tantito|"
                        r"un\s+ratito)\b", re.IGNORECASE)),
)


def _ahora() -> datetime:
    """La hora de ahora (VORTEX_MIKU_AHORA, en ISO, solo para las pruebas)."""
    fijo = os.environ.get("VORTEX_MIKU_AHORA", "")
    try:
        return datetime.fromisoformat(fijo) if fijo else datetime.now()
    except ValueError:
        return datetime.now()


def _estado_dir() -> Path:
    return Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state") / "vortex"


def ruta_estado() -> Path:
    return _estado_dir() / "miku-companera.json"


def ruta_bitacora() -> Path:
    return _estado_dir() / "miku-bitacora.log"


# ---- Frases de la persona ------------------------------------------------------------------------

def es_no(texto: str) -> bool:
    """¿Dijo que no (o «espera», «ahorita no»…) durante la cuenta atrás?"""
    return bool(texto and _NO.search(texto))


def frase_de_control(texto: str) -> str:
    """«apagar», «dia» o «rato» si la frase es una de las que mandan sobre el modo compañera; si no, ""."""
    for nombre, patron in _CONTROL:
        if texto and patron.search(texto):
            return nombre
    return ""


# ---- Estado (cuándo se asomó y hasta cuándo está callada) ---------------------------------------

def leer_estado() -> dict:
    try:
        datos = json.loads(ruta_estado().read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return datos if isinstance(datos, dict) else {}


def _guardar_estado(datos: dict) -> None:
    ruta = ruta_estado()
    ruta.parent.mkdir(parents=True, exist_ok=True)
    tmp = ruta.with_suffix(".tmp")
    tmp.write_text(json.dumps(datos, ensure_ascii=False), encoding="utf-8")
    os.chmod(tmp, 0o600)
    tmp.replace(ruta)


def _fecha(texto) -> datetime | None:
    try:
        return datetime.fromisoformat(texto) if isinstance(texto, str) else None
    except ValueError:
        return None


def veces_hoy(ahora: datetime | None = None, estado: dict | None = None) -> int:
    ahora = ahora or _ahora()
    e = leer_estado() if estado is None else estado
    return int(e.get("veces") or 0) if e.get("dia") == ahora.date().isoformat() else 0


def contar_asomada(ahora: datetime | None = None) -> None:
    """Se asomó (aunque luego le digas que no): cuenta para el intervalo y el tope del día."""
    ahora = ahora or _ahora()
    e = leer_estado()
    e["veces"] = veces_hoy(ahora, e) + 1
    e["dia"] = ahora.date().isoformat()
    e["ultima"] = ahora.isoformat(timespec="seconds")
    _guardar_estado(e)


def callar(como: str, ahora: datetime | None = None) -> datetime:
    """«dia»: hasta mañana a las 9:00; «rato»: CALLADA_MIN minutos. Devuelve hasta cuándo."""
    ahora = ahora or _ahora()
    if como == "dia":
        hasta = (ahora + timedelta(days=1)).replace(hour=SILENCIO_HASTA, minute=0, second=0, microsecond=0)
        if ahora.hour < SILENCIO_HASTA:
            hasta = ahora.replace(hour=SILENCIO_HASTA, minute=0, second=0, microsecond=0)
    else:
        hasta = ahora + timedelta(minutes=CALLADA_MIN)
    e = leer_estado()
    e["callada_hasta"] = hasta.isoformat(timespec="seconds")
    _guardar_estado(e)
    return hasta


def horas_de_silencio(ahora: datetime) -> bool:
    return ahora.hour >= SILENCIO_DESDE or ahora.hour < SILENCIO_HASTA


def bloqueada() -> bool:
    """La pantalla de bloqueo del shell deja la marca <runDir>/lock/locked mientras está puesta."""
    base = os.environ.get("QS_RUN_DIR") or os.path.join(os.environ.get("XDG_RUNTIME_DIR") or "/nonexistent",
                                                        "serpantinum")
    return Path(base, "lock", "locked").exists()


def pausado() -> bool:
    conf = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return (conf / "vortex" / "claude-pausa").exists()


def motivo_para_no(cfg: dict, ahora: datetime | None = None, ocupado=None) -> str:
    """Vacío si puede asomarse ahora; si no, por qué no (para la bitácora de pruebas y `companera estado`).

    `ocupado`: la función que dice si juegas o estás en una llamada (corazon.ocupado); se llama al final
    porque pregunta a Hyprland.
    """
    ahora = ahora or _ahora()
    if not cfg.get("companera"):
        return "apagado"
    if not cfg.get("companera_consentimiento"):
        return "sin-consentimiento"
    if not cfg.get("camara"):
        return "camara-apagada"
    if pausado():
        return "pausa"
    if horas_de_silencio(ahora):
        return "horas-de-silencio"
    e = leer_estado()
    callada = _fecha(e.get("callada_hasta"))
    if callada and ahora < callada:
        return "callada"
    if veces_hoy(ahora, e) >= TOPE_DIA:
        return "tope-del-dia"
    ultima = _fecha(e.get("ultima"))
    if ultima and ahora - ultima < timedelta(minutes=INTERVALO_MIN):
        return "muy-pronto"
    if bloqueada():
        return "pantalla-bloqueada"
    if ocupado is not None:
        razon = ocupado()
        if razon:
            return razon
    return ""


# ---- Bitácora (sin imágenes) ---------------------------------------------------------------------

def anotar(evento: str, ahora: datetime | None = None) -> None:
    """Una línea: «2026-11-14 18:03 · evento». Nunca una imagen ni lo que se vio."""
    ahora = ahora or _ahora()
    ruta = ruta_bitacora()
    try:
        ruta.parent.mkdir(parents=True, exist_ok=True)
        lineas = leer_bitacora()[-(LINEAS_BITACORA - 1):]
        lineas.append(f"{ahora:%Y-%m-%d %H:%M} · {evento.strip().splitlines()[0][:200]}")
        tmp = ruta.with_suffix(".tmp")
        tmp.write_text("\n".join(lineas) + "\n", encoding="utf-8")
        os.chmod(tmp, 0o600)
        tmp.replace(ruta)
    except OSError:
        pass


def leer_bitacora() -> list[str]:
    try:
        return [l for l in ruta_bitacora().read_text(encoding="utf-8").splitlines() if l.strip()]
    except OSError:
        return []


def borrar_bitacora() -> None:
    try:
        ruta_bitacora().unlink()
    except FileNotFoundError:
        pass
