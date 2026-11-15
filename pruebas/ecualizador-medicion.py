#!/usr/bin/env python3
"""Mide el ecualizador de verdad: ruido rosa por EasyEffects y el espectro de lo que sale.

Levanta un PipeWire aparte (su propio XDG_RUNTIME_DIR, su D-Bus, WirePlumber sin tarjetas ni Bluetooth) con
una sola salida, un sink nulo «prueba_salida», y un EasyEffects que solo ve esa salida. Así no se toca nada
del sonido de la persona: ni su salida, ni su volumen, ni su EasyEffects ni su preset.

Pone el ecualizador con el mismo script del panel de música (media/equalizer.sh) y con el de Vortex Player
(vortex_player.ecualizador), reproduce ruido rosa hacia EasyEffects, graba el monitor de la salida de prueba
y compara el espectro contra el ecualizador plano: una banda a +12 dB tiene que subir esa zona y dejar las
demás casi igual. También revisa que se mantenga al reiniciar EasyEffects y que «Plano» lo deje como estaba.

    pruebas/ecualizador-medicion.sh   (con límite de memoria, D-Bus aparte y sin ventanas)
"""

import json
import math
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import wave
from pathlib import Path

import numpy as np

AQUI = Path(__file__).resolve().parent.parent
SCRIPT_EQ = AQUI / "shell/src/quickshell/media/equalizer.sh"
TASA = 48000
FALLOS = 0


def probar(nombre, ok, detalle=""):
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}" + (f"  ({detalle})" if detalle else ""), flush=True)
    if not ok:
        FALLOS += 1


def ruido_rosa(ruta: Path, segundos: float = 9.0) -> None:
    """Ruido rosa (1/f) estéreo a -18 dBFS de RMS, 16 bits."""
    n = int(TASA * segundos)
    rng = np.random.default_rng(7)
    canales = []
    for _ in range(2):
        espectro = np.fft.rfft(rng.standard_normal(n))
        f = np.fft.rfftfreq(n, 1 / TASA)
        f[0] = f[1]
        espectro /= np.sqrt(f)
        x = np.fft.irfft(espectro, n)
        x *= 10 ** (-18 / 20) / np.sqrt(np.mean(x ** 2))
        canales.append(x)
    datos = np.clip(np.stack(canales, axis=1), -1, 1)
    with wave.open(str(ruta), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(TASA)
        w.writeframes((datos * 32767).astype("<i2").tobytes())


def espectro(ruta: Path) -> tuple[np.ndarray, np.ndarray]:
    """Densidad espectral promedio (Welch, ventanas de Hann de 8192) de la grabación, sin el primer segundo."""
    with wave.open(str(ruta), "rb") as w:
        ch, ancho = w.getnchannels(), w.getsampwidth()
        x = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2" if ancho == 2 else "<f4").astype(float)
    x = x.reshape(-1, ch).mean(axis=1)[TASA:]
    if ancho == 2:
        x /= 32768
    n = 8192
    ventana = np.hanning(n)
    trozos = [x[i:i + n] * ventana for i in range(0, len(x) - n, n // 2)]
    if not trozos:
        return np.array([]), np.array([])
    psd = np.mean([np.abs(np.fft.rfft(t)) ** 2 for t in trozos], axis=0)
    return np.fft.rfftfreq(n, 1 / TASA), psd


def nivel(f: np.ndarray, psd: np.ndarray, centro: float) -> float:
    """dB de la energía en un tercio de octava alrededor de «centro»."""
    zona = (f >= centro / 2 ** (1 / 6)) & (f <= centro * 2 ** (1 / 6))
    return 10 * math.log10(float(np.sum(psd[zona])) + 1e-20)


class Audio:
    """El PipeWire aparte, con su WirePlumber y su EasyEffects."""

    def __init__(self, tmp: Path):
        self.tmp = tmp
        self.procesos: dict[str, subprocess.Popen] = {}
        h = tmp / "home"
        run = tmp / "run"
        for d in (h, run, h / ".config/pipewire/pipewire.conf.d", h / ".config/wireplumber/wireplumber.conf.d"):
            d.mkdir(parents=True, exist_ok=True)
        run.chmod(0o700)
        self.env = {k: v for k, v in os.environ.items()
                    if k not in ("PIPEWIRE_REMOTE", "PULSE_SERVER", "WAYLAND_DISPLAY", "DISPLAY", "PIPEWIRE_RUNTIME_DIR",
                                 "SERPANTINUM_DIR", "VORTEX_PLAYER_EQ_MPV", "VORTEX_PLAYER_EQ")}
        self.env.update(HOME=str(h), XDG_RUNTIME_DIR=str(run), XDG_CONFIG_HOME=str(h / ".config"),
                        XDG_DATA_HOME=str(h / ".local/share"), XDG_STATE_HOME=str(h / ".local/state"),
                        XDG_CACHE_HOME=str(h / ".cache"), QT_QPA_PLATFORM="offscreen",
                        SERPANTINUM_DIR=str(AQUI / "shell/src"))
        # Una sola salida, nula: lo que llegue se puede grabar de su monitor y no suena en ningún lado.
        (h / ".config/pipewire/pipewire.conf.d/prueba.conf").write_text("""
context.objects = [
  { factory = adapter
    args = { factory.name = support.null-audio-sink node.name = "prueba_salida" node.description = "Salida de prueba"
             media.class = "Audio/Sink" audio.position = [ FL FR ] audio.rate = 48000 } }
]
""")
        # Sin tarjetas de sonido, Bluetooth ni cámaras: este PipeWire no toca el hardware de la persona.
        (h / ".config/wireplumber/wireplumber.conf.d/prueba.conf").write_text("""
wireplumber.profiles = {
  main = {
    monitor.alsa = disabled
    monitor.alsa-midi = disabled
    monitor.bluez = disabled
    monitor.bluez-midi = disabled
    monitor.v4l2 = disabled
    monitor.libcamera = disabled
    support.logind = disabled
  }
}
""")

    def correr(self, nombre: str, cmd: list[str]) -> None:
        log = open(self.tmp / f"{nombre}.log", "ab")
        self.procesos[nombre] = subprocess.Popen(cmd, env=self.env, stdout=log, stderr=subprocess.STDOUT,
                                                 stdin=subprocess.DEVNULL, start_new_session=True)

    def parar(self, nombre: str) -> None:
        p = self.procesos.pop(nombre, None)
        if p and p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)

    def nodos(self) -> set[str]:
        r = subprocess.run(["pw-dump"], env=self.env, capture_output=True, text=True, timeout=20)
        try:
            return {o["info"]["props"].get("node.name", "") for o in json.loads(r.stdout or "[]")
                    if o.get("type") == "PipeWire:Interface:Node"}
        except (ValueError, KeyError, TypeError):
            return set()

    def esperar_nodo(self, nombre: str, segundos: float = 20) -> bool:
        limite = time.monotonic() + segundos
        while time.monotonic() < limite:
            if nombre in self.nodos():
                return True
            time.sleep(0.3)
        return False

    def arrancar(self) -> bool:
        self.correr("pipewire", ["pipewire"])
        time.sleep(0.8)
        self.correr("wireplumber", ["wireplumber"])
        return self.esperar_nodo("prueba_salida")

    def arrancar_easyeffects(self) -> bool:
        self.correr("easyeffects", ["easyeffects", "-w"])
        return self.esperar_nodo("easyeffects_sink", 30)

    def parar_easyeffects(self) -> None:
        subprocess.run(["easyeffects", "-q"], env=self.env, capture_output=True, timeout=15)
        time.sleep(1)
        self.parar("easyeffects")

    def eq(self, *args: str) -> bool:
        return subprocess.run(["bash", str(SCRIPT_EQ), *args], env=self.env, capture_output=True, timeout=30).returncode == 0

    def medir(self, ruido: Path, nombre: str) -> tuple[np.ndarray, np.ndarray]:
        """Reproduce el ruido hacia EasyEffects y graba lo que sale por la salida de prueba."""
        grab = self.tmp / f"{nombre}.wav"
        grab.unlink(missing_ok=True)
        self.correr(f"rec-{nombre}", ["pw-record", "--target", "prueba_salida", "-P", "{ stream.capture.sink = true }",
                                      "--rate", str(TASA), "--channels", "2", "--format", "s16", str(grab)])
        time.sleep(0.8)
        self.correr(f"play-{nombre}", ["pw-play", "--target", "easyeffects_sink", str(ruido)])
        time.sleep(6.5)
        self.parar(f"play-{nombre}")
        self.parar(f"rec-{nombre}")
        time.sleep(0.3)
        return espectro(grab) if grab.exists() else (np.array([]), np.array([]))

    def cerrar(self) -> None:
        try:
            self.parar_easyeffects()
        except (OSError, subprocess.SubprocessError):
            pass
        for nombre in list(self.procesos)[::-1]:
            self.parar(nombre)


def comparar(base, otro, centro: float, testigos: tuple[float, ...]) -> tuple[float, list[float]]:
    sube = nivel(*otro, centro) - nivel(*base, centro)
    resto = [nivel(*otro, c) - nivel(*base, c) for c in testigos]
    return sube, resto


def main() -> int:
    for programa in ("pipewire", "wireplumber", "pw-play", "pw-record", "pw-dump", "easyeffects", "jq"):
        if not shutil.which(programa):
            print(f"Falta {programa}: se salta.")
            return 0
    tmp = Path(tempfile.mkdtemp(prefix="vortex-eq-medicion-"))
    audio = Audio(tmp)
    try:
        print("== PipeWire aparte, con una salida de prueba y EasyEffects")
        probar("arranca PipeWire aparte con la salida de prueba", audio.arrancar())
        probar("arranca EasyEffects (sin ventana) en ese PipeWire", audio.arrancar_easyeffects())
        ruido = tmp / "ruido-rosa.wav"
        ruido_rosa(ruido)

        print("== Panel de música (equalizer.sh): plano contra 1 kHz a +12 dB")
        probar("pone el ecualizador plano", audio.eq("preset", "Flat"))
        time.sleep(2)
        plano = audio.medir(ruido, "plano")
        probar("sale sonido por la salida de prueba", len(plano[1]) > 0 and nivel(*plano, 1000) > -60,
               f"{nivel(*plano, 1000):.1f} dB a 1 kHz" if len(plano[1]) else "nada grabado")
        bandas = "31:0,63:0,125:0,250:0,500:0,1000:12,2000:0,4000:0,8000:0,16000:0"
        probar("sube 1 kHz a +12 dB", audio.eq("set_bands", bandas) and audio.eq("apply"))
        time.sleep(2)
        mil = audio.medir(ruido, "mil")
        sube, resto = comparar(plano, mil, 1000, (100, 8000))
        print(f"      1 kHz: {sube:+.1f} dB · 100 Hz: {resto[0]:+.1f} dB · 8 kHz: {resto[1]:+.1f} dB")
        probar("el espectro sube en 1 kHz (al menos +8 dB)", sube >= 8, f"{sube:+.1f} dB")
        probar("y casi no cambia lejos de ahí (100 Hz y 8 kHz, ±3 dB)", all(abs(r) <= 3 for r in resto),
               ", ".join(f"{r:+.1f}" for r in resto))

        print("== Se mantiene al reiniciar EasyEffects (como al volver a iniciar sesión)")
        audio.parar_easyeffects()
        audio.arrancar_easyeffects()
        audio.eq("restaurar")
        time.sleep(2)
        otra_vez = audio.medir(ruido, "reinicio")
        sube2, _ = comparar(plano, otra_vez, 1000, ())
        probar("después de reiniciar sigue subido 1 kHz", sube2 >= 8, f"{sube2:+.1f} dB")

        print("== Vortex Player (vortex_player.ecualizador): 125 Hz a +12 dB")
        sys.path.insert(0, str(AQUI / "apps/vortex-player"))
        os.environ.update(audio.env)  # su script y su estado son los del PipeWire aparte
        from vortex_player import ecualizador
        probar("el player usa el mismo ecualizador del panel (EasyEffects)", not ecualizador.en_mpv() and ecualizador.script() == SCRIPT_EQ)
        b = [{"f": f, "g": 12.0 if f == 125 else 0.0} for f in (31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)]
        probar("pone 125 Hz a +12 dB desde el player", ecualizador.poner_bandas(b))
        time.sleep(2)
        graves = audio.medir(ruido, "graves")
        sube3, resto3 = comparar(plano, graves, 125, (1000, 8000))
        print(f"      125 Hz: {sube3:+.1f} dB · 1 kHz: {resto3[0]:+.1f} dB · 8 kHz: {resto3[1]:+.1f} dB")
        probar("el espectro sube en 125 Hz (al menos +8 dB)", sube3 >= 8, f"{sube3:+.1f} dB")
        probar("y casi no cambia en 1 kHz y 8 kHz (±3 dB)", all(abs(r) <= 3 for r in resto3), ", ".join(f"{r:+.1f}" for r in resto3))

        print("== Restablecer (Plano)")
        probar("el preset Plano se aplica", ecualizador.poner_preset("Flat"))
        time.sleep(2)
        de_nuevo = audio.medir(ruido, "de-nuevo")
        dif = [nivel(*de_nuevo, c) - nivel(*plano, c) for c in (125, 1000, 8000)]
        probar("y el sonido vuelve a ser el de antes (±1.5 dB)", all(abs(d) <= 1.5 for d in dif), ", ".join(f"{d:+.1f}" for d in dif))
    finally:
        audio.cerrar()
        log = (tmp / "easyeffects.log").read_text(errors="replace") if (tmp / "easyeffects.log").exists() else ""
        if "Could not find the plugin" in log:
            print("\n  EasyEffects dice que le faltan plugins: " +
                  ", ".join(sorted({l.split("plugin: ")[-1].strip() for l in log.splitlines() if "Could not find the plugin" in l})[:4]))
            print("  → falta el paquete lsp-plugins-lv2 (sin él, el ecualizador deja pasar el audio tal cual).")
        shutil.rmtree(tmp, ignore_errors=True)
    print()
    if FALLOS:
        print(f"FALLARON {FALLOS} pruebas")
        return 1
    print("Todo bien")
    return 0


if __name__ == "__main__":
    sys.exit(main())
