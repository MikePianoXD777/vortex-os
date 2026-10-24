#!/usr/bin/env python3
"""Prueba del trabajador de mejores momentos (momentos.py), con un video hecho por ffmpeg y un Claude falso.

El video dura 2 min y tiene 3 «jugadas» (sonido fuerte y pantalla roja) a los 30, 65 y 100 s. Se revisa
que los candidatos caigan ahí, que Claude reciba las miniaturas SIN herramientas, que el .vortex tenga los
cortes con su audio enlazado y sus marcadores, y que sin Claude (o con una respuesta rota) se quede con los
picos. No gasta la suscripción ni toca nada fuera de una carpeta temporal.

  python3 -I apps/miku/pruebas/momentos.py
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
FALLOS = 0
JUGADAS = (30, 65, 100)


def probar(nombre: str, ok: bool, extra: str = "") -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}" + (f"   [{extra[:300]}]" if not ok and extra else ""))
    FALLOS += 0 if ok else 1


def hacer_video(destino: Path) -> None:
    rojo = "+".join(f"between(t,{t - 2},{t + 2})" for t in JUGADAS)
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y",
                    "-f", "lavfi", "-i", "testsrc2=s=320x180:r=24:d=120",
                    "-f", "lavfi", "-i", "anoisesrc=a=0.02:d=120", "-f", "lavfi", "-i", "sine=f=440:d=120",
                    "-filter_complex", f"[0:v]drawbox=c=red:t=fill:enable='{rojo}'[v];"
                                       f"[2:a]volume=volume='if({rojo},0.9,0)':eval=frame[s];"
                                       "[1:a][s]amix=inputs=2:normalize=0[a]",
                    "-map", "[v]", "-map", "[a]", "-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac",
                    "-shortest", str(destino)], check=True)


# Claude falso: guarda lo que le pasaron y contesta lo que diga $RESPUESTA (en el formato de stream-json).
CLAUDE = """#!/usr/bin/env python3
import json, os, sys
entrada = sys.stdin.read()
json.dump({"argv": sys.argv[1:], "entrada": json.loads(entrada)}, open(os.environ["LLAMADA"], "w"))
print(json.dumps({"type": "result", "subtype": "success", "result": os.environ.get("RESPUESTA", ""),
                  "session_id": "00000000-0000-0000-0000-000000000000"}))
"""


def correr(env: dict, video: Path) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", str(RAIZ / "miku/momentos.py"), str(video)], env=env,
                          capture_output=True, text=True, timeout=300)


def main() -> int:
    for programa in ("ffmpeg", "ffprobe"):
        if not shutil.which(programa):
            print(f"Falta {programa}: no se puede probar")
            return 1
    tmp = Path(tempfile.mkdtemp(prefix="vortex-miku-momentos."))
    videos, home, bin_ = tmp / "videos", tmp / "home", tmp / "bin"
    for d in (videos, home / ".local/bin", bin_):
        d.mkdir(parents=True)
    video = videos / "Partida prueba.mp4"
    hacer_video(video)
    claude = home / ".local/bin/claude"
    claude.write_text(CLAUDE)
    claude.chmod(0o755)
    vortex = bin_ / "vortex"                       # «vortex claude activado» → sí
    vortex.write_text("#!/bin/sh\nexit 0\n")
    vortex.chmod(0o755)
    llamada = tmp / "llamada.json"
    env = {"PATH": f"{bin_}:/usr/bin:/bin", "HOME": str(home), "VORTEX_BIN": str(vortex),
           "VORTEX_MIKU_VIDEOS": str(videos), "VORTEX_MIKU_TAREAS": str(tmp / "tareas"),
           "VORTEX_MIKU_SIN_AVISOS": "1", "LLAMADA": str(llamada)}
    proyecto = videos / "Partida prueba - mejores momentos.vortex"

    print("== Con Claude")
    respuesta = {"momentos": [{"candidato": 3, "titulo": "La tercera", "por_que": "Pantalla roja"},
                              {"candidato": 1, "titulo": "La primera", "por_que": "Sonido fuerte"},
                              {"candidato": 99, "titulo": "No existe"}]}
    r = correr({**env, "RESPUESTA": "Aquí van:\n" + json.dumps(respuesta)}, video)
    probar("termina bien", r.returncode == 0, r.stderr)
    tarea = next((tmp / "tareas").iterdir())
    cands = json.loads((tarea / "candidatos.json").read_text())
    picos = [c["pico"] for c in cands]
    probar("encuentra las 3 jugadas entre los candidatos",
           all(any(abs(p - t) <= 4 for p in picos) for t in JUGADAS), str(picos))
    probar("cada candidato trae su tira de miniaturas", all(Path(c["miniatura"]).is_file() for c in cands))
    datos = json.loads(llamada.read_text())
    argv = datos["argv"]
    probar("Claude va sin herramientas, restringido y con Sonnet", argv[argv.index("--tools") + 1] == ""
           and "--restricted" in argv and "--strict-mcp-config" in argv and argv[argv.index("--model") + 1] == "sonnet")
    imagenes = [c for c in datos["entrada"]["message"]["content"] if c.get("type") == "image"]
    probar("le llegan las miniaturas como imágenes", len(imagenes) == len(cands))
    p = json.loads(proyecto.read_text())
    seq = p["sequences"][0]
    pistas = {t["name"]: t for t in seq["tracks"]}
    v1, a1 = pistas["V1"]["clips"], pistas["A1"]["clips"]
    probar("formato 6 de Vortex Studio, con la secuencia activa", p["formato"] == 6 and p["activa"] == seq["id"])
    probar("los momentos que eligió, en orden y sin el que no existe", [c["name"] for c in v1] == ["La primera", "La tercera"],
           str([c["name"] for c in v1]))
    probar("cortes del video original, uno tras otro", v1[0]["start"] == 0 and v1[1]["start"] == v1[0]["duration"]
           and v1[0]["source"] == video.name and v1[0]["in_point"] < v1[1]["in_point"])
    probar("el audio va enlazado en A1", [c["link"] for c in a1] == [c["link"] for c in v1] and all(c["link"] for c in v1))
    probar("un marcador por momento, con su porqué", [(m["name"], m["note"]) for m in seq["markers"]]
           == [("La primera", "Sonido fuerte"), ("La tercera", "Pantalla roja")])
    probar("el tamaño y los fps del video", (seq["width"], seq["height"], seq["fps"]) == (320, 180, 24.0))

    print("== Sin Claude")
    proyecto.unlink()
    claude.unlink()
    r = correr(env, video)
    p = json.loads(proyecto.read_text())
    nombres = [c["name"] for c in next(t for t in p["sequences"][0]["tracks"] if t["name"] == "V1")["clips"]]
    probar("se queda con los picos más altos", r.returncode == 0 and 3 <= len(nombres) <= 5
           and nombres[0] == "Momento 1", str(nombres) + r.stderr)

    print("== Claude contesta algo roto")
    claude.write_text(CLAUDE)
    claude.chmod(0o755)
    r = correr({**env, "RESPUESTA": "no sé"}, video)
    p = json.loads(proyecto.read_text())
    nombres = [c["name"] for c in next(t for t in p["sequences"][0]["tracks"] if t["name"] == "V1")["clips"]]
    probar("también se queda con los picos", r.returncode == 0 and nombres and nombres[0] == "Momento 1", str(nombres))

    print("== Video corto")
    corto = videos / "corto.mp4"
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=320x180:r=24:d=12",
                    "-c:v", "libx264", "-preset", "ultrafast", str(corto)], check=True)
    r = correr(env, corto)
    p = json.loads((videos / "corto - mejores momentos.vortex").read_text())
    pistas = {t["name"]: t for t in p["sequences"][0]["tracks"]}
    probar("va entero y sin audio (no tiene)", r.returncode == 0 and len(pistas["V1"]["clips"]) == 1
           and abs(pistas["V1"]["clips"][0]["duration"] - 12) < 0.2 and not pistas["A1"]["clips"], r.stderr)

    shutil.rmtree(tmp, ignore_errors=True)
    print("\nTodo bien" if not FALLOS else f"\n{FALLOS} fallaron")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
