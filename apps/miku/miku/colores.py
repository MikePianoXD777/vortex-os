"""Colores y escala de Miku: siguen al fondo de pantalla como el resto del escritorio (y como la tienda).

El shell (Serpantinum) genera con matugen la paleta del fondo en ~/.local/state/serpantinum/qs_colors.json
(claves estilo Catppuccin: base, mantle, text, blue…). Aquí se lee, se vigila (matugen reescribe el archivo:
se vuelve a vigilar tras cada cambio) y se convierte en los colores de la interfaz, asegurando contraste de
texto >= 4.5:1. Si el archivo no existe o está roto, se usa la paleta fija de Vortexine's. La conversión es
la misma que usa Vortexine's Shop (apps/shop/vortex_shop/colores.py).

También lee, solo para leer, general.uiScale de ~/.config/serpantinum/settings.json: el tamaño de la
interfaz que eligió la persona en Ajustes.

Para pruebas: VORTEX_MIKU_COLORES=/ruta/al/json cambia el archivo de la paleta.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

from . import rutas

RADIO = 18          # esquinas de las tarjetas, como en la 1.2

# Paleta fija de Vortexine's (la del instalador): respaldo cuando no hay paleta del fondo.
RESPALDO: dict[str, str] = {
    "fondo": "#0b0d1a", "barra": "#11142a", "tarjeta": "#141833", "tarjetaHover": "#191e40",
    "campo": "#161a36", "borde": "#232850", "bordeCampo": "#2c3366", "bordeHover": "#3d4896",
    "texto": "#e8ebff", "suave": "#c3c8ea", "tenue": "#a3aad6",
    "azul": "#9ec5ff", "azulHover": "#b8d6ff", "azulPresion": "#86b2f2", "violeta": "#c6a8ff",
    "sobreAcento": "#0b0d1a", "peligro": "#ff9db0", "exito": "#8fe3b4",
    "realce": "#1a1f45", "realceFuerte": "#232a5c", "relleno": "#1f2552",
    "barraSel": "#222a5e", "barraHover": "#191e45",
    "peligroFondo": "#2b1a33", "peligroBorde": "#6b3550", "velo": "#cc060812", "flotante": "#e60b0d1a",
}


# --- Aritmética de color ---

def _rgb(h: str) -> tuple[float, float, float]:
    h = h.lstrip("#")
    if len(h) == 8:   # #AARRGGBB (se ignora el alfa)
        h = h[2:]
    return int(h[0:2], 16) / 255, int(h[2:4], 16) / 255, int(h[4:6], 16) / 255


def _hex(c: tuple[float, float, float]) -> str:
    return "#" + "".join(f"{max(0, min(255, round(v * 255))):02x}" for v in c)


def mezclar(a: str, b: str, t: float) -> str:
    """a + (b - a) * t, por canal."""
    ra, rb = _rgb(a), _rgb(b)
    return _hex(tuple(x + (y - x) * t for x, y in zip(ra, rb)))


def luminancia(h: str) -> float:
    def lin(v: float) -> float:
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (lin(v) for v in _rgb(h))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contraste(a: str, b: str) -> float:
    la, lb = luminancia(a), luminancia(b)
    claro, oscuro = max(la, lb), min(la, lb)
    return (claro + 0.05) / (oscuro + 0.05)


def asegurar(color: str, fondos: list[str], minimo: float = 4.5) -> str:
    """Acerca `color` al blanco o al negro (el que contraste más con los fondos) hasta llegar al contraste."""
    peor = min(fondos, key=lambda f: contraste(color, f))
    destino = "#ffffff" if luminancia(peor) < 0.4 else "#000000"
    nuevo = color
    for paso in range(1, 21):
        if min(contraste(nuevo, f) for f in fondos) >= minimo:
            return nuevo
        nuevo = mezclar(color, destino, paso * 0.05)
    return nuevo


def texto_sobre(acento: str, candidatos: list[str]) -> str:
    """El primer candidato que dé 4.5:1 sobre el acento; si ninguno, negro o blanco según la luminancia."""
    for c in candidatos:
        if contraste(c, acento) >= 4.5:
            return c
    return "#000000" if contraste("#000000", acento) >= contraste("#ffffff", acento) else "#ffffff"


def _valido(j: dict, clave: str) -> str | None:
    v = j.get(clave)
    if isinstance(v, str) and len(v) in (7, 9) and v.startswith("#"):
        try:
            _rgb(v)
            return v
        except ValueError:
            return None
    return None


def desde_matugen(j: dict) -> dict[str, str] | None:
    """Paleta de matugen (qs_colors.json) → colores de la interfaz. None si le faltan claves esenciales."""
    g = lambda k, otro=None: _valido(j, k) or (_valido(j, otro) if otro else None)  # noqa: E731
    base, mantle, surface0 = g("base", "crust"), g("mantle", "base"), g("surface0")
    surface1, surface2 = g("surface1", "surface0"), g("surface2", "surface1")
    texto, subtexto, azul = g("text"), g("subtext0", "text"), g("blue")
    if None in (base, mantle, surface0, surface1, surface2, texto, subtexto, azul):
        return None
    crust = g("crust", "base")
    violeta = g("peach") or g("mauve") or azul
    peligro = g("red") or RESPALDO["peligro"]
    exito = g("green") or RESPALDO["exito"]
    fondos = [base, mantle, surface0, surface1]   # sobre lo que se lee texto
    texto = asegurar(texto, fondos, 7)
    suave = asegurar(mezclar(texto, subtexto, 0.45), fondos)
    tenue = asegurar(subtexto, fondos)
    azul = asegurar(azul, [base, surface0])
    violeta = asegurar(violeta, [base, surface0])
    peligro = asegurar(peligro, [base, surface0])
    exito = asegurar(exito, [base, surface0])
    oscuro = luminancia(base) < 0.4
    azul_hover = mezclar(azul, "#ffffff" if oscuro else "#000000", 0.22)
    azul_presion = mezclar(azul, "#000000" if oscuro else "#ffffff", 0.14)
    # El texto del botón debe leerse sobre el azul normal, el de «hover» y el de «presionado»; si ningún
    # candidato lo logra en los tres, los estados se acercan al azul normal (que con negro o blanco siempre da).
    estados = [azul, azul_hover, azul_presion]
    candidatos = [c for c in (crust, base, "#000000", "#ffffff") if c]
    sobre = next((c for c in candidatos if min(contraste(c, e) for e in estados) >= 4.5), None)
    if sobre is None:
        sobre = texto_sobre(azul, candidatos)
        for t in (0.75, 0.5, 0.25, 0.0):
            azul_hover = mezclar(azul, azul_hover, t) if t else azul
            azul_presion = mezclar(azul, azul_presion, t) if t else azul
            if min(contraste(sobre, e) for e in (azul_hover, azul_presion)) >= 4.5:
                break
    return {
        "fondo": base, "barra": mantle, "tarjeta": surface0, "tarjetaHover": mezclar(surface0, surface1, 0.6),
        "campo": surface1, "borde": surface2, "bordeCampo": mezclar(surface2, subtexto, 0.25),
        "bordeHover": mezclar(surface2, azul, 0.45),
        "texto": texto, "suave": suave, "tenue": tenue,
        "azul": azul, "azulHover": azul_hover, "azulPresion": azul_presion, "violeta": violeta,
        "sobreAcento": sobre, "peligro": peligro, "exito": exito,
        "realce": mezclar(surface0, azul, 0.10), "realceFuerte": mezclar(surface0, azul, 0.22),
        "relleno": mezclar(surface0, texto, 0.10),
        "barraSel": mezclar(mantle, azul, 0.20), "barraHover": mezclar(mantle, azul, 0.09),
        "peligroFondo": mezclar(surface0, peligro, 0.14), "peligroBorde": mezclar(surface0, peligro, 0.45),
        "velo": "#cc" + mezclar(base, "#000000", 0.35).lstrip("#"),
        "flotante": "#e6" + base.lstrip("#")[-6:],
    }


def ruta_paleta() -> Path:
    return Path(os.environ.get("VORTEX_MIKU_COLORES") or rutas.PALETA_SHELL)


def leer(ruta: Path | None = None) -> dict[str, str]:
    """La paleta del archivo, o el respaldo si no existe o está roto."""
    try:
        j = json.loads((ruta or ruta_paleta()).read_text(encoding="utf-8"))
        paleta = desde_matugen(j) if isinstance(j, dict) else None
    except (OSError, ValueError):
        paleta = None
    return paleta or dict(RESPALDO)


def a_css(color: str) -> str:
    """«#rrggbb» o «#aarrggbb» (Qt) → «rgba(r, g, b, a)» para CSS."""
    h = color.lstrip("#")
    a = int(h[0:2], 16) / 255 if len(h) == 8 else 1.0
    r, g, b = (round(v * 255) for v in _rgb(color))
    return f"rgba({r}, {g}, {b}, {a:.2f})"


def con_alfa(color: str, alfa: float) -> str:
    """El mismo color con otro alfa, como «#aarrggbb» (el formato de Qt)."""
    return f"#{round(max(0.0, min(1.0, alfa)) * 255):02x}{_hex(_rgb(color))[1:]}"


def escala_ui(ruta: Path | None = None) -> float:
    """general.uiScale de los ajustes del shell (1.0 si no está o no es razonable). Solo lectura."""
    try:
        j = json.loads((ruta or rutas.AJUSTES_SHELL).read_text(encoding="utf-8"))
        v = j.get("general", {}).get("uiScale")
    except (OSError, ValueError, AttributeError):
        return 1.0
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not 0.6 <= float(v) <= 1.6:
        return 1.0
    return float(v)


def vigilante(al_cambiar):
    """Un QObject que llama a `al_cambiar(paleta)` cuando matugen reescribe el archivo de la paleta."""
    from PySide6.QtCore import QFileSystemWatcher, QObject, QTimer

    class Vigilante(QObject):
        def __init__(self):
            super().__init__()
            self._ruta = ruta_paleta()
            self.paleta = leer(self._ruta)
            self._v = QFileSystemWatcher(self)
            self._espera = QTimer(self)               # matugen escribe en varios pasos: se espera a que se calme
            self._espera.setSingleShot(True)
            self._espera.setInterval(150)
            self._espera.timeout.connect(self._recargar)
            self._v.fileChanged.connect(lambda _: self._espera.start())
            self._v.directoryChanged.connect(lambda _: self._espera.start())
            self._vigilar()

        def _vigilar(self) -> None:
            """Vigila el archivo (si existe) y su carpeta (para enterarse si se crea o se reemplaza)."""
            if self._ruta.exists() and str(self._ruta) not in self._v.files():
                self._v.addPath(str(self._ruta))      # al reemplazarse el archivo se pierde: se vuelve a agregar
            if self._ruta.parent.is_dir() and str(self._ruta.parent) not in self._v.directories():
                self._v.addPath(str(self._ruta.parent))

        def _recargar(self) -> None:
            self._vigilar()
            nueva = leer(self._ruta)
            if nueva != self.paleta:
                self.paleta = nueva
                al_cambiar(nueva)

    return Vigilante()
