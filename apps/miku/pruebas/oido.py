#!/usr/bin/env python3
"""Prueba del oído SIN micrófono: la «voz» sale de un WAV que genera la propia voz de Miku.

    python3 -I pruebas/oido.py CARPETA_TRABAJO

Necesita whisper-cli (VORTEX_MIKU_WHISPER si no está instalado) y los modelos de ~/.local/share/vortex/miku/{voz,oido},
ffmpeg, y la voz (VORTEX_MIKU_LIBS). Comprueba que:
  - transcribe en español lo que se dijo (con el modelo bajado, sin internet);
  - corta solo ~1,5 s después de que la voz termina;
  - si nadie habla, no devuelve nada (a los segundos de espera);
  - un clic en el micrófono (cortar_ahora) termina la escucha al instante;
  - no deja ningún archivo con audio (se miran la carpeta de trabajo, /tmp, /dev/shm y el runtime).
"""

import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import numpy as np  # noqa: E402

from miku import audio, oido, rutas  # noqa: E402

RAIZ = Path(__file__).resolve().parent.parent


def sintetizar(texto: str, destino: Path) -> None:
    r = subprocess.run([sys.executable, "-I", str(RAIZ / "miku/voz_worker.py"), "--estilo", "normal", "--salida", str(destino), texto])
    if r.returncode:
        raise SystemExit("no pude generar la voz de prueba")


def armar_entrada(voz_wav: Path, destino: Path, antes: float = 0.8, despues: float = 4.0, ruido_db: float = -62) -> float:
    """Ruido de fondo suave + la voz + silencio. Devuelve cuándo termina la voz (s)."""
    x, sr = audio.leer_wav(str(voz_wav))
    rng = np.random.default_rng(1)
    a = (rng.standard_normal(int(antes * sr)) * 10 ** (ruido_db / 20)).astype(np.float32)
    d = (rng.standard_normal(int(despues * sr)) * 10 ** (ruido_db / 20)).astype(np.float32)
    total = np.concatenate([a, x * 0.5, d])
    destino.write_bytes(audio.a_wav(total, sr))
    # La voz sintética termina con silencio digital (ceros): el fin de la voz es donde acaba el sonido de verdad.
    voz_real = np.nonzero(np.abs(x) > 0.003)[0]
    return antes + (int(voz_real[-1]) + 1 if len(voz_real) else len(x)) / sr


def parece_audio(p: Path) -> bool:
    if p.suffix.lower() in (".wav", ".pcm", ".raw", ".ogg", ".opus", ".flac", ".mp3", ".m4a"):
        return True
    try:
        with open(p, "rb") as f:
            return f.read(4) in (b"RIFF", b"OggS", b"fLaC")
    except OSError:
        return False


def listados() -> set[str]:
    """Archivos de audio en los sitios donde algo podría dejar una grabación."""
    encontrados = set()
    # En /tmp y /dev/shm solo cuentan los archivos de audio (otros programas escriben ahí); en la
    # carpeta de Miku (runtime) y en la carpeta desde donde se corre, cualquier archivo nuevo es sospechoso
    # (por ejemplo un «-.txt» con tu voz transcrita).
    for d, todo in ((Path("/tmp"), False), (Path("/dev/shm"), False), (rutas.runtime(), True), (Path.cwd(), True)):
        if d.is_dir():
            for p in (d.rglob("*") if d != Path.cwd() else d.glob("*")):
                try:
                    if p.is_file() and (todo or parece_audio(p)) and "oido" not in p.parts:
                        encontrados.add(str(p))
                except OSError:
                    pass
    return encontrados


def main() -> int:
    trabajo = Path(sys.argv[1])
    trabajo.mkdir(parents=True, exist_ok=True)
    fallos = []

    def comprobar(nombre, cond, extra=""):
        print(("  ok  " if cond else "  MAL ") + nombre + (f"  {extra}" if extra else ""), flush=True)
        if not cond:
            fallos.append(nombre)

    frase = "Hola Miku, ¿puedes decirme qué hora es en Tokio?"
    voz = trabajo / "voz.wav"
    entrada = trabajo / "entrada.wav"
    sintetizar(frase, voz)
    fin_voz = armar_entrada(voz, entrada)
    antes = listados()

    print("== Escuchar una frase")
    os.environ["VORTEX_MIKU_ENTRADA"] = str(entrada)
    o = oido.Oido(silencio=1.5, espera=8.0)
    niveles, hablo = [], []
    t0 = time.monotonic()
    pcm = o.escuchar(al_nivel=niveles.append, al_hablar=lambda: hablo.append(time.monotonic() - t0))
    dur = time.monotonic() - t0
    comprobar("devolvió audio", pcm is not None and len(pcm) > 16000)
    comprobar("avisó el volumen del micrófono (para el indicador)", len(niveles) > 20 and max(niveles) > 0.4, f"{len(niveles)} avisos, máx {max(niveles):.2f}")
    # La fuente corre en tiempo real (-re): el corte debe llegar ~1,5 s después de terminar la voz.
    # (el audio de la voz termina con algo de silencio, así que el corte llega un poco antes de fin_voz + 1,5)
    comprobar("cortó solo, ~1,5 s después de que terminó la voz", fin_voz + 0.8 < dur < fin_voz + 2.2, f"voz hasta {fin_voz:.1f}s, cortó a {dur:.1f}s")
    t1 = time.monotonic()
    texto = oido.transcribir(pcm)
    print(f"      transcripción en {time.monotonic() - t1:.1f}s: «{texto}»")
    palabras = texto.lower()
    comprobar("transcribió en español lo que se dijo", "hola" in palabras and "tokio" in palabras and "hora" in palabras)

    print("== Nadie habla")
    silencio = trabajo / "silencio.wav"
    rng = np.random.default_rng(2)
    silencio.write_bytes(audio.a_wav((rng.standard_normal(16000 * 12) * 10 ** (-62 / 20)).astype(np.float32), 16000))
    os.environ["VORTEX_MIKU_ENTRADA"] = str(silencio)
    o2 = oido.Oido(silencio=1.5, espera=3.0)
    t0 = time.monotonic()
    comprobar("sin voz no devuelve nada", o2.escuchar() is None)
    comprobar("se rinde tras la espera", 2.5 < time.monotonic() - t0 < 5.5, f"{time.monotonic() - t0:.1f}s")

    print("== Clic en el micrófono corta la escucha")
    os.environ["VORTEX_MIKU_ENTRADA"] = str(entrada)
    o3 = oido.Oido(silencio=1.5, espera=8.0)
    import threading
    threading.Timer(fin_voz - 0.3, o3.cortar_ahora).start()      # clic poco antes de terminar de hablar
    t0 = time.monotonic()
    pcm3 = o3.escuchar()
    comprobar("el clic termina la escucha ya, con lo dicho hasta entonces", pcm3 is not None and time.monotonic() - t0 < fin_voz + 0.5, f"{time.monotonic() - t0:.1f}s")

    print("== Cancelar (se escribió en vez de hablar)")
    o4 = oido.Oido(silencio=1.5, espera=8.0)
    threading.Timer(1.5, o4.cancelar).start()
    t0 = time.monotonic()
    comprobar("cancelar descarta lo grabado", o4.escuchar() is None and time.monotonic() - t0 < 3)
    hijos = subprocess.run(["ps", "--ppid", str(os.getpid()), "-o", "pid=,stat=,comm="], capture_output=True, text=True).stdout.strip().splitlines()
    vivos = [h for h in hijos if " Z" not in h and not h.split()[-1] in ("ps",)]
    comprobar("el grabador se cerró (ningún ffmpeg ni pw-record hijo)", not vivos, "; ".join(vivos))

    print("== Alucinaciones de Whisper")
    comprobar("«Subtítulos por la comunidad de Amara.org» se descarta como si no hubieras dicho nada",
              oido._FRASES_FANTASMA.match("Subtítulos realizados por la comunidad de Amara.org") is not None)
    comprobar("«gracias» (de verdad) no se descarta", oido._FRASES_FANTASMA.match("gracias") is None)

    print("== Privacidad: no se guarda audio")
    nuevos = [p for p in listados() - antes if not p.startswith(str(trabajo))]
    comprobar("no apareció ningún archivo nuevo en /tmp, /dev/shm ni el runtime", not nuevos, str(nuevos[:5]))
    print("FALLOS:", len(fallos), fallos if fallos else "")
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
