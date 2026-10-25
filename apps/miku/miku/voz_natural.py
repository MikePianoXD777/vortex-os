#!/usr/bin/env python3
"""Proceso de la voz natural de Miku (Qwen3-TTS con faster-qwen3-tts): misma conversación que voz_worker.

No corre con el Python del sistema sino con el de ~/.local/share/vortex/miku/natural/venv, donde
«vortex-miku natural instalar» puso PyTorch y el motor (no caben en la ISO). La voz es la de
voces/miku-natural.wav: una voz inventada con Qwen3-TTS VoiceDesign a partir de una descripción
(no es la de ninguna persona), que el modelo Base repite con cualquier texto.

Tres modos (el archivo «modo» de esa carpeta, lo elige la instalación):
  gpu   el modelo 1.7B en PyTorch con CUDA graphs. Carga al abrir a Miku (~8 s, ~5 GB de VRAM) y lo
        suelta al cerrarla: sin Miku abierta no ocupa la GPU.
  cuda  el mismo 1.7B (la misma voz) cuantizado en GGUF (Q8_0, o Q4_K_M con menos de 8 GB) con
        qwentts.cpp en la GPU: para las NVIDIA serie 10 (GTX 1060) o de 4-5 GB. ~4-5 GB de VRAM.
  cpu   el mismo 1.7B en GGUF Q4_K_M (con menos de 12 GB de RAM, el 0.6B) con qwentts.cpp (ggml), para
        PCs sin una GPU que sirva. Lenta: cada frase tarda unos 7 s más lo que dura (casi todo es leer
        la voz de muestra, que va delante de cada frase). Carga en ~5 s y usa ~5,5 GB de RAM (el 0.6B, ~4).

  entra   {"id": 1, "texto": "Hola."}                 (o {"cmd": "salir"})
  sale    {"listo": true, "voz": "miku-natural", "modo": "gpu"|"cuda"|"cpu"}
          {"id": 1, "wav": "...", "seg": 1.8, "visemas": [...]}  o  {"id": 1, "error": "..."}

También: voz_natural.py --voz miku-natural --salida frase.wav "Hola."
"""

import argparse
import contextlib
import itertools
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from miku import audio, elevenlabs, motor_voz, rutas, voces  # noqa: E402
from miku.voz_worker import morir_con_el_padre  # noqa: E402

# Todo lo de Hugging Face (el modelo y su tokenizador de audio) sale de la carpeta de la voz natural, sin red.
os.environ["HF_HUB_CACHE"] = str(rutas.NATURAL / "hf")
os.environ.setdefault("HF_HUB_OFFLINE", "1")


def tope_tokens(texto: str) -> int:
    """Cuántos pasos puede generar como mucho: el códec da 12 pasos por segundo de audio, y se calcula para
    hablar muy despacio (8 letras por segundo). Sin tope, a veces el modelo se desboca y genera los 2048
    pasos (casi 3 minutos de audio sin sentido); en la CPU eso traba a Miku varios minutos."""
    return min(2048, int(12.5 * max(4.0, len(texto) / 8)) + 24)


def minimo_tokens(texto: str) -> int:
    """Pasos antes de los que el modelo no puede terminar (QWENTTS_MIN_NEW_TOKENS): lo que tardaría en decir el
    texto hablando muy rápido (22 letras por segundo, 12,5 pasos por segundo), y medio segundo como mínimo."""
    return max(6, round(12.5 * len(texto) / 22))


def duracion_minima(texto: str) -> float:
    """Segundos por debajo de los cuales el audio salió cortado: ni hablando muy rápido (40 letras por
    segundo) se dice el texto en menos."""
    return max(0.25, len(texto) / 40)


class VozNatural:
    def __init__(self, id_voz: str):
        self.voz = voces.buscar(id_voz)
        if self.voz is None or self.voz.get("motor") != "qwen":
            raise ValueError(f"«{id_voz}» no es una voz natural.")
        self.modo = voces.modo_natural()
        self.muestra = str(rutas.RAIZ / self.voz["muestra"])
        self.muestra_texto = self.voz["muestra_texto"]
        if self.modo in ("cpu", "cuda"):
            # Los dos con qwentts.cpp: la compilación de Vortex de NATURAL/qwentts (la de CPU, o la de CUDA para
            # usar la GPU con el mismo modelo que en modo gpu).
            cuda = self.modo == "cuda"
            if not cuda:
                # ggml reparte el trabajo con OpenMP: con los núcleos físicos va igual de rápido que con todos los
                # hilos y deja respirar al resto del sistema. Se lee al cargar la biblioteca: antes del import.
                os.environ.setdefault("OMP_NUM_THREADS", str(rutas.hilos()))
            from faster_qwen3_tts import FasterQwen3TTS
            self.modelo = FasterQwen3TTS.from_pretrained(
                self.voz["modelo"] if cuda else voces.modelo_natural_cpu(self.voz), backend="ggml",
                quant=voces.cuantizacion_natural() if cuda else self.voz["cuantizacion_cpu"],
                qwentts_library_path=self.libqwen(),
                cache_dir=str(rutas.NATURAL / "hf"), local_files_only=True,
                qwentts_ref_cache_dir=str(rutas.NATURAL / "referencias"))   # la voz de muestra ya procesada
            # Calienta con pocos pasos: procesa la muestra (la 1.ª vez, y la guarda) sin generar una frase entera.
            self.decir_crudo("Hola.", max_new_tokens=4)
            return
        import torch
        if not torch.cuda.is_available():
            raise RuntimeError("no hay una GPU NVIDIA disponible para la voz natural")
        from faster_qwen3_tts import FasterQwen3TTS
        self.modelo = FasterQwen3TTS.from_pretrained(self.voz["modelo"], cache_dir=str(rutas.NATURAL / "hf"),
                                                     local_files_only=True)
        self.decir_crudo("Hola.")                       # calienta: captura los CUDA graphs ya, no en la primera frase

    @staticmethod
    def libqwen() -> str | None:
        """La compilación de qwentts.cpp de Vortex (con el parche de QWENTTS_MIN_NEW_TOKENS). Las instalaciones en
        la CPU de antes de la 1.7 no la tienen: usan la de la rueda (y «natural mejorar» las rehace)."""
        lib = rutas.NATURAL / "qwentts" / "libqwen.so"
        return str(lib) if lib.is_file() else None

    def decir_crudo(self, texto: str, **opciones):
        wavs, sr = self.modelo.generate_voice_clone(text=texto, language="Spanish", ref_audio=self.muestra,
                                                    ref_text=self.muestra_texto, **opciones)
        return wavs[0], sr

    def sintetizar(self, texto: str):
        import numpy as np
        hablado = motor_voz.texto_hablado(elevenlabs.quitar_etiquetas(texto))
        if not hablado:
            raise ValueError("no hay nada que decir")
        # Con qwentts.cpp (modos cpu y cuda), el modelo cuantizado tiende a terminar en los primeros pasos y sale
        # un audio vacío o de una décima (sobre todo con frases cortas, como «Hola, ¿cómo estás?»). La compilación
        # de Vortex no lo deja terminar antes de minimo_tokens(); y si aun así sale demasiado corto, se repite.
        os.environ["QWENTTS_MIN_NEW_TOKENS"] = str(minimo_tokens(hablado))   # lo lee qwentts.cpp en cada paso
        for _ in range(4 if self.modo in ("cpu", "cuda") else 1):
            x, sr = self.decir_crudo(hablado, max_new_tokens=tope_tokens(hablado))
            x = np.asarray(x, dtype=np.float32)
            if len(x) >= sr * duracion_minima(hablado):
                break
        return audio.nivelar(x, sr, rms_db=-22.0, relacion=1.6), sr


def servir(voz: VozNatural, id_voz: str) -> None:
    morir_con_el_padre()
    carpeta = rutas.temporales()

    def enviar(obj):
        sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
        sys.stdout.flush()

    enviar({"listo": True, "voz": id_voz, "modo": voz.modo})
    contador = itertools.count()
    for linea in sys.stdin:
        try:
            pedido = json.loads(linea)
        except ValueError:
            continue
        if pedido.get("cmd") == "salir":
            break
        n = pedido.get("id")
        try:
            t0 = time.monotonic()
            with contextlib.redirect_stdout(sys.stderr):
                x, sr = voz.sintetizar(str(pedido.get("texto", "")))
            ruta = carpeta / f"voz-{os.getpid()}-{next(contador)}.wav"
            fd = os.open(ruta, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
            with os.fdopen(fd, "wb") as f:
                f.write(audio.a_wav(x, sr))
            enviar({"id": n, "wav": str(ruta), "seg": round(len(x) / sr, 3), "visemas": audio.visemas(x, sr),
                    "tardo": round(time.monotonic() - t0, 2)})
        except Exception as e:  # una frase mala no debe tumbar la voz
            enviar({"id": n, "error": f"{type(e).__name__}: {e}"})


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--voz", default="miku-natural")
    p.add_argument("--salida", help="escribe una frase a este WAV en vez de servir por stdin/stdout")
    p.add_argument("texto", nargs="*")
    a = p.parse_args()
    try:
        with contextlib.redirect_stdout(sys.stderr):      # la librería avisa con print(); stdout es para el JSON
            voz = VozNatural(a.voz)
    except Exception as e:
        mensaje = f"{type(e).__name__}: {e}"
        if a.salida:
            print(f"vortex-miku: no pude cargar la voz natural ({mensaje})", file=sys.stderr)
        else:
            print(json.dumps({"error": mensaje}, ensure_ascii=False), flush=True)
        return 1
    if a.salida:
        x, sr = voz.sintetizar(" ".join(a.texto))
        Path(a.salida).write_bytes(audio.a_wav(x, sr))
        return 0
    servir(voz, a.voz)
    return 0


if __name__ == "__main__":
    sys.exit(main())
