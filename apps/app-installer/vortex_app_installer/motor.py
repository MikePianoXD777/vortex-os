"""El cerebro de la ventana, sin Qt: qué es el archivo, qué se le ofrece a la persona y hacerlo.

    info = motor.analizar(ruta)              # detectar + lo que tarda (Flathub, instancias…)
    vista = motor.para_mostrar(info)         # dict listo para la interfaz (textos ya traducidos)
    resultado = motor.ejecutar(info, "descomprimir", {}, progreso, cancelar)

`ejecutar` lanza `comprimidos.Conflicto` (el destino ya existe: la interfaz pregunta y vuelve a llamar
con {"nombre": …} o {"reemplazar": True}) y `comprimidos.NecesitaContrasena` (vuelve con
{"contrasena": …}); cualquier otro problema es un `Problema` con el mensaje para la persona.
"""
from __future__ import annotations

import os
from pathlib import Path

from . import comprimidos, detectar, flathub, flatpak, instalar, mods, paquetes, registro
from .comprimidos import Conflicto, NecesitaContrasena
from .detectar import Info
from .textos import idioma, tr

MAX_ENTRADAS = 400   # las que se muestran en la lista de contenido


class Problema(Exception):
    """Un error que se le muestra a la persona tal cual (ya traducido)."""

    def __init__(self, mensaje: str, comando: str = ""):
        super().__init__(mensaje)
        self.comando = comando   # un comando para copiar (lo que necesita root), si aplica


def tamano_legible(n: int) -> str:
    if n < 1024:
        return tr("bytes", n=n)
    for unidad in ("KB", "MB", "GB", "TB"):
        n /= 1024
        if n < 1024 or unidad == "TB":
            texto = ("%.0f" if n >= 100 else "%.1f") % n
            if texto.endswith(".0"):
                texto = texto[:-2]
            return (texto.replace(".", ",") if idioma() == "es" else texto) + " " + unidad
    return str(n)


# ------------------------------------------------------------------------------------ analizar

def analizar(ruta: Path | str, contrasena: str | None = None) -> Info:
    """detectar() más lo que necesita red o comandos: el programa en Flathub, las instancias de
    Minecraft, con qué abrir un .exe. Puede tardar unos segundos (se corre fuera de la ventana)."""
    info = detectar.detectar(ruta, contrasena)
    if info.tipo in ("deb", "rpm", "windows"):
        nombre = info.extra.get("nombre_programa", "")
        try:
            info.extra["flathub"] = flathub.mejor(nombre, flathub.buscar(flathub.para_buscar(nombre))) if nombre else None
        except flathub.SinRed:
            info.extra["flathub"] = None
            info.extra["flathub_sin_red"] = True
    if info.tipo == "windows":
        info.extra["opciones_windows"] = instalar.opciones_windows()
    if info.tipo == "mod":
        info.extra["instancias"] = mods.instancias()
    if info.tipo == "flatpakref" and not info.extra.get("nombre_programa"):
        info.extra["nombre_programa"] = Path(info.nombre).stem
    return info


# ------------------------------------------------------------------------------------ qué mostrar

_TIPOS = {"comprimido": "tipo_comprimido", "programa": "tipo_programa", "appimage": "tipo_appimage",
          "deb": "tipo_deb", "rpm": "tipo_rpm", "flatpakref": "tipo_flatpak", "flatpak": "tipo_flatpak",
          "windows": "tipo_windows", "mod": "tipo_mod", "jar": "tipo_jar", "desconocido": "tipo_desconocido",
          "carpeta": "tipo_carpeta", "falta": "tipo_falta"}


def _accion(id_: str, texto: str, tipo: str = "secundario", icono: str = "", confirmar: dict | None = None,
            carpeta: bool = False, datos: dict | None = None) -> dict:
    return {"id": id_, "texto": texto, "tipo": tipo, "icono": icono, "confirmar": confirmar or {},
            "elegirCarpeta": carpeta, "datos": datos or {}}


def _confirmar(titulo: str, texto: str, aceptar: str) -> dict:
    return {"titulo": titulo, "texto": texto, "aceptar": aceptar}


def tipo_texto(info: Info) -> str:
    if info.tipo == "script":
        return tr("tipo_programa_suelto") if info.extra.get("binario") else tr("tipo_script")
    return tr(_TIPOS.get(info.tipo, "tipo_desconocido"))


def para_mostrar(info: Info) -> dict:
    """Lo que la interfaz necesita para dibujar el archivo: textos, avisos, lista de contenido y botones."""
    nombre_corto = Path(info.ruta).parent.name
    vista = {
        "ruta": info.ruta, "nombre": info.nombre, "tipo": info.tipo, "tipoTexto": tipo_texto(info),
        "tamano": tamano_legible(info.tamano), "origen": "", "aviso": "", "descripcion": "", "nota": "",
        "error": info.error, "comando": info.falta_comando, "entradas": [], "cuantas": 0, "contenido": "",
        "vista": "", "comandoAdmin": "", "flathub": {}, "instancias": [], "acciones": [],
        "pideContrasena": bool(info.extra.get("pide_contrasena")),
    }
    sitio = detectar.sitio(info.origen)
    if sitio:
        vista["origen"] = tr("origen_sitio", sitio=sitio)
    elif info.de_internet:
        vista["origen"] = tr("origen_descargas")
    else:
        vista["origen"] = tr("origen_carpeta", carpeta=nombre_corto or "/")
    if info.ejecuta_codigo and info.de_internet:
        vista["aviso"] = tr("aviso_internet")

    acciones = vista["acciones"]
    t = info.tipo
    if t in ("comprimido", "programa"):
        entradas = info.entradas
        vista["cuantas"] = len(entradas)
        vista["contenido"] = tr("contenido_resumen", n=len(entradas),
                                tam=tamano_legible(comprimidos.tamano_total(entradas)))
        vista["entradas"] = [{"nombre": e.nombre, "carpeta": e.tipo == "carpeta", "enlace": e.tipo in ("enlace", "duro"),
                              "tamano": tamano_legible(e.tamano) if e.tipo == "archivo" else "",
                              "peligrosa": comprimidos.peligrosa(e)} for e in entradas[:MAX_ENTRADAS]]
        peligrosas = sum(1 for e in entradas if comprimidos.peligrosa(e))
        if peligrosas:
            vista["nota"] = tr("nota_peligrosas", n=peligrosas)
        destino = comprimidos.destino_sugerido(Path(info.ruta), entradas, Path(info.ruta).parent) if entradas \
            else Path(info.ruta).parent / comprimidos.nombre_sin_extension(info.nombre)
        if t == "programa":
            carpeta, _ = instalar.plan_programa(entradas, Path(info.ruta))
            vista["descripcion"] = tr("desc_programa", carpeta="~/.local/opt/" + carpeta)
            acciones.append(_accion("instalar_programa", tr("boton_instalar"), "primario", "descargar"))
            acciones.append(_accion("descomprimir", tr("boton_descomprimir_aqui"), icono="caja"))
            acciones.append(_accion("descomprimir", tr("boton_descomprimir_en"), icono="carpeta", carpeta=True))
        elif vista["pideContrasena"] or info.error or entradas:
            vista["descripcion"] = tr("desc_comprimido", destino=destino.name) if entradas else tr("desc_comprimido_sin_lista")
            if not info.error:
                acciones.append(_accion("descomprimir", tr("boton_descomprimir"), "primario", "caja"))
                acciones.append(_accion("descomprimir", tr("boton_descomprimir_en"), icono="carpeta", carpeta=True))
        else:
            vista["descripcion"] = tr("desc_comprimido_vacio")
    elif t == "appimage":
        vista["descripcion"] = tr("desc_appimage")
        acciones.append(_accion("instalar_appimage", tr("boton_instalar"), "primario", "descargar",
                                _confirmar(tr("conf_instalar_titulo", nombre=info.nombre), tr("conf_instalar_appimage"),
                                           tr("boton_instalar")) if vista["aviso"] else None))
        acciones.append(_accion("abrir_appimage", tr("boton_solo_abrir"), icono="abrir",
                                confirmar=_confirmar(tr("conf_abrir_titulo", nombre=info.nombre),
                                                     tr("conf_abrir_appimage"), tr("boton_abrir"))))
    elif t == "script":
        vista["vista"] = info.extra.get("vista", "")
        vista["descripcion"] = tr("desc_binario") if info.extra.get("binario") else tr("desc_script")
        if info.extra.get("pide_admin"):
            vista["comandoAdmin"] = info.extra.get("comando_admin", "")
        acciones.append(_accion("correr", tr("boton_abrir_terminal"), "primario", "terminal",
                                _confirmar(tr("conf_correr_titulo", nombre=info.nombre), tr("conf_correr"),
                                           tr("boton_abrir_terminal"))))
    elif t in ("deb", "rpm"):
        app = info.extra.get("flathub")
        base = tr("desc_deb") if t == "deb" else tr("desc_rpm")
        nombre = info.extra.get("nombre_programa") or info.nombre
        if app:
            vista["flathub"] = app
            vista["descripcion"] = base + " " + tr("desc_en_flathub", nombre=app["nombre"])
            acciones.append(_accion("flathub", tr("boton_instalar_flathub"), "primario", "descargar",
                                    datos={"app": app["id"], "nombre": app["nombre"]}))
        elif info.extra.get("flathub_sin_red"):
            vista["descripcion"] = base + " " + tr("desc_flathub_sin_red")
        else:
            vista["descripcion"] = base + " " + tr("desc_no_flathub", nombre=nombre)
        acciones.append(_accion("ver_contenido", tr("boton_ver_contenido"), "primario" if not app else "secundario",
                                "caja"))
    elif t in ("flatpakref", "flatpak"):
        if not flatpak.hay_flatpak():
            vista["descripcion"] = tr("falta_flatpak")
            vista["comando"] = "sudo pacman -S flatpak"
        else:
            nombre = info.extra.get("nombre_programa") or Path(info.nombre).stem
            vista["descripcion"] = tr("desc_flatpakref", nombre=nombre) if t == "flatpakref" else tr("desc_flatpak")
            acciones.append(_accion("instalar_flatpak", tr("boton_instalar"), "primario", "descargar"))
    elif t == "windows":
        opciones = info.extra.get("opciones_windows") or []
        app = info.extra.get("flathub")
        texto = tr("desc_windows_con", programa=opciones[0]["nombre"]) if opciones else tr("desc_windows_sin")
        if app:
            vista["flathub"] = app
            texto += " " + tr("desc_windows_flathub", nombre=app["nombre"])
            acciones.append(_accion("flathub", tr("boton_instalar_flathub_linux"), "primario", "descargar",
                                    datos={"app": app["id"], "nombre": app["nombre"]}))
        vista["descripcion"] = texto
        for o in opciones:
            acciones.append(_accion("windows", tr("boton_abrir_con", programa=o["nombre"]),
                                    "secundario" if app or acciones else "primario", "abrir",
                                    _confirmar(tr("conf_abrir_titulo", nombre=info.nombre), tr("conf_windows"),
                                               tr("boton_abrir")), datos={"opcion": o["id"]}))
        if not opciones:
            vista["comando"] = "sudo pacman -S wine"
    elif t == "mod":
        mod = info.extra.get("mod") or {}
        loader = {"fabric": "Fabric", "quilt": "Quilt", "forge": "Forge", "neoforge": "NeoForge"}.get(mod.get("loader"), "")
        vista["descripcion"] = tr("desc_mod", nombre=mod.get("nombre") or info.nombre, loader=loader)
        vista["mod"] = mod
        instancias = info.extra.get("instancias")
        if instancias:
            vista["instancias"] = [dict(i, compatible=not i.get("loader") or i.get("loader") == mod.get("loader")
                                        or (mod.get("loader") == "fabric" and i.get("loader") == "quilt"))
                                   for i in instancias]
            vista["nota"] = tr("nota_mod_instancias")
        elif instancias is None:
            vista["nota"] = tr("nota_mod_sin_comando")
        else:
            vista["nota"] = tr("nota_mod_sin_instancias")
        acciones.append(_accion("mod_carpeta", tr("boton_elegir_carpeta"), "primario" if not instancias else "secundario",
                                "carpeta", carpeta=True))
    elif t == "jar":
        vista["descripcion"] = tr("desc_jar")
        acciones.append(_accion("java", tr("boton_abrir_java"), "primario", "abrir",
                                _confirmar(tr("conf_abrir_titulo", nombre=info.nombre), tr("conf_java"), tr("boton_abrir"))))
        if not instalar.shutil.which("java"):
            vista["comando"] = "sudo pacman -S jre-openjdk"
            acciones.clear()
    elif t == "carpeta":
        vista["descripcion"] = tr("desc_carpeta")
    elif t == "falta":
        vista["descripcion"] = tr("desc_falta")
    else:
        vista["descripcion"] = tr("desc_desconocido")
    if t not in ("falta",):
        acciones.append(_accion("abrir_carpeta", tr("boton_abrir_carpeta"), "primario" if not acciones else "fantasma",
                                "carpeta"))
    return vista


# ------------------------------------------------------------------------------------ hacer

def _resultado(titulo: str, texto: str, carpeta: str = "", abrir: str = "", deshacible: bool = True, **extra) -> dict:
    return {"titulo": titulo, "texto": texto, "carpeta": carpeta, "abrir": abrir, "deshacible": deshacible, **extra}


def _corto(ruta: Path) -> str:
    """«/home/mike/Descargas/x» → «~/Descargas/x»."""
    casa = str(instalar.rutas.casa())
    s = str(ruta)
    return "~" + s[len(casa):] if s == casa or s.startswith(casa + os.sep) else s


def ejecutar(info: Info, accion: str, opciones: dict | None = None, progreso=None, cancelar=None) -> dict:
    opciones = opciones or {}
    ruta = Path(info.ruta)
    contrasena = opciones.get("contrasena") or None
    try:
        if accion == "descomprimir":
            return _descomprimir(info, ruta, opciones, contrasena, progreso, cancelar)
        if accion == "instalar_programa":
            entradas = info.entradas or comprimidos.listar(ruta, contrasena)
            e = instalar.instalar_programa(ruta, entradas, bool(opciones.get("reemplazar")), contrasena, progreso, cancelar)
            return _resultado(tr("hecho_instalado", nombre=e["nombre"]), tr("hecho_en_lanzador"), abrir=e["abrir"],
                              id=e["id"])
        if accion == "instalar_appimage":
            e = instalar.instalar_appimage(ruta, bool(opciones.get("reemplazar")), progreso, cancelar)
            return _resultado(tr("hecho_instalado", nombre=e["nombre"]), tr("hecho_en_lanzador"), abrir=e["abrir"],
                              id=e["id"])
        if accion == "abrir_appimage":
            instalar.abrir_appimage(ruta)
            return _resultado(tr("hecho_abierto", nombre=info.nombre), tr("hecho_abierto_appimage"), deshacible=False)
        if accion == "correr":
            instalar.correr_script(ruta)
            return _resultado(tr("hecho_terminal"), tr("hecho_terminal_texto"), deshacible=False)
        if accion == "flathub":
            app_id = opciones.get("app") or (info.extra.get("flathub") or {}).get("id")
            nombre = opciones.get("nombre") or (info.extra.get("flathub") or {}).get("nombre") or app_id
            if not app_id:
                raise Problema(tr("flatpak_fallo"))
            flatpak.instalar_flathub(app_id, progreso, cancelar)
            e = registro.agregar({"id": "flatpak-" + app_id, "tipo": "flatpak", "nombre": nombre, "flatpak": app_id,
                                  "archivo": info.nombre, "rutas": []})
            return _resultado(tr("hecho_instalado", nombre=nombre), tr("hecho_en_lanzador"), flatpak=app_id, id=e["id"])
        if accion == "instalar_flatpak":
            app_id = flatpak.instalar_archivo(ruta, info.tipo, progreso, cancelar)
            nombre = info.extra.get("nombre_programa") or app_id or info.nombre
            if app_id:
                registro.agregar({"id": "flatpak-" + app_id, "tipo": "flatpak", "nombre": nombre, "flatpak": app_id,
                                  "archivo": info.nombre, "rutas": []})
            return _resultado(tr("hecho_instalado", nombre=nombre), tr("hecho_en_lanzador"), flatpak=app_id or "",
                              deshacible=bool(app_id))
        if accion == "ver_contenido":
            return _ver_contenido(info, ruta, opciones, progreso, cancelar)
        if accion == "windows":
            instalar.abrir_windows(ruta, opciones.get("opcion", ""))
            return _resultado(tr("hecho_abierto", nombre=info.nombre), tr("hecho_windows"), deshacible=False)
        if accion == "java":
            instalar.abrir_java(ruta)
            return _resultado(tr("hecho_abierto", nombre=info.nombre), tr("hecho_java"), deshacible=False)
        if accion in ("mod_instancia", "mod_carpeta"):
            return _mod(info, ruta, accion, opciones)
        if accion == "abrir_carpeta":
            instalar.abrir_carpeta(ruta)
            return {}
    except (Conflicto, NecesitaContrasena, comprimidos.Cancelado, flatpak.Cancelado):
        raise
    except comprimidos.FaltaPrograma as e:
        raise Problema(str(e), e.comando) from None
    except (comprimidos.ErrorComprimido, instalar.ErrorInstalar, flatpak.ErrorFlatpak) as e:
        raise Problema(str(e)) from None
    except PermissionError:
        raise Problema(tr("sin_permiso")) from None
    except OSError as e:
        if e.errno == 28:
            raise Problema(tr("sin_espacio")) from None
        raise Problema(tr("error_general", detalle=e.strerror or str(e))) from None
    except RuntimeError as e:
        raise Problema(str(e)) from None
    raise Problema(tr("error_general", detalle=accion))


def _descomprimir(info: Info, ruta: Path, opciones: dict, contrasena, progreso, cancelar) -> dict:
    entradas = info.entradas
    if not entradas or contrasena:
        entradas = comprimidos.listar(ruta, contrasena)
        info.entradas = entradas
    padre = Path(opciones.get("carpeta") or ruta.parent)
    destino = padre / opciones["nombre"] if opciones.get("nombre") else \
        comprimidos.destino_sugerido(ruta, entradas, padre)
    r = comprimidos.extraer(ruta, destino, entradas, contrasena, progreso, cancelar)
    registro.agregar({"id": registro.id_libre("descomprimido", r.destino.name), "tipo": "descomprimido",
                      "nombre": r.destino.name, "rutas": [str(r.destino)], "archivo": info.nombre,
                      "abrir": str(r.destino)})
    texto = tr("hecho_en_carpeta", carpeta=_corto(r.destino))
    if r.saltados:
        texto += " " + tr("hecho_saltados", n=len(r.saltados))
    return _resultado(tr("hecho_descomprimido"), texto, carpeta=str(r.destino), saltados=len(r.saltados))


def _ver_contenido(info: Info, ruta: Path, opciones: dict, progreso, cancelar) -> dict:
    """Saca lo que trae un .deb (su data.tar) o un .rpm a una carpeta, solo para verlo."""
    padre = Path(opciones.get("carpeta") or ruta.parent)
    destino = padre / (opciones.get("nombre") or comprimidos.nombre_sin_extension(info.nombre))
    if os.path.lexists(destino):
        raise Conflicto(destino, comprimidos.nombre_libre(destino))
    if info.tipo == "deb":
        import tempfile
        with tempfile.TemporaryDirectory(prefix="vortex-deb-") as tmp:
            nombres = [e.nombre for e in comprimidos.listar(ruta)]
            datos = next((n for n in nombres if n.startswith("data.tar")), None)
            if not datos:
                raise Problema(tr("comprimido_danado"))
            r = comprimidos._correr([comprimidos._bsdtar(), "-xf", str(ruta), "-C", tmp, datos])
            if r.returncode != 0:
                raise Problema(tr("comprimido_danado"))
            interior = Path(tmp) / datos
            res = comprimidos.extraer(interior, destino, None, None, progreso, cancelar, quitar_raiz=False)
    else:
        res = comprimidos.extraer(ruta, destino, None, None, progreso, cancelar, quitar_raiz=False)
    registro.agregar({"id": registro.id_libre("descomprimido", res.destino.name), "tipo": "descomprimido",
                      "nombre": res.destino.name, "rutas": [str(res.destino)], "archivo": info.nombre,
                      "abrir": str(res.destino)})
    return _resultado(tr("hecho_descomprimido"), tr("hecho_en_carpeta", carpeta=_corto(res.destino)),
                      carpeta=str(res.destino))


def _mod(info: Info, ruta: Path, accion: str, opciones: dict) -> dict:
    nombre_mod = (info.extra.get("mod") or {}).get("nombre") or info.nombre
    if accion == "mod_instancia":
        destino = mods.mover(ruta, opciones.get("ruta", ""))
        donde = opciones.get("instancia") or Path(opciones.get("ruta", "")).name
    else:
        destino = mods.copiar_a_carpeta(ruta, Path(opciones.get("carpeta", "")), opciones.get("nombre"))
        donde = destino.parent.parent.name if destino.parent.name == "mods" else destino.parent.name
    registro.agregar({"id": registro.id_libre("mod", nombre_mod), "tipo": "mod", "nombre": nombre_mod,
                      "rutas": [str(destino)], "archivo": info.nombre, "abrir": str(destino.parent),
                      "detalle": donde})
    return _resultado(tr("hecho_mod", nombre=nombre_mod, donde=donde), tr("hecho_mod_texto"), carpeta=str(destino.parent))


# ------------------------------------------------------------------------------------ pestaña «Instalado»

_TIPO_INSTALADO = {"appimage": "instalado_appimage", "programa": "instalado_programa", "flatpak": "instalado_flatpak",
                   "mod": "instalado_mod", "descomprimido": "instalado_descomprimido"}


def instalados() -> list[dict]:
    salida = []
    for e in registro.cargar():
        fecha = e.get("fecha", "")[:10]
        if len(fecha) == 10:
            a, m, d = fecha.split("-")
            fecha = "%s/%s/%s" % (d, m, a) if idioma() == "es" else "%s-%s-%s" % (a, m, d)
        salida.append({"id": e["id"], "tipo": e["tipo"], "nombre": e.get("nombre") or e["id"],
                       "tipoTexto": tr("instalado_descomprimido_archivo") if e["tipo"] == "descomprimido"
                       and os.path.isfile(e.get("abrir") or "") else tr(_TIPO_INSTALADO.get(e["tipo"], "instalado_programa")),
                       "fecha": fecha, "icono": e.get("icono") or "", "abrir": e.get("abrir") or "",
                       "detalle": e.get("detalle") or e.get("archivo") or "",
                       "boton": tr("boton_desinstalar") if e["tipo"] in ("appimage", "programa", "flatpak")
                       else tr("boton_quitar")})
    return salida


def deshacer(id_: str) -> str:
    try:
        e = registro.deshacer(id_)
    except ValueError:
        raise Problema(tr("ya_no_esta")) from None
    except flatpak.ErrorFlatpak as e:
        raise Problema(str(e)) from None
    except OSError as e:
        raise Problema(tr("error_general", detalle=e.strerror or str(e))) from None
    clave = "deshecho_app" if e["tipo"] in ("appimage", "programa", "flatpak") else "deshecho_otro"
    return tr(clave, nombre=e.get("nombre") or id_)


def abrir_instalado(id_: str) -> None:
    e = registro.buscar(id_)
    if e is None:
        raise Problema(tr("ya_no_esta"))
    if e["tipo"] == "flatpak":
        flatpak.abrir(e["flatpak"])
    elif e["tipo"] == "appimage":
        instalar.lanzar([e["abrir"]], env=instalar._entorno_appimage())
    elif e["tipo"] == "programa":
        instalar.lanzar([e["abrir"]], carpeta=Path(e["abrir"]).parent)
    else:
        instalar.abrir_carpeta(Path(e["abrir"]))
