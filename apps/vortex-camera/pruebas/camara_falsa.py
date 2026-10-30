"""Una cámara de mentiras para las pruebas: un servidor RTSP mínimo que manda la imagen de prueba de
ffmpeg (testsrc, H.264) a quien la pida, con usuario y contraseña (Basic), como una Imou.

    python3 pruebas/camara_falsa.py 8554 admin CODIGO     # y se ve con: ffplay rtsp://admin:CODIGO@127.0.0.1:8554/cam/realmonitor?channel=1&subtype=1

Solo entiende lo que usan ffmpeg y Qt: OPTIONS, DESCRIBE, SETUP (por TCP intercalado; a UDP le dice
461 y el cliente se pasa a TCP), PLAY y TEARDOWN. `rutas` guarda qué pidió cada cliente.
"""
from __future__ import annotations

import base64
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time


class CamaraFalsa:
    def __init__(self, puerto: int = 0, usuario: str = "admin", codigo: str = "L2C3D4E5"):
        self.usuario, self.codigo = usuario, codigo
        self.rutas: list[str] = []
        self._clientes: list[socket.socket] = []
        self._candado = threading.Lock()
        self._servidor = socket.create_server(("127.0.0.1", puerto))
        self.puerto = self._servidor.getsockname()[1]
        self._rtp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._rtp.bind(("127.0.0.1", 0))
        puerto_rtp = self._rtp.getsockname()[1]
        fd, self._sdp_archivo = tempfile.mkstemp(suffix=".sdp")
        os.close(fd)
        self._ffmpeg = subprocess.Popen(
            ["ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin", "-re", "-f", "lavfi",
             "-i", "testsrc=size=640x360:rate=15", "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency",
             "-g", "15", "-pix_fmt", "yuv420p", "-f", "rtp", "-sdp_file", self._sdp_archivo,
             f"rtp://127.0.0.1:{puerto_rtp}?pkt_size=1200"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        limite = time.monotonic() + 10
        while time.monotonic() < limite and not os.path.getsize(self._sdp_archivo):
            time.sleep(0.05)
        with open(self._sdp_archivo, encoding="utf-8") as f:
            self.sdp = self._preparar_sdp(f.read())
        self._vivo = True
        threading.Thread(target=self._repartir, daemon=True).start()
        threading.Thread(target=self._aceptar, daemon=True).start()

    @staticmethod
    def _preparar_sdp(sdp: str) -> str:
        lineas = []
        for linea in sdp.splitlines():
            if linea.startswith("m=video"):
                partes = linea.split()
                partes[1] = "0"
                linea = " ".join(partes)
            if linea.startswith("c="):
                linea = "c=IN IP4 0.0.0.0"
            lineas.append(linea)
            if linea.startswith("m=video"):
                lineas.append("a=control:trackID=0")
        return "\r\n".join(l for l in lineas if l and not l.startswith("SDP:")) + "\r\n"

    def url(self, codigo: str | None = None, ruta: str = "/cam/realmonitor?channel=1&subtype=1") -> str:
        return f"rtsp://{self.usuario}:{self.codigo if codigo is None else codigo}@127.0.0.1:{self.puerto}{ruta}"

    def _repartir(self) -> None:
        while self._vivo:
            try:
                paquete = self._rtp.recv(65535)
            except OSError:
                return
            marco = b"$\x00" + len(paquete).to_bytes(2, "big") + paquete
            with self._candado:
                for c in list(self._clientes):
                    try:
                        c.sendall(marco)
                    except OSError:
                        self._clientes.remove(c)

    def _aceptar(self) -> None:
        while self._vivo:
            try:
                conexion, _ = self._servidor.accept()
            except OSError:
                return
            threading.Thread(target=self._atender, args=(conexion,), daemon=True).start()

    def _atender(self, c: socket.socket) -> None:
        buffer = b""
        esperado = "Basic " + base64.b64encode(f"{self.usuario}:{self.codigo}".encode()).decode()
        try:
            while self._vivo:
                while b"\r\n\r\n" not in buffer:
                    datos = c.recv(4096)
                    if not datos:
                        return
                    buffer += datos
                    # lo intercalado que manda el cliente (RTCP) se tira
                    while buffer.startswith(b"$") and len(buffer) >= 4:
                        n = int.from_bytes(buffer[2:4], "big")
                        if len(buffer) < 4 + n:
                            break
                        buffer = buffer[4 + n:]
                cabeza, buffer = buffer.split(b"\r\n\r\n", 1)
                lineas = cabeza.decode("utf-8", "replace").split("\r\n")
                metodo, ruta, _ = (lineas[0].split(" ") + ["", "", ""])[:3]
                h = {k.strip().lower(): v.strip() for k, _, v in (l.partition(":") for l in lineas[1:])}
                cseq = h.get("cseq", "0")
                if metodo == "OPTIONS":
                    self._responder(c, cseq, "200 OK", {"Public": "OPTIONS, DESCRIBE, SETUP, PLAY, TEARDOWN"})
                    continue
                if h.get("authorization") != esperado:
                    self._responder(c, cseq, "401 Unauthorized", {"WWW-Authenticate": 'Basic realm="Login to camara"'})
                    continue
                if metodo == "DESCRIBE":
                    self.rutas.append(ruta.split("@")[-1] if "://" not in ruta else ruta.split("/", 3)[-1])
                    base = ruta.split("?")[0].rstrip("/") + "/"
                    self._responder(c, cseq, "200 OK", {"Content-Type": "application/sdp", "Content-Base": base}, self.sdp)
                elif metodo == "SETUP":
                    transporte = h.get("transport", "")
                    if "TCP" not in transporte.upper():
                        self._responder(c, cseq, "461 Unsupported Transport", {})
                        continue
                    self._responder(c, cseq, "200 OK", {"Transport": "RTP/AVP/TCP;unicast;interleaved=0-1",
                                                        "Session": "12345678;timeout=60"})
                elif metodo == "PLAY":
                    self._responder(c, cseq, "200 OK", {"Session": "12345678", "Range": "npt=0.000-"})
                    with self._candado:
                        self._clientes.append(c)
                elif metodo == "TEARDOWN":
                    self._responder(c, cseq, "200 OK", {"Session": "12345678"})
                    return
                else:
                    self._responder(c, cseq, "405 Method Not Allowed", {})
        except OSError:
            pass
        finally:
            with self._candado:
                if c in self._clientes:
                    self._clientes.remove(c)
            c.close()

    def _responder(self, c: socket.socket, cseq: str, estado: str, cabeceras: dict, cuerpo: str = "") -> None:
        texto = f"RTSP/1.0 {estado}\r\nCSeq: {cseq}\r\n"
        for k, v in cabeceras.items():
            texto += f"{k}: {v}\r\n"
        if cuerpo:
            texto += f"Content-Length: {len(cuerpo.encode())}\r\n"
        with self._candado:
            c.sendall((texto + "\r\n" + cuerpo).encode())

    def cerrar(self) -> None:
        self._vivo = False
        self._ffmpeg.terminate()
        try:
            self._ffmpeg.wait(3)
        except subprocess.TimeoutExpired:
            self._ffmpeg.kill()
        for s in (self._servidor, self._rtp):
            s.close()
        with self._candado:
            for c in self._clientes:
                c.close()
            self._clientes.clear()
        os.unlink(self._sdp_archivo)


if __name__ == "__main__":
    puerto = int(sys.argv[1]) if len(sys.argv) > 1 else 8554
    cam = CamaraFalsa(puerto, *(sys.argv[2:4]))
    print("Cámara falsa en", cam.url(), flush=True)
    try:
        while True:
            time.sleep(3600)
    except KeyboardInterrupt:
        cam.cerrar()
