"""El vault: una carpeta de notas en Markdown, como Obsidian.

Entiende lo mismo que Obsidian: carpetas, [[enlaces]] (con #sección, ^bloque, |alias y ![[incrustados]]),
#etiquetas (también anidadas, #juegos/roblox), frontmatter YAML con `tags` y `aliases`, retroenlaces y el
grafo. Se puede abrir cualquier carpeta, también un vault de Obsidian que ya uses, sin romperlo: ignora
las carpetas que empiezan con punto (.obsidian/, .trash/, .git/), no escribe nada fuera de las notas y lo
que se borra va a .trash/ del vault, como hace Obsidian.

Todas las rutas son relativas al vault, con «/» y con «.md» (por ejemplo «Ideas/Juegos.md»).
"""
from __future__ import annotations

import os
import re
import tempfile
import unicodedata
from collections import Counter
from pathlib import Path, PurePosixPath


class ErrorVault(ValueError):
    """Una ruta que no se puede usar (por ejemplo, una que se sale del vault)."""


PAPELERA = ".trash"
PROHIBIDOS = re.compile(r'[\\/:*?"<>|#^\[\]]')

# ------------------------------------------------------------------ lectura de Markdown

_CODIGO_BLOQUE = re.compile(r"^(```|~~~).*?(?:^\1[^\n]*$|\Z)", re.M | re.S)
_CODIGO_LINEA = re.compile(r"`[^`\n]+`")
_COMENTARIO = re.compile(r"%%.*?%%", re.S)
_ENLACE = re.compile(r"(!?)\[\[([^\[\]\n]+?)\]\]")
_ETIQUETA = re.compile(r"(?<![\w#&/\\])#([\w][\w/-]*)")


def _tapar(texto: str) -> str:
    """El texto con el código y los comentarios %%…%% cambiados por espacios (mismo largo y mismas líneas).

    Así lo que va en código no cuenta como enlace ni etiqueta, y las posiciones siguen sirviendo.
    """
    def espacios(m: re.Match) -> str:
        return re.sub(r"[^\n]", " ", m.group(0))
    for patron in (_CODIGO_BLOQUE, _COMENTARIO, _CODIGO_LINEA):
        texto = patron.sub(espacios, texto)
    return texto


def _valor_yaml(valor: str):
    valor = valor.strip()
    if valor.startswith("[") and valor.endswith("]"):
        return [_quitar_comillas(v) for v in _partir_lista(valor[1:-1]) if v.strip()]
    if valor in ("", "~", "null"):
        return None
    if valor.lower() in ("true", "false"):
        return valor.lower() == "true"
    return _quitar_comillas(valor)


def _quitar_comillas(valor: str) -> str:
    valor = valor.strip()
    if len(valor) >= 2 and valor[0] == valor[-1] and valor[0] in "\"'":
        return valor[1:-1]
    return valor


def _partir_lista(texto: str) -> list[str]:
    """Parte «a, "b, c", d» por comas que no vayan entre comillas."""
    partes, actual, comilla = [], "", ""
    for c in texto:
        if comilla:
            actual += c
            if c == comilla:
                comilla = ""
        elif c in "\"'":
            comilla = c
            actual += c
        elif c == ",":
            partes.append(actual)
            actual = ""
        else:
            actual += c
    partes.append(actual)
    return partes


def separar_frontmatter(texto: str) -> tuple[dict, str]:
    """(frontmatter, cuerpo). Entiende el YAML sencillo que usa Obsidian: «clave: valor», listas [a, b] y
    listas con «- elemento». Si no hay frontmatter, ({}, texto)."""
    if texto.startswith("﻿"):
        texto = texto[1:]
    if not texto.startswith("---"):
        return {}, texto
    lineas = texto.split("\n")
    if lineas[0].strip() != "---":
        return {}, texto
    fin = None
    for i in range(1, len(lineas)):
        if lineas[i].strip() in ("---", "..."):
            fin = i
            break
    if fin is None:
        return {}, texto
    datos: dict = {}
    clave = None
    for linea in lineas[1:fin]:
        linea = linea.rstrip("\r")
        if not linea.strip() or linea.lstrip().startswith("#"):
            continue
        suelta = linea.strip()
        if suelta.startswith("- ") or suelta == "-":
            if clave is not None:
                if not isinstance(datos.get(clave), list):
                    datos[clave] = [] if datos.get(clave) in (None, "") else [datos[clave]]
                elemento = _quitar_comillas(suelta[1:].strip())
                if elemento:
                    datos[clave].append(elemento)
            continue
        if ":" in linea and not linea[0].isspace():
            clave, _, valor = linea.partition(":")
            clave = clave.strip()
            datos[clave] = _valor_yaml(valor)
    return datos, "\n".join(lineas[fin + 1:])


def _lista(valor) -> list[str]:
    """Una lista de textos a partir de lo que venga en el frontmatter (lista, «a, b» o «a b»)."""
    if valor is None or valor is False:
        return []
    if isinstance(valor, list):
        return [str(v).strip() for v in valor if str(v).strip()]
    return [v for v in re.split(r"[,\s]+", str(valor)) if v]


def etiquetas_de(texto: str) -> list[str]:
    """Las etiquetas de una nota (del frontmatter `tags` y de los #etiqueta del texto), sin «#» y en
    minúsculas, sin repetir. Las que son solo números (#2026) no cuentan, como en Obsidian."""
    frente, cuerpo = separar_frontmatter(texto)
    vistas: dict[str, None] = {}
    for clave in ("tags", "tag", "etiquetas"):
        for t in _lista(frente.get(clave)):
            t = t.lstrip("#").strip().lower()
            if t:
                vistas.setdefault(t, None)
    for m in _ETIQUETA.finditer(_tapar(cuerpo)):
        t = m.group(1).rstrip("/-").lower()
        if t and not t.replace("/", "").isdigit():
            vistas.setdefault(t, None)
    return list(vistas)


def alias_de(frente: dict) -> list[str]:
    salida = []
    for clave in ("aliases", "alias"):
        salida.extend(_lista(frente.get(clave)) if isinstance(frente.get(clave), list)
                      else [a.strip() for a in str(frente.get(clave) or "").split(",") if a.strip()])
    return salida


def _partir_enlace(interior: str) -> tuple[str, str, str]:
    """«Nota#Sección|texto» → ("Nota", "Sección", "texto")."""
    destino, _, mostrado = interior.partition("|")
    nombre, seccion = destino, ""
    for separador in ("#", "^"):
        if separador in nombre:
            nombre, _, resto = nombre.partition(separador)
            seccion = (separador if separador == "^" else "") + resto
            break
    return nombre.strip(), seccion.strip(), mostrado.strip()


def enlaces_de(texto: str) -> list[dict]:
    """Los [[enlaces]] de un texto, en orden: {"nombre", "seccion", "texto", "incrustado", "inicio", "fin"}.

    «inicio» y «fin» son las posiciones del [[…]] completo en `texto` (sirven para reescribirlo).
    """
    salida = []
    for m in _ENLACE.finditer(_tapar(texto)):
        nombre, seccion, mostrado = _partir_enlace(texto[m.start(2):m.end(2)])
        salida.append({"nombre": nombre, "seccion": seccion, "texto": mostrado, "incrustado": bool(m.group(1)),
                       "inicio": m.start(), "fin": m.end()})
    return salida


def _normal(texto: str) -> str:
    """Minúsculas y sin acentos, para buscar («Canción» → «cancion»)."""
    return "".join(c for c in unicodedata.normalize("NFD", texto.lower()) if unicodedata.category(c) != "Mn")


def _normal_con_mapa(texto: str) -> tuple[str, list[int]]:
    """Como _normal, pero también dice de qué posición del original salió cada letra."""
    letras, mapa = [], []
    for i, c in enumerate(texto):
        for n in _normal(c):
            letras.append(n)
            mapa.append(i)
    return "".join(letras), mapa


def _recorte(texto: str, inicio: int, fin: int, antes: int = 60, despues: int = 110) -> str:
    a = max(0, inicio - antes)
    b = min(len(texto), fin + despues)
    if a > 0:
        espacio = texto.find(" ", a, inicio)
        a = espacio + 1 if espacio >= 0 else a
    if b < len(texto):
        espacio = texto.rfind(" ", fin, b)
        b = espacio if espacio > 0 else b
    trozo = " ".join(texto[a:b].split())
    return ("…" if a > 0 else "") + trozo + ("…" if b < len(texto) else "")


def limpiar_nombre(titulo: str) -> str:
    """Un nombre de archivo que sirve en cualquier sistema y en Obsidian (sin \\ / : * ? " < > | # ^ [ ])."""
    nombre = " ".join(PROHIBIDOS.sub(" ", titulo or "").split()).strip().lstrip(".").strip()
    return nombre[:180] or "Sin título"


# ------------------------------------------------------------------ el vault

class Vault:
    """Una carpeta de notas .md. Se crea si no existe."""

    def __init__(self, ruta):
        self.raiz = Path(ruta).expanduser().resolve()
        self.raiz.mkdir(parents=True, exist_ok=True)
        self.nombre = self.raiz.name
        self._cache: dict[str, tuple[int, int, dict]] = {}

    def __repr__(self) -> str:
        return f"Vault({str(self.raiz)!r})"

    # ---------------------------------------------------------------- rutas

    def _rel(self, ruta) -> str:
        """Normaliza una ruta relativa y revisa que no se salga del vault."""
        texto = str(ruta).replace("\\", "/").strip()
        if Path(texto).is_absolute():
            try:
                texto = Path(texto).resolve().relative_to(self.raiz).as_posix()
            except ValueError:
                raise ErrorVault(f"«{ruta}» no está dentro del vault.") from None
        partes = [p for p in PurePosixPath(texto).parts if p not in ("", ".", "/")]
        if not partes or ".." in partes:
            raise ErrorVault(f"«{ruta}» no es una ruta válida dentro del vault.")
        return "/".join(partes)

    def ruta_absoluta(self, ruta) -> Path:
        return self.raiz / self._rel(ruta)

    def existe(self, ruta) -> bool:
        try:
            return self.ruta_absoluta(ruta).is_file()
        except ErrorVault:
            return False

    def _recorrer(self):
        """Las rutas relativas de todas las notas, sin entrar a carpetas ocultas."""
        for carpeta, subcarpetas, archivos in os.walk(self.raiz):
            subcarpetas[:] = sorted(d for d in subcarpetas if not d.startswith("."))
            base = Path(carpeta).relative_to(self.raiz).as_posix()
            for archivo in sorted(archivos):
                if archivo.lower().endswith(".md") and not archivo.startswith("."):
                    yield archivo if base == "." else f"{base}/{archivo}"

    # ---------------------------------------------------------------- lo que dice cada nota (con caché)

    def _info(self, rel: str) -> dict | None:
        ruta = self.raiz / rel
        try:
            st = ruta.stat()
        except OSError:
            self._cache.pop(rel, None)
            return None
        guardado = self._cache.get(rel)
        if guardado and guardado[0] == st.st_mtime_ns and guardado[1] == st.st_size:
            return guardado[2]
        try:
            texto = ruta.read_text(encoding="utf-8", errors="replace")
        except OSError:
            return None
        frente, _ = separar_frontmatter(texto)
        info = {
            "ruta": rel,
            "titulo": PurePosixPath(rel).stem,
            "modificado": st.st_mtime,
            "etiquetas": etiquetas_de(texto),
            "alias": alias_de(frente),
            "carpeta": PurePosixPath(rel).parent.as_posix() if "/" in rel else "",
            "bytes": st.st_size,
            "_enlaces": [{k: e[k] for k in ("nombre", "seccion", "texto", "incrustado")} for e in enlaces_de(texto)],
        }
        self._cache[rel] = (st.st_mtime_ns, st.st_size, info)
        return info

    def _todas(self) -> list[dict]:
        salida = []
        for rel in self._recorrer():
            info = self._info(rel)
            if info is not None:
                salida.append(info)
        vivas = {i["ruta"] for i in salida}
        for rel in [r for r in self._cache if r not in vivas]:
            del self._cache[rel]
        return salida

    @staticmethod
    def _publica(info: dict) -> dict:
        return {k: (list(v) if isinstance(v, list) else v) for k, v in info.items() if not k.startswith("_")}

    def notas(self) -> list[dict]:
        """Todas las notas: {"ruta", "titulo", "modificado", "etiquetas", "alias", "carpeta", "bytes"}."""
        return [self._publica(i) for i in self._todas()]

    def frontmatter(self, ruta) -> dict:
        return separar_frontmatter(self.leer(ruta))[0]

    # ---------------------------------------------------------------- resolver enlaces

    def _mapa(self, todas: list[dict] | None = None) -> dict:
        todas = self._todas() if todas is None else todas
        por_nombre: dict[str, list[str]] = {}
        por_ruta: dict[str, str] = {}
        por_alias: dict[str, list[str]] = {}
        for info in todas:
            rel = info["ruta"]
            por_nombre.setdefault(info["titulo"].lower(), []).append(rel)
            por_ruta[rel[:-3].lower()] = rel
            for a in info["alias"]:
                por_alias.setdefault(a.lower(), []).append(rel)
        return {"nombre": por_nombre, "ruta": por_ruta, "alias": por_alias}

    @staticmethod
    def _elegir(candidatas: list[str], desde: str | None) -> str:
        if len(candidatas) == 1:
            return candidatas[0]
        carpeta = PurePosixPath(desde).parent.as_posix() if desde and "/" in desde else ""
        misma = [c for c in candidatas if (PurePosixPath(c).parent.as_posix() if "/" in c else "") == carpeta]
        return sorted(misma or candidatas, key=lambda c: (c.count("/"), len(c), c))[0]

    def _resolver(self, mapa: dict, nombre_enlace: str, desde: str | None = None) -> str | None:
        nombre, _, _ = _partir_enlace(nombre_enlace.strip().lstrip("!").removeprefix("[[").removesuffix("]]"))
        if not nombre:
            return desde          # [[#Sección]]: un enlace a la misma nota
        clave = nombre.replace("\\", "/").strip("/").lower()
        if clave.endswith(".md"):
            clave = clave[:-3]
        if "/" in clave:
            if clave in mapa["ruta"]:
                return mapa["ruta"][clave]
            sufijo = [r for k, r in mapa["ruta"].items() if k.endswith("/" + clave)]
            if sufijo:
                return self._elegir(sufijo, desde)
            return None
        if clave in mapa["nombre"]:
            return self._elegir(mapa["nombre"][clave], desde)
        if clave in mapa["alias"]:
            return self._elegir(mapa["alias"][clave], desde)
        return None

    def resolver(self, nombre_enlace: str, desde: str | None = None) -> str | None:
        """La nota a la que lleva un enlace («Nota», «Nota#sección», «Carpeta/Nota», un alias…), o None."""
        return self._resolver(self._mapa(), nombre_enlace, desde)

    # ---------------------------------------------------------------- archivos

    def arbol(self) -> dict:
        """Las carpetas y notas, para la vista de archivos (carpetas primero, luego notas)."""
        def carpeta(ruta_abs: Path, rel: str) -> dict:
            nodo = {"nombre": ruta_abs.name if rel else self.nombre, "ruta": rel, "tipo": "carpeta", "hijos": []}
            try:
                hijos = sorted(ruta_abs.iterdir(), key=lambda p: p.name.lower())
            except OSError:
                return nodo
            subs = [h for h in hijos if h.is_dir() and not h.name.startswith(".")]
            notas = [h for h in hijos if h.is_file() and h.name.lower().endswith(".md") and not h.name.startswith(".")]
            for s in subs:
                nodo["hijos"].append(carpeta(s, f"{rel}/{s.name}" if rel else s.name))
            for n in notas:
                ruta_n = f"{rel}/{n.name}" if rel else n.name
                try:
                    modificado = n.stat().st_mtime
                except OSError:
                    modificado = 0.0
                nodo["hijos"].append({"nombre": n.name, "ruta": ruta_n, "tipo": "nota", "titulo": n.stem,
                                      "modificado": modificado})
            return nodo
        return carpeta(self.raiz, "")

    def leer(self, ruta) -> str:
        return self.ruta_absoluta(ruta).read_text(encoding="utf-8", errors="replace")

    def guardar(self, ruta, texto: str) -> None:
        """Guarda la nota sin riesgo de dejarla a medias (temporal + os.replace)."""
        destino = self.ruta_absoluta(ruta)
        destino.parent.mkdir(parents=True, exist_ok=True)
        fd, temporal = tempfile.mkstemp(prefix=".", suffix=".tmp", dir=destino.parent)
        try:
            with os.fdopen(fd, "w", encoding="utf-8", newline="") as f:
                f.write(texto)
                f.flush()
                os.fsync(f.fileno())
            try:
                os.chmod(temporal, destino.stat().st_mode & 0o777)
            except OSError:
                os.chmod(temporal, 0o644)
            os.replace(temporal, destino)
        except BaseException:
            try:
                os.unlink(temporal)
            except OSError:
                pass
            raise
        self._cache.pop(self._rel(ruta), None)

    def _libre(self, carpeta: str, nombre: str) -> str:
        """«carpeta/nombre.md», o «nombre 1.md», «nombre 2.md»… si ya existe."""
        prefijo = f"{carpeta}/" if carpeta else ""
        candidata, n = f"{prefijo}{nombre}.md", 1
        while (self.raiz / candidata).exists():
            candidata = f"{prefijo}{nombre} {n}.md"
            n += 1
        return candidata

    def crear(self, titulo: str, carpeta: str = "") -> str:
        """Crea una nota vacía y regresa su ruta."""
        carpeta = self._rel(carpeta) if str(carpeta).strip("/ .") else ""
        rel = self._libre(carpeta, limpiar_nombre(titulo))
        self.guardar(rel, "")
        return rel

    def crear_carpeta(self, ruta) -> str:
        rel = self._rel(ruta)
        (self.raiz / rel).mkdir(parents=True, exist_ok=True)
        return rel

    def renombrar(self, ruta, nuevo: str) -> str:
        """Le cambia el nombre (o la mueve, si `nuevo` es una ruta) y arregla los [[enlaces]] que le llegaban."""
        vieja = self._rel(ruta)
        if not (self.raiz / vieja).is_file():
            raise FileNotFoundError(vieja)
        nuevo = str(nuevo).strip()
        if "/" in nuevo or "\\" in nuevo or nuevo.lower().endswith(".md"):
            nueva = self._rel(nuevo)
            if not nueva.lower().endswith(".md"):
                nueva += ".md"
            partes = nueva.split("/")
            partes[-1] = limpiar_nombre(partes[-1][:-3]) + ".md"
            nueva = "/".join(partes)
        else:
            carpeta = PurePosixPath(vieja).parent.as_posix() if "/" in vieja else ""
            nueva = (f"{carpeta}/" if carpeta else "") + limpiar_nombre(nuevo) + ".md"
        if nueva == vieja:
            return vieja
        destino = self.raiz / nueva
        if destino.exists() and not (vieja.lower() == nueva.lower() and destino.samefile(self.raiz / vieja)):
            raise FileExistsError(f"Ya hay una nota llamada «{PurePosixPath(nueva).stem}».")

        # Antes de mover: qué enlaces de qué notas llevan a la vieja (y no por un alias, que sigue sirviendo).
        todas = self._todas()
        mapa = self._mapa(todas)
        alias_viejos = {a.lower() for i in todas if i["ruta"] == vieja for a in i["alias"]}
        por_cambiar: dict[str, list[dict]] = {}
        for info in todas:
            if not any(e["nombre"] for e in info["_enlaces"]):
                continue
            texto = self.leer(info["ruta"])
            for e in enlaces_de(texto):
                if not e["nombre"] or e["nombre"].lower() in alias_viejos:
                    continue
                if self._resolver(mapa, e["nombre"], info["ruta"]) == vieja:
                    por_cambiar.setdefault(info["ruta"], []).append(e)

        destino.parent.mkdir(parents=True, exist_ok=True)
        os.replace(self.raiz / vieja, destino)
        self._cache.pop(vieja, None)

        mapa_nuevo = self._mapa()
        titulo = PurePosixPath(nueva).stem
        unica = len(mapa_nuevo["nombre"].get(titulo.lower(), [])) == 1
        for rel, enlaces in por_cambiar.items():
            rel = nueva if rel == vieja else rel
            texto = self.leer(rel)
            for e in sorted(enlaces, key=lambda e: e["inicio"], reverse=True):
                destino_enlace = titulo if unica and "/" not in e["nombre"] else nueva[:-3]
                interior = destino_enlace
                if e["seccion"]:
                    interior += e["seccion"] if e["seccion"].startswith("^") else "#" + e["seccion"]
                if e["texto"]:
                    interior += "|" + e["texto"]
                texto = texto[:e["inicio"]] + ("!" if e["incrustado"] else "") + "[[" + interior + "]]" + texto[e["fin"]:]
            self.guardar(rel, texto)
        return nueva

    def borrar(self, ruta) -> str:
        """La manda a .trash/ del vault (como Obsidian; de ahí se puede recuperar). Regresa su ruta ahí."""
        rel = self._rel(ruta)
        origen = self.raiz / rel
        if not origen.exists():
            raise FileNotFoundError(rel)
        papelera = self.raiz / PAPELERA
        papelera.mkdir(exist_ok=True)
        base = PurePosixPath(rel)
        destino, n = papelera / base.name, 1
        while destino.exists():
            destino = papelera / f"{base.stem} {n}{base.suffix}"
            n += 1
        os.replace(origen, destino)
        self._cache.pop(rel, None)
        return f"{PAPELERA}/{destino.name}"

    # ---------------------------------------------------------------- buscar

    def buscar(self, texto: str) -> list[dict]:
        """Las notas que tienen todas las palabras (en el título o el texto), sin importar mayúsculas ni
        acentos: {"ruta", "titulo", "fragmento", "coincidencias"}. «#etiqueta» busca por etiqueta."""
        consulta = (texto or "").strip()
        if not consulta:
            return []
        resultados = []
        if consulta.startswith("#") and " " not in consulta:
            etiqueta = consulta[1:].lower()
            for info in self._todas():
                if any(t == etiqueta or t.startswith(etiqueta + "/") for t in info["etiquetas"]):
                    contenido = self.leer(info["ruta"])
                    i = contenido.lower().find(consulta.lower())
                    fragmento = _recorte(contenido, i, i + len(consulta)) if i >= 0 else _recorte(contenido, 0, 0)
                    resultados.append({"ruta": info["ruta"], "titulo": info["titulo"], "fragmento": fragmento,
                                       "coincidencias": 1, "_t": 0, "_m": info["modificado"]})
        else:
            palabras = [_normal(p) for p in consulta.split() if _normal(p)]
            for info in self._todas():
                titulo_n = _normal(info["titulo"])
                contenido = self.leer(info["ruta"])
                normal, mapa = _normal_con_mapa(contenido)
                cuentas = [normal.count(p) + titulo_n.count(p) for p in palabras]
                if not all(cuentas):
                    continue
                en_titulo = sum(p in titulo_n for p in palabras)
                fragmento = ""
                for p in palabras:
                    i = normal.find(p)
                    if i >= 0:
                        fragmento = _recorte(contenido, mapa[i], mapa[i + len(p) - 1] + 1)
                        break
                if not fragmento:
                    fragmento = _recorte(separar_frontmatter(contenido)[1], 0, 0)
                resultados.append({"ruta": info["ruta"], "titulo": info["titulo"], "fragmento": fragmento,
                                   "coincidencias": sum(cuentas), "_t": -en_titulo, "_m": info["modificado"]})
        resultados.sort(key=lambda r: (r["_t"], -r["coincidencias"], -r["_m"], r["ruta"]))
        return [{k: v for k, v in r.items() if not k.startswith("_")} for r in resultados]

    # ---------------------------------------------------------------- enlaces y grafo

    def enlaces(self, ruta) -> list[dict]:
        """Los [[enlaces]] que salen de la nota: {"nombre", "ruta" (o None), "existe", "seccion", "texto"}."""
        rel = self._rel(ruta)
        info = self._info(rel)
        if info is None:
            raise FileNotFoundError(rel)
        mapa = self._mapa()
        salida, vistos = [], set()
        for e in info["_enlaces"]:
            clave = (e["nombre"].lower(), e["seccion"].lower())
            if clave in vistos:
                continue
            vistos.add(clave)
            destino = self._resolver(mapa, e["nombre"], rel) if e["nombre"] else rel
            salida.append({"nombre": e["nombre"], "ruta": destino, "existe": destino is not None,
                           "seccion": e["seccion"], "texto": e["texto"]})
        return salida

    def retroenlaces(self, ruta) -> list[dict]:
        """Las notas que enlazan a esta: {"ruta", "titulo", "fragmento"} (la línea donde está el enlace)."""
        rel = self._rel(ruta)
        todas = self._todas()
        mapa = self._mapa(todas)
        salida = []
        for info in todas:
            if info["ruta"] == rel:
                continue
            if not any(e["nombre"] and self._resolver(mapa, e["nombre"], info["ruta"]) == rel for e in info["_enlaces"]):
                continue
            texto = self.leer(info["ruta"])
            fragmento = ""
            for e in enlaces_de(texto):
                if e["nombre"] and self._resolver(mapa, e["nombre"], info["ruta"]) == rel:
                    inicio = texto.rfind("\n", 0, e["inicio"]) + 1
                    fin = texto.find("\n", e["fin"])
                    linea = texto[inicio:fin if fin >= 0 else len(texto)].strip()
                    fragmento = linea if len(linea) <= 220 else _recorte(linea, e["inicio"] - inicio, e["fin"] - inicio)
                    break
            salida.append({"ruta": info["ruta"], "titulo": info["titulo"], "fragmento": fragmento})
        return salida

    def grafo(self) -> dict:
        """{"nodos": [{"id", "titulo", "etiquetas", "grado", "existe"}], "aristas": [{"origen", "destino"}]}.

        Los enlaces a notas que todavía no existen salen como nodos con existe=False e id «?nombre».
        Los incrustados de imágenes u otros archivos no cuentan.
        """
        todas = self._todas()
        mapa = self._mapa(todas)
        nodos = {i["ruta"]: {"id": i["ruta"], "titulo": i["titulo"], "etiquetas": list(i["etiquetas"]),
                             "grado": 0, "existe": True} for i in todas}
        aristas: set[tuple[str, str]] = set()
        for info in todas:
            for e in info["_enlaces"]:
                nombre = e["nombre"]
                if not nombre:
                    continue
                sufijo = PurePosixPath(nombre).suffix.lower()
                if sufijo and sufijo != ".md" and re.fullmatch(r"\.[a-z0-9]{2,5}", sufijo):
                    continue   # foto.png, documento.pdf…
                destino = self._resolver(mapa, nombre, info["ruta"])
                if destino is None:
                    destino = "?" + nombre
                    nodos.setdefault(destino, {"id": destino, "titulo": nombre, "etiquetas": [], "grado": 0,
                                               "existe": False})
                if destino == info["ruta"]:
                    continue
                aristas.add((info["ruta"], destino))
        for origen, destino in aristas:
            nodos[origen]["grado"] += 1
            nodos[destino]["grado"] += 1
        return {"nodos": list(nodos.values()),
                "aristas": [{"origen": o, "destino": d} for o, d in sorted(aristas)]}

    def etiquetas(self) -> dict[str, int]:
        """{"etiqueta": cuántas notas la tienen}, de la más usada a la menos."""
        cuenta = Counter(t for i in self._todas() for t in i["etiquetas"])
        return dict(sorted(cuenta.items(), key=lambda kv: (-kv[1], kv[0])))
