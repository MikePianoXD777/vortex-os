"""El asistente: junta el oído, el cerebro y la voz, y le cuenta a la ventana qué pasa.

No dibuja nada: la ventana (ui_avatar.py o ui_panel.py) se conecta a sus señales. El flujo es
  escuchar → transcribir → Claude (en flujo) → cada frase completa a la voz → sonar con la boca moviéndose.
El micrófono solo se abre mientras se escucha, y se escucha solo con la ventana abierta.

La primera vez que se abre baja los modelos de voz y de oído (~560 MB, descargas.py) mostrando el progreso;
sin red lo dice y sigue funcionando por texto. Claude viene apagado en Vortexine's: si no está activado o
no hay sesión, el globo lo dice y ofrece un botón (Activar Claude / Iniciar sesión).
"""

import json
import os
import re
import shutil
import subprocess
import sys
import threading
from pathlib import Path

from PySide6.QtCore import QObject, QProcess, QProcessEnvironment, QTimer, Signal

from . import cerebro as modulo_cerebro
from . import config, descargas, elevenlabs, oido, rutas, voces

# Estados que la ventana muestra con su indicador.
REPOSO, ESCUCHANDO, TRANSCRIBIENDO, PENSANDO, VIENDO, HABLANDO = (
    "reposo", "escuchando", "transcribiendo", "pensando", "viendo", "hablando")

CIERRE_PANEL_SEG = 20          # el panel de solo voz se cierra solo tras tanto tiempo sin que lo toquen
ESPERA_SEGUIMIENTO = 8.0       # tras contestar, cuánto escucha por si sigues hablando
ESPERA_PRIMERA = 10.0          # al abrir, cuánto espera a que empieces a hablar

_RE_TRISTE = re.compile(r"\b(lo siento|perd[oó]n|no puedo|no pude|error|problema|fall[oó]|no s[eé])\b", re.I)
_RE_ALEGRE = re.compile(r"[!¡]|\b(genial|qué bien|me alegra|me encanta|me gusta|claro que s[ií]|listo|de nada)\b", re.I)


_TRISTES = {"sighs", "sad", "exhales", "crying", "softly", "whispers"}
_ALEGRES = {"laughs", "laughs softly", "giggles", "excited", "happy"}
_SORPRESA = {"surprised", "curious", "nervous", "hesitates"}


# Con la voz natural (Qwen3-TTS) la emoción sale de cómo está escrito: se le pide a Claude que se note.
PERSONA_NATURAL = (
    "Tu voz es natural y expresiva, y suena como escribes: deja que se note lo que sientes con interjecciones "
    "(«ay», «mmm», «oye»), puntos suspensivos para las pausas y signos de exclamación o de pregunta cuando toque, "
    "sin exagerar. Si quieres, pon antes de una frase una etiqueta en inglés entre corchetes como [sighs], [sad], "
    "[laughs softly] o [excited]: no se lee en voz alta ni se muestra, solo cambia tu cara."
)


def emocion_de(frase: str) -> tuple[str, float]:
    """Una emoción suave para la frase que va a decir (Miku es apagada: nunca pasa de una sonrisa leve).

    Si la frase trae etiquetas de ElevenLabs ([sighs], [laughs softly]...), mandan ellas.
    """
    etiquetas = set(elevenlabs.etiquetas_de(frase))
    if etiquetas & _ALEGRES:
        return "happy", 0.45
    if etiquetas & _SORPRESA:
        return "surprised", 0.3
    if etiquetas & _TRISTES:
        return "sad", 0.4
    frase = elevenlabs.quitar_etiquetas(frase)
    if _RE_TRISTE.search(frase):
        return "sad", 0.3
    if _RE_ALEGRE.search(frase):
        return "happy", 0.35
    return "relaxed", 0.3


def reproductor() -> list[str] | None:
    """Programa que suena un WAV: pw-play (PipeWire) y, si falta, paplay, aplay o ffplay."""
    for cmd in (["pw-play"], ["paplay"], ["aplay", "-q"], ["ffplay", "-nodisp", "-autoexit", "-loglevel", "quiet"]):
        if shutil.which(cmd[0]):
            return cmd
    return None


class Asistente(QObject):
    estado = Signal(str, str)           # (estado, texto corto para mostrar)
    usuario = Signal(str)               # lo que dijiste (o escribiste)
    respuesta = Signal(str, bool)       # texto de Miku hasta ahora, ¿ya terminó?
    aviso = Signal(str)                 # mensaje para la persona (qué falta, qué hacer)
    nivel = Signal(float)               # volumen del micrófono mientras escucha, 0..1
    boca = Signal(object, int)          # (pista de visemas [[aa, ih, ou, ee, oh], ...], cuadros por segundo)
    boca_fin = Signal()
    emocion = Signal(str, float)
    cerrar = Signal()                   # pide a la ventana que se cierre (el panel se cierra solo)
    progreso = Signal(str, str, float)  # (título, detalle, 0..1) de la descarga de los modelos; fracción < 0 = ya no hay
    acciones = Signal(object)           # botones bajo la respuesta: [[id, texto], ...] (vacía = ninguno)

    # Hilos → hilo principal.
    _texto_oido = Signal(object)        # str, "" si no se entendió, None si nadie habló
    _error_oido = Signal(str)
    _evento_cerebro = Signal(str, object)
    _sig_progreso = Signal(str, float)
    _sig_paquete = Signal(str)
    _sig_descarga_fin = Signal(str)

    def __init__(self, cfg: dict, avatar: bool, parent=None):
        super().__init__(parent)
        self.cfg = cfg
        self.avatar = avatar
        self.estado_actual = REPOSO
        self.oido = oido.Oido(silencio=cfg["silencio"], espera=ESPERA_PRIMERA)
        self.cerebro = modulo_cerebro.Cerebro(avatar, lambda tipo, **d: self._evento_cerebro.emit(tipo, d))
        motor = (voces.buscar(cfg["voz"]) or {}).get("motor")
        if motor == "elevenlabs":
            self.cerebro.persona += "\n\n" + elevenlabs.PERSONA
        elif motor == "qwen":
            self.cerebro.persona += "\n\n" + PERSONA_NATURAL
        self.divisor = modulo_cerebro.DivisorFrases()
        self._acumulado = ""
        self._cerebro_listo = True       # ¿terminó Claude el turno actual?
        self._turno = 0                  # sube cada vez que se calla a Miku; descarta el audio de un turno viejo
        self._pregunta_id = 0            # sube con cada pregunta a Claude; descarta lo que llegue de una vieja
        self._ultimo_turno_con_error = False
        self._hilo_oido: threading.Thread | None = None
        # Voz
        self._voz: QProcess | None = None
        self._voz_lista = False
        self._voz_error = False
        self._voz_buf = b""
        self._pedidos: dict[int, tuple[str, int]] = {}      # id de frase → (texto, turno)
        self._siguiente_id = 0
        self._cola: list[tuple[str, float, list, str, int]] = []   # (wav, seg, visemas, texto, turno) por sonar
        self._sonando: QProcess | None = None
        self._ruta_sonando: str | None = None
        self._avisos_dados: set[str] = set()
        self._cerrado = False
        self._descargando = False
        self._cancelar_descarga = threading.Event()
        self._titulo_descarga = ""
        self._claude_ok = False          # ¿Claude está activado, instalado y con sesión? (se comprueba al abrir)
        self.acciones_visibles = False   # hay un botón (Activar Claude / Iniciar sesión) esperando a la persona
        # Cierre automático del panel de solo voz
        self._reloj_ocioso = QTimer(self)
        self._reloj_ocioso.setSingleShot(True)
        self._reloj_ocioso.setInterval(CIERRE_PANEL_SEG * 1000)
        self._reloj_ocioso.timeout.connect(self._por_ocio)
        self._reloj_seguir = QTimer(self)
        self._reloj_seguir.setSingleShot(True)
        self._reloj_seguir.timeout.connect(lambda: self.escuchar(seguimiento=True))
        self._texto_oido.connect(self._al_texto_oido)
        self._error_oido.connect(self._al_error_oido)
        self._evento_cerebro.connect(self._al_evento_cerebro)
        self._sig_progreso.connect(self._al_progreso)
        self._sig_paquete.connect(self._al_paquete)
        self._sig_descarga_fin.connect(self._al_fin_descarga)

    # ---- Arranque ---------------------------------------------------------------------------

    def iniciar(self) -> None:
        """Al abrir: baja lo que falte (la primera vez), levanta la voz y se pone a escuchar enseguida."""
        if descargas.pendientes():
            self._iniciar_descarga()
        else:
            self._arrancar_voz()
        threading.Thread(target=self._comprobar_claude, daemon=True).start()

    def _comprobar_claude(self) -> None:
        """Antes de ponerse a escuchar mira si Claude está listo; si no, lo dice con su botón."""
        clave, mensaje = self.cerebro.comprobar()
        self._evento_cerebro.emit("error" if clave != "ok" else "claude_listo", {"clave": clave, "mensaje": mensaje})

    # ---- Primera vez: bajar los modelos de voz y de oído ------------------------------------------

    def _iniciar_descarga(self) -> None:
        if self._descargando or self._cerrado:
            return
        nombres = descargas.pendientes()
        if not nombres:
            return
        total = descargas.bytes_pendientes(nombres)
        tamano = descargas.TAMANO_TOTAL_TEXTO if len(nombres) == len(descargas.paquetes()) else f"~{descargas.megas(total)}"
        self._titulo_descarga = f"Preparando la voz de Miku (una sola vez, {tamano})…"
        self._descargando = True
        self._cancelar_descarga = threading.Event()
        self.progreso.emit(self._titulo_descarga, "Empezando…", 0.0)
        cancelar = self._cancelar_descarga

        def trabajo():
            d = descargas.Descarga(nombres, progreso=self._sig_progreso.emit, cancelar=cancelar, al_paquete=self._sig_paquete.emit)
            try:
                d.correr()
                self._sig_descarga_fin.emit("")
            except descargas.ErrorDescarga as e:
                self._sig_descarga_fin.emit(str(e))
            except Exception as e:  # noqa: BLE001 - el hilo no debe morir sin avisar
                self._sig_descarga_fin.emit(f"No pude preparar la voz de Miku ({type(e).__name__}). Te respondo solo por texto.")
        threading.Thread(target=trabajo, daemon=True).start()

    def _al_progreso(self, detalle: str, fraccion: float) -> None:
        if self._descargando and not self._cerrado:
            self.progreso.emit(self._titulo_descarga, detalle, fraccion)

    def _al_paquete(self, paquete: str) -> None:
        """Un paquete (el oído, o la voz) terminó de bajarse: ya se puede usar sin esperar al otro."""
        if self._cerrado:
            return
        if paquete == "voz":
            self._voz_error = False
            self._arrancar_voz()
        elif paquete == "oido":
            self._escuchar_si_toca()

    def _al_fin_descarga(self, error: str) -> None:
        self._descargando = False
        self.progreso.emit("", "", -1.0)
        if self._cerrado:
            return
        if error:
            self._voz_error = not voces.modelo_listo()
            self._avisar_una_vez("descarga", error)
            if self.estado_actual == REPOSO:
                self._iniciar_reloj_ocioso()
        elif self.estado_actual == REPOSO and not self.acciones_visibles:
            self._escuchar_si_toca()

    def _escuchar_si_toca(self) -> None:
        """Se pone a escuchar si no está haciendo otra cosa ni hay un problema con Claude pendiente."""
        if self.estado_actual == REPOSO and self._cerebro_listo and not self.acciones_visibles and self._claude_ok:
            self.escuchar()

    # ---- Voz (proceso aparte con Supertonic 3) --------------------------------------------------

    def _arrancar_voz(self, id_voz: str | None = None) -> None:
        id_voz = id_voz or self.cfg["voz"]
        voz = voces.buscar(id_voz)
        if voz is None:
            self._sin_voz(f"No existe la voz «{id_voz}». Mira las que hay con: vortex-miku voz")
            return
        if not voces.disponible(voz):
            if voz.get("motor") in ("qwen", "elevenlabs") and id_voz != voces.VOZ_DEFECTO:
                self._avisar_una_vez("voz-natural", f"La voz «{voz['nombre']}» no está lista; hablo con la de siempre.")
                self._arrancar_voz(voces.VOZ_DEFECTO)
                return
            if self._descargando:                  # se está bajando: arranca sola al terminar
                self._voz_error = True
                return
            self._sin_voz(f"La voz «{id_voz}» no está instalada. Instálala con: vortex-miku voz {id_voz}")
            return
        p = QProcess(self)
        env = QProcessEnvironment.systemEnvironment()
        if voz.get("motor") == "qwen":
            # La voz natural corre con el Python de su entorno (PyTorch y Qwen3-TTS no están en el sistema).
            script = Path(__file__).resolve().parent / "voz_natural.py"
            env.insert("HF_HUB_OFFLINE", "1")
            p.setProgram(str(rutas.NATURAL_PYTHON))
            p.setArguments(["-I", str(script), "--voz", id_voz])
        else:
            script = Path(__file__).resolve().parent / "voz_worker.py"
            p.setProgram(sys.executable)
            p.setArguments(["-I", str(script), "--voz", id_voz, "--estilo", "miku" if self.avatar else "normal"])
        p.setProcessEnvironment(env)
        p.readyReadStandardOutput.connect(self._leer_voz)
        p.finished.connect(self._voz_termino)
        p.start()
        self._voz = p
        self._voz_id = id_voz

    def _sin_voz(self, motivo: str) -> None:
        self._voz_error = True
        self._avisar_una_vez("voz", motivo + " Te respondo solo por texto.")

    def _avisar_una_vez(self, clave: str, texto: str) -> None:
        if clave not in self._avisos_dados:
            self._avisos_dados.add(clave)
            self.aviso.emit(texto)

    def _leer_voz(self) -> None:
        self._voz_buf += bytes(self._voz.readAllStandardOutput())
        while b"\n" in self._voz_buf:
            linea, self._voz_buf = self._voz_buf.split(b"\n", 1)
            try:
                m = json.loads(linea)
            except ValueError:
                continue
            if m.get("listo"):
                self._voz_lista = True
            elif "wav" in m:
                texto, turno = self._pedidos.pop(m.get("id"), ("", -1))      # -1: pedido descartado, no suena
                if m.get("aviso"):
                    self._avisar_una_vez("voz-respaldo", m["aviso"])
                self._cola.append((m["wav"], float(m.get("seg", 0)), m.get("visemas") or [], texto, turno))
                self._sonar_siguiente()
            elif m.get("error") and m.get("id") is None:
                if getattr(self, "_voz_id", voces.VOZ_DEFECTO) != voces.VOZ_DEFECTO:
                    self._cambiar_a_respaldo(f"No pude cargar la voz natural ({m['error']}).")
                    return
                self._sin_voz(f"No pude cargar la voz ({m['error']}).")
            elif m.get("error"):
                self._pedidos.pop(m.get("id"), None)
                self._avisar_una_vez("voz-frase", f"La voz falló con una frase ({m['error']}).")
                self._revisar_fin_de_turno()

    def _cambiar_a_respaldo(self, motivo: str) -> None:
        """La voz elegida no arrancó: se sigue con la de siempre (Supertonic F1) sin perder lo pendiente."""
        viejo, self._voz = self._voz, None
        if viejo is not None:
            viejo.finished.disconnect(self._voz_termino)
            viejo.kill()
            viejo.deleteLater()
        self._voz_buf = b""
        self._voz_lista = False
        self._avisar_una_vez("voz-natural", motivo + " Hablo con la voz de siempre.")
        pendientes = sorted(self._pedidos.items())
        self._pedidos.clear()
        self._arrancar_voz(voces.VOZ_DEFECTO)
        for _n, (frase, turno) in pendientes:
            if turno == self._turno:
                self._decir(frase)

    def _voz_termino(self, *_args) -> None:
        if not self._cerrado and not self._voz_error and getattr(self, "_voz_id", voces.VOZ_DEFECTO) != voces.VOZ_DEFECTO \
                and not self._voz_lista:
            self._cambiar_a_respaldo("La voz natural se cerró al arrancar.")
            return
        if not self._cerrado and not self._voz_error:
            self._sin_voz("La voz se detuvo.")
            self._pedidos.clear()
            self._revisar_fin_de_turno()

    def _decir(self, frase: str) -> None:
        """Manda una frase a la voz (si hay voz). Suena cuando le toque."""
        frase = frase.strip()
        if not frase or self._voz_error or self._voz is None:
            return
        self._siguiente_id += 1
        self._pedidos[self._siguiente_id] = (frase, self._turno)
        pedido = json.dumps({"id": self._siguiente_id, "texto": frase}, ensure_ascii=False) + "\n"
        self._voz.write(pedido.encode("utf-8"))

    def _sonar_siguiente(self) -> None:
        if self._sonando is not None or self._cerrado:
            return
        while self._cola:
            wav, seg, vis, texto, turno = self._cola.pop(0)
            if turno != self._turno:                    # de una pregunta vieja (la interrumpiste)
                self._borrar(wav)
                continue
            self._reproducir(wav, seg, vis, texto)
            return
        self._revisar_fin_de_turno()

    def _reproducir(self, wav: str, seg: float, vis: list, texto: str) -> None:
        self._cambiar_estado(HABLANDO, "")
        self._reloj_seguir.stop()
        self.emocion.emit(*emocion_de(texto))
        self.boca.emit(vis, 30)
        salida_prueba = os.environ.get("VORTEX_MIKU_SALIDA")
        if salida_prueba:
            # Pruebas: no suena nada; el WAV se guarda en esa carpeta y se espera lo que duraría.
            os.makedirs(salida_prueba, exist_ok=True)
            destino = os.path.join(salida_prueba, os.path.basename(wav))
            shutil.copyfile(wav, destino)
            self._ruta_sonando = wav
            self._sonando = QProcess(self)          # marcador: hay algo "sonando"
            QTimer.singleShot(int(seg * 1000 / float(os.environ.get("VORTEX_MIKU_SALIDA_VELOCIDAD", "1"))), self._termino_de_sonar)
            return
        cmd = reproductor()
        if cmd is None:
            self._avisar_una_vez("audio", "No encuentro con qué reproducir sonido (pw-play). Te respondo solo por texto.")
            self._borrar(wav)
            self._termino_de_sonar()
            return
        p = QProcess(self)
        self._sonando = p
        self._ruta_sonando = wav
        p.finished.connect(lambda *_: self._termino_de_sonar())
        p.errorOccurred.connect(lambda *_: self._termino_de_sonar())
        p.start(cmd[0], cmd[1:] + [wav])

    def _termino_de_sonar(self) -> None:
        p, self._sonando = self._sonando, None
        if p is None:
            return
        p.deleteLater()
        if self._ruta_sonando:
            self._borrar(self._ruta_sonando)
            self._ruta_sonando = None
        self.boca_fin.emit()
        self._sonar_siguiente()

    def _borrar(self, ruta: str) -> None:
        try:
            os.unlink(ruta)
        except OSError:
            pass

    def parar_de_hablar(self) -> None:
        """Calla a Miku: descarta lo que falta por sonar y corta lo que suena."""
        self._turno += 1
        self._pedidos.clear()
        for wav, *_ in self._cola:
            self._borrar(wav)
        self._cola.clear()
        p = self._sonando
        if p is not None:
            if p.state() != QProcess.ProcessState.NotRunning:
                p.finished.disconnect()
                p.errorOccurred.disconnect()
                p.kill()
            self._termino_de_sonar()
        self.boca_fin.emit()

    # ---- Oído -----------------------------------------------------------------------------------

    def escuchar(self, seguimiento: bool = False) -> None:
        """Abre el micrófono hasta que termines de hablar. Si ya estaba escuchando, lo corta y transcribe."""
        if self._cerrado:
            return
        if self._hilo_oido is not None and self._hilo_oido.is_alive():
            self.oido.cortar_ahora()          # clic en el micrófono con la escucha abierta = «ya acabé»
            return
        if not descargas.paquete_listo("oido"):
            # Sin el modelo del oído no se abre el micrófono. Si no se está bajando (falló por falta de red),
            # tocar el micrófono lo intenta otra vez.
            if not self._descargando:
                self._iniciar_descarga()
            if not self._descargando:
                self._avisar_una_vez("oido", "Todavía no tengo el oído: se baja la primera vez con internet. Mientras tanto puedes escribirme.")
            return
        self._reloj_ocioso.stop()
        self._reloj_seguir.stop()
        if not seguimiento:
            self.parar_de_hablar()
            self._cancelar_cerebro()
        self.oido.espera = ESPERA_SEGUIMIENTO if seguimiento else ESPERA_PRIMERA
        self._cambiar_estado(ESCUCHANDO, "Escuchando...")

        def trabajo():
            try:
                pcm = self.oido.escuchar(al_nivel=self.nivel.emit, al_hablar=None)
                if pcm is None:
                    self._texto_oido.emit(None)
                    return
                self._texto_oido.emit("__transcribiendo__")
                self._texto_oido.emit(oido.transcribir(pcm))
            except oido.ErrorOido as e:
                self._error_oido.emit(str(e))
            except Exception as e:  # noqa: BLE001 - el hilo no debe morir sin avisar
                self._error_oido.emit(f"El micrófono falló ({type(e).__name__}).")
        self._hilo_oido = threading.Thread(target=trabajo, daemon=True)
        self._hilo_oido.start()

    def _al_texto_oido(self, texto) -> None:
        if self._cerrado:
            return
        if texto == "__transcribiendo__":
            if self.estado_actual == ESCUCHANDO:
                self._cambiar_estado(TRANSCRIBIENDO, "Un momento...")
            return
        if self.estado_actual not in (ESCUCHANDO, TRANSCRIBIENDO):
            return                                  # llegó tarde: ya le escribiste o te interrumpiste
        if self._ultimo_turno_con_error:
            self._cambiar_estado(REPOSO, "")
            self._iniciar_reloj_ocioso()
            return
        if not texto:
            self._cambiar_estado(REPOSO, "No te escuché. Toca el micrófono o escríbeme." if texto is None else
                                 "No te entendí. Toca el micrófono o escríbeme.")
            self._iniciar_reloj_ocioso()
            return
        self.enviar(texto)

    def _al_error_oido(self, mensaje: str) -> None:
        self._cambiar_estado(REPOSO, "")
        self.aviso.emit(mensaje)
        self._iniciar_reloj_ocioso()

    def cortar_escucha(self) -> None:
        """Termina la escucha ya (clic en el micrófono mientras escucha)."""
        self.oido.cortar_ahora()

    # ---- Cerebro --------------------------------------------------------------------------------

    def enviar(self, texto: str) -> None:
        """Una pregunta (hablada o escrita) a Claude. Interrumpe lo que Miku estuviera diciendo."""
        texto = texto.strip()
        if not texto or self._cerrado:
            return
        if self._hilo_oido is not None and self._hilo_oido.is_alive():
            self.oido.cancelar()
        self._reloj_ocioso.stop()
        self._reloj_seguir.stop()
        self._poner_acciones([])
        self.parar_de_hablar()
        self._cancelar_cerebro()
        self.divisor = modulo_cerebro.DivisorFrases()
        self._acumulado = ""
        self._cerebro_listo = False
        self._ultimo_turno_con_error = False
        self.usuario.emit(texto)
        self.respuesta.emit("", False)
        self._cambiar_estado(PENSANDO, "Pensando...")
        self._pregunta_id += 1
        threading.Thread(target=self.cerebro.preguntar, args=(texto, self._pregunta_id), daemon=True).start()

    def _cancelar_cerebro(self) -> None:
        self.cerebro.cancelar()
        self._pregunta_id += 1           # lo que Claude alcance a decir de la pregunta cancelada se ignora
        self._cerebro_listo = True

    def _al_evento_cerebro(self, tipo: str, datos: dict) -> None:
        if self._cerrado or datos.get("etiqueta", self._pregunta_id) != self._pregunta_id:
            return
        if tipo == "texto":
            self._acumulado += datos["delta"]
            self.respuesta.emit(elevenlabs.quitar_etiquetas(self._acumulado), False)
            for frase in self.divisor.alimentar(datos["delta"]):
                self._decir(frase)
        elif tipo in ("bloque_fin", "herramienta"):
            if self._acumulado and not self._acumulado.endswith((" ", "\n")):
                self._acumulado += " "            # entre un bloque de texto y el siguiente («Déjame ver.» y lo que ve)
            resto = self.divisor.vaciar()
            if resto:
                self._decir(resto)
            if tipo == "herramienta":
                self._cambiar_estado(VIENDO, "Mirando tu pantalla...")
        elif tipo == "listo":
            resto = self.divisor.vaciar()
            if resto:
                self._decir(resto)
            self._cerebro_listo = True
            self.respuesta.emit(elevenlabs.quitar_etiquetas(self._acumulado or datos.get("texto", "")), True)
            self._revisar_fin_de_turno()
        elif tipo == "claude_listo":
            self._claude_ok = True
            if self.estado_actual == REPOSO and descargas.paquete_listo("oido"):
                self._escuchar_si_toca()
        elif tipo == "error":
            self._fallo(datos.get("clave", "otro"), datos["mensaje"])

    BOTONES = {"activar": [["activar", "Activar Claude"]], "sesion": [["sesion", "Iniciar sesión"]]}

    def _fallo(self, clave: str, mensaje: str) -> None:
        """Muestra el problema en el globo y lo dice en voz alta (en español, con qué hacer)."""
        self._cerebro_listo = True
        self._ultimo_turno_con_error = True
        self._claude_ok = False if clave in self.BOTONES else self._claude_ok
        self.parar_de_hablar()
        self.oido.cancelar()
        self.respuesta.emit(mensaje, True)
        self._poner_acciones(self.BOTONES.get(clave, []))
        self._decir(mensaje)
        self._cambiar_estado(REPOSO, "")
        self._revisar_fin_de_turno()

    def _poner_acciones(self, botones: list) -> None:
        self.acciones_visibles = bool(botones)
        self.acciones.emit(botones)

    def accion(self, id_accion: str) -> None:
        """Un botón del globo. Activar Claude: lo instala y abre su terminal para entrar con tu cuenta. Iniciar sesión: abre esa terminal."""
        vortex = modulo_cerebro.encontrar_vortex()
        if not vortex:
            self.aviso.emit("No encuentro el comando vortex. Abre una terminal y escribe: vortex claude")
            return
        if id_accion == "activar":
            # Aparte de Miku (setsid -f): la terminal de Claude tiene que seguir abierta aunque cierres a Miku.
            orden = ["setsid", "-f", "bash", "-c", '"$1" claude activar && exec "$1" claude', "vortex-miku", vortex]
            texto = "Activando Claude. Se abrirá una terminal para que inicies sesión con tu cuenta; cuando termines, vuelve a hablarme."
        elif id_accion == "sesion":
            orden = ["setsid", "-f", vortex, "claude"]
            texto = "Inicia sesión con tu cuenta en la terminal que se abrió; cuando termines, vuelve a hablarme."
        else:
            return
        try:
            subprocess.Popen(orden, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True)
        except OSError as e:
            self.aviso.emit(f"No pude abrir Claude ({e.strerror or e}). Abre una terminal y escribe: vortex claude")
            return
        self._poner_acciones([])
        self._ultimo_turno_con_error = False
        self.aviso.emit(texto)

    # ---- Fin de turno ---------------------------------------------------------------------------

    def _revisar_fin_de_turno(self) -> None:
        """Cuando Claude terminó y la voz ya no tiene nada pendiente, vuelve al reposo o a escuchar."""
        if self._cerrado or not self._cerebro_listo or self._sonando is not None or self._cola or self._pedidos:
            return
        if self.estado_actual in (ESCUCHANDO, TRANSCRIBIENDO):
            return
        if self.estado_actual != REPOSO:
            self._cambiar_estado(REPOSO, "")
        if self.cfg.get("seguir_escuchando") and not self._ultimo_turno_con_error and self._acumulado:
            self._reloj_seguir.start(500)             # medio segundo, para no oírse a sí misma
        else:
            self._iniciar_reloj_ocioso()

    def _iniciar_reloj_ocioso(self) -> None:
        if not self.avatar:
            self._reloj_ocioso.start()

    def tocada(self) -> None:
        """La persona tocó la ventana: el panel de solo voz no se cierra solo todavía."""
        if self._reloj_ocioso.isActive():
            self._reloj_ocioso.start()

    def _por_ocio(self) -> None:
        if self.estado_actual == REPOSO and self._sonando is None and not self._cola and not self._descargando:
            self.cerrar.emit()
        else:
            self._reloj_ocioso.start()

    def _cambiar_estado(self, estado: str, texto: str) -> None:
        self.estado_actual = estado
        self.estado.emit(estado, texto)

    # ---- Cierre ---------------------------------------------------------------------------------

    def cerrar_todo(self) -> None:
        """Cierra la voz, el oído y Claude, borra los audios temporales y la conversación."""
        if self._cerrado:
            return
        self._cerrado = True
        self._cancelar_descarga.set()
        self._reloj_ocioso.stop()
        self._reloj_seguir.stop()
        self.oido.cancelar()
        self.cerebro.olvidar()
        if self._sonando is not None and self._sonando.state() != QProcess.ProcessState.NotRunning:
            self._sonando.kill()
        if self._voz is not None and self._voz.state() != QProcess.ProcessState.NotRunning:
            self._voz.write(b'{"cmd": "salir"}\n')
            if not self._voz.waitForFinished(1500):
                self._voz.terminate()
                if not self._voz.waitForFinished(1500):
                    self._voz.kill()
        for wav, *_ in self._cola:
            self._borrar(wav)
        if self._ruta_sonando:
            self._borrar(self._ruta_sonando)
        for f in rutas.temporales().glob("voz-*.wav"):
            self._borrar(str(f))
