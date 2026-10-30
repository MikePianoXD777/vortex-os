"""Las cámaras y las ventanas del mosaico, y las direcciones RTSP de cada una.

Todo se guarda en ~/.config/vortex-camera/camaras.json con permisos 600 (solo tú lo lees), porque ahí van
los códigos de seguridad. Cada «ventana» es una pestaña del mosaico, como «Window 1» y «Window 2» de
Imou Life: su división (1, 4, 9, 16, 25 o 32 cuadros) y qué cámara va en cada cuadro.

Las cámaras Imou son Dahua por dentro: el usuario es siempre «admin» y la contraseña, el código de
seguridad de la etiqueta. Video en vivo y grabado de la microSD:
    rtsp://admin:<código>@<ip>:554/cam/realmonitor?channel=1&subtype=0     (1 = subflujo, el ligero)
    rtsp://admin:<código>@<ip>:554/cam/playback?channel=1&starttime=AAAA_MM_DD_hh_mm_ss&endtime=…

Para pruebas: VORTEX_CAMERA_CONFIG=/otra/carpeta cambia dónde se guarda.
"""
from __future__ import annotations

import json
import os
import tempfile
import uuid
from dataclasses import asdict, dataclass, fields
from datetime import datetime
from pathlib import Path
from urllib.parse import quote

DIVISIONES = (1, 4, 9, 16, 25, 32)   # hasta 32 cámaras por ventana


def carpeta_config() -> Path:
    if os.environ.get("VORTEX_CAMERA_CONFIG"):
        return Path(os.environ["VORTEX_CAMERA_CONFIG"])
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "vortex-camera"


def limpiar_serie(serie: str) -> str:
    """« 9a0 b-1234 » → «9A0B1234»: así se escribe en la etiqueta y así la devuelve la cámara."""
    return "".join(ch for ch in serie.upper() if ch.isalnum())


@dataclass
class Camara:
    id: str
    nombre: str
    serie: str = ""
    codigo: str = ""          # el código de seguridad (Safety Code / SC) de la etiqueta
    ip: str = ""
    puerto: int = 554
    canal: int = 1
    usuario: str = "admin"
    ptz: str = ""             # cómo se mueve: «onvif», «cgi», «no» o «» (sin probar)

    @classmethod
    def desde(cls, datos: dict) -> "Camara":
        validos = {f.name for f in fields(cls)}
        c = cls(**{k: v for k, v in datos.items() if k in validos})
        c.nombre = str(c.nombre).strip() or "Cámara"
        c.serie = limpiar_serie(str(c.serie))
        c.codigo = str(c.codigo).strip()
        c.ip = str(c.ip).strip()
        c.usuario = str(c.usuario).strip() or "admin"
        try:
            c.puerto = int(c.puerto) if 0 < int(c.puerto) < 65536 else 554
        except (TypeError, ValueError):
            c.puerto = 554
        try:
            c.canal = max(1, int(c.canal))
        except (TypeError, ValueError):
            c.canal = 1
        return c

    def _base(self) -> str:
        cred = f"{quote(self.usuario, safe='')}:{quote(self.codigo, safe='')}@" if self.codigo else ""
        host = f"[{self.ip}]" if ":" in self.ip else self.ip
        return f"rtsp://{cred}{host}:{self.puerto}"

    def url_vivo(self, principal: bool = False) -> str:
        """El video en vivo. El subflujo (principal=False) es el ligero, para el mosaico."""
        if not self.ip:
            return ""
        return f"{self._base()}/cam/realmonitor?channel={self.canal}&subtype={0 if principal else 1}"

    def url_grabado(self, inicio: datetime, fin: datetime) -> str:
        """Lo que la cámara guardó en su microSD, de `inicio` a `fin` (hora de la cámara)."""
        if not self.ip:
            return ""
        f = "%Y_%m_%d_%H_%M_%S"
        return (f"{self._base()}/cam/playback?channel={self.canal}"
                f"&starttime={inicio.strftime(f)}&endtime={fin.strftime(f)}")

    def publica(self) -> dict:
        """Para la interfaz: todo menos el código (QML pide las URL al puente cuando las necesita)."""
        d = asdict(self)
        d["tieneCodigo"] = bool(d.pop("codigo"))
        return d


class Almacen:
    """Las cámaras y las ventanas, en memoria y en el archivo."""

    def __init__(self, carpeta: Path | str | None = None):
        self.ruta = Path(carpeta or carpeta_config()) / "camaras.json"
        self.camaras: list[Camara] = []
        self.ventanas: list[dict] = []
        self.cargar()

    # --- archivo ---

    def cargar(self) -> None:
        try:
            datos = json.loads(self.ruta.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            datos = {}
        if not isinstance(datos, dict):
            datos = {}
        self.camaras = []
        for c in datos.get("camaras") or []:
            if isinstance(c, dict) and c.get("id"):
                self.camaras.append(Camara.desde(c))
        ids = {c.id for c in self.camaras}
        self.ventanas = []
        for v in datos.get("ventanas") or []:
            if not isinstance(v, dict):
                continue
            division = v.get("division") if v.get("division") in DIVISIONES else 4
            celdas = [x if isinstance(x, str) and x in ids else "" for x in (v.get("celdas") or [])]
            self.ventanas.append({"nombre": str(v.get("nombre") or "Ventana"), "division": division, "celdas": celdas})
        if not self.ventanas:
            self.ventanas = [self._ventana_nueva(1)]
        for v in self.ventanas:
            self._rellenar(v)

    def guardar(self) -> None:
        """Escribe completo en un archivo temporal (600) y lo cambia por el bueno: nunca queda a medias."""
        self.ruta.parent.mkdir(parents=True, exist_ok=True)
        os.chmod(self.ruta.parent, 0o700)
        datos = {"version": 1, "camaras": [asdict(c) for c in self.camaras], "ventanas": self.ventanas}
        fd, tmp = tempfile.mkstemp(prefix=".camaras-", dir=self.ruta.parent)
        try:
            os.fchmod(fd, 0o600)
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(datos, f, ensure_ascii=False, indent=2)
            os.replace(tmp, self.ruta)
        except BaseException:
            Path(tmp).unlink(missing_ok=True)
            raise

    # --- cámaras ---

    def camara(self, ident: str) -> Camara | None:
        return next((c for c in self.camaras if c.id == ident), None)

    def por_serie(self, serie: str) -> Camara | None:
        serie = limpiar_serie(serie)
        return next((c for c in self.camaras if serie and c.serie == serie), None)

    def agregar(self, datos: dict) -> Camara:
        """La agrega y la pone en el primer cuadro libre: si la ventana se llena, crece (hasta 32 cuadros)
        y, si ya no puede, se abre otra ventana."""
        c = Camara.desde({**datos, "id": uuid.uuid4().hex[:10]})
        self.camaras.append(c)
        self._acomodar(c.id)
        self.guardar()
        return c

    def actualizar(self, ident: str, datos: dict) -> Camara | None:
        c = self.camara(ident)
        if c is None:
            return None
        nueva = Camara.desde({**asdict(c), **datos, "id": c.id})
        if (nueva.ip, nueva.puerto, nueva.codigo, nueva.usuario) != (c.ip, c.puerto, c.codigo, c.usuario):
            nueva.ptz = ""   # otra cámara u otra contraseña: se vuelve a probar cómo se mueve
        self.camaras[self.camaras.index(c)] = nueva
        self.guardar()
        return nueva

    def quitar(self, ident: str) -> None:
        self.camaras = [c for c in self.camaras if c.id != ident]
        for v in self.ventanas:
            v["celdas"] = ["" if x == ident else x for x in v["celdas"]]
        self.guardar()

    # --- ventanas ---

    @staticmethod
    def _ventana_nueva(n: int) -> dict:
        return {"nombre": f"Ventana {n}", "division": 4, "celdas": [""] * 4}

    @staticmethod
    def _rellenar(v: dict) -> None:
        if len(v["celdas"]) < v["division"]:
            v["celdas"] += [""] * (v["division"] - len(v["celdas"]))

    def _acomodar(self, ident: str) -> None:
        for v in self.ventanas:
            if ident in v["celdas"][:v["division"]]:
                return
        for v in self.ventanas:
            while True:
                self._rellenar(v)
                visibles = v["celdas"][:v["division"]]
                if "" in visibles:
                    v["celdas"][visibles.index("")] = ident
                    return
                mas = next((d for d in DIVISIONES if d > v["division"]), None)
                if mas is None:
                    break
                v["division"] = mas
        v = self._ventana_nueva(len(self.ventanas) + 1)
        v["celdas"][0] = ident
        self.ventanas.append(v)

    def nueva_ventana(self) -> int:
        self.ventanas.append(self._ventana_nueva(len(self.ventanas) + 1))
        self.guardar()
        return len(self.ventanas) - 1

    def quitar_ventana(self, i: int) -> None:
        if len(self.ventanas) > 1 and 0 <= i < len(self.ventanas):
            del self.ventanas[i]
            self.guardar()

    def renombrar_ventana(self, i: int, nombre: str) -> None:
        if 0 <= i < len(self.ventanas) and nombre.strip():
            self.ventanas[i]["nombre"] = nombre.strip()[:40]
            self.guardar()

    def poner_division(self, i: int, division: int) -> None:
        if 0 <= i < len(self.ventanas) and division in DIVISIONES:
            self.ventanas[i]["division"] = division
            self._rellenar(self.ventanas[i])
            self.guardar()

    def poner_celda(self, i: int, pos: int, ident: str) -> None:
        """Pone la cámara en el cuadro `pos` («» lo vacía). Si ya estaba en otro cuadro de la misma
        ventana, se intercambian: así no sale dos veces."""
        if not (0 <= i < len(self.ventanas)) or ident and self.camara(ident) is None:
            return
        v = self.ventanas[i]
        if not (0 <= pos < v["division"]):
            return
        self._rellenar(v)
        if ident and ident in v["celdas"]:
            v["celdas"][v["celdas"].index(ident)] = v["celdas"][pos]
        v["celdas"][pos] = ident
        self.guardar()
