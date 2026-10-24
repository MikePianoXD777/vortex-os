#!/usr/bin/env python3
"""Proceso de la voz natural de Miku (Qwen3-TTS con faster-qwen3-tts): misma conversación que voz_worker.

No corre con el Python del sistema sino con el de ~/.local/share/vortex/miku/natural/venv, donde
«vortex-miku natural instalar» puso PyTorch y el motor (no caben en la ISO). La voz es la de
voces/miku-natural.wav: una voz inventada con Qwen3-TTS VoiceDesign a partir de una descripción
(no es la de ninguna persona), que el modelo Base repite con cualquier texto.

Dos modos (el archivo «modo» de esa carpeta, lo elige la instalación):
  gpu   el modelo 1.7B en PyTorch con CUDA graphs. Carga al abrir a Miku (~8 s, ~5 GB de VRAM) y lo
        suelta al cerrarla: sin Miku abierta no ocupa la GPU.
  cpu   el modelo 0.6B cuantizado (GGUF Q4_K_M) con qwentts.cpp (ggml), para PCs sin una GPU que
        sirva. Lenta: cada frase tarda unos 6 s más lo que dura (casi todo es leer la voz de
        muestra, que va delante de cada frase). Carga en ~8 s y usa ~4 GB de RAM.

  entra   {"id": 1, "texto": "Hola."}                 (o {"cmd": "salir"})
  sale    {"listo": true, "voz": "miku-natural", "modo": "gpu"|"cpu"}
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


class VozNatural:
    def __init__(self, id_voz: str):
        self.voz = voces.buscar(id_voz)
        if self.voz is None or self.voz.get("motor") != "qwen":
            raise ValueError(f"«{id_voz}» no es una voz natural.")
        self.modo = voces.modo_natural()
        self.muestra = str(rutas.RAIZ / self.voz["muestra"])
        self.muestra_texto = self.voz["muestra_texto"]
        if self.modo == "cpu":
            # La rueda de qwentts.cpp que pone la instalación es la de CPU: no carga nada de CUDA.
            from faster_qwen3_tts import FasterQwen3TTS
            self.modelo = FasterQwen3TTS.from_pretrained(
                self.voz["modelo_cpu"], backend="ggml", quant=self.voz["cuantizacion_cpu"],
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

    def decir_crudo(self, texto: str, **opciones):
        wavs, sr = self.modelo.generate_voice_clone(text=texto, language="Spanish", ref_audio=self.muestra,
                                                    ref_text=self.muestra_texto, **opciones)
        return wavs[0], sr

    def sintetizar(self, texto: str):
        import numpy as np
        hablado = motor_voz.texto_hablado(elevenlabs.quitar_etiquetas(texto))
        if not hablado:
            raise ValueError("no hay nada que decir")
        x, sr = self.decir_crudo(hablado)
        x = np.asarray(x, dtype=np.float32)
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
