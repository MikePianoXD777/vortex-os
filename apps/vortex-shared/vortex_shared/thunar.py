"""«Enviar con Vortexine Shared» en el clic derecho de Thunar, sin tocar tus otras acciones."""
from __future__ import annotations

import os
from pathlib import Path

ID_ACCION = "vortex-shared-enviar"
ACCION = f"""<action>
\t<icon>vortex-shared</icon>
\t<name>Enviar con Vortexine Shared</name>
\t<submenu></submenu>
\t<unique-id>{ID_ACCION}</unique-id>
\t<command>vortex-shared ventana %F</command>
\t<description>Pásalo a otra compu con Vortexine's OS, cerca o lejos</description>
\t<range>*</range>
\t<patterns>*</patterns>
\t<directories/>
\t<audio-files/>
\t<image-files/>
\t<other-files/>
\t<text-files/>
\t<video-files/>
</action>
"""


def poner_accion() -> None:
    uca = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config") / "Thunar" / "uca.xml"
    try:
        xml = uca.read_text()
    except OSError:
        xml = '<?xml version="1.0" encoding="UTF-8"?>\n<actions>\n</actions>\n'
    if f"<unique-id>{ID_ACCION}</unique-id>" in xml or "</actions>" not in xml:
        return   # ya está, o no es un uca.xml que entendamos: mejor no tocarlo
    i = xml.rindex("</actions>")
    uca.parent.mkdir(parents=True, exist_ok=True)
    uca.write_text(xml[:i] + ACCION + xml[i:])
