"""Pruebas de los sentimientos de Miku (corazon.py y «vortex-miku sentir/sentimientos/habla-sola»).

Todo en carpetas temporales: no toca tus notas de Documentos/Miku, tu miku.json ni tu escritorio
(notify-send y hyprctl son de mentira). La hora se fija con VORTEX_MIKU_AHORA.

    python3 -I apps/miku/pruebas/corazon.py
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path

AQUI = Path(__file__).resolve().parent
MIKU = AQUI.parent
TMP = Path(tempfile.mkdtemp(prefix="vortex-miku-corazon-"))
BIN = TMP / "bin"
BIN.mkdir()
AVISOS = TMP / "avisos.txt"
(BIN / "notify-send").write_text(f'#!/bin/sh\nfor a; do last="$a"; done\nprintf "%s\\n" "$last" >>"{AVISOS}"\n')
(BIN / "hyprctl").write_text(f'#!/bin/sh\ncat "{TMP}/ventana.json" 2>/dev/null || echo "{{}}"\n')
(BIN / "playerctl").write_text("#!/bin/sh\nexit 1\n")
for f in BIN.iterdir():
    f.chmod(0o755)
os.environ.update(HOME=str(TMP / "home"), XDG_CONFIG_HOME=str(TMP / "home/.config"),
                  XDG_STATE_HOME=str(TMP / "home/.local/state"), XDG_RUNTIME_DIR=str(TMP / "run"),
                  VORTEX_MIKU_CORAZON=str(TMP / "Documentos/Miku"), PATH=f"{BIN}:/usr/bin:/bin")
os.environ.pop("HYPRLAND_INSTANCE_SIGNATURE", None)
for d in ("home/.config", "run"):
    (TMP / d).mkdir(parents=True)
sys.path.insert(0, str(MIKU))

from miku import corazon  # noqa: E402

FALLOS = []


def ver(cond: bool, texto: str) -> None:
    print(("  ok  " if cond else "  MAL ") + texto, flush=True)
    if not cond:
        FALLOS.append(texto)


def a_las(cuando: str) -> None:
    os.environ["VORTEX_MIKU_AHORA"] = cuando


def poner(**cambios) -> None:
    e = corazon.cargar()
    e.update(cambios)
    corazon._guardar(e)


def cli(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", str(MIKU / "vortex-miku"), *args], capture_output=True, text=True,
                          timeout=30, env=os.environ)


print("== Lo que le dices")
a_las("2026-10-29T18:00")
e = corazon.cargar()
ver(e["afecto"] == corazon.AFECTO_INICIAL and e["animo"] == "tranquila", "empieza tranquila y con 25 de afecto")
r = corazon.al_hablar("hola Miku")
e = corazon.cargar()
ver(e["dias_juntos"] == 1 and e["afecto"] > 25, f"un saludo el primer día sube un poquito ({e['afecto']})")
ver(oct(os.stat(corazon.carpeta()).st_mode & 0o777) == "0o700", "su carpeta es solo tuya (700)")
ver((corazon.carpeta() / "Corazón.md").read_text().startswith("---\ntags: [miku, vortexine]"), "Corazón.md es una nota con tags")
r = corazon.al_hablar("eres muy linda")
ver(r["sonrojo"] >= 0.8 and r["animo"] == "apenada", "algo bonito la sonroja y la pone apenada")
ver(any("algo bonito" in l for l in corazon.diario_de_hoy()), "…y lo anota en su diario de hoy")
r = corazon.al_hablar("te quiero preguntar algo")
ver(r["sonrojo"] == 0, "«te quiero preguntar» no cuenta como cariño")
for _ in range(6):
    corazon.al_hablar("te quiero mucho")
e = corazon.cargar()
ver(e["subio_hoy"] <= corazon.SUBIDA_MAXIMA_DIA + 1e-9 and e["afecto"] <= 25 + corazon.SUBIDA_MAXIMA_DIA + 1e-9,
    f"el cariño sube despacio: tope de {corazon.SUBIDA_MAXIMA_DIA} al día ({e['afecto']:.1f})")
antes = e["afecto"]
r = corazon.al_hablar("eres tonta")
ver(r["animo"] == "triste" and corazon.cargar()["afecto"] < antes, "lo grosero la pone triste y baja el afecto")
ver(corazon.al_hablar("cállate")["animo"] == "", "«cállate» (pedirle que se calle) no la ofende")
a_las("2026-11-04T18:00")
r = corazon.al_hablar("hola")
ver(r["dias_sin_hablar"] == 6 and corazon.cargar()["afecto"] >= antes - 3, "no hablarle no la castiga (no baja el afecto)")
ver("Hace 6 días" not in corazon.contexto(datetime(2026, 11, 4, 18), cancion=""), "…ya hablaron hoy: no dice que lo extrañó")
poner(ultima_charla="2026-10-29T18:00")
ver("lo extrañaste, sin reclamarle" in corazon.contexto(datetime(2026, 11, 4, 18), cancion=""), "tras días sin hablar lo extraña, sin reclamar")

print("== La marca oculta")
f = corazon.FiltroMarca()
trozos = ["Qué bueno ", "que ganaste. [laughs softly] Me alegra", ".\n\n[", "[coraz", "on animo=emocionada; sonrojo=0.6; ",
          "afecto=+1; recuerdo=Juega Roblox con su primo]]"]
visto = "".join(f.alimentar(t) for t in trozos) + f.vaciar()
ver("[[" not in visto and "corazon" not in visto, "la marca no llega a la voz ni a la pantalla, aunque venga en pedazos")
ver("[laughs softly]" in visto, "…y las etiquetas de ElevenLabs siguen pasando")
ver(corazon.leer_marca(f.marca).get("animo") == "emocionada", "la marca se lee completa")
f2 = corazon.FiltroMarca()
ver(f2.alimentar("Hola [[corazon animo=feli") + f2.vaciar() == "Hola ", "una marca cortada a la mitad se tira")
a_las("2026-11-04T18:05")
antes = corazon.cargar()["afecto"]
r = corazon.al_responder(visto + "\n" + f.marca)
e = corazon.cargar()
ver(r["animo"] == "emocionada" and abs(r["sonrojo"] - 0.6) < 1e-9, "su respuesta cambia su ánimo y su sonrojo")
ver(e["afecto"] > antes, "…y su afecto (lo que marcó la IA)")
ver("Juega Roblox con su primo" in corazon.recuerdos(), "guarda el recuerdo en Recuerdos.md")
corazon.al_responder("ok [[corazon animo=feliz; recuerdo=juega roblox con su primo]]")
ver(corazon.recuerdos().count("Juega Roblox con su primo") == 1 and len(corazon.recuerdos()) == 1, "no repite recuerdos")
corazon.al_responder("ok [[corazon animo=dragón; sonrojo=7; afecto=+50]]")
e2 = corazon.cargar()
ver(e2["animo"] == "feliz" and e2["afecto"] - e["afecto"] <= 2.0001, "una marca rara no rompe nada (ánimo válido, topes)")
ver(corazon.al_responder("Yo también te quiero mucho.")["sonrojo"] >= 0.7, "si ella dice algo con cariño, también se sonroja")
ruta = corazon.carpeta() / "Recuerdos.md"
ruta.write_text(ruta.read_text().replace("- Juega Roblox con su primo", "- Su color favorito es el morado"))
ver(corazon.recuerdos() == ["Su color favorito es el morado"], "si corriges Recuerdos.md, ella lo sabe")

print("== Lo que le cuenta a la IA")
ctx = corazon.contexto(datetime(2026, 11, 4, 3, 10), cancion="«Idol» de YOASOBI (en Vortex Player)")
ver("[[corazon animo=" in ctx, "le pide la marca al final")
ver("madrugada" in ctx and "regáñalo con cariño" in ctx, "a las 3 a. m. sabe que te estás desvelando")
ver("YOASOBI" in ctx, "sabe qué música suena")
ver("morado" in ctx, "sabe lo que recuerda de ti")
ver("subido de tono" in ctx and "celosa" in ctx, "sin celos, sin culpa y nada subido de tono")
ver("confesarle" not in ctx, "todavía no se declara (falta cariño)")

print("== Declararse")
poner(afecto=90.0, dias_juntos=6, declarada="")
ver(corazon.nivel(corazon.cargar())[0] == "le gusta", "con 90 de afecto le gustas")
ver("confesarle" in corazon.contexto(datetime(2026, 11, 4, 20), cancion=""), "un día tranquilo por la tarde se puede declarar")
ver("confesarle" not in corazon.contexto(datetime(2026, 11, 4, 9), cancion=""), "…en la mañana no")
(TMP / "ventana.json").write_text('{"fullscreen": 2}')
ver("confesarle" not in corazon.contexto(datetime(2026, 11, 4, 20), cancion=""), "…ni mientras juegas")
(TMP / "ventana.json").unlink()
a_las("2026-11-04T20:30")
r = corazon.al_responder("Oye… me gustas. [[corazon animo=apenada; sonrojo=1; declaracion=si]]")
e = corazon.cargar()
ver(r["declaro"] and e["declarada"] == "2026-11-04" and corazon.nivel(e)[0] == "enamorada", "se declara (y queda anotado)")
ver(not corazon.al_responder("[[corazon declaracion=si]]")["declaro"], "solo se declara una vez")
ver("confesarle" not in corazon.contexto(datetime(2026, 11, 5, 20), cancion=""), "después ya no lo vuelve a intentar")

print("== Partidas y desvelos")
a_las("2026-11-05T22:00")
ver(corazon.evento_partida("Roblox", 10, 60, 60) == "", "una partida corta: no dice nada")
frase = corazon.evento_partida("Roblox", 95, 70, 88)
ver("1 hora y 35 minutos de Roblox" in frase and "GPU llegó a 88" in frase, f"tras una partida larga lo comenta: «{frase}»")
ver(corazon.cargar()["animo"] == "preocupada", "…y se preocupa si la GPU se calentó")
corazon.evento_partida("Roblox", 30, 60, 60)
corazon.evento_partida("Roblox", 30, 60, 60)
ver(corazon.evento_partida("Roblox", 30, 60, 60) == "", "habla sola poquito: máximo 3 partidas al día")
ver(any("Roblox" in l for l in corazon.diario_de_hoy()), "las partidas van a su diario")
ver(corazon.revisar(datetime(2026, 11, 6, 14, 0)) == "", "de día no hay desvelo")
ver(corazon.revisar(datetime(2026, 11, 6, 2, 30)) != "", "a las 2:30 a. m. te dice que te duermas")
ver(corazon.revisar(datetime(2026, 11, 6, 3, 30)) == "", "…una sola vez por noche")
(TMP / "ventana.json").write_text('{"fullscreen": 1}')
ver(corazon.revisar(datetime(2026, 11, 7, 2, 30)) == "", "nunca mientras juegas (pantalla completa)")
(TMP / "ventana.json").unlink()
estado = Path(os.environ["XDG_STATE_HOME"]) / "vortex"
estado.mkdir(parents=True)
(estado / "modo-juego").touch()
ver(corazon.jugando(), "…ni con el modo juego puesto")
(estado / "modo-juego").unlink()

print("== El asistente (sin voz ni IA de verdad)")
from PySide6.QtCore import QObject  # noqa: E402
from miku import asistente, cerebro as modulo_cerebro  # noqa: E402


class AsistenteMinimo(asistente.Asistente):
    """Solo lo que usa el camino de la respuesta: lo que diría la voz se anota en .dicho."""

    def __init__(self):
        QObject.__init__(self)
        self.cfg = {"sentimientos": True}
        self._cerrado, self._pregunta_id, self._animo = False, 1, None
        self._filtro, self._acumulado, self.divisor = corazon.FiltroMarca(), "", modulo_cerebro.DivisorFrases()
        self.dicho, self.caras, self.rubores, self.pantalla = [], [], [], []
        self.emocion.connect(lambda n, p: self.caras.append(n))
        self.sonrojo.connect(self.rubores.append)
        self.respuesta.connect(lambda t, fin: self.pantalla.append(t) if fin else None)

    def _decir(self, frase): self.dicho.append(frase)
    def _revisar_fin_de_turno(self): pass
    def _cambiar_estado(self, *a): pass


a_las("2026-11-10T19:00")
m = AsistenteMinimo()
m._sentir_lo_que_dijo("eres muy tierna")
ver(m.rubores and m.rubores[-1] >= 0.8 and m.caras[-1] == "happy", "le dices algo bonito: se sonroja en la ventana")
for trozo in ["Ay… gracias. ", "Tú también eres muy lindo conmigo.", "\n[[corazon animo=apenada; ", "sonrojo=0.9; afecto=1]]"]:
    m._al_evento_cerebro("texto", {"delta": trozo, "etiqueta": 1})
m._al_evento_cerebro("listo", {"texto": "", "etiqueta": 1})
ver(not any("corazon" in d or "[[" in d for d in m.dicho), f"la voz nunca dice la marca: {m.dicho}")
ver(m.pantalla and "corazon" not in m.pantalla[-1] and "lindo conmigo" in m.pantalla[-1], "ni sale en la pantalla")
ver(corazon.cargar()["animo"] == "apenada" and m._animo == corazon.expresion("apenada"), "su ánimo queda como base de su cara")
ver(m.rubores[-1] >= 0.85, "y se sonroja con lo que ella misma dijo")
m.cfg["sentimientos"] = False
n = len(m.rubores)
m._sentir_lo_que_dijo("te amo")
ver(len(m.rubores) == n, "con los sentimientos apagados no siente nada")

print("== vortex-miku")
r = cli("sentimientos")
ver(r.returncode == 0 and "sentimientos=on" in r.stdout and "enamorada" in r.stdout, "sentimientos: dice cómo está su corazón")
a_las("2026-11-08T02:40")
AVISOS.unlink(missing_ok=True)
r = cli("sentir", "revisar")
fin = time.time() + 5
while time.time() < fin and not AVISOS.exists():
    time.sleep(0.1)
ver(r.returncode == 0 and AVISOS.exists() and "2:40" in AVISOS.read_text(), "sentir revisar: avisa si te desvelas")
ver(cli("habla-sola", "off").stdout.strip() == "habla-sola=off", "habla-sola off")
AVISOS.unlink(missing_ok=True)
a_las("2026-11-09T02:40")
cli("sentir", "revisar")
time.sleep(1)
ver(not AVISOS.exists(), "…y ya no avisa")
cli("habla-sola", "on")
ver(cli("sentimientos", "off").returncode == 0, "sentimientos off")
n = len(corazon.diario_de_hoy())
cli("sentir", "partida", "Roblox", "50", "60", "60")
ver(len(corazon.diario_de_hoy()) == n, "…y ya no siente nada (no anota la partida)")
ver(cli("sentimientos", "on").returncode == 0 and "sentimientos=on" in cli("sentimientos").stdout, "sentimientos on")
ver(cli("sentimientos", "talvez").returncode == 2, "un valor raro da error de uso")

shutil.rmtree(TMP, ignore_errors=True)
print()
if FALLOS:
    print(f"FALLARON {len(FALLOS)}: {FALLOS}")
    sys.exit(1)
print("Todo bien")
