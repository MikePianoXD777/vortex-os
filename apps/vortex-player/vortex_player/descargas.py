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


# --- buscar (2.1) ----------------------------------------------------------------------------------
# Buscar sin tener el enlace. YouTube Music por su API web (la misma que usa su página, sin llave ni cuenta):
# en ~0,6 s trae título, artistas, álbum, duración y portada; la búsqueda de yt-dlp en YouTube Music solo
# da el título. Si YouTube Music cambia su respuesta, se cae a la de yt-dlp. YouTube (videos), con yt-dlp.

YTM_CLIENTE = {"clientName": "WEB_REMIX", "clientVersion": "1.20251020.01.00", "hl": "es", "gl": "MX"}
YTM_SOLO_CANCIONES = "EgWKAQIIAWoQEAMQBBAJEAoQBRAREBUQFQ%3D%3D"   # el filtro «Canciones» de su página


def _pedir_json(url: str, cuerpo: dict) -> dict:
    req = urllib.request.Request(url, data=json.dumps(cuerpo).encode(), headers={
        "Content-Type": "application/json", "User-Agent": AGENTE,
        "Origin": "https://music.youtube.com", "Referer": "https://music.youtube.com/"})
    with urllib.request.urlopen(req, timeout=20) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def _segundos(texto: str) -> int:
    """«3:34» o «1:02:03» → segundos (0 si no es una duración)."""
    if not re.fullmatch(r"\d{1,2}(:\d{2}){1,2}", texto.strip()):
        return 0
    s = 0
    for parte in texto.strip().split(":"):
        s = s * 60 + int(parte)
    return s


def _todos(o, clave: str) -> Iterator[dict]:
    if isinstance(o, dict):
        for k, v in o.items():
            if k == clave:
                yield v
            yield from _todos(v, clave)
    elif isinstance(o, list):
        for v in o:
            yield from _todos(v, clave)


def leer_ytmusic(respuesta: dict) -> list[dict]:
    """Las canciones de una respuesta de búsqueda de YouTube Music."""
    canciones = []
    for it in _todos(respuesta, "musicResponsiveListItemRenderer"):
        vid = (it.get("playlistItemData") or {}).get("videoId")
        columnas = [[r.get("text", "") for r in ((c.get("musicResponsiveListItemFlexColumnRenderer") or {})
                                                  .get("text") or {}).get("runs") or []]
                    for c in it.get("flexColumns") or []]
        if not vid or not columnas or not columnas[0]:
            continue
        # La segunda columna: «Artista • Álbum • 3:34» (con «Canción •» delante si la búsqueda no va filtrada)
        datos = [x.strip() for x in "".join(columnas[1] if len(columnas) > 1 else []).split("•")]
        datos = [x for x in datos if x and x not in ("Canción", "Song")]
        duracion = _segundos(datos[-1]) if datos else 0
        if duracion:
            datos = datos[:-1]
        fotos = (((it.get("thumbnail") or {}).get("musicThumbnailRenderer") or {}).get("thumbnail") or {}).get("thumbnails") or []
        portada = fotos[-1]["url"] if fotos else ""
        # Las portadas de YouTube Music vienen a 60 px: se piden a 226 (el tamaño de su lista grande)
        portada = re.sub(r"=w\d+-h\d+", "=w226-h226", portada)
        canciones.append({"id": vid, "url": f"https://music.youtube.com/watch?v={vid}", "titulo": "".join(columnas[0]),
                          "artistas": datos[0] if datos else "", "album": datos[1] if len(datos) > 1 else "",
                          "duracion": duracion, "portada": portada})
    return canciones


def leer_ytdlp(salida: str, musica: bool) -> list[dict]:
    """Los resultados de `yt-dlp --flat-playlist -J` de una búsqueda."""
    try:
        entradas = json.loads(salida or "{}").get("entries") or []
    except ValueError:
        return []
    canciones = []
    for e in entradas:
        vid = e.get("id") or ""
        if not re.fullmatch(r"[\w-]{11}", vid):
            continue   # canales y playlists que se cuelan en la búsqueda
        canciones.append({"id": vid, "url": (f"https://music.youtube.com/watch?v={vid}" if musica
                                             else f"https://www.youtube.com/watch?v={vid}"),
                          "titulo": e.get("title") or "", "artistas": e.get("channel") or e.get("uploader") or "",
                          "album": "", "duracion": int(e.get("duration") or 0),
                          "portada": f"https://i.ytimg.com/vi/{vid}/mqdefault.jpg"})
    return canciones


def buscar(texto: str, fuente: str = "ytmusic", n: int = 20, yt_dlp: str = "yt-dlp",
           pedir: Callable[[str, dict], dict] = _pedir_json) -> list[dict]:
    """Busca canciones en YouTube Music («ytmusic») o videos en YouTube («youtube»): [{id, url, titulo,
    artistas, album, duracion, portada}]. Lanza OSError si no hay conexión."""
    texto = " ".join(texto.split())
    if not texto:
        return []
    if fuente == "ytmusic":
        try:
            r = pedir("https://music.youtube.com/youtubei/v1/search?prettyPrint=false",
                      {"context": {"client": YTM_CLIENTE}, "query": texto,
                       "params": urllib.parse.unquote(YTM_SOLO_CANCIONES)})
            canciones = leer_ytmusic(r)
            if canciones:
                return canciones[:n]
        except ValueError:
            pass
        objetivo = "https://music.youtube.com/search?q=" + urllib.parse.quote(texto) + "#songs"
    else:
        objetivo = f"ytsearch{n}:{texto}"
    try:
        r = subprocess.run([yt_dlp, "--flat-playlist", "-J", "--no-warnings", "--playlist-items", f"1:{n}",
                            "--", objetivo], capture_output=True, text=True, timeout=60)
    except FileNotFoundError:
        raise OSError("Falta yt-dlp: sudo pacman -S yt-dlp") from None
    except subprocess.TimeoutExpired:
        raise OSError("YouTube tardó demasiado en contestar") from None
    if r.returncode != 0 and not r.stdout:
        raise OSError(_error_claro((r.stderr.strip().splitlines() or ["Sin conexión con YouTube"])[-1].removeprefix("ERROR:").strip()))
    return leer_ytdlp(r.stdout, fuente == "ytmusic")


# --- recomendaciones (2.1) -------------------------------------------------------------------------
# La «radio» de YouTube Music de una canción (lo que su página pone en «A continuación»): ~50 parecidas.
# Para una playlist se toman unas canciones de semilla, se intercalan sus radios y se quita lo que ya tienes.

def leer_radio(respuesta: dict) -> list[dict]:
    canciones = []
    for it in _todos(respuesta, "playlistPanelVideoRenderer"):
        vid = it.get("videoId")
        titulo = "".join(r.get("text", "") for r in (it.get("title") or {}).get("runs") or [])
        if not vid or not titulo:
            continue
        # «Artista • Álbum • 2023»
        datos = [x.strip() for x in "".join(r.get("text", "") for r in (it.get("longBylineText") or {}).get("runs") or []).split("•")]
        datos = [x for x in datos if x and not re.fullmatch(r"\d{4}", x)]
        fotos = (it.get("thumbnail") or {}).get("thumbnails") or []
        duracion = "".join(r.get("text", "") for r in (it.get("lengthText") or {}).get("runs") or [])
        canciones.append({"id": vid, "url": f"https://music.youtube.com/watch?v={vid}", "titulo": titulo,
                          "artistas": datos[0] if datos else "", "album": datos[1] if len(datos) > 1 else "",
                          "duracion": _segundos(duracion),
                          "portada": re.sub(r"=w\d+-h\d+", "=w226-h226", fotos[-1]["url"]) if fotos else ""})
    return canciones


def radio(video_id: str, pedir: Callable[[str, dict], dict] = _pedir_json) -> list[dict]:
    """Las canciones parecidas a esta (sin ella misma). Lanza OSError sin conexión."""
    try:
        r = pedir("https://music.youtube.com/youtubei/v1/next?prettyPrint=false",
                  {"context": {"client": YTM_CLIENTE}, "videoId": video_id, "playlistId": "RDAMVM" + video_id,
                   "isAudioOnly": True})
    except ValueError:
        return []
    return [c for c in leer_radio(r) if c["id"] != video_id]


def _clave(titulo: str, artista: str) -> str:
    """Para reconocer la misma canción aunque venga de otro lado: «idol (from oshi no ko)» ≈ «idol»."""
    t = re.sub(r"[\(\[].*?[\)\]]|\s-\s.*$", "", titulo.lower())
    return re.sub(r"\W+", "", t) + "|" + re.sub(r"\W+", "", artista.lower().split(",")[0])


def recomendar(semillas: list[dict], tienes: list[dict], n: int = 20,
               pedir: Callable[[str, dict], dict] = _pedir_json) -> list[dict]:
    """Recomendaciones para unas canciones de semilla ({id?, titulo, artista}): las radios de cada una,
    intercaladas, sin repetir y sin lo que ya tienes (`tienes`: {id?, titulo, artista}). Las semillas sin id
    (tus archivos) se buscan primero en YouTube Music."""
    ids = []
    for s in semillas:
        vid = s.get("id") or ""
        if not vid:
            try:
                encontradas = buscar(f"{s.get('artista', '')} {s.get('titulo', '')}", "ytmusic", 1, pedir=pedir)
            except OSError:
                encontradas = []
            vid = encontradas[0]["id"] if encontradas else ""
        if vid and vid not in ids:
            ids.append(vid)
    if not ids:
        return []
    radios, errores = [], []
    for vid in ids:
        try:
            radios.append(radio(vid, pedir))
        except OSError as e:
            errores.append(e)
    if not radios:
        raise errores[0] if errores else OSError("Sin conexión con YouTube")
    ya_ids = {t["id"] for t in tienes if t.get("id")} | set(ids)
    ya_claves = {_clave(t.get("titulo", ""), t.get("artista", "")) for t in tienes}
    salida, vistas = [], set()
    for fila in range(max(len(r) for r in radios)):
        for r in radios:
            if fila < len(r):
                c = r[fila]
                k = _clave(c["titulo"], c["artistas"])
                if c["id"] in ya_ids or k in ya_claves or c["id"] in vistas or k in vistas:
                    continue
                vistas |= {c["id"], k}
                salida.append(c)
                if len(salida) >= n:
                    return salida
    return salida


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
