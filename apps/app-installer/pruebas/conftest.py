"""Todas las pruebas corren en un HOME temporal: nada toca la carpeta personal de verdad, ni el lanzador,
ni Flatpak, ni Minecraft, ni la red. `vortex` y `flatpak` son scripts falsos en un PATH propio."""
from __future__ import annotations

import os
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from vortex_app_installer import flathub, instalar  # noqa: E402


def escribir_script(ruta: Path, texto: str) -> Path:
    ruta.write_text(texto, encoding="utf-8")
    ruta.chmod(0o755)
    return ruta


@pytest.fixture
def casa(tmp_path, monkeypatch):
    home = tmp_path / "casa"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    for v in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "APPIMAGE_EXTRACT_AND_RUN"):
        monkeypatch.delenv(v, raising=False)
    monkeypatch.setenv("VORTEX_APP_INSTALLER_IDIOMA", "es")
    bins = tmp_path / "bin"
    bins.mkdir()
    monkeypatch.setenv("PATH", "%s:%s" % (bins, os.environ.get("PATH", "")))
    # Por defecto no hay «vortex mods» ni Flatpak de verdad: scripts que no hacen nada.
    escribir_script(bins / "vortex", "#!/bin/sh\nexit 1\n")
    escribir_script(bins / "flatpak", '#!/bin/sh\necho "$@" >> "%s/flatpak.log"\nexit 0\n' % tmp_path)
    lanzados: list[list[str]] = []
    monkeypatch.setattr(instalar, "lanzar", lambda args, env=None, carpeta=None: lanzados.append(list(args)))

    def sin_red(*_a, **_k):
        raise flathub.SinRed("pruebas sin red")
    monkeypatch.setattr(flathub, "buscar", sin_red)
    descargas = home / "Descargas"
    descargas.mkdir()
    return SimpleNamespace(home=home, bins=bins, tmp=tmp_path, lanzados=lanzados, descargas=descargas)
