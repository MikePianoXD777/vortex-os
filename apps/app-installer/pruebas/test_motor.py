"""Flathub para .deb/.rpm, Flatpak, mods de Minecraft y los textos en los dos idiomas."""
from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

import muestras
from conftest import escribir_script
from vortex_app_installer import flathub, motor, registro, textos

RAIZ = Path(__file__).resolve().parents[1]


def test_flathub_empareja_bien():
    hits = [{"id": "com.google.Chrome", "nombre": "Google Chrome", "resumen": "", "icono": ""},
            {"id": "com.visualstudio.code", "nombre": "Visual Studio Code", "resumen": "", "icono": ""},
            {"id": "com.discordapp.Discord", "nombre": "Discord", "resumen": "", "icono": ""}]
    assert flathub.mejor("google-chrome-stable", hits)["id"] == "com.google.Chrome"
    assert flathub.mejor("code", hits)["id"] == "com.visualstudio.code"
    assert flathub.mejor("Discord", hits)["id"] == "com.discordapp.Discord"
    assert flathub.mejor("libreoffice", hits) is None
    dev = [{"id": "com.google.ChromeDev", "nombre": "Google Chrome (unstable)", "resumen": "", "icono": ""}] + hits
    assert flathub.mejor("google-chrome-stable", dev)["id"] == "com.google.Chrome"
    assert flathub.mejor("google-chrome-unstable", dev)["id"] == "com.google.ChromeDev"
    assert flathub.mejor("google-chrome-stable", dev[:1]) is None
    assert flathub.para_buscar("google-chrome-stable") == "google chrome"
    assert flathub.para_buscar("obs-studio") == "obs studio"


def test_deb_ofrece_flathub(casa, monkeypatch):
    monkeypatch.setattr(flathub, "buscar", lambda texto, **k: [
        {"id": "com.google.Chrome", "nombre": "Google Chrome", "resumen": "El navegador", "icono": ""}])
    info = motor.analizar(muestras.deb(casa.descargas / "google-chrome-stable_current_amd64.deb"))
    vista = motor.para_mostrar(info)
    assert vista["flathub"]["id"] == "com.google.Chrome"
    assert vista["acciones"][0]["id"] == "flathub"
    r = motor.ejecutar(info, "flathub", vista["acciones"][0]["datos"])
    log = (casa.tmp / "flatpak.log").read_text()
    assert "remote-add --user --if-not-exists flathub" in log
    assert "install --user -y --noninteractive flathub com.google.Chrome" in log
    assert r["flatpak"] == "com.google.Chrome"
    assert [e["tipo"] for e in registro.cargar()] == ["flatpak"]
    motor.deshacer("flatpak-com.google.Chrome")
    assert "uninstall --user -y --noninteractive com.google.Chrome" in (casa.tmp / "flatpak.log").read_text()
    assert registro.cargar() == []


def test_deb_sin_flathub_saca_el_contenido(casa):
    info = motor.analizar(muestras.deb(casa.descargas / "raro_1.0_amd64.deb", "paquete-raro"))
    vista = motor.para_mostrar(info)
    assert not vista["flathub"] and vista["acciones"][0]["id"] == "ver_contenido"
    r = motor.ejecutar(info, "ver_contenido", {})
    assert (Path(r["carpeta"]) / "usr/share/doc/leeme.txt").read_text() == "hola"


def test_flatpakref(casa):
    ref = casa.descargas / "org.example.App.flatpakref"
    ref.write_text("[Flatpak Ref]\nName=org.example.App\nTitle=Ejemplo\n")
    r = motor.ejecutar(motor.analizar(ref), "instalar_flatpak", {})
    assert "install --user -y --noninteractive --from %s" % ref in (casa.tmp / "flatpak.log").read_text()
    assert r["flatpak"] == "org.example.App" and registro.cargar()[0]["nombre"] == "Ejemplo"


def _vortex_falso(casa, instancias):
    lineas = "\n".join(json.dumps(i) for i in instancias)
    escribir_script(casa.bins / "vortex", f"""#!/bin/sh
case "$1 $2" in
  "mods instancias") cat <<'FIN'
{lineas}
FIN
  ;;
  "mods mover") mkdir -p "$4/mods" && mv -n "$3" "$4/mods/" && echo "$4/mods/$(basename "$3")" ;;
  *) exit 1 ;;
esac
""")


def test_mod_a_una_instancia(casa):
    juego = casa.home / ".local/share/PrismLauncher/instances/Fabric/minecraft"
    juego.mkdir(parents=True)
    _vortex_falso(casa, [{"nombre": "Fabric 1.21", "launcher": "Prism", "ruta": str(juego), "version": "1.21.1",
                          "loader": "fabric"},
                         {"nombre": "Forge viejo", "launcher": "Prism", "ruta": str(casa.home / "x"),
                          "version": "1.20.1", "loader": "forge"}])
    mod = muestras.mod_fabric(casa.descargas / "sodium.jar")
    info = motor.analizar(mod)
    vista = motor.para_mostrar(info)
    assert [i["compatible"] for i in vista["instancias"]] == [True, False]
    r = motor.ejecutar(info, "mod_instancia", {"ruta": str(juego), "instancia": "Fabric 1.21"})
    assert (juego / "mods" / "sodium.jar").exists() and "Fabric 1.21" in r["titulo"]
    motor.deshacer(registro.cargar()[0]["id"])
    assert not (juego / "mods" / "sodium.jar").exists()


def test_mod_sin_vortex_mods_carpeta_a_mano(casa):
    (casa.bins / "vortex").unlink()
    escribir_script(casa.bins / "vortex", "#!/bin/sh\nexit 127\n")
    info = motor.analizar(muestras.mod_fabric(casa.descargas / "sodium.jar"))
    vista = motor.para_mostrar(info)
    assert vista["acciones"][0]["id"] == "mod_carpeta" and vista["acciones"][0]["elegirCarpeta"]
    juego = casa.home / ".minecraft"
    (juego / "mods").mkdir(parents=True)
    motor.ejecutar(info, "mod_carpeta", {"carpeta": str(juego)})
    assert (juego / "mods" / "sodium.jar").exists() and (casa.descargas / "sodium.jar").exists()


def test_textos_completos_en_los_dos_idiomas(monkeypatch):
    usadas = set()
    for p in (RAIZ / "vortex_app_installer").glob("*.py"):
        usadas |= set(re.findall(r'tr\("([a-z_0-9]+)"', p.read_text()))
    for p in (RAIZ / "vortex_app_installer/qml").glob("*.qml"):
        usadas |= set(re.findall(r"\bt\.([a-z_0-9]+)", p.read_text()))
    usadas |= set(motor._TIPOS.values()) | set(motor._TIPO_INSTALADO.values())
    faltan = sorted(u for u in usadas if u not in textos.TEXTOS)
    assert faltan == []
    for clave, (es, en) in textos.TEXTOS.items():
        assert es and en, clave
        assert set(re.findall(r"\{(\w+)\}", es)) == set(re.findall(r"\{(\w+)\}", en)), clave
    monkeypatch.setenv("VORTEX_APP_INSTALLER_IDIOMA", "en")
    assert textos.tr("hecho_instalado", nombre="X") == "“X” is installed"
    assert textos.tr("contenido_resumen", n=1, tam="1 KB") == "1 item · 1 KB in total"


def test_idioma_por_locale(monkeypatch):
    monkeypatch.delenv("VORTEX_APP_INSTALLER_IDIOMA", raising=False)
    monkeypatch.setenv("LC_ALL", "")
    monkeypatch.setenv("LC_MESSAGES", "")
    monkeypatch.setenv("LANG", "es_MX.UTF-8")
    assert textos.idioma() == "es"
    monkeypatch.setenv("LANG", "en_US.UTF-8")
    assert textos.idioma() == "en"


def test_tamano_legible(casa):
    assert motor.tamano_legible(500) == "500 bytes"
    assert motor.tamano_legible(1536) == "1,5 KB"
    assert motor.tamano_legible(147 * 1024 * 1024) == "147 MB"


def test_desktop_de_la_app_tiene_todos_los_mime():
    texto = (RAIZ / "share/applications/vortex-app-installer.desktop").read_text()
    mime = re.search(r"^MimeType=(.*)$", texto, re.M).group(1).split(";")
    for m in ("application/zip", "application/x-rar", "application/x-7z-compressed", "application/x-compressed-tar",
              "application/x-xz-compressed-tar", "application/x-zstd-compressed-tar", "application/x-bzip2-compressed-tar",
              "application/gzip", "application/x-xz", "application/zstd", "application/x-cd-image",
              "application/vnd.appimage", "application/x-shellscript", "application/vnd.debian.binary-package",
              "application/x-rpm", "application/vnd.flatpak.ref", "application/vnd.flatpak",
              "application/x-msdos-program", "application/x-msi", "application/x-java-archive"):
        assert m in mime, m
    assert re.search(r"^Exec=vortex-app-installer %f$", texto, re.M)
