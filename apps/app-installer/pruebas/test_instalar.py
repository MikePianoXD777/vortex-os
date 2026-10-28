"""AppImage falso, programa .tar.gz, desinstalar (a la papelera) y lo que corre código."""
from __future__ import annotations

import os

import pytest

import muestras
from conftest import escribir_script
from vortex_app_installer import comprimidos, detectar, instalar, motor, registro


def _desktop(casa, nombre):
    return instalar.leer_desktop((casa.home / ".local/share/applications" / nombre).read_text())


def test_appimage_va_a_aplicaciones_si_ya_la_usas(casa):
    """Vortexine's en español: si ya existe ~/Aplicaciones, ahí van (y no se crea ~/Applications)."""
    (casa.home / "Aplicaciones").mkdir()
    origen = muestras.appimage_falso(casa.descargas / "Falso-1.0-x86_64.AppImage")
    motor.ejecutar(detectar.detectar(origen), "instalar_appimage", {})
    assert (casa.home / "Aplicaciones" / "Falso-1.0-x86_64.AppImage").exists()
    assert not (casa.home / "Applications").exists()


def test_appimage_instalar_y_desinstalar(casa):
    origen = muestras.appimage_falso(casa.descargas / "Falso-1.0-x86_64.AppImage")
    info = detectar.detectar(origen)
    vista = motor.para_mostrar(info)
    assert vista["aviso"] and vista["acciones"][0]["confirmar"]   # viene de internet: se confirma
    r = motor.ejecutar(info, "instalar_appimage", {})
    copia = casa.home / "Applications" / "Falso-1.0-x86_64.AppImage"
    assert copia.exists() and os.access(copia, os.X_OK) and origen.exists()
    g = _desktop(casa, "falso.desktop")
    assert instalar.valor(g, "Name") == "Falso"
    assert instalar.valor(g, "Exec").startswith(str(copia)) and instalar.valor(g, "Exec").endswith("%U")
    assert instalar.valor(g, "Icon") == "falso"
    assert (casa.home / ".local/share/icons/hicolor/64x64/apps/falso.png").exists()
    script = casa.home / ".local/share/vortex-app-installer/desinstaladores/appimage-falso.sh"
    assert script.exists() and str(copia) in script.read_text()
    assert r["id"] == "appimage-falso"
    assert [e["nombre"] for e in motor.instalados()] == ["Falso"]

    # otra vez: pregunta antes de reemplazar, y al reemplazar la vieja va a la papelera
    with pytest.raises(comprimidos.Conflicto):
        motor.ejecutar(info, "instalar_appimage", {})
    nueva = muestras.appimage_falso(casa.descargas / "Falso-2.0-x86_64.AppImage")
    motor.ejecutar(detectar.detectar(nueva), "instalar_appimage", {"reemplazar": True})
    assert not copia.exists() and (casa.home / "Applications" / "Falso-2.0-x86_64.AppImage").exists()
    assert len(registro.cargar()) == 1

    motor.deshacer("appimage-falso")
    assert not list((casa.home / "Applications").iterdir())
    assert not (casa.home / ".local/share/applications/falso.desktop").exists()
    assert not (casa.home / ".local/share/icons/hicolor/64x64/apps/falso.png").exists()
    assert registro.cargar() == []
    assert (casa.home / ".local/share/Trash/files").exists()


def test_appimage_solo_abrir(casa):
    origen = muestras.appimage_falso(casa.descargas / "Falso.AppImage")
    motor.ejecutar(detectar.detectar(origen), "abrir_appimage", {})
    assert os.access(origen, os.X_OK)
    assert casa.lanzados == [[str(origen)]]
    assert registro.cargar() == []   # no se instaló nada


def test_appimage_no_pisa_uno_ajeno(casa):
    ajeno = casa.home / "Applications" / "Falso.AppImage"
    ajeno.parent.mkdir()
    ajeno.write_text("de otra persona")
    info = detectar.detectar(muestras.appimage_falso(casa.descargas / "Falso.AppImage"))
    with pytest.raises(comprimidos.Conflicto):
        motor.ejecutar(info, "instalar_appimage", {})
    motor.ejecutar(info, "instalar_appimage", {"reemplazar": True})
    assert ajeno.read_text() == "de otra persona"   # el suyo no se toca: el nuevo va al lado
    assert (casa.home / "Applications" / "Falso (2).AppImage").exists()


def test_programa_tar_gz(casa):
    origen = muestras.programa_tar(casa.descargas / "MiPrograma-1.0-linux-x86_64.tar.gz")
    info = detectar.detectar(origen)
    r = motor.ejecutar(info, "instalar_programa", {})
    opt = casa.home / ".local/opt/mi-programa"
    exe = opt / "mi-programa" / "mi-programa"
    assert exe.exists() and r["abrir"] == str(exe)
    g = _desktop(casa, "mi-programa.desktop")
    assert instalar.valor(g, "Exec") == str(exe) + " %F"            # el «@EXEC@» se reemplazó
    assert instalar.valor(g, "TryExec") == str(exe)
    assert instalar.valor(g, "Exec", "Desktop Action nueva") == str(exe) + " --nueva"
    assert instalar.valor(g, "Icon") == "mi-programa"
    assert (casa.home / ".local/share/icons/hicolor/128x128/apps/mi-programa.png").exists()
    desinstalar = opt / "desinstalar.sh"
    assert os.access(desinstalar, os.X_OK) and str(opt) in desinstalar.read_text()
    assert (opt / "desinstalar.sh.original").read_text().endswith("echo suyo\n")   # el suyo se guarda aparte
    assert [e["tipo"] for e in registro.cargar()] == ["programa"]

    motor.deshacer(r["id"])
    assert not opt.exists() and not (casa.home / ".local/share/applications/mi-programa.desktop").exists()
    assert registro.cargar() == []


def test_programa_el_desinstalar_sh_funciona(casa):
    import subprocess
    info = detectar.detectar(muestras.programa_tar(casa.descargas / "MiPrograma.tar.gz"))
    motor.ejecutar(info, "instalar_programa", {})
    opt = casa.home / ".local/opt/mi-programa"
    subprocess.run(["sh", str(opt / "desinstalar.sh")], check=True, capture_output=True,
                   env=dict(os.environ, HOME=str(casa.home)))
    assert not opt.exists()
    assert registro.cargar() == []   # la entrada se va sola: sus archivos ya no existen


def test_programa_ya_instalado_pregunta(casa):
    info = detectar.detectar(muestras.programa_tar(casa.descargas / "MiPrograma.tar.gz"))
    motor.ejecutar(info, "instalar_programa", {})
    with pytest.raises(comprimidos.Conflicto):
        motor.ejecutar(info, "instalar_programa", {})
    motor.ejecutar(info, "instalar_programa", {"reemplazar": True})
    assert len(registro.cargar()) == 1 and (casa.home / ".local/opt/mi-programa").exists()


def test_deshacer_nunca_toca_carpetas_protegidas(casa):
    registro.agregar({"id": "descomprimido-trampa", "tipo": "descomprimido", "nombre": "x",
                      "rutas": [str(casa.home), str(casa.descargas), "/"]})
    motor.deshacer("descomprimido-trampa")
    assert casa.home.exists() and casa.descargas.exists()


def test_script_se_confirma_y_va_a_terminal(casa):
    escribir_script(casa.bins / "kitty", "#!/bin/sh\nexit 0\n")
    sh = casa.descargas / "instalar.sh"
    sh.write_text("#!/bin/bash\necho hola\n")
    info = detectar.detectar(sh)
    accion = motor.para_mostrar(info)["acciones"][0]
    assert accion["id"] == "correr" and accion["confirmar"]["titulo"]
    motor.ejecutar(info, "correr", {})
    assert casa.lanzados[0][:2] == ["kitty", "--hold"] and casa.lanzados[0][-2:] == ["/bin/bash", str(sh)]
    assert "sudo" not in casa.lanzados[0]


def test_script_que_quiere_sudo_no_se_corre(casa):
    escribir_script(casa.bins / "kitty", "#!/bin/sh\nexit 0\n")
    sh = casa.descargas / "root.sh"
    sh.write_text("#!/usr/bin/sudo /bin/sh\necho x\n")
    with pytest.raises(motor.Problema):
        motor.ejecutar(detectar.detectar(sh), "correr", {})
    assert casa.lanzados == []


def test_windows_sin_wine(casa, monkeypatch):
    monkeypatch.setattr(instalar, "opciones_windows", lambda: [])
    exe = casa.descargas / "Setup.exe"
    exe.write_bytes(b"MZ" + b"\x00" * 10)
    vista = motor.para_mostrar(motor.analizar(exe))
    assert vista["comando"] == "sudo pacman -S wine"
    assert [a["id"] for a in vista["acciones"]] == ["abrir_carpeta"]


def test_windows_con_wine(casa, monkeypatch):
    monkeypatch.setattr(instalar, "opciones_windows", lambda: [{"id": "wine", "nombre": "Wine"}])
    exe = casa.descargas / "juego.exe"
    exe.write_bytes(b"MZ" + b"\x00" * 10)
    info = motor.analizar(exe)
    accion = motor.para_mostrar(info)["acciones"][0]
    assert accion["id"] == "windows" and accion["confirmar"]
    motor.ejecutar(info, "windows", accion["datos"])
    assert casa.lanzados == [["wine", str(exe)]]
