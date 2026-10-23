"""Doble falso del motor de Vortexine's Shop, con la MISMA interfaz que el real
(vortex_shop.catalogo, vortex_shop.flatpak y vortex_shop.errores), para probar la interfaz sin
red y sin tocar flatpak. Los íconos y las capturas son SVG generados en una carpeta temporal.

    motor = crear_motor(tmp, sin_red=False, duracion=3.0)   # → vortex_shop.puente.Motor
    motor.control.sin_red = True                            # perillas para las pruebas
"""
from __future__ import annotations

import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from types import SimpleNamespace


# --- errores.py ---
class SinRed(Exception):
    pass


class ErrorFlatpak(Exception):
    pass


class AppNoEncontrada(ErrorFlatpak):
    pass


class FlatpakSinRed(ErrorFlatpak, SinRed):
    pass


# --- Dataclasses del catálogo (mismos campos que el contrato) ---
@dataclass
class App:
    id: str
    nombre: str
    resumen: str
    icono_url: str
    desarrollador: str
    verificado: bool
    descargas_mes: int | None = None


@dataclass
class Categoria:
    id: str
    nombre: str
    icono: str


@dataclass
class AppDetalle(App):
    descripcion: str = ""
    capturas: list[str] = field(default_factory=list)
    licencia: str = ""
    web: str | None = None
    version: str | None = None
    tamano_descarga: int | None = None
    categorias: list[str] = field(default_factory=list)


CATEGORIAS = [
    Categoria("Game", "Juegos", "applications-games-symbolic"),
    Categoria("Office", "Productividad", "applications-office-symbolic"),
    Categoria("Graphics", "Gráficos y fotos", "applications-graphics-symbolic"),
    Categoria("AudioVideo", "Audio y video", "applications-multimedia-symbolic"),
    Categoria("Development", "Desarrollo", "applications-engineering-symbolic"),
    Categoria("Education", "Educación", "applications-science-symbolic"),
    Categoria("Network", "Internet", "applications-internet-symbolic"),
    Categoria("Utility", "Utilidades", "applications-utilities-symbolic"),
]

# (id, nombre, resumen, desarrollador, verificado, categoría, descargas/mes, MB)
_DATOS = [
    ("org.mozilla.firefox", "Firefox", "Navegador web rápido y privado", "Mozilla", True, "Network", 920_000, 82),
    ("org.chromium.Chromium", "Chromium", "El navegador de código abierto detrás de Chrome", "The Chromium Authors", False, "Network", 310_000, 140),
    ("com.valvesoftware.Steam", "Steam", "Tu biblioteca de juegos, tienda y comunidad", "Valve", True, "Game", 640_000, 48),
    ("net.lutris.Lutris", "Lutris", "Juega a todo en Linux desde un solo sitio", "Lutris Team", False, "Game", 120_000, 36),
    ("com.heroicgameslauncher.hgl", "Heroic Games Launcher", "Juegos de Epic, GOG y Amazon en Linux", "Heroic Games Launcher", True, "Game", 180_000, 120),
    ("org.prismlauncher.PrismLauncher", "Prism Launcher", "Lanzador de Minecraft con instancias separadas y mods", "Prism Launcher", True, "Game", 95_000, 54),
    ("com.discordapp.Discord", "Discord", "Chat de voz, video y texto para comunidades", "Discord Inc.", True, "Network", 700_000, 110),
    ("org.telegram.desktop", "Telegram", "Mensajería rápida y segura", "Telegram FZ-LLC", True, "Network", 420_000, 65),
    ("org.signal.Signal", "Signal", "Mensajes privados con cifrado de extremo a extremo", "Signal Foundation", True, "Network", 150_000, 98),
    ("com.spotify.Client", "Spotify", "Música y podcasts en streaming", "Spotify", True, "AudioVideo", 520_000, 215),
    ("org.videolan.VLC", "VLC", "Reproduce cualquier video o audio, sin complicaciones", "VideoLAN", True, "AudioVideo", 800_000, 105),
    ("com.obsproject.Studio", "OBS Studio", "Graba y transmite en vivo con escenas y fuentes", "OBS Project", True, "AudioVideo", 410_000, 170),
    ("org.audacityteam.Audacity", "Audacity", "Edita y graba audio multipista", "Audacity Team", True, "AudioVideo", 160_000, 62),
    ("org.kde.kdenlive", "Kdenlive", "Editor de video profesional, libre y gratuito", "KDE", True, "AudioVideo", 130_000, 280),
    ("org.gimp.GIMP", "GIMP", "Edita fotos y crea imágenes con herramientas avanzadas", "GIMP", True, "Graphics", 450_000, 200),
    ("org.inkscape.Inkscape", "Inkscape", "Dibujo vectorial profesional", "Inkscape Project", True, "Graphics", 210_000, 150),
    ("org.kde.krita", "Krita", "Pintura digital para artistas", "KDE", True, "Graphics", 230_000, 310),
    ("org.blender.Blender", "Blender", "Modelado 3D, animación y renderizado", "Blender Foundation", True, "Graphics", 270_000, 380),
    ("org.libreoffice.LibreOffice", "LibreOffice", "Suite de oficina completa: texto, hojas de cálculo y presentaciones", "The Document Foundation", True, "Office", 880_000, 520),
    ("md.obsidian.Obsidian", "Obsidian", "Notas enlazadas en archivos de texto que son tuyos", "Obsidian", False, "Office", 190_000, 125),
    ("com.bitwarden.desktop", "Bitwarden", "Administrador de contraseñas de código abierto", "Bitwarden Inc.", True, "Utility", 140_000, 88),
    ("org.mozilla.Thunderbird", "Thunderbird", "Correo, calendario y contactos en un solo sitio", "Mozilla", True, "Office", 300_000, 95),
    ("com.visualstudio.code", "Visual Studio Code", "Editor de código con extensiones, depuración y Git", "Microsoft", True, "Development", 560_000, 140),
    ("org.godotengine.Godot", "Godot Engine", "Motor para crear juegos 2D y 3D", "Godot Foundation", True, "Development", 85_000, 90),
    ("org.gnome.Builder", "GNOME Builder", "Un IDE pensado para el escritorio GNOME", "GNOME", True, "Development", 20_000, 75),
    ("net.ankiweb.Anki", "Anki", "Tarjetas de estudio con repetición espaciada", "Ankitects", False, "Education", 60_000, 130),
    ("org.kde.kalgebra", "KAlgebra", "Calculadora gráfica y herramienta de álgebra", "KDE", True, "Education", 4_000, 25),
    ("org.qbittorrent.qBittorrent", "qBittorrent", "Cliente BitTorrent ligero y sin anuncios", "qBittorrent", True, "Network", 290_000, 70),
    ("io.github.flattool.Warehouse", "Warehouse", "Administra tus Flatpaks y sus datos con facilidad", "Heliguy", False, "Utility", 55_000, 12),
    ("com.github.tchx84.Flatseal", "Flatseal", "Revisa y cambia los permisos de tus apps", "Martín Abente Lahaye", False, "Utility", 240_000, 8),
    ("org.gnome.Calculator", "Calculadora", "Calculadora sencilla con modo científico y de programación", "GNOME", True, "Utility", 100_000, 6),
    ("us.zoom.Zoom", "Zoom", "Videollamadas y reuniones en línea", "Zoom", False, "Network", 200_000, 160),
    ("org.vinegarhq.Sober", "Sober", "Cliente no oficial de Roblox para Linux, con mejoras de rendimiento y compatibilidad", "Vinegar", False, "Game", 150_000, 45),
    ("io.github.Hexchat.cliente.muy.largo.IdDePrueba", "Un nombre de app extraordinariamente largo para probar los cortes de texto", "Un resumen igualmente larguísimo que debe cortarse con puntos suspensivos y no romper la tarjeta", "Un desarrollador con un nombre larguísimo S.A. de C.V.", True, "Utility", 900, 33),
    ("org.sinicono.Sencilla", "App sin ícono", "Su ícono no carga: se ve la inicial sobre un color", "Alguien", False, "Utility", None, 2),
]

_COLORES = [("#25306e", "#9ec5ff"), ("#352570", "#c6a8ff"), ("#1c4558", "#8fdcf0"), ("#44255a", "#f0a8e6"),
            ("#244a3f", "#8fe3b4"), ("#58402a", "#f5c88f")]


def _svg_icono(letra: str, tono: int) -> str:
    fondo, tinta = _COLORES[tono % len(_COLORES)]
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">'
            f'<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{tinta}"/>'
            f'<stop offset="1" stop-color="{fondo}"/></linearGradient></defs>'
            f'<rect width="128" height="128" rx="30" fill="url(#g)"/>'
            f'<circle cx="64" cy="64" r="30" fill="none" stroke="#ffffff" stroke-opacity="0.85" stroke-width="9"/>'
            f'<circle cx="64" cy="64" r="9" fill="#ffffff" fill-opacity="0.9"/></svg>')


def _svg_captura(titulo: str, tono: int, n: int) -> str:
    fondo, tinta = _COLORES[(tono + n) % len(_COLORES)]
    barras = "".join(f'<rect x="60" y="{140 + i * 70}" width="{420 + (i * 97 + n * 40) % 380}" height="22" rx="11" '
                     f'fill="{tinta}" fill-opacity="0.5"/>' for i in range(6))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 720">'
            f'<rect width="1280" height="720" fill="#10132a"/><rect width="1280" height="64" fill="{fondo}"/>'
            f'<circle cx="34" cy="32" r="9" fill="#ff8a8a"/><circle cx="64" cy="32" r="9" fill="#ffd27f"/>'
            f'<circle cx="94" cy="32" r="9" fill="#8fe3b4"/>'
            f'<text x="640" y="42" font-size="24" fill="#e8ebff" text-anchor="middle" font-family="sans-serif">{titulo} · captura {n + 1}</text>'
            f'{barras}<rect x="860" y="130" width="360" height="460" rx="24" fill="{fondo}"/></svg>')


def cargar_datos_reales(carpeta: str | Path) -> dict[str, AppDetalle]:
    """Apps con URLs REALES de Flathub (íconos y capturas https) a partir de respuestas guardadas:
    los *.json que dejó la prueba del motor (listados de la API de Flathub y fichas appstream)."""
    import html
    import json
    import re

    carpeta = Path(carpeta)
    ids_cat = {"game": "Game", "audiovideo": "AudioVideo", "office": "Office", "graphics": "Graphics",
               "development": "Development", "education": "Education", "network": "Network",
               "utility": "Utility", "system": "Utility"}
    apps: dict[str, AppDetalle] = {}
    for nombre in ("categoria_game.json", "trending.json", "busqueda_minecraft.json"):
        ruta = carpeta / nombre
        if not ruta.exists():
            continue
        for h in json.loads(ruta.read_text(encoding="utf-8")).get("hits", []):
            cats = h.get("main_categories") or []
            cats = [cats] if isinstance(cats, str) else cats
            apps[h["app_id"]] = AppDetalle(
                id=h["app_id"], nombre=h.get("name") or h["app_id"], resumen=h.get("summary") or "",
                icono_url=h.get("icon") or "", desarrollador=h.get("developer_name") or "",
                verificado=bool(h.get("verification_verified")),
                descargas_mes=h.get("installs_last_month"),
                licencia=h.get("project_license") or "", categorias=[ids_cat.get(c.lower(), "Utility") for c in cats])
    for ruta in sorted(carpeta.glob("appstream_*.json")):
        d = json.loads(ruta.read_text(encoding="utf-8"))
        capturas = []
        for sh in d.get("screenshots") or []:
            tamanos = sh.get("sizes") or []
            elegido = next((s for s in tamanos if s.get("width") == "752"), tamanos[0] if tamanos else None)
            if elegido:
                capturas.append(elegido["src"])
        icono = next((i["url"] for i in d.get("icons") or [] if i.get("width") == 128 and i.get("scale") is None), d.get("icon") or "")
        texto = re.sub(r"</(p|li)>", "\n\n", d.get("description") or "")
        texto = html.unescape(re.sub(r"<[^>]+>", "", texto))
        texto = "\n".join(re.sub(r"[ \t]+", " ", l).strip() for l in texto.splitlines()).strip()
        previo = apps.get(d["id"])
        cats = [ids_cat.get(c.lower()) for c in d.get("categories") or []]
        apps[d["id"]] = AppDetalle(
            id=d["id"], nombre=d.get("name") or d["id"], resumen=d.get("summary") or "", icono_url=icono,
            desarrollador=d.get("developer_name") or "", verificado=True,
            descargas_mes=previo.descargas_mes if previo else None, descripcion=re.sub(r"\n{3,}", "\n\n", texto),
            capturas=capturas, licencia=d.get("project_license") or "", web=None,
            version=(d.get("releases") or [{}])[0].get("version"), tamano_descarga=40 * 1024 * 1024,
            categorias=[c for c in cats if c] or ["Utility"])
    return apps


class Control(SimpleNamespace):
    """Perillas de las pruebas: sin_red, duracion (s de una instalación), fallan (ids que fallan)."""


def crear_motor(carpeta: str | Path, sin_red: bool = False, apps_reales: dict[str, AppDetalle] | None = None, duracion: float = 3.0, latencia: float = 0.25,
                instaladas: tuple[str, ...] = (), actualizaciones: tuple[str, ...] = ()):
    """Devuelve un `vortex_shop.puente.Motor` hecho con el doble falso."""
    from vortex_shop.puente import Motor   # import aquí para que este módulo se pueda leer sin PySide

    carpeta = Path(carpeta)
    carpeta.mkdir(parents=True, exist_ok=True)
    control = Control(sin_red=sin_red, duracion=duracion, latencia=latencia, fallan=set(),
                      instaladas=list(instaladas), actualizaciones=list(actualizaciones),
                      registro=[], abiertas=[], del_sistema=set())
    apps: dict[str, AppDetalle] = {}
    for n, (i, nombre, resumen, dev, ver, cat, desc, mb) in enumerate(_DATOS):
        icono = carpeta / f"icono-{n}.svg"
        icono.write_text(_svg_icono(nombre[0], n), encoding="utf-8")
        capturas = []
        for k in range(4):
            c = carpeta / f"captura-{n}-{k}.svg"
            c.write_text(_svg_captura(nombre, n, k), encoding="utf-8")
            capturas.append(c.as_uri())
        apps[i] = AppDetalle(
            id=i, nombre=nombre, resumen=resumen,
            icono_url=("file:///no/existe/icono.png" if "sinicono" in i else icono.as_uri()),
            desarrollador=dev, verificado=ver, descargas_mes=desc,
            descripcion=(f"{nombre}: {resumen[0].lower() + resumen[1:]}.\n\n"
                         "Funciones principales:\n"
                         "• Fácil de usar desde el primer momento.\n"
                         "• Se actualiza solo desde la tienda.\n"
                         "• Funciona en un entorno aislado (Flatpak): tus archivos están a salvo.\n\n"
                         "Este texto es de prueba: viene del doble falso del motor, no de Flathub. "
                         "Sirve para ver cómo se acomoda una descripción larga con varios párrafos y "
                         "saltos de línea dentro de la página de detalle de la app."),
            capturas=capturas, licencia="GPL-3.0-or-later" if n % 2 else "MIT",
            web=f"https://example.org/{nombre.split()[0].lower()}", version=f"{1 + n % 5}.{n % 10}.{n % 7}",
            tamano_descarga=mb * 1024 * 1024, categorias=[cat])
    recomendadas = ["org.mozilla.firefox", "com.valvesoftware.Steam", "org.videolan.VLC", "org.gimp.GIMP",
                    "com.obsproject.Studio", "org.libreoffice.LibreOffice"]
    if apps_reales:   # modo «logos reales»: todo viene de Flathub, el resto no se usa
        apps = dict(apps_reales)
        recomendadas = [i for i in recomendadas + ["com.spotify.Client", "org.prismlauncher.PrismLauncher",
                                                   "org.vinegarhq.Sober"] if i in apps]
    orden_destacados = list(apps)

    def revisar() -> None:
        time.sleep(control.latencia)
        if control.sin_red:
            raise SinRed("sin red")

    class Catalogo:
        POR_PAGINA = 6   # el real da 30; aquí se ve el «Mostrar más»

        def __init__(self, cache_dir=None, idioma="es", api=None):
            self.idioma = idioma

        def destacados(self):
            revisar()
            return [apps[i] for i in orden_destacados[:12]]

        def recomendadas(self):
            revisar()
            return [apps[i] for i in recomendadas]

        def categorias(self):
            revisar()
            return list(CATEGORIAS)

        def por_categoria(self, cat_id, pagina=1):
            revisar()
            todas = [a for a in apps.values() if cat_id in a.categorias]
            return todas[(pagina - 1) * 6:pagina * 6]

        def buscar(self, texto):
            revisar()
            t = texto.casefold()
            return [a for a in apps.values() if t in a.nombre.casefold() or t in a.resumen.casefold()]

        def detalle(self, app_id):
            revisar()
            if app_id not in apps:
                raise AppNoEncontrada("No se encontró esa app en la tienda.")
            return apps[app_id]

    candado = threading.Lock()

    def _marchar(app_id, progreso, textos):
        pasos = 20
        for k in range(pasos + 1):
            time.sleep(control.duracion / pasos)
            if app_id in control.fallan and k == pasos // 2:
                raise ErrorFlatpak(f"No se pudo completar la operación con {app_id}: se acabó el espacio en disco.")
            progreso(int(100 * k / pasos), textos[min(len(textos) - 1, k * len(textos) // pasos)])

    def instaladas_() -> list[str]:
        return list(control.instaladas)

    def actualizaciones_() -> list[str]:
        if control.sin_red:
            raise SinRed("sin red")
        return list(control.actualizaciones)

    def instalar(app_id, progreso):
        control.registro.append(("instalar", app_id))
        _marchar(app_id, progreso, ["Descargando…", "Instalando…", "Terminando…"])
        with candado:
            if app_id not in control.instaladas:
                control.instaladas.append(app_id)

    def quitar(app_id, progreso):
        control.registro.append(("quitar", app_id))
        _marchar(app_id, progreso, ["Quitando…"])
        with candado:
            control.instaladas = [i for i in control.instaladas if i != app_id]
            control.actualizaciones = [i for i in control.actualizaciones if i != app_id]

    def actualizar(app_ids, progreso):
        ids = app_ids if app_ids is not None else list(control.actualizaciones)
        control.registro.append(("actualizar", tuple(ids)))
        _marchar(ids[0] if ids else "", progreso, ["Descargando…", "Actualizando…"])
        with candado:
            control.actualizaciones = [i for i in control.actualizaciones if i not in ids]

    def es_del_sistema(app_id):
        return app_id in control.del_sistema

    def abrir(app_id):
        control.abiertas.append(app_id)

    flatpak = SimpleNamespace(instaladas=instaladas_, actualizaciones=actualizaciones_, instalar=instalar,
                              quitar=quitar, actualizar=actualizar, abrir=abrir, preparar=lambda: None,
                              es_del_sistema=es_del_sistema)
    errores = SimpleNamespace(SinRed=SinRed, ErrorFlatpak=ErrorFlatpak, AppNoEncontrada=AppNoEncontrada,
                              FlatpakSinRed=FlatpakSinRed)
    motor = Motor(catalogo=Catalogo(), flatpak=flatpak, errores=errores)
    motor.control = control   # type: ignore[attr-defined]
    return motor
