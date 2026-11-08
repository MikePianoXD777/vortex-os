"""«Preguntar a mi vault»: la IA contesta con lo que dicen tus notas (RAG).

Cada nota se parte en fragmentos de ~800 caracteres (por sección y párrafo) y un modelo de embeddings
(qwen3-embedding, nomic-embed-text…) los vuelve vectores. Se guardan en un SQLite en CACHE/memoria/; al
preguntar se busca el fragmento más parecido (similitud coseno, en Python puro; con numpy si está) y se
le pasa al modelo de chat pidiéndole que cite con [[título]].

El índice es incremental: solo vuelve a leer las notas que cambiaron (por fecha, y luego por hash) y saca
las que se borraron. Si no hay modelo de embeddings, busca por palabras con Vault.buscar y lo avisa.
"""
from __future__ import annotations

import hashlib
import math
import operator
import re
import sqlite3
import threading
from array import array
from pathlib import Path

from . import catalogo, rutas
from .ollama import ErrorOllama, Ollama
from .vault import Vault, _normal, separar_frontmatter

TAMANO = 800          # caracteres por fragmento, más o menos
LOTE = 16             # fragmentos por llamada a /api/embed
POR_NOTA = 2          # como mucho, fragmentos de la misma nota en un contexto

try:   # numpy es opcional: solo lo hace más rápido con vaults grandes
    import numpy as _np
except ImportError:   # pragma: no cover
    _np = None

_TITULO = re.compile(r"^(#{1,6})\s+(.*\S)\s*$")
_ORACION = re.compile(r"(?<=[.!?…])\s+")

PALABRAS_VACIAS = set("""
a al algo algun alguna algunas alguno algunos ante antes aqui asi aun bien cada como con contra cual cuales cuando
cuanto de del desde donde dos el ella ellas ellos en entre era eran es esa esas ese eso esos esta estaba estan estar
estas este esto estos fue fueron ha han hasta hay la las le les lo los mas me mi mis mucho muy nada ni no nos nosotros
nuestra nuestro o os otra otro para pero poco por porque que quien quienes se sea segun ser si sin sobre solo son su
sus tambien tan tanto te tengo ti tiene tienen todo todos tu tus un una unas uno unos usted ya yo dime dice decir hago
hace hacer puedo puede sabes saber cosa cosas mis notas nota vault
the of and to in is it for on that this what how
""".split())


def fragmentar(texto: str) -> list[tuple[str, str]]:
    """[(sección, texto)] de ~TAMANO caracteres, sin el frontmatter. La sección es el último título (#)."""
    _, cuerpo = separar_frontmatter(texto)
    secciones: list[tuple[str, list[str]]] = [("", [])]
    parrafo: list[str] = []
    en_codigo = False

    def cerrar():
        if parrafo:
            secciones[-1][1].append("\n".join(parrafo).strip())
            parrafo.clear()

    for linea in cuerpo.split("\n"):
        if linea.lstrip().startswith(("```", "~~~")):
            en_codigo = not en_codigo
        m = None if en_codigo else _TITULO.match(linea)
        if m:
            cerrar()
            secciones.append((m.group(2).strip(), []))
        elif not linea.strip() and not en_codigo:
            cerrar()
        else:
            parrafo.append(linea.rstrip())
    cerrar()

    salida: list[tuple[str, str]] = []
    for seccion, parrafos in secciones:
        piezas: list[str] = []
        for p in parrafos:
            if not p:
                continue
            if len(p) <= TAMANO:
                piezas.append(p)
                continue
            actual = ""
            for oracion in _ORACION.split(p):
                while len(oracion) > TAMANO:
                    if actual:
                        piezas.append(actual)
                        actual = ""
                    corte = oracion.rfind(" ", 0, TAMANO)
                    corte = corte if corte > TAMANO // 2 else TAMANO
                    piezas.append(oracion[:corte].strip())
                    oracion = oracion[corte:].strip()
                if actual and len(actual) + 1 + len(oracion) > TAMANO:
                    piezas.append(actual)
                    actual = ""
                actual = f"{actual} {oracion}".strip()
            if actual:
                piezas.append(actual)
        actual = ""
        for pieza in piezas:
            if actual and len(actual) + 2 + len(pieza) > TAMANO:
                salida.append((seccion, actual))
                actual = ""
            actual = f"{actual}\n\n{pieza}" if actual else pieza
        if actual:
            salida.append((seccion, actual))
        elif seccion and not piezas:
            salida.append((seccion, seccion))   # un título sin texto también dice algo
    return [(s, t) for s, t in salida if t.strip()]


def _palabras(texto: str) -> list[str]:
    vistas: dict[str, None] = {}
    for p in re.findall(r"[\w#-]+", _normal(texto)):
        p = p.strip("-")
        if len(p) >= 3 and p not in PALABRAS_VACIAS:
            vistas.setdefault(p, None)
    return list(vistas)


def _vector_a_bytes(v: list[float]) -> bytes:
    return array("f", v).tobytes()


def _bytes_a_vector(b: bytes) -> array:
    a = array("f")
    a.frombytes(b)
    return a


class Memoria:
    """El índice de un vault para preguntarle cosas. Úsala desde un hilo aparte: tarda."""

    def __init__(self, vault: Vault, ollama: Ollama, modelo_embeddings: str | None = None):
        self.vault = vault
        self.ollama = ollama
        self._modelo_fijo = modelo_embeddings
        self._modelo: str | None = modelo_embeddings
        self.ultimo_contexto: list[dict] = []
        self._cache_vectores: tuple[str, list] | None = None
        self._candado = threading.Lock()
        raiz = str(vault.raiz)
        nombre = re.sub(r"[^\w-]+", "-", vault.nombre).strip("-")[:40] or "vault"
        self.ruta_indice: Path = rutas.CACHE / "memoria" / f"{nombre}-{hashlib.sha1(raiz.encode()).hexdigest()[:10]}.sqlite"

    # ---------------------------------------------------------------- modelo y modo

    def _buscar_modelo(self) -> str | None:
        if self._modelo_fijo:
            return self._modelo_fijo
        try:
            instalados = self.ollama.modelos()
        except ErrorOllama:
            return self._modelo     # sin Ollama: el que ya se había encontrado (si hubo)
        nombres = [m["nombre"] for m in instalados]

        def tiene(id: str) -> str | None:
            for n in nombres:
                if n == id or n == f"{id}:latest" or (":" not in id and n.split(":")[0] == id):
                    return n
            return None
        for preferido in [catalogo.EMBEDDINGS] + [m["id"] for m in catalogo.de_tipo("vault")]:
            encontrado = tiene(preferido)
            if encontrado:
                return encontrado
        for m in instalados:
            if m.get("embeddings"):
                return m["nombre"]
        return None

    @property
    def modelo_embeddings(self) -> str | None:
        return self._modelo

    @property
    def modo(self) -> str:
        """"embeddings" si hay modelo de embeddings, si no "palabras"."""
        return "embeddings" if self._modelo else "palabras"

    @property
    def aviso(self) -> str:
        if self._modelo:
            return ""
        return ("Busco en tus notas por palabras porque no tienes un modelo para el vault. Baja "
                f"«{catalogo.EMBEDDINGS}» en Modelos para que entienda lo que quieres decir, no solo las palabras.")

    # ---------------------------------------------------------------- SQLite

    def _conectar(self) -> sqlite3.Connection:
        self.ruta_indice.parent.mkdir(parents=True, exist_ok=True)
        db = sqlite3.connect(self.ruta_indice, timeout=10)
        db.executescript("""
            CREATE TABLE IF NOT EXISTS meta (clave TEXT PRIMARY KEY, valor TEXT);
            CREATE TABLE IF NOT EXISTS notas (ruta TEXT PRIMARY KEY, mtime REAL, hash TEXT);
            CREATE TABLE IF NOT EXISTS fragmentos (id INTEGER PRIMARY KEY, ruta TEXT, titulo TEXT, seccion TEXT,
                                                   texto TEXT, vector BLOB, norma REAL);
            CREATE INDEX IF NOT EXISTS fragmentos_ruta ON fragmentos(ruta);
        """)
        return db

    @staticmethod
    def _meta(db: sqlite3.Connection, clave: str) -> str | None:
        fila = db.execute("SELECT valor FROM meta WHERE clave=?", (clave,)).fetchone()
        return fila[0] if fila else None

    @staticmethod
    def _poner_meta(db: sqlite3.Connection, clave: str, valor: str) -> None:
        db.execute("INSERT OR REPLACE INTO meta VALUES (?, ?)", (clave, valor))

    def estado(self) -> dict:
        with self._candado:
            db = self._conectar()
            try:
                return {"notas": db.execute("SELECT COUNT(*) FROM notas").fetchone()[0],
                        "fragmentos": db.execute("SELECT COUNT(*) FROM fragmentos").fetchone()[0],
                        "modelo": self._meta(db, "modelo") or self._modelo, "modo": self.modo}
            finally:
                db.close()

    def borrar_indice(self) -> None:
        with self._candado:
            self._cache_vectores = None
            for sufijo in ("", "-wal", "-shm", "-journal"):
                try:
                    Path(str(self.ruta_indice) + sufijo).unlink()
                except FileNotFoundError:
                    pass

    # ---------------------------------------------------------------- prefijos de cada modelo

    def _texto_documento(self, titulo: str, seccion: str, texto: str) -> str:
        cabeza = titulo + (f" › {seccion}" if seccion and seccion != titulo else "")
        base = f"{cabeza}\n{texto}"
        return ("search_document: " + base) if "nomic" in (self._modelo or "") else base

    def _texto_pregunta(self, pregunta: str) -> str:
        modelo = self._modelo or ""
        if "nomic" in modelo:
            return "search_query: " + pregunta
        if "qwen3-embedding" in modelo:
            return f"Instruct: Dada una pregunta, encuentra las notas que la contestan\nQuery: {pregunta}"
        return pregunta

    # ---------------------------------------------------------------- índice

    def actualizar(self, al_progreso=None, cancelar: threading.Event | None = None) -> dict:
        """Pone al día el índice. al_progreso(hechas, total, ruta). Regresa lo que hizo."""
        self._modelo = self._buscar_modelo()
        notas = self.vault.notas()
        resumen = {"notas": len(notas), "nuevas": 0, "quitadas": 0, "fragmentos": 0, "cancelado": False}
        if not self._modelo:
            return resumen
        with self._candado:
            db = self._conectar()
            try:
                if self._meta(db, "modelo") != self._modelo:
                    db.execute("DELETE FROM fragmentos")
                    db.execute("DELETE FROM notas")
                    self._poner_meta(db, "modelo", self._modelo)
                    db.commit()
                vivas = {n["ruta"]: n for n in notas}
                guardadas = {r: (m, h) for r, m, h in db.execute("SELECT ruta, mtime, hash FROM notas")}
                for ruta in [r for r in guardadas if r not in vivas]:
                    db.execute("DELETE FROM fragmentos WHERE ruta=?", (ruta,))
                    db.execute("DELETE FROM notas WHERE ruta=?", (ruta,))
                    resumen["quitadas"] += 1
                db.commit()

                pendientes: list[tuple[dict, str, str]] = []
                for ruta, nota in vivas.items():
                    anterior = guardadas.get(ruta)
                    if anterior and anterior[0] == nota["modificado"]:
                        continue
                    try:
                        texto = self.vault.leer(ruta)
                    except OSError:
                        continue
                    huella = hashlib.sha1(texto.encode("utf-8", "replace")).hexdigest()
                    if anterior and anterior[1] == huella:
                        db.execute("UPDATE notas SET mtime=? WHERE ruta=?", (nota["modificado"], ruta))
                        continue
                    pendientes.append((nota, texto, huella))
                db.commit()

                total = len(pendientes)
                for i, (nota, texto, huella) in enumerate(pendientes):
                    if cancelar is not None and cancelar.is_set():
                        resumen["cancelado"] = True
                        break
                    trozos = fragmentar(texto)
                    vectores: list[list[float]] = []
                    for j in range(0, len(trozos), LOTE):
                        if cancelar is not None and cancelar.is_set():
                            resumen["cancelado"] = True
                            break
                        lote = trozos[j:j + LOTE]
                        vectores += self.ollama.embeber(
                            self._modelo, [self._texto_documento(nota["titulo"], s, t) for s, t in lote])
                    if resumen["cancelado"]:
                        break
                    db.execute("DELETE FROM fragmentos WHERE ruta=?", (nota["ruta"],))
                    for (seccion, trozo), vector in zip(trozos, vectores):
                        norma = math.sqrt(sum(x * x for x in vector)) or 1.0
                        db.execute("INSERT INTO fragmentos (ruta, titulo, seccion, texto, vector, norma) "
                                   "VALUES (?, ?, ?, ?, ?, ?)",
                                   (nota["ruta"], nota["titulo"], seccion, trozo, _vector_a_bytes(vector), norma))
                    db.execute("INSERT OR REPLACE INTO notas VALUES (?, ?, ?)", (nota["ruta"], nota["modificado"], huella))
                    db.commit()
                    resumen["nuevas"] += 1
                    if al_progreso is not None:
                        al_progreso(i + 1, total, nota["ruta"])
                resumen["fragmentos"] = db.execute("SELECT COUNT(*) FROM fragmentos").fetchone()[0]
                self._poner_meta(db, "version", str(int(self._meta(db, "version") or 0) + 1))
                db.commit()
            finally:
                db.close()
            self._cache_vectores = None
        return resumen

    def _vectores(self) -> list:
        """Todos los fragmentos con su vector (en memoria mientras el índice no cambie)."""
        db = self._conectar()
        try:
            version = f"{self._meta(db, 'modelo')}:{self._meta(db, 'version')}"
            if self._cache_vectores and self._cache_vectores[0] == version:
                return self._cache_vectores[1]
            filas = [(r, t, s, x, _bytes_a_vector(v), n) for r, t, s, x, v, n in
                     db.execute("SELECT ruta, titulo, seccion, texto, vector, norma FROM fragmentos")]
        finally:
            db.close()
        self._cache_vectores = (version, filas)
        return filas

    # ---------------------------------------------------------------- buscar contexto

    def contexto(self, pregunta: str, k: int = 6) -> list[dict]:
        """Los k fragmentos que mejor contestan la pregunta: {"ruta", "titulo", "fragmento", "puntaje"}."""
        if not (pregunta or "").strip():
            return []
        if self._modelo is None:
            self._modelo = self._buscar_modelo()
        filas = []
        if self._modelo:
            with self._candado:
                filas = self._vectores()
        if not filas:
            return self._por_palabras(pregunta, k)
        consulta = self.ollama.embeber(self._modelo, [self._texto_pregunta(pregunta)])[0]
        norma_q = math.sqrt(sum(x * x for x in consulta)) or 1.0
        if _np is not None and filas and len(filas[0][4]) == len(consulta):
            matriz = _np.array([f[4] for f in filas], dtype=_np.float32)
            normas = _np.array([f[5] for f in filas], dtype=_np.float32)
            puntajes = (matriz @ _np.array(consulta, dtype=_np.float32)) / (normas * norma_q)
            puntajes = [float(p) for p in puntajes]
        else:
            puntajes = [sum(map(operator.mul, f[4], consulta)) / (f[5] * norma_q) if len(f[4]) == len(consulta) else -1.0
                        for f in filas]
        orden = sorted(range(len(filas)), key=lambda i: puntajes[i], reverse=True)
        salida, por_nota = [], {}
        for i in orden:
            ruta, titulo, seccion, texto, _, _ = filas[i]
            if por_nota.get(ruta, 0) >= POR_NOTA or not self.vault.existe(ruta):
                continue
            por_nota[ruta] = por_nota.get(ruta, 0) + 1
            salida.append({"ruta": ruta, "titulo": titulo, "fragmento": texto, "puntaje": round(puntajes[i], 4),
                           "seccion": seccion})
            if len(salida) >= k:
                break
        return salida

    def _por_palabras(self, pregunta: str, k: int) -> list[dict]:
        """Sin embeddings: las notas que más palabras de la pregunta tienen, con su mejor fragmento."""
        palabras = _palabras(pregunta)
        if not palabras:
            return []
        notas: dict[str, dict] = {}
        for p in palabras:
            for r in self.vault.buscar(p):
                n = notas.setdefault(r["ruta"], {"titulo": r["titulo"], "palabras": 0, "veces": 0})
                n["palabras"] += 1
                n["veces"] += r.get("coincidencias", 1)
        orden = sorted(notas.items(), key=lambda kv: (-kv[1]["palabras"], -kv[1]["veces"], kv[0]))[:k]
        salida = []
        for ruta, n in orden:
            try:
                trozos = fragmentar(self.vault.leer(ruta)) or [("", n["titulo"])]
            except OSError:
                continue
            mejor = max(trozos, key=lambda st: sum(_normal(st[0] + " " + st[1]).count(p) for p in palabras))
            salida.append({"ruta": ruta, "titulo": n["titulo"], "fragmento": mejor[1],
                           "puntaje": round(n["palabras"] / len(palabras), 4), "seccion": mejor[0]})
        return salida

    # ---------------------------------------------------------------- mensajes para el chat

    def mensajes_con_contexto(self, pregunta: str, historial: list | None = None) -> list:
        """[system, *historial, pregunta] para Ollama.chat, con los fragmentos de tus notas en el system."""
        fragmentos = self.contexto(pregunta)
        self.ultimo_contexto = fragmentos
        partes = [
            "Eres la IA de Vortexine's Local AI y contestas preguntas sobre las notas del vault de la persona.",
            "Contesta siempre en español, claro y directo.",
            "Usa la información de las notas de abajo. Si no alcanza para contestar, dilo con honestidad y no "
            "inventes nada.",
            "Cada vez que uses algo de una nota, cítala con su título entre dobles corchetes, exactamente como "
            "aparece, por ejemplo [[Título de la nota]].",
        ]
        if fragmentos:
            if self.modo == "palabras" or not self._modelo:
                partes.append("(Estas notas se encontraron buscando las palabras de la pregunta.)")
            partes.append("\nFragmentos de las notas:")
            for f in fragmentos:
                cabeza = f"[[{f['titulo']}]]" + (f" › {f['seccion']}" if f.get("seccion") else "")
                partes.append(f"\n--- {cabeza} ({f['ruta']}) ---\n{f['fragmento'].strip()}")
        else:
            partes.append("\nNo encontré notas relacionadas con esta pregunta. Dilo, y si contestas algo general, "
                          "aclara que no viene de sus notas.")
        mensajes = [{"role": "system", "content": "\n".join(partes)}]
        for m in historial or []:
            if isinstance(m, dict) and m.get("role") in ("user", "assistant") and m.get("content") is not None:
                limpio = {"role": m["role"], "content": m["content"]}
                if m.get("images"):
                    limpio["images"] = m["images"]
                mensajes.append(limpio)
        mensajes.append({"role": "user", "content": pregunta})
        return mensajes
