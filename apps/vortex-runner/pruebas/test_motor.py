"""Pruebas del motor de Vortexine's Runner, sin Wine ni Proton de verdad: un umu-run falso, carpetas de
Proton falsas y una casa temporal (VORTEX_RUNNER_HOME). Para el lector de .exe se usan los .exe que trae
el Proton de CachyOS si está instalado (si no, esas pruebas se saltan).

    python3 -m unittest discover -s pruebas      (desde apps/vortex-runner)
"""
from __future__ import annotations

import json
import os
import shutil
import stat
import sys
import tempfile
import time
import unittest
from pathlib import Path

AQUI = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(AQUI))

WINE_EXES = next((p for p in (
    Path("/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/x86_64-windows"),
    Path("/usr/share/steam/compatibilitytools.d/proton-cachyos-native/files/lib/wine/x86_64-windows"),
) if (p / "winecfg.exe").is_file()), None)

UMU_FALSO = """#!/bin/sh
# Anota con qué lo llamaron y se queda «corriendo» un rato (con el WINEPREFIX en su entorno)
printf '%s\\n' "$@" > "$VORTEX_RUNNER_HOME/umu-args"
env | grep -E '^(WINEPREFIX|PROTONPATH|GAMEID|STORE|VORTEX_RUNNER_ID)=' | sort > "$VORTEX_RUNNER_HOME/umu-env"
pwd > "$VORTEX_RUNNER_HOME/umu-pwd"
if [ -n "$UMU_INSTALA" ]; then mkdir -p "$WINEPREFIX/drive_c/Program Files/Juego" "$WINEPREFIX/drive_c/windows";
  cp "$UMU_INSTALA" "$WINEPREFIX/drive_c/Program Files/Juego/Juego.exe";
  cp "$UMU_INSTALA" "$WINEPREFIX/drive_c/Program Files/Juego/unins000.exe";
  cp "$UMU_INSTALA" "$WINEPREFIX/drive_c/windows/notepad.exe"; fi
exec sleep "${UMU_DURA:-0}"
"""


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="vortex-runner-pruebas-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        os.environ["VORTEX_RUNNER_HOME"] = str(self.tmp)
        umu = self.tmp / "umu-run"
        umu.write_text(UMU_FALSO)
        umu.chmod(umu.stat().st_mode | stat.S_IEXEC)
        os.environ["VORTEX_RUNNER_UMU"] = str(umu)
        self.protones = self.tmp / "protones"
        for nombre in ("GE-Proton9-20", "proton-cachyos-slr", "GE-Proton10-3"):
            (self.protones / nombre).mkdir(parents=True)
            (self.protones / nombre / "proton").write_text("#!/bin/sh\n")
            (self.protones / nombre / "version").write_text(f"123 {nombre}\n")
        (self.protones / "no-es-proton").mkdir()
        os.environ["VORTEX_RUNNER_PROTONES"] = str(self.protones)
        from vortex_runner import rutas
        rutas.recargar()
        self.juego = self.tmp / "Mis juegos" / "super_juego-x64.exe"
        self.juego.parent.mkdir(parents=True)
        self.juego.write_bytes(b"MZ" + b"\0" * 200)

    def tearDown(self):
        for v in ("VORTEX_RUNNER_HOME", "VORTEX_RUNNER_UMU", "VORTEX_RUNNER_PROTONES", "UMU_DURA", "UMU_INSTALA"):
            os.environ.pop(v, None)


class TestPE(unittest.TestCase):
    def test_no_pe(self):
        from vortex_runner import pe
        with tempfile.NamedTemporaryFile(suffix=".exe") as f:
            f.write(b"hola"); f.flush()
            self.assertIsNone(pe.guardar_icono(f.name, f.name + "-icono"))
            self.assertEqual(pe.nombre(f.name), pe._bonito(Path(f.name).stem))

    def test_nombre_bonito(self):
        from vortex_runner import pe
        self.assertEqual(pe._bonito("super_juego-x64"), "Super Juego")
        self.assertEqual(pe._bonito("HollowKnight"), "Hollow Knight")
        self.assertEqual(pe._bonito("Game.Shipping"), "Game")

    @unittest.skipUnless(WINE_EXES, "sin el Proton de CachyOS no hay .exe de verdad")
    def test_exe_real(self):
        from vortex_runner import pe
        self.assertEqual(pe.nombre(WINE_EXES / "regedit.exe"), "Wine")      # su ProductName
        with tempfile.TemporaryDirectory() as d:
            ic = pe.guardar_icono(WINE_EXES / "winecfg.exe", Path(d) / "icono")
            self.assertIsNotNone(ic)
            self.assertTrue(ic.read_bytes()[:4] in (b"\x89PNG", b"\0\0\1\0"))


class TestProtones(Base):
    def test_orden(self):
        from vortex_runner import protones
        l = protones.lista()
        self.assertEqual([p["id"] for p in l], ["proton-cachyos-slr", "GE-Proton10-3", "GE-Proton9-20", "GE-Proton"])
        self.assertEqual(l[0]["nombre"], "Proton CachyOS")
        self.assertEqual(l[0]["version"], "proton-cachyos-slr")
        self.assertEqual(protones.por_defecto()["id"], "proton-cachyos-slr")
        self.assertEqual(protones.buscar("ya-no-existe")["id"], "proton-cachyos-slr")
        self.assertEqual(protones.buscar("GE-Proton")["ruta"], "GE-Proton")

    def test_sin_cachyos(self):
        from vortex_runner import correr, protones
        shutil.rmtree(self.protones / "proton-cachyos-slr")
        self.assertEqual(protones.por_defecto()["id"], "GE-Proton10-3")
        self.assertEqual(correr.falta_motor(), ["proton-cachyos-slr"])
        os.environ["VORTEX_RUNNER_UMU"] = str(self.tmp / "no-existe")
        self.assertEqual(correr.falta_motor(), ["umu-launcher", "proton-cachyos-slr"])


class TestBiblioteca(Base):
    def test_agregar_y_cambiar(self):
        from vortex_runner import biblioteca, rutas
        e = biblioteca.agregar(self.juego)
        self.assertEqual(e["nombre"], "Super Juego")
        self.assertTrue(Path(e["prefijo"]).is_dir())
        self.assertIn(rutas.prefijos(), Path(e["prefijo"]).parents)
        self.assertEqual(biblioteca.agregar(self.juego)["id"], e["id"])      # no se repite
        self.assertEqual(len(biblioteca.cargar()), 1)
        biblioteca.cambiar(e["id"], nombre="  Mi Juego ", argumentos="-dx11 -windowed", hackeo="no")
        x = biblioteca.por_id(e["id"])
        self.assertEqual((x["nombre"], x["argumentos"]), ("Mi Juego", "-dx11 -windowed"))
        self.assertNotIn("hackeo", x)
        biblioteca.cambiar(e["id"], nombre="   ")
        self.assertEqual(biblioteca.por_id(e["id"])["nombre"], "Mi Juego")

    def test_errores(self):
        from vortex_runner import biblioteca
        with self.assertRaises(biblioteca.ErrorBiblioteca):
            biblioteca.agregar(self.tmp / "nada.exe")
        txt = self.tmp / "notas.txt"
        txt.write_text("x")
        with self.assertRaises(biblioteca.ErrorBiblioteca):
            biblioteca.agregar(txt)
        rota = Path(os.environ["VORTEX_RUNNER_HOME"]) / "datos" / "biblioteca.json"
        rota.parent.mkdir(parents=True, exist_ok=True)
        rota.write_text("{roto")
        self.assertEqual(biblioteca.cargar(), [])

    def test_lanzador_y_quitar(self):
        from vortex_runner import biblioteca, rutas
        e = biblioteca.agregar(self.juego)
        d = biblioteca.crear_lanzador(e)
        texto = d.read_text()
        self.assertRegex(texto, rf'Exec=("[^"]*/vortex-runner"|vortex-runner) --correr {e["id"]}\n')
        self.assertIn("Name=Super Juego", texto)
        self.assertTrue(biblioteca.por_id(e["id"])["lanzador"])
        biblioteca.cambiar(e["id"], nombre="Otro")
        self.assertIn("Name=Otro", d.read_text())                         # el lanzador se pone al día
        (Path(e["prefijo"]) / "drive_c").mkdir()
        biblioteca.quitar(e["id"], borrar_prefijo=False)
        self.assertFalse(d.exists())
        self.assertTrue(Path(e["prefijo"]).is_dir())                      # tus partidas se quedan
        e2 = biblioteca.agregar(self.juego)
        biblioteca.quitar(e2["id"], borrar_prefijo=True)
        self.assertFalse(Path(e2["prefijo"]).exists())
        self.assertTrue(self.juego.exists())                              # el .exe tuyo nunca se borra

    def test_no_borra_prefijo_ajeno(self):
        from vortex_runner import biblioteca
        ajeno = self.tmp / "mi-prefijo-de-lutris"
        ajeno.mkdir()
        e = biblioteca.agregar(self.juego, prefijo=ajeno)
        biblioteca.quitar(e["id"], borrar_prefijo=True)
        self.assertTrue(ajeno.is_dir())

    def test_parece_instalador(self):
        from vortex_runner import biblioteca
        for n in ("setup.exe", "Steam_Install.exe", "programa-instalador.exe", "algo.msi"):
            self.assertTrue(biblioteca.parece_instalador(n), n)
        self.assertFalse(biblioteca.parece_instalador("Juego.exe"))


class TestCorrer(Base):
    def test_lanzar_con_umu(self):
        from vortex_runner import biblioteca, correr
        e = biblioteca.agregar(self.juego)
        biblioteca.cambiar(e["id"], argumentos="-windowed 'con espacio'")
        e = biblioteca.por_id(e["id"])
        correr.lanzar(e).wait(10)
        args = (self.tmp / "umu-args").read_text().splitlines()
        self.assertEqual(args, [str(self.juego.resolve()), "-windowed", "con espacio"])
        env = dict(l.split("=", 1) for l in (self.tmp / "umu-env").read_text().splitlines())
        self.assertEqual(env["WINEPREFIX"], e["prefijo"])
        self.assertEqual(env["PROTONPATH"], str((self.protones / "proton-cachyos-slr").resolve()))
        self.assertEqual((env["GAMEID"], env["STORE"], env["VORTEX_RUNNER_ID"]), ("umu-default", "none", e["id"]))
        self.assertEqual((self.tmp / "umu-pwd").read_text().strip(), str(self.juego.parent.resolve()))
        biblioteca.cambiar(e["id"], proton="GE-Proton")
        correr.lanzar(biblioteca.por_id(e["id"])).wait(10)
        self.assertIn("PROTONPATH=GE-Proton", (self.tmp / "umu-env").read_text())

    def test_msi(self):
        from vortex_runner import biblioteca, correr
        msi = self.tmp / "algo.msi"
        msi.write_bytes(b"x")
        e = biblioteca.agregar(msi)
        self.assertEqual(correr.orden(e)[1:], ["msiexec", "/i", str(msi.resolve())])

    def test_corriendo_y_detener(self):
        from vortex_runner import biblioteca, correr
        e = biblioteca.agregar(self.juego)
        os.environ["UMU_DURA"] = "30"
        p = correr.lanzar(e)
        limite = time.time() + 5
        while not correr.corriendo(e) and time.time() < limite:
            time.sleep(0.1)
        self.assertTrue(correr.corriendo(e))
        correr.detener(e, espera=3)
        p.wait(5)
        self.assertFalse(correr.corriendo(e))

    def test_correr_y_esperar_suma_tiempo(self):
        from vortex_runner import biblioteca, correr
        e = biblioteca.agregar(self.juego)
        self.assertEqual(correr.correr_y_esperar(e), 0)
        x = biblioteca.por_id(e["id"])
        self.assertGreater(x["ultima_vez"], 0)
        self.assertEqual(x["minutos"], 0)

    def test_sin_umu(self):
        from vortex_runner import biblioteca, correr
        e = biblioteca.agregar(self.juego)
        os.environ["VORTEX_RUNNER_UMU"] = str(self.tmp / "no-existe")
        with self.assertRaises(FileNotFoundError):
            correr.lanzar(e)

    def test_instalar_motor_con_pkexec_falso(self):
        from vortex_runner import correr
        shutil.rmtree(self.protones / "proton-cachyos-slr")
        pk = self.tmp / "pkexec"
        pk.write_text(f'#!/bin/sh\necho "$@" > "{self.tmp}/pk"\n')
        pk.chmod(0o755)
        os.environ["VORTEX_RUNNER_PKEXEC"] = str(pk)
        try:
            self.assertEqual(correr.instalar_motor().returncode, 0)
        finally:
            os.environ.pop("VORTEX_RUNNER_PKEXEC")
        self.assertEqual((self.tmp / "pk").read_text().strip(), "pacman -S --needed --noconfirm proton-cachyos-slr")


@unittest.skipUnless(WINE_EXES, "sin el Proton de CachyOS no hay .exe de verdad")
class TestInstalador(Base):
    def test_exes_nuevos(self):
        from vortex_runner import biblioteca, correr
        setup = self.tmp / "setup.exe"
        shutil.copy(WINE_EXES / "winecfg.exe", setup)
        prefijo = biblioteca.nuevo_prefijo("Juego")
        antes = biblioteca.exes_en(prefijo)
        e = {"id": "instalando", "prefijo": str(prefijo), "exe": str(setup), "argumentos": ""}
        os.environ["UMU_INSTALA"] = str(WINE_EXES / "winecfg.exe")
        correr.lanzar(e).wait(10)
        nuevos = biblioteca.exes_nuevos(prefijo, antes)
        self.assertEqual([Path(n["exe"]).name for n in nuevos], ["Juego.exe"])   # sin el desinstalador ni windows/
        x = biblioteca.agregar(nuevos[0]["exe"], prefijo=prefijo)
        self.assertEqual(x["prefijo"], str(prefijo))
        self.assertTrue(x["icono"])


if __name__ == "__main__":
    unittest.main()
