# -*- mode: python ; coding: utf-8 -*-
"""Vortexine Connect para Windows con PyInstaller.

Modo onedir: arranca rápido porque no descomprime nada al abrir. El resultado queda en
dist\\vortex-connect\\.

    pyinstaller empaquetado\\windows\\vortex-connect.spec --noconfirm --clean   (desde apps\\vortex-connect)
"""
import os

RAIZ = os.path.abspath(os.path.join(SPECPATH, "..", ".."))

a = Analysis(
    [os.path.join(SPECPATH, "arrancar.py")],
    pathex=[RAIZ],
    # La interfaz (QML, íconos) va adentro, junto al código que la carga
    datas=[(os.path.join(RAIZ, "vortex_connect", "qml"), os.path.join("vortex_connect", "qml"))],
    hiddenimports=["PySide6.QtQuick", "PySide6.QtQuickControls2", "PySide6.QtNetwork", "PySide6.QtSvg"],
    # Lo que no usa: fuera del paquete 
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
    name="vortex-connect",
    console=False,          # app de ventana: sin terminal detrás
    strip=False,
    upx=False,
    icon=os.path.join(SPECPATH, "vortex-connect.ico"),
)

coll = COLLECT(exe, a.binaries, a.datas, strip=False, upx=False, name="vortex-connect")
