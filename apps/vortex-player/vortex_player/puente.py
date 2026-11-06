"""El puente entre el motor y la interfaz (QML): lo que la ventana lee y los botones que llama.

Leer las carpetas y bajar canciones corren en hilos aparte; sus avisos vuelven al hilo de Qt por
señales. La base SQLite se comparte entre hilos con un candado.
"""
from __future__ import annotations

import hashlib
import json
import os
import random
import re
import shutil
import subprocess
import threading
import time
from pathlib import Path
from urllib.parse import quote

from PySide6.QtCore import Property, QFileSystemWatcher, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QColor, QGuiApplication, QImage

from . import audio, cuenta_google, descargas, ecualizador, letras, plataforma, rutas, sincronia
from .avance import Avance
from . import biblioteca
from .biblioteca import Biblioteca, id_de
from .reproductor import Reproductor

NOMBRES_PORTADA = ("cover.jpg", "cover.png", "folder.jpg", "folder.png", "front.jpg", "Cover.jpg", "Folder.jpg", "AlbumArt.jpg")
NAVEGADORES = ["", "firefox", "chrome", "chromium", "brave", "edge", "opera", "vivaldi", "zen"]


def ruta_de(texto: str) -> str:
    """Ruta local desde lo que llega de QML o del explorador (file:///…)."""
    if texto.startswith("file:"):
        return QUrl(texto).toLocalFile()
    return texto


class Puente(QObject):
    cambioCanciones = Signal()
    cambioPlaylists = Signal()
    cambioDescargas = Signal()
    cambioEq = Signal()
    cambioSalida = Signal()
    cambioAjustes = Signal()
    cambioEscaneo = Signal()
    cambioEnLinea = Signal()
    cambioRecomendaciones = Signal()
    aviso = Signal(str)
    mostrarVentana = Signal()
    portadaLista = Signal(str)
    _evento = Signal(int, "QVariantMap")         # de los hilos de descarga al hilo de Qt
    _escaneado = Signal("QVariantMap")
    _resultados = Signal(int, "QVariantList", str)   # de la búsqueda en YouTube (hilo) al hilo de Qt
    _recomendadas = Signal(int, int, "QVariantList", str)   # (vuelta, playlist, canciones, error)
    cambioNube = Signal()
    _nube_fin = Signal("QVariantMap")                       # del hilo de la nube al hilo de Qt
    _quitar_de_cola = Signal(str)
    cambioLetra = Signal()
    _letra_lista = Signal(int, "QVariantMap")               # del hilo de las letras al hilo de Qt
    cambioParaTi = Signal()
    _para_ti_listo = Signal(int, "QVariantList", "QStringList", str)
    cambioLetras = Signal()
    _letras_avance = Signal("QVariantMap")                  # del hilo de las letras sin internet
    _radio_lista = Signal(int, "QVariantList", str)        # 1.4: del hilo de la radio sin fin
    cambioResumen = Signal()
    _enlace_listo = Signal(str, str, str)                  # 1.5: (qué hacer, enlace, texto) del hilo de compartir
    cambioMezcla = Signal()
    _mezcla_lista = Signal("QVariantMap")
    cambioSeguir = Signal()
    _otro_aparato = Signal("QVariantMap")
    cambioNovedades = Signal()
    _novedades_listas = Signal("QVariantList", "QVariantMap")
    _lanzamiento_listo = Signal(str, str, str, "QVariantList")   # (qué hacer, id, playlist OLAK, canciones)

    def __init__(self, parent: QObject | None = None, biblioteca: Biblioteca | None = None,
                 reproductor: Reproductor | None = None):
        super().__init__(parent)
        self.bib = biblioteca or Biblioteca()
        self.candado = threading.Lock()
        self.r = reproductor or Reproductor(self)
        self._buscar = ""
        self._orden = "agregada"
        self._vista_playlist = -1
        self._canciones: list[dict] = []
        self._playlists: list[dict] = []
        self._descargas: list[dict] = []
        self._eq = ecualizador.estado()
        self._eq_local = 0.0   # cuándo se movió una banda aquí (time.monotonic)
        self._salida: dict = {}
        self._escaneando = False
        self._portadas: dict[str, str] = {}
        self._parar_descargas = False
        self._cola_urls: list[tuple[int, str]] = []
        self._hilo_descargas: threading.Thread | None = None
        # 2.1: buscar en YouTube Music / YouTube y bajar a la biblioteca
        self._en_linea: list[dict] = []
        self._buscando_linea = False
        self._error_linea = ""
        self._busqueda_linea = 0            # la última búsqueda: las respuestas viejas se ignoran
        self._ids_biblioteca: set[str] = set()
        # 2.1: recomendaciones para la playlist que estás viendo (se guardan mientras la app está abierta)
        self._recom: list[dict] = []
        self._recom_para = -1
        self._recom_cargando = False
        self._recom_error = ""
        self._recom_vuelta = 0
        self._recom_guardadas: dict[int, list[dict]] = {}
        # 2.1.1: escuchar 30 s de una recomendada o de un resultado antes de bajarla
        self._avance = Avance(self)
        self._seguir_tras_avance = False
        self._avance.empezo.connect(self._pausar_por_avance)
        self._avance.termino.connect(self._seguir_tras_el_avance)
        self._avance.error.connect(self.aviso)
        self.r.quiereSonar.connect(self._musica_quiere_sonar)

        # 2.4: tu biblioteca en la nube (Google Drive), la misma que en el teléfono
        s = cuenta_google.sesion()
        self._nube = {"correo": s.get("correo", ""), "nombre": s.get("nombre", ""), "trabajando": False,
                      "texto": "", "ultima": 0.0, "error": ""}
        self._nube_pendientes: set[str] = set()   # ids que mandó bajar la sincronización
        self._nube_fallidas: set[str] = set()     # no se reintentan hasta volver a abrir la app
        self._nube_otra_vez = False
        self._reloj_nube = QTimer(self, interval=10 * 60 * 1000)
        self._reloj_nube.timeout.connect(self.sincronizar)
        self._reloj_nube.start()
        self._espera_nube = QTimer(self, singleShot=True, interval=15000)   # un rato después de cambiar algo
        self._espera_nube.timeout.connect(self.sincronizar)
        self._nube_fin.connect(self._fin_nube)
        self._quitar_de_cola.connect(self.r.quitarDeCola)

        self._evento.connect(self._en_evento)
        self._escaneado.connect(self._fin_escaneo)
        self._resultados.connect(self._fin_busqueda_linea)
        self._recomendadas.connect(self._fin_recomendar)
        self.cambioDescargas.connect(self.cambioEnLinea)
        self.cambioEnLinea.connect(self.cambioRecomendaciones)
        self.r.ponerVolumen(int(self.bib.ajustes().get("volumen", 80)))

        # El ecualizador es el del panel del escritorio: si cambia allá, cambia aquí
        self._vigia_eq = QFileSystemWatcher(self)
        self._vigia_eq.fileChanged.connect(self._eq_cambio)
        self._vigia_eq.directoryChanged.connect(self._eq_cambio)
        self._vigilar_eq()
        self._eq_a_mpv()

        # Por dónde suena (y con qué códec) cada 3 s: pactl tarda ~10 ms
        self._reloj_salida = QTimer(self, interval=3000)
        self._reloj_salida.timeout.connect(self.actualizarSalida)
        if not plataforma.WINDOWS:   # pactl y PipeWire son de Linux
            self._reloj_salida.start()
            self.actualizarSalida()

        # 2.4: seguir donde te quedaste, la letra de lo que suena y «Para ti» (de lo último que escuchaste)
        self._sesion = self._leer_sesion()
        self._restaurada = False
        self._letra: dict = {"ruta": "", "lineas": [], "sincronizada": False, "buscando": False}
        self._letra_vuelta = 0
        self._para_ti: list[dict] = []
        self._para_ti_de: list[str] = []
        self._para_ti_cargando = False
        self._para_ti_error = ""
        self._para_ti_vuelta = 0
        self._colores: dict[str, str] = {}
        self._letra_lista.connect(self._fin_letra)
        self._para_ti_listo.connect(self._fin_para_ti)
        self.r.cambioCancion.connect(self._cambio_cancion)
        self.r.cambioEstado.connect(self._guardar_sesion)
        self._reloj_sesion = QTimer(self, interval=5000)
        self._reloj_sesion.timeout.connect(lambda: self.r.pausado or self._guardar_sesion())
        self._reloj_sesion.start()
        self.cambioDescargas.connect(self.cambioParaTi)

        # 2.4: ❤️, crossfade y las letras de toda tu música guardadas para verlas sin internet
        self._me_gusta: list[str] = []
        self.r.ponerFundido(float(self.bib.ajustes().get("fundido", 5)))
        self._letras_estado = {"con": 0, "total": 0, "trabajando": False}
        self._letras_avance.connect(self._en_letras)
        self._espera_letras = QTimer(self, singleShot=True, interval=20000)
        self._espera_letras.timeout.connect(self.bajarLetras)

        # 1.4: la radio sin fin (al acabarse la cola siguen parecidas, sin bajarlas)
        self._radio_vuelta = 0
        self._bajando_radio: list[str] = []   # las de la radio que mandaste bajar
        self._radio_para = ""   # la última de la cola cuando se pidió (si cambia la cola, la respuesta no sirve)
        self._radio_lista.connect(self._fin_radio)
        self.r.seAcaba.connect(self._pedir_radio)
        # 1.4: «Tu resumen»: cada canción que de verdad escuchaste (30 s o más) cuenta
        self._resumen: dict = {}
        self.r.escuchada.connect(self._anotar_escucha)
        self._reloj_resumen = QTimer(self, interval=60 * 60 * 1000)   # por si la semana cambia con la app abierta
        self._reloj_resumen.timeout.connect(self.cambioResumen)
        self._reloj_resumen.start()
        self._enlace_listo.connect(self._fin_enlace)

        # 1.5: tu mezcla del día, seguir en el otro aparato, lo nuevo de tus artistas, la radio con datos
        # móviles y el despertador
        self._mezcla = self._leer_json("mezcla.json")
        self._mezcla_armando = False
        self._mezcla_lista.connect(self._fin_mezcla)
        self._seguir: dict = {}
        self._sonando_subido = 0          # cuándo subió esta PC lo que suena (ms)
        self._sonando_ultimo: tuple = ()
        self._revisado_otro = 0.0
        self._otro_aparato.connect(self._fin_otro_aparato)
        self.r.cambioEstado.connect(lambda: self.r.pausado and self._espera_sonando.start())
        self._espera_sonando = QTimer(self, singleShot=True, interval=2500)
        self._espera_sonando.timeout.connect(self._subir_sonando)
        self._reloj_sonando = QTimer(self, interval=60 * 1000)
        self._reloj_sonando.timeout.connect(lambda: self.r.pausado or self._subir_sonando())
        self._reloj_sonando.start()
        self._reloj_otro = QTimer(self, interval=5 * 60 * 1000)
        self._reloj_otro.timeout.connect(self.revisarOtroAparato)
        self._reloj_otro.start()
        self._novedades = self._leer_json("novedades.json")
        self._novedades_listas.connect(self._fin_novedades)
        self._lanzamiento_listo.connect(self._fin_lanzamiento)
        self._aviso_datos = False
        try:
            from PySide6.QtNetwork import QNetworkInformation
            QNetworkInformation.loadDefaultBackend()
        except Exception:  # noqa: BLE001 (sin backend: se toma como red normal)
            pass
        self._despertado = ""             # el día que ya sonó el despertador
        self._reloj_despertador = QTimer(self, interval=20 * 1000)
        self._reloj_despertador.timeout.connect(self._revisar_despertador)
        self._reloj_despertador.start()
        QTimer.singleShot(4000, self.revisarOtroAparato)
        QTimer.singleShot(8000, self.armarMezcla)
        QTimer.singleShot(15000, self.revisarNovedades)

        self._preparar_audio()
        self._recargar()
        QTimer.singleShot(0, self._restaurar)
        QTimer.singleShot(0, self.escanear)

    # --- arranque ---------------------------------------------------------------------------------

    def _preparar_audio(self) -> None:
        """Sin configurar nada: frecuencia nativa y LDAC a 990 kbps quedan listos desde la primera vez
        (se notan al volver a entrar, o con «Aplicar ahora» en Ajustes)."""
        if plataforma.WINDOWS:
            self._audio_pendiente = False
            return
        cambio = audio.config_hires()
        cambio = audio.config_ldac(bool(self.bib.ajustes().get("ldac_hq", True))) or cambio
        self._audio_pendiente = cambio

    # --- canciones y playlists --------------------------------------------------------------------

    def _recargar(self) -> None:
        with self.candado:
            if self._vista_playlist >= 0:
                lista = self.bib.canciones_de_playlist(self._vista_playlist)
                if self._buscar:
                    q = self._buscar.lower()
                    lista = [c for c in lista if q in " ".join((c["titulo"], c["artista"], c["album"])).lower()]
            else:
                lista = self.bib.canciones(self._buscar, self._orden)
            self._playlists = self.bib.playlists()
            for p in self._playlists:   # 2.4: el mosaico de la portada
                p["portadas"] = self.bib.primeras_de_playlist(p["id"])
            self._me_gusta = self.bib.me_gusta()
            # Lo bajado de YouTube lleva su id en el nombre («Título [id].opus»): así la búsqueda sabe qué ya tienes
            self._ids_biblioteca = {m.group(1) for (r,) in self.bib.db.execute("SELECT ruta FROM canciones")
                                    if (m := re.search(r"\[([\w-]{11})\]\.\w+$", r))}
        for c in lista:
            c["portada"] = self._portadas.get(c["ruta"], "")
        self._canciones = lista
        self.cambioCanciones.emit()
        self.cambioPlaylists.emit()
        self.cambioEnLinea.emit()

    @Slot(str)
    def buscar(self, texto: str) -> None:
        self._buscar = texto
        self._recargar()

    @Slot(str)
    def ordenar(self, orden: str) -> None:
        self._orden = orden
        self._recargar()

    @Slot(int)
    def verPlaylist(self, pid: int) -> None:
        self._vista_playlist = pid
        self._recargar()
        if pid >= 0:
            self.recomendar(pid, False)

    def _origen(self) -> str:
        p = next((p for p in self._playlists if p["id"] == self._vista_playlist), None)
        return p["nombre"] if p else "Tu biblioteca"

    @Slot(int)
    def tocar(self, i: int) -> None:
        self.r.ponerOrigen(self._origen())
        self.r.tocar(self._canciones, i)

    @Slot()
    def tocarAleatorio(self) -> None:
        if not self._canciones:
            return
        self.r.ponerOrigen(self._origen())
        if not self.r.aleatorio:
            self.r.alternarAleatorio()
        import random
        self.r.tocar(self._canciones, random.randrange(len(self._canciones)))

    @Slot(str, result=int)
    def crearPlaylist(self, nombre: str) -> int:
        with self.candado:
            pid = self.bib.crear_playlist(nombre)
        self._recargar()
        self._cambio_local()
        return pid

    @Slot(int, str)
    def renombrarPlaylist(self, pid: int, nombre: str) -> None:
        with self.candado:
            self.bib.renombrar_playlist(pid, nombre)
        self._recargar()
        self._cambio_local()

    @Slot(int)
    def borrarPlaylist(self, pid: int) -> None:
        with self.candado:
            self.bib.borrar_playlist(pid)
        if self._vista_playlist == pid:
            self._vista_playlist = -1
        self._recargar()
        self._cambio_local()

    @Slot(int, str)
    def agregarAPlaylist(self, pid: int, ruta: str) -> None:
        with self.candado:
            self.bib.agregar_a_playlist(pid, ruta)
        self._recargar()
        self._cambio_local()
        self.aviso.emit("Agregada a la playlist")

    @Slot(int, str)
    def quitarDePlaylist(self, pid: int, ruta: str) -> None:
        with self.candado:
            self.bib.quitar_de_playlist(pid, ruta)
        self._recargar()
        self._cambio_local()

    @Slot(str)
    def quitarDeBiblioteca(self, ruta: str) -> None:
        """La quita de la biblioteca y, si la bajó Vortex Player, también borra el archivo (a la papelera)."""
        c = self.bib.cancion(ruta)
        if c and c.get("origen") == "youtube":
            self._a_la_papelera(ruta)
            with self.candado:
                self.bib.anotar_cancion_borrada(id_de(ruta))   # también se quita de la nube y del teléfono
        with self.candado:
            self.bib.quitar(ruta)
        self.r.quitarDeCola(ruta)
        self._recargar()
        self._cambio_local()

    @staticmethod
    def _a_la_papelera(ruta: str) -> None:
        plataforma.a_la_papelera(ruta)
        jpg = Path(ruta).with_suffix(".jpg")
        if jpg.exists():
            plataforma.a_la_papelera(str(jpg))

    @Slot(str)
    def abrirCarpeta(self, ruta: str) -> None:
        plataforma.abrir(str(Path(ruta).parent))

    # --- agregar música: carpetas y archivos --------------------------------------------------------

    @Slot()
    def escanear(self) -> None:
        if self._escaneando:
            return
        self._escaneando = True
        self.cambioEscaneo.emit()

        def trabajo() -> None:
            with self.candado:
                r = self.bib.escanear()
            self._escaneado.emit(r)
        threading.Thread(target=trabajo, daemon=True).start()

    def _fin_escaneo(self, r: dict) -> None:
        self._escaneando = False
        self.cambioEscaneo.emit()
        self._recargar()
        if not self._nube["ultima"]:
            self.sincronizar()
        if r.get("nuevas") and r.get("total", 0) > r["nuevas"]:
            self.aviso.emit(f"{r['nuevas']} canciones nuevas en tu biblioteca")
        QTimer.singleShot(3000, self.bajarLetras)

    @Slot(str)
    def agregarCarpeta(self, url: str) -> None:
        carpeta = ruta_de(url)
        carpetas = list(self.bib.ajustes()["carpetas"])
        if carpeta and carpeta not in carpetas and Path(carpeta) != rutas.musica():
            carpetas.append(carpeta)
            Biblioteca.guardar_ajuste("carpetas", carpetas)
            self.cambioAjustes.emit()
        self.escanear()

    @Slot(str)
    def quitarCarpeta(self, carpeta: str) -> None:
        Biblioteca.guardar_ajuste("carpetas", [c for c in self.bib.ajustes()["carpetas"] if c != carpeta])
        with self.candado:
            for c in self.bib.canciones():
                if Path(c["ruta"]).is_relative_to(carpeta):
                    self.bib.quitar(c["ruta"])
        self.cambioAjustes.emit()
        self._recargar()

    @Slot("QVariantList")
    def agregarArchivos(self, urls: list) -> None:
        """Archivos sueltos (o arrastrados a la ventana): se agregan donde están, sin copiarlos."""
        rutas_ = [Path(u.toLocalFile() if isinstance(u, QUrl) else ruta_de(str(u))) for u in urls]

        def trabajo() -> None:
            n = 0
            for r in Biblioteca.archivos_de(rutas_):
                with self.candado:
                    n += self.bib.agregar_archivo(r) is not None
            self._escaneado.emit({"nuevas": n, "total": n + 1})
        threading.Thread(target=trabajo, daemon=True).start()

    # --- portadas ------------------------------------------------------------------------------------

    def portada_de(self, ruta: str) -> str:
        """URL de la portada: la .jpg que dejó yt-dlp, la del disco (cover.jpg…) o la que trae el
        archivo adentro (se saca una vez con ffmpeg a la caché). "" si no tiene."""
        if not ruta:
            return ""
        if ruta in self._portadas:
            return self._portadas[ruta]
        p = Path(ruta)
        for candidata in [p.with_suffix(".jpg"), p.with_suffix(".png")] + [p.parent / n for n in NOMBRES_PORTADA]:
            if candidata.is_file():
                self._portadas[ruta] = QUrl.fromLocalFile(str(candidata)).toString()
                return self._portadas[ruta]
        cache = rutas.cache() / "portadas" / (hashlib.sha1(ruta.encode()).hexdigest() + ".jpg")
        if cache.is_file():
            self._portadas[ruta] = QUrl.fromLocalFile(str(cache)).toString()
            return self._portadas[ruta]
        self._portadas[ruta] = ""
        c = self.bib.cancion(ruta)
        if c and c.get("portada") and shutil.which("ffmpeg"):
            def sacar() -> None:
                cache.parent.mkdir(parents=True, exist_ok=True)
                r = subprocess.run(["ffmpeg", "-v", "quiet", "-y", "-i", ruta, "-an", "-vf", "scale=600:-2",
                                    "-frames:v", "1", str(cache)], capture_output=True)
                if r.returncode == 0 and cache.is_file():
                    self._portadas[ruta] = QUrl.fromLocalFile(str(cache)).toString()
                    self.portadaLista.emit(ruta)
            threading.Thread(target=sacar, daemon=True).start()
        return ""

    @Slot(str, result=str)
    def portada(self, ruta: str) -> str:
        return self.portada_de(ruta)

    # --- descargas ---------------------------------------------------------------------------------

    # --- buscar en YouTube Music / YouTube (2.1) ------------------------------------------------------

    @Slot(str, str)
    def buscarEnLinea(self, texto: str, fuente: str) -> None:
        self._busqueda_linea += 1
        n = self._busqueda_linea
        texto = texto.strip()
        self._error_linea = ""
        if not texto:
            self._en_linea, self._buscando_linea = [], False
            self.cambioEnLinea.emit()
            return
        self._buscando_linea = True
        self.cambioEnLinea.emit()

        def hilo() -> None:
            try:
                self._resultados.emit(n, descargas.buscar(texto, fuente), "")
            except OSError as e:   # sin red, yt-dlp que falta o YouTube que no contesta
                claro = e.args[0] if len(e.args) == 1 and isinstance(e.args[0], str) else "Sin conexión con YouTube"
                self._resultados.emit(n, [], claro)

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_busqueda_linea(self, n: int, canciones: list, error: str) -> None:
        if n != self._busqueda_linea:
            return
        self._en_linea, self._error_linea, self._buscando_linea = list(canciones), error, False
        self.cambioEnLinea.emit()

    def _con_estado(self, lista: list[dict]) -> list[dict]:
        bajando = {d.get("id") for d in self._descargas if d.get("id") and d["estado"] in ("espera", "bajando")}
        return [{**c, "titulo": letras.titulo_limpio(c.get("titulo", ""), c.get("artistas", "")),
                 "estado": "biblioteca" if c["id"] in self._ids_biblioteca
                 else "bajando" if c["id"] in bajando else ""} for c in lista]

    def _sin_las_tuyas(self, lista: list[dict]) -> list[dict]:
        """2.4: las recomendadas no repiten lo que ya tienes: al bajarla, se quita de la lista."""
        return [c for c in self._con_estado(lista) if c["estado"] != "biblioteca"]

    # --- recomendaciones para una playlist (2.1) -------------------------------------------------------

    @staticmethod
    def _id_de(ruta: str) -> str:
        m = re.search(r"\[([\w-]{11})\]\.\w+$", ruta)
        return m.group(1) if m else ""

    @Slot(int, bool)
    def recomendar(self, pid: int, otras: bool) -> None:
        """Las de una playlist: las guardadas, o con «otras» unas nuevas (otras canciones de semilla)."""
        self._recom_vuelta += 1
        vuelta = self._recom_vuelta
        self._recom_para, self._recom_error = pid, ""
        if not otras and pid in self._recom_guardadas:
            self._recom, self._recom_cargando = self._recom_guardadas[pid], False
            self.cambioRecomendaciones.emit()
            return
        with self.candado:
            de_la_playlist = self.bib.canciones_de_playlist(pid)
            todas = self.bib.canciones()
        if not de_la_playlist:
            self._recom, self._recom_cargando = [], False
            self.cambioRecomendaciones.emit()
            return
        # Hasta 3 de semilla, al azar: «Otras» trae algo distinto cada vez. Primero las bajadas de YouTube
        # (ya traen su id y no hay que buscarlas)
        con_id = [c for c in de_la_playlist if self._id_de(c["ruta"])]
        sin_id = [c for c in de_la_playlist if not self._id_de(c["ruta"])]
        random.shuffle(con_id)
        random.shuffle(sin_id)
        semillas = [{"id": self._id_de(c["ruta"]), "titulo": c["titulo"], "artista": c["artista"]}
                    for c in (con_id + sin_id)[:3]]
        tienes = [{"id": self._id_de(c["ruta"]), "titulo": c["titulo"], "artista": c["artista"]} for c in todas]
        if otras:   # 2.4: «Otras» no repite las que ya te había enseñado
            tienes += [{"id": c["id"], "titulo": c["titulo"], "artista": c["artistas"]} for c in self._recom_guardadas.get(pid, [])]
        self._recom_cargando = True
        if otras:
            self._recom = []
        self.cambioRecomendaciones.emit()

        def hilo() -> None:
            try:
                self._recomendadas.emit(vuelta, pid, descargas.recomendar(semillas, tienes), "")
            except OSError as e:
                claro = e.args[0] if len(e.args) == 1 and isinstance(e.args[0], str) else "Sin conexión con YouTube"
                self._recomendadas.emit(vuelta, pid, [], claro)

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_recomendar(self, vuelta: int, pid: int, canciones: list, error: str) -> None:
        if canciones:
            self._recom_guardadas[pid] = list(canciones)
        if vuelta != self._recom_vuelta:
            return
        self._recom, self._recom_error, self._recom_cargando = list(canciones), error, False
        self.cambioRecomendaciones.emit()

    @Slot(int)
    def agregarRecomendada(self, i: int) -> None:
        """La baja a tu biblioteca y la agrega a la playlist para la que se recomendó."""
        lista = self._sin_las_tuyas(self._recom)
        if 0 <= i < len(lista) and self._recom_para >= 0:
            c = lista[i]
            self.bajar(c["url"], self._recom_para, c["id"],
                       f"{c['titulo']} · {c['artistas']}" if c["artistas"] else c["titulo"])

    # --- avance: 30 s antes de bajarla (2.1.1) -------------------------------------------------------

    def _escuchar(self, c: dict) -> None:
        if self._avance.idActual == c["id"]:
            self._avance.parar()
        else:
            self._avance.tocar(c["id"], c["url"], float(c.get("duracion") or 0), self.r.volumen)

    @Slot(int)
    def escucharRecomendada(self, i: int) -> None:
        """Toca 30 s de la recomendada i (otra vez, la para)."""
        lista = self._sin_las_tuyas(self._recom)
        if 0 <= i < len(lista):
            self._escuchar(lista[i])

    @Slot(int)
    def escucharResultado(self, i: int) -> None:
        if 0 <= i < len(self._en_linea):
            self._escuchar(self._en_linea[i])

    @Slot()
    def pararAvance(self) -> None:
        self._avance.parar()

    def _pausar_por_avance(self) -> None:
        self._seguir_tras_avance = not self.r.pausado
        self.r.pausar()

    def _seguir_tras_el_avance(self) -> None:
        if self._seguir_tras_avance:
            self._seguir_tras_avance = False
            self.r.seguir()

    def _musica_quiere_sonar(self) -> None:
        # Si le das play a tu música mientras suena un avance, el avance se calla
        if self._avance.idActual:
            self._seguir_tras_avance = False
            self._avance.parar()

    @Slot(int, int)
    def bajarResultado(self, i: int, playlist: int) -> None:
        """Baja el resultado i a la biblioteca; con playlist ≥ 0, además lo agrega a esa playlist."""
        if 0 <= i < len(self._en_linea):
            c = self._en_linea[i]
            self.bajar(c["url"], playlist, c["id"], f"{c['titulo']} · {c['artistas']}" if c["artistas"] else c["titulo"])

    @Slot(str)
    def bajar(self, url: str, playlist: int = -1, vid: str = "", nombre: str = "") -> None:
        url = url.strip()
        t = descargas.tipo_de(url)
        if t is None:
            self.aviso.emit("Pega un enlace de YouTube, YouTube Music o Spotify")
            return
        n = len(self._descargas)
        self._descargas.append({"url": url, "servicio": t[0], "nombre": nombre or url, "total": 0, "hechas": 0,
                                "fallidas": 0, "actual": "En espera…", "porcentaje": 0.0, "estado": "espera",
                                "avisos": [], "playlist": playlist, "id": vid})
        self.cambioDescargas.emit()
        self._cola_urls.append((n, url))
        if self._hilo_descargas is None or not self._hilo_descargas.is_alive():
            self._parar_descargas = False
            self._hilo_descargas = threading.Thread(target=self._bajar_todo, daemon=True)
            self._hilo_descargas.start()

    def _bajar_todo(self) -> None:
        while self._cola_urls and not self._parar_descargas:
            n, url = self._cola_urls.pop(0)
            self._evento.emit(n, {"tipo": "empieza"})
            for ev in descargas.descargar(url, rutas.descargas(), self.bib.ajustes().get("navegador_premium", ""),
                                          parar=lambda: self._parar_descargas):
                if ev["tipo"] == "archivo":
                    # Se lee aquí (en el hilo) para no trabar la ventana con ffprobe
                    with self.candado:
                        info = self.bib.agregar_archivo(Path(ev["ruta"]), origen="youtube")
                    ev["ok"] = info is not None
                self._evento.emit(n, ev)

    def _en_evento(self, n: int, ev: dict) -> None:
        d = self._descargas[n]
        tipo = ev.get("tipo")
        if tipo == "empieza":
            d["estado"] = "bajando"
            d["actual"] = "Leyendo el enlace…"
        elif tipo == "inicio":
            # Lo que viene de la búsqueda ya trae su nombre («Título · Artista»)
            d["nombre"] = ev.get("nombre") or (d["nombre"] if d["nombre"] != d["url"]
                                               else "Canción" if not ev.get("es_lista") else "Playlist")
            d["total"] = ev.get("total", 0)
            if ev.get("es_lista"):
                with self.candado:
                    d["playlist"] = self.bib.crear_playlist(d["nombre"], url=d["url"])
        elif tipo == "cancion":
            d["actual"] = ev.get("titulo", "")
            d["porcentaje"] = 0.0
        elif tipo == "progreso":
            d["porcentaje"] = ev.get("porcentaje", 0.0)
            if ev.get("titulo") and d["servicio"] != "spotify":
                d["actual"] = ev["titulo"]
        elif tipo == "archivo":
            d["hechas"] += 1
            if d["nombre"] in (d["url"], "Canción"):
                d["nombre"] = Path(ev["ruta"]).stem.rsplit(" [", 1)[0]
            if d["playlist"] >= 0 and ev.get("ok"):
                with self.candado:
                    self.bib.agregar_a_playlist(d["playlist"], ev["ruta"])
            self._recargar()
        elif tipo == "aviso":
            d["avisos"] = (d["avisos"] + [ev.get("texto", "")])[-5:]
            if ev.get("fatal"):
                d["estado"] = "error"
                d["actual"] = ev.get("texto", "")
        elif tipo == "fin":
            d["fallidas"] = ev.get("fallidas", 0)
            if d.get("id") in self._nube_pendientes:
                self._nube_pendientes.discard(d["id"])
                if not d["hechas"]:
                    self._nube_fallidas.add(d["id"])
            self._cambio_local()   # lo nuevo sube a la nube y entra a sus playlists
            if d["estado"] != "error":
                d["estado"] = "lista"
                d["actual"] = (f"{ev.get('bajadas', 0)} bajadas" +
                               (f" · {d['fallidas']} no se encontraron" if d["fallidas"] else ""))
            d["porcentaje"] = 100.0
            if d["estado"] == "lista":
                self.aviso.emit(f"Listo: {d['nombre']}")
            self._espera_letras.start()   # 2.4: su letra, para verla sin internet
        self.cambioDescargas.emit()

    @Slot()
    def pararDescargas(self) -> None:
        self._parar_descargas = True
        self._cola_urls.clear()

    @Slot()
    def limpiarDescargas(self) -> None:
        if self._hilo_descargas and self._hilo_descargas.is_alive():
            return
        self._descargas = []
        self.cambioDescargas.emit()

    # --- la nube: tu biblioteca en Google Drive, la misma que en el teléfono (2.4) -------------------------

    def _cambio_local(self) -> None:
        if self._nube["correo"]:
            self._espera_nube.start()

    def _poner_nube(self, **cambios) -> None:
        self._nube = {**self._nube, **cambios}
        self.cambioNube.emit()

    @Slot()
    def iniciarSesionGoogle(self) -> None:
        if self._nube["trabajando"]:
            return
        self._poner_nube(trabajando=True, texto="Elige tu cuenta en el navegador…", error="")

        def abrir(url: str) -> None:
            plataforma.abrir(url)

        def hilo() -> None:
            try:
                d = cuenta_google.iniciar_sesion(abrir)
                self._nube_fin.emit({"sesion": True, "correo": d["correo"], "nombre": d["nombre"]})
            except Exception as e:  # noqa: BLE001 (cualquier falla se le dice a la persona)
                self._nube_fin.emit({"sesion": True, "error": str(e)})
        threading.Thread(target=hilo, daemon=True).start()

    @Slot()
    def cerrarSesionGoogle(self) -> None:
        threading.Thread(target=cuenta_google.cerrar_sesion, daemon=True).start()
        self._poner_nube(correo="", nombre="", texto="", ultima=0.0, error="")
        self.aviso.emit("Sesión cerrada. Tus canciones se quedan en esta PC.")

    @Slot()
    def sincronizar(self) -> None:
        if not self._nube["correo"]:
            return
        if self._nube["trabajando"]:
            self._nube_otra_vez = True
            return
        self._poner_nube(trabajando=True, texto="Sincronizando con la nube…", error="")

        def borrar(vid: str, ruta: str) -> None:   # la borraste en el teléfono (ya se tiene el candado)
            self._a_la_papelera(ruta)
            self.bib.quitar(ruta)
            self._quitar_de_cola.emit(ruta)

        def hilo() -> None:
            try:
                for intento in range(2):
                    try:
                        _, plan = sincronia.sincronizar(self.bib, cuenta_google.Drive(cuenta_google.token()),
                                                        borrar, self.candado)
                        break
                    except cuenta_google.SinPermiso:
                        cuenta_google.olvidar_token()
                        if intento:
                            raise
                bajar = [{"id": vid, "nombre": " · ".join(x for x in (c.get("titulo"), c.get("artista")) if x) or vid}
                         for vid, c in plan.bajar]
                self._nube_fin.emit({"ok": True, "bajar": bajar, "borradas": len(plan.borrar)})
            except cuenta_google.SinPermiso as e:
                self._nube_fin.emit({"ok": False, "error": str(e), "salir": True})
            except Exception as e:  # noqa: BLE001
                self._nube_fin.emit({"ok": False, "error": f"No se pudo sincronizar: {e}"})
        threading.Thread(target=hilo, daemon=True).start()

    def _fin_nube(self, r: dict) -> None:
        if r.get("sesion"):   # terminó «Iniciar sesión»
            if r.get("error"):
                self._poner_nube(trabajando=False, texto="", error=r["error"])
                return
            self._poner_nube(trabajando=False, correo=r["correo"], nombre=r["nombre"])
            self.aviso.emit(f"Listo: tu música se sincroniza con {r['correo']}")
            self.sincronizar()
            return
        if not r.get("ok"):
            self._poner_nube(trabajando=False, texto="", error=r.get("error", ""))
            if r.get("salir"):
                cuenta_google.cerrar_sesion()
                self._poner_nube(correo="", nombre="")
            return
        en_cola = {d.get("id") for d in self._descargas if d.get("id") and d["estado"] in ("espera", "bajando")}
        nuevas = [c for c in r["bajar"] if c["id"] not in en_cola and c["id"] not in self._nube_fallidas]
        for c in nuevas:
            self._nube_pendientes.add(c["id"])
            self.bajar(f"https://music.youtube.com/watch?v={c['id']}", -1, c["id"], c["nombre"])
        texto = (f"Bajando {len(nuevas)} canciones de tu otro dispositivo" if nuevas
                 else f"No se pudieron bajar {len(self._nube_fallidas)}; se reintentan al volver a abrir" if self._nube_fallidas
                 else "")
        self._poner_nube(trabajando=False, ultima=time.time(), texto=texto, error="")
        self._recargar()
        if self._nube_otra_vez:
            self._nube_otra_vez = False
            self._espera_nube.start()

    def _estado_nube(self) -> dict:
        return {**self._nube, "configurada": cuenta_google.configurada()}

    # --- ecualizador -------------------------------------------------------------------------------

    def _vigilar_eq(self) -> None:
        archivo = ecualizador.archivo_estado()
        for lista, cosa in ((self._vigia_eq.files(), str(archivo)), (self._vigia_eq.directories(), str(archivo.parent))):
            if cosa not in lista and Path(cosa).exists():
                self._vigia_eq.addPath(cosa)

    def _eq_cambio(self, *_: str) -> None:
        self._vigilar_eq()   # el script reescribe el archivo: se vuelve a vigilar
        # 2.1: mientras mueves una banda aquí, el script escribe el archivo a medias (set_bands y luego apply)
        # y releerlo regresaba la barra a un valor viejo. Se vuelve a leer cuando terminó de aplicar.
        espera = self._eq_local + 2.0 - time.monotonic()
        if espera > 0:
            QTimer.singleShot(int(espera * 1000) + 50, self._eq_cambio)
            return
        nuevo = ecualizador.estado()
        if nuevo != self._eq:
            self._eq = nuevo
            self.cambioEq.emit()
        self._eq_a_mpv()

    def _eq_a_mpv(self) -> None:
        """2.5: en Windows el ecualizador suena dentro de mpv."""
        if ecualizador.en_mpv():
            self.r.ponerEq(ecualizador.filtro_mpv(self._eq["bandas"]))

    @Slot(int, float)
    def ponerBanda(self, i: int, g: float) -> None:
        bandas = [dict(b) for b in self._eq["bandas"]]
        if 0 <= i < len(bandas):
            bandas[i]["g"] = round(max(-12.0, min(12.0, g)), 1)
            self._eq = {**self._eq, "bandas": bandas, "preset": "Custom"}
            self._eq_local = time.monotonic()
            self.cambioEq.emit()
            self._aplicar_eq.start()

    @Slot(str)
    def ponerPreset(self, nombre: str) -> None:
        threading.Thread(target=ecualizador.poner_preset, args=(nombre,), daemon=True).start()

    def _aplicar_bandas(self) -> None:
        bandas = [dict(b) for b in self._eq["bandas"]]
        self._eq_a_mpv()

        def aplicar() -> None:
            ecualizador.poner_bandas(bandas)
            # EasyEffects puede tardar más que la espera de _eq_cambio: se cuenta desde que terminó
            self._eq_local = max(self._eq_local, time.monotonic() - 1.5)

        threading.Thread(target=aplicar, daemon=True).start()

    @property
    def _aplicar_eq(self) -> QTimer:
        # Al soltar la barra (o 400 ms sin moverla) se aplica, como «Aplicar» en el panel
        if not hasattr(self, "_timer_eq"):
            self._timer_eq = QTimer(self, singleShot=True, interval=400)
            self._timer_eq.timeout.connect(self._aplicar_bandas)
        return self._timer_eq

    # --- salida de audio y ajustes -----------------------------------------------------------------

    @Slot()
    def actualizarSalida(self) -> None:
        s = audio.salida()
        if s != self._salida:
            self._salida = s
            self.cambioSalida.emit()

    @Slot()
    def forzarLdac(self) -> None:
        if audio.forzar_ldac(self._salida.get("tarjeta", "")):
            self.aviso.emit("Cambiado a LDAC")
            QTimer.singleShot(1500, self.actualizarSalida)
        else:
            self.aviso.emit("Estos audífonos no ofrecen LDAC ahorita")

    @Slot(bool)
    def ponerLdacHq(self, activar: bool) -> None:
        Biblioteca.guardar_ajuste("ldac_hq", activar)
        if audio.config_ldac(activar):
            self._audio_pendiente = True
        self.cambioAjustes.emit()

    @Slot(str)
    def ponerNavegador(self, navegador: str) -> None:
        Biblioteca.guardar_ajuste("navegador_premium", navegador if navegador in NAVEGADORES else "")
        self.cambioAjustes.emit()

    @Slot(int)
    def guardarVolumen(self, v: int) -> None:
        self.r.ponerVolumen(v)
        Biblioteca.guardar_ajuste("volumen", int(v))

    @Slot()
    def aplicarAudio(self) -> None:
        """Vuelve a arrancar PipeWire para que lea la frecuencia nativa y LDAC (corta el sonido 1 s)."""
        posicion, sonaba = self.r.posicion, not self.r.pausado
        self.r.pausar()
        if audio.reiniciar_audio():
            self._audio_pendiente = False
            self.cambioAjustes.emit()
            self.aviso.emit("Listo: frecuencia nativa y LDAC a 990 kbps")
            if sonaba:
                QTimer.singleShot(2500, lambda: (self.r.buscar(posicion), self.r.seguir()))
        else:
            self.aviso.emit("No pude reiniciar el audio")
        QTimer.singleShot(2500, self.actualizarSalida)

    # --- 2.4: seguir donde te quedaste ---------------------------------------------------------------

    def _archivo_sesion(self) -> Path:
        return rutas.config() / "sesion.json"

    def _leer_sesion(self) -> dict:
        try:
            d = json.loads(self._archivo_sesion().read_text())
            return d if isinstance(d, dict) else {}
        except (OSError, ValueError):
            return {}

    def _guardar_sesion(self) -> None:
        if not self._restaurada:
            return
        if self.r.cancion.get("ruta"):
            self._sesion["cola"] = self.r.sesion()
        try:
            self._archivo_sesion().parent.mkdir(parents=True, exist_ok=True)
            tmp = self._archivo_sesion().with_suffix(".tmp")
            tmp.write_text(json.dumps(self._sesion, ensure_ascii=False))
            tmp.replace(self._archivo_sesion())
        except OSError:
            pass

    def _restaurar(self) -> None:
        """Al abrir: la última canción, en pausa, en el segundo donde ibas, con su cola."""
        s = self._sesion.get("cola") or {}
        if not self.r.cancion.get("ruta") and s.get("rutas"):
            with self.candado:
                cola = [c for r in s["rutas"] if (c := self.bib.cancion(r))]
            self.r.restaurar(cola, s)
        self._restaurada = True
        self._cambio_cancion()

    def _anotar_escuchada(self, c: dict) -> None:
        """Las últimas 25 que escuchaste (sin repetir), para «Para ti»."""
        nueva = {"id": id_de(c["ruta"]), "titulo": c.get("titulo", ""), "artista": c.get("artista", "")}
        h = [x for x in self._sesion.get("historial", []) if (x.get("titulo"), x.get("artista")) != (nueva["titulo"], nueva["artista"])]
        self._sesion["historial"] = ([nueva] + h)[:25]

    def _cambio_cancion(self) -> None:
        c = self.r.cancion
        ruta = c.get("ruta", "")
        if ruta == self._letra["ruta"] or not self._restaurada:
            return
        if ruta and self.r.sesion()["posicion"] < 1:   # lo restaurado no cuenta como «escuchada» otra vez
            self._anotar_escuchada(c)
        self._guardar_sesion()
        self._buscar_letra(c)

    # --- 2.4: la letra -----------------------------------------------------------------------------

    def _buscar_letra(self, c: dict) -> None:
        self._letra_vuelta += 1
        vuelta = self._letra_vuelta
        ruta = c.get("ruta", "")
        self._letra = {"ruta": ruta, "lineas": [], "sincronizada": False, "buscando": bool(ruta)}
        self.cambioLetra.emit()
        if not ruta:
            return
        titulo, artista, dur = c.get("titulo") or Path(ruta).stem, c.get("artista") or "", int(c.get("duracion") or 0)

        def hilo() -> None:
            try:
                l = letras.de(rutas.cache() / "letras", titulo, artista, dur, id_de(ruta), descargas.letra_youtube)
                self._letra_lista.emit(vuelta, l or {})
            except OSError:
                self._letra_lista.emit(vuelta, {"error": "Sin conexión: no pude buscar la letra"})

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_letra(self, vuelta: int, l: dict) -> None:
        if vuelta != self._letra_vuelta:
            return
        self._letra = {"ruta": self._letra["ruta"], "lineas": l.get("lineas", []), "sincronizada": bool(l.get("sincronizada")),
                       "buscando": False, "error": l.get("error", "")}
        self.cambioLetra.emit()

    @Slot(float, result=int)
    def lineaActual(self, segundos: float) -> int:
        return letras.actual(self._letra["lineas"], segundos * 1000) if self._letra["sincronizada"] else -1

    # --- 2.4: «Para ti» en Agregar música ----------------------------------------------------------

    @Slot(bool)
    def recomendarParaTi(self, otras: bool) -> None:
        h = self._sesion.get("historial", [])
        semillas = (random.sample(h[:10], len(h[:10])) if otras else h)[:3]
        with self.candado:
            todas = self.bib.canciones()
        if not semillas:
            semillas = [{"id": id_de(c["ruta"]), "titulo": c["titulo"], "artista": c["artista"]} for c in random.sample(todas, min(3, len(todas)))]
        if not semillas:
            return
        de = [x["titulo"] for x in semillas]
        if not otras and self._para_ti and de == self._para_ti_de:
            return
        self._para_ti_vuelta += 1
        vuelta = self._para_ti_vuelta
        tienes = [{"id": id_de(c["ruta"]), "titulo": c["titulo"], "artista": c["artista"]} for c in todas]
        if otras:
            tienes += [{"id": c["id"], "titulo": c["titulo"], "artista": c["artistas"]} for c in self._para_ti]
        self._para_ti_cargando, self._para_ti_error = True, ""
        self.cambioParaTi.emit()

        def hilo() -> None:
            try:
                self._para_ti_listo.emit(vuelta, descargas.recomendar(semillas, tienes, 25), de, "")
            except OSError as e:
                claro = e.args[0] if len(e.args) == 1 and isinstance(e.args[0], str) else "Sin conexión con YouTube"
                self._para_ti_listo.emit(vuelta, [], de, claro)

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_para_ti(self, vuelta: int, canciones: list, de: list, error: str) -> None:
        if vuelta != self._para_ti_vuelta:
            return
        if canciones or not self._para_ti:
            self._para_ti, self._para_ti_de = list(canciones), list(de)
        self._para_ti_error, self._para_ti_cargando = error, False
        self.cambioParaTi.emit()

    @Slot(int)
    def bajarParaTi(self, i: int) -> None:
        lista = self._sin_las_tuyas(self._para_ti)
        if 0 <= i < len(lista):
            c = lista[i]
            self.bajar(c["url"], -1, c["id"], f"{c['titulo']} · {c['artistas']}" if c["artistas"] else c["titulo"])

    @Slot(int)
    def escucharParaTi(self, i: int) -> None:
        lista = self._sin_las_tuyas(self._para_ti)
        if 0 <= i < len(lista):
            self._escuchar(lista[i])

    # --- 2.4: ❤️ «Tus me gusta», la cola y el crossfade ------------------------------------------------

    @Slot(str)
    def alternarMeGusta(self, ruta: str) -> None:
        if not ruta:
            return
        with self.candado:
            gusta = self.bib.alternar_me_gusta(ruta)
        self._recargar()
        self._cambio_local()
        self.aviso.emit("Agregada a Tus me gusta" if gusta else "Quitada de Tus me gusta")

    @Slot("QVariantMap")
    def tocarDespues(self, c: dict) -> None:
        self.r.tocarDespues(c)
        self.aviso.emit("Suena después de esta")

    @Slot("QVariantMap")
    def agregarACola(self, c: dict) -> None:
        self.r.agregarACola(c)
        self.aviso.emit("Agregada a la cola")

    # --- 1.4: la radio sin fin ---------------------------------------------------------------------------

    @Slot(bool)
    def ponerRadio(self, activar: bool) -> None:
        Biblioteca.guardar_ajuste("radio", bool(activar))
        self.cambioAjustes.emit()
        if activar:
            self._pedir_radio()

    @Slot(bool)
    def ponerDiscord(self, activar: bool) -> None:
        Biblioteca.guardar_ajuste("discord", bool(activar))
        self.cambioAjustes.emit()

    def _pedir_radio(self) -> None:
        """Suena la última de la cola: se piden 10 parecidas a ella y a las dos de antes."""
        cola, i = self.r.cola, self.r.indice
        # 1.5: la que pediste con «Iniciar radio» sigue aunque la radio sin fin esté apagada
        a_mano = self.r.origen.startswith("Radio de ")
        if (not (a_mano or self.bib.ajustes().get("radio", True)) or not cola or i + 1 < len(cola) or self.r.repetir != "no"
                or self._radio_para == cola[-1]["ruta"]):
            return
        medida = self._con_datos_medidos()
        if os.environ.get("VORTEX_PLAYER_SIN_RED") and not medida:
            return
        semillas = [{"id": c.get("id") or id_de(c["ruta"]), "titulo": c.get("titulo", ""), "artista": c.get("artista", "")}
                    for c in reversed(cola[max(0, i - 2):i + 1])]
        with self.candado:
            todas = self.bib.canciones()
        tienes = [{"id": c.get("id") or id_de(c["ruta"]), "titulo": c.get("titulo", ""), "artista": c.get("artista", "")}
                  for c in todas + cola]
        if medida:   # 1.5: con datos móviles la radio sigue con tu música (no gasta tu plan)
            self.r.agregarRadio(self._radio_local(semillas, {c["ruta"] for c in cola}, todas))
            if not self._aviso_datos:
                self._aviso_datos = True
                self.aviso.emit("Con datos móviles, la radio sigue con tu música")
            return
        self._radio_vuelta += 1
        vuelta, self._radio_para = self._radio_vuelta, cola[-1]["ruta"]

        def hilo() -> None:
            try:
                self._radio_lista.emit(vuelta, descargas.recomendar(semillas, tienes, 10), "")
            except OSError:
                self._radio_lista.emit(vuelta, [], "Sin conexión: la radio no pudo seguir")

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_radio(self, vuelta: int, canciones: list, error: str) -> None:
        if vuelta != self._radio_vuelta:
            return
        cola, para = self.r.cola, self._radio_para
        self._radio_para = ""
        if not cola or cola[-1]["ruta"] != para or not (self.r.origen.startswith("Radio de ")
                                                         or self.bib.ajustes().get("radio", True)):
            return
        if error or not canciones:
            if self.r.cancion and self.r.indice + 1 >= len(self.r.cola) and self.r.pausado:
                self.aviso.emit(error or "La radio no encontró más canciones")
            return
        lista = []
        for c in canciones:
            self._portadas[c["url"]] = c.get("portada", "")
            lista.append({"ruta": c["url"], "id": c["id"], "titulo": c["titulo"], "artista": c.get("artistas", ""),
                          "album": c.get("album", ""), "duracion": float(c.get("duracion") or 0), "radio": True})
        self.r.agregarRadio(lista)

    # --- 1.4: «Tu resumen» de la semana ------------------------------------------------------------------

    def _anotar_escucha(self, c: dict, segundos: float) -> None:
        with self.candado:
            self.bib.anotar_escucha(c, segundos)
        self._resumen = {}
        self.cambioResumen.emit()

    def _estado_resumen(self) -> dict:
        esta = biblioteca.semana(0)
        if self._resumen.get("semana", {}).get("desde") != esta[0]:   # cambió la semana: se vuelve a sacar
            with self.candado:
                pasada = self.bib.resumen(*biblioteca.semana(1))
                self._resumen = {"semana": self.bib.resumen(*esta), "pasada": pasada}
        visto = float(self.bib.ajustes().get("resumen_visto", 0))
        # el aviso sale una vez por semana, el lunes que abres la app, si la pasada escuchaste algo
        return {**self._resumen, "aviso": self._resumen["pasada"]["escuchas"] > 0 and visto < esta[0]}

    @Slot()
    def resumenVisto(self) -> None:
        Biblioteca.guardar_ajuste("resumen_visto", biblioteca.semana(0)[0])
        self.cambioResumen.emit()

    # --- 1.5: la radio con datos móviles ---------------------------------------------------------------------

    @staticmethod
    def _con_datos_medidos() -> bool:
        """¿La conexión se cobra por lo que gastas (datos del cel, «conexión de uso medido» en Windows)?"""
        if os.environ.get("VORTEX_PLAYER_MEDIDA") is not None:   # pruebas
            return os.environ["VORTEX_PLAYER_MEDIDA"] == "1"
        try:
            from PySide6.QtNetwork import QNetworkInformation
            i = QNetworkInformation.instance()
            return bool(i and i.supports(QNetworkInformation.Feature.Metered) and i.isMetered())
        except Exception:  # noqa: BLE001
            return False

    @staticmethod
    def _radio_local(semillas: list[dict], en_cola: set[str], todas: list[dict], n: int = 10) -> list[dict]:
        """Diez de tu biblioteca: primero de los mismos artistas, luego al azar."""
        artistas = {letras.limpiar_artista(s.get("artista", "")).lower() for s in semillas} - {""}
        libres = [c for c in todas if c["ruta"] not in en_cola]
        random.shuffle(libres)
        mismas = [c for c in libres if letras.limpiar_artista(c.get("artista", "")).lower() in artistas]
        return (mismas + [c for c in libres if c not in mismas])[:n]

    # --- 1.5: tu mezcla del día -------------------------------------------------------------------------

    @staticmethod
    def _leer_json(nombre: str) -> dict:
        try:
            d = json.loads((rutas.config() / nombre).read_text())
            return d if isinstance(d, dict) else {}
        except (OSError, ValueError):
            return {}

    @staticmethod
    def _guardar_json(nombre: str, d: dict) -> None:
        try:
            rutas.config().mkdir(parents=True, exist_ok=True)
            tmp = rutas.config() / (nombre + ".tmp")
            tmp.write_text(json.dumps(d, ensure_ascii=False))
            tmp.replace(rutas.config() / nombre)
        except OSError:
            pass

    @Slot()
    def armarMezcla(self, forzar: bool = False) -> None:
        """Una vez al día: lo que más escuchas (30 días) revuelto con 15 parecidas que no tienes."""
        hoy = time.strftime("%Y-%m-%d")
        if self._mezcla_armando or (not forzar and self._mezcla.get("fecha") == hoy) or os.environ.get("VORTEX_PLAYER_SIN_RED"):
            return
        with self.candado:
            tuyas = self.bib.mas_escuchadas(time.time() - 30 * 86400, 15)
            todas = self.bib.canciones()
        if len(tuyas) < 15:
            resto = [c for c in todas if c["ruta"] not in {t["ruta"] for t in tuyas}]
            tuyas += random.sample(resto, min(15 - len(tuyas), len(resto)))
        if not tuyas:
            return
        semillas = [{"id": id_de(c["ruta"]), "titulo": c["titulo"], "artista": c["artista"]}
                    for c in random.sample(tuyas[:10], min(4, len(tuyas[:10])))]
        tienes = [{"id": id_de(c["ruta"]), "titulo": c["titulo"], "artista": c["artista"]} for c in todas]
        self._mezcla_armando = True

        def hilo() -> None:
            try:
                nuevas = descargas.recomendar(semillas, tienes, 15)
            except OSError:
                nuevas = []
            self._mezcla_lista.emit({"fecha": hoy, "tuyas": [c["ruta"] for c in tuyas], "nuevas": nuevas})

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_mezcla(self, r: dict) -> None:
        self._mezcla_armando = False
        if not r.get("nuevas"):
            return   # sin internet: se intenta la próxima vez que abras la app
        tuyas = list(r["tuyas"])
        random.shuffle(tuyas)
        nuevas = [self._de_la_radio(c) for c in r["nuevas"]]
        canciones = []
        for k in range(max(len(tuyas), len(nuevas))):   # una tuya, una nueva…
            if k < len(tuyas):
                canciones.append({"ruta": tuyas[k]})
            if k < len(nuevas):
                canciones.append(nuevas[k])
        self._mezcla = {"fecha": r["fecha"], "canciones": canciones}
        self._guardar_json("mezcla.json", self._mezcla)
        self.cambioMezcla.emit()

    def _de_la_radio(self, c: dict) -> dict:
        """Una de YouTube Music (de `descargas.buscar`/`radio`) como canción de la cola, que suena sin bajarla."""
        self._portadas[c["url"]] = c.get("portada", "")
        return {"ruta": c["url"], "id": c["id"], "titulo": c["titulo"], "artista": c.get("artistas", ""),
                "album": c.get("album", ""), "duracion": float(c.get("duracion") or 0), "portada": c.get("portada", ""),
                "radio": True}

    def _canciones_mezcla(self) -> list[dict]:
        salida = []
        for c in self._mezcla.get("canciones") or []:
            if c.get("radio"):
                self._portadas.setdefault(c["ruta"], c.get("portada", ""))
                salida.append(c)
            elif (t := self.bib.cancion(c["ruta"])):
                salida.append(t)
        return salida

    def _estado_mezcla(self) -> dict:
        lista = self._canciones_mezcla() if self._mezcla.get("canciones") else []
        return {"fecha": self._mezcla.get("fecha", ""), "cuantas": len(lista), "hoy": self._mezcla.get("fecha") == time.strftime("%Y-%m-%d"),
                "nuevas": sum(1 for c in lista if c.get("radio")), "portadas": [c["ruta"] for c in lista[:4]],
                "armando": self._mezcla_armando}

    @Slot()
    def tocarMezcla(self) -> None:
        lista = self._canciones_mezcla()
        if lista:
            self.r.ponerOrigen("Tu mezcla del día")
            self.r.tocar(lista, 0)

    # --- 1.5: seguir en el otro aparato (lo que suena va a la nube) ----------------------------------------

    def _aparato(self) -> str:
        a = self.bib.ajustes().get("aparato")
        if not a:
            import uuid
            a = "pc-" + uuid.uuid4().hex[:12]
            Biblioteca.guardar_ajuste("aparato", a)
        return a

    def _subir_sonando(self, esperar: float = 0) -> None:
        """Lo que suena y en qué segundo, para que el cel ofrezca seguir ahí. Solo las que tienen id de YouTube."""
        c = self.r.cancion
        vid = c.get("id") or id_de(c.get("ruta", ""))
        if not self._nube["correo"] or not vid or os.environ.get("VORTEX_PLAYER_SIN_RED"):
            return
        clave = (vid, int(self.r.posicion) // 10, self.r.pausado)
        if clave == self._sonando_ultimo:
            return
        self._sonando_ultimo = clave
        self._sonando_subido = int(time.time() * 1000)
        portada = c.get("portada") or self._portadas.get(c.get("ruta", ""), "")
        datos = {"aparato": self._aparato(), "nombre": "tu PC", "id": vid, "titulo": c.get("titulo", ""),
                 "artista": c.get("artista", ""), "album": c.get("album", ""), "duracion": float(c.get("duracion") or 0),
                 "portada": portada if portada.startswith("http") else "", "posicion": round(self.r.posicion, 1),
                 "cuando": self._sonando_subido}

        def hilo() -> None:
            try:
                d = cuenta_google.Drive(cuenta_google.token(), "sonando.json")
                fid = d.buscar()
                texto = json.dumps(datos, ensure_ascii=False)
                d.escribir(fid, texto) if fid else d.crear(texto)
            except Exception:  # noqa: BLE001 (sin internet: ya se subirá la próxima)
                pass

        h = threading.Thread(target=hilo, daemon=True)
        h.start()
        if esperar:
            h.join(esperar)

    def al_salir(self) -> None:
        if self.r.posicion > 3:
            self._sonando_ultimo = ()
            self._subir_sonando(esperar=4)

    @Slot()
    def revisarOtroAparato(self) -> None:
        """Al abrir la app, al volver a la ventana y cada 5 min: ¿sonaba algo en el cel?"""
        if (not self._nube["correo"] or time.monotonic() - self._revisado_otro < 45
                or os.environ.get("VORTEX_PLAYER_SIN_RED")):
            return
        self._revisado_otro = time.monotonic()

        def hilo() -> None:
            try:
                d = cuenta_google.Drive(cuenta_google.token(), "sonando.json")
                fid = d.buscar()
                self._otro_aparato.emit(json.loads(d.leer(fid)) if fid else {})
            except Exception:  # noqa: BLE001
                pass

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_otro_aparato(self, d: dict) -> None:
        ahora = time.time() * 1000
        if (not d.get("id") or d.get("aparato") == self._aparato() or ahora - d.get("cuando", 0) > 12 * 3600 * 1000
                or d.get("cuando", 0) <= max(self._sonando_subido, float(self.bib.ajustes().get("seguir_visto", 0)))):
            return
        c = self.r.cancion
        if (c.get("id") or id_de(c.get("ruta", ""))) == d["id"] and abs(self.r.posicion - d.get("posicion", 0)) < 30:
            return
        self._seguir = d
        self.cambioSeguir.emit()

    @Slot()
    def seguirAqui(self) -> None:
        d, self._seguir = self._seguir, {}
        self.cambioSeguir.emit()
        if not d:
            return
        Biblioteca.guardar_ajuste("seguir_visto", d.get("cuando", 0))
        with self.candado:
            ruta = self.bib.rutas_por_id().get(d["id"])
            c = self.bib.cancion(ruta) if ruta else None
        if not c:   # no la tienes aquí: suena de YouTube Music, como las de la radio
            c = self._de_la_radio({"url": f"https://music.youtube.com/watch?v={d['id']}", "id": d["id"],
                                   "titulo": d.get("titulo", ""), "artistas": d.get("artista", ""),
                                   "album": d.get("album", ""), "duracion": d.get("duracion", 0), "portada": d.get("portada", "")})
        self.r.ponerOrigen("Desde " + d.get("nombre", "tu cel"))
        self.r.tocar_desde([c], 0, float(d.get("posicion", 0)))

    @Slot()
    def descartarSeguir(self) -> None:
        if self._seguir:
            Biblioteca.guardar_ajuste("seguir_visto", self._seguir.get("cuando", 0))
        self._seguir = {}
        self.cambioSeguir.emit()

    def _estado_seguir(self) -> dict:
        d = self._seguir
        if not d:
            return {}
        minutos = int((time.time() * 1000 - d.get("cuando", 0)) / 60000)
        return {**d, "hace": "hace un momento" if minutos < 1 else f"hace {minutos} min" if minutos < 60 else f"hace {minutos // 60} h"}

    # --- 1.5: lo nuevo de tus artistas -------------------------------------------------------------------

    @Slot()
    def revisarNovedades(self, forzar: bool = False) -> None:
        """Una vez al día: los álbumes y singles de este año de tus 5 artistas más escuchados."""
        hoy = time.strftime("%Y-%m-%d")
        if (not forzar and self._novedades.get("fecha") == hoy) or os.environ.get("VORTEX_PLAYER_SIN_RED"):
            return
        with self.candado:
            artistas = self.bib.artistas_mas_escuchados(time.time() - 60 * 86400, 5)
            claves = {descargas._clave(c["titulo"], c["artista"]) for c in self.bib.canciones()}
        ids = dict(self._novedades.get("ids") or {})
        anio = time.localtime().tm_year
        desde_anio = anio - 1 if time.localtime().tm_mon <= 2 else anio

        def hilo() -> None:
            lista = []
            for a in artistas:
                try:
                    canal = ids.get(a) or descargas.artista_id(a)
                    if not canal:
                        continue
                    ids[a] = canal
                    for x in descargas.lanzamientos(canal, a)[:8]:
                        if x["anio"] >= desde_anio and descargas._clave(x["titulo"], a) not in claves:
                            lista.append(x)
                except OSError:
                    return   # sin internet: mañana
            lista.sort(key=lambda x: -x["anio"])
            self._novedades_listas.emit(lista[:12], ids)

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_novedades(self, lista: list, ids: dict) -> None:
        vistos = set(self._novedades.get("vistos") or [])
        nuevas = [x for x in lista if x["id"] not in vistos]
        self._novedades = {"fecha": time.strftime("%Y-%m-%d"), "lista": list(lista), "ids": dict(ids),
                           "vistos": sorted(vistos | {x["id"] for x in lista})}
        self._guardar_json("novedades.json", self._novedades)
        self.cambioNovedades.emit()
        if nuevas:
            x = nuevas[0]
            texto = (f"Nuevo de {x['artista']}: «{x['titulo']}»" if len(nuevas) == 1
                     else f"{len(nuevas)} lanzamientos nuevos de tus artistas, como «{x['titulo']}» de {x['artista']}")
            self.aviso.emit(texto)
            if not plataforma.WINDOWS and shutil.which("notify-send"):
                subprocess.Popen(["notify-send", "-a", "Vortex Player", "-i", "vortex-player", "Lo nuevo de tus artistas", texto],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    @Slot(str, str)
    def lanzamiento(self, album: str, como: str) -> None:
        """«tocar»: suena entero sin bajarlo; «bajar»: se baja entero a tu biblioteca."""
        def hilo() -> None:
            try:
                lista, canciones = descargas.canciones_de_lanzamiento(album)
            except OSError:
                lista, canciones = "", []
            self._lanzamiento_listo.emit(como, album, lista, canciones)

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_lanzamiento(self, como: str, album: str, lista: str, canciones: list) -> None:
        x = next((x for x in self._novedades.get("lista") or [] if x["id"] == album), {})
        if not canciones:
            self.aviso.emit("Sin conexión con YouTube Music")
            return
        if como == "bajar":
            url = (f"https://music.youtube.com/playlist?list={lista}" if lista and len(canciones) > 1
                   else f"https://music.youtube.com/watch?v={canciones[0]['id']}")
            self.bajar(url, -1, "" if len(canciones) > 1 else canciones[0]["id"], f"{x.get('titulo', '')} · {x.get('artista', '')}")
            return
        cola = [self._de_la_radio({"url": f"https://music.youtube.com/watch?v={c['id']}", "id": c["id"], "titulo": c["titulo"],
                                   "artistas": x.get("artista", ""), "album": x.get("titulo", ""), "portada": x.get("portada", "")})
                for c in canciones]
        self.r.ponerOrigen(x.get("titulo") or "Lo nuevo de tus artistas")
        self.r.tocar(cola, 0)

    # --- 1.5: el despertador (con la app abierta) -----------------------------------------------------------

    @Slot("QVariantMap")
    def ponerDespertador(self, d: dict) -> None:
        """{"activo", "hora": "07:00", "playlist": -1 (tu mezcla del día) o el id de una}"""
        actual = dict(self.bib.ajustes().get("despertador") or {})
        actual.update({k: d[k] for k in ("activo", "hora", "playlist") if k in d})
        Biblioteca.guardar_ajuste("despertador", actual)
        self.cambioAjustes.emit()

    def _revisar_despertador(self) -> None:
        d = self.bib.ajustes().get("despertador") or {}
        hoy = time.strftime("%Y-%m-%d")
        if not d.get("activo") or d.get("hora") != time.strftime("%H:%M") or self._despertado == hoy:
            return
        self._despertado = hoy
        self.despertar(int(d.get("playlist", -1)))

    @Slot(int)
    def despertar(self, playlist: int = -1) -> None:
        """Suena tu mezcla del día (o esa playlist) subiendo el volumen poco a poco durante un minuto."""
        lista = self._canciones_mezcla() if playlist < 0 else self.bib.canciones_de_playlist(playlist)
        if not lista:
            with self.candado:
                lista = self.bib.canciones()
            random.shuffle(lista)
        if not lista:
            return
        meta = max(30, self.r._volumen)
        self.r.ponerVolumen(5)
        self.r.ponerOrigen("Despertador")
        self.r.tocar(lista, 0)
        self.mostrarVentana.emit()
        pasos = [0]

        def subir() -> None:
            pasos[0] += 1
            self.r.ponerVolumen(5 + round((meta - 5) * pasos[0] / 30))
            if pasos[0] < 30:
                QTimer.singleShot(2000, subir)

        QTimer.singleShot(2000, subir)

    # --- 1.5: «Iniciar radio» y compartir ------------------------------------------------------------------

    @Slot("QVariantMap")
    def iniciarRadio(self, c: dict) -> None:
        """Toca esta canción y detrás, sin fin, parecidas a ella (como «Ir a la radio» de Spotify)."""
        if not c or not c.get("ruta"):
            return
        self.r.tocar([c], 0)
        self.r.ponerOrigen("Radio de " + (c.get("titulo") or Path(c["ruta"]).stem))
        self._radio_para = ""
        self._pedir_radio()
        self.aviso.emit("Radio de «" + (c.get("titulo") or "esta canción") + "»")

    @Slot("QVariantMap", str)
    def compartir(self, c: dict, como: str) -> None:
        """«copiar» el enlace de YouTube Music o mandarlo por «whatsapp». Las tuyas sin id se buscan primero."""
        if not c or not c.get("ruta"):
            return
        titulo, artista = c.get("titulo") or Path(c["ruta"]).stem, c.get("artista") or ""
        texto = f"{titulo} · {artista}" if artista else titulo
        vid = c.get("id") or id_de(c["ruta"])
        if vid:
            self._fin_enlace(como, f"https://music.youtube.com/watch?v={vid}", texto)
            return

        def hilo() -> None:
            try:
                r = descargas.buscar(f"{artista} {titulo}", "ytmusic", 1)
            except OSError:
                r = []
            self._enlace_listo.emit(como, r[0]["url"] if r else "", texto)

        threading.Thread(target=hilo, daemon=True).start()

    @Slot(str)
    def compartirPlaylist(self, como: str) -> None:
        """1.6: la playlist que estás viendo como enlace de YouTube («copiar» o «whatsapp»). Lo abre
        cualquiera; en Vortex Player se pega en «Agregar música» y se baja con su nombre."""
        canciones, nombre = list(self._canciones), self._origen()
        if not canciones:
            return
        self.aviso.emit("Armando el enlace…")

        def hilo() -> None:
            ids = []
            for c in canciones:
                vid = c.get("id") or id_de(c["ruta"])
                if not vid:   # las tuyas sin id: se buscan en YouTube Music, como al compartir una canción
                    try:
                        r = descargas.buscar(f"{c.get('artista') or ''} {c.get('titulo') or Path(c['ruta']).stem}",
                                             "ytmusic", 1)
                    except OSError:
                        r = []
                    vid = r[0]["id"] if r else ""
                ids.append(vid)
            self._enlace_listo.emit(como, "\n".join(descargas.enlaces_playlist(nombre, ids)), nombre)

        threading.Thread(target=hilo, daemon=True).start()

    def _fin_enlace(self, como: str, url: str, texto: str) -> None:
        if not url:
            self.aviso.emit("No encontré esta canción en YouTube Music")
        elif como == "whatsapp":
            plataforma.abrir("https://wa.me/?text=" + quote(f"{texto} {url}"))
        else:
            QGuiApplication.clipboard().setText(url)
            self.aviso.emit("Enlace copiado" if "\n" not in url else f"{url.count(chr(10)) + 1} enlaces copiados")

    @Slot(str)
    def bajarDeLaRadio(self, ruta: str) -> None:
        """La de la radio que suena (o una de la cola), a tu biblioteca."""
        c = next((x for x in self.r.cola if x.get("ruta") == ruta and x.get("radio")), None)
        if not c or ruta in self._bajando_radio:
            return
        self._bajando_radio.append(ruta)
        self.bajar(ruta, -1, c.get("id", ""), f"{c['titulo']} · {c['artista']}" if c.get("artista") else c["titulo"])
        self.aviso.emit("Bajando a tu biblioteca")

    @Slot(int)
    def ponerFundido(self, segundos: int) -> None:
        Biblioteca.guardar_ajuste("fundido", int(segundos))
        self.r.ponerFundido(float(segundos))
        self.cambioAjustes.emit()

    # --- 2.4: las letras de toda tu música, para verlas sin internet -------------------------------------

    @Slot()
    def bajarLetras(self) -> None:
        """Busca (despacito, en un hilo) la letra de cada canción que todavía no tiene; se reanuda sola."""
        if self._letras_estado["trabajando"]:
            return
        carpeta = rutas.cache() / "letras"
        with self.candado:
            todas = self.bib.canciones()
        self._letras_estado = {**self._letras_estado, "trabajando": True}

        def hilo() -> None:
            con = 0
            faltan = []
            for c in todas:
                args = (c["titulo"] or Path(c["ruta"]).stem, c["artista"] or "", int(c.get("duracion") or 0))
                if letras.guardada(carpeta, *args):
                    con += 1
                else:
                    faltan.append((args, id_de(c["ruta"])))
            self._letras_avance.emit({"con": con, "total": len(todas), "trabajando": bool(faltan)})
            for args, vid in faltan:
                try:
                    letras.de(carpeta, *args, vid, descargas.letra_youtube)
                except OSError:
                    break   # sin internet (o LRCLIB caído): otro día
                con += 1
                self._letras_avance.emit({"con": con, "total": len(todas), "trabajando": True})
                time.sleep(1.5)   # sin prisa: LRCLIB es gratis y de una persona
            self._letras_avance.emit({"con": con, "total": len(todas), "trabajando": False})

        threading.Thread(target=hilo, daemon=True).start()

    def _en_letras(self, e: dict) -> None:
        self._letras_estado = e
        self.cambioLetras.emit()

    # --- 2.4: el color de una portada (el degradado de la playlist y de la letra, como Spotify) ---------

    @Slot(str, result=str)
    def colorDe(self, ruta: str) -> str:
        """El promedio de los pixeles con más color de la portada, oscurecido para que se lea el texto."""
        if ruta in self._colores:
            return self._colores[ruta]
        url = self.portada_de(ruta)
        if not url:
            return ""
        img = QImage(QUrl(url).toLocalFile())
        if img.isNull():
            return ""
        img = img.scaled(24, 24)
        r = g = b = peso = 0.0
        for x in range(img.width()):
            for y in range(img.height()):
                c = img.pixelColor(x, y)
                w = 0.05 + c.hsvSaturationF() ** 2 * c.valueF()
                r, g, b, peso = r + c.redF() * w, g + c.greenF() * w, b + c.blueF() * w, peso + w
        c = QColor.fromRgbF(r / peso, g / peso, b / peso)
        h, sat, v = c.hsvHueF(), min(0.75, c.hsvSaturationF() * 1.2), min(0.45, max(0.28, c.valueF()))
        self._colores[ruta] = QColor.fromHsvF(max(0.0, h), sat, v).name()
        return self._colores[ruta]

    @Slot(str)
    def ponerPerfil(self, nombre: str) -> None:
        """2.4: un perfil del ecualizador (los mismos que en el teléfono) sobre tus bandas."""
        if nombre not in ecualizador.PERFILES:
            return
        bandas = ecualizador.bandas_de_perfil(nombre, self._eq["bandas"])
        self._eq = {**self._eq, "bandas": bandas, "preset": "Custom"}
        self._eq_local = time.monotonic()
        self.cambioEq.emit()
        self._aplicar_bandas()

    # --- propiedades para QML --------------------------------------------------------------------

    def _ajustes(self) -> dict:
        a = self.bib.ajustes()
        return {**a, "musica": str(rutas.musica()), "descargas": str(rutas.descargas()),
                "audioPendiente": getattr(self, "_audio_pendiente", False),
                "hayEq": ecualizador.disponible(), "hayYtDlp": shutil.which("yt-dlp") is not None,
                "navegadores": NAVEGADORES}

    canciones = Property("QVariantList", lambda s: s._canciones, notify=cambioCanciones)
    playlists = Property("QVariantList", lambda s: s._playlists, notify=cambioPlaylists)
    vistaPlaylist = Property(int, lambda s: s._vista_playlist, notify=cambioCanciones)
    descargasLista = Property("QVariantList", lambda s: s._descargas, notify=cambioDescargas)
    eq = Property("QVariantMap", lambda s: s._eq, notify=cambioEq)
    presets = Property("QStringList", lambda s: ecualizador.PRESETS, constant=True)
    salida = Property("QVariantMap", lambda s: s._salida, notify=cambioSalida)
    ajustes = Property("QVariantMap", _ajustes, notify=cambioAjustes)
    escaneando = Property(bool, lambda s: s._escaneando, notify=cambioEscaneo)
    enLinea = Property("QVariantList", lambda s: s._con_estado(s._en_linea), notify=cambioEnLinea)
    recomendaciones = Property("QVariantList", lambda s: s._sin_las_tuyas(s._recom), notify=cambioRecomendaciones)
    recomendandoPara = Property(int, lambda s: s._recom_para, notify=cambioRecomendaciones)
    cargandoRecomendaciones = Property(bool, lambda s: s._recom_cargando, notify=cambioRecomendaciones)
    errorRecomendaciones = Property(str, lambda s: s._recom_error, notify=cambioRecomendaciones)
    buscandoEnLinea = Property(bool, lambda s: s._buscando_linea, notify=cambioEnLinea)
    errorEnLinea = Property(str, lambda s: s._error_linea, notify=cambioEnLinea)
    nube = Property("QVariantMap", _estado_nube, notify=cambioNube)
    reproductor = Property(QObject, lambda s: s.r, constant=True)
    letra = Property("QVariantMap", lambda s: s._letra, notify=cambioLetra)
    paraTi = Property("QVariantList", lambda s: s._sin_las_tuyas(s._para_ti), notify=cambioParaTi)
    paraTiDe = Property("QStringList", lambda s: s._para_ti_de, notify=cambioParaTi)
    cargandoParaTi = Property(bool, lambda s: s._para_ti_cargando, notify=cambioParaTi)
    errorParaTi = Property(str, lambda s: s._para_ti_error, notify=cambioParaTi)
    perfiles = Property("QStringList", lambda s: list(ecualizador.PERFILES), constant=True)
    perfilEq = Property(str, lambda s: ecualizador.perfil_de(s._eq["bandas"]), notify=cambioEq)
    avance = Property(QObject, lambda s: s._avance, constant=True)
    meGusta = Property("QStringList", lambda s: s._me_gusta, notify=cambioPlaylists)
    letrasGuardadas = Property("QVariantMap", lambda s: s._letras_estado, notify=cambioLetras)
    resumen = Property("QVariantMap", _estado_resumen, notify=cambioResumen)
    mezcla = Property("QVariantMap", _estado_mezcla, notify=cambioMezcla)
    seguir = Property("QVariantMap", _estado_seguir, notify=cambioSeguir)
    novedades = Property("QVariantList", lambda s: s._novedades.get("lista") or [], notify=cambioNovedades)
    bajandoRadio = Property("QStringList", lambda s: s._bajando_radio, notify=cambioDescargas)


def uri(ruta: str) -> str:
    return "file://" + quote(ruta)
