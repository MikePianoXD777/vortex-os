"""Los modelos de la voz y del oído de Miku: no vienen en la ISO, se bajan una sola vez al abrirla.

  voz   Supertonic 3 (Supertone, ONNX) y la voz F1: ~380 MB  → ~/.local/share/vortex/miku/voz/
  oido  Whisper small cuantizado (ggml, para whisper.cpp): ~181 MB → ~/.local/share/vortex/miku/oido/

Cada archivo tiene URL fija (a un commit, no a «main») y su sha256: lo que llega distinto de lo esperado
no se instala nunca. Se baja a un «.parte» y, si se corta la red, se reanuda donde se quedó. Un archivo
presente en su sitio ya fue comprobado al bajarlo, así que mirar si falta algo es instantáneo.

Solo biblioteca estándar: lo usan la app, «vortex-miku preparar» y «vortex-miku voz».

Para pruebas: VORTEX_MIKU_MANIFIESTO=ruta.json cambia la lista de archivos (mismo formato que PAQUETES);
como cada archivo sigue comprobándose con su sha256, no se abre ningún hueco.
"""

import hashlib
import json
import os
import shutil
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from . import rutas

_HF_VOZ = "https://huggingface.co/Supertone/supertonic-3/resolve/3cadd1ee6394adea1bd021217a0e650ede09a323"
_HF_OIDO = "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1"

# Orden de descarga: primero el oído (más chico) y luego la voz.
PAQUETES: dict[str, dict] = {
    "oido": {
        "nombre": "el oído",
        "archivos": [
            {"ruta": "ggml-small-q5_1.bin", "url": f"{_HF_OIDO}/ggml-small-q5_1.bin", "bytes": 190085487,
             "sha256": "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb"},
        ],
    },
    "voz": {
        "nombre": "la voz",
        "archivos": [
            {"ruta": "onnx/duration_predictor.onnx", "url": f"{_HF_VOZ}/onnx/duration_predictor.onnx", "bytes": 3700147,
             "sha256": "c3eb91414d5ff8a7a239b7fe9e34e7e2bf8a8140d8375ffb14718b1c639325db"},
            {"ruta": "onnx/text_encoder.onnx", "url": f"{_HF_VOZ}/onnx/text_encoder.onnx", "bytes": 36416150,
             "sha256": "c7befd5ea8c3119769e8a6c1486c4edc6a3bc8365c67621c881bbb774b9902ff"},
            {"ruta": "onnx/vector_estimator.onnx", "url": f"{_HF_VOZ}/onnx/vector_estimator.onnx", "bytes": 256534781,
             "sha256": "883ac868ea0275ef0e991524dc64f16b3c0376efd7c320af6b53f5b780d7c61c"},
            {"ruta": "onnx/vocoder.onnx", "url": f"{_HF_VOZ}/onnx/vocoder.onnx", "bytes": 101424195,
             "sha256": "085de76dd8e8d5836d6ca66826601f615939218f90e519f70ee8a36ed2a4c4ba"},
            {"ruta": "onnx/tts.json", "url": f"{_HF_VOZ}/onnx/tts.json", "bytes": 8253,
             "sha256": "42078d3aef1cd43ab43021f3c54f47d2d75ceb4e75f627f118890128b06a0d09"},
            {"ruta": "onnx/unicode_indexer.json", "url": f"{_HF_VOZ}/onnx/unicode_indexer.json", "bytes": 277676,
             "sha256": "9bf7346e43883a81f8645c81224f786d43c5b57f3641f6e7671a7d6c493cb24f"},
            {"ruta": "voice_styles/F1.json", "url": f"{_HF_VOZ}/voice_styles/F1.json", "bytes": 292046,
             "sha256": "bbdec6ee00231c2c742ad05483df5334cab3b52fda3ba38e6a07059c4563dbc2"},
            # La licencia viaja con los pesos (BigScience OpenRAIL-M lo exige a quien los reciba).
            {"ruta": "LICENSE", "url": f"{_HF_VOZ}/LICENSE", "bytes": 15007,
             "sha256": "0d944a9110fed9a9602d60e0423a272903e7bd21ab060490774efc77c2275e9f"},
        ],
    },
}

TAMANO_TOTAL_TEXTO = "~560 MB"        # lo que se le dice a la persona; son 563 MiB (590 MB) en total


class ErrorDescarga(Exception):
    """Algo falló al bajar; el mensaje está en español y dice qué hacer."""


def paquetes() -> dict[str, dict]:
    ruta = os.environ.get("VORTEX_MIKU_MANIFIESTO")
    if ruta:
        try:
            datos = json.loads(Path(ruta).read_text(encoding="utf-8"))
            if isinstance(datos, dict) and datos:
                return datos
        except (OSError, ValueError):
            pass
    return PAQUETES


def carpeta(paquete: str) -> Path:
    return rutas.DATOS / paquete


def archivos_faltantes(paquete: str) -> list[dict]:
    """Los archivos del paquete que aún no están en su sitio (o están a medias)."""
    base = carpeta(paquete)
    faltan = []
    for a in paquetes()[paquete]["archivos"]:
        destino = base / a["ruta"]
        try:
            if destino.is_file() and destino.stat().st_size == a["bytes"]:
                continue
        except OSError:
            pass
        faltan.append(a)
    return faltan


def paquete_listo(paquete: str) -> bool:
    return not archivos_faltantes(paquete)


def pendientes() -> list[str]:
    """Los paquetes (en orden) a los que les falta algo."""
    return [p for p in paquetes() if not paquete_listo(p)]


def bytes_pendientes(nombres: list[str] | None = None) -> int:
    return sum(a["bytes"] for p in (nombres or pendientes()) for a in archivos_faltantes(p))


def megas(n: float) -> str:
    """Tamaño para mostrar (en MiB, que es como lo cuenta el disco: la descarga entera son ~563)."""
    return f"{n / (1 << 20):.0f} MB"


# ---- Descarga ------------------------------------------------------------------------------------

class _RedirigirSoloSeguro(urllib.request.HTTPRedirectHandler):
    """Solo se sigue una redirección a https (o a esta misma máquina, para las pruebas)."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not _url_permitida(newurl):
            raise urllib.error.URLError(f"redirección no permitida a {urllib.parse.urlsplit(newurl).netloc}")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def _url_permitida(url: str) -> bool:
    p = urllib.parse.urlsplit(url)
    return p.scheme == "https" or (p.scheme == "http" and p.hostname in ("127.0.0.1", "localhost", "::1"))


_ABRIDOR = urllib.request.build_opener(_RedirigirSoloSeguro)


def sha256_de(ruta: Path) -> str:
    h = hashlib.sha256()
    with open(ruta, "rb") as f:
        for bloque in iter(lambda: f.read(1 << 20), b""):
            h.update(bloque)
    return h.hexdigest()


class Descarga:
    """Baja los paquetes que falten. `progreso(texto, fraccion)` se llama unas 4 veces por segundo
    y `al_paquete(nombre)` cada vez que uno termina.

    `cancelar` (un threading.Event) corta la descarga en cuanto puede; lo ya bajado queda en el
    «.parte» para reanudar la próxima vez.
    """

    def __init__(self, nombres: list[str] | None = None, progreso=None, cancelar: threading.Event | None = None, al_paquete=None):
        self.nombres = nombres if nombres is not None else pendientes()
        self.progreso = progreso or (lambda texto, fraccion: None)
        self.al_paquete = al_paquete or (lambda nombre: None)       # cada paquete que termina (para usarlo ya)
        self.cancelar = cancelar or threading.Event()
        self.total = bytes_pendientes(self.nombres)
        self.hecho = 0
        self._ultimo_aviso = 0.0

    def correr(self) -> None:
        """Baja todo lo que falte o lanza ErrorDescarga con un mensaje para mostrar."""
        self._hay_espacio()
        for p in self.nombres:
            base = carpeta(p)
            for a in archivos_faltantes(p):
                self._bajar(a, base / a["ruta"], paquetes()[p].get("nombre", p))
            self.al_paquete(p)
        self._avisar(forzar=True)

    def _hay_espacio(self) -> None:
        rutas.DATOS.mkdir(parents=True, exist_ok=True)
        libre = shutil.disk_usage(rutas.DATOS).free
        if libre < self.total + (200 << 20):
            raise ErrorDescarga(f"No hay espacio para bajar la voz de Miku: hacen falta {megas(self.total + (200 << 20))} libres "
                                f"y hay {megas(libre)}.")

    def _avisar(self, forzar: bool = False, que: str = "") -> None:
        ahora = time.monotonic()
        if not forzar and ahora - self._ultimo_aviso < 0.25:
            return
        self._ultimo_aviso = ahora
        fraccion = self.hecho / self.total if self.total else 1.0
        self.progreso(f"{que} {megas(self.hecho)} de {megas(self.total)}".strip(), min(1.0, fraccion))

    def _bajar(self, a: dict, destino: Path, que: str) -> None:
        if not _url_permitida(a["url"]):
            raise ErrorDescarga("La dirección de descarga no es segura; no se baja.")
        destino.parent.mkdir(parents=True, exist_ok=True)
        parte = destino.with_name(destino.name + ".parte")
        intentos = 0
        while True:
            try:
                self._intento(a, destino, parte, que)
                return
            except ErrorDescarga:
                raise
            except (urllib.error.URLError, OSError, TimeoutError, ConnectionError, ValueError) as e:
                if isinstance(e, urllib.error.HTTPError) and 400 <= e.code < 500 and e.code not in (408, 416, 429):
                    raise ErrorDescarga(f"No encuentro {que} de Miku en el servidor (error {e.code}). "
                                        "Mientras tanto te respondo solo por texto.") from e
                intentos += 1
                if self.cancelar.is_set():
                    raise ErrorDescarga("Se canceló la descarga.") from e
                if intentos >= 4:
                    raise ErrorDescarga(f"No pude bajar {que} de Miku ({_resumen_error(e)}). ¿Hay internet? "
                                        "Mientras tanto te respondo solo por texto; vuelvo a intentarlo la próxima vez.") from e
                time.sleep(min(2 * intentos, 5))

    def _intento(self, a: dict, destino: Path, parte: Path, que: str) -> None:
        tamano = parte.stat().st_size if parte.is_file() else 0
        if tamano > a["bytes"]:
            parte.unlink()
            tamano = 0
        cab = {"User-Agent": "vortex-miku (Vortexine's OS)"}
        if tamano:
            cab["Range"] = f"bytes={tamano}-"
        h = hashlib.sha256()
        if tamano:                                   # lo ya bajado también cuenta para el sha256
            with open(parte, "rb") as f:
                for bloque in iter(lambda: f.read(1 << 20), b""):
                    h.update(bloque)
        base = self.hecho
        try:
            with _ABRIDOR.open(urllib.request.Request(a["url"], headers=cab), timeout=25) as r:
                if tamano and r.status != 206:       # el servidor no entiende Range: se empieza de nuevo
                    tamano = 0
                    h = hashlib.sha256()
                with open(parte, "ab" if tamano else "wb") as f:
                    self.hecho = base + tamano
                    while True:
                        if self.cancelar.is_set():
                            raise ErrorDescarga("Se canceló la descarga.")
                        bloque = r.read(1 << 18)
                        if not bloque:
                            break
                        f.write(bloque)
                        h.update(bloque)
                        tamano += len(bloque)
                        self.hecho = base + tamano
                        if tamano > a["bytes"]:
                            raise ErrorDescarga(f"«{a['ruta']}» llegó más grande de lo esperado; no se instala.")
                        self._avisar(que="Bajando " + que)
        except urllib.error.HTTPError as e:
            if e.code == 416:                        # el .parte ya estaba completo o no sirve: se rehace
                parte.unlink(missing_ok=True)
            self.hecho = base
            raise
        except (urllib.error.URLError, OSError, TimeoutError, ConnectionError):
            self.hecho = base + (parte.stat().st_size if parte.is_file() else 0)    # lo bajado se conserva
            raise
        if tamano != a["bytes"]:
            self.hecho = base + tamano
            raise urllib.error.URLError(f"llegó incompleto ({tamano} de {a['bytes']} bytes)")
        if h.hexdigest() != a["sha256"]:
            parte.unlink(missing_ok=True)
            self.hecho = base
            raise ErrorDescarga(f"«{a['ruta']}» llegó distinto de lo esperado (sha256); no se instala. Vuelve a intentarlo más tarde.")
        os.replace(parte, destino)
        self.hecho = base + tamano


def _resumen_error(e: Exception) -> str:
    if isinstance(e, urllib.error.HTTPError):
        return f"el servidor respondió {e.code}"
    if isinstance(e, urllib.error.URLError):
        return str(e.reason)[:80]
    return type(e).__name__


def bajar_archivo(paquete_dest: Path, a: dict, progreso=None) -> None:
    """Un archivo suelto (p. ej. el estilo de una voz opcional) a `paquete_dest / a['ruta']`, con su sha256."""
    d = Descarga([], progreso)
    d.total = a["bytes"]
    destino = paquete_dest / a["ruta"]
    if destino.is_file() and destino.stat().st_size == a["bytes"]:
        return
    d._bajar(a, destino, "la voz")
