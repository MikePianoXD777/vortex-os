"""El catálogo de Vortexine's Shop: todo lo que hay en Flathub, leído de su API pública.

    https://flathub.org/api/v2/   (documentada en https://flathub.org/api/v2/openapi.json)

Endpoints que se usan (todos verificados contra la API real; los de lista exigen mandar «page» y
«per_page» juntos, si no responden 400):

    GET  /collection/trending|popular?page=&per_page=&locale=   listas de apps (Meilisearch)
    GET  /collection/category                                    ids de las categorías
    GET  /collection/category/{id}?page=&per_page=&sort_by=&locale=
    POST /search?locale=   {"query", "hits_per_page", "page"}   búsqueda en todo el catálogo
    GET  /appstream/{app_id}?locale=                             ficha completa (descripción, capturas)
    GET  /summary/{app_id}                                       tamaño de descarga
    GET  /app-picks/app-of-the-day/{AAAA-MM-DD}                  la app del día y las de la semana
    GET  /app-picks/apps-of-the-week/{AAAA-MM-DD}

El idioma se pide con «locale» y Flathub traduce lo que tenga traducido (nombre y resumen de las
apps que lo tienen; si no, viene en inglés). Solo biblioteca estándar.

Red: timeouts de 12 s, User-Agent «vortexines-shop/1.0» y caché en disco (~/.cache/vortexines-shop/,
un JSON por petición): vale 1 h para listas y búsquedas, 24 h para fichas. Si no hay red y hay caché,
se usa aunque esté vieja; si no hay nada, SinRed. Para probar sin internet, VORTEX_SHOP_API apunta
a otro servidor con la misma forma (pruebas/tienda.sh levanta uno falso).
"""
import concurrent.futures
import hashlib
import http.client
import json
import os
import re
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field

from . import AGENTE
from .errores import AppNoEncontrada, SinRed
from .ids import es_id_valido
from .texto import html_a_texto

API_POR_DEFECTO = "https://flathub.org/api/v2"
ESPERA = 12                  # segundos de timeout por petición
TTL_LISTAS = 3600            # 1 h: listas, categorías por página y búsquedas
TTL_DETALLES = 24 * 3600     # 24 h: fichas, tamaños y la lista de categorías
POR_PAGINA = 30              # apps por página (en categorías y búsquedas)
MINIMO_DESTACADOS = 6
MAXIMO_RESPUESTA = 16 * 1024 * 1024

# Lo que Vortexine's recomienda (verificado: todos existen en Flathub). Mismo orden en pantalla.
RECOMENDADAS = [
    "com.valvesoftware.Steam",            # Steam
    "org.prismlauncher.PrismLauncher",    # Prism Launcher (Minecraft)
    "org.vinegarhq.Sober",                # Sober (Roblox)
    "com.heroicgameslauncher.hgl",        # Heroic (Epic y GOG)
    "com.usebottles.bottles",             # Bottles
    "net.lutris.Lutris",                  # Lutris
    "net.davidotek.pupgui2",              # ProtonUp-Qt
    "sh.ppy.osu",                         # osu! (lazer)
    "org.pegasus_frontend.Pegasus",       # Pegasus Frontend
    "org.libretro.RetroArch",             # RetroArch
    "com.discordapp.Discord",             # Discord
    "dev.vencord.Vesktop",                # Vesktop
    "org.telegram.desktop",               # Telegram
    "us.zoom.Zoom",                       # Zoom
    "org.mozilla.firefox",                # Firefox
    "com.brave.Browser",                  # Brave
    "com.spotify.Client",                 # Spotify
    "org.videolan.VLC",                   # VLC
    "com.obsproject.Studio",              # OBS Studio
    "org.kde.kdenlive",                   # Kdenlive
    "org.gimp.GIMP",                      # GIMP
    "org.libreoffice.LibreOffice",        # LibreOffice
    "org.b3log.siyuan",                   # SiYuan
    "com.github.wwmm.easyeffects",        # EasyEffects
    "com.github.tchx84.Flatseal",         # Flatseal (permisos de las apps)
]

# Categorías de Flathub (las que devuelve /collection/category) con su nombre y su ícono simbólico.
# El orden es el de la pantalla; las que Flathub agregue después salen al final con su id.
_CATEGORIAS = [
    ("game", "Juegos", "applications-games-symbolic"),
    ("audiovideo", "Audio y vídeo", "applications-multimedia-symbolic"),
    ("graphics", "Gráficos y fotos", "applications-graphics-symbolic"),
    ("development", "Programación", "applications-engineering-symbolic"),
    ("office", "Oficina", "applications-office-symbolic"),
    ("network", "Internet y chat", "applications-internet-symbolic"),
    ("utility", "Utilidades", "applications-utilities-symbolic"),
    ("system", "Sistema", "applications-system-symbolic"),
    ("education", "Educación", "applications-education-symbolic"),
    ("science", "Ciencia", "applications-science-symbolic"),
    ("healthfitness", "Salud y deporte", "applications-health-symbolic"),
]
_CATEGORIA_NUEVA = "applications-other-symbolic"

# Categorías de la ficha (las del .desktop de la app) en español. Las que no están aquí no se muestran.
_ETIQUETAS = {
    "AudioVideo": "Audio y vídeo", "Audio": "Audio", "Video": "Vídeo", "Development": "Programación",
    "Education": "Educación", "Game": "Juegos", "Graphics": "Gráficos", "Network": "Internet",
    "Office": "Oficina", "Science": "Ciencia", "Settings": "Configuración", "System": "Sistema",
    "Utility": "Utilidades",
    "ActionGame": "Acción", "AdventureGame": "Aventura", "ArcadeGame": "Arcade", "BoardGame": "Juegos de mesa",
    "BlocksGame": "Bloques", "CardGame": "Cartas", "KidsGame": "Infantil", "LogicGame": "Lógica",
    "RolePlaying": "Rol", "Shooter": "Disparos", "Simulation": "Simulación", "SportsGame": "Deportes",
    "StrategyGame": "Estrategia", "Emulator": "Emuladores",
    "Music": "Música", "Player": "Reproductores", "Recorder": "Grabación", "Midi": "MIDI", "Mixer": "Mezcla",
    "Sequencer": "Secuenciadores", "Tuner": "Afinadores", "AudioVideoEditing": "Edición de audio y vídeo",
    "Photography": "Fotografía", "RasterGraphics": "Imágenes", "VectorGraphics": "Gráficos vectoriales",
    "3DGraphics": "Gráficos 3D", "Scanning": "Escáner", "OCR": "Reconocimiento de texto", "Viewer": "Visores",
    "Chat": "Chat", "InstantMessaging": "Mensajería", "Email": "Correo", "WebBrowser": "Navegadores",
    "FileTransfer": "Descargas", "P2P": "P2P", "VideoConference": "Videollamadas", "Telephony": "Llamadas",
    "IRCClient": "IRC", "News": "Noticias", "RemoteAccess": "Acceso remoto", "Feed": "Noticias",
    "TextEditor": "Editores de texto", "IDE": "Entornos de desarrollo", "Debugger": "Depuración",
    "WebDevelopment": "Desarrollo web", "Calculator": "Calculadoras", "Math": "Matemáticas",
    "Calendar": "Calendario", "Spreadsheet": "Hojas de cálculo", "WordProcessor": "Procesadores de texto",
    "Presentation": "Presentaciones", "Database": "Bases de datos", "Dictionary": "Diccionarios",
    "Archiving": "Archivos comprimidos", "FileManager": "Archivos", "TerminalEmulator": "Terminales",
    "Monitor": "Monitores", "Security": "Seguridad", "Languages": "Idiomas", "Astronomy": "Astronomía",
    "Biology": "Biología", "Chemistry": "Química", "Physics": "Física", "Geography": "Geografía",
    "Maps": "Mapas", "Art": "Arte", "Electronics": "Electrónica", "Engineering": "Ingeniería",
}

# Tipos que el índice de Flathub puede traer y que no son una app que se pueda abrir.
_NO_APPS = {"runtime", "addon", "extension", "localization", "generic"}


@dataclass
class App:
    id: str
    nombre: str
    resumen: str
    icono_url: str
    desarrollador: str
    verificado: bool
    descargas_mes: "int | None" = None


@dataclass
class Categoria:
    id: str
    nombre: str
    icono: str


@dataclass
class AppDetalle(App):
    descripcion: str = ""
    capturas: list = field(default_factory=list)
    licencia: str = ""
    web: "str | None" = None
    version: "str | None" = None
    tamano_descarga: "int | None" = None
    categorias: list = field(default_factory=list)


class _NoExiste(Exception):
    """El servidor dice que eso no existe (404 o 422)."""


class _Fallo(Exception):
    """No se pudo hablar con el servidor (sin red, timeout, error 5xx, respuesta ilegible)."""


def _texto(valor):
    return valor.strip() if isinstance(valor, str) else ""


def _entero(valor):
    if isinstance(valor, bool):
        return None
    if isinstance(valor, int):
        return valor
    if isinstance(valor, float):
        return int(valor)
    if isinstance(valor, str) and valor.strip().isdigit():
        return int(valor.strip())
    return None


def _verdadero(valor):
    return valor is True or (isinstance(valor, str) and valor.strip().lower() == "true")


def _url_web(valor):
    valor = _texto(valor)
    return valor if valor.lower().startswith(("http://", "https://")) else ""


def _licencia(valor):
    """La licencia SPDX de AppStream, legible. «LicenseRef-proprietary» (con o sin «=url») es software
    propietario; las demás LicenseRef-* se muestran sin el prefijo."""
    valor = _texto(valor)
    if not valor:
        return "Sin información"
    valor = valor.split("=", 1)[0]
    if valor.lower().startswith("licenseref-proprietary"):
        return "Propietaria"
    return valor.replace("LicenseRef-", "")


def _app_de_hit(hit):
    """Una app de las listas y búsquedas de Flathub. None si el elemento no es una app usable."""
    if not isinstance(hit, dict):
        return None
    app_id = hit.get("app_id")
    if not es_id_valido(app_id) or hit.get("type") in _NO_APPS:
        return None
    return App(
        id=app_id,
        nombre=_texto(hit.get("name")) or app_id,
        resumen=_texto(hit.get("summary")),
        icono_url=_url_web(hit.get("icon")),
        desarrollador=_texto(hit.get("developer_name")),
        verificado=_verdadero(hit.get("verification_verified")),
        descargas_mes=_entero(hit.get("installs_last_month")),
    )


def _app_de_appstream(app_id, ficha):
    """Una app a partir de su ficha de /appstream (no trae el número de descargas)."""
    meta = ficha.get("metadata") if isinstance(ficha.get("metadata"), dict) else {}
    return App(
        id=app_id,
        nombre=_texto(ficha.get("name")) or app_id,
        resumen=_texto(ficha.get("summary")),
        icono_url=_url_web(ficha.get("icon")),
        desarrollador=_texto(ficha.get("developer_name")),
        verificado=_verdadero(meta.get("flathub::verification::verified")),
        descargas_mes=None,
    )


def _captura(sizes):
    """De los tamaños de una captura, el más grande que no pase de 1280 px de ancho (o el más chico
    si todos pasan): la tienda no necesita bajar el original de 4K."""
    candidatas = []
    for tam in sizes if isinstance(sizes, list) else []:
        if not isinstance(tam, dict):
            continue
        src = _url_web(tam.get("src"))
        ancho = _entero(tam.get("width")) or 0
        if src:
            candidatas.append((ancho, src))
    if not candidatas:
        return ""
    dentro = [c for c in candidatas if c[0] <= 1280]
    return max(dentro)[1] if dentro else min(candidatas)[1]


def _version(releases):
    """La versión de la publicación más reciente."""
    mejor, marca = None, -1
    for rel in releases if isinstance(releases, list) else []:
        if not isinstance(rel, dict) or not _texto(rel.get("version")):
            continue
        t = _entero(rel.get("timestamp"))
        t = -1 if t is None else t
        if mejor is None or t > marca:
            mejor, marca = _texto(rel.get("version")), t
    return mejor


class Catalogo:
    """Lo que hay en Flathub. Todos los métodos pueden lanzar SinRed (sin internet y sin caché);
    detalle() también AppNoEncontrada si Flathub no tiene esa app."""

    POR_PAGINA = POR_PAGINA

    def __init__(self, cache_dir=None, idioma="es", api=None):
        idioma = _texto(idioma)
        self.idioma = idioma if re.fullmatch(r"[A-Za-z]{2,3}([_-][A-Za-z0-9]{2,8})?", idioma) else "es"
        base = (api or os.environ.get("VORTEX_SHOP_API") or API_POR_DEFECTO).strip()
        if not base.lower().startswith(("http://", "https://")):
            base = API_POR_DEFECTO
        self.api = base.rstrip("/")
        if cache_dir:
            self.cache_dir = os.fspath(cache_dir)
        else:
            raiz = os.environ.get("XDG_CACHE_HOME") or os.path.join(os.path.expanduser("~"), ".cache")
            self.cache_dir = os.path.join(raiz, "vortexines-shop")
        self._ahora = time.time      # las pruebas lo cambian para simular que pasó el tiempo

    # ------------------------------------------------------------------ red y caché

    def _archivo_cache(self, metodo, url, cuerpo):
        huella = "%s %s %s" % (metodo, url, json.dumps(cuerpo, sort_keys=True) if cuerpo is not None else "")
        return os.path.join(self.cache_dir, hashlib.sha256(huella.encode()).hexdigest()[:40] + ".json")

    @staticmethod
    def _leer_cache(archivo):
        """(momento, datos) o None si no hay caché o está dañada."""
        try:
            with open(archivo, encoding="utf-8") as f:
                guardado = json.load(f)
            return float(guardado["t"]), guardado["datos"]
        except (OSError, ValueError, KeyError, TypeError):
            return None

    def _guardar_cache(self, archivo, ahora, datos):
        # Se escribe en un archivo aparte y se renombra: otro hilo (o un corte de luz) nunca ve un JSON a medias.
        tmp = "%s.%d.%d.tmp" % (archivo, os.getpid(), threading.get_ident())
        try:
            os.makedirs(self.cache_dir, mode=0o700, exist_ok=True)
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump({"t": ahora, "datos": datos}, f, ensure_ascii=False)
            os.replace(tmp, archivo)
        except OSError:
            try:
                os.unlink(tmp)
            except OSError:
                pass

    def _descargar(self, url, cuerpo):
        cabeceras = {"User-Agent": AGENTE, "Accept": "application/json"}
        datos = None
        if cuerpo is not None:
            datos = json.dumps(cuerpo).encode("utf-8")
            cabeceras["Content-Type"] = "application/json"
        peticion = urllib.request.Request(url, data=datos, headers=cabeceras)
        try:
            with urllib.request.urlopen(peticion, timeout=ESPERA) as r:
                crudo = r.read(MAXIMO_RESPUESTA + 1)
            if len(crudo) > MAXIMO_RESPUESTA:
                raise _Fallo("respuesta demasiado grande")
            return json.loads(crudo.decode("utf-8"))
        except urllib.error.HTTPError as e:
            e.close()
            if e.code in (404, 422):
                raise _NoExiste(url) from e
            raise _Fallo("HTTP %s" % e.code) from e
        except (urllib.error.URLError, OSError, http.client.HTTPException, ValueError) as e:
            raise _Fallo(str(e)) from e

    def _pedir(self, ruta, ttl, params=None, cuerpo=None):
        """JSON de la ruta: de la caché si es reciente; si no, de la red (y se guarda). Con la red
        caída se usa la caché aunque esté vieja. AppNoEncontrada si el servidor dice que no existe;
        SinRed si no hay red ni caché."""
        url = self.api + ruta
        if params:
            url += "?" + urllib.parse.urlencode(params)
        archivo = self._archivo_cache("POST" if cuerpo is not None else "GET", url, cuerpo)
        guardado = self._leer_cache(archivo)
        ahora = self._ahora()
        if guardado and 0 <= ahora - guardado[0] < ttl:
            return guardado[1]
        try:
            datos = self._descargar(url, cuerpo)
        except _NoExiste:
            raise AppNoEncontrada("Flathub no tiene lo que buscabas.") from None
        except _Fallo:
            if guardado:
                return guardado[1]
            raise SinRed() from None
        self._guardar_cache(archivo, ahora, datos)
        return datos

    # ------------------------------------------------------------------ listas

    @staticmethod
    def _apps(datos):
        """Las apps de una respuesta de lista (campo «hits»), sin repetidas ni basura."""
        vistas, salida = set(), []
        for hit in datos.get("hits") or [] if isinstance(datos, dict) else []:
            app = _app_de_hit(hit)
            if app and app.id not in vistas:
                vistas.add(app.id)
                salida.append(app)
        return salida

    def _coleccion(self, nombre, cantidad):
        datos = self._pedir("/collection/" + nombre, TTL_LISTAS,
                            params={"page": 1, "per_page": cantidad, "locale": self.idioma})
        return self._apps(datos)

    def _resolver(self, app_id):
        """App a partir de su id (ficha de /appstream). AppNoEncontrada si Flathub no la tiene."""
        ficha = self._pedir("/appstream/" + urllib.parse.quote(app_id, safe=""), TTL_DETALLES,
                            params={"locale": self.idioma})
        if not isinstance(ficha, dict):
            raise AppNoEncontrada("Flathub no tiene esa aplicación.")
        return _app_de_appstream(app_id, ficha)

    def _resolver_varias(self, ids):
        """Las apps de esos ids, en el mismo orden, pidiendo las fichas en paralelo. Las que Flathub
        ya no tiene se saltan. SinRed solo si no se pudo conseguir ninguna por falta de red."""
        def una(app_id):
            try:
                return self._resolver(app_id), None
            except AppNoEncontrada:
                return None, None
            except SinRed as e:
                return None, e
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            resultados = list(pool.map(una, ids))
        apps = [app for app, _ in resultados if app]
        if not apps:
            for _, error in resultados:
                if error:
                    raise error
        return apps

    def destacados(self):
        """La app del día y las de la semana de Flathub; si faltan, completa con las que más suben."""
        hoy = time.strftime("%Y-%m-%d", time.gmtime(self._ahora()))
        ids = []
        try:
            dia = self._pedir("/app-picks/app-of-the-day/" + hoy, TTL_LISTAS)
            if isinstance(dia, dict) and es_id_valido(dia.get("app_id")):
                ids.append(dia["app_id"])
            semana = self._pedir("/app-picks/apps-of-the-week/" + hoy, TTL_LISTAS)
            elegidas = semana.get("apps") if isinstance(semana, dict) else None
            for elegida in sorted((e for e in elegidas or [] if isinstance(e, dict)),
                                  key=lambda e: _entero(e.get("position")) or 0):
                if es_id_valido(elegida.get("app_id")) and elegida["app_id"] not in ids:
                    ids.append(elegida["app_id"])
        except (SinRed, AppNoEncontrada):
            pass        # sin las selecciones del día, las populares de abajo hacen de destacados
        try:
            apps = self._resolver_varias(ids) if ids else []
        except SinRed:
            apps = []
        if len(apps) < MINIMO_DESTACADOS:
            tendencia = self._coleccion("trending", MINIMO_DESTACADOS * 2)    # SinRed si no hay nada
            ya = {a.id for a in apps}
            apps += [a for a in tendencia if a.id not in ya][:MINIMO_DESTACADOS - len(apps)]
        return apps

    def recomendadas(self):
        """La lista de Vortexine's (RECOMENDADAS), con los datos de Flathub. Las que Flathub ya no
        tenga no salen."""
        return self._resolver_varias(RECOMENDADAS)

    def categorias(self):
        """Las categorías de Flathub, en español y en el orden de la pantalla."""
        ids = self._pedir("/collection/category", TTL_DETALLES)
        ids = [i.strip().lower() for i in ids if isinstance(i, str)] if isinstance(ids, list) else []
        conocidas = {c[0]: c for c in _CATEGORIAS}
        salida = [Categoria(*conocidas[i]) for i, *_ in _CATEGORIAS if i in ids]
        for i in ids:
            if i not in conocidas and re.fullmatch(r"[a-z0-9_-]{1,40}", i):
                salida.append(Categoria(i, i.capitalize(), _CATEGORIA_NUEVA))
        return salida

    def por_categoria(self, cat_id, pagina=1):
        """Una página (POR_PAGINA apps) de una categoría, de las más instaladas a las menos. La lista
        viene vacía cuando la página no existe o la categoría tampoco."""
        cat_id = _texto(cat_id).lower()
        if not re.fullmatch(r"[a-z0-9_-]{1,40}", cat_id):
            return []
        pagina = max(1, _entero(pagina) or 1)
        try:
            datos = self._pedir("/collection/category/" + urllib.parse.quote(cat_id, safe=""), TTL_LISTAS,
                                params={"page": pagina, "per_page": POR_PAGINA,
                                        "sort_by": "installs_last_month", "locale": self.idioma})
        except AppNoEncontrada:
            return []
        return self._apps(datos)

    def buscar(self, texto):
        """Las apps que coinciden con lo escrito (nombre, resumen, palabras clave), las mejores primero."""
        texto = " ".join(_texto(texto).split())[:100]
        if not texto:
            return []
        datos = self._pedir("/search", TTL_LISTAS, params={"locale": self.idioma},
                            cuerpo={"query": texto, "hits_per_page": POR_PAGINA, "page": 1})
        return self._apps(datos)

    # ------------------------------------------------------------------ ficha

    def detalle(self, app_id):
        """La ficha completa de una app. AppNoEncontrada si el id no es válido o Flathub no la tiene."""
        if not es_id_valido(app_id):
            raise AppNoEncontrada("Esa aplicación no existe.")
        seguro = urllib.parse.quote(app_id, safe="")

        def tamano():
            # El tamaño es un extra: sin él la ficha sale igual.
            try:
                return self._pedir("/summary/" + seguro, TTL_DETALLES)
            except (SinRed, AppNoEncontrada):
                return None

        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            pedido_resumen = pool.submit(tamano)
            ficha = self._pedir("/appstream/" + seguro, TTL_DETALLES, params={"locale": self.idioma})
            resumen = pedido_resumen.result()
        if not isinstance(ficha, dict):
            raise AppNoEncontrada("Flathub no tiene esa aplicación.")

        base = _app_de_appstream(app_id, ficha)
        urls = ficha.get("urls") if isinstance(ficha.get("urls"), dict) else {}
        capturas = []
        for cap in ficha.get("screenshots") if isinstance(ficha.get("screenshots"), list) else []:
            url = _captura(cap.get("sizes")) if isinstance(cap, dict) else ""
            if url and url not in capturas:
                capturas.append(url)
        etiquetas = []
        for cat in ficha.get("categories") if isinstance(ficha.get("categories"), list) else []:
            nombre = _ETIQUETAS.get(cat) if isinstance(cat, str) else None
            if nombre and nombre not in etiquetas:
                etiquetas.append(nombre)
        tam = _entero(resumen.get("download_size")) if isinstance(resumen, dict) else None
        return AppDetalle(
            id=base.id, nombre=base.nombre, resumen=base.resumen, icono_url=base.icono_url,
            desarrollador=base.desarrollador, verificado=base.verificado, descargas_mes=None,
            descripcion=html_a_texto(ficha.get("description")) or base.resumen,
            capturas=capturas[:12],
            licencia=_licencia(ficha.get("project_license")),
            web=_url_web(urls.get("homepage")) or None,
            version=_version(ficha.get("releases")),
            tamano_descarga=tam if tam and tam > 0 else None,
            categorias=etiquetas,
        )
