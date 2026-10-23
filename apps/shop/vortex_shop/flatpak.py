"""Instalar, quitar y actualizar apps de Flathub para TU usuario (flatpak --user): sin contraseña,
sin polkit y sin terminal.

Dos formas de hablar con Flatpak, la misma interfaz:

  1. libflatpak por GObject Introspection (python-gobject, que ya viene en la ISO): da el progreso
     real de la descarga y los errores sin tener que leer texto. Es la que se usa si está.
  2. La CLI «flatpak --noninteractive», corriendo en una pseudo-terminal para que Flatpak dibuje sus
     barras de progreso y de ahí se saca el porcentaje. Respaldo si no hay libflatpak, o a la fuerza
     con VORTEX_SHOP_FLATPAK=cli.

Las apps que ya están a nivel sistema (las que pone vortex-flatpaks al arrancar: Sober, Heroic,
Spotify...) cuentan como instaladas y se pueden abrir, pero no se pueden quitar desde aquí: quitarlas
pide ser administrador. Los avisos y errores son siempre mensajes en español para mostrar tal cual.

Las funciones que cambian algo (instalar, quitar, actualizar) corren en el hilo que las llama y van
avisando por progreso(porcentaje, texto) desde ese mismo hilo; la interfaz las llama desde un hilo
de trabajo. De a una por vez: si llega otra mientras corre una, espera su turno.
"""
import contextlib
import errno
import fcntl
import os
import pty
import re
import select
import shutil
import struct
import subprocess
import termios
import threading

from .errores import AppNoEncontrada, ErrorFlatpak, FlatpakSinRed
from .ids import es_id_valido, validar_id

REMOTO = "flathub"
URL_REPO = "https://dl.flathub.org/repo/flathub.flatpakrepo"
RAMA = "stable"
# Bit «NON_FATAL» de TransactionErrorDetails (el valor es 1; PyGObject lo muestra con el nombre «FATAL»).
_NO_FATAL = 1

_CERROJO = threading.Lock()
_NO_CARGADO = object()
_gi = _NO_CARGADO       # (Flatpak, GLib) de gi.repository, None si no hay; las pruebas lo cambian


# ---------------------------------------------------------------------- utilidades

def _validar(app_id):
    try:
        return validar_id(app_id)
    except ValueError as e:
        raise AppNoEncontrada(str(e)) from None


def _es_app(app_id):
    """Los BaseApp (io.qt.qtwebengine.BaseApp...) son andamios de otras apps, no se abren ni se muestran."""
    return es_id_valido(app_id) and not app_id.endswith(".BaseApp")


class _Avance:
    """El progreso(porcentaje, texto) de la interfaz, a prueba de tonterías: entero de 0 a 100, y una
    falla de la interfaz no tumba la instalación."""

    def __init__(self, progreso):
        self._progreso = progreso
        self.ultimo = (-1, "")

    def __call__(self, porcentaje, texto):
        porcentaje = max(0, min(100, int(porcentaje)))
        if (porcentaje, texto) == self.ultimo:
            return
        self.ultimo = (porcentaje, texto)
        if self._progreso is not None:
            try:
                self._progreso(porcentaje, texto)
            except Exception:       # noqa: BLE001 - lo que haga mal la interfaz no es problema de Flatpak
                pass


@contextlib.contextmanager
def _turno(avance):
    """Una operación a la vez (Flatpak bloquea el repositorio de todos modos)."""
    if not _CERROJO.acquire(blocking=False):
        avance(0, "Esperando a que termine otra operación…")
        _CERROJO.acquire()
    try:
        yield
    finally:
        _CERROJO.release()


_RED = re.compile(
    r"could not resolve|unable to connect|network is unreachable|temporary failure in name|name or service not known"
    r"|connection (refused|reset|timed out)|timed out|no route to host|failed to connect|unable to load summary"
    r"|error (while|when) (downloading|fetching)|while fetching|no se pudo resolver|no se puede conectar"
    r"|no se pudo conectar|sin conexi|error de red|tiempo de espera|nombre o servicio no conocido|red inaccesible",
    re.I)
_ESPACIO = re.compile(
    r"no space left|not enough disk space|insufficient (disk )?space|no hay espacio|espacio (insuficiente|en disco)"
    r"|sin espacio|no queda espacio", re.I)
_NO_ESTA = re.compile(
    r"no remote refs found|nothing matches|not found in remote|no matches? found|no se encontr|ninguna coincidencia"
    r"|ningún elemento coincide|no hay refs", re.I)
_YA_ESTA = re.compile(r"already installed|ya est[aá] instalad", re.I)
_PERMISO = re.compile(r"permission denied|not allowed|authentication|polkit|permiso denegado", re.I)


def _q(nombre):
    return "«%s»" % nombre


def _error(texto, accion, objeto):
    """Convierte lo que dijo Flatpak (en inglés o en el idioma del sistema) en un error en español.
    objeto es lo que se estaba tocando, ya con comillas si es una app: «Steam» o tus aplicaciones."""
    texto = (texto or "").strip()
    if _RED.search(texto):
        return FlatpakSinRed()
    if _ESPACIO.search(texto):
        return ErrorFlatpak("No hay espacio suficiente en el disco para %s %s. Libera espacio e inténtalo de nuevo."
                            % (accion, objeto))
    if _NO_ESTA.search(texto):
        return AppNoEncontrada("Flathub no tiene %s." % objeto)
    if _PERMISO.search(texto):
        return ErrorFlatpak("No hay permiso para %s %s: es del sistema, no de tu usuario." % (accion, objeto))
    lineas = [renglon.strip() for renglon in texto.splitlines() if renglon.strip()]
    # La última línea que diga «error» (Flatpak la deja al final), sin el «error:» de adelante.
    motivo = next((renglon for renglon in reversed(lineas) if re.search(r"error", renglon, re.I)),
                  lineas[-1] if lineas else "")
    motivo = re.sub(r"^(error|Error)\s*:\s*", "", motivo)[:200]
    return ErrorFlatpak("No se pudo %s %s.%s" % (accion, objeto, (" " + motivo) if motivo else ""))


_ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b[()][A-Za-z0-9]")


# ---------------------------------------------------------------------- libflatpak (gi)

def _libflatpak():
    """(Flatpak, GLib) de GObject Introspection, o None si no hay libflatpak para Python."""
    global _gi
    if _gi is _NO_CARGADO:
        _gi = None
        if os.environ.get("VORTEX_SHOP_FLATPAK", "").lower() != "cli":
            try:
                import gi
                gi.require_version("Flatpak", "1.0")
                from gi.repository import Flatpak, GLib
                _gi = (Flatpak, GLib)
            except (ImportError, ValueError):
                _gi = None
    return _gi


def _gi_usuario(gi):
    return gi[0].Installation.new_user(None)


def _gi_sistema(gi):
    return gi[0].Installation.new_system(None)


def _gi_apps(gi, instalacion):
    """Las InstalledRef de apps (no runtimes) de una instalación."""
    try:
        refs = instalacion.list_installed_refs_by_kind(gi[0].RefKind.APP, None)
    except gi[1].Error:
        return []
    return [r for r in refs if _es_app(r.get_name())]


def _gi_buscar(gi, app_id):
    """(ref_de_usuario, ref_de_sistema): cada una es la InstalledRef o None."""
    de_usuario = next((r for r in _gi_apps(gi, _gi_usuario(gi)) if r.get_name() == app_id), None)
    de_sistema = next((r for r in _gi_apps(gi, _gi_sistema(gi)) if r.get_name() == app_id), None)
    return de_usuario, de_sistema


def _nombre_de(ref, app_id):
    try:
        return ref.get_appdata_name() or app_id
    except Exception:       # noqa: BLE001
        return app_id


class _TransaccionGi:
    """Una transacción de libflatpak con su progreso: junta las señales (nueva operación, avance,
    operación hecha, error) y las convierte en porcentaje y texto. El porcentaje pondera cada operación
    por su tamaño de descarga, para que un runtime grande no parezca "la mitad" cuando es el 95 %."""

    def __init__(self, gi, instalacion, avance, accion):
        self.gi = gi
        self.avance = avance
        self.accion = accion        # "instalar", "quitar" o "actualizar"
        self.pesos = {}
        self.total = 1.0
        self.hecho = 0.0
        self.error = None
        self.tr = gi[0].Transaction.new_for_installation(instalacion, None)
        self.tr.set_no_interaction(True)
        self.tr.connect("ready", self._al_estar_lista)
        self.tr.connect("new-operation", self._al_empezar)
        self.tr.connect("operation-done", self._al_terminar)
        self.tr.connect("operation-error", self._al_fallar)
        self.tr.connect("choose-remote-for-ref", self._elegir_remoto)
        self.tr.connect("add-new-remote", lambda *a: False)

    # señales: todas devuelven lo que libflatpak espera (True = seguir)
    def _al_estar_lista(self, tr):
        self.pesos = {}
        for op in tr.get_operations():
            try:
                tam = op.get_download_size() or 0
            except Exception:       # noqa: BLE001
                tam = 0
            self.pesos[op.get_ref()] = float(tam) + 1.0
        self.total = sum(self.pesos.values()) or 1.0
        return True

    def _fraccion(self, ref, fraccion):
        return min(99, int((self.hecho + self.pesos.get(ref, 1.0) * fraccion) / self.total * 100))

    def _al_empezar(self, tr, op, progreso):
        ref = op.get_ref()
        quitando = op.get_operation_type() == self.gi[0].TransactionOperationType.UNINSTALL
        if quitando:
            self.avance(self._fraccion(ref, 0), "Quitando…")
        else:
            self.avance(self._fraccion(ref, 0), "Preparando…")
        try:
            progreso.set_update_frequency(250)
        except Exception:       # noqa: BLE001
            pass

        def al_avanzar(prog):
            if quitando:
                return
            if prog.get_is_estimating():
                self.avance(self._fraccion(ref, 0), "Preparando…")
                return
            por_ciento = prog.get_progress()
            if por_ciento >= 100:
                # Ya se bajó todo; falta dejarlo en su sitio.
                self.avance(self._fraccion(ref, 1), "Instalando…" if self.accion != "actualizar" else "Actualizando…")
            else:
                self.avance(self._fraccion(ref, por_ciento / 100.0), "Descargando… %d %%" % por_ciento)
        progreso.connect("changed", al_avanzar)

    def _al_terminar(self, tr, op, commit, resultado):
        self.hecho += self.pesos.get(op.get_ref(), 1.0)
        self.avance(min(99, int(self.hecho / self.total * 100)),
                    "Quitando…" if self.accion == "quitar" else "Instalando…")

    def _al_fallar(self, tr, op, error, detalles):
        self.error = error.message if hasattr(error, "message") else str(error)
        return bool(int(detalles) & _NO_FATAL)      # los fallos de extras opcionales no frenan

    def _elegir_remoto(self, tr, ref, runtime_ref, remotos):
        # El runtime que necesita la app se baja del mismo Flathub.
        return remotos.index(REMOTO) if REMOTO in remotos else 0

    def correr(self, objeto):
        try:
            self.tr.run(None)
        except self.gi[1].Error as e:
            mensaje = self.error or getattr(e, "message", str(e))
            if _YA_ESTA.search(mensaje or ""):
                return
            raise _error(mensaje, self.accion, objeto) from None


def _gi_instaladas(gi):
    vistas = {r.get_name() for inst in (_gi_usuario(gi), _gi_sistema(gi)) for r in _gi_apps(gi, inst)}
    return sorted(vistas)


def _gi_actualizaciones(gi):
    try:
        refs = _gi_usuario(gi).list_installed_refs_for_update(None)
    except gi[1].Error as e:
        raise _error(getattr(e, "message", str(e)), "buscar actualizaciones de", "tus aplicaciones") from None
    return sorted({r.get_name() for r in refs if r.get_kind() == gi[0].RefKind.APP and _es_app(r.get_name())})


def _gi_instalar(gi, app_id, avance):
    flatpak = gi[0]
    ref = "app/%s/%s/%s" % (app_id, flatpak.get_default_arch(), RAMA)
    t = _TransaccionGi(gi, _gi_usuario(gi), avance, "instalar")
    try:
        t.tr.add_install(REMOTO, ref, None)
    except gi[1].Error as e:
        raise _error(getattr(e, "message", str(e)), "instalar", _q(app_id)) from None
    t.correr(_q(app_id))


def _gi_quitar(gi, ref, avance):
    t = _TransaccionGi(gi, _gi_usuario(gi), avance, "quitar")
    try:
        # Con los runtimes que ya nadie usa: así quitar una app libera de verdad el espacio.
        t.tr.set_include_unused_uninstall_ops(True)
    except (AttributeError, TypeError):     # libflatpak viejo: queda solo la app
        pass
    try:
        t.tr.add_uninstall(ref.format_ref())
    except gi[1].Error as e:
        raise _error(getattr(e, "message", str(e)), "quitar", _q(ref.get_name())) from None
    t.correr(_q(_nombre_de(ref, ref.get_name())))


def _gi_actualizar(gi, app_ids, avance):
    usuario = _gi_usuario(gi)
    try:
        refs = usuario.list_installed_refs_for_update(None)
    except gi[1].Error as e:
        raise _error(getattr(e, "message", str(e)), "buscar actualizaciones de", "tus aplicaciones") from None
    if app_ids is not None:
        refs = [r for r in refs if r.get_kind() == gi[0].RefKind.APP and r.get_name() in app_ids]
    if not refs:
        return False
    t = _TransaccionGi(gi, usuario, avance, "actualizar")
    try:
        for r in refs:
            t.tr.add_update(r.format_ref(), None, None)
    except gi[1].Error as e:
        raise _error(getattr(e, "message", str(e)), "actualizar", "tus aplicaciones") from None
    t.correr("tus aplicaciones")
    return True


# ---------------------------------------------------------------------- CLI de flatpak

def _exe():
    exe = shutil.which("flatpak")
    if not exe:
        raise ErrorFlatpak("Flatpak no está instalado en este equipo, así que la tienda no puede instalar aplicaciones.")
    return exe


def _entorno():
    # Los mensajes de flatpak en inglés y sin adornos Unicode: así los patrones de arriba los entienden.
    entorno = dict(os.environ)
    entorno.update(LC_ALL="C", LANG="C", LANGUAGE="C")
    if entorno.get("TERM", "dumb") == "dumb":
        entorno["TERM"] = "xterm"       # con «dumb» flatpak no dibuja la barra de progreso
    entorno.pop("PYTHONPATH", None)
    return entorno


def _cli_lineas(argumentos, tiempo=120):
    """Salida (líneas) de «flatpak ...» en solo lectura. ErrorFlatpak si falla."""
    try:
        r = subprocess.run([_exe()] + argumentos, stdin=subprocess.DEVNULL, capture_output=True,
                           text=True, errors="replace", timeout=tiempo, env=_entorno(), check=False)
    except subprocess.TimeoutExpired:
        raise ErrorFlatpak("Flatpak tardó demasiado en responder. Inténtalo de nuevo.") from None
    except OSError as e:
        raise ErrorFlatpak("No se pudo ejecutar Flatpak: %s" % e.strerror) from None
    if r.returncode != 0:
        raise _error(r.stderr or r.stdout, "consultar", "tus aplicaciones")
    return [renglon.strip() for renglon in r.stdout.splitlines()]


def _cli_ids(argumentos):
    return sorted({renglon for renglon in _cli_lineas(argumentos) if _es_app(renglon)})


def _cli_instaladas(donde=None):
    return _cli_ids(["list", "--app", "--columns=application"] + ([donde] if donde else []))


def _cli_correr(argumentos, avance, accion, objeto, texto_descarga="Descargando…"):
    """Corre «flatpak ...» en una pseudo-terminal (para que dibuje su barra de progreso), saca los
    porcentajes de lo que escribe y los pasa a avance. Si falla, lanza el error en español."""
    exe = _exe()
    maestro, esclavo = pty.openpty()
    # Una terminal ancha: con menos de unas 40 columnas flatpak no dibuja la barra.
    fcntl.ioctl(esclavo, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 200, 0, 0))
    try:
        proceso = subprocess.Popen([exe] + argumentos, stdin=subprocess.DEVNULL, stdout=esclavo, stderr=esclavo,
                                   close_fds=True, env=_entorno(), start_new_session=True)
    except OSError as e:
        os.close(maestro)
        os.close(esclavo)
        raise ErrorFlatpak("No se pudo ejecutar Flatpak: %s" % e.strerror) from None
    os.close(esclavo)

    cola, pendiente = [], ""
    op_actual, op_total = 1, 1
    por_ciento = 0
    avance(0, "Preparando…")
    try:
        while True:
            try:
                listos, _, _ = select.select([maestro], [], [], 0.5)
            except InterruptedError:
                continue
            if not listos:
                if proceso.poll() is not None:
                    break
                continue
            try:
                trozo = os.read(maestro, 4096)
            except OSError as e:
                if e.errno == errno.EIO:        # el otro lado de la terminal se cerró: terminó
                    break
                raise
            if not trozo:
                break
            pendiente += _ANSI.sub("", trozo.decode("utf-8", "replace"))
            *completos, pendiente = re.split(r"[\r\n]", pendiente)
            for linea in completos + [pendiente]:
                if not linea.strip():
                    continue
                m = re.search(r"(Installing|Updating|Uninstalling)\s+(\d+)/(\d+)", linea)
                if m:
                    op_actual, op_total = max(1, int(m.group(2))), max(1, int(m.group(3)))
                p = re.search(r"(\d{1,3})\s?%", linea)
                if re.search(r"Uninstalling", linea):
                    avance(int(min(99, (op_actual - 1) * 100 / op_total)), "Quitando…")
                elif not p and m:
                    # sin barra (terminal sin adornos): al menos cuántas operaciones van
                    avance(int(min(99, (op_actual - 1) * 100 / op_total)),
                           "Instalando… %d de %d" % (op_actual, op_total) if op_total > 1 else "Instalando…")
                elif p:
                    por_ciento = min(100, int(p.group(1)))
                    total = ((op_actual - 1) + por_ciento / 100.0) / op_total * 100
                    if por_ciento >= 100:
                        avance(int(min(99, total)), "Instalando…" if accion != "actualizar" else "Actualizando…")
                    else:
                        avance(int(min(99, total)), "%s %d %%" % (texto_descarga, por_ciento))
                if linea is not pendiente:
                    cola.append(linea.strip())
            cola = cola[-60:]
    finally:
        os.close(maestro)
        proceso.wait()
    if proceso.returncode != 0:
        salida = "\n".join(cola + ([pendiente.strip()] if pendiente.strip() else []))
        error = _error(salida, accion, objeto)
        if _YA_ESTA.search(salida):
            return
        raise error


# ---------------------------------------------------------------------- lo que se usa desde la tienda

def instaladas():
    """Ids de las apps instaladas (de tu usuario y del sistema), sin runtimes. Orden alfabético."""
    gi = _libflatpak()
    return _gi_instaladas(gi) if gi else _cli_instaladas()


def es_del_sistema(app_id):
    """True si la app está instalada para todo el sistema y no en tu usuario (la tienda no puede quitarla)."""
    _validar(app_id)
    gi = _libflatpak()
    if gi:
        usuario, sistema = _gi_buscar(gi, app_id)
        return sistema is not None and usuario is None
    return app_id in _cli_instaladas("--system") and app_id not in _cli_instaladas("--user")


def actualizaciones():
    """Ids de tus apps (instaladas para tu usuario) que tienen una versión nueva. Pide la lista a
    Flathub: FlatpakSinRed si no hay internet. Las del sistema no se cuentan (actualizarlas pide ser
    administrador)."""
    gi = _libflatpak()
    if gi:
        return _gi_actualizaciones(gi)
    return _cli_ids(["remote-ls", "--user", "--updates", "--app", "--columns=application"])


def preparar():
    """Deja Flathub como fuente de tu usuario si falta (flatpak remote-add --user --if-not-exists).
    Devuelve True si lo agregó. Sin internet y sin Flathub: FlatpakSinRed."""
    existentes = _cli_lineas(["remotes", "--user", "--columns=name"])
    if REMOTO in existentes:
        return False
    try:
        r = subprocess.run([_exe(), "remote-add", "--user", "--if-not-exists", REMOTO, URL_REPO],
                           stdin=subprocess.DEVNULL, capture_output=True, text=True, errors="replace",
                           timeout=90, env=_entorno(), check=False)
    except subprocess.TimeoutExpired:
        raise FlatpakSinRed() from None
    except OSError as e:
        raise ErrorFlatpak("No se pudo ejecutar Flatpak: %s" % e.strerror) from None
    if r.returncode != 0:
        raise _error(r.stderr or r.stdout, "preparar", "Flathub")
    return True


def instalar(app_id, progreso=None):
    """Instala la app para tu usuario. progreso(0-100, texto) va avisando; termina con (100, «Listo»)."""
    _validar(app_id)
    avance = _Avance(progreso)
    with _turno(avance):
        gi = _libflatpak()
        ya = (any(r.get_name() == app_id for r in _gi_apps(gi, _gi_usuario(gi)) + _gi_apps(gi, _gi_sistema(gi)))
              if gi else app_id in _cli_instaladas())
        if ya:
            avance(100, "Ya estaba instalada")
            return
        avance(0, "Preparando…")
        preparar()
        if gi:
            _gi_instalar(gi, app_id, avance)
        else:
            _cli_correr(["install", "--user", "--noninteractive", "-y", REMOTO, app_id], avance, "instalar", _q(app_id))
        avance(100, "Listo")


def quitar(app_id, progreso=None):
    """Quita la app de tu usuario (sus datos en ~/.var/app se conservan). Si es del sistema, ErrorFlatpak
    explicando que desde aquí no se puede."""
    _validar(app_id)
    avance = _Avance(progreso)
    with _turno(avance):
        gi = _libflatpak()
        if gi:
            usuario, sistema = _gi_buscar(gi, app_id)
            nombre = _nombre_de(usuario or sistema, app_id) if (usuario or sistema) else app_id
            en_usuario, en_sistema = usuario is not None, sistema is not None
        else:
            nombre = app_id
            en_usuario, en_sistema = app_id in _cli_instaladas("--user"), app_id in _cli_instaladas("--system")
        if not en_usuario:
            if en_sistema:
                raise ErrorFlatpak("«%s» es parte del sistema: la instaló Vortexine's OS para todos los usuarios y "
                                   "desde la tienda no se puede quitar." % nombre)
            raise ErrorFlatpak("«%s» no está instalada." % nombre)
        avance(0, "Quitando…")
        if gi:
            _gi_quitar(gi, usuario, avance)
        else:
            _cli_correr(["uninstall", "--user", "--noninteractive", "-y", app_id], avance, "quitar", _q(nombre))
            # Los runtimes que quedaron sin ninguna app que los use.
            with contextlib.suppress(ErrorFlatpak):
                _cli_correr(["uninstall", "--user", "--unused", "--noninteractive", "-y"], avance, "quitar", _q(nombre))
        avance(100, "Listo")


def actualizar(app_ids=None, progreso=None):
    """Actualiza las apps de tu usuario: las de la lista, o todas si app_ids es None."""
    if app_ids is not None:
        app_ids = [_validar(i) for i in app_ids]
    avance = _Avance(progreso)
    with _turno(avance):
        avance(0, "Buscando actualizaciones…")
        gi = _libflatpak()
        if gi:
            hubo = _gi_actualizar(gi, set(app_ids) if app_ids is not None else None, avance)
        else:
            if app_ids is not None:
                pendientes = set(_cli_ids(["remote-ls", "--user", "--updates", "--app", "--columns=application"]))
                app_ids = [i for i in app_ids if i in pendientes]
            hubo = app_ids is None or bool(app_ids)
            if hubo:
                _cli_correr(["update", "--user", "--noninteractive", "-y"] + (app_ids or []), avance, "actualizar",
                            "tus aplicaciones", "Descargando…")
        avance(100, "Listo" if hubo else "Todo está al día")


def abrir(app_id):
    """Abre la app, desacoplada de la tienda (cerrar la tienda no la cierra, y no queda como hija)."""
    _validar(app_id)
    exe = _exe()
    try:
        instalada = app_id in instaladas()
    except ErrorFlatpak:
        instalada = True        # si ni se pudo consultar, se intenta abrir de todos modos
    if not instalada:
        raise ErrorFlatpak("«%s» no está instalada." % app_id)
    setsid = shutil.which("setsid")
    # «setsid -f» hace el fork y devuelve el control al instante: la app queda colgada de init, no de la tienda.
    orden = ([setsid, "-f"] if setsid else []) + [exe, "run", app_id]
    try:
        proceso = subprocess.Popen(orden, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, close_fds=True, cwd=os.path.expanduser("~"),
                                   start_new_session=not setsid)
    except OSError as e:
        raise ErrorFlatpak("No se pudo abrir «%s»: %s" % (app_id, e.strerror)) from None
    if setsid:
        proceso.wait()
    else:
        threading.Thread(target=proceso.wait, daemon=True).start()     # recoge el cadáver cuando la app cierre
