#!/usr/bin/env python3
"""Las claves de idioma del shell: cada I18n.t("clave") del QML existe en es.json y en en.json.

Si falta en los dos, el shell enseña la clave cruda (o el texto en inglés de respaldo); si solo falta en
es.json, sale en inglés con el sistema en español. Las claves que se arman en vivo («guide.tabs.» + id)
no se pueden revisar así y se saltan. También revisa que las variables ({cmd}, {n}…) de cada texto en
español sean las mismas que en inglés.

  python3 -I pruebas/idioma.py
"""

import json
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent / "shell" / "src"
LLAMADA = re.compile(r'I18n\.t\(\s*"([^"]+)"')
VARIABLE = re.compile(r"\{(\w+)\}")


def plano(d: dict, prefijo: str = "") -> dict:
    salida = {}
    for k, v in d.items():
        clave = f"{prefijo}.{k}" if prefijo else k
        if isinstance(v, dict):
            salida.update(plano(v, clave))
        else:
            salida[clave] = v
    return salida


def main() -> int:
    idiomas = {i: plano(json.loads((RAIZ / f"assets/languages/{i}.json").read_text(encoding="utf-8")))
               for i in ("es", "en")}
    usadas: dict[str, str] = {}
    for qml in sorted((RAIZ / "quickshell").rglob("*.qml")):
        for clave in LLAMADA.findall(qml.read_text(encoding="utf-8")):
            if not clave.endswith("."):
                usadas.setdefault(clave, str(qml.relative_to(RAIZ)))
    fallos = 0
    for idioma, textos in idiomas.items():
        faltan = sorted(k for k in usadas if k not in textos)
        print(f"  {'ok ' if not faltan else 'MAL'} {len(usadas)} claves usadas, todas en {idioma}.json"
              + (f": faltan {len(faltan)}" if faltan else ""))
        for k in faltan:
            print(f"        {k}   ({usadas[k]})")
        fallos += bool(faltan)
    distintas = sorted(k for k, v in idiomas["es"].items() if isinstance(v, str) and isinstance(idiomas["en"].get(k), str)
                       and set(VARIABLE.findall(v)) != set(VARIABLE.findall(idiomas["en"][k])))
    print(f"  {'ok ' if not distintas else 'MAL'} las variables {{…}} de es.json son las de en.json"
          + (f": {len(distintas)} distintas" if distintas else ""))
    for k in distintas:
        print(f"        {k}: es={idiomas['es'][k]!r} en={idiomas['en'][k]!r}")
    fallos += bool(distintas)
    print("\nTodo bien" if not fallos else "\nHay fallos")
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
