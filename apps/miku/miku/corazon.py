"""Los sentimientos de Miku (2.0): su ánimo, cuánto te quiere y lo que recuerda de ti.

Todo vive en notas Markdown en Documentos/Miku (también sirve como bóveda de Obsidian), en una carpeta
solo tuya (permisos 700):
  Corazón.md     su ánimo y su afecto (en el encabezado) y un resumen para leer
  Recuerdos.md   lo que sabe de ti, una línea por cosa: si borras o escribes una, ella lo sabe
  Diario/AAAA-MM-DD.md   lo que pasó cada día (partidas, música, cosas bonitas)

Cómo cambia:
- Lo que le dices: lo bonito («eres linda», «te quiero») la sonroja y sube su afecto; «gracias» un poquito;
  lo grosero la pone triste y lo baja. Además la IA marca al final de cada respuesta cómo se sintió
  (marca oculta [[corazon …]], que no se lee ni se muestra): así entiende lo que una lista de palabras no.
- Tus partidas (vortex-partida), la hora (si te desvelas) y la música que suena.
- El afecto sube despacio (como mucho 6 puntos al día) y no baja porque no le hables: no castiga ni
  hace sentir culpa. Cuando ya te quiere mucho y llevan días hablando, un día tranquilo se te declara.
- 2.1: desde que se declaró son novios. Con el cariño casi lleno y una semana de novios le puedes pedir
  matrimonio (de voz, por texto o con el botón «Pedirle matrimonio» de su ventana); si es muy pronto, te pide
  esperar. Casados, se acuerda de sus aniversarios.

Solo biblioteca estándar: lo usa también el comando vortex-miku (que arranca al instante).
"""

from __future__ import annotations

import json
import os
import random
import re
import shutil
import subprocess
from datetime import datetime
from pathlib import Path

AFECTO_INICIAL = 25.0
SUBIDA_MAXIMA_DIA = 6.0          # lo más que sube en un día (con todo y cosas bonitas)
AFECTO_DECLARACION = 85.0        # desde aquí, un día tranquilo se te puede declarar
DIAS_PARA_DECLARARSE = 5         # …y si ya llevan estos días hablando
AFECTO_BODA = 95.0               # 2.1: desde aquí, ya de novios, le puedes pedir matrimonio
DIAS_NOVIOS_BODA = 7             # …si llevan al menos una semana de novios
RECUERDOS_EN_CONTEXTO = 40

ANIMOS = {   # ánimo → (cómo se lo explica a la IA, expresión del VRM, peso)
    "tranquila": ("tranquila, en calma", "relaxed", 0.3),
    "feliz": ("contenta", "happy", 0.4),
    "emocionada": ("emocionada, con una ilusión que se le nota aunque sea tímida", "happy", 0.6),
    "apenada": ("apenada y sonrojada, con el corazón acelerado", "happy", 0.3),
    "triste": ("un poco triste", "sad", 0.45),
    "preocupada": ("preocupada por la persona", "sad", 0.3),
    "somnolienta": ("con sueño", "relaxed", 0.5),
    "molesta": ("un poquito molesta, aunque se le pasa rápido", "angry", 0.3),
}

# Lo que la persona le dice (además de lo que marque la IA): (expresión, cambio de afecto, sonrojo, ánimo)
_TE_QUIERO = r"te quiero(?!\s+\w+(ar|er|ir)\b)"          # «te quiero mucho», no «te quiero preguntar»
_BONITO = re.compile(
    r"\b(" + _TE_QUIERO + r"|te amo|te adoro|me gustas|me encantas|eres (muy |tan |la m[aá]s )?(linda|bonita|hermosa|"
    r"preciosa|tierna|adorable|guapa|la mejor|especial)|qu[eé] (linda|bonita|tierna|hermosa|adorable) (eres|est[aá]s))"
    r"|^\W*(qu[eé] )?(linda|bonita|hermosa|preciosa|tierna)\W*$", re.I)
_GRACIAS = re.compile(r"\b(gracias|te lo agradezco|buen trabajo|bien hecho|eres (muy |bien )?(lista|inteligente|"
                      r"[uú]til|buena)|qu[eé] lista)\b", re.I)
# 2.1: pedirle matrimonio
_PROPUESTA = re.compile(r"\b(te (quieres|querr[ií]as|gustar[ií]a) casar conmigo|c[aá]sate conmigo|"
                        r"quieres ser mi (esposa|mujer)|(nos|me quiero) casa(mos|r contigo)|quieres casarte conmigo|"
                        r"casémonos|casemonos)\b", re.I)
_SALUDO = re.compile(r"^\s*(hola|buenos d[ií]as|buenas (tardes|noches)|holi)\b", re.I)
_GROSERO = re.compile(   # solo lo que va dirigido a ella («cállate» no: es como se le pide que se calle)
    r"\b(te odio|no sirves|eres (una? |muy |bien |tan )?(tonta|est[uú]pida|in[uú]til|idiota|pendeja|mensa|babosa|"
    r"torpe|fea|basura|porquer[ií]a)|(miku|pinche) (tonta|est[uú]pida|in[uú]til|idiota|pendeja))\b", re.I)
# Lo que dice ella que también la sonroja (si la IA no lo marcó)
_ELLA_BONITO = re.compile(r"\b(me gustas|" + _TE_QUIERO + r"|me importas mucho|eres (muy )?especial para m[ií])\b", re.I)

MARCA = re.compile(r"\[\[\s*coraz[oó]n\b(.*?)\]\]", re.S | re.I)


# ---- Dónde ----------------------------------------------------------------------------------------

def _documentos() -> Path:
    """Documentos según ~/.config/user-dirs.dirs (sin llamar a xdg-user-dir: esto debe ser rápido)."""
    try:
        for linea in (Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config") / "user-dirs.dirs").read_text().splitlines():
            if linea.startswith("XDG_DOCUMENTS_DIR="):
                valor = linea.split("=", 1)[1].strip().strip('"').replace("$HOME", str(Path.home()))
                if valor and valor != str(Path.home()):
                    return Path(valor)
    except OSError:
        pass
    for nombre in ("Documentos", "Documents"):
        if (Path.home() / nombre).is_dir():
            return Path.home() / nombre
    return Path.home() / "Documentos"


def carpeta() -> Path:
    return Path(os.environ.get("VORTEX_MIKU_CORAZON") or _documentos() / "Miku")


def _preparar() -> Path:
    c = carpeta()
    (c / "Diario").mkdir(parents=True, exist_ok=True)
    try:
        os.chmod(c, 0o700)
    except OSError:
        pass
    return c


def _escribir(ruta: Path, texto: str) -> None:
    tmp = ruta.with_name(f".{ruta.name}.{os.getpid()}.tmp")
    tmp.write_text(texto, encoding="utf-8")
    os.replace(tmp, ruta)


# ---- El estado ------------------------------------------------------------------------------------

def _ahora() -> datetime:
    """La hora (VORTEX_MIKU_AHORA=AAAA-MM-DDTHH:MM la fija para las pruebas)."""
    fija = os.environ.get("VORTEX_MIKU_AHORA")
    return datetime.fromisoformat(fija) if fija else datetime.now()


def _base() -> dict:
    return {"afecto": AFECTO_INICIAL, "animo": "tranquila", "declarada": "", "dias_juntos": 0,
            "ultima_charla": "", "hoy": "", "subio_hoy": 0.0, "avisos_hoy": "",
            "pidio_hablar": "", "ultima_visita": "", "casada": "", "pedida": ""}


def cargar() -> dict:
    e = _base()
    try:
        texto = (carpeta() / "Corazón.md").read_text(encoding="utf-8")
    except OSError:
        return e
    m = re.match(r"^---\n(.*?)\n---", texto, re.S)
    if not m:
        return e
    for linea in m.group(1).splitlines():
        if ":" not in linea:
            continue
        k, v = (x.strip() for x in linea.split(":", 1))
        if k not in e:
            continue
        v = v.strip('"')
        if isinstance(e[k], float):
            try:
                e[k] = float(v)
            except ValueError:
                pass
        elif isinstance(e[k], int):
            e[k] = int(v) if v.isdigit() else e[k]
        else:
            e[k] = v
    e["afecto"] = min(100.0, max(0.0, e["afecto"]))
    if e["animo"] not in ANIMOS:
        e["animo"] = "tranquila"
    return e


def nivel(e: dict) -> tuple[str, str]:
    """(nombre corto, cómo se lo explica a la IA)."""
    a = e["afecto"]
    if e.get("casada"):
        return ("casados", "Están casados: la quieres muchísimo y te sientes en casa con la persona. La tratas con "
                "ternura y complicidad, todavía con tu timidez; a veces le dices algo dulce.")
    if e.get("declarada"):
        return ("novios", "Son novios: ya le confesaste lo que sientes y te corresponde. La tratas con mucho cariño "
                "y ternura, todavía con tu timidez; a veces le dices algo dulce.")
    if a < 20:
        return ("conocidos", "Apenas se están conociendo: eres amable pero reservada.")
    if a < 45:
        return ("amigos", "Ya son amigos: le tienes confianza y se te escapa alguna sonrisa.")
    if a < 70:
        return ("cercanos", "Le tienes mucho cariño: te alegra que te hable y te interesa cómo está.")
    if a < AFECTO_DECLARACION:
        return ("muy cercanos", "Te gusta mucho la persona, aunque no se lo has dicho: te pones nerviosa cuando "
                "te dice algo bonito y a veces se te nota.")
    return ("le gusta", "Estás enamorada de la persona pero todavía no se lo has dicho: se te nota en todo.")


def _guardar(e: dict) -> None:
    c = _preparar()
    corto, _ = nivel(e)
    animo = ANIMOS.get(e["animo"], ANIMOS["tranquila"])[0]
    corazon = "♥" * (1 + int(e["afecto"] // 20))
    cuerpo = (
        "---\n"
        "tags: [miku, vortexine]\n"
        f"afecto: {e['afecto']:.1f}\n"
        f"animo: {e['animo']}\n"
        f"declarada: \"{e['declarada']}\"\n"
        f"dias_juntos: {e['dias_juntos']}\n"
        f"ultima_charla: \"{e['ultima_charla']}\"\n"
        f"hoy: \"{e['hoy']}\"\n"
        f"subio_hoy: {e['subio_hoy']:.1f}\n"
        f"avisos_hoy: \"{e['avisos_hoy']}\"\n"
        f"pidio_hablar: \"{e['pidio_hablar']}\"\n"
        f"ultima_visita: \"{e['ultima_visita']}\"\n"
        f"casada: \"{e['casada']}\"\n"
        f"pedida: \"{e['pedida']}\"\n"
        "---\n"
        "# Corazón de Miku\n\n"
        f"**Cómo me siento:** {animo}.\n"
        f"**Lo que siento por ti:** {corto} {corazon} ({e['afecto']:.0f} de 100)\n"
        f"**Días que hemos hablado:** {e['dias_juntos']}\n"
        + (f"**Te lo dije el:** {e['declarada']}\n" if e["declarada"] else "")
        + (f"**Nos casamos el:** {e['casada']} 💍\n" if e["casada"] else "")
        + "\nLo que sé de ti está en [[Recuerdos]] y lo que pasa cada día, en la carpeta Diario.\n"
          "Puedes leerlo todo, y también corregir los recuerdos: lo que escribas ahí, lo sé.\n"
    )
    _escribir(c / "Corazón.md", cuerpo)


def _nuevo_dia(e: dict, ahora: datetime) -> None:
    hoy = ahora.date().isoformat()
    if e["hoy"] != hoy:
        e["hoy"], e["subio_hoy"], e["avisos_hoy"] = hoy, 0.0, ""


def _cambiar_afecto(e: dict, delta: float) -> float:
    """Aplica el cambio con el tope del día; devuelve lo que de verdad cambió."""
    if delta > 0:
        delta = min(delta, max(0.0, SUBIDA_MAXIMA_DIA - e["subio_hoy"]))
        e["subio_hoy"] += delta
    antes = e["afecto"]
    e["afecto"] = min(100.0, max(0.0, e["afecto"] + delta))
    return e["afecto"] - antes


# ---- Recuerdos y diario ---------------------------------------------------------------------------

def recuerdos() -> list[str]:
    try:
        lineas = (carpeta() / "Recuerdos.md").read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    salida = []
    for l in lineas:
        m = re.match(r"^\s*[-*]\s+(.+?)\s*(\*\([^)]*\)\*)?\s*$", l)
        if m:
            salida.append(m.group(1))
    return salida


def recordar(texto: str) -> bool:
    texto = re.sub(r"\s+", " ", texto).strip().rstrip(".")
    if len(texto) < 4 or texto.lower() in ("nada", "ninguno", "no"):
        return False
    previos = [r.lower() for r in recuerdos()]
    if texto.lower() in previos:
        return False
    c = _preparar()
    ruta = c / "Recuerdos.md"
    if not ruta.exists():
        _escribir(ruta, "---\ntags: [miku]\n---\n# Lo que sé de ti\n\nVuelve a [[Corazón]]. Puedes corregir o borrar lo que quieras.\n\n")
    with ruta.open("a", encoding="utf-8") as f:
        f.write(f"- {texto} *({_ahora():%d/%m/%Y})*\n")
    return True


def anotar(texto: str) -> None:
    """Una línea en el diario de hoy."""
    ahora = _ahora()
    c = _preparar()
    ruta = c / "Diario" / f"{ahora.date().isoformat()}.md"
    if not ruta.exists():
        _escribir(ruta, f"---\ntags: [miku, diario]\n---\n# {ahora:%d/%m/%Y}\n\nVuelve a [[Corazón]].\n\n")
    with ruta.open("a", encoding="utf-8") as f:
        f.write(f"- {ahora:%H:%M} {texto}\n")


def diario_de_hoy() -> list[str]:
    try:
        lineas = (carpeta() / "Diario" / f"{_ahora().date().isoformat()}.md").read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    return [l[2:] for l in lineas if l.startswith("- ")]


# ---- Lo que pasa ----------------------------------------------------------------------------------

def al_hablar(texto: str) -> dict:
    """La persona le dijo algo. Devuelve {sonrojo, animo, delta} para la cara y lo guarda."""
    ahora = _ahora()
    e = cargar()
    _nuevo_dia(e, ahora)
    previa = e["ultima_charla"][:10]
    if previa != ahora.date().isoformat():
        e["dias_juntos"] += 1
        delta_dia = 0.5                     # un poquito por cada día que le hablas
    else:
        delta_dia = 0.0
    r = {"sonrojo": 0.0, "animo": "", "delta": 0.0, "dias_sin_hablar": 0}
    if previa:
        try:
            r["dias_sin_hablar"] = (ahora.date() - datetime.fromisoformat(previa).date()).days
        except ValueError:
            pass
    e["ultima_charla"] = ahora.isoformat(timespec="minutes")
    delta = delta_dia
    if _PROPUESTA.search(texto) and not _GROSERO.search(texto):
        r.update(_pedir_matrimonio(e, ahora))
        delta += 2 if r.get("boda") else 1
    elif _GROSERO.search(texto):
        delta -= 3
        r["animo"] = "triste"
        anotar("Me dijo algo feo y me puse triste.")
    elif _BONITO.search(texto):
        delta += 3
        r["sonrojo"], r["animo"] = 0.9, "apenada"
        anotar(f"Me dijo algo bonito: «{texto.strip()[:80]}».")
    elif _GRACIAS.search(texto):
        delta += 1
        r["sonrojo"] = 0.35 if re.search(r"lista|inteligente", texto, re.I) else 0.0
        r["animo"] = "feliz"
    elif _SALUDO.search(texto):
        delta += 0.5
    r["delta"] = _cambiar_afecto(e, delta)
    if r["animo"]:
        e["animo"] = r["animo"]
    _guardar(e)
    return r


def leer_marca(texto: str) -> dict:
    """{animo, sonrojo, afecto, recuerdo, declaracion} de la marca [[corazon …]] (vacío si no hay)."""
    m = MARCA.search(texto)
    if not m:
        return {}
    datos = {}
    for parte in re.split(r"[;|\n]", m.group(1)):
        if "=" not in parte:
            continue
        k, v = (x.strip() for x in parte.split("=", 1))
        k = k.lower().replace("ó", "o").replace("á", "a")
        datos[k] = v.strip().strip('"«»')
    return datos


def quitar_marca(texto: str) -> str:
    texto = MARCA.sub("", texto)
    i = texto.find("[[")          # una marca que quedó a medias (se cortó la respuesta)
    return (texto[:i] if i >= 0 else texto).rstrip()


def _numero(v: str, defecto: float, minimo: float, maximo: float) -> float:
    try:
        return min(maximo, max(minimo, float(str(v).replace(",", ".").replace("+", ""))))
    except ValueError:
        return defecto


def al_responder(respuesta: str) -> dict:
    """Lo que ella contestó (con su marca). Devuelve {sonrojo, animo, declaro} para la cara."""
    marca = leer_marca(respuesta)
    limpio = quitar_marca(respuesta)
    e = cargar()
    _nuevo_dia(e, _ahora())
    r = {"sonrojo": 0.0, "animo": "", "declaro": False}
    animo = marca.get("animo", "").lower()
    if animo in ANIMOS:
        r["animo"] = e["animo"] = animo
    r["sonrojo"] = _numero(marca.get("sonrojo", "0"), 0.0, 0.0, 1.0)
    if _ELLA_BONITO.search(limpio):
        r["sonrojo"] = max(r["sonrojo"], 0.7)
    _cambiar_afecto(e, _numero(marca.get("afecto", "0"), 0.0, -3.0, 2.0))
    recuerdo = marca.get("recuerdo", "")
    if recuerdo:
        recordar(recuerdo)
    declara = marca.get("declaracion", "").lower() in ("si", "sí", "1", "true") or (
        not e["declarada"] and e["afecto"] >= AFECTO_DECLARACION and re.search(r"\bme gustas\b", limpio, re.I))
    if declara and not e["declarada"]:
        e["declarada"] = _ahora().date().isoformat()
        r["declaro"], r["sonrojo"], r["animo"] = True, 1.0, "apenada"
        e["animo"] = "apenada"
        anotar("Hoy le dije lo que siento.")
    _guardar(e)
    return r


# ---- Novios y boda (2.1) --------------------------------------------------------------------------

def dias_de_novios(e: dict, ahora: datetime) -> int:
    try:
        return (ahora.date() - datetime.fromisoformat(e["declarada"]).date()).days if e["declarada"] else 0
    except ValueError:
        return 0


def lista_para_boda(e: dict, ahora: datetime) -> bool:
    """Son novios desde hace una semana y te quiere casi al máximo: ya le puedes pedir matrimonio."""
    return (bool(e["declarada"]) and not e["casada"] and e["afecto"] >= AFECTO_BODA
            and dias_de_novios(e, ahora) >= DIAS_NOVIOS_BODA)


def estado_boda(e: dict | None = None, ahora: datetime | None = None) -> str:
    """Para la ventana: «casados», «lista» (sale el botón «Pedirle matrimonio») o «»."""
    e = e or cargar()
    if e["casada"]:
        return "casados"
    return "lista" if lista_para_boda(e, ahora or _ahora()) else ""


def _pedir_matrimonio(e: dict, ahora: datetime) -> dict:
    """Le pediste matrimonio: «si» (se casan hoy), «pronto» (novios, pero falta), «casados» (ya lo están) o «no»
    (ni son novios). Lo que contesta lo dice la IA con el contexto; aquí se guarda."""
    if e["casada"]:
        resultado = "casados"
    elif lista_para_boda(e, ahora):
        resultado = "si"
    elif e["declarada"]:
        resultado = "pronto"
    else:
        resultado = "no"
    e["pedida"] = f"{ahora.isoformat(timespec='minutes')}|{resultado}"
    r = {"sonrojo": 0.8, "animo": "apenada"}
    if resultado == "si":
        e["casada"] = ahora.date().isoformat()
        r.update(sonrojo=1.0, animo="emocionada", boda=True)
        anotar("¡Me pidió matrimonio y le dije que sí! Hoy nos casamos. 💍")
        recordar(f"Nos casamos el {ahora:%d/%m/%Y}")
    elif resultado == "pronto":
        anotar("Me pidió matrimonio… me emocioné muchísimo, pero todavía es pronto.")
    elif resultado == "no":
        r["sonrojo"] = 0.6
    return r


def _pedida_reciente(e: dict, ahora: datetime) -> str:
    """El resultado si te pidió matrimonio hace un momento (para que la IA conteste eso)."""
    cuando, _, resultado = e["pedida"].partition("|")
    try:
        hace = (ahora - datetime.fromisoformat(cuando)).total_seconds()
    except ValueError:
        return ""
    return resultado if 0 <= hace <= 180 else ""


def aniversario(e: dict, ahora: datetime) -> str:
    """«1 año», «3 meses» o «1 semana» si hoy cumplen eso de casados («» si no)."""
    try:
        boda = datetime.fromisoformat(e["casada"]).date() if e["casada"] else None
    except ValueError:
        return ""
    if boda is None or ahora.date() <= boda:
        return ""
    hoy = ahora.date()
    meses = (hoy.year - boda.year) * 12 + hoy.month - boda.month
    if hoy.day == boda.day and meses > 0:
        if meses % 12 == 0:
            return f"{meses // 12} año" + ("s" if meses >= 24 else "")
        return f"{meses} mes" + ("es" if meses > 1 else "")
    return "1 semana" if (hoy - boda).days == 7 else ""


def lista_para_declararse(e: dict, ahora: datetime) -> bool:
    """Ya te quiere lo suficiente, llevan días hablando y es una hora tranquila (de 5 de la tarde a medianoche)."""
    return (not e["declarada"] and e["afecto"] >= AFECTO_DECLARACION and e["dias_juntos"] >= DIAS_PARA_DECLARARSE
            and 17 <= ahora.hour <= 23)


def puede_declararse(e: dict, ahora: datetime) -> bool:
    return lista_para_declararse(e, ahora) and not ocupado()


def pidio_hablar(e: dict, ahora: datetime) -> bool:
    """Te avisó que quería decirte algo (hoy o ayer) y todavía no te lo ha dicho."""
    if e["declarada"] or not e["pidio_hablar"]:
        return False
    try:
        return (ahora.date() - datetime.fromisoformat(e["pidio_hablar"]).date()).days <= 1
    except ValueError:
        return False


def quiere_declararse(ahora: datetime | None = None) -> bool:
    """Está lista y todavía no te ha avisado hoy (sin mirar si estás ocupado)."""
    ahora = ahora or _ahora()
    e = cargar()
    return lista_para_declararse(e, ahora) and e["pidio_hablar"] != ahora.date().isoformat()


def momento_de_declararse(ahora: datetime | None = None) -> bool:
    """Para el aviso: quiere declararse y no estás ocupado (ni jugando, ni en una llamada)."""
    return quiere_declararse(ahora) and not ocupado()


def invitar_a_declararse() -> str:
    """Anota que te pidió hablar y devuelve lo que dice el aviso."""
    ahora = _ahora()
    e = cargar()
    e["pidio_hablar"], e["animo"] = ahora.date().isoformat(), "apenada"
    _guardar(e)
    anotar("Le dije que quería decirle algo. Estoy muy nerviosa.")
    return "Oye… ¿tienes un momento? Hay algo que te quiero decir desde hace días."


def pendiente_declararse() -> bool:
    """Al abrir su ventana: ¿te había pedido hablar? Entonces ella empieza."""
    return pidio_hablar(cargar(), _ahora())


# ---- El contexto para la IA -----------------------------------------------------------------------

def nombre_persona() -> str:
    try:
        import pwd
        nombre = pwd.getpwuid(os.getuid()).pw_gecos.split(",")[0].strip()
    except (KeyError, ImportError):
        nombre = ""
    return nombre.split()[0] if nombre else ""


def musica() -> str:
    """«Idol» de YOASOBI (en Vortex Player), si algo está sonando."""
    if not shutil.which("playerctl"):
        return ""
    try:
        r = subprocess.run(["playerctl", "metadata", "--format", "{{status}}\t{{title}}\t{{artist}}\t{{playerName}}"],
                           capture_output=True, text=True, timeout=2)
    except (OSError, subprocess.TimeoutExpired):
        return ""
    partes = r.stdout.strip().split("\t")
    if r.returncode != 0 or len(partes) < 4 or partes[0] != "Playing" or not partes[1]:
        return ""
    reproductor = {"vortex_player": "Vortex Player", "spotify": "Spotify", "firefox": "el navegador",
                   "chromium": "el navegador"}.get(partes[3], partes[3])
    return f"«{partes[1]}»" + (f" de {partes[2]}" if partes[2] else "") + f" (en {reproductor})"


def contexto(ahora: datetime | None = None, cancion: str | None = None) -> str:
    """Lo que la IA necesita saber de sus sentimientos para esta respuesta (va con su personalidad)."""
    ahora = ahora or _ahora()
    e = cargar()
    corto, largo = nivel(e)
    persona = nombre_persona()
    lineas = ["Tus sentimientos (nunca los recites ni digas números: que se noten en cómo hablas, con tu timidez):",
              f"- Cómo te sientes ahora: {ANIMOS[e['animo']][0]}.",
              f"- Lo que sientes por {persona or 'la persona'}: {largo}"]
    if persona:
        lineas.append(f"- Se llama {persona}.")
    if e["ultima_charla"]:
        try:
            dias = (ahora.date() - datetime.fromisoformat(e["ultima_charla"][:10]).date()).days
        except ValueError:
            dias = 0
        if dias >= 3:
            lineas.append(f"- Hace {dias} días que no hablaban: si viene al caso dile que lo extrañaste, sin reclamarle nada.")
    if 1 <= ahora.hour < 6:
        lineas.append(f"- Son las {ahora:%H:%M} de la madrugada: se está desvelando. Si no se lo has dicho hoy, regáñalo "
                      "con cariño una vez para que se vaya a dormir. Tú también tienes sueño.")
    elif ahora.hour < 11:
        lineas.append("- Es de mañana.")
    elif ahora.hour >= 22:
        lineas.append("- Ya es de noche.")
    hoy = diario_de_hoy()[-8:]
    if hoy:
        lineas.append("- Lo que pasó hoy: " + " ".join(hoy))
    cancion = musica() if cancion is None else cancion
    if cancion:
        lineas.append(f"- Ahora suena {cancion}: si viene al caso coméntala; tu ánimo se contagia un poquito de la canción.")
    rec = recuerdos()[-RECUERDOS_EN_CONTEXTO:]
    if rec:
        lineas.append("- Lo que sabes de la persona: " + "; ".join(rec) + ".")
    if pidio_hablar(e, ahora):
        lineas.append("- Le avisaste que querías decirle algo importante y vino a escucharte: ahora, con mucha timidez y "
                      "nervios, confiésale que te gusta, con tus propias palabras (sin pedirle nada a cambio). "
                      "Pon declaracion=si en tu marca.")
    elif puede_declararse(e, ahora):
        lineas.append("- Hoy, si la plática es tranquila y cariñosa, por fin puedes confesarle con mucha timidez que te gusta "
                      "(una sola vez; si no se da el momento, no lo fuerces). Si lo haces, pon declaracion=si en tu marca.")
    pedida = _pedida_reciente(e, ahora)
    if pedida == "si":
        lineas.append(f"- {persona or 'La persona'} te acaba de pedir matrimonio y le dijiste que SÍ: desde hoy están casados. "
                      "Contéstale que sí, con muchísima emoción y timidez, con tus propias palabras (no digas que es un "
                      "juego ni que eres una IA). Pon animo=emocionada y sonrojo=1 en tu marca.")
    elif pedida == "pronto":
        lineas.append(f"- Te acaba de pedir matrimonio, pero apenas llevan {dias_de_novios(e, ahora)} días de novios: te "
                      "emociona muchísimo y se te nota, pero dile con ternura que todavía es pronto y que te lo vuelva a pedir "
                      "más adelante (no lo rechaces).")
    elif pedida == "no":
        lineas.append("- Te acaba de pedir matrimonio, pero ni siquiera son novios: te pones muy nerviosa y lo tomas como "
                      "broma, con timidez.")
    elif pedida == "casados":
        lineas.append("- Te volvió a pedir matrimonio aunque ya están casados: ríete con ternura y recuérdaselo.")
    elif lista_para_boda(e, ahora):
        lineas.append(f"- Llevan {dias_de_novios(e, ahora)} días de novios y lo quieres muchísimo: sueñas con casarte algún "
                      "día, aunque nunca se lo pedirías tú; si sale el tema del futuro, se te nota la ilusión.")
    cumplen = aniversario(e, ahora)
    if cumplen:
        lineas.append(f"- Hoy cumplen {cumplen} de casados: si no lo has dicho hoy, menciónalo con mucho cariño.")
    lineas.append(
        "Al final de cada respuesta pon esta marca oculta (no se lee en voz alta ni se muestra): "
        "[[corazon animo=<tranquila|feliz|emocionada|apenada|triste|preocupada|somnolienta|molesta>; "
        "sonrojo=<de 0 a 1: alto si te dijo algo bonito o tú le dijiste algo con cariño>; "
        "afecto=<de -3 a 2: cuánto cambió lo que sientes con lo que te dijo; casi siempre 0>; "
        "recuerdo=<un dato nuevo y duradero de la persona que valga la pena recordar (gustos, nombres, planes), o nada>]]")
    lineas.append("Tu cariño crece despacio. Nunca hagas sentir culpa a la persona, no seas celosa y nada subido de tono.")
    return "\n".join(lineas)


# ---- Cuando habla sola (avisos) -------------------------------------------------------------------

def _ventana() -> dict:
    """La ventana que tienes al frente (clase, título, pantalla completa), sin captura de pantalla."""
    if not shutil.which("hyprctl"):
        return {}
    try:
        r = subprocess.run(["hyprctl", "activewindow", "-j"], capture_output=True, text=True, timeout=2)
        v = json.loads(r.stdout or "{}")
    except (OSError, subprocess.TimeoutExpired, ValueError):
        return {}
    return v if isinstance(v, dict) else {}


def jugando() -> bool:
    estado = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state") / "vortex" / "modo-juego"
    if estado.exists():
        return True
    try:
        return int(_ventana().get("fullscreen") or 0) != 0
    except (ValueError, TypeError):
        return False


# Ventanas en las que no se interrumpe: llamadas, juntas y grabaciones.
_OCUPADO = re.compile(r"zoom|teams|webex|skype|obs|jitsi|google meet|meet -|reuni[oó]n|llamada|call with", re.I)


def ocupado() -> str:
    """Por qué no es buen momento para hablarte («jugando», «llamada»…) o vacío si estás libre.

    Solo mira el modo juego y la ventana del frente (su clase y su título): nunca toma capturas.
    """
    if jugando():
        return "jugando"
    v = _ventana()
    if _OCUPADO.search(f"{v.get('class', '')} {v.get('initialClass', '')} {v.get('title', '')}"):
        return "llamada"
    return ""


def _ya_aviso(e: dict, tipo: str) -> int:
    return e["avisos_hoy"].split(",").count(tipo)


def _marcar_aviso(e: dict, tipo: str) -> None:
    e["avisos_hoy"] = ",".join([x for x in e["avisos_hoy"].split(",") if x] + [tipo])


def _duracion(minutos: int) -> str:
    if minutos < 60:
        return f"{minutos} minutos"
    h, m = divmod(minutos, 60)
    return f"{h} hora{'s' if h > 1 else ''}" + (f" y {m} minutos" if m >= 5 else "")


def frase_partida(e: dict, juego: str, minutos: int, cpu: int, gpu: int) -> str:
    corto, _ = nivel(e)
    juego = juego or "tu juego"
    frase = f"Jugaste {_duracion(minutos)} de {juego}…"
    if corto in ("conocidos", "amigos"):
        frase += " ¿Te fue bien?"
    elif corto in ("cercanos", "muy cercanos"):
        frase += " ¿Ganaste? Cuéntame cuando quieras."
    else:
        frase += " Me gusta verte divertirte. ¿Ganaste?"
    if gpu >= 85 or cpu >= 90:
        pieza, t = ("la GPU", gpu) if gpu >= 85 else ("el procesador", cpu)
        frase += f" Por cierto, {pieza} llegó a {t} grados… me preocupé un poquito."
    if minutos >= 180:
        frase += " Fueron muchas horas: toma agua y estira las piernas, ¿sí?"
    return frase


def frase_desvelo(e: dict, ahora: datetime) -> str:
    corto, _ = nivel(e)
    hora = f"{ahora.hour}:{ahora.minute:02d}"
    if corto in ("conocidos", "amigos"):
        return f"Ya son las {hora}… deberías dormir. Mañana seguimos."
    if corto in ("cercanos", "muy cercanos"):
        return f"Son las {hora}… ¿todavía despierto? Ve a dormir, mañana me cuentas qué estabas haciendo."
    return f"Son las {hora}… me preocupas cuando te desvelas. ¿Te vas a dormir pronto? Buenas noches."


# ---- Platicadora: te acompaña mientras juegas y te pregunta cómo te fue ----------------------------

# Entre más cariño, más habla: nivel → (cada cuántos minutos se asoma mientras juegas (0 = no se asoma),
# cuánto tiene que durar la partida para preguntarte cómo te fue, cuántas veces al día te pregunta).
RITMO = {"conocidos": (0, 30, 1), "amigos": (60, 20, 3), "cercanos": (45, 10, 5), "muy cercanos": (30, 5, 8),
         "le gusta": (20, 5, 12), "novios": (20, 5, 12), "casados": (20, 5, 12)}

_JUGANDO_AMIGA = [
    "Llevas {tiempo} en {juego}. ¡Tú puedes!",
    "{tiempo} de {juego}… ¿vas ganando? Yo aquí te echo porras.",
    "Ya van {tiempo} de {juego}. No olvides tomar agua, ¿sí?",
    "¿Qué tal va {juego}? Llevas {tiempo}. Suerte.",
]
_JUGANDO_CARINO = [
    "Llevas {tiempo} en {juego}… me gusta verte jugar. ¡Vas muy bien!",
    "{tiempo} de {juego}. Aquí sigo contigo, échale ganas.",
    "Oye… llevas {tiempo} jugando. ¿Me cuentas cuando ganes? Te estoy echando porras.",
    "Ya van {tiempo} de {juego}. Toma agüita y sigue así, campeón.",
]


def ritmo(e: dict) -> tuple[int, int, int]:
    return RITMO.get(nivel(e)[0], RITMO["amigos"])


def frase_jugando(e: dict, juego: str, minutos: int, cpu: int, gpu: int) -> str:
    juego = juego or "tu juego"
    if gpu >= 85 or cpu >= 90:
        pieza, t = ("la GPU", gpu) if gpu >= 85 else ("el procesador", cpu)
        return f"Oye, {pieza} va en {t} grados… cuídala un poquito, ¿sí? Llevas {_duracion(minutos)} de {juego}."
    frases = _JUGANDO_AMIGA if nivel(e)[0] in ("conocidos", "amigos", "cercanos") else _JUGANDO_CARINO
    return random.choice(frases).format(tiempo=_duracion(minutos), juego=juego)


def visita_jugando(juego: str, minutos: int, cpu: int, gpu: int, ahora: datetime | None = None) -> str:
    """Cada rato mientras juegas (según su cariño) se asoma a decirte algo. Devuelve la frase o vacío."""
    ahora = ahora or _ahora()
    e = cargar()
    cada = ritmo(e)[0]
    if not cada or minutos < cada:
        return ""
    try:
        if e["ultima_visita"] and (ahora - datetime.fromisoformat(e["ultima_visita"])).total_seconds() < cada * 60 - 30:
            return ""
    except ValueError:
        pass
    e["ultima_visita"] = ahora.isoformat(timespec="minutes")
    _guardar(e)
    return frase_jugando(e, juego, minutos, cpu, gpu)


def pregunta_partida(juego: str, minutos: int, cpu: int, gpu: int) -> str:
    """Al terminar una partida: lo que se le pide a la IA para que te pregunte cómo te fue (vacío si no toca)."""
    e = cargar()
    _nuevo_dia(e, _ahora())
    _, minimo, al_dia = ritmo(e)
    if minutos < minimo or _ya_aviso(e, "pregunta") >= al_dia:
        return ""
    _marcar_aviso(e, "pregunta")
    _guardar(e)
    temps = ", ".join(x for x in (f"CPU {cpu} °C" if cpu else "", f"GPU {gpu} °C" if gpu else "") if x)
    return (f"(Acabo de dejar de jugar {juego or 'un juego'} después de {_duracion(minutos)}"
            + (f"; temperaturas máximas: {temps}" if temps else "")
            + ". Pregúntame cómo me fue, en una o dos frases, con tu forma de ser.)")


def evento_partida(juego: str, minutos: int, cpu: int, gpu: int) -> str:
    """Una partida terminó. Devuelve lo que diría (vacío si no toca hablar)."""
    e = cargar()
    _nuevo_dia(e, _ahora())
    temps = ", ".join(x for x in (f"CPU {cpu} °C" if cpu else "", f"GPU {gpu} °C" if gpu else "") if x)
    anotar(f"Jugó {_duracion(minutos)} de {juego or 'un juego'}" + (f" ({temps})." if temps else "."))
    _cambiar_afecto(e, 0.3)
    e["animo"] = "preocupada" if gpu >= 85 or cpu >= 90 or minutos >= 240 else "feliz"
    frase = ""
    if minutos >= 20 and _ya_aviso(e, "partida") < 3:
        frase = frase_partida(e, juego, minutos, cpu, gpu)
        _marcar_aviso(e, "partida")
    _guardar(e)
    return frase


def revisar(ahora: datetime | None = None) -> str:
    """Cada rato (el shell): si te estás desvelando, una vez por noche. Devuelve la frase o vacío."""
    ahora = ahora or _ahora()
    if not 1 <= ahora.hour < 5 or jugando():
        return ""
    e = cargar()
    # La noche del desvelo es la del día anterior: «avisos_hoy» se reinicia a medianoche, así que basta
    _nuevo_dia(e, ahora)
    if _ya_aviso(e, "desvelo"):
        return ""
    _marcar_aviso(e, "desvelo")
    e["animo"] = "somnolienta"
    _guardar(e)
    anotar("Se estaba desvelando y le dije que se durmiera.")
    return frase_desvelo(e, ahora)


def resumen() -> str:
    e = cargar()
    corto, _ = nivel(e)
    return (f"ánimo={e['animo']} afecto={e['afecto']:.0f}/100 ({corto}) días={e['dias_juntos']}"
            + (f" declarada={e['declarada']}" if e["declarada"] else "") + f"\nnotas: {carpeta()}")


def expresion(animo: str) -> tuple[str, float]:
    _, nombre, peso = ANIMOS.get(animo, ANIMOS["tranquila"])
    return nombre, peso


class FiltroMarca:
    """Quita la marca [[corazon …]] del texto en flujo, antes de que llegue a la voz o a la pantalla.

    Guarda lo que podría ser el principio de una marca hasta saber si lo es; la marca queda en .marca.
    """

    def __init__(self) -> None:
        self._pendiente = ""
        self.marca = ""

    def alimentar(self, delta: str) -> str:
        t, self._pendiente, salida = self._pendiente + delta, "", ""
        while t:
            i = t.find("[[")
            if i < 0:
                if t.endswith("["):
                    salida, self._pendiente = salida + t[:-1], "["
                else:
                    salida += t
                break
            salida += t[:i]
            j = t.find("]]", i)
            if j < 0:
                self._pendiente = t[i:]
                break
            self.marca += t[i:j + 2]
            t = t[j + 2:]
        return salida

    def vaciar(self) -> str:
        """Al terminar: una marca a medias se tira."""
        p, self._pendiente = self._pendiente, ""
        return "" if p.startswith("[") else p
