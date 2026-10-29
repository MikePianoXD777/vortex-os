"""Bajar canciones y playlists de YouTube, YouTube Music y Spotify.

- YouTube y YouTube Music: yt-dlp baja el mejor audio que haya y lo guarda tal cual (Opus o AAC), sin
  recomprimirlo: convertirlo a FLAC no le devuelve la calidad y solo engaña a la etiqueta. Sin anuncios:
  se baja el audio de la canción, no el reproductor de YouTube.
- Spotify: no deja bajar su audio. De la página pública de la playlist, álbum o canción (la que usan
  para insertarla en otras páginas, sin cuenta ni llave) se leen los nombres, y cada canción se busca y
  se baja de YouTube Music.
- Con YouTube Music Premium (ajuste «navegador_premium») yt-dlp usa la sesión de tu navegador y baja
  AAC a 256 kbps en vez de Opus a ~160.

`descargar()` es un generador de eventos para la interfaz: {"tipo": "inicio" | "cancion" | "progreso" |
"archivo" | "aviso" | "fin", …}.
"""
from __future__ import annotations

import html
import json
import re
import subprocess
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable, Iterator

AGENTE = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0"
MARCA = "VORTEX"


def tipo_de(url: str) -> tuple[str, str, str] | None:
    """(«spotify» | «youtube», clase, id) o None si no es un enlace que se pueda bajar."""
    url = url.strip()
    m = re.match(r"^spotify:(track|album|playlist):([A-Za-z0-9]+)$", url)
    if m:
        return "spotify", m.group(1), m.group(2)
    try:
        p = urllib.parse.urlparse(url if "://" in url else "https://" + url)
    except ValueError:
        return None
    host = (p.hostname or "").lower()
    if host == "open.spotify.com":
        m = re.search(r"/(track|album|playlist)/([A-Za-z0-9]+)", p.path)
        return ("spotify", m.group(1), m.group(2)) if m else None
    if host in ("youtu.be",):
        return "youtube", "cancion", p.path.strip("/")
    if host.endswith("youtube.com"):
        q = urllib.parse.parse_qs(p.query)
        if p.path == "/watch" and "v" in q:
            return "youtube", "cancion", q["v"][0]
        if p.path == "/playlist" and "list" in q:
            return "youtube", "playlist", q["list"][0]
        m = re.match(r"^/(?:browse|channel)/([\w-]+)", p.path)
        if m:
            return "youtube", "playlist", m.group(1)
        m = re.match(r"^/shorts/([\w-]+)", p.path)
        if m:
            return "youtube", "cancion", m.group(1)
    return None


# --- Spotify ---------------------------------------------------------------------------------------

def _bajar_texto(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": AGENTE, "Accept-Language": "es-MX,es;q=0.9"})
    with urllib.request.urlopen(req, timeout=20) as r:
        return r.read().decode("utf-8", "replace")


def leer_spotify(pagina: str) -> tuple[str, list[dict]]:
    """(nombre, [{titulo, artistas, duracion}]) desde la página «embed» de Spotify."""
    m = re.search(r'<script id="__NEXT_DATA__"[^>]*>(.*?)</script>', pagina, re.S)
    if not m:
        raise ValueError("Spotify cambió su página: no encontré la lista de canciones")
    datos = json.loads(html.unescape(m.group(1)) if m.group(1).lstrip().startswith("&") else m.group(1))
    entidad = datos["props"]["pageProps"]["state"]["data"]["entity"]
    nombre = entidad.get("name") or entidad.get("title") or "Spotify"
    pistas = []
    for t in entidad.get("trackList") or []:
        pistas.append({"titulo": t.get("title", ""), "artistas": t.get("subtitle", "").replace(" ", " "),
                       "duracion": round((t.get("duration") or 0) / 1000)})
    if not pistas and entidad.get("type") == "track":
        artistas = ", ".join(a.get("name", "") for a in entidad.get("artists") or [])
        pistas.append({"titulo": nombre, "artistas": artistas, "duracion": round((entidad.get("duration") or 0) / 1000)})
    return nombre, [p for p in pistas if p["titulo"]]


def pistas_spotify(clase: str, sid: str, bajar: Callable[[str], str] = _bajar_texto) -> tuple[str, list[dict]]:
    return leer_spotify(bajar(f"https://open.spotify.com/embed/{clase}/{sid}"))


def busqueda_ytmusic(pista: dict) -> str:
    """La búsqueda de YouTube Music (sección «Canciones») que encuentra esa canción."""
    principal = pista["artistas"].split(",")[0].strip()
    q = f"{principal} {pista['titulo']}".strip()
    return "https://music.youtube.com/search?q=" + urllib.parse.quote(q) + "#songs"


# --- yt-dlp ----------------------------------------------------------------------------------------

def comando(objetivo: str, destino: Path, navegador: str = "", una: bool = False, yt_dlp: str = "yt-dlp") -> list[str]:
    """La línea de yt-dlp que baja el audio de `objetivo` a `destino` sin recomprimirlo."""
    c = [yt_dlp, "--newline", "--no-colors", "--ignore-errors", "--no-mtime",
         "-f", "bestaudio/best", "-x",                    # -x sin --audio-format: el mismo códec, solo cambia el envase
         "--embed-metadata", "--write-thumbnail", "--convert-thumbnails", "jpg",
         "--parse-metadata", "%(track,title)s:%(meta_title)s",
         "--parse-metadata", "%(artist,creator,uploader)s:%(meta_artist)s",
         "--parse-metadata", "%(album|)s:%(meta_album)s",
         "--parse-metadata", "%(webpage_url)s:%(meta_purl)s",
         "-o", str(destino / "%(artist,creator,uploader|Desconocido)s" / "%(track,title)s [%(id)s].%(ext)s"),
         "-o", "thumbnail:" + str(destino / "%(artist,creator,uploader|Desconocido)s" / "%(track,title)s [%(id)s].%(ext)s"),
         "--print", f"after_move:{MARCA}A\t%(filepath)s",
         "--progress-template", f"download:{MARCA}P\t%(progress._percent_str)s\t%(info.track,info.title)s"]
    if navegador:
        c += ["--cookies-from-browser", navegador]
    c += ["--no-playlist"] if una else ["--yes-playlist"]
    if objetivo.startswith("https://music.youtube.com/search"):
        c += ["--playlist-items", "1"]
    return c + ["--", objetivo]


def leer_linea(linea: str) -> dict | None:
    """Un evento a partir de una línea de yt-dlp."""
    linea = linea.rstrip("\n")
    if linea.startswith(MARCA + "A\t"):
        return {"tipo": "archivo", "ruta": linea.split("\t", 1)[1]}
    if linea.startswith(MARCA + "P\t"):
        partes = linea.split("\t")
        try:
            pct = float(partes[1].strip().rstrip("%"))
        except (IndexError, ValueError):
            pct = 0.0
        return {"tipo": "progreso", "porcentaje": pct, "titulo": partes[2] if len(partes) > 2 else ""}
    if linea.startswith("ERROR:"):
        return {"tipo": "aviso", "texto": _error_claro(linea[6:].strip())}
    return None


def _error_claro(e: str) -> str:
    if "Sign in to confirm" in e or "confirm you" in e:
        return "YouTube pide confirmar que no eres un bot: inicia sesión en tu navegador y elige ese navegador en Ajustes"
    if "Private video" in e or "private" in e.lower():
        return "Es privada: solo la puede ver su dueño"
    if "unavailable" in e.lower():
        return "Ya no está disponible en YouTube"
    if "cookies" in e.lower():
        return "No pude leer la sesión del navegador elegido (ciérralo y vuelve a intentar)"
    if "Unable to download" in e or "urlopen" in e or "timed out" in e:
        return "Sin conexión con YouTube"
    return e[:200]


def titulo_playlist(url: str, yt_dlp: str = "yt-dlp", navegador: str = "") -> tuple[str, int]:
    """Nombre y número de canciones de una playlist de YouTube (sin bajar nada)."""
    c = [yt_dlp, "--flat-playlist", "-J", "--no-warnings"]
    if navegador:
        c += ["--cookies-from-browser", navegador]
    try:
        r = subprocess.run(c + ["--", url], capture_output=True, text=True, timeout=60)
        j = json.loads(r.stdout or "{}")
    except (OSError, subprocess.TimeoutExpired, ValueError):
        return "", 0
    nombre = (j.get("title") or "").removeprefix("Album - ").strip()
    return nombre, len(j.get("entries") or [])


def _correr(c: list[str], parar: Callable[[], bool]) -> Iterator[dict]:
    try:
        proc = subprocess.Popen(c, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
    except FileNotFoundError:
        yield {"tipo": "aviso", "texto": "Falta yt-dlp: sudo pacman -S yt-dlp", "fatal": True}
        return
    assert proc.stdout is not None
    with proc.stdout:
        for linea in proc.stdout:
            if parar():
                proc.terminate()
                break
            ev = leer_linea(linea)
            if ev:
                yield ev
    proc.wait()


def descargar(url: str, destino: Path, navegador: str = "", yt_dlp: str = "yt-dlp",
              bajar: Callable[[str], str] = _bajar_texto, parar: Callable[[], bool] = lambda: False) -> Iterator[dict]:
    """Baja lo que haya en la URL. Eventos: inicio (nombre, total, es_lista), cancion (i, titulo),
    progreso, archivo (ruta, i), aviso (texto) y fin (bajadas, fallidas)."""
    t = tipo_de(url)
    if t is None:
        yield {"tipo": "aviso", "texto": "Ese enlace no es de YouTube, YouTube Music ni Spotify", "fatal": True}
        yield {"tipo": "fin", "bajadas": 0, "fallidas": 0}
        return
    servicio, clase, _id = t
    destino.mkdir(parents=True, exist_ok=True)
    bajadas = fallidas = 0

    if servicio == "spotify":
        try:
            nombre, pistas = pistas_spotify(clase, _id, bajar)
        except (OSError, ValueError, KeyError) as e:
            yield {"tipo": "aviso", "texto": f"No pude leer la lista de Spotify: {e}", "fatal": True}
            yield {"tipo": "fin", "bajadas": 0, "fallidas": 0}
            return
        yield {"tipo": "inicio", "nombre": nombre, "total": len(pistas), "es_lista": clase != "track"}
        for i, p in enumerate(pistas):
            if parar():
                break
            yield {"tipo": "cancion", "i": i, "titulo": f"{p['titulo']} · {p['artistas']}"}
            ruta = None
            for ev in _correr(comando(busqueda_ytmusic(p), destino, navegador, yt_dlp=yt_dlp), parar):
                if ev["tipo"] == "archivo":
                    ruta = ev["ruta"]
                    ev["i"] = i
                yield ev
            if ruta:
                bajadas += 1
            else:
                fallidas += 1
                yield {"tipo": "aviso", "texto": f"No encontré «{p['titulo']}» en YouTube Music"}
        yield {"tipo": "fin", "bajadas": bajadas, "fallidas": fallidas}
        return

    es_lista = clase == "playlist"
    nombre, total = titulo_playlist(url, yt_dlp, navegador) if es_lista else ("", 1)
    yield {"tipo": "inicio", "nombre": nombre, "total": total, "es_lista": es_lista}
    i = 0
    for ev in _correr(comando(url, destino, navegador, una=not es_lista, yt_dlp=yt_dlp), parar):
        if ev["tipo"] == "archivo":
            ev["i"] = i
            i += 1
            bajadas += 1
        yield ev
    if total and bajadas < total and not parar():
        fallidas = total - bajadas
    yield {"tipo": "fin", "bajadas": bajadas, "fallidas": fallidas}
