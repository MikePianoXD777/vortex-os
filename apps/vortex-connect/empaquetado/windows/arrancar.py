"""El .exe de Vortexine Connect para Windows arranca aquí (PyInstaller necesita un archivo, no `-m`)."""
import sys

from vortex_connect.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
