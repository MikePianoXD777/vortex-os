#!/usr/bin/env python3
"""Proceso de la voz de Miku: carga el modelo una vez y convierte frases en audio.

Habla con la app por líneas de JSON:
  entra   {"id": 1, "texto": "Hola."}                 (o {"cmd": "salir"})
  sale    {"listo": true, "voz": "supertonic-f1"}     al cargar el modelo
          {"id": 1, "wav": "/run/user/1000/vortex-miku/voz-1.wav", "seg": 1.8, "visemas": [[0,0,0,0,0], ...]}
          {"id": 1, "error": "texto del problema"}
«visemas» tiene un cuadro cada 1/30 s con [aa, ih, ou, ee, oh] de 0 a 100: es lo que mueve la boca.

También sirve para sacar una frase a un archivo (así se prueban las voces, sin reproducir nada):
  voz_worker.py --voz supertonic-f1 --estilo miku --salida frase.wav "Hola... soy Miku."
"""

import argparse
import itertools
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from miku import audio, motor_voz, rutas, voces  # noqa: E402


def morir_con_el_padre() -> None:
    """Si la app muere sin avisar, este proceso muere con ella (PR_SET_PDEATHSIG)."""
    try:
        import ctypes
        import signal
        ctypes.CDLL("libc.so.6", use_errno=True).prctl(1, int(signal.SIGTERM))
        if os.getppid() == 1:
            sys.exit(0)
    except OSError:
        pass


class Voz:
    def __init__(self, id_voz: str, miku: bool, ajustes: dict | None = None):
        self.voz = voces.buscar(id_voz)
        if self.voz is None:
            raise ValueError(f"No existe la voz «{id_voz}». Mira las que hay con: vortex-miku voz")
        if not voces.disponible(self.voz):
            raise FileNotFoundError(f"La voz «{id_voz}» no está instalada. Instálala con: vortex-miku voz {id_voz}")
        self.estilo = motor_voz.estilo_de(self.voz, miku)
        if ajustes:
            self.estilo.update(ajustes)
        self.motor = motor_voz.cargar(self.voz)

    def sintetizar(self, texto: str):
        """Devuelve (audio float32, tasa) ya con el estilo aplicado."""
        hablado = motor_voz.texto_hablado(texto)
        if not hablado:
            raise ValueError("no hay nada que decir")
        e = self.estilo
        x, sr = self.motor.sintetizar(hablado, e.get("velocidad", 1.0))
        return motor_voz.procesar(x, sr, e), sr


def servir(voz: Voz, id_voz: str) -> None:
    morir_con_el_padre()
    salida = sys.stdout
    carpeta = rutas.temporales()

    def enviar(obj):
        salida.write(json.dumps(obj, ensure_ascii=False) + "\n")
        salida.flush()

    enviar({"listo": True, "voz": id_voz})
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
    p.add_argument("--voz", default=voces.VOZ_DEFECTO)
    p.add_argument("--estilo", choices=("miku", "normal"), default="miku")
    p.add_argument("--salida", help="escribe una frase a este WAV en vez de servir por stdin/stdout")
    p.add_argument("--tono", type=float, help="centésimas de semitono (negativo = más grave)")
    p.add_argument("--velocidad", type=float, help="velocidad del motor (1 = normal, 0.95 = un poco más lenta)")
    p.add_argument("--corte", type=float, help="Hz del paso bajo (menos brillo); 0 = sin filtro")
    p.add_argument("--estante", type=float, help="dB que bajan los agudos sobre 3,5 kHz")
    p.add_argument("--nivel", type=float, help="volumen medio en dBFS (-26 bajito, -20 normal)")
    p.add_argument("texto", nargs="*")
    a = p.parse_args()
    ajustes = {k: getattr(a, k) for k in ("tono", "velocidad", "corte", "estante", "nivel") if getattr(a, k) is not None}
    try:
        voz = Voz(a.voz, a.estilo == "miku", ajustes)
    except Exception as e:
        if a.salida:
            print(f"vortex-miku: {e}", file=sys.stderr)
        else:
            print(json.dumps({"error": str(e)}, ensure_ascii=False), flush=True)
        return 1
    if a.salida:
        x, sr = voz.sintetizar(" ".join(a.texto))
        Path(a.salida).write_bytes(audio.a_wav(x, sr))
        return 0
    servir(voz, a.voz)
    return 0


if __name__ == "__main__":
    sys.exit(main())
