#!/usr/bin/env python3
"""Prueba de la voz de ElevenLabs contra un servidor falso (sin internet, sin cuenta, sin gastar créditos).

HOME temporal y sin bus de sesión (secret-tool falla y la clave va al archivo 0600 del HOME de prueba):
no toca tu llavero ni tu miku.json. Comprueba: etiquetas fuera del texto y emociones, el comando
«vortex-miku elevenlabs ID» (revisa la clave sin gastar, la guarda 0600 y elige la voz), «decir» con la
voz de ElevenLabs (pide PCM 24 kHz con las etiquetas y eleven_v4_turbo), el respaldo a eleven_v3 si la
cuenta no tiene el v4, y que una clave mala se explique.

  python3 -I apps/miku/pruebas/elevenlabs.py
"""

import http.server
import json
import math
import os
import struct
import subprocess
import sys
import tempfile
import threading
import wave
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ))

FALLOS = 0
PEDIDOS: list[dict] = []
SIN_V4 = threading.Event()
VOZ = "xYrwTY3IRh6VBodoZUNN"


def probar(nombre: str, ok: bool) -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}")
    FALLOS += 0 if ok else 1


class Falso(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def _responder(self, codigo: int, cuerpo: bytes, tipo: str = "application/json") -> None:
        self.send_response(codigo)
        self.send_header("Content-Type", tipo)
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)

    def do_GET(self):
        if self.headers.get("xi-api-key") != "clave-buena":
            return self._responder(401, b'{"detail":{"status":"invalid_api_key"}}')
        if self.path == f"/v1/voices/{VOZ}":
            return self._responder(200, json.dumps({"name": "Voz de prueba"}).encode())
        self._responder(404, b'{"detail":"voice_not_found"}')

    def do_POST(self):
        cuerpo = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        PEDIDOS.append({"ruta": self.path, "clave": self.headers.get("xi-api-key"), **cuerpo})
        if self.headers.get("xi-api-key") != "clave-buena":
            return self._responder(401, b'{"detail":{"status":"invalid_api_key"}}')
        if SIN_V4.is_set() and cuerpo.get("model_id") == "eleven_v4_turbo":
            return self._responder(400, b'{"detail":{"message":"model not available for this account"}}')
        # 0,6 s de un tono de 220 Hz en PCM de 16 bits a 24 kHz, como pcm_24000
        muestras = [int(8000 * math.sin(2 * math.pi * 220 * i / 24000)) for i in range(int(0.6 * 24000))]
        self._responder(200, struct.pack(f"<{len(muestras)}h", *muestras), "audio/pcm")


def main() -> int:
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Falso)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    tmp = Path(tempfile.mkdtemp(prefix="vortex-miku-eleven."))
    env = {k: v for k, v in os.environ.items() if not k.startswith(("XDG_", "ELEVENLABS"))}
    env.update(HOME=str(tmp), XDG_RUNTIME_DIR=str(tmp / "rt"), DBUS_SESSION_BUS_ADDRESS="disabled:",
               VORTEX_MIKU_ELEVENLABS_API=f"http://127.0.0.1:{srv.server_address[1]}/v1", VORTEX_MIKU_RAIZ=str(RAIZ))
    (tmp / "rt").mkdir(mode=0o700)
    os.environ.update(env)
    from miku import asistente, elevenlabs  # noqa: E402  (después de fijar HOME y la API falsa)

    print("== Etiquetas y emociones")
    probar("quita las etiquetas del texto", elevenlabs.quitar_etiquetas("[sighs] Hola... [whispers] ya casi.") == "Hola... ya casi.")
    probar("no toca corchetes que no son etiquetas", elevenlabs.quitar_etiquetas("usa [1] y [x](y)") == "usa [1] y [x](y)")
    probar("[laughs softly] pone cara alegre", asistente.emocion_de("[laughs softly] Qué bueno.")[0] == "happy")
    probar("[sighs] pone cara triste", asistente.emocion_de("[sighs] Bueno...")[0] == "sad")

    vortex_miku = [sys.executable, "-I", str(RAIZ / "vortex-miku")]
    print("== vortex-miku elevenlabs")
    r = subprocess.run(vortex_miku + ["elevenlabs", VOZ], env={**env, "ELEVENLABS_API_KEY": "clave-mala"},
                       capture_output=True, text=True, stdin=subprocess.DEVNULL)
    probar("con clave mala lo explica y no guarda nada", r.returncode != 0 and "no aceptó la clave" in r.stderr
           and not (tmp / ".config/vortex/miku.json").exists())
    r = subprocess.run(vortex_miku + ["elevenlabs", VOZ], env={**env, "ELEVENLABS_API_KEY": "clave-buena"},
                       capture_output=True, text=True, stdin=subprocess.DEVNULL)
    clave = tmp / ".config/vortex/elevenlabs.clave"
    cfg = json.loads((tmp / ".config/vortex/miku.json").read_text()) if r.returncode == 0 else {}
    probar("revisa la voz sin gastar (solo un GET) y la elige", r.returncode == 0 and "Voz de prueba" in r.stdout
           and cfg.get("voz") == "elevenlabs" and cfg.get("elevenlabs_voz") == VOZ and not PEDIDOS)
    probar("la clave queda en un archivo 0600 (sin llavero)", clave.is_file() and (clave.stat().st_mode & 0o777) == 0o600
           and clave.read_text().strip() == "clave-buena")
    r = subprocess.run(vortex_miku + ["voz"], env=env, capture_output=True, text=True)
    probar("«vortex-miku voz» la muestra lista y elegida", "* elevenlabs" in r.stdout and "[lista]" in r.stdout.split("* elevenlabs")[1].splitlines()[0])

    print("== Hablar con ElevenLabs")
    wav = tmp / "frase.wav"
    r = subprocess.run(vortex_miku + ["decir", "[sighs] Hola... soy Miku.", "-o", str(wav)], env=env, capture_output=True, text=True)
    p = PEDIDOS[-1] if PEDIDOS else {}
    probar("pide PCM 24 kHz con eleven_v4_turbo y las etiquetas", r.returncode == 0 and p.get("model_id") == "eleven_v4_turbo"
           and p.get("ruta", "").endswith(f"/text-to-speech/{VOZ}?output_format=pcm_24000") and p.get("text") == "[sighs] Hola... soy Miku.")
    try:
        with wave.open(str(wav)) as w:
            dur = w.getnframes() / w.getframerate()
        probar("deja un WAV con el audio", 0.5 < dur < 0.7)
    except (OSError, wave.Error):
        probar("deja un WAV con el audio", False)
    SIN_V4.set()
    r = subprocess.run(vortex_miku + ["decir", "Otra frase.", "-o", str(wav)], env=env, capture_output=True, text=True)
    probar("si la cuenta no tiene el v4, usa eleven_v3", r.returncode == 0 and [x.get("model_id") for x in PEDIDOS[-2:]] == ["eleven_v4_turbo", "eleven_v3"])
    clave.write_text("clave-mala\n")
    r = subprocess.run(vortex_miku + ["decir", "Hola.", "-o", str(wav)], env=env, capture_output=True, text=True)
    probar("clave mala sin voz local de respaldo: lo explica", r.returncode != 0 and "no aceptó la clave" in r.stderr)

    r = subprocess.run(vortex_miku + ["elevenlabs", "quitar"], env=env, capture_output=True, text=True)
    cfg = json.loads((tmp / ".config/vortex/miku.json").read_text())
    probar("«elevenlabs quitar» borra la clave y vuelve a la F1", r.returncode == 0 and not clave.exists() and cfg.get("voz") == "supertonic-f1")

    srv.shutdown()
    subprocess.run(["rm", "-rf", str(tmp)])
    print("\nFALLARON", FALLOS, "pruebas" if FALLOS else "") if FALLOS else print("\nTodo bien")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
