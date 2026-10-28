"""Descomprimir sin peligro: rutas con «../», enlaces que se salen, no anidar, no pisar, contraseñas."""
from __future__ import annotations

import os
import subprocess
import tarfile

import pytest

import muestras
from vortex_app_installer import comprimidos, detectar, motor, registro


def _todo(carpeta):
    return sorted(str(p.relative_to(carpeta)) for p in carpeta.rglob("*"))


def test_rutas_peligrosas_no_salen(casa):
    trampa = casa.tmp / "trampa"
    trampa.mkdir()
    origen = muestras.peligroso_tar(trampa / "mal.tar.gz")
    antes = set(os.listdir(casa.tmp)), set(os.listdir(trampa))
    info = detectar.detectar(origen)
    assert motor.para_mostrar(info)["nota"]   # se avisa antes de descomprimir
    r = motor.ejecutar(info, "descomprimir", {})
    destino = trampa / "bien"
    assert r["carpeta"] == str(destino) and r["saltados"] >= 3
    assert not (casa.tmp / "fuera.txt").exists() and not (trampa / "fuera.txt").exists()
    assert (set(os.listdir(casa.tmp)), set(os.listdir(trampa)) - {"bien"}) == antes
    # nada dentro apunta fuera de la carpeta
    for p in destino.rglob("*"):
        if p.is_symlink():
            assert os.path.realpath(p).startswith(str(destino) + os.sep), p
    assert (destino / "a.txt").read_text() == "a"
    assert (destino / "ok").is_symlink()          # el enlace bueno (a.txt, dentro) se queda
    assert not (destino / "sube").exists() and not (destino / "absoluto").is_symlink()
    assert not list(trampa.glob(".vortex-descomprimiendo-*"))   # sin restos de la carpeta temporal


def test_zip_con_rutas_peligrosas(casa):
    ruta = muestras.zip_(casa.descargas / "malo.zip", {"../../escapado.txt": b"x", "/absoluto.txt": b"y",
                                                         "normal/ok.txt": b"z"})
    info = detectar.detectar(ruta)
    r = motor.ejecutar(info, "descomprimir", {})
    assert not (casa.home / "escapado.txt").exists() and not (casa.tmp / "escapado.txt").exists()
    assert (casa.descargas / "malo" / "normal" / "ok.txt").read_text() == "z"
    assert r["saltados"] == 1


def test_una_sola_carpeta_no_se_anida(casa):
    ruta = muestras.tar(casa.descargas / "Proyecto-2.0.tar.gz", [("Proyecto-2.0/", b"", tarfile.DIRTYPE, "", 0o755),
                                                                ("Proyecto-2.0/a.txt", b"a")])
    motor.ejecutar(detectar.detectar(ruta), "descomprimir", {})
    assert (casa.descargas / "Proyecto-2.0" / "a.txt").exists()
    assert not (casa.descargas / "Proyecto-2.0" / "Proyecto-2.0").exists()


def test_varias_cosas_van_a_una_carpeta_con_su_nombre(casa):
    ruta = muestras.zip_(casa.descargas / "fotos.zip", {"a.jpg": b"a", "b.jpg": b"b"})
    motor.ejecutar(detectar.detectar(ruta), "descomprimir", {})
    assert _todo(casa.descargas / "fotos") == ["a.jpg", "b.jpg"]


def test_no_pisa_lo_que_existe(casa):
    ruta = muestras.zip_(casa.descargas / "fotos.zip", {"a.jpg": b"nuevo"})
    (casa.descargas / "fotos").mkdir()
    (casa.descargas / "fotos" / "a.jpg").write_bytes(b"mio")
    info = detectar.detectar(ruta)
    with pytest.raises(comprimidos.Conflicto) as e:
        motor.ejecutar(info, "descomprimir", {})
    assert e.value.sugerida.name == "fotos (2)"
    assert (casa.descargas / "fotos" / "a.jpg").read_bytes() == b"mio"
    motor.ejecutar(info, "descomprimir", {"nombre": e.value.sugerida.name})
    assert (casa.descargas / "fotos (2)" / "a.jpg").read_bytes() == b"nuevo"


def test_en_otra_carpeta(casa):
    ruta = muestras.zip_(casa.descargas / "x.zip", {"x/1.txt": b"1"})
    otra = casa.home / "Documentos"
    r = motor.ejecutar(detectar.detectar(ruta), "descomprimir", {"carpeta": str(otra)})
    assert (otra / "x" / "1.txt").exists() and r["carpeta"] == str(otra / "x")


def test_contrasena(casa):
    fuente = casa.tmp / "secreto"
    fuente.mkdir()
    (fuente / "s.txt").write_text("secreto")
    ruta = casa.descargas / "c.zip"
    subprocess.run(["bsdtar", "--format", "zip", "--options", "zip:encryption=aes256", "--passphrase", "clave",
                    "-cf", str(ruta), "-C", str(fuente), "s.txt"], check=True)
    info = detectar.detectar(ruta)
    with pytest.raises(comprimidos.NecesitaContrasena) as e:
        motor.ejecutar(info, "descomprimir", {})
    assert not e.value.incorrecta
    with pytest.raises(comprimidos.NecesitaContrasena) as e:
        motor.ejecutar(info, "descomprimir", {"contrasena": "mala"})
    assert e.value.incorrecta
    assert not (casa.descargas / "c").exists()   # lo de a medias no se queda
    motor.ejecutar(info, "descomprimir", {"contrasena": "clave"})
    assert (casa.descargas / "c" / "s.txt").read_text() == "secreto"


def test_suelto_y_progreso(casa):
    import gzip
    ruta = casa.descargas / "grande.txt.gz"
    ruta.write_bytes(gzip.compress(os.urandom(3 << 20)))
    avances = []
    motor.ejecutar(detectar.detectar(ruta), "descomprimir", {}, avances.append)
    assert (casa.descargas / "grande.txt").stat().st_size == 3 << 20
    assert avances and avances[-1] == pytest.approx(1.0, abs=0.01) and avances == sorted(avances)


def test_cancelar_no_deja_restos(casa):
    import threading
    ruta = muestras.tar(casa.descargas / "pesado.tar", [("p/%d.bin" % i, os.urandom(1 << 20)) for i in range(20)], "w")
    cancelar = threading.Event()
    cancelar.set()
    with pytest.raises(comprimidos.Cancelado):
        motor.ejecutar(detectar.detectar(ruta), "descomprimir", {}, None, cancelar)
    assert not (casa.descargas / "p").exists() and not list(casa.descargas.glob(".vortex-*"))


def test_iso_de_solo_lectura_se_puede_quitar(casa):
    fuente = casa.tmp / "f"
    (fuente / "dir").mkdir(parents=True)
    (fuente / "dir" / "a.txt").write_text("a")
    ruta = casa.descargas / "disco.iso"
    subprocess.run(["bsdtar", "--format", "iso9660", "-cf", str(ruta), "-C", str(fuente), "dir"], check=True)
    r = motor.ejecutar(detectar.detectar(ruta), "descomprimir", {})
    destino = r["carpeta"]
    e = registro.cargar()[0]
    assert e["tipo"] == "descomprimido" and e["rutas"] == [destino]
    motor.deshacer(e["id"])
    assert not os.path.exists(destino)
    assert list((casa.home / ".local/share/Trash/files").iterdir())   # a la papelera, no borrado


def test_falta_7z_lo_dice_con_el_comando(casa, monkeypatch):
    monkeypatch.setattr(comprimidos.shutil, "which", lambda n: None if n in ("7z", "7zz", "7za", "unrar") else "/usr/bin/" + n)
    ruta = casa.descargas / "raro.7z"
    ruta.write_bytes(b"7z\xbc\xaf\x27\x1c" + b"\x00" * 30)
    info = detectar.detectar(ruta)
    assert info.falta_comando == "sudo pacman -S 7zip"
    assert motor.para_mostrar(info)["comando"] == "sudo pacman -S 7zip"
