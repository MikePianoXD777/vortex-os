"""MPRIS: Vortex Player se anuncia en D-Bus como reproductor (org.mpris.MediaPlayer2.vortex_player).

Así lo ven el panel de música del escritorio (con «Vía Vortex Player», la portada, la barra de tiempo y
los botones), las teclas multimedia y cualquier otro control. Usa dbus-python con el bucle de GLib, que
es el mismo bucle de eventos de Qt en Linux. Si falta dbus-python, la app funciona igual sin MPRIS.
"""
from __future__ import annotations

from pathlib import Path
from urllib.parse import quote

try:
    import dbus
    import dbus.service
    from dbus.mainloop.glib import DBusGMainLoop
except ImportError:  # pragma: no cover - sin dbus-python
    dbus = None

RAIZ = "org.mpris.MediaPlayer2"
PLAYER = "org.mpris.MediaPlayer2.Player"
PROPS = "org.freedesktop.DBus.Properties"
RUTA = "/org/mpris/MediaPlayer2"


def _uri(ruta: str) -> str:
    return "file://" + quote(ruta) if ruta else ""


def metadatos(c: dict, portada: str = "") -> dict:
    """Los xesam/mpris de la canción (en tipos de Python; se pasan a D-Bus en `_dbus_meta`)."""
    if not c:
        return {}
    m = {"mpris:trackid": "/org/vortex/player/cancion/%d" % (abs(hash(c.get("ruta", ""))) % 10**12),
         "mpris:length": int(float(c.get("duracion") or 0) * 1_000_000),
         "xesam:title": c.get("titulo") or Path(c.get("ruta", "")).stem,
         "xesam:artist": [a.strip() for a in (c.get("artista") or "").split(",") if a.strip()] or [""],
         "xesam:album": c.get("album") or "",
         "xesam:url": _uri(c.get("ruta", ""))}
    if portada:
        m["mpris:artUrl"] = portada
    return m


if dbus is not None:
    class Mpris(dbus.service.Object):
        def __init__(self, reproductor, portada_de, levantar):
            DBusGMainLoop(set_as_default=True)
            self.bus = dbus.SessionBus()
            self.nombre = dbus.service.BusName(RAIZ + ".vortex_player", self.bus)
            super().__init__(self.bus, RUTA)
            self.r = reproductor
            self.portada_de = portada_de        # ruta → URL de la portada ("" si no hay)
            self.levantar = levantar            # trae la ventana al frente
            self._estado_previo = None
            reproductor.cambioCancion.connect(lambda: self._cambiaron(["Metadata", "CanGoNext", "CanGoPrevious"]))
            reproductor.cambioEstado.connect(self._estado)

        # --- propiedades -------------------------------------------------------------------------

        def _estado_texto(self) -> str:
            if not self.r.cancion:
                return "Stopped"
            return "Paused" if self.r.pausado else "Playing"

        def _dbus_meta(self) -> dbus.Dictionary:
            c = self.r.cancion
            m = metadatos(c, self.portada_de(c.get("ruta", "")) if c else "")
            d = dbus.Dictionary({}, signature="sv")
            for k, v in m.items():
                if k == "mpris:trackid":
                    d[k] = dbus.ObjectPath(v)
                elif k == "mpris:length":
                    d[k] = dbus.Int64(v)
                elif isinstance(v, list):
                    d[k] = dbus.Array(v, signature="s")
                else:
                    d[k] = dbus.String(v)
            return d

        def _todas(self, interfaz: str) -> dict:
            if interfaz == RAIZ:
                return {"CanQuit": True, "CanRaise": True, "HasTrackList": False, "Identity": "Vortex Player",
                        "DesktopEntry": "vortex-player",
                        "SupportedUriSchemes": dbus.Array(["file"], signature="s"),
                        "SupportedMimeTypes": dbus.Array(["audio/flac", "audio/x-wav", "audio/mpeg", "audio/ogg",
                                                          "audio/opus", "audio/mp4", "audio/x-aiff"], signature="s")}
            if interfaz == PLAYER:
                return {"PlaybackStatus": self._estado_texto(),
                        "LoopStatus": {"no": "None", "todo": "Playlist", "una": "Track"}[self.r.repetir],
                        "Shuffle": dbus.Boolean(self.r.aleatorio), "Rate": dbus.Double(1.0),
                        "Metadata": self._dbus_meta(), "Volume": dbus.Double(self.r.volumen / 100),
                        "Position": dbus.Int64(int(self.r.posicion * 1_000_000)),
                        "MinimumRate": dbus.Double(1.0), "MaximumRate": dbus.Double(1.0),
                        "CanGoNext": dbus.Boolean(self.r.haySiguiente), "CanGoPrevious": dbus.Boolean(self.r.hayAnterior),
                        "CanPlay": dbus.Boolean(bool(self.r.cancion)), "CanPause": dbus.Boolean(bool(self.r.cancion)),
                        "CanSeek": dbus.Boolean(bool(self.r.cancion)), "CanControl": True}
            return {}

        def _cambiaron(self, nombres: list[str]) -> None:
            todas = self._todas(PLAYER)
            self.PropertiesChanged(PLAYER, {n: todas[n] for n in nombres}, [])

        def _estado(self) -> None:
            ahora = (self._estado_texto(), self.r.repetir, self.r.aleatorio, self.r.volumen)
            if ahora != self._estado_previo:
                self._estado_previo = ahora
                self._cambiaron(["PlaybackStatus", "LoopStatus", "Shuffle", "Volume", "CanPlay", "CanPause"])

        @dbus.service.method(PROPS, in_signature="ss", out_signature="v")
        def Get(self, interfaz, nombre):
            return self._todas(interfaz)[nombre]

        @dbus.service.method(PROPS, in_signature="s", out_signature="a{sv}")
        def GetAll(self, interfaz):
            return self._todas(interfaz)

        @dbus.service.method(PROPS, in_signature="ssv")
        def Set(self, interfaz, nombre, valor):
            if nombre == "Volume":
                self.r.ponerVolumen(int(float(valor) * 100))
            elif nombre == "Shuffle" and bool(valor) != self.r.aleatorio:
                self.r.alternarAleatorio()
            elif nombre == "LoopStatus":
                quiere = {"None": "no", "Playlist": "todo", "Track": "una"}.get(str(valor), "no")
                for _ in range(3):
                    if self.r.repetir == quiere:
                        break
                    self.r.alternarRepetir()

        @dbus.service.signal(PROPS, signature="sa{sv}as")
        def PropertiesChanged(self, interfaz, cambiadas, invalidadas):
            pass

        # --- org.mpris.MediaPlayer2 ---------------------------------------------------------------

        @dbus.service.method(RAIZ)
        def Raise(self):
            self.levantar()

        @dbus.service.method(RAIZ)
        def Quit(self):
            from PySide6.QtCore import QCoreApplication
            QCoreApplication.quit()

        # --- org.mpris.MediaPlayer2.Player --------------------------------------------------------

        @dbus.service.method(PLAYER)
        def Next(self):
            self.r.siguiente()

        @dbus.service.method(PLAYER)
        def Previous(self):
            self.r.anterior()

        @dbus.service.method(PLAYER)
        def Pause(self):
            self.r.pausar()

        @dbus.service.method(PLAYER)
        def PlayPause(self):
            self.r.alternar()

        @dbus.service.method(PLAYER)
        def Stop(self):
            self.r.pausar()
            self.r.buscar(0)

        @dbus.service.method(PLAYER)
        def Play(self):
            self.r.seguir()

        @dbus.service.method(PLAYER, in_signature="x")
        def Seek(self, offset):
            self.r.buscar(self.r.posicion + offset / 1_000_000)
            self.Seeked(dbus.Int64(int(self.r.posicion * 1_000_000)))

        @dbus.service.method(PLAYER, in_signature="ox")
        def SetPosition(self, _pista, posicion):
            self.r.buscar(posicion / 1_000_000)
            self.Seeked(dbus.Int64(int(posicion)))

        @dbus.service.method(PLAYER, in_signature="s")
        def OpenUri(self, uri):
            pass

        @dbus.service.signal(PLAYER, signature="x")
        def Seeked(self, posicion):
            pass
else:  # pragma: no cover
    Mpris = None
