"""Comprimidos: qué traen y descomprimirlos sin peligro.

Todo pasa por bsdtar (libarchive), que lee zip, 7z, rar, iso, tar y sus variantes comprimidas. Solo
si bsdtar no puede (un .rar o un .7z con contraseña, un RAR raro) se usa unrar o 7z, si están
instalados; si faltan, se dice qué paquete instalar.

Seguridad al descomprimir:
- Se descomprime primero en una carpeta temporal escondida dentro del destino y, al terminar, se
  mueve a su nombre final: nunca se escribe encima de nada que ya existiera (si el destino existe,
  `Conflicto` propone otro nombre y la persona decide).
- bsdtar ya se niega a escribir rutas con «..» y a seguir enlaces; además, al terminar se quitan los
  enlaces simbólicos que apunten fuera de la carpeta, y lo que se saltó se avisa.
- bsdtar corre sin terminal (nueva sesión), así que nunca se queda pidiendo una contraseña.
"""
from __future__ import annotations

import bz2
import gzip
import lzma
import os
import posixpath
import re
import shutil
import signal
import subprocess
import tempfile
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath

from .textos import tr

# Extensiones, de la más larga a la más corta («.tar.gz» antes que «.gz»).
EXT_TAR = (".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2", ".tar.lz4", ".tar.lzma", ".tar.lz", ".tgz",
           ".txz", ".tzst", ".tbz2", ".tbz", ".tar")
EXT_SUELTOS = {".gz": "gzip", ".xz": "xz", ".zst": "zstd", ".bz2": "bzip2"}
EXT_OTROS = (".zip", ".rar", ".7z", ".iso")
EXT_TODAS = EXT_TAR + EXT_OTROS + tuple(EXT_SUELTOS)

# Contraseña de relleno: con ella bsdtar nunca pregunta (sin --passphrase intentaría leer la terminal).
_RELLENO = "\x01vortex-sin-contraseña\x01"

PAQUETES = {"7z": "7zip", "unrar": "unrar"}


class ErrorComprimido(Exception):
    """Algo que se le dice a la persona tal cual (ya traducido)."""


class NecesitaContrasena(ErrorComprimido):
    def __init__(self, incorrecta: bool):
        super().__init__(tr("contrasena_mala") if incorrecta else tr("contrasena_pide"))
        self.incorrecta = incorrecta


class FaltaPrograma(ErrorComprimido):
    def __init__(self, programa: str):
        self.programa = programa
        self.paquete = PAQUETES.get(programa, programa)
        self.comando = "sudo pacman -S " + self.paquete
        super().__init__(tr("falta_programa", programa=programa))


class Cancelado(Exception):
    pass


class Conflicto(Exception):
    """El destino ya existe. `sugerida` es un nombre libre al lado («Carpeta (2)»)."""

    def __init__(self, ruta: Path, sugerida: Path):
        super().__init__(str(ruta))
        self.ruta = ruta
        self.sugerida = sugerida


@dataclass
class Entrada:
    nombre: str            # ruta dentro del comprimido, sin «./» ni «/» al principio
    tipo: str              # «archivo», «carpeta», «enlace» (simbólico) o «duro»
    tamano: int = 0
    destino: str = ""      # a dónde apunta, si es enlace
    modo: str = ""         # «-rwxr-xr-x», si se sabe

    @property
    def ejecutable(self) -> bool:
        return self.tipo == "archivo" and len(self.modo) >= 4 and "x" in self.modo[1:4]

    @property
    def profundidad(self) -> int:
        return len(PurePosixPath(self.nombre).parts)


@dataclass
class Resultado:
    destino: Path
    saltados: list[str] = field(default_factory=list)


# --------------------------------------------------------------------------------------- nombres

def _bajo(nombre: str) -> str:
    return nombre.lower()


def formato_suelto(ruta: Path | str) -> str | None:
    """«gzip», «xz»… si es un archivo suelto comprimido (no un .tar.gz), o None."""
    n = _bajo(str(ruta))
    if n.endswith(EXT_TAR):
        return None
    for ext, fmt in EXT_SUELTOS.items():
        if n.endswith(ext):
            return None if _es_tar_por_dentro(Path(ruta), fmt) else fmt
    return None


def nombre_sin_extension(nombre: str) -> str:
    bajo = _bajo(nombre)
    for ext in EXT_TODAS + (".deb", ".rpm"):
        if bajo.endswith(ext) and len(nombre) > len(ext):
            return nombre[: -len(ext)]
    return Path(nombre).stem or nombre


def nombre_libre(ruta: Path) -> Path:
    """«Carpeta» → «Carpeta (2)» (o (3)…); «foto.txt» → «foto (2).txt»."""
    es_carpeta = not ruta.suffix or ruta.is_dir()
    base, ext = (ruta.name, "") if es_carpeta else (ruta.stem, ruta.suffix)
    n = 2
    while True:
        candidata = ruta.with_name("%s (%d)%s" % (base, n, ext))
        if not os.path.lexists(candidata):
            return candidata
        n += 1


def _limpiar(nombre: str) -> str:
    while nombre.startswith("./"):
        nombre = nombre[2:]
    return nombre.lstrip("/").rstrip("/")


# --------------------------------------------------------------------------------------- análisis

def peligrosa(e: Entrada) -> bool:
    """True si la entrada quiere escribir o apuntar fuera de la carpeta («../», enlaces que salen)."""
    if ".." in PurePosixPath(e.nombre).parts:
        return True
    if e.tipo in ("enlace", "duro") and e.destino:
        if e.destino.startswith("/"):
            return e.tipo == "enlace"   # un enlace duro «/x» bsdtar lo deja dentro (le quita la «/»)
        base = posixpath.dirname(e.nombre) if e.tipo == "enlace" else ""
        norm = posixpath.normpath(posixpath.join(base, e.destino))
        return norm == ".." or norm.startswith("../")
    return False


def raiz_unica(entradas: list[Entrada]) -> str | None:
    """El nombre de la carpeta si todo el comprimido vive en una sola carpeta (así no se anida dos veces).
    Lo peligroso no cuenta: no se va a descomprimir."""
    entradas = [e for e in entradas if not peligrosa(e)]
    if not entradas:
        return None
    tops = {PurePosixPath(e.nombre).parts[0] for e in entradas if e.nombre}
    if len(tops) != 1:
        return None
    top = tops.pop()
    if top in ("..", "."):
        return None
    hay_dentro = any(e.profundidad > 1 for e in entradas)
    es_carpeta = any(e.nombre == top and e.tipo == "carpeta" for e in entradas)
    solo_carpeta = all(e.nombre != top or e.tipo == "carpeta" for e in entradas)
    return top if (hay_dentro or es_carpeta) and solo_carpeta else None


def tamano_total(entradas: list[Entrada]) -> int:
    return sum(e.tamano for e in entradas if e.tipo == "archivo")


def destino_sugerido(archivo: Path, entradas: list[Entrada], padre: Path) -> Path:
    """Dónde queda lo descomprimido: la carpeta que ya trae, o una con el nombre del archivo."""
    raiz = raiz_unica(entradas)
    if raiz:
        return padre / raiz
    return padre / nombre_sin_extension(archivo.name)


# --------------------------------------------------------------------------------------- procesos

def _entorno() -> dict:
    env = dict(os.environ)
    env["LC_ALL"] = "C.UTF-8"   # mensajes de error en inglés (los que se reconocen) y nombres en UTF-8
    env["LANG"] = "C.UTF-8"
    return env


def _correr(args: list[str], timeout: float | None = 600) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True, text=True,
                              errors="surrogateescape", env=_entorno(), timeout=timeout,
                              start_new_session=True, check=False)
    except FileNotFoundError:
        raise FaltaPrograma(args[0]) from None
    except subprocess.TimeoutExpired:
        raise ErrorComprimido(tr("comprimido_tarda")) from None


def _bsdtar() -> str:
    exe = shutil.which("bsdtar")
    if not exe:
        raise FaltaPrograma("bsdtar")
    return exe


def _pide_contrasena(texto: str) -> bool:
    t = texto.lower()
    return "passphrase" in t or "password" in t


def _sin_soporte(texto: str) -> bool:
    t = texto.lower()
    return "encrypt" in t or "unsupported" in t or "not supported" in t or "rar5" in t


def _es_tar_por_dentro(ruta: Path, fmt: str) -> bool:
    """¿Un «.gz» suelto es en realidad un tar? (mira «ustar» en lo descomprimido)."""
    try:
        with _abrir_suelto(ruta, fmt) as f:
            cabeza = f.read(512)
        return cabeza[257:262] == b"ustar"
    except Exception:   # noqa: BLE001 (dañado, formato raro: no es tar)
        return False


def _abrir_suelto(ruta: Path, fmt: str):
    if fmt == "gzip":
        return gzip.open(ruta, "rb")
    if fmt == "xz":
        return lzma.open(ruta, "rb")
    if fmt == "bzip2":
        return bz2.open(ruta, "rb")
    try:
        from compression import zstd   # Python 3.14+
    except ImportError:
        return _ZstdCli(ruta)
    return zstd.open(ruta, "rb")


class _ZstdCli:
    """Respaldo para .zst sin el módulo de Python: «zstd -dc» por una tubería."""

    def __init__(self, ruta: Path):
        exe = shutil.which("zstd")
        if not exe:
            raise FaltaPrograma("zstd")
        self._p = subprocess.Popen([exe, "-dcq", str(ruta)], stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def read(self, n: int = -1) -> bytes:
        return self._p.stdout.read(n)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self._p.stdout.close()
        self._p.kill()
        self._p.wait()


# --------------------------------------------------------------------------------------- listar

_LINEA = re.compile(r"^(?P<modo>[-dlhcbpsD][-rwxsStTl]{9})\S*\s+\d+\s+\S+\s+\S+\s+(?P<tam>\d+)"
                    r"\s+\S+\s+\d+\s+\S+ (?P<nombre>.*)$")


def _desde_bsdtar(salida: str) -> list[Entrada]:
    entradas = []
    for linea in salida.splitlines():
        m = _LINEA.match(linea)
        if not m:
            continue
        modo, nombre, destino = m["modo"], m["nombre"], ""
        if modo[0] == "l" and " -> " in nombre:
            nombre, destino = nombre.split(" -> ", 1)
            tipo = "enlace"
        elif modo[0] == "h" and " link to " in nombre:
            nombre, destino = nombre.split(" link to ", 1)
            tipo = "duro"
        elif modo[0] == "d" or nombre.endswith("/"):
            tipo = "carpeta"
        else:
            tipo = "archivo"
        nombre = _limpiar(nombre)
        if nombre in ("", "."):
            continue
        entradas.append(Entrada(nombre, tipo, int(m["tam"]) if tipo == "archivo" else 0, destino, modo))
    return entradas


def _desde_7z(salida: str) -> list[Entrada]:
    entradas, bloque = [], {}
    for linea in salida.splitlines() + [""]:
        if not linea.strip():
            if "Path" in bloque:
                carpeta = bloque.get("Folder") == "+" or bloque.get("Attributes", "").startswith("D")
                tam = int(bloque.get("Size") or 0) if (bloque.get("Size") or "0").isdigit() else 0
                nombre = _limpiar(bloque["Path"])
                if nombre:
                    entradas.append(Entrada(nombre, "carpeta" if carpeta else "archivo", 0 if carpeta else tam))
            bloque = {}
            continue
        if " = " in linea:
            k, v = linea.split(" = ", 1)
            bloque[k.strip()] = v
    return entradas[1:] if entradas and entradas[0].nombre.endswith((".7z", ".rar")) else entradas


def _desde_unrar(salida: str) -> list[Entrada]:
    entradas, bloque = [], {}
    for linea in salida.splitlines() + [""]:
        s = linea.strip()
        if s.startswith("Name:"):
            if "Name" in bloque:
                entradas.append(bloque)
            bloque = {"Name": s[5:].strip()}
        elif ":" in s and bloque:
            k, v = s.split(":", 1)
            bloque[k.strip()] = v.strip()
    if "Name" in bloque:
        entradas.append(bloque)
    salida_e = []
    for b in entradas:
        carpeta = b.get("Type", "").lower().startswith("dir")
        tam = b.get("Size", "0")
        salida_e.append(Entrada(_limpiar(b["Name"]), "carpeta" if carpeta else "archivo",
                                0 if carpeta or not tam.isdigit() else int(tam)))
    return [e for e in salida_e if e.nombre]


def listar(archivo: Path | str, contrasena: str | None = None) -> list[Entrada]:
    """Lo que trae el comprimido. Lanza NecesitaContrasena, FaltaPrograma o ErrorComprimido."""
    archivo = Path(archivo)
    fmt = formato_suelto(archivo)
    if fmt:
        tam = 0
        if fmt == "gzip":   # el tamaño original va en los últimos 4 bytes (módulo 4 GiB)
            try:
                with open(archivo, "rb") as f:
                    f.seek(-4, os.SEEK_END)
                    tam = int.from_bytes(f.read(4), "little")
            except OSError:
                tam = 0
        return [Entrada(nombre_sin_extension(archivo.name), "archivo", tam)]
    r = _correr([_bsdtar(), "-tvf", str(archivo), "--passphrase", contrasena or _RELLENO])
    if r.returncode == 0:
        return _desde_bsdtar(r.stdout)
    error = r.stderr
    if _pide_contrasena(error) and not _sin_soporte(error):
        raise NecesitaContrasena(incorrecta=bool(contrasena))
    otro = _otro_programa(archivo)
    if otro:   # un .rar o .7z que bsdtar no puede: con unrar o 7z, si están
        return _listar_con(otro, archivo, contrasena)
    raise ErrorComprimido(tr("comprimido_danado"))


def _otro_programa(archivo: Path) -> str | None:
    """El programa de respaldo para este formato: «unrar» para .rar, «7z» para .7z (y para .rar sin unrar)."""
    ext = archivo.suffix.lower()
    if ext == ".rar":
        return "unrar" if shutil.which("unrar") else "7z"
    if ext == ".7z":
        return "7z"
    return None


def _exe_7z() -> str:
    for nombre in ("7z", "7zz", "7za"):
        exe = shutil.which(nombre)
        if exe:
            return exe
    raise FaltaPrograma("7z")


def _listar_con(programa: str, archivo: Path, contrasena: str | None) -> list[Entrada]:
    if programa == "unrar":
        exe = shutil.which("unrar")
        if not exe:
            raise FaltaPrograma("unrar")
        r = _correr([exe, "lt", "-p" + (contrasena or "-"), str(archivo)])
        if r.returncode == 0:
            return _desde_unrar(r.stdout)
        if r.returncode == 11 or _pide_contrasena(r.stdout + r.stderr):
            raise NecesitaContrasena(incorrecta=bool(contrasena))
        raise ErrorComprimido(tr("comprimido_danado"))
    r = _correr([_exe_7z(), "l", "-slt", "-p" + (contrasena or _RELLENO), str(archivo)])
    if r.returncode == 0:
        return _desde_7z(r.stdout)
    if _pide_contrasena(r.stdout + r.stderr):
        raise NecesitaContrasena(incorrecta=bool(contrasena))
    raise ErrorComprimido(tr("comprimido_danado"))


# --------------------------------------------------------------------------------------- extraer

def _limpiar_enlaces(raiz: Path) -> list[str]:
    """Quita los enlaces simbólicos que apuntan fuera de `raiz`. Devuelve los que quitó."""
    quitados, base = [], os.path.abspath(raiz)
    for carpeta, dirs, archivos in os.walk(base):
        for n in dirs + archivos:
            p = os.path.join(carpeta, n)
            if not os.path.islink(p):
                continue
            destino = os.readlink(p)
            real = os.path.normpath(destino if os.path.isabs(destino) else os.path.join(carpeta, destino))
            if real != base and not real.startswith(base + os.sep):
                os.unlink(p)
                quitados.append(os.path.relpath(p, base))
    return quitados


def _permiso_de_escritura(raiz: Path) -> None:
    """Un .iso trae carpetas de solo lectura: sin esto no se podrían mover ni mandar a la papelera."""
    for carpeta, dirs, _ in os.walk(raiz):
        for d in dirs:
            p = os.path.join(carpeta, d)
            if not os.path.islink(p):
                try:
                    os.chmod(p, os.stat(p).st_mode | 0o700)
                except OSError:
                    pass
    try:
        os.chmod(raiz, os.stat(raiz).st_mode | 0o700)
    except OSError:
        pass


def _vigilar(proc: subprocess.Popen, temporal: Path, tamanos: dict[str, int], progreso, cancelar) -> str:
    """Lee la salida de bsdtar -v (o 7z / unrar) mientras avanza; devuelve todo lo que dijo.

    bsdtar escribe «x nombre» al empezar cada archivo, pero el salto de línea hasta terminarlo: por eso
    se lee a pedazos y el renglón a medias ya dice qué archivo se está escribiendo (y su tamaño en disco
    da el avance dentro de él)."""
    total = max(1, sum(tamanos.values()))
    partes: list[bytes] = []
    estado = {"hecho": 0, "actual": None, "contados": set()}
    flujo = proc.stderr or proc.stdout

    def empezar(nombre: str) -> None:
        nombre = _limpiar(nombre)
        if nombre == estado["actual"]:
            return
        previo = estado["actual"]
        if previo and previo not in estado["contados"]:
            estado["hecho"] += tamanos.get(previo, 0)
            estado["contados"].add(previo)
        estado["actual"] = nombre

    def renglon(texto: str, completo: bool) -> None:
        if texto.startswith("x "):
            nombre = texto[2:]
            if nombre not in tamanos and ": " in nombre:
                nombre = nombre.split(": ", 1)[0]
            if completo or _limpiar(nombre) in tamanos:
                empezar(nombre)
        elif completo and progreso:
            m = re.search(r"(\d{1,3})%", texto)
            if m:
                progreso(min(1.0, int(m.group(1)) / 100))

    def leer():
        resto = b""
        while True:
            try:
                trozo = os.read(flujo.fileno(), 65536)
            except OSError:
                break
            if not trozo:
                break
            partes.append(trozo)
            resto += trozo
            *completos, resto = re.split(rb"[\n\r]", resto)
            for c in completos:
                renglon(c.decode("utf-8", "surrogateescape"), True)
            if resto:
                renglon(resto.decode("utf-8", "surrogateescape"), False)

    hilo = threading.Thread(target=leer, daemon=True)
    hilo.start()
    while proc.poll() is None:
        if cancelar is not None and cancelar.is_set():
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except OSError:
                proc.kill()
            proc.wait()
            raise Cancelado()
        actual = estado["actual"]
        if progreso and actual is not None:
            parcial = 0
            try:
                parcial = os.lstat(temporal / actual).st_size
            except OSError:
                pass
            ya = estado["hecho"] + (0 if actual in estado["contados"] else min(parcial, tamanos.get(actual, 0)))
            progreso(min(1.0, ya / total))
        time.sleep(0.12)
    hilo.join(timeout=2)
    return b"".join(partes).decode("utf-8", "surrogateescape")


def _popen(args: list[str]) -> subprocess.Popen:
    try:
        return subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                stderr=subprocess.PIPE, env=_entorno(), start_new_session=True)
    except FileNotFoundError:
        raise FaltaPrograma(args[0]) from None


def _popen_unido(args: list[str]) -> subprocess.Popen:
    """Como _popen, pero con la salida normal y la de error juntas (7z y unrar dan el avance por stdout)."""
    try:
        return subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, env=_entorno(), start_new_session=True)
    except FileNotFoundError:
        raise FaltaPrograma(args[0]) from None


def _error_extraer(texto: str, codigo: int, contrasena: str | None, nombres=()) -> None:
    """Convierte lo que dijo bsdtar en la excepción que toca, o no hace nada si solo se saltó lo peligroso."""
    t = texto.lower()
    if "no space left" in t:
        raise ErrorComprimido(tr("sin_espacio"))
    if "passphrase" in t or "password" in t:
        raise NecesitaContrasena(incorrecta=bool(contrasena))
    inofensivos = ("path contains '..'", "cannot extract through symlink", "removing leading",
                   "error exit delayed", "hard-link target", "absolute path")
    if codigo == 0:
        return
    for linea in t.splitlines():
        l = linea.strip()
        if not l or (l.startswith("x ") and (": " not in l or l[2:] in nombres)):
            continue   # solo el nombre de lo que se descomprimió: bien
        if any(i in l for i in inofensivos):
            continue   # lo peligroso que bsdtar se negó a escribir (se avisa aparte)
        raise ErrorComprimido(tr("comprimido_danado"))


def extraer(archivo: Path | str, destino: Path | str, entradas: list[Entrada] | None = None,
            contrasena: str | None = None, progreso=None, cancelar: threading.Event | None = None,
            quitar_raiz: bool = True) -> Resultado:
    """Descomprime `archivo` en `destino` (que no debe existir). Si todo viene en una sola carpeta,
    su contenido queda directo en `destino` (no «destino/carpeta»), salvo con quitar_raiz=False."""
    archivo, destino = Path(archivo), Path(destino)
    if os.path.lexists(destino):
        raise Conflicto(destino, nombre_libre(destino))
    padre = destino.parent
    padre.mkdir(parents=True, exist_ok=True)
    fmt = formato_suelto(archivo)
    temporal = Path(tempfile.mkdtemp(prefix=".vortex-descomprimiendo-", dir=padre))
    try:
        if fmt:
            _extraer_suelto(archivo, fmt, temporal / destino.name, progreso, cancelar)
            if os.path.lexists(destino):
                raise Conflicto(destino, nombre_libre(destino))
            os.rename(temporal / destino.name, destino)
            return Resultado(destino)
        if entradas is None:
            entradas = listar(archivo, contrasena)
        tamanos = {e.nombre: e.tamano for e in entradas if e.tipo == "archivo"}
        _extraer_en(archivo, temporal, tamanos, contrasena, progreso, cancelar)
        saltados = [e.nombre for e in entradas if peligrosa(e)]
        saltados += [q for q in _limpiar_enlaces(temporal) if q not in saltados]
        _permiso_de_escritura(temporal)
        raiz = raiz_unica(entradas) if quitar_raiz else None
        origen = temporal / raiz if raiz and (temporal / raiz).is_dir() and not (temporal / raiz).is_symlink() \
            else temporal
        if os.path.lexists(destino):
            raise Conflicto(destino, nombre_libre(destino))
        os.rename(origen, destino)
        if origen is temporal:
            os.chmod(destino, 0o755)   # mkdtemp la crea solo para ti (0700)
        if progreso:
            progreso(1.0)
        return Resultado(destino, saltados)
    finally:
        if temporal.exists():
            _permiso_de_escritura(temporal)
            shutil.rmtree(temporal, ignore_errors=True)


def _extraer_en(archivo: Path, temporal: Path, tamanos: dict[str, int], contrasena: str | None,
                progreso, cancelar) -> None:
    args = [_bsdtar(), "-xvf", str(archivo), "-C", str(temporal), "--no-same-owner", "--no-acls",
            "--no-xattrs", "--no-fflags", "--passphrase", contrasena or _RELLENO]
    proc = _popen(args)
    texto = _vigilar(proc, temporal, tamanos, progreso, cancelar)
    if proc.returncode == 0:
        return
    otro = _otro_programa(archivo)
    if otro and _sin_soporte(texto):
        _vaciar(temporal)
        return _extraer_con(otro, archivo, temporal, contrasena, progreso, cancelar)
    _error_extraer(texto, proc.returncode, contrasena, {n.lower() for n in tamanos})


def _vaciar(carpeta: Path) -> None:
    _permiso_de_escritura(carpeta)
    for hijo in carpeta.iterdir():
        if hijo.is_dir() and not hijo.is_symlink():
            shutil.rmtree(hijo, ignore_errors=True)
        else:
            hijo.unlink(missing_ok=True)


def _extraer_con(programa: str, archivo: Path, temporal: Path, contrasena: str | None, progreso, cancelar) -> None:
    if programa == "unrar":
        exe = shutil.which("unrar")
        if not exe:
            raise FaltaPrograma("unrar")
        args = [exe, "x", "-o-", "-y", "-p" + (contrasena or "-"), str(archivo), str(temporal) + "/"]
    else:
        args = [_exe_7z(), "x", "-y", "-bsp2", "-bso0", "-snl-", "-p" + (contrasena or _RELLENO),
                "-o" + str(temporal), str(archivo)]
    proc = _popen_unido(args) if programa == "unrar" else _popen(args)
    texto = _vigilar(proc, temporal, {}, progreso, cancelar)
    if proc.returncode == 0:
        return
    if proc.returncode == 11 or _pide_contrasena(texto):
        raise NecesitaContrasena(incorrecta=bool(contrasena))
    if "no space" in texto.lower():
        raise ErrorComprimido(tr("sin_espacio"))
    raise ErrorComprimido(tr("comprimido_danado"))


def _extraer_suelto(archivo: Path, fmt: str, salida: Path, progreso, cancelar) -> None:
    total = max(1, archivo.stat().st_size)
    try:
        with open(archivo, "rb") as crudo, _abrir_desde(crudo, fmt, archivo) as f, open(salida, "xb") as out:
            while True:
                if cancelar is not None and cancelar.is_set():
                    raise Cancelado()
                bloque = f.read(1 << 20)
                if not bloque:
                    break
                out.write(bloque)
                if progreso:
                    try:
                        progreso(min(1.0, crudo.tell() / total))
                    except (OSError, ValueError):
                        pass
    except (OSError, EOFError, lzma.LZMAError, ValueError) as e:
        if isinstance(e, OSError) and e.errno == 28:
            raise ErrorComprimido(tr("sin_espacio")) from None
        raise ErrorComprimido(tr("comprimido_danado")) from None


def _abrir_desde(crudo, fmt: str, archivo: Path):
    if fmt == "gzip":
        return gzip.GzipFile(fileobj=crudo)
    if fmt == "xz":
        return lzma.LZMAFile(crudo)
    if fmt == "bzip2":
        return bz2.BZ2File(crudo)
    try:
        from compression import zstd
    except ImportError:
        return _ZstdCli(archivo)
    return zstd.ZstdFile(crudo)
