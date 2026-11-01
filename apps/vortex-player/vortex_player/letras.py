"""Las letras, de LRCLIB (lrclib.net: gratis, sin cuenta ni llave). Casi siempre vienen sincronizadas (LRC),
así que se van marcando al ritmo de la canción como en Spotify. Si LRCLIB no la tiene (o está caído), la de
YouTube Music (de Musixmatch, sin tiempos). Se guardan en ~/.cache/vortex-player/letras: la segunda vez salen
sin internet. El mismo orden de búsqueda que `Letras.kt` del teléfono.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable

API = "https://lrclib.net/api"
AGENTE = "Vortex Player (https://github.com/MikePianoXD777/vortex-os)"
REINTENTAR = 7 * 24 * 3600   # si no había letra, se vuelve a buscar en una semana
MEJORAR = 3 * 24 * 3600      # una sin tiempos: se busca la sincronizada cada 3 días

_BASURA = re.compile(
    r"\s*[(\[【「]([^)\]】」]*?(official|oficial|video|vídeo|audio|lyric|letra|visuali[sz]er|mv\b|m/v|hd|4k|hq|"
    r"remaster|en vivo|live|explicit|clean|color coded|sub|español|english)[^)\]】」]*)[)\]】」]", re.I)
_PARENTESIS = re.compile(r"\s*[(\[【「][^)\]】」]*[)\]】」]")
_MARCA = re.compile(r"\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?]")


# 2.4: lo que se pega al final de los títulos de video sin paréntesis: «… Official Music Video», «… MV»
_COLA = re.compile(r"\s*[-|/~]?\s*(official\s+(music\s+)?video|official\s+(audio|mv|lyric\s+video)|music\s+video|"
                   r"lyric\s+video|lyrics?|m/?v|audio|visuali[sz]er|video\s+oficial|v[ií]deo\s+oficial|"
                   r"letra|hd|4k)\s*$", re.I)
_COMILLAS = re.compile(r"^(.*?)\s*[「『【\"“]([^」』】\"”]+)[」』】\"”]")


def artista_limpio(artista: str) -> str:
    """«YOASOBI - Topic» → «YOASOBI», «AdoVEVO» → «Ado» (deja a todos los artistas, no solo el primero)."""
    return re.sub(r"\s*-\s*Topic$|VEVO$|\s+Official$|\s*(official\s+)?(channel|ch\.?)$", "", (artista or "").strip(), flags=re.I).strip()


def titulo_limpio(titulo: str, artista: str = "") -> str:
    """El título como en Spotify: sin «(Official Music Video)», sin «Artista - » adelante, sin «feat. …».
    «YOASOBI「アイドル」Official Music Video» → «アイドル»."""
    t = titulo or ""
    m = _COMILLAS.match(t)
    a = limpiar_artista(artista).lower()
    if m and m.group(1).strip() and (not a or a in m.group(1).lower() or m.group(1).strip().lower() in a):
        t = m.group(2)
    t = limpiar(t, artista)
    for _ in range(3):
        t2 = _COLA.sub("", t).strip(" -|~")
        if t2 == t or not t2:
            break
        t = t2
    return t.strip() or (titulo or "").strip()


def limpiar_artista(artista: str) -> str:
    a = re.split(r",| & | x | feat| ft\.", artista or "")[0]
    return re.sub(r"\s*-\s*Topic$|VEVO$|\s+Official$", "", a, flags=re.I).strip()


def limpiar(titulo: str, artista: str = "") -> str:
    """«YOASOBI - アイドル (Official Music Video)» → «アイドル»; quita lo que no es parte del título."""
    t = _BASURA.sub("", titulo).strip()
    guion = t.find(" - ")
    if guion > 0:
        izq, a = t[:guion].strip(), limpiar_artista(artista)
        if not a or izq.lower() == a.lower() or izq.lower() in a.lower() or a.lower() in izq.lower():
            t = t[guion + 3:].strip()
    t = re.sub(r"\s+(featuring\b|feat\b\.?|ft\b\.?).*$", "", t, flags=re.I)
    return t.strip(" -|\"") or titulo.strip()


def leer_lrc(lrc: str) -> list[dict]:
    """«[01:23.45] texto» → {ms: 83450, texto}. Varias marcas en una línea dan varias líneas."""
    salida = []
    for fila in lrc.splitlines():
        marcas = list(_MARCA.finditer(fila))
        if not marcas:
            continue
        texto = fila[marcas[-1].end():].strip()
        for m in marcas:
            mi, se, fr = m.groups()
            frac = int((fr or "0").ljust(3, "0")[:3]) if fr else 0
            salida.append({"ms": int(mi) * 60000 + int(se) * 1000 + frac, "texto": texto})
    return sorted(salida, key=lambda l: l["ms"])


def texto_plano(plano: str) -> list[dict]:
    return [{"ms": -1, "texto": l.strip()} for l in plano.splitlines()]


def _tiene(o: dict, clave: str) -> bool:
    v = o.get(clave)
    return isinstance(v, str) and v.strip() != ""


def elegir(resultados: list, segundos: int, holgura: int = 4) -> dict | None:
    """De los resultados de LRCLIB, el que dura lo mismo (±4 s), mejor si viene sincronizado."""
    todos = [o for o in resultados if isinstance(o, dict) and (_tiene(o, "syncedLyrics") or _tiene(o, "plainLyrics"))]
    cerca = todos if segundos <= 0 else [o for o in todos if abs(float(o.get("duration") or 0) - segundos) <= holgura]
    return next((o for o in cerca if _tiene(o, "syncedLyrics")), cerca[0] if cerca else None)


def de_json(o: dict) -> dict | None:
    if _tiene(o, "syncedLyrics"):
        lineas = leer_lrc(o["syncedLyrics"])
        if lineas:
            return {"lineas": lineas, "sincronizada": True}
    if _tiene(o, "plainLyrics"):
        return {"lineas": texto_plano(o["plainLyrics"]), "sincronizada": False}
    return None


def _pedir(url: str) -> object | None:
    if os.environ.get("VORTEX_PLAYER_SIN_RED"):   # las pruebas no salen a internet
        raise OSError("sin red (pruebas)")
    req = urllib.request.Request(url, headers={"User-Agent": AGENTE})
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise OSError(f"LRCLIB contestó {e.code}") from None


pedir: Callable[[str], object | None] = _pedir


def buscar(titulo: str, artista: str, segundos: int) -> dict | None:
    """Busca en internet: primero exacto (título, artista y duración) y luego más suelto."""
    t, a = limpiar(titulo, artista), limpiar_artista(artista)
    c = urllib.parse.quote
    if a and segundos > 0:
        r = pedir(f"{API}/get?track_name={c(t)}&artist_name={c(a)}&duration={segundos}")
        if isinstance(r, dict) and (l := de_json(r)):
            return l
    corto = _PARENTESIS.sub("", t).strip() or t
    intentos = []
    if a:
        intentos.append(f"{API}/search?track_name={c(corto)}&artist_name={c(a)}")
    intentos.append(f"{API}/search?q={c((a + ' ' + corto).strip())}")
    if corto != t:
        intentos.append(f"{API}/search?q={c((a + ' ' + t).strip())}")
    for url in intentos:
        r = pedir(url)
        if isinstance(r, list) and (o := elegir(r, segundos)) and (l := de_json(o)):
            return l
    return None


def _archivo(carpeta: Path, titulo: str, artista: str, segundos: int) -> Path:
    h = hashlib.sha1(f"{titulo}|{artista}|{segundos}".encode()).hexdigest()[:20]
    return carpeta / f"{h}.json"


def guardada(carpeta: Path, titulo: str, artista: str, segundos: int) -> bool:
    """Si ya se buscó (la haya o no): para bajar las letras de toda tu música sin repetir (2.4)."""
    return _archivo(carpeta, titulo, artista, segundos).exists()


def de(carpeta: Path, titulo: str, artista: str, segundos: int, video_id: str = "",
       de_youtube: Callable[[str], str | None] | None = None) -> dict | None:
    """La letra guardada, o la busca (LRCLIB y luego YouTube Music) y la guarda: {lineas, sincronizada} o
    None si no hay (y no se vuelve a buscar en una semana). Sin internet lanza OSError y no anota nada."""
    f = _archivo(carpeta, titulo, artista, segundos)
    ahora = time.time()
    guardada = None
    try:
        o = json.loads(f.read_text())
        if o.get("nada"):
            if ahora - o.get("cuando", 0) < REINTENTAR:
                return None
        else:
            guardada = {"lineas": o["lineas"], "sincronizada": bool(o["sincronizada"])}
            if guardada["sincronizada"] or ahora - o.get("cuando", 0) < MEJORAR:
                return guardada
    except (OSError, ValueError, KeyError, TypeError):
        pass
    falla: Exception | None = None
    try:
        letra = buscar(titulo, artista, segundos)
    except (OSError, ValueError) as e:
        falla, letra = e, None
    if letra is None and video_id and de_youtube:
        try:
            plano = de_youtube(video_id)
            letra = {"lineas": texto_plano(plano), "sincronizada": False} if plano else None
        except (OSError, ValueError) as e:
            falla = falla or e
    if letra is None:
        if guardada:
            return guardada
        if falla:
            raise OSError(str(falla))
    carpeta.mkdir(parents=True, exist_ok=True)
    f.write_text(json.dumps({"nada": True, "cuando": ahora} if letra is None else {**letra, "cuando": ahora}, ensure_ascii=False))
    return letra


def actual(lineas: list[dict], ms: float) -> int:
    """La línea que va sonando (-1 antes de la primera)."""
    i = -1
    for k, l in enumerate(lineas):
        if l["ms"] <= ms + 300:
            i = k
        else:
            break
    return i
