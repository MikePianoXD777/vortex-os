"""Archivos de muestra para las pruebas: comprimidos de todos los formatos, con rutas peligrosas, un
AppImage falso (un script que entiende --appimage-extract), un programa .tar.gz, .deb, mods…"""
from __future__ import annotations

import io
import json
import subprocess
import tarfile
import zipfile
from pathlib import Path

PNG_64 = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x40\x00\x00\x00\x40\x08\x06\x00\x00\x00"
PNG_128 = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x80\x00\x00\x00\x80\x08\x06\x00\x00\x00"


def _agregar(t: tarfile.TarFile, nombre: str, datos: bytes = b"hola", tipo=tarfile.REGTYPE, enlace: str = "",
             modo: int = 0o644) -> None:
    i = tarfile.TarInfo(nombre)
    i.type, i.linkname, i.mode = tipo, enlace, modo
    if tipo == tarfile.REGTYPE:
        i.size = len(datos)
        t.addfile(i, io.BytesIO(datos))
    else:
        t.addfile(i)


def tar(ruta: Path, entradas: list[tuple], modo: str = "w:gz") -> Path:
    """entradas: (nombre, datos) o (nombre, datos, tipo, enlace, modo)."""
    with tarfile.open(ruta, modo) as t:
        for e in entradas:
            _agregar(t, *e)
    return ruta


def zip_(ruta: Path, entradas: dict[str, bytes]) -> Path:
    with zipfile.ZipFile(ruta, "w") as z:
        for n, d in entradas.items():
            z.writestr(n, d)
    return ruta


def peligroso_tar(ruta: Path) -> Path:
    return tar(ruta, [("bien/a.txt", b"a"), ("../fuera.txt", b"x"), ("bien/sube", b"", tarfile.SYMTYPE, "../../.."),
                      ("bien/absoluto", b"", tarfile.SYMTYPE, "/etc/passwd"), ("bien/sube/x.txt", b"x"),
                      ("bien/duro", b"", tarfile.LNKTYPE, "../fuera.txt"), ("bien/ok", b"", tarfile.SYMTYPE, "a.txt")])


def programa_tar(ruta: Path) -> Path:
    """Como VortexStudio-*-linux-x86_64.tar.gz: carpeta, .desktop con «@EXEC@», ejecutable e ícono."""
    desktop = ("[Desktop Entry]\nType=Application\nName=Mi Programa\nGenericName=Editor\nExec=\"@EXEC@\" %F\n"
               "TryExec=@EXEC@\nIcon=mi-programa\nCategories=Utility;\nStartupWMClass=mi-programa\n"
               "Actions=nueva;\n\n[Desktop Action nueva]\nName=Ventana nueva\nExec=\"@EXEC@\" --nueva\n").encode()
    return tar(ruta, [("MiPrograma-1.0/", b"", tarfile.DIRTYPE, "", 0o755),
                      ("MiPrograma-1.0/mi-programa.desktop", desktop),
                      ("MiPrograma-1.0/instalar.sh", b"#!/bin/sh\necho instalar\n", tarfile.REGTYPE, "", 0o755),
                      ("MiPrograma-1.0/desinstalar.sh", b"#!/bin/sh\necho suyo\n", tarfile.REGTYPE, "", 0o755),
                      ("MiPrograma-1.0/mi-programa/mi-programa", b"#!/bin/sh\necho hola\n", tarfile.REGTYPE, "", 0o755),
                      ("MiPrograma-1.0/mi-programa/_internal/libx.so", b"\x7fELF", tarfile.REGTYPE, "", 0o755),
                      ("MiPrograma-1.0/mi-programa/iconos/mi-programa.png", PNG_128)])


def appimage_falso(ruta: Path, nombre: str = "Falso", icono: bytes = PNG_64) -> Path:
    """Un «AppImage» que es un script: con --appimage-extract PATRÓN deja squashfs-root/… como el de verdad."""
    octal = "".join("\\%03o" % b for b in icono)
    texto = f"""#!/bin/sh
if [ "$1" = "--appimage-extract" ]; then
  mkdir -p squashfs-root
  case "$2" in
    "*.desktop") printf '[Desktop Entry]\\nType=Application\\nName={nombre}\\nExec=AppRun %%U\\nIcon=falso\\nCategories=Utility;\\nX-AppImage-Version=1.0\\n' > squashfs-root/falso.desktop ;;
    "falso.png") printf '{octal}' > squashfs-root/falso.png ;;
    ".DirIcon") ln -sf falso.png squashfs-root/.DirIcon ;;
  esac
  exit 0
fi
echo "corriendo {nombre}"
"""
    ruta.write_text(texto, encoding="utf-8")
    ruta.chmod(0o644)   # como llega de internet: sin permiso de ejecución
    return ruta


def deb(ruta: Path, paquete: str = "google-chrome-stable") -> Path:
    carpeta = ruta.parent / (ruta.name + ".partes")
    carpeta.mkdir()
    control = f"Package: {paquete}\nVersion: 1.0\nArchitecture: amd64\nDescription: Un navegador\n más texto\n".encode()
    tar(carpeta / "control.tar.gz", [("./control", control)])
    tar(carpeta / "data.tar.gz", [("./usr/bin/programa", b"#!/bin/sh\n", tarfile.REGTYPE, "", 0o755),
                                  ("./usr/share/doc/leeme.txt", b"hola")])
    (carpeta / "debian-binary").write_text("2.0\n")
    subprocess.run(["bsdtar", "--format", "ar", "-cf", str(ruta), "debian-binary", "control.tar.gz", "data.tar.gz"],
                   cwd=carpeta, check=True)
    return ruta


def mod_fabric(ruta: Path) -> Path:
    return zip_(ruta, {"fabric.mod.json": json.dumps({"schemaVersion": 1, "id": "sodium", "name": "Sodium",
                                                      "version": "0.6.0", "depends": {"minecraft": "1.21.1"}}).encode(),
                       "net/x/A.class": b"\xca\xfe\xba\xbe"})


def mod_neoforge(ruta: Path) -> Path:
    toml = b'modLoader="javafml"\nloaderVersion="[4,)"\n[[mods]]\nmodId="jei"\nversion="${file.jarVersion}"\ndisplayName="Just Enough Items"\n'
    return zip_(ruta, {"META-INF/neoforge.mods.toml": toml, "META-INF/MANIFEST.MF": b"Manifest-Version: 1.0\n"})


def jar_java(ruta: Path) -> Path:
    return zip_(ruta, {"META-INF/MANIFEST.MF": b"Manifest-Version: 1.0\nMain-Class: A\n", "A.class": b"\xca\xfe"})
