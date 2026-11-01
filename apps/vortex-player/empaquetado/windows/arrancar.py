"""El .exe de Vortex Player para Windows arranca aquí (PyInstaller necesita un archivo, no `-m`)."""
import sys

from vortex_player.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
