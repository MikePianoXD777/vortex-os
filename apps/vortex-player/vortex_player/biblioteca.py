"""La biblioteca: las canciones de tus carpetas, las bajadas y las playlists, en una base SQLite.

Las carpetas que se leen son Música (con Música/Vortex Player, donde quedan las bajadas) y las que
agregues. Cada archivo se lee una sola vez con ffprobe; si cambia (otra fecha de modificación) se
vuelve a leer, y si desaparece se quita. Las playlists guardan rutas, y las importadas recuerdan de
qué URL salieron para poder volver a sincronizarlas.
"""
from __future__ import annotations

import json
import os
import sqlite3
import time
from dataclasses import asdict
from pathlib import Path
from typing import Callable, Iterable

from . import calidad, rutas

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
"""

AJUSTES_BASE = {
    "carpetas": [],             # además de Música
    "navegador_premium": "",    # «firefox», «chrome»…: baja con tu sesión de YouTube Music Premium
    "volumen": 80,
    "ldac_hq": True,            # pedir LDAC a 990 kbps cuando hay audífonos que lo soportan
}


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
        self.db.commit()

    def cerrar(self) -> None:
        self.db.close()

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
        return [dict(r) for r in self.db.execute(
            "SELECT p.id, p.nombre, p.url, COUNT(c.ruta) AS canciones FROM playlists p "
            "LEFT JOIN playlist_canciones c ON c.playlist = p.id GROUP BY p.id ORDER BY p.creada DESC")]

    def crear_playlist(self, nombre: str, url: str = "") -> int:
        if url:
            previa = self.db.execute("SELECT id FROM playlists WHERE url = ?", (url,)).fetchone()
            if previa:
                return previa["id"]
        cur = self.db.execute("INSERT INTO playlists (nombre, url, creada) VALUES (?,?,?)",
                              (nombre.strip() or "Playlist", url, time.time()))
        self.db.commit()
        return int(cur.lastrowid)

    def renombrar_playlist(self, pid: int, nombre: str) -> None:
        self.db.execute("UPDATE playlists SET nombre = ? WHERE id = ?", (nombre.strip() or "Playlist", pid))
        self.db.commit()

    def borrar_playlist(self, pid: int) -> None:
        self.db.execute("DELETE FROM playlists WHERE id = ?", (pid,))
        self.db.commit()

    def agregar_a_playlist(self, pid: int, ruta: str) -> None:
        if self.db.execute("SELECT 1 FROM playlist_canciones WHERE playlist = ? AND ruta = ?", (pid, ruta)).fetchone():
            return
        pos = self.db.execute("SELECT COALESCE(MAX(pos), -1) + 1 FROM playlist_canciones WHERE playlist = ?",
                              (pid,)).fetchone()[0]
        self.db.execute("INSERT INTO playlist_canciones VALUES (?,?,?)", (pid, pos, ruta))
        self.db.commit()

    def quitar_de_playlist(self, pid: int, ruta: str) -> None:
        self.db.execute("DELETE FROM playlist_canciones WHERE playlist = ? AND ruta = ?", (pid, ruta))
        self.db.commit()

    def canciones_de_playlist(self, pid: int) -> list[dict]:
        return [self._fila(r) for r in self.db.execute(
            "SELECT c.* FROM playlist_canciones p JOIN canciones c ON c.ruta = p.ruta "
            "WHERE p.playlist = ? ORDER BY p.pos", (pid,))]


def info_a_dict(info: calidad.Info) -> dict:
    d = asdict(info)
    d["etiqueta"], d["nivel"] = info.etiqueta, info.nivel
    return d
