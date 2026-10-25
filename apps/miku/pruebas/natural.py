#!/usr/bin/env python3
"""Pruebas de la voz natural: el respaldo a la F1, el modo lento en la CPU y cómo elige el modo la instalación.

1. Sin GPU ni PyTorch: VORTEX_MIKU_NATURAL apunta a una carpeta con un «python» falso que (a) avisa un error
   al cargar, (b) se muere sin decir nada o (c) arranca en modo CPU. En (a) y (b) Miku sigue con la F1 sin
   perder la frase; en (c) habla con la natural y avisa una sola vez que va en modo lento.
2. «vortex-miku natural instalar» con la GPU, el procesador, pip y la descarga de mentira: con una GPU que
   sirva instala lo de siempre; sin ella, con un procesador de 10.ª generación o más nuevo (o con --cpu), el
   modo CPU (PyTorch de CPU, la rueda de CPU de qwentts.cpp y el GGUF Q4_K_M); sin ninguno no instala; al
   cambiar de modo rehace el entorno; «natural estado» dice el modo. Qué procesadores cuentan. El modo
   --auto (el que lanza la ventana de Miku) y que la ventana lo lance solo cuando toca.
3. Con una instalación CPU de verdad (VORTEX_MIKU_NATURAL_CPU, o si existe
   ~/.cache/vortex-miku-cpu-prueba/natural del usuario real): el proceso carga en la CPU y habla. Si no hay,
   se salta.

Hace falta el modelo de Supertonic (la F1) en el HOME de prueba (como en las demás pruebas, con env.sh) y
PySide6. Sin ventanas (QT_QPA_PLATFORM=offscreen).

  python3 -I apps/miku/pruebas/natural.py
"""

import contextlib
import importlib
import importlib.machinery
import importlib.util
import io
import os
import pwd
import shutil
import sys
import tempfile
import time
import types
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ))
FALLOS = 0

# Un «python» de la voz natural que arranca en modo CPU y contesta cada frase con medio segundo de silencio.
PYTHON_LENTO = '''#!/usr/bin/python3
import json, os, sys, wave
print(json.dumps({"listo": True, "voz": "miku-natural", "modo": "cpu"}), flush=True)
for n, linea in enumerate(sys.stdin):
    pedido = json.loads(linea)
    if pedido.get("cmd") == "salir":
        break
    ruta = os.path.join(CARPETA, f"falsa-{os.getpid()}-{n}.wav")
    with wave.open(ruta, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(24000); w.writeframes(bytes(24000))
    print(json.dumps({"id": pedido.get("id"), "wav": ruta, "seg": 0.5, "visemas": []}), flush=True)
'''


def probar(nombre: str, ok: bool) -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}")
    FALLOS += 0 if ok else 1


def recargar(carpeta: Path):
    """Apunta la voz natural a otra carpeta (VORTEX_MIKU_NATURAL) y recarga lo que la lee al importarse."""
    os.environ["VORTEX_MIKU_NATURAL"] = str(carpeta)
    from miku import rutas, voces
    importlib.reload(rutas)
    importlib.reload(voces)
    from miku import asistente
    importlib.reload(asistente)
    return rutas, voces, asistente


def armar(caso: str) -> Path:
    d = Path(tempfile.mkdtemp(prefix=f"vortex-miku-natural-{caso}."))
    (d / "venv/bin").mkdir(parents=True)
    py = d / "venv/bin/python"
    if caso == "error":
        py.write_text('#!/bin/sh\necho \'{"error": "RuntimeError: no hay una GPU NVIDIA disponible"}\'\nexit 1\n')
    elif caso == "lenta":
        from miku import rutas
        py.write_text(PYTHON_LENTO.replace("CARPETA", repr(str(rutas.temporales())), 1))
        (d / "modo").write_text("cpu\n")
    else:
        py.write_text("#!/bin/sh\nexit 3\n")
    py.chmod(0o755)
    (d / "listo").write_text("prueba\n")
    return d


def hablar(app, asistente, voces, carpeta: Path, espera: float):
    """Abre el asistente con la voz natural, le pide una frase y espera a que «suene» (o a que pase «espera»)."""
    from PySide6.QtCore import QTimer
    cfg = {"avatar": False, "voz": "miku-natural", "modelo": "", "seguir_escuchando": False, "silencio": 1.5}
    salida = carpeta / "sono"                   # VORTEX_MIKU_SALIDA: lo que «suena» se copia aquí
    os.environ["VORTEX_MIKU_SALIDA"] = str(salida)
    a = asistente.Asistente(cfg, avatar=False)
    avisos = []
    a.aviso.connect(avisos.append)
    t0 = time.monotonic()
    marcas = {}
    a._arrancar_voz()
    a._decir("Hola, ¿cómo estás?")
    limite = QTimer()                           # no singleShot: uno pendiente de un caso anterior cortaría este
    limite.setSingleShot(True)
    limite.timeout.connect(app.quit)
    limite.start(int(espera * 1000))
    reloj = QTimer()

    def mirar():
        if a._voz_lista and "lista" not in marcas:
            marcas["lista"] = time.monotonic() - t0
        if salida.is_dir() and any(salida.glob("*.wav")):
            marcas["sono"] = time.monotonic() - t0
            app.quit()
    reloj.timeout.connect(mirar)
    reloj.start(100)
    app.exec()
    reloj.stop()
    limite.stop()
    return a, avisos, salida, marcas


def respaldo(app) -> None:
    for caso in ("error", "muere", "lenta"):
        carpeta = armar(caso)
        rutas, voces, asistente = recargar(carpeta)
        print({"error": "== La voz natural avisa un error al cargar", "muere": "== La voz natural se muere al cargar",
               "lenta": "== La voz natural arranca en modo CPU (lenta)"}[caso])
        probar("la voz natural cuenta como instalada", voces.disponible(voces.buscar("miku-natural")))
        a, avisos, salida, _ = hablar(app, asistente, voces, carpeta, 15)
        if caso == "lenta":
            probar("sigue con la voz natural", getattr(a, "_voz_id", "") == "miku-natural" and a._voz_lista)
            probar("avisa una vez que va en modo lento", sum("modo lento" in t for t in avisos) == 1)
            probar("no cae a la voz de siempre", not any("voz de siempre" in t for t in avisos))
        else:
            probar("cambia a la F1", getattr(a, "_voz_id", "") == voces.VOZ_DEFECTO and a._voz_lista)
            probar("avisa una vez que usa la voz de siempre", sum("voz de siempre" in t for t in avisos) == 1)
            probar("no dice que va en modo lento", not any("modo lento" in t for t in avisos))
        probar("la frase sale", salida.is_dir() and len(list(salida.glob("*.wav"))) == 1)
        a.cerrar_todo()
        shutil.rmtree(carpeta, ignore_errors=True)


# ---- Instalación: cómo elige el modo ----------------------------------------------------------------

def cargar_comando():
    """El comando vortex-miku (un script sin .py) como módulo, para cambiarle la GPU, pip y la descarga."""
    cargador = importlib.machinery.SourceFileLoader("vortex_miku_prueba", str(RAIZ / "vortex-miku"))
    m = importlib.util.module_from_spec(importlib.util.spec_from_loader(cargador.name, cargador))
    cargador.exec_module(m)
    return m


class SubprocesoFalso:
    """Anota lo que se correría. Crear el venv deja un python; la prueba de la voz deja su WAV."""

    def __init__(self):
        self.ordenes: list[list[str]] = []

    def run(self, cmd, **_kw):
        cmd = [str(c) for c in cmd]
        self.ordenes.append(cmd)
        if cmd[1:3] == ["-m", "venv"]:
            py = Path(cmd[3]) / "bin" / "python"
            py.parent.mkdir(parents=True, exist_ok=True)
            py.write_text("#!/bin/sh\n")
        if "--salida" in cmd:
            Path(cmd[cmd.index("--salida") + 1]).write_bytes(b"RIFF")
        return types.SimpleNamespace(returncode=0)

    def pip(self) -> list[str]:
        return [" ".join(c) for c in self.ordenes if "pip" in c]


def instalar(carpeta: Path, args: list[str], gpu_sirve: bool, cpu_sirve: bool = True, vieja: str = "",
             orden: str = "instalar"):
    """Corre «vortex-miku natural instalar ARGS» de mentira. Devuelve (código, texto, subproceso, módulo)."""
    rutas, voces, _ = recargar(carpeta)
    m = cargar_comando()
    falso = SubprocesoFalso()
    m.subprocess = falso
    m.gpu_para_natural = lambda: ((True, "NVIDIA GeForce RTX 3080 (10 GB)") if gpu_sirve else
                                  (False, "tu NVIDIA GeForce GTX 1060 6GB es muy vieja para la voz natural "
                                          "(hace falta una serie 20 o más nueva)."))
    m.cpu_para_natural = lambda: ((True, "Intel(R) Core(TM) i9-11900K (Intel Core de 11.ª generación)") if cpu_sirve else
                                  (False, "tu Intel(R) Core(TM) i7-4790K no llega: en la CPU hace falta un Intel Core "
                                          "de 10.ª generación o más nuevo, o un Ryzen 3000 o más nuevo."))
    # vieja: la cuantización si la NVIDIA sirve en modo cuda («» = no hay); la descarga de qwentts.cpp, de mentira.
    m.gpu_vieja_para_natural = lambda: ((True, "NVIDIA GeForce GTX 1060 6GB (6 GB)", vieja) if vieja else
                                        (False, "no hay una GPU NVIDIA con su driver.", ""))
    m.libs_bajadas = []

    def bajar_libs():
        m.libs_bajadas.append(str(m.rutas.NATURAL))
        (m.rutas.NATURAL / "cuda").mkdir(parents=True, exist_ok=True)
        (m.rutas.NATURAL / "cuda" / "libqwen.so").write_text("")
        return 0
    m.bajar_libs_cuda = bajar_libs
    m.reabrir_si_esta_abierta = lambda: False
    m.reabrio = False

    def reabrir():
        m.reabrio = True
        return True
    m.reabrir_si_esta_abierta = reabrir
    texto = io.StringIO()
    with contextlib.redirect_stdout(texto), contextlib.redirect_stderr(texto):
        rc = m.orden_natural([orden, *args])
    return rc, texto.getvalue(), falso, m


def estado(m) -> str:
    texto = io.StringIO()
    with contextlib.redirect_stdout(texto):
        m.orden_natural(["estado"])
    return texto.getvalue()


def eleccion() -> None:
    from miku import rutas
    copia = rutas.CONFIG.read_bytes() if rutas.CONFIG.is_file() else None     # instalar cambia la voz elegida
    base = Path(tempfile.mkdtemp(prefix="vortex-miku-natural-modo."))
    try:
        print("== Instalar con una GPU que sirva: lo de siempre (GPU)")
        rc, texto, falso, m = instalar(base / "gpu", [], gpu_sirve=True)
        pip = " | ".join(falso.pip())
        probar("termina bien", rc == 0)
        probar("guarda el modo gpu", (base / "gpu/modo").read_text().strip() == "gpu" and (base / "gpu/listo").is_file())
        probar("instala faster-qwen3-tts con su PyTorch de CUDA (sin el de CPU ni qwentts.cpp)",
               "faster-qwen3-tts==0.5.4" in pip and "download.pytorch.org" not in pip and "qwentts" not in pip)
        probar("baja el modelo 1.7B", any("Qwen/Qwen3-TTS-12Hz-1.7B-Base" in c for c in falso.ordenes))
        probar("«natural estado» dice GPU", "Modo: GPU" in estado(m))

        print("== Instalar sin una GPU que sirva (GTX 1060): modo CPU, avisando que es lento")
        rc, texto, falso, m = instalar(base / "cpu", [], gpu_sirve=False)
        pip = falso.pip()
        probar("termina bien", rc == 0)
        probar("explica por qué y que será lenta", "GTX 1060" in texto and "lenta" in texto)
        probar("guarda el modo cpu", (base / "cpu/modo").read_text().strip() == "cpu" and (base / "cpu/listo").is_file())
        probar("primero el PyTorch de CPU, de su índice",
               len(pip) == 2 and "--index-url https://download.pytorch.org/whl/cpu" in pip[0] and "torch==" in pip[0])
        probar("luego faster-qwen3-tts y la rueda de CPU de qwentts.cpp, fijada con sha256 (nada de CUDA)",
               len(pip) == 2 and "faster-qwen3-tts==0.5.4" in pip[1] and "qwentts_cpp_python-0.5.0%2Bcpu" in pip[1]
               and "#sha256=" in pip[1] and "nvidia" not in pip[1] and "download.pytorch.org" not in pip[1])
        probar("baja el GGUF del 0.6B en Q4_K_M", any("resolve_gguf_paths" in " ".join(c) and "Qwen/Qwen3-TTS-12Hz-0.6B-Base"
                                                     in c and "Q4_K_M" in c for c in falso.ordenes))
        e = estado(m)
        probar("«natural estado» dice CPU (lento) y que la GPU no sirve", "Modo: CPU (lento" in e and "no sirve" in e)

        print("== Instalar con --cpu aunque la GPU sirva")
        rc, texto, falso, m = instalar(base / "forzada", ["--cpu"], gpu_sirve=True)
        probar("usa el modo CPU", rc == 0 and (base / "forzada/modo").read_text().strip() == "cpu"
               and "Pediste el modo CPU" in texto)

        print("== Cambiar de modo (GPU → CPU) rehace el entorno")
        (base / "gpu/venv/viejo").write_text("PyTorch de CUDA\n")
        (base / "gpu/hf").mkdir(exist_ok=True)
        rc, texto, falso, m = instalar(base / "gpu", ["--cpu"], gpu_sirve=True)
        probar("borra el entorno y el modelo de la GPU y crea uno nuevo", rc == 0 and not (base / "gpu/venv/viejo").exists()
               and not (base / "gpu/hf").exists() and any(c[1:3] == ["-m", "venv"] for c in falso.ordenes))
        probar("queda en modo cpu", (base / "gpu/modo").read_text().strip() == "cpu")
        print("== Una instalación de antes (sin archivo «modo») cuenta como GPU")
        (base / "gpu/modo").unlink()
        probar("estado: Modo GPU", "Modo: GPU" in estado(m))

        print("== Opciones que no existen")
        rc, texto, falso, m = instalar(base / "mala", ["--rapido"], gpu_sirve=True)
        probar("se rechazan (código 2) sin instalar nada", rc == 2 and not falso.ordenes)

        print("== Sin GPU que sirva y con un procesador viejo (i7-4790K): no instala")
        rc, texto, falso, m = instalar(base / "vieja", [], gpu_sirve=False, cpu_sirve=False)
        probar("sale con error, explica por qué y no baja nada",
               rc == 1 and "4790K" in texto and "voz de siempre" in texto and "--cpu" in texto and not falso.ordenes)
        e = estado(m)
        probar("«natural estado» dice que el procesador no sirve y que se queda con la de siempre",
               "Procesador: no sirve" in e and "con la de siempre" in e)
        rc, texto, falso, m = instalar(base / "vieja", ["--cpu"], gpu_sirve=False, cpu_sirve=False)
        probar("con --cpu igual la instala en la CPU", rc == 0 and (base / "vieja/modo").read_text().strip() == "cpu")

        print("== --auto (lo lanza la ventana de Miku)")
        from miku import config, voces as v
        config.guardar({"voz": v.VOZ_DEFECTO, "voz_natural_auto": True})
        rc, texto, falso, m = instalar(base / "auto", ["--auto"], gpu_sirve=False, cpu_sirve=True)
        avisos = [c for c in falso.ordenes if c[0] == "notify-send"]
        probar("instala en la CPU y deja la voz natural elegida",
               rc == 0 and (base / "auto/listo").is_file() and config.cargar()["voz"] == "miku-natural")
        probar("avisa al empezar y al terminar con notificaciones", len(avisos) == 2
               and "preparando" in " ".join(avisos[0]) and "lenta" in " ".join(avisos[0])
               and "ya tiene su voz natural" in " ".join(avisos[1]))
        probar("no reabre a Miku", not m.reabrio)
        rc, texto, falso, m = instalar(base / "auto", ["--auto"], gpu_sirve=True)
        probar("ya instalada: no hace nada", rc == 0 and not falso.ordenes)
        config.guardar({"voz": "supertonic-f3"})
        rc, texto, falso, m = instalar(base / "auto2", ["--auto"], gpu_sirve=True)
        probar("si elegiste otra voz, no la instala", rc == 0 and not falso.ordenes)
        config.guardar({"voz": v.VOZ_DEFECTO})
        rc, texto, falso, m = instalar(base / "auto3", ["--auto"], gpu_sirve=False, cpu_sirve=False)
        probar("sin PC que aguante: sale bien, sin bajar nada", rc == 0 and not falso.ordenes and "de siempre" in texto)
        texto = io.StringIO()
        with contextlib.redirect_stdout(texto):
            m.orden_natural(["quitar"])
        rc, texto, falso, m = instalar(base / "auto4", ["--auto"], gpu_sirve=True)
        probar("después de «natural quitar» no la vuelve a instalar sola",
               config.cargar()["voz_natural_auto"] is False and rc == 0 and not falso.ordenes)

        print("== Instalar con una GTX 1060 (sin GPU para PyTorch): modo CUDA, la misma voz 1.7B")
        rc, texto, falso, m = instalar(base / "cuda", [], gpu_sirve=False, vieja="Q4_K_M")
        pip = falso.pip()
        probar("termina bien", rc == 0)
        probar("guarda el modo cuda y la cuantización", (base / "cuda/modo").read_text().strip() == "cuda"
               and (base / "cuda/cuantizacion").read_text().strip() == "Q4_K_M" and (base / "cuda/listo").is_file())
        probar("PyTorch de CPU y luego qwentts.cpp con el CUDA 12 de pip (sin el PyTorch de CUDA)",
               len(pip) == 2 and "download.pytorch.org/whl/cpu" in pip[0] and "nvidia-cublas-cu12==" in pip[1]
               and "nvidia-cuda-runtime-cu12==" in pip[1] and "qwentts_cpp_python-0.5.0%2Bcpu" in pip[1])
        probar("baja la compilación de qwentts.cpp para CUDA", m.libs_bajadas == [str(base / "cuda")])
        probar("baja el GGUF del 1.7B (la voz de las GPU) en Q4_K_M",
               any("resolve_gguf_paths" in " ".join(c) and "Qwen/Qwen3-TTS-12Hz-1.7B-Base" in c and "Q4_K_M" in c
                   for c in falso.ordenes))
        probar("no avisa que será lenta", "lenta" not in texto)
        probar("«natural estado» dice GPU con qwentts.cpp", "Modo: GPU con qwentts.cpp (Q4_K_M)" in estado(m))
        rc, texto, falso, m = instalar(base / "cuda8", [], gpu_sirve=False, vieja="Q8_0")
        probar("con 8 GB o más, Q8_0", rc == 0 and (base / "cuda8/cuantizacion").read_text().strip() == "Q8_0"
               and any("Q8_0" in c for c in falso.ordenes))
        print("== Cambiar de CUDA a CPU borra también las bibliotecas de CUDA")
        rc, texto, falso, m = instalar(base / "cuda8", ["--cpu"], gpu_sirve=False, vieja="Q8_0")
        probar("CUDA → CPU quita la carpeta cuda", rc == 0 and not (base / "cuda8/cuda").exists())

        print("== «natural mejorar»: una voz que quedó en la CPU pasa a la GPU si ahora sirve")
        config.guardar({"voz": "miku-natural", "voz_natural_auto": True})
        real = Path(tempfile.mkdtemp(prefix="vortex-miku-mejorar."))
        try:
            instalar(real / "natural", ["--cpu"], gpu_sirve=False, vieja="")
            (real / "natural/venv/marca-cpu").write_text("")
            rc, texto, falso, m = instalar(real / "natural", [], gpu_sirve=False, vieja="Q4_K_M", orden="mejorar")
            probar("la rehace en modo cuda", rc == 0 and (real / "natural/modo").read_text().strip() == "cuda"
                   and (real / "natural/listo").is_file())
            probar("la arma en otra carpeta y la cambia al final (sin carpetas sobrantes)",
                   m.libs_bajadas == [str(real / "natural-nueva")] and not (real / "natural/venv/marca-cpu").exists()
                   and not (real / "natural-nueva").exists() and not (real / "natural-vieja").exists())
            probar("deja la voz natural elegida y avisa con una notificación", config.cargar()["voz"] == "miku-natural"
                   and any("mejoró" in " ".join(c) for c in falso.ordenes if c[0] == "notify-send"))
            probar("rutas vuelve a apuntar a la carpeta de siempre", str(m.rutas.NATURAL) == str(real / "natural"))
            rc, texto, falso, m = instalar(real / "natural", [], gpu_sirve=False, vieja="Q4_K_M", orden="mejorar")
            probar("ya en modo cuda: no hace nada", rc == 0 and not falso.ordenes and "No hace falta" in texto)
            instalar(real / "otra", ["--cpu"], gpu_sirve=False, vieja="")
            rc, texto, falso, m = instalar(real / "otra", [], gpu_sirve=False, vieja="", orden="mejorar")
            probar("en la CPU y sin GPU que sirva: no hace nada",
                   rc == 0 and not falso.ordenes and (real / "otra/modo").read_text().strip() == "cpu")
        finally:
            shutil.rmtree(real, ignore_errors=True)

        print("== Qué procesadores aguantan la voz natural en la CPU")
        for nombre, sirve in (("Intel(R) Core(TM) i7-10700F CPU @ 2.90GHz", True),
                              ("11th Gen Intel(R) Core(TM) i9-11900K @ 3.50GHz", True),
                              ("11th Gen Intel(R) Core(TM) i5-1135G7 @ 2.40GHz", True),
                              ("Intel(R) Core(TM) i7-1065G7 CPU @ 1.30GHz", True),
                              ("13th Gen Intel(R) Core(TM) i9-13900HX", True),
                              ("Intel(R) Core(TM) Ultra 7 155H", True),
                              ("AMD Ryzen 5 5600H with Radeon Graphics", True),
                              ("AMD Ryzen 7 3700X 8-Core Processor", True),
                              ("AMD Ryzen 5 PRO 4650G with Radeon Graphics", True),
                              ("AMD Ryzen AI 9 HX 370 w/ Radeon 890M", True),
                              ("Intel(R) Core(TM) i7-8700K CPU @ 3.70GHz", False),
                              ("Intel(R) Core(TM) i7-8565U CPU @ 1.80GHz", False),
                              ("AMD Ryzen 5 3500U with Radeon Vega Mobile Gfx", False),
                              ("AMD Ryzen 5 2600 Six-Core Processor", False),
                              ("Intel(R) Celeron(R) N4020 CPU @ 1.10GHz", False),
                              ("Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz", False)):
            g = m.generacion_cpu(nombre)
            probar(f"{'sí' if sirve else 'no'}: {nombre}", (g is not None and g[0]) == sirve)
    finally:
        shutil.rmtree(base, ignore_errors=True)
        if copia is None:
            rutas.CONFIG.unlink(missing_ok=True)
        else:
            rutas.CONFIG.write_bytes(copia)


# ---- Con una instalación CPU de verdad -----------------------------------------------------------------

def instalacion_cpu() -> Path | None:
    d = os.environ.get("VORTEX_MIKU_NATURAL_CPU")
    carpeta = Path(d) if d else Path(pwd.getpwuid(os.getuid()).pw_dir) / ".cache/vortex-miku-cpu-prueba/natural"
    try:
        listo = (carpeta / "listo").is_file() and (carpeta / "modo").read_text().strip() == "cpu"
    except OSError:
        listo = False
    return carpeta if listo else None


def cpu_de_verdad(app) -> None:
    print("== La voz natural de verdad en la CPU")
    carpeta = instalacion_cpu()
    if carpeta is None:
        print("  --  no hay una instalación CPU de prueba (VORTEX_MIKU_NATURAL_CPU): se salta")
        return
    rutas, voces, asistente = recargar(carpeta)
    salida = Path(tempfile.mkdtemp(prefix="vortex-miku-natural-cpu."))
    a, avisos, sono, marcas = hablar(app, asistente, voces, salida, 180)
    probar("carga en la CPU y queda lista (no cae a la F1)", getattr(a, "_voz_id", "") == "miku-natural" and a._voz_lista)
    probar("avisa una vez que va en modo lento", sum("modo lento" in t for t in avisos) == 1)
    wavs = list(sono.glob("*.wav")) if sono.is_dir() else []
    probar("la frase suena", len(wavs) == 1 and wavs[0].stat().st_size > 24000)
    if "lista" in marcas and "sono" in marcas:
        print(f"      lista a los {marcas['lista']:.1f} s; la primera frase sonó a los {marcas['sono']:.1f} s")
    else:
        print(f"      tiempos: {marcas}; avisos: {avisos}")
    a.cerrar_todo()
    shutil.rmtree(salida, ignore_errors=True)


def auto_desde_ventana() -> None:
    """La ventana lanza «natural instalar --auto» aparte solo si toca (con VORTEX_MIKU_NATURAL_AUTO y un CLI falso)."""
    print("== La ventana de Miku lanza la voz natural sola")
    from miku import config
    base = Path(tempfile.mkdtemp(prefix="vortex-miku-natural-auto."))
    copia = None
    try:
        rutas, voces, asistente = recargar(base / "natural")
        copia = rutas.CONFIG.read_bytes() if rutas.CONFIG.is_file() else None
        anotado = base / "llamado"
        cli = base / "vortex-miku"
        cli.write_text(f'#!/bin/sh\necho "$@" >> {anotado}\n')
        cli.chmod(0o755)
        os.environ.update(VORTEX_MIKU_NATURAL_AUTO="1", VORTEX_MIKU_CLI=str(cli))
        a = asistente.Asistente({"avatar": False, "voz": voces.VOZ_DEFECTO, "modelo": "", "seguir_escuchando": False,
                                 "silencio": 1.5}, avatar=False)

        def llamado() -> str:
            for _ in range(30):
                if anotado.is_file():
                    break
                time.sleep(0.1)
            texto = anotado.read_text() if anotado.is_file() else ""
            anotado.unlink(missing_ok=True)
            return texto

        config.guardar({"voz": voces.VOZ_DEFECTO, "voz_natural_auto": True})
        a._natural_en_segundo_plano()
        probar("con la voz de siempre y sin la natural: lanza «natural instalar --auto»",
               llamado().strip() == "natural instalar --auto")
        config.guardar({"voz_natural_auto": False})
        a._natural_en_segundo_plano()
        probar("con la natural quitada a propósito: no", llamado() == "")
        config.guardar({"voz_natural_auto": True, "voz": "supertonic-f2"})
        a._natural_en_segundo_plano()
        probar("con otra voz elegida: no", llamado() == "")
        # Ya instalada: en la CPU pide «natural mejorar» (por si ahora la GPU sirve); en la GPU, nada.
        (base / "natural/venv/bin").mkdir(parents=True)
        (base / "natural/venv/bin/python").write_text("")
        (base / "natural/listo").write_text("")
        (base / "natural/modo").write_text("cpu\n")
        config.guardar({"voz": "miku-natural"})
        a._natural_en_segundo_plano()
        probar("instalada en la CPU: lanza «natural mejorar»", llamado().strip() == "natural mejorar")
        (base / "natural/modo").write_text("cuda\n")
        a._natural_en_segundo_plano()
        probar("instalada en modo cuda: no", llamado() == "")
        (base / "natural/listo").unlink()
        config.guardar({"voz": voces.VOZ_DEFECTO})
        os.environ["VORTEX_MIKU_NATURAL_AUTO"] = "0"
        a._natural_en_segundo_plano()
        probar("con VORTEX_MIKU_NATURAL_AUTO=0: no", llamado() == "")
        del os.environ["VORTEX_MIKU_NATURAL_AUTO"]
        a._natural_en_segundo_plano()
        probar("desde el repo (no el paquete): no", llamado() == "")
        a.cerrar_todo()
    finally:
        os.environ.pop("VORTEX_MIKU_NATURAL_AUTO", None)
        os.environ.pop("VORTEX_MIKU_CLI", None)
        from miku import rutas
        if copia is None:
            rutas.CONFIG.unlink(missing_ok=True)
        else:
            rutas.CONFIG.write_bytes(copia)
        shutil.rmtree(base, ignore_errors=True)


def main() -> int:
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from PySide6.QtCore import QCoreApplication
    app = QCoreApplication(sys.argv)
    respaldo(app)
    eleccion()
    auto_desde_ventana()
    cpu_de_verdad(app)
    print("\nTodo bien" if not FALLOS else f"\nFALLARON {FALLOS} pruebas")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
