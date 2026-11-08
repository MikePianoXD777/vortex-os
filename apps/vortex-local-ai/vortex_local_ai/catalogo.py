"""El catálogo de la Vortexine: los modelos que recomendamos bajar, y cuáles le caben a tu PC.

Los nombres y tamaños salen de ollama.com/library (páginas de «tags», revisadas el 07/11/2026). `gb` es
la descarga en Linux (el GGUF que baja la etiqueta, casi siempre Q4_K_M). `vram_gb` es lo que necesita
para ir completo en la tarjeta de video con el contexto de 8K de Vortexine's (los pesos más la memoria
de la plática, ~1-2 GB más).

Si un modelo no te cabe completo, Ollama manda una parte a la CPU: funciona, pero más lento. Los modelos
«moe» (mezcla de expertos) aguantan mucho mejor eso, porque solo usan una parte por palabra.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

FUENTE = "ollama.com/library (tags revisadas el 07/11/2026)"

TIPOS = {
    "chat": "Platicar",
    "codigo": "Programar",
    "razonamiento": "Pensar a fondo",
    "vision": "Ver imágenes",
    "vault": "Para tu vault",
}

# El modelo de embeddings para «Preguntar a mi vault». Qwen3 Embedding entiende bien el español (nomic-embed-
# text es más chico, pero casi solo inglés).
EMBEDDINGS = "qwen3-embedding:0.6b"

# Con menos que esto no cabe entero ni el modelo más chico (como en «vortex-ia»): mejor usar el procesador.
VRAM_MINIMA_MB = 3380
# En el procesador manda la velocidad: los modelos que bajan más de esto ya se sienten lentos.
CPU_MAX_GB = 3.5


def _m(id, nombre, descripcion, para, gb, vram_gb, contexto, piensa=False, moe=False, vision=False):
    return {"id": id, "nombre": nombre, "descripcion": descripcion, "para": para, "gb": gb, "vram_gb": vram_gb,
            "contexto": contexto, "piensa": piensa, "moe": moe, "vision": vision}


CATALOGO: list[dict] = [
    # ---- Platicar
    _m("llama3.2:3b", "Llama 3.2 3B",
       "Chiquito y ligero: va en casi cualquier PC, hasta sin tarjeta de video. Bueno para dudas sencillas.",
       "chat", 2.0, 3.0, "128K"),
    _m("qwen3.5:2b", "Qwen 3.5 2B",
       "Muy rápido y entiende imágenes. Para PCs modestas o laptops.",
       "chat", 2.7, 3.5, "256K", piensa=True, vision=True),
    _m("qwen3.5:4b", "Qwen 3.5 4B",
       "Listo y ligero: habla bien español, ve imágenes y va bien hasta sin tarjeta de video.",
       "chat", 3.3, 4.5, "256K", piensa=True, vision=True),
    _m("gemma4:e2b", "Gemma 4 E2B",
       "El Gemma de Google en tamaño mini: ve imágenes y escucha audio, y va bien en laptops.",
       "chat", 4.6, 5.5, "128K", piensa=True, vision=True),
    _m("qwen3:8b", "Qwen 3 8B",
       "Un todoterreno muy probado: platica, explica y piensa antes de contestar si se lo pides.",
       "chat", 5.2, 6.5, "40K", piensa=True),
    _m("qwen3.5:9b", "Qwen 3.5 9B",
       "El recomendado para la mayoría: listo, rápido, habla muy bien español y ve imágenes.",
       "chat", 6.6, 8.0, "256K", piensa=True, vision=True),
    _m("gemma4:e4b", "Gemma 4 E4B",
       "El Gemma de Google mediano: buena plática, ve imágenes y escucha audio.",
       "chat", 6.6, 8.0, "128K", piensa=True, vision=True),
    _m("gemma4:12b", "Gemma 4 12B",
       "Más inteligente que los medianos; ideal si tienes una tarjeta de 10-12 GB.",
       "chat", 8.0, 10.0, "256K", piensa=True, vision=True),
    _m("qwen3.6:35b-a3b", "Qwen 3.6 35B-A3B",
       "Un modelo grande pero rápido (usa solo una parte a la vez). Para PCs con mucha memoria.",
       "chat", 23.0, 26.0, "256K", piensa=True, moe=True, vision=True),
    # ---- Pensar a fondo
    _m("deepseek-r1:8b", "DeepSeek R1 8B",
       "Piensa paso a paso antes de contestar: bueno para mate y problemas difíciles.",
       "razonamiento", 5.2, 6.5, "128K", piensa=True),
    _m("gpt-oss:20b", "GPT-OSS 20B",
       "El modelo abierto de OpenAI: razona muy bien y aguanta ir en parte en la CPU.",
       "razonamiento", 14.0, 16.0, "128K", piensa=True, moe=True),
    _m("qwen3.6:27b", "Qwen 3.6 27B",
       "De los más listos que puedes correr en casa. Necesita una tarjeta de video grande.",
       "razonamiento", 17.0, 20.0, "256K", piensa=True, vision=True),
    # ---- Programar
    _m("qwen2.5-coder:7b", "Qwen 2.5 Coder 7B",
       "Para programar: escribe, explica y arregla código. Ligero.",
       "codigo", 4.7, 6.0, "32K"),
    _m("qwen3-coder:30b", "Qwen 3 Coder 30B",
       "El mejor para programar en tu PC: entiende proyectos grandes. Rápido aunque no quepa completo.",
       "codigo", 19.0, 22.0, "256K", moe=True),
    # ---- Ver imágenes
    _m("qwen3-vl:4b", "Qwen 3 VL 4B",
       "Mira fotos y capturas de pantalla y te dice qué hay, o lee el texto que tienen.",
       "vision", 3.3, 4.5, "256K", piensa=True, vision=True),
    _m("qwen3-vl:8b", "Qwen 3 VL 8B",
       "Como el 4B pero más fino: entiende mejor gráficas, documentos y fotos complicadas.",
       "vision", 6.2, 8.0, "256K", piensa=True, vision=True),
    # ---- Para tu vault (embeddings: no platican, convierten tus notas en números para buscarlas)
    _m("qwen3-embedding:0.6b", "Qwen 3 Embedding 0.6B",
       "Le enseña a la IA a buscar en tus notas. Entiende español. Es el que usa «Preguntar a mi vault».",
       "vault", 0.64, 1.2, "32K"),
    _m("nomic-embed-text", "Nomic Embed Text",
       "Otro buscador de notas, más chiquito; funciona mejor con notas en inglés.",
       "vault", 0.27, 0.6, "2K"),
]


def por_id(id: str) -> dict | None:
    """El modelo del catálogo con ese id (acepta «x» como «x:latest»)."""
    if not id:
        return None
    for m in CATALOGO:
        if m["id"] == id or f"{m['id']}:latest" == id or m["id"] == f"{id}:latest":
            return m
    return None


def de_tipo(para: str) -> list[dict]:
    """Los modelos del catálogo de un tipo («chat», «codigo», …)."""
    return [m for m in CATALOGO if m["para"] == para]


# ------------------------------------------------------------------ tu memoria de video

def _ram() -> tuple[int, int]:
    """(total_mb, libre_mb) de la RAM."""
    total = libre = 0
    try:
        for linea in Path("/proc/meminfo").read_text().splitlines():
            clave, _, valor = linea.partition(":")
            if clave == "MemTotal":
                total = int(valor.split()[0]) // 1024
            elif clave == "MemAvailable":
                libre = int(valor.split()[0]) // 1024
    except (OSError, ValueError, IndexError):
        pass
    return total, libre


def _de_vortex_ia(ram_mb: int) -> dict | None:
    """Lo que diga «vortex-ia modelos --json», si existe (lo hace otra parte de Vortexine's)."""
    if not shutil.which("vortex-ia"):
        return None
    try:
        r = subprocess.run(["vortex-ia", "modelos", "--json"], capture_output=True, text=True, timeout=8)
        datos = json.loads(r.stdout) if r.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired, ValueError):
        return None
    if not isinstance(datos, dict):
        return None
    maquina = datos.get("maquina") if isinstance(datos.get("maquina"), dict) else {}
    for candidato in (datos, datos.get("vram"), datos.get("gpu"), datos.get("tarjeta"), maquina.get("gpu")):
        # vortex-ia da {"maquina": {"gpu": {"tipo", "nombre", "vram_gb", "libre_gb", "de_entrada"}}}
        if isinstance(candidato, dict) and not candidato.get("total_mb") and candidato.get("vram_gb"):
            try:
                candidato = dict(candidato, total_mb=int(float(candidato["vram_gb"]) * 1024),
                                 libre_mb=int(float(candidato.get("libre_gb") or 0) * 1024) or None)
            except (TypeError, ValueError):
                continue
        if isinstance(candidato, dict) and candidato.get("total_mb"):
            try:
                tipo = str(candidato.get("tipo") or "cpu").lower()
                return {"nombre": str(candidato.get("nombre") or "Tarjeta de video"),
                        "total_mb": int(candidato["total_mb"]),
                        "libre_mb": int(candidato.get("libre_mb") or candidato["total_mb"]),
                        "tipo": tipo if tipo in ("nvidia", "amd", "intel", "cpu") else "cpu",
                        "ram_mb": ram_mb, "de_entrada": bool(candidato.get("de_entrada"))}
            except (TypeError, ValueError):
                return None
    return None


def _de_nvidia(ram_mb: int) -> dict | None:
    if not shutil.which("nvidia-smi"):
        return None
    try:
        r = subprocess.run(["nvidia-smi", "--query-gpu=name,memory.total,memory.free", "--format=csv,noheader,nounits"],
                           capture_output=True, text=True, timeout=8)
    except (OSError, subprocess.TimeoutExpired):
        return None
    mejor = None
    for linea in r.stdout.splitlines() if r.returncode == 0 else []:
        partes = [p.strip() for p in linea.split(",")]
        if len(partes) < 3:
            continue
        try:
            total, libre = int(float(partes[1])), int(float(partes[2]))
        except ValueError:
            continue
        if mejor is None or total > mejor["total_mb"]:
            mejor = {"nombre": partes[0], "total_mb": total, "libre_mb": libre, "tipo": "nvidia", "ram_mb": ram_mb}
    return mejor


_VENDEDORES = {"0x10de": "nvidia", "0x1002": "amd", "0x8086": "intel"}


def _de_sysfs(ram_mb: int, ram_libre: int, drm: Path = Path("/sys/class/drm")) -> dict | None:
    """AMD (y las Intel Arc) dicen su VRAM en /sys/class/drm/cardN/device/mem_info_vram_*."""
    mejor = None
    integrada = None
    for tarjeta in sorted(drm.glob("card[0-9]*")):
        if "-" in tarjeta.name:      # card0-HDMI-A-1: son las salidas, no la tarjeta
            continue
        dispositivo = tarjeta / "device"
        try:
            tipo = _VENDEDORES.get((dispositivo / "vendor").read_text().strip(), "")
        except OSError:
            continue
        if not tipo:
            continue
        try:
            total = int((dispositivo / "mem_info_vram_total").read_text()) // (1024 * 1024)
            usada = int((dispositivo / "mem_info_vram_used").read_text()) // (1024 * 1024)
        except (OSError, ValueError):
            if tipo == "intel":
                integrada = tipo
            continue
        if total < 1024:   # una integrada con su pedacito reservado: usa la RAM compartida
            integrada = integrada or tipo
            continue
        nombre = {"amd": "Tarjeta AMD", "intel": "Tarjeta Intel", "nvidia": "Tarjeta NVIDIA"}[tipo]
        if mejor is None or total > mejor["total_mb"]:
            mejor = {"nombre": nombre, "total_mb": total, "libre_mb": max(total - usada, 0), "tipo": tipo,
                     "ram_mb": ram_mb}
    if mejor is None and integrada and ram_mb:
        # La integrada comparte la RAM: Ollama (Vulkan) puede usar más o menos la mitad.
        return {"nombre": f"Gráficos integrados {integrada.upper() if integrada == 'amd' else integrada.capitalize()}",
                "total_mb": ram_mb // 2, "libre_mb": ram_libre // 2, "tipo": integrada, "ram_mb": ram_mb}
    return mejor


def vram_disponible() -> dict:
    """La memoria con la que puede correr los modelos: {"nombre", "total_mb", "libre_mb", "tipo", "ram_mb"}.

    Primero «vortex-ia modelos --json» (si existe), luego nvidia-smi, luego /sys/class/drm (AMD, Intel Arc),
    y si no hay tarjeta o se queda corta para la IA, la RAM (tipo "cpu"). VORTEX_LOCAL_AI_VRAM_MB=<n> la fija
    (para pruebas).
    """
    ram_mb, ram_libre = _ram()
    fija = os.environ.get("VORTEX_LOCAL_AI_VRAM_MB")
    if fija:
        try:
            mb = int(fija)
            return {"nombre": "Tarjeta de prueba", "total_mb": mb, "libre_mb": mb, "tipo": "nvidia", "ram_mb": ram_mb}
        except ValueError:
            pass
    for buscar in (lambda: _de_vortex_ia(ram_mb), lambda: _de_nvidia(ram_mb), lambda: _de_sysfs(ram_mb, ram_libre)):
        try:
            resultado = buscar()
        except Exception:   # nunca tumbar la app por no poder leer la tarjeta
            resultado = None
        if resultado:
            if resultado.get("de_entrada") or resultado["total_mb"] < VRAM_MINIMA_MB:
                return {"nombre": "Procesador (tu tarjeta de video se queda corta)", "total_mb": ram_mb,
                        "libre_mb": ram_libre, "tipo": "cpu", "ram_mb": ram_mb, "de_entrada": True}
            return resultado
    return {"nombre": "Procesador (sin tarjeta de video)", "total_mb": ram_mb, "libre_mb": ram_libre, "tipo": "cpu",
            "ram_mb": ram_mb}


# ------------------------------------------------------------------ ¿te cabe?

def _necesita_gb(modelo) -> tuple[float, bool]:
    """(GB que necesita, es_moe) de un dict del catálogo, un id o un dict de Ollama.modelos()."""
    if isinstance(modelo, str):
        encontrado = por_id(modelo)
        if encontrado is None:
            return 0.0, False
        modelo = encontrado
    if not isinstance(modelo, dict):
        return 0.0, False
    if modelo.get("vram_gb"):
        return float(modelo["vram_gb"]), bool(modelo.get("moe"))
    del_catalogo = por_id(str(modelo.get("id") or modelo.get("nombre") or ""))
    if del_catalogo is not None:
        return float(del_catalogo["vram_gb"]), bool(del_catalogo.get("moe"))
    if modelo.get("bytes"):
        return int(modelo["bytes"]) / 1e9 * 1.15 + 0.8, False
    if modelo.get("gb"):
        return float(modelo["gb"]) * 1.15 + 0.8, bool(modelo.get("moe"))
    return 0.0, False


def cabe(modelo, vram: dict) -> str:
    """"si" (va completo y holgado), "justo" (con poco margen, o una parte en la CPU y más lento) o "no"."""
    necesita, moe = _necesita_gb(modelo)
    total = (vram.get("total_mb") or 0) / 1024
    ram = (vram.get("ram_mb") or 0) / 1024
    if necesita <= 0 or total <= 0:
        return "no"
    if vram.get("tipo") == "cpu":
        # Sin tarjeta todo va en la RAM: deja espacio para el sistema y tus apps.
        if necesita <= total * 0.5:
            return "si"
        return "justo" if necesita <= total * 0.75 else "no"
    if necesita <= total - 0.3:
        return "si"
    # Lo que no cabe en la tarjeta se va a la RAM. Un moe lo aguanta bien; uno normal se pone lento rápido.
    extra = min(ram * 0.5, total * (1.5 if moe else 0.3))
    return "justo" if necesita <= total + extra else "no"


def recomendado(vram: dict) -> dict:
    """El mejor modelo para platicar que te cabe completo; si ninguno, el más chico. Sin gráfica, uno ligero."""
    platica = de_tipo("chat")
    caben = [m for m in platica if cabe(m, vram) == "si"]
    if vram.get("tipo") == "cpu":
        caben = [m for m in caben if m["gb"] <= CPU_MAX_GB]
    if caben:
        return max(caben, key=lambda m: m["vram_gb"])
    return min(platica, key=lambda m: m["vram_gb"])
