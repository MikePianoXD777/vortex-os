"""El nombre y el ícono de un .exe de Windows, leídos del propio archivo (formato PE), sin Wine ni
programas extra.

- nombre(ruta): el «ProductName» o «FileDescription» de su información de versión; si no tiene, el
  nombre del archivo arreglado («my_game-x64.exe» → «My Game»).
- guardar_icono(ruta, destino): saca el ícono más grande (RT_GROUP_ICON) y lo guarda como .ico o .png
  (los de 256 px suelen venir en PNG). Devuelve la ruta o None.

Solo lee la cabecera y la sección de recursos (hasta 64 MB), no el .exe entero: hay juegos de varios GB.
Cualquier cosa rara (no es un PE, está cortado...) da None, nunca una excepción.
"""
from __future__ import annotations

import re
import struct
from pathlib import Path

RT_ICON = 3
RT_GROUP_ICON = 14
RT_VERSION = 16
MAX_RECURSOS = 64 * 1024 * 1024


class _PE:
    def __init__(self, ruta: Path):
        self.f = open(ruta, "rb")
        try:
            self._leer_cabecera()
        except Exception:
            self.f.close()
            raise

    def close(self) -> None:
        self.f.close()

    def _leer(self, pos: int, n: int) -> bytes:
        self.f.seek(pos)
        datos = self.f.read(n)
        if len(datos) != n:
            raise ValueError("archivo cortado")
        return datos

    def _leer_cabecera(self) -> None:
        if self._leer(0, 2) != b"MZ":
            raise ValueError("no es un ejecutable de Windows")
        pe = struct.unpack("<I", self._leer(0x3C, 4))[0]
        if self._leer(pe, 4) != b"PE\0\0":
            raise ValueError("no es PE")
        coff = pe + 4
        n_secciones, = struct.unpack("<H", self._leer(coff + 2, 2))
        tam_opcional, = struct.unpack("<H", self._leer(coff + 16, 2))
        opcional = coff + 20
        magia, = struct.unpack("<H", self._leer(opcional, 2))
        directorios = opcional + (96 if magia == 0x10B else 112)
        self.rva_recursos, self.tam_recursos = struct.unpack("<II", self._leer(directorios + 2 * 8, 8))
        self.secciones = []
        tabla = opcional + tam_opcional
        for i in range(min(n_secciones, 96)):
            s = self._leer(tabla + 40 * i, 40)
            vtam, vdir, tam_crudo, pos_cruda = struct.unpack("<IIII", s[8:24])
            self.secciones.append((vdir, max(vtam, tam_crudo), pos_cruda))
        self.base = None
        self.crudo = b""
        if self.rva_recursos:
            for vdir, tam, pos in self.secciones:
                if vdir <= self.rva_recursos < vdir + tam:
                    self.base = self.rva_recursos
                    inicio = pos + (self.rva_recursos - vdir)
                    self.f.seek(inicio)
                    self.crudo = self.f.read(min(max(self.tam_recursos, tam - (self.rva_recursos - vdir)), MAX_RECURSOS))
                    break

    def _u16(self, pos: int) -> int:
        return struct.unpack_from("<H", self.crudo, pos)[0]

    def _u32(self, pos: int) -> int:
        return struct.unpack_from("<I", self.crudo, pos)[0]

    def _entradas(self, pos: int) -> list[tuple[int | str, int, bool]]:
        """(id o nombre, desplazamiento, es_carpeta) de un IMAGE_RESOURCE_DIRECTORY."""
        nombradas = self._u16(pos + 12)
        con_id = self._u16(pos + 14)
        salida = []
        for i in range(min(nombradas + con_id, 4096)):
            e = pos + 16 + 8 * i
            nombre, dato = self._u32(e), self._u32(e + 4)
            if nombre & 0x80000000:
                p = nombre & 0x7FFFFFFF
                largo = self._u16(p)
                clave: int | str = self.crudo[p + 2:p + 2 + 2 * largo].decode("utf-16-le", "replace")
            else:
                clave = nombre & 0xFFFF
            salida.append((clave, dato & 0x7FFFFFFF, bool(dato & 0x80000000)))
        return salida

    def recursos(self, tipo: int) -> dict[int | str, bytes]:
        """{id: datos} de un tipo de recurso (el primer idioma de cada uno)."""
        if self.base is None or not self.crudo:
            return {}
        salida: dict[int | str, bytes] = {}
        for clave, pos, carpeta in self._entradas(0):
            if clave != tipo or not carpeta:
                continue
            for rid, pos2, carpeta2 in self._entradas(pos):
                hoja = pos2
                if carpeta2:
                    idiomas = self._entradas(pos2)
                    if not idiomas:
                        continue
                    hoja = idiomas[0][1]
                rva, tam = self._u32(hoja), self._u32(hoja + 4)
                ini = rva - self.base
                if 0 <= ini and ini + tam <= len(self.crudo):
                    salida[rid] = self.crudo[ini:ini + tam]
        return salida


def _abrir(ruta: str | Path) -> _PE | None:
    try:
        return _PE(Path(ruta))
    except Exception:
        return None


# ---- información de versión ----------------------------------------------------------------------

def _cadenas_version(datos: bytes) -> dict[str, str]:
    """Las cadenas de StringFileInfo (ProductName, FileDescription...) de un VS_VERSIONINFO."""
    salida: dict[str, str] = {}

    def bloque(pos: int, fin: int, profundidad: int) -> None:
        while pos + 6 <= fin and profundidad < 6:
            largo, largo_valor, tipo = struct.unpack_from("<HHH", datos, pos)
            if largo == 0:
                return
            final = min(pos + largo, fin)
            k = pos + 6
            j = k
            while j + 1 < final and datos[j:j + 2] != b"\0\0":
                j += 2
            clave = datos[k:j].decode("utf-16-le", "replace")
            valor_pos = (j + 2 + 3) & ~3
            if tipo == 1 and largo_valor and profundidad >= 3:
                crudo = datos[valor_pos:valor_pos + 2 * largo_valor]
                salida.setdefault(clave, crudo.decode("utf-16-le", "replace").split("\0", 1)[0].strip())
                hijos = final
            elif clave == "VS_VERSION_INFO":
                hijos = (valor_pos + largo_valor + 3) & ~3
            else:
                hijos = (valor_pos + (2 * largo_valor if tipo == 1 else largo_valor) + 3) & ~3
            if clave in ("VS_VERSION_INFO", "StringFileInfo") or (profundidad == 2 and clave != "VarFileInfo"):
                bloque(hijos, final, profundidad + 1)
            pos = (final + 3) & ~3

    try:
        bloque(0, len(datos), 0)
    except struct.error:
        pass
    return salida


GENERICOS = re.compile(r"^(microsoft|windows|setup|installer|launcher|game|application|app|\?+)$", re.I)


def _bonito(stem: str) -> str:
    s = re.sub(r"[_\-.]+", " ", stem)
    s = re.sub(r"\b(x64|x86|win64|win32|64bit|32bit|shipping)\b", "", s, flags=re.I)
    s = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", s)
    s = re.sub(r"\s+", " ", s).strip()
    if s and s == s.lower():
        s = s.title()
    return s or stem


def nombre(ruta: str | Path) -> str:
    ruta = Path(ruta)
    pe = _abrir(ruta)
    if pe is not None:
        try:
            for datos in pe.recursos(RT_VERSION).values():
                c = _cadenas_version(datos)
                for clave in ("ProductName", "FileDescription"):
                    v = (c.get(clave) or "").strip()
                    if v and not GENERICOS.match(v) and len(v) <= 60:
                        return v
        except Exception:
            pass
        finally:
            pe.close()
    return _bonito(ruta.stem)


# ---- ícono --------------------------------------------------------------------------------------

def guardar_icono(ruta: str | Path, destino: str | Path) -> Path | None:
    """Guarda el ícono más grande del .exe en `destino` (sin extensión: se le pone .png o .ico)."""
    pe = _abrir(ruta)
    if pe is None:
        return None
    try:
        grupos = pe.recursos(RT_GROUP_ICON)
        if not grupos:
            return None
        iconos = pe.recursos(RT_ICON)
        mejor = None
        for g in grupos.values():
            if len(g) < 6:
                continue
            n = struct.unpack_from("<H", g, 4)[0]
            for i in range(n):
                p = 6 + 14 * i
                if p + 14 > len(g):
                    break
                ancho, alto, colores, _, planos, bits, tam, iid = struct.unpack_from("<BBBBHHIH", g, p)
                datos = iconos.get(iid)
                if not datos:
                    continue
                orden = ((ancho or 256), bits)
                if mejor is None or orden > mejor[0]:
                    mejor = (orden, g[p:p + 12], datos)
            if mejor:
                break
        if mejor is None:
            return None
        _, entrada, datos = mejor
        destino = Path(destino)
        destino.parent.mkdir(parents=True, exist_ok=True)
        if datos[:8] == b"\x89PNG\r\n\x1a\n":
            final = destino.with_suffix(".png")
            final.write_bytes(datos)
        else:
            final = destino.with_suffix(".ico")
            cabecera = struct.pack("<HHH", 0, 1, 1) + entrada[:8] + struct.pack("<I", len(datos)) + struct.pack("<I", 22)
            final.write_bytes(cabecera + datos)
        return final
    except Exception:
        return None
    finally:
        pe.close()
