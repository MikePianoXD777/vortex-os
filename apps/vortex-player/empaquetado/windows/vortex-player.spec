# -*- mode: python ; coding: utf-8 -*-
"""Vortex Player para Windows con PyInstaller (2.5).

Modo onedir: arranca rápido porque no descomprime nada al abrir. El resultado queda en
dist\\vortex-player\\; el workflow le agrega la carpeta bin\\ con mpv, ffmpeg, ffprobe, yt-dlp y deno.

    pyinstaller empaquetado\\windows\\vortex-player.spec --noconfirm --clean   (desde apps\\vortex-player)
"""
import os

RAIZ = os.path.abspath(os.path.join(SPECPATH, "..", ".."))

a = Analysis(
    [os.path.join(SPECPATH, "arrancar.py")],
    pathex=[RAIZ],
    # La interfaz (QML, íconos) va adentro, junto al código que la carga
    datas=[(os.path.join(RAIZ, "vortex_player", "qml"), os.path.join("vortex_player", "qml"))],
    hiddenimports=["PySide6.QtQuick", "PySide6.QtQuickControls2", "PySide6.QtNetwork", "PySide6.QtSvg"],
    # Lo que no usa: fuera del paquete (MPRIS es de Linux)
    excludes=["dbus", "gi", "tkinter", "PySide6.QtWebEngineCore", "PySide6.QtWebEngineQuick",
              "PySide6.Qt3DCore", "PySide6.Qt3DRender", "PySide6.QtCharts", "PySide6.QtDataVisualization",
              "PySide6.QtQuick3D", "PySide6.QtBluetooth", "PySide6.QtNfc", "PySide6.QtDesigner",
              "PySide6.QtMultimedia", "PySide6.QtPdf", "PySide6.QtLocation", "PySide6.QtPositioning"],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz, a.scripts, [],
    exclude_binaries=True,
    name="vortex-player",
    console=False,          # app de ventana: sin terminal detrás
    strip=False,
    upx=False,
    icon=os.path.join(SPECPATH, "vortex-player.ico"),
)

coll = COLLECT(exe, a.binaries, a.datas, strip=False, upx=False, name="vortex-player")
