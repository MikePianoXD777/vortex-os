"""Detectar cada formato por su firma y su extensión."""
from __future__ import annotations

import subprocess
import tarfile

import pytest

import muestras
from vortex_app_installer import detectar, motor


def _bsdtar_crear(destino, formato, carpeta):
    (carpeta / "dentro.txt").write_text("hola")
    subprocess.run(["bsdtar", "--format", formato, "-cf", str(destino), "-C", str(carpeta), "dentro.txt"], check=True)
    return destino


@pytest.mark.parametrize("nombre,modo", [("a.tar.gz", "w:gz"), ("a.tgz", "w:gz"), ("a.tar.xz", "w:xz"),
                                         ("a.tar.bz2", "w:bz2"), ("a.tar.zst", "w:zst"), ("a.tar", "w")])
def test_tar_de_todos_los_sabores(casa, nombre, modo):
    ruta = muestras.tar(casa.descargas / nombre, [("carpeta/uno.txt", b"1"), ("carpeta/dos.txt", b"22")], modo)
    info = detectar.detectar(ruta)
    assert info.tipo == "comprimido"
    assert sorted(e.nombre for e in info.entradas if e.tipo == "archivo") == ["carpeta/dos.txt", "carpeta/uno.txt"]
    assert info.de_internet   # está en Descargas


def test_zip_7z_iso(casa):
    muestras.zip_(casa.descargas / "a.zip", {"x/y.txt": b"y"})
    otra = casa.tmp / "fuente"
    otra.mkdir()
    _bsdtar_crear(casa.descargas / "a.7z", "7zip", otra)
    _bsdtar_crear(casa.descargas / "a.iso", "iso9660", otra)
    for n, fmt in (("a.zip", "zip"), ("a.7z", "7z"), ("a.iso", "iso")):
        info = detectar.detectar(casa.descargas / n)
        assert (info.tipo, info.formato) == ("comprimido", fmt), n
        assert info.entradas, n


def test_comprimido_sin_extension_por_firma(casa):
    ruta = muestras.zip_(casa.home / "sin_nombre", {"a.txt": b"a"})
    info = detectar.detectar(ruta)
    assert (info.tipo, info.formato) == ("comprimido", "zip")
    assert not info.de_internet


def test_sueltos(casa):
    import bz2
    import gzip
    import lzma
    from compression import zstd
    for ext, mod in ((".gz", gzip), (".xz", lzma), (".bz2", bz2), (".zst", zstd)):
        ruta = casa.descargas / ("notas.txt" + ext)
        ruta.write_bytes(mod.compress(b"hola mundo\n" * 100))
        info = detectar.detectar(ruta)
        assert info.tipo == "comprimido", ext
        assert [e.nombre for e in info.entradas] == ["notas.txt"], ext


def test_gz_que_es_un_tar(casa):
    ruta = muestras.tar(casa.descargas / "paquete.gz", [("a/b.txt", b"b")], "w:gz")
    assert [e.nombre for e in detectar.detectar(ruta).entradas] == ["a/b.txt"]


def test_rar_dañado_se_dice_claro(casa):
    ruta = casa.descargas / "roto.rar"
    ruta.write_bytes(b"Rar!\x1a\x07\x01\x00" + b"\x00" * 40)
    info = detectar.detectar(ruta)
    assert info.tipo == "comprimido"
    assert info.error   # dañado o falta un programa, pero nunca se queda callado


def test_programa_tar_gz(casa):
    info = detectar.detectar(muestras.programa_tar(casa.descargas / "MiPrograma-1.0-linux-x86_64.tar.gz"))
    assert info.tipo == "programa"
    vista = motor.para_mostrar(info)
    assert vista["acciones"][0]["id"] == "instalar_programa"
    assert "~/.local/opt/mi-programa" in vista["descripcion"]


def test_appimage_por_extension_y_por_firma(casa):
    a = muestras.appimage_falso(casa.descargas / "Falso-1.0-x86_64.AppImage")
    assert detectar.detectar(a).tipo == "appimage"
    b = casa.descargas / "sin_extension"
    b.write_bytes(b"\x7fELF\x02\x01\x01\x00AI\x02" + b"\x00" * 100)
    assert detectar.detectar(b).tipo == "appimage"


def test_scripts_y_binarios(casa):
    sh = casa.descargas / "instalar.sh"
    sh.write_text("#!/bin/bash\n# instalador\nsudo cp x /usr/bin\n")
    info = detectar.detectar(sh)
    assert info.tipo == "script"
    assert info.extra["vista"].startswith("#!/bin/bash")
    assert info.extra["pide_admin"]
    run = casa.descargas / "juego.run"
    run.write_bytes(b"#!/bin/sh\necho hola\nexit 0\n\x00\x01\x02binario pegado")
    info = detectar.detectar(run)
    assert info.tipo == "script" and "binario" not in info.extra["vista"]
    binario = casa.descargas / "programa.bin"
    binario.write_bytes(b"\x7fELF\x02\x01\x01\x00" + b"\x00" * 64)
    info = detectar.detectar(binario)
    assert info.tipo == "script" and info.extra["binario"] and info.extra["vista"] == ""
    assert info.ejecuta_codigo and motor.para_mostrar(info)["aviso"]


def test_deb_y_rpm(casa):
    info = detectar.detectar(muestras.deb(casa.descargas / "google-chrome-stable_current_amd64.deb"))
    assert info.tipo == "deb"
    assert info.extra["nombre"] == "google-chrome-stable"
    rpm = casa.descargas / "hola-mundo-1.2-3.fc40.x86_64.rpm"
    rpm.write_bytes(b"\xed\xab\xee\xdb" + b"\x00" * 200)
    info = detectar.detectar(rpm)
    assert info.tipo == "rpm" and info.extra["nombre"] == "hola-mundo"


def test_flatpak(casa):
    ref = casa.descargas / "org.example.App.flatpakref"
    ref.write_text("[Flatpak Ref]\nName=org.example.App\nTitle=Ejemplo\nUrl=https://dl.flathub.org/repo/\n")
    info = detectar.detectar(ref)
    assert info.tipo == "flatpakref" and info.extra["app_id"] == "org.example.App"
    bundle = casa.descargas / "app.flatpak"
    bundle.write_bytes(b"xxx")
    assert detectar.detectar(bundle).tipo == "flatpak"


def test_windows(casa):
    exe = casa.descargas / "DiscordSetup-1.0.9-x64.exe"
    exe.write_bytes(b"MZ" + b"\x00" * 100)
    info = detectar.detectar(exe)
    assert info.tipo == "windows" and info.extra["nombre_programa"] == "Discord"
    msi = casa.descargas / "programa.msi"
    msi.write_bytes(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1" + b"\x00" * 100)
    assert detectar.detectar(msi).tipo == "windows"


def test_jar_mod_y_programa(casa):
    fabric = detectar.detectar(muestras.mod_fabric(casa.descargas / "sodium-fabric-0.6.0.jar"))
    assert fabric.tipo == "mod" and fabric.extra["mod"]["loader"] == "fabric"
    assert fabric.extra["mod"]["nombre"] == "Sodium" and fabric.extra["mod"]["minecraft"] == "1.21.1"
    neo = detectar.detectar(muestras.mod_neoforge(casa.descargas / "jei.jar"))
    assert neo.tipo == "mod" and neo.extra["mod"]["loader"] == "neoforge"
    assert neo.extra["mod"]["nombre"] == "Just Enough Items" and neo.extra["mod"]["version"] == ""
    assert detectar.detectar(muestras.jar_java(casa.descargas / "herramienta.jar")).tipo == "jar"


def test_desconocido_y_carpeta(casa):
    raro = casa.descargas / "datos.xyz"
    raro.write_bytes(bytes(range(256)) * 4)
    info = detectar.detectar(raro)
    assert info.tipo == "desconocido"
    vista = motor.para_mostrar(info)
    assert [a["id"] for a in vista["acciones"]] == ["abrir_carpeta"]
    assert detectar.detectar(casa.descargas).tipo == "carpeta"
    assert detectar.detectar(casa.home / "no-existe.zip").tipo == "falta"


def test_origen_del_navegador(casa):
    ruta = muestras.zip_(casa.home / "bajado.zip", {"a": b"a"})
    try:
        import os
        os.setxattr(ruta, "user.xdg.origin.url", b"https://www.github.com/x/y/releases/download/v1/bajado.zip")
    except OSError:
        pytest.skip("este sistema de archivos no guarda atributos")
    info = detectar.detectar(ruta)
    assert info.de_internet and detectar.sitio(info.origen) == "github.com"
    assert motor.para_mostrar(info)["origen"] == "Bajado de github.com"
