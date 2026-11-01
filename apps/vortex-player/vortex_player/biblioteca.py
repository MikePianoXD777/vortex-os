"""La biblioteca: las canciones de tus carpetas, las bajadas y las playlists, en una base SQLite.

Las carpetas que se leen son Música (con Música/Vortex Player, donde quedan las bajadas) y las que
agregues. Cada archivo se lee una sola vez con ffprobe; si cambia (otra fecha de modificación) se
vuelve a leer, y si desaparece se quita. Las playlists guardan rutas, y las importadas recuerdan de
qué URL salieron para poder volver a sincronizarlas.
"""
from __future__ import annotations

import json
import os
import re
import sqlite3
import time
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import Callable, Iterable

from . import calidad, letras, nube, rutas

ESQUEMA = """
CREATE TABLE IF NOT EXISTS canciones (
    ruta TEXT PRIMARY KEY, mtime REAL, titulo TEXT, artista TEXT, album TEXT, pista INTEGER,
    duracion REAL, codec TEXT, bits INTEGER, hz INTEGER, kbps INTEGER, portada INTEGER,
    origen TEXT DEFAULT '', agregada REAL
);
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL, url TEXT DEFAULT '', creada REAL
);
CREATE TABLE IF NOT EXISTS playlist_canciones (
    playlist INTEGER REFERENCES playlists(id) ON DELETE CASCADE, pos INTEGER, ruta TEXT,
    PRIMARY KEY (playlist, pos)
);
CREATE TABLE IF NOT EXISTS borradas (tipo TEXT, clave TEXT, cuando REAL, PRIMARY KEY (tipo, clave));
CREATE TABLE IF NOT EXISTS escuchas (cuando REAL, ruta TEXT, titulo TEXT, artista TEXT, segundos REAL);
CREATE INDEX IF NOT EXISTS escuchas_cuando ON escuchas (cuando);
"""

# 2.4: «Tus me gusta» es una playlist con este id fijo en la nube, el mismo en la PC y en el teléfono
# (si cada uno le pusiera el suyo, al sincronizar saldrían dos)
ME_GUSTA = "me-gusta"
NOMBRE_ME_GUSTA = "Tus me gusta"

# Lo bajado de YouTube lleva su id en el nombre: «Título [id].opus»
ID_EN_RUTA = re.compile(r"\[([\w-]{11})\]\.\w+$")
# 1.4: las de la radio suenan desde YouTube Music: «https://music.youtube.com/watch?v=id»
ID_EN_URL = re.compile(r"^https?://.*[?&]v=([\w-]{11})")


def id_de(ruta: str) -> str:
    m = ID_EN_RUTA.search(ruta) or ID_EN_URL.search(ruta)
    return m.group(1) if m else ""


def _ms(t: float | None) -> int:
    return int((t or 0) * 1000)

AJUSTES_BASE = {
    "carpetas": [],             # además de Música
    "navegador_premium": "",    # «firefox», «chrome»…: baja con tu sesión de YouTube Music Premium
    "volumen": 80,
    "ldac_hq": True,            # pedir LDAC a 990 kbps cuando hay audífonos que lo soportan
    "fundido": 5,               # 2.4: segundos de crossfade entre canciones (0 = sin)
    "radio": True,              # 1.4: al acabarse la cola siguen canciones parecidas (radio sin fin)
}


def semana(atras: int = 0, ahora: float | None = None) -> tuple[float, float]:
    """1.4: de lunes a lunes (a la medianoche de aquí): 0 = esta semana, 1 = la pasada."""
    import datetime as dt
    hoy = dt.datetime.fromtimestamp(ahora or time.time()).replace(hour=0, minute=0, second=0, microsecond=0)
    lunes = hoy - dt.timedelta(days=hoy.weekday() + 7 * atras)
    return lunes.timestamp(), (lunes + dt.timedelta(days=7)).timestamp()


def origen_de(ruta: str, extra: dict | None = None) -> str:
    """«youtube» si la bajó Vortex Player de YouTube o YouTube Music; si no, «local»."""
    texto = " ".join((extra or {}).values())
    if "youtube.com" in texto or "youtu.be" in texto:
        return "youtube"
    try:
        if Path(ruta).resolve().is_relative_to(rutas.descargas().resolve()):
            return "youtube"
    except OSError:
        pass
    return "local"


class Biblioteca:
    def __init__(self, archivo: Path | str | None = None, ffprobe: str = "ffprobe"):
        self.archivo = Path(archivo) if archivo else rutas.datos() / "biblioteca.db"
        self.archivo.parent.mkdir(parents=True, exist_ok=True)
        self.ffprobe = ffprobe
        self.db = sqlite3.connect(self.archivo, check_same_thread=False)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA foreign_keys = ON")
        self.db.executescript(ESQUEMA)
        # 2.4: la nube. Cada playlist lleva su id en la nube y cuándo cambió
        columnas = {r["name"] for r in self.db.execute("PRAGMA table_info(playlists)")}
        if "nube" not in columnas:
            self.db.execute("ALTER TABLE playlists ADD COLUMN nube TEXT DEFAULT ''")
        if "modificada" not in columnas:
            self.db.execute("ALTER TABLE playlists ADD COLUMN modificada REAL DEFAULT 0")
            self.db.execute("UPDATE playlists SET modificada = creada")
        self.db.commit()

    def cerrar(self) -> None:
        self.db.close()

    # --- 1.4: «Tu resumen» (lo que escuchaste cada semana) ---------------------------------------------

    def anotar_escucha(self, c: dict, segundos: float, cuando: float | None = None) -> None:
        self.db.execute("INSERT INTO escuchas VALUES (?, ?, ?, ?, ?)",
                        (cuando or time.time(), c.get("ruta", ""), c.get("titulo") or Path(c.get("ruta", "")).stem,
                         c.get("artista", ""), float(segundos)))
        self.db.commit()

    def mas_escuchadas(self, desde: float, n: int = 15) -> list[dict]:
        """1.5: tus canciones (las que siguen en tu biblioteca) que más sonaron desde `desde`."""
        filas = self.db.execute(
            "SELECT e.ruta, COUNT(*) AS veces FROM escuchas e JOIN canciones c ON c.ruta = e.ruta "
            "WHERE e.cuando >= ? GROUP BY e.ruta ORDER BY veces DESC, MAX(e.cuando) DESC LIMIT ?", (desde, n)).fetchall()
        return [c for f in filas if (c := self.cancion(f["ruta"]))]

    def artistas_mas_escuchados(self, desde: float, n: int = 5) -> list[str]:
        """1.5: tus artistas de los últimos días; sin escuchas, los que más tienes en tu biblioteca."""
        cuenta: dict[str, list] = {}
        filas = self.db.execute("SELECT artista FROM escuchas WHERE cuando >= ?", (desde,)).fetchall()
        if not filas:
            filas = self.db.execute("SELECT artista FROM canciones").fetchall()
        for f in filas:
            a = letras.limpiar_artista(f["artista"] or "")
            if a:
                cuenta.setdefault(a.lower(), [a, 0])[1] += 1
        return [a for a, _ in sorted(cuenta.values(), key=lambda x: -x[1])[:n]]

    def resumen(self, desde: float, hasta: float, n: int = 5) -> dict:
        """Lo que escuchaste entre `desde` y `hasta`: minutos, cuántas, cuántos días y lo más escuchado."""
        filas = self.db.execute("SELECT * FROM escuchas WHERE cuando >= ? AND cuando < ?", (desde, hasta)).fetchall()
        canciones: dict[tuple, dict] = {}
        artistas: dict[str, dict] = {}
        dias = set()
        for f in filas:
            dias.add(time.strftime("%Y-%m-%d", time.localtime(f["cuando"])))
            nombre = letras.limpiar_artista(f["artista"] or "")   # «YOASOBI - Topic» es «YOASOBI»
            k = (f["titulo"].lower(), nombre.lower())
            c = canciones.setdefault(k, {"titulo": f["titulo"], "artista": nombre or f["artista"] or "", "ruta": f["ruta"],
                                         "veces": 0, "minutos": 0.0})
            c["veces"] += 1
            c["minutos"] += f["segundos"] / 60
            if nombre:
                a = artistas.setdefault(nombre.lower(), {"artista": nombre, "veces": 0, "minutos": 0.0, "ruta": f["ruta"]})
                a["veces"] += 1
                a["minutos"] += f["segundos"] / 60
        orden = lambda x: (-x["veces"], -x["minutos"])
        top = sorted(canciones.values(), key=orden)[:n]
        top_a = sorted(artistas.values(), key=orden)[:n]
        for x in top + top_a:
            x["minutos"] = round(x["minutos"])
        return {"desde": desde, "hasta": hasta, "minutos": round(sum(f["segundos"] for f in filas) / 60),
                "escuchas": len(filas), "distintas": len(canciones), "dias": len(dias),
                "canciones": top, "artistas": top_a}

    # --- ajustes ---------------------------------------------------------------------------------

    @staticmethod
    def ajustes() -> dict:
        try:
            guardados = json.loads((rutas.config() / "ajustes.json").read_text())
        except (OSError, ValueError):
            guardados = {}
        return {**AJUSTES_BASE, **(guardados if isinstance(guardados, dict) else {})}

    @staticmethod
    def guardar_ajuste(clave: str, valor) -> None:
        a = Biblioteca.ajustes()
        a[clave] = valor
        rutas.config().mkdir(parents=True, exist_ok=True)
        tmp = rutas.config() / "ajustes.json.tmp"
        tmp.write_text(json.dumps(a, indent=2, ensure_ascii=False))
        tmp.replace(rutas.config() / "ajustes.json")

    def carpetas(self) -> list[Path]:
        lista = [rutas.musica()] + [Path(c) for c in self.ajustes()["carpetas"]]
        vistas: list[Path] = []
        for c in lista:
            if c not in vistas:
                vistas.append(c)
        return vistas

    # --- leer las carpetas -----------------------------------------------------------------------

    @staticmethod
    def archivos_de(carpetas: Iterable[Path]) -> Iterable[Path]:
        for carpeta in carpetas:
            if carpeta.is_file():
                if carpeta.suffix.lower() in calidad.EXTENSIONES:
                    yield carpeta
                continue
            for raiz, dirs, nombres in os.walk(carpeta):
                dirs[:] = [d for d in dirs if not d.startswith(".")]
                for n in nombres:
                    if Path(n).suffix.lower() in calidad.EXTENSIONES and not n.startswith("."):
                        yield Path(raiz) / n

    def agregar_archivo(self, ruta: Path, origen: str = "") -> calidad.Info | None:
        """Lee un archivo y lo guarda (o lo actualiza). None si no es audio."""
        try:
            mtime = ruta.stat().st_mtime
        except OSError:
            return None
        info = calidad.leer(ruta, self.ffprobe)
        if info is None:
            return None
        previa = self.db.execute("SELECT agregada, origen FROM canciones WHERE ruta = ?", (str(ruta),)).fetchone()
        if previa is None and (vid := id_de(str(ruta))):
            self.db.execute("DELETE FROM borradas WHERE tipo = 'cancion' AND clave = ?", (vid,))   # la volviste a bajar
        self.db.execute(
            "INSERT OR REPLACE INTO canciones VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (str(ruta), mtime, info.titulo, info.artista, info.album, info.pista, info.duracion, info.codec,
             info.bits, info.hz, info.kbps, int(info.portada),
             origen or (previa["origen"] if previa and previa["origen"] else origen_de(str(ruta), info.extra)),
             previa["agregada"] if previa else time.time()))
        self.db.commit()
        return info

    def escanear(self, al_avanzar: Callable[[int, int], None] | None = None) -> dict[str, int]:
        """Pone la base al día con las carpetas: nuevas, cambiadas y borradas."""
        conocidas = {r["ruta"]: r["mtime"] for r in self.db.execute("SELECT ruta, mtime FROM canciones")}
        encontradas = list(self.archivos_de(self.carpetas()))
        nuevas = cambiadas = 0
        for i, ruta in enumerate(encontradas):
            try:
                mtime = ruta.stat().st_mtime
            except OSError:
                continue
            antes = conocidas.get(str(ruta))
            if antes is None or abs(antes - mtime) > 0.5:
                if self.agregar_archivo(ruta):
                    nuevas += antes is None
                    cambiadas += antes is not None
            if al_avanzar:
                al_avanzar(i + 1, len(encontradas))
        vistas = {str(r) for r in encontradas}
        borradas = [r for r in conocidas if r not in vistas and not Path(r).exists()]
        self.db.executemany("DELETE FROM canciones WHERE ruta = ?", [(r,) for r in borradas])
        self.db.commit()
        return {"nuevas": nuevas, "cambiadas": cambiadas, "borradas": len(borradas), "total": len(vistas)}

    # --- consultas -------------------------------------------------------------------------------

    @staticmethod
    def _fila(r: sqlite3.Row) -> dict:
        d = dict(r)
        d["etiqueta"] = calidad.etiqueta(d["codec"] or "", d["bits"] or 0, d["hz"] or 0, d["kbps"] or 0)
        d["nivel"] = calidad.nivel(d["codec"] or "", d["bits"] or 0, d["hz"] or 0)
        # 2.4: lo bajado de YouTube se ve con su título limpio («アイドル», no «YOASOBI - アイドル (Official…)»)
        if id_de(d["ruta"]):
            d["titulo"] = letras.titulo_limpio(d["titulo"] or "", d["artista"] or "")
            d["artista"] = letras.artista_limpio(d["artista"] or "")
        return d

    def canciones(self, buscar: str = "", orden: str = "agregada") -> list[dict]:
        orden_sql = {"agregada": "agregada DESC", "titulo": "titulo COLLATE NOCASE",
                     "artista": "artista COLLATE NOCASE, album COLLATE NOCASE, pista",
                     "calidad": "(codec IN ('flac','alac','wavpack','ape') OR codec LIKE 'pcm_%') DESC, bits DESC, hz DESC, kbps DESC"
                     }.get(orden, "agregada DESC")
        sql = "SELECT * FROM canciones"
        args: tuple = ()
        if buscar.strip():
            sql += " WHERE titulo LIKE ? OR artista LIKE ? OR album LIKE ?"
            q = f"%{buscar.strip()}%"
            args = (q, q, q)
        return [self._fila(r) for r in self.db.execute(f"{sql} ORDER BY {orden_sql}", args)]

    def cancion(self, ruta: str) -> dict | None:
        r = self.db.execute("SELECT * FROM canciones WHERE ruta = ?", (ruta,)).fetchone()
        return self._fila(r) if r else None

    def quitar(self, ruta: str) -> None:
        """Solo de la biblioteca (el archivo no se toca; se vuelve a ver si sigue en una carpeta)."""
        self.db.execute("DELETE FROM canciones WHERE ruta = ?", (ruta,))
        self.db.execute("DELETE FROM playlist_canciones WHERE ruta = ?", (ruta,))
        self.db.commit()

    # --- playlists -------------------------------------------------------------------------------

    def playlists(self) -> list[dict]:
        """«Tus me gusta» siempre primero (2.4), luego la más nueva."""
        return [{**dict(r), "meGusta": r["nube"] == ME_GUSTA} for r in self.db.execute(
            "SELECT p.id, p.nombre, p.url, p.nube, COUNT(c.ruta) AS canciones FROM playlists p "
            "LEFT JOIN playlist_canciones c ON c.playlist = p.id GROUP BY p.id "
            "ORDER BY p.nube = ? DESC, p.creada DESC", (ME_GUSTA,))]

    # --- 2.4: «Tus me gusta» ------------------------------------------------------------------------

    def id_me_gusta(self, crear: bool = False) -> int:
        """La playlist de «Tus me gusta» (-1 si todavía no le has dado ❤️ a nada)."""
        fila = self.db.execute("SELECT id FROM playlists WHERE nube = ?", (ME_GUSTA,)).fetchone()
        if fila or not crear:
            return fila["id"] if fila else -1
        ahora = time.time()
        cur = self.db.execute("INSERT INTO playlists (nombre, url, creada, modificada, nube) VALUES (?,?,?,?,?)",
                              (NOMBRE_ME_GUSTA, "", ahora, ahora, ME_GUSTA))
        self.db.commit()
        return int(cur.lastrowid)

    def me_gusta(self) -> list[str]:
        pid = self.id_me_gusta()
        return [] if pid < 0 else [r for (r,) in self.db.execute(
            "SELECT ruta FROM playlist_canciones WHERE playlist = ? ORDER BY pos", (pid,))]

    def alternar_me_gusta(self, ruta: str) -> bool:
        """❤️ o ya no. Las nuevas van hasta arriba, como en Spotify. Devuelve si ahora te gusta."""
        pid = self.id_me_gusta(crear=True)
        if self.db.execute("DELETE FROM playlist_canciones WHERE playlist = ? AND ruta = ?", (pid, ruta)).rowcount:
            gusta = False
        else:
            pos = self.db.execute("SELECT COALESCE(MIN(pos), 1) - 1 FROM playlist_canciones WHERE playlist = ?",
                                  (pid,)).fetchone()[0]
            self.db.execute("INSERT INTO playlist_canciones VALUES (?,?,?)", (pid, pos, ruta))
            gusta = True
        self._cambio(pid)
        self.db.commit()
        return gusta

    def crear_playlist(self, nombre: str, url: str = "") -> int:
        if url:
            previa = self.db.execute("SELECT id FROM playlists WHERE url = ?", (url,)).fetchone()
            if previa:
                return previa["id"]
        ahora = time.time()
        cur = self.db.execute("INSERT INTO playlists (nombre, url, creada, modificada) VALUES (?,?,?,?)",
                              (nombre.strip() or "Playlist", url, ahora, ahora))
        self.db.commit()
        return int(cur.lastrowid)

    def _cambio(self, pid: int) -> None:
        """Marca la playlist como cambiada aquí (en la próxima sincronización gana esta versión)."""
        self.db.execute("UPDATE playlists SET modificada = ? WHERE id = ?", (time.time(), pid))

    def renombrar_playlist(self, pid: int, nombre: str) -> None:
        self.db.execute("UPDATE playlists SET nombre = ? WHERE id = ?", (nombre.strip() or "Playlist", pid))
        self._cambio(pid)
        self.db.commit()

    def borrar_playlist(self, pid: int, anotar: bool = True) -> None:
        """Con `anotar`, queda anotada para que también se borre de la nube y del teléfono."""
        fila = self.db.execute("SELECT nube FROM playlists WHERE id = ?", (pid,)).fetchone()
        if anotar and fila and fila["nube"]:
            self._anotar_borrada("playlist", fila["nube"])
        self.db.execute("DELETE FROM playlists WHERE id = ?", (pid,))
        self.db.commit()

    def agregar_a_playlist(self, pid: int, ruta: str) -> None:
        if self.db.execute("SELECT 1 FROM playlist_canciones WHERE playlist = ? AND ruta = ?", (pid, ruta)).fetchone():
            return
        pos = self.db.execute("SELECT COALESCE(MAX(pos), -1) + 1 FROM playlist_canciones WHERE playlist = ?",
                              (pid,)).fetchone()[0]
        self.db.execute("INSERT INTO playlist_canciones VALUES (?,?,?)", (pid, pos, ruta))
        self._cambio(pid)
        self.db.commit()

    def quitar_de_playlist(self, pid: int, ruta: str) -> None:
        if self.db.execute("DELETE FROM playlist_canciones WHERE playlist = ? AND ruta = ?", (pid, ruta)).rowcount:
            self._cambio(pid)
        self.db.commit()

    def canciones_de_playlist(self, pid: int) -> list[dict]:
        return [self._fila(r) for r in self.db.execute(
            "SELECT c.* FROM playlist_canciones p JOIN canciones c ON c.ruta = p.ruta "
            "WHERE p.playlist = ? ORDER BY p.pos", (pid,))]


    def primeras_de_playlist(self, pid: int, n: int = 4) -> list[str]:
        """Las rutas de las primeras canciones (para el mosaico de la portada, 2.4)."""
        return [r[0] for r in self.db.execute(
            "SELECT p.ruta FROM playlist_canciones p JOIN canciones c ON c.ruta = p.ruta WHERE p.playlist = ? "
            "ORDER BY p.pos LIMIT ?", (pid, n))]

    # --- la nube (2.4) ----------------------------------------------------------------------------

    def _anotar_borrada(self, tipo: str, clave: str) -> None:
        self.db.execute("INSERT OR REPLACE INTO borradas VALUES (?,?,?)", (tipo, clave, time.time()))

    def anotar_cancion_borrada(self, vid: str) -> None:
        """La borraste tú: que también se quite de la nube y del teléfono."""
        if vid:
            self._anotar_borrada("cancion", vid)
            self.db.commit()

    def _borradas(self, tipo: str) -> dict[str, int]:
        return {r["clave"]: _ms(r["cuando"]) for r in self.db.execute("SELECT clave, cuando FROM borradas WHERE tipo = ?", (tipo,))}

    def rutas_por_id(self) -> dict[str, str]:
        """id de YouTube → ruta, de todo lo que hay en la biblioteca."""
        return {vid: r for (r,) in self.db.execute("SELECT ruta FROM canciones ORDER BY agregada") if (vid := id_de(r))}

    def estado_nube(self) -> nube.Local:
        """Lo que hay aquí, como lo entiende la nube. A las playlists sin id en la nube se les da uno."""
        local = nube.Local(quitadas=self._borradas("cancion"), borradas=self._borradas("playlist"))
        for r in self.db.execute("SELECT * FROM canciones"):
            if vid := id_de(r["ruta"]):
                local.canciones[vid] = {"titulo": r["titulo"] or "", "artista": r["artista"] or "", "album": r["album"] or "",
                                        "duracion": int(r["duracion"] or 0), "agregada": _ms(r["agregada"])}
        for p in self.db.execute("SELECT * FROM playlists").fetchall():
            u = p["nube"]
            if not u:
                u = str(uuid.uuid4())
                self.db.execute("UPDATE playlists SET nube = ? WHERE id = ?", (u, p["id"]))
            ids: list[str] = []
            for (ruta,) in self.db.execute("SELECT ruta FROM playlist_canciones WHERE playlist = ? ORDER BY pos", (p["id"],)):
                if (vid := id_de(ruta)) and vid not in ids:
                    ids.append(vid)
            local.playlists[u] = {"nombre": p["nombre"], "url": p["url"] or "", "canciones": ids,
                                  "modificada": _ms(p["modificada"])}
        self.db.commit()
        return local

    def poner_playlist(self, u: str, p: dict, ruta_de_id: dict[str, str]) -> None:
        """Deja la playlist como viene de la nube; las canciones de aquí que no son de YouTube se quedan al final."""
        fila = self.db.execute("SELECT id FROM playlists WHERE nube = ?", (u,)).fetchone()
        if fila:
            pid = fila["id"]
        else:
            pid = self.db.execute("INSERT INTO playlists (nombre, url, creada, nube) VALUES (?,?,?,?)",
                                  (p.get("nombre") or "Playlist", p.get("url", ""), time.time(), u)).lastrowid
        locales = [r for (r,) in self.db.execute("SELECT ruta FROM playlist_canciones WHERE playlist = ? ORDER BY pos", (pid,))
                   if not id_de(r)]
        rutas_nuevas: list[str] = []
        for r in [ruta_de_id[v] for v in p.get("canciones", []) if v in ruta_de_id] + locales:
            if r not in rutas_nuevas:
                rutas_nuevas.append(r)
        self.db.execute("UPDATE playlists SET nombre = ?, url = ?, modificada = ? WHERE id = ?",
                        (p.get("nombre") or "Playlist", p.get("url", ""), p.get("modificada", 0) / 1000, pid))
        self.db.execute("DELETE FROM playlist_canciones WHERE playlist = ?", (pid,))
        self.db.executemany("INSERT INTO playlist_canciones VALUES (?,?,?)", [(pid, i, r) for i, r in enumerate(rutas_nuevas)])
        self.db.commit()

    def quitar_playlist_nube(self, u: str) -> None:
        fila = self.db.execute("SELECT id FROM playlists WHERE nube = ?", (u,)).fetchone()
        if fila:
            self.borrar_playlist(fila["id"], anotar=False)


def info_a_dict(info: calidad.Info) -> dict:
    d = asdict(info)
    d["etiqueta"], d["nivel"] = info.etiqueta, info.nivel
    return d
