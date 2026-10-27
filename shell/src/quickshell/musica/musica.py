#!/usr/bin/env python3
"""Vortex 1.10: el metrónomo y el afinador del panel «Metrónomo y afinador» (musica/MusicaPopup.qml).

  musica.py DIR metronomo BPM COMPAS   empieza a sonar (o, si ya suena, cambia el tempo y el compás)
  musica.py DIR parar                  lo detiene
  musica.py afinador                   escucha el micrófono: una línea JSON por lectura, 10 por segundo

El metrónomo es su propio proceso, para que siga sonando con el panel cerrado: uno solo (candado en
DIR/metronomo.lock) que lee DIR/metronomo-control.json y escribe DIR/metronomo.json en cada pulso.
Hace el sonido él mismo y se lo pasa a pw-cat: los pulsos se cuentan en muestras, no con
temporizadores, así caen exactos aunque la computadora esté ocupada.

El afinador lee el micrófono con pw-record y busca la nota con YIN (de Cheveigné y Kawahara, 2002),
en Python puro: no necesita numpy.
"""
import ctypes
import fcntl
import json
import math
import os
import signal
import subprocess
import sys
import time
from array import array
from operator import mul

PROPIEDADES = '{"application.name":"Vortexine\'s OS","media.role":"Music"}'


def morir_con_el_padre():
    # Si el shell se cae, que no se quede escuchando el micrófono ni sonando (PR_SET_PDEATHSIG).
    try:
        ctypes.CDLL(None).prctl(1, signal.SIGTERM)
    except Exception:
        pass


def escribir_json(ruta, datos):
    tmp = ruta + ".tmp"
    with open(tmp, "w") as f:
        json.dump(datos, f)
    os.replace(tmp, ruta)


def leer_json(ruta):
    try:
        with open(ruta) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


# ---------------------------------------------------------------- metrónomo

TASA = 48000
BLOQUE = 480  # 10 ms: cada cuánto revisa si cambió el tempo
ADELANTO = 0.06  # lo que tarda en oírse lo que se escribe: el pulso se marca en el panel cuando suena


def golpe(frecuencia, volumen):
    n = int(TASA * 0.035)
    return array("h", (int(32767 * volumen * math.sin(2 * math.pi * frecuencia * i / TASA) * math.exp(-i / (TASA * 0.008)))
                       for i in range(n)))


def ajustes(control):
    bpm = max(20, min(300, int(control.get("bpm", 100))))
    compas = max(1, min(12, int(control.get("compas", 4))))
    return bpm, compas


def metronomo(carpeta, bpm, compas):
    os.makedirs(carpeta, exist_ok=True)
    ruta_control = os.path.join(carpeta, "metronomo-control.json")
    ruta_estado = os.path.join(carpeta, "metronomo.json")
    escribir_json(ruta_control, {"bpm": bpm, "compas": compas, "sonando": True})

    candado = open(os.path.join(carpeta, "metronomo.lock"), "w")
    try:
        fcntl.flock(candado, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        return 0  # ya está sonando: lee el control nuevo solo

    salir = []
    signal.signal(signal.SIGTERM, lambda *_: salir.append(1))
    signal.signal(signal.SIGINT, lambda *_: salir.append(1))
    try:
        cat = subprocess.Popen(["pw-cat", "--playback", "--raw", "--format", "s16", "--rate", str(TASA),
                                "--channels", "1", "--latency", "20ms", "-P", PROPIEDADES, "-"],
                               stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        escribir_json(ruta_estado, {"sonando": False, "error": "pw-cat"})
        return 1
    try:
        fcntl.fcntl(cat.stdin.fileno(), 1031, 4096)  # F_SETPIPE_SZ: que un cambio de tempo se oiga ya
    except OSError:
        pass

    acento, normal = golpe(1760, 0.9), golpe(1175, 0.6)
    silencio = bytes(BLOQUE * 2)
    muestra = 0  # muestras escritas
    siguiente = 0.0  # muestra del próximo pulso
    ultimo = 0.0
    pulso = 0
    cuenta = 0
    resto = array("h")
    pendientes = []  # (cuándo se oye, número de pulso, pulso en el compás)
    marca = None
    estado = {"sonando": True, "bpm": bpm, "compas": compas, "pulso": -1, "n": 0, "t": time.time()}
    escribir_json(ruta_estado, estado)

    while not salir:
        try:
            m = os.stat(ruta_control).st_mtime_ns
        except OSError:
            m = None
        if m != marca:
            marca = m
            control = leer_json(ruta_control)
            if not control.get("sonando", True):
                break
            nuevo_bpm, nuevo_compas = ajustes(control)
            if nuevo_bpm != bpm and cuenta:
                siguiente = max(float(muestra), ultimo + TASA * 60 / nuevo_bpm)
            if nuevo_compas != compas:
                pulso = 0
            bpm, compas = nuevo_bpm, nuevo_compas
            estado.update(bpm=bpm, compas=compas)
            escribir_json(ruta_estado, estado)
        periodo = TASA * 60 / bpm

        buf = array("h", silencio)
        if resto:
            k = min(len(resto), BLOQUE)
            buf[0:k] = resto[0:k]
            resto = resto[k:]
        while siguiente < muestra + BLOQUE:
            desde = max(0, int(round(siguiente)) - muestra)
            g = acento if compas > 1 and pulso == 0 else normal
            k = min(len(g), BLOQUE - desde)
            buf[desde:desde + k] = g[0:k]
            resto = g[k:]
            pendientes.append((time.monotonic() + ADELANTO + desde / TASA, cuenta, pulso))
            ultimo = siguiente
            cuenta += 1
            pulso = (pulso + 1) % compas
            siguiente += periodo
        try:
            cat.stdin.write(buf.tobytes())
            cat.stdin.flush()
        except (BrokenPipeError, ValueError, OSError):
            break
        muestra += BLOQUE

        ahora = time.monotonic()
        while pendientes and pendientes[0][0] <= ahora:
            _, n, p = pendientes.pop(0)
            estado.update(pulso=p, n=n + 1, t=time.time())
            escribir_json(ruta_estado, estado)

    try:
        cat.stdin.close()
    except OSError:
        pass
    cat.terminate()
    estado.update(sonando=False, pulso=-1)
    escribir_json(ruta_estado, estado)
    return 0


def parar(carpeta):
    ruta = os.path.join(carpeta, "metronomo-control.json")
    control = leer_json(ruta)
    control["sonando"] = False
    if os.path.isdir(carpeta):
        escribir_json(ruta, control)
    return 0


# ---------------------------------------------------------------- afinador

TASA_MIC = 16000
LECTURA = 1600  # 100 ms entre lecturas
VENTANA = 2048
RETRASO_MAX = TASA_MIC // 55  # La1, lo más grave que busca
RETRASO_MIN = TASA_MIC // 1600  # lo más agudo: un poco arriba del Sol5
UMBRAL = 0.15
SILENCIO = 400  # RMS (de 32768) por debajo del que no hay nota


def tono(x):
    """Frecuencia de la nota en x (YIN), o 0 si no hay una clara."""
    w = len(x) - RETRASO_MAX - 1
    acumulado = [0.0]
    for v in x:
        acumulado.append(acumulado[-1] + v * v)
    e0 = acumulado[w]
    cabeza = x[:w]
    d = [0.0] * (RETRASO_MAX + 2)
    for tau in range(1, RETRASO_MAX + 2):
        d[tau] = e0 + acumulado[tau + w] - acumulado[tau] - 2 * sum(map(mul, cabeza, x[tau:tau + w]))
    dn = [1.0] * (RETRASO_MAX + 2)
    suma = 0.0
    for tau in range(1, RETRASO_MAX + 2):
        suma += d[tau]
        dn[tau] = d[tau] * tau / suma if suma > 0 else 1.0

    tau = RETRASO_MIN
    while tau <= RETRASO_MAX and dn[tau] >= UMBRAL:
        tau += 1
    if tau > RETRASO_MAX:
        tau = min(range(RETRASO_MIN, RETRASO_MAX + 1), key=lambda t: dn[t])
        if dn[tau] > 0.35:
            return 0.0
    while tau < RETRASO_MAX and dn[tau + 1] < dn[tau]:
        tau += 1
    a, b, c = d[tau - 1], d[tau], d[tau + 1]  # la diferencia sin normalizar: la parábola queda más fina
    curva = a - 2 * b + c
    fino = tau + (0.5 * (a - c) / curva if curva > 0 else 0.0)
    return TASA_MIC / fino


def nota(hz):
    midi = 69 + 12 * math.log2(hz / 440)
    n = round(midi)
    return n, round(100 * (midi - n))


def afinador():
    morir_con_el_padre()
    try:
        rec = subprocess.Popen(["pw-record", "--raw", "--format", "s16", "--rate", str(TASA_MIC), "--channels", "1",
                                "-P", PROPIEDADES, "-"],
                               stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, preexec_fn=morir_con_el_padre)
    except OSError:
        print(json.dumps({"error": "pw-record"}), flush=True)
        return 1
    signal.signal(signal.SIGTERM, lambda *_: (rec.terminate(), sys.exit(0)))

    x = [0.0] * VENTANA
    ultimos = []
    while True:
        datos = rec.stdout.read(LECTURA * 2)
        if not datos or len(datos) < 2:
            break
        nuevas = array("h")
        nuevas.frombytes(datos[:len(datos) // 2 * 2])
        x = (x + [float(v) for v in nuevas])[-VENTANA:]
        rms = math.sqrt(sum(map(mul, x[-LECTURA:], x[-LECTURA:])) / LECTURA)
        salida = {"hz": 0, "nivel": round(min(1.0, rms / 8000), 2)}
        hz = tono(x) if rms >= SILENCIO else 0.0
        if hz:
            ultimos = (ultimos + [hz])[-3:]
            hz = sorted(ultimos)[len(ultimos) // 2]  # la mediana de las últimas 3: la aguja no brinca
            n, cents = nota(hz)
            salida.update(hz=round(hz, 1), midi=n, cents=cents)
        else:
            ultimos = []
        try:
            print(json.dumps(salida), flush=True)
        except BrokenPipeError:
            break
    rec.terminate()
    return 0


def main(args):
    if args[:1] == ["afinador"]:
        return afinador()
    if len(args) >= 2 and args[1] == "parar":
        return parar(args[0])
    if len(args) >= 2 and args[1] == "metronomo":
        bpm, compas = ajustes({"bpm": args[2] if len(args) > 2 else 100, "compas": args[3] if len(args) > 3 else 4})
        return metronomo(args[0], bpm, compas)
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
