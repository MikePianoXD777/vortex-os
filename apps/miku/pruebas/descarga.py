#!/usr/bin/env python3
"""Prueba de la descarga de los modelos (voz y oído) contra un servidor local FALSO: no se baja nada de internet.

    QT_QPA_PLATFORM=offscreen python3 -I pruebas/descarga.py CARPETA_DE_TRABAJO

Con un manifiesto de mentira (VORTEX_MIKU_MANIFIESTO) con archivos pequeños y su sha256, comprueba:
  descarga completa con su progreso; reanudar tras cortarse la conexión (cabecera Range); un archivo que
  llega distinto (sha256) no se instala ni deja restos; sin red lo dice en español; una redirección a un
  servidor no seguro se rechaza; sin espacio no empieza; y en la app: título «Preparando la voz de Miku
  (una sola vez, ~560 MB)…», progreso que sube hasta el final y se esconde, y el aviso sin internet.
"""

import hashlib
import http.server
import json
import os
import re
import shutil
import sys
import threading
import time
from pathlib import Path

trabajo = Path(sys.argv[1]).resolve()
shutil.rmtree(trabajo, ignore_errors=True)
(trabajo / "web").mkdir(parents=True)
(trabajo / "datos").mkdir()
# Datos y runtime propios de esta prueba (así no se toca lo que haya en el HOME de pruebas).
os.environ["XDG_DATA_HOME"] = str(trabajo / "datos")
os.environ["XDG_RUNTIME_DIR"] = str(trabajo)
os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from miku import descargas  # noqa: E402

# ---- Archivos falsos y su manifiesto -------------------------------------------------------------

ARCHIVOS = {
    "oido/ggml-small-q5_1.bin": os.urandom(900_000),
    "voz/onnx/vector_estimator.onnx": os.urandom(1_400_000),
    "voz/onnx/tts.json": b'{"falso": true}' * 20,
    "voz/voice_styles/F1.json": b'{"estilo": 1}' * 50,
    "voz/LICENSE": b"licencia de mentira\n" * 30,
}
for ruta, datos in ARCHIVOS.items():
    (trabajo / "web" / ruta).parent.mkdir(parents=True, exist_ok=True)
    (trabajo / "web" / ruta).write_bytes(datos)

estado = {"cortar_una_vez": set(), "corromper": set(), "rangos": [], "peticiones": []}


class Manejador(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def do_GET(self):
        ruta = self.path.lstrip("/")
        estado["peticiones"].append(ruta)
        if ruta.startswith("redirige/"):
            self.send_response(302)
            self.send_header("Location", "http://ejemplo.invalido/malo.bin")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        datos = ARCHIVOS.get(ruta)
        if datos is None:
            self.send_error(404)
            return
        if ruta in estado["corromper"]:
            datos = bytes([datos[0] ^ 1]) + datos[1:]
        inicio = 0
        m = re.match(r"bytes=(\d+)-", self.headers.get("Range", ""))
        if m:
            inicio = int(m.group(1))
            estado["rangos"].append((ruta, inicio))
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {inicio}-{len(datos) - 1}/{len(datos)}")
        else:
            self.send_response(200)
        cuerpo = datos[inicio:]
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        if ruta in estado["cortar_una_vez"]:               # se corta la conexión a la mitad
            estado["cortar_una_vez"].discard(ruta)
            self.wfile.write(cuerpo[: len(cuerpo) // 2])
            self.wfile.flush()
            self.connection.close()
            return
        for i in range(0, len(cuerpo), 65536):
            self.wfile.write(cuerpo[i:i + 65536])


servidor = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Manejador)
puerto = servidor.server_address[1]
threading.Thread(target=servidor.serve_forever, daemon=True).start()


def manifiesto(puerto_url: int = puerto) -> dict:
    def entrada(ruta_web: str, ruta_dest: str) -> dict:
        d = ARCHIVOS[ruta_web]
        return {"ruta": ruta_dest, "url": f"http://127.0.0.1:{puerto_url}/{ruta_web}", "bytes": len(d), "sha256": hashlib.sha256(d).hexdigest()}
    return {
        "oido": {"nombre": "el oído", "archivos": [entrada("oido/ggml-small-q5_1.bin", "ggml-small-q5_1.bin")]},
        "voz": {"nombre": "la voz", "archivos": [entrada("voz/onnx/vector_estimator.onnx", "onnx/vector_estimator.onnx"),
                                                   entrada("voz/onnx/tts.json", "onnx/tts.json"),
                                                   entrada("voz/voice_styles/F1.json", "voice_styles/F1.json"),
                                                   entrada("voz/LICENSE", "LICENSE")]},
    }


def poner_manifiesto(m: dict) -> None:
    (trabajo / "manifiesto.json").write_text(json.dumps(m))
    os.environ["VORTEX_MIKU_MANIFIESTO"] = str(trabajo / "manifiesto.json")


def limpiar_datos() -> None:
    shutil.rmtree(trabajo / "datos" / "vortex", ignore_errors=True)


fallos = []


def comprobar(nombre, cond, extra=""):
    print(("  ok  " if cond else "  MAL ") + nombre + (f"  {extra}" if extra else ""), flush=True)
    if not cond:
        fallos.append(nombre)


def restos() -> list[str]:
    return [str(p.relative_to(trabajo)) for p in (trabajo / "datos").rglob("*.parte")]


def main() -> int:
    import types
    descargas.time = types.SimpleNamespace(monotonic=time.monotonic, sleep=lambda s: None)    # los reintentos no esperan (la prueba va rápido)

    print("== Manifiesto real")
    nombres = list(descargas.PAQUETES)
    total = sum(a["bytes"] for p in descargas.PAQUETES.values() for a in p["archivos"])
    comprobar("el de verdad lista el oído y la voz con URL fija a un commit y sha256 de 64 hex",
              nombres == ["oido", "voz"] and all(re.fullmatch(r"[0-9a-f]{64}", a["sha256"]) and re.search(r"/resolve/[0-9a-f]{40}/", a["url"])
                                                  for p in descargas.PAQUETES.values() for a in p["archivos"]))
    comprobar("pesan ~563 MB (lo que se le dice a la persona es ~560 MB)", 555 < total / (1 << 20) < 570, f"{total / (1 << 20):.0f} MB")

    print("== Descarga completa")
    poner_manifiesto(manifiesto())
    limpiar_datos()
    comprobar("sin nada bajado, faltan los dos paquetes", descargas.pendientes() == ["oido", "voz"])
    avances, paquetes_listos = [], []
    d = descargas.Descarga(progreso=lambda t, f: avances.append((t, f)), al_paquete=paquetes_listos.append)
    d.correr()
    comprobar("baja todo y queda en ~/.local/share/vortex/miku/{oido,voz}/",
              descargas.pendientes() == [] and (trabajo / "datos/vortex/miku/voz/onnx/tts.json").is_file()
              and (trabajo / "datos/vortex/miku/oido/ggml-small-q5_1.bin").read_bytes() == ARCHIVOS["oido/ggml-small-q5_1.bin"])
    fracciones = [f for _, f in avances]
    comprobar("el progreso solo sube y termina en 100 %", fracciones == sorted(fracciones) and fracciones[-1] == 1.0 and len(fracciones) >= 2, f"{len(fracciones)} avisos")
    comprobar("avisa cada paquete en orden (el oído primero, para poder hablarle)", paquetes_listos == ["oido", "voz"])
    comprobar("sin restos «.parte»", not restos())
    n = len(estado["peticiones"])
    descargas.Descarga().correr()
    comprobar("si ya está todo no vuelve a bajar nada", len(estado["peticiones"]) == n)

    print("== Se corta la conexión y se reanuda")
    limpiar_datos()
    estado["cortar_una_vez"].add("voz/onnx/vector_estimator.onnx")
    estado["rangos"].clear()
    descargas.Descarga().correr()
    comprobar("reanuda con Range desde donde se quedó (no empieza de cero)",
              any(r[0] == "voz/onnx/vector_estimator.onnx" and r[1] > 100_000 for r in estado["rangos"]), str(estado["rangos"]))
    comprobar("y el archivo final es idéntico (sha256)", descargas.sha256_de(trabajo / "datos/vortex/miku/voz/onnx/vector_estimator.onnx") ==
              hashlib.sha256(ARCHIVOS["voz/onnx/vector_estimator.onnx"]).hexdigest())

    print("== Llega distinto de lo esperado")
    limpiar_datos()
    estado["corromper"].add("oido/ggml-small-q5_1.bin")
    try:
        descargas.Descarga().correr()
        comprobar("rechaza el archivo corrupto", False)
    except descargas.ErrorDescarga as e:
        comprobar("rechaza el archivo corrupto con un mensaje en español", "sha256" in str(e) and "no se instala" in str(e), str(e))
    comprobar("no instala nada del archivo malo ni deja el «.parte»", not (trabajo / "datos/vortex/miku/oido/ggml-small-q5_1.bin").exists() and not restos())
    estado["corromper"].clear()

    print("== Sin red")
    limpiar_datos()
    poner_manifiesto(manifiesto(1))                         # el puerto 1: nadie escucha
    t0 = time.monotonic()
    try:
        descargas.Descarga().correr()
        comprobar("sin red falla", False)
    except descargas.ErrorDescarga as e:
        comprobar("sin red lo dice y que funciona solo con texto", "internet" in str(e) and "solo por texto" in str(e), str(e))
    comprobar("falla rápido (sin esperar minutos)", time.monotonic() - t0 < 10, f"{time.monotonic() - t0:.1f}s")

    print("== Una redirección a un servidor no seguro")
    m = manifiesto()
    m["oido"]["archivos"][0]["url"] = f"http://127.0.0.1:{puerto}/redirige/x"
    poner_manifiesto(m)
    limpiar_datos()
    try:
        descargas.Descarga(["oido"]).correr()
        comprobar("rechaza la redirección", False)
    except descargas.ErrorDescarga as e:
        comprobar("rechaza la redirección a http://ejemplo.invalido", "internet" in str(e), str(e)[:90])
    m["oido"]["archivos"][0]["url"] = "http://servidor-externo.example/x.bin"
    poner_manifiesto(m)
    try:
        descargas.Descarga(["oido"]).correr()
        comprobar("no baja de http a otra máquina", False)
    except descargas.ErrorDescarga as e:
        comprobar("no baja de http a otra máquina", "no es segura" in str(e), str(e))

    print("== Sin espacio en el disco")
    poner_manifiesto(manifiesto())
    limpiar_datos()
    real = shutil.disk_usage
    descargas.shutil.disk_usage = lambda p: type("U", (), {"free": 10_000_000})()
    try:
        descargas.Descarga().correr()
        comprobar("avisa que no hay espacio", False)
    except descargas.ErrorDescarga as e:
        comprobar("avisa que no hay espacio antes de empezar", "No hay espacio" in str(e), str(e)[:80])
    descargas.shutil.disk_usage = real

    print("== En la app (Asistente)")
    from PySide6.QtWidgets import QApplication

    from miku import config
    from miku.asistente import Asistente
    app = QApplication(sys.argv[:1])
    limpiar_datos()
    poner_manifiesto(manifiesto())
    cfg = config.cargar()
    a = Asistente(cfg, False)
    a._arrancar_voz = lambda: None                          # los archivos de voz son de mentira: no se carga ningún motor
    a._escuchar_si_toca = lambda: None                      # y el micrófono no se toca
    prog, avisos = [], []
    a.progreso.connect(lambda t, d_, f: prog.append((t, d_, f)))
    a.aviso.connect(avisos.append)
    a.iniciar()
    fin = time.time() + 20
    while time.time() < fin and not (prog and prog[-1][2] < 0):
        app.processEvents()
        time.sleep(0.02)
    comprobar("muestra «Preparando la voz de Miku (una sola vez, ~560 MB)…»", prog and prog[0][0] == "Preparando la voz de Miku (una sola vez, ~560 MB)…", prog[0][0] if prog else "")
    comprobar("el progreso sube y al final se esconde (fracción -1)", prog[-1][2] < 0 and any(0 < f < 1 for _, _, f in prog) and max(f for _, _, f in prog) == 1.0)
    comprobar("el detalle dice cuánto lleva", any("Bajando" in d_ and " de " in d_ for _, d_, _ in prog), next((d_ for _, d_, _ in prog if "Bajando" in d_), ""))
    comprobar("quedaron bajados el oído y la voz", descargas.pendientes() == [])
    a.cerrar_todo()

    # Sin red en la app: lo dice y sigue por texto.
    limpiar_datos()
    poner_manifiesto(manifiesto(1))
    a = Asistente(cfg, False)
    a._arrancar_voz = lambda: None
    prog, avisos = [], []
    a.progreso.connect(lambda t, d_, f: prog.append((t, d_, f)))
    a.aviso.connect(avisos.append)
    a.iniciar()
    fin = time.time() + 20
    while time.time() < fin and not (prog and prog[-1][2] < 0):
        app.processEvents()
        time.sleep(0.02)
    app.processEvents()
    comprobar("sin red dice que no pudo bajarla y que responde solo por texto",
              any("¿Hay internet?" in x and "solo por texto" in x for x in avisos), "; ".join(avisos)[:140])
    comprobar("la barra se esconde", prog and prog[-1][2] < 0)
    a.cerrar_todo()

    servidor.shutdown()
    print("FALLOS:", len(fallos), fallos if fallos else "")
    return 1 if fallos else 0


if __name__ == "__main__":
    codigo = main()
    sys.stdout.flush()
    os._exit(codigo)
