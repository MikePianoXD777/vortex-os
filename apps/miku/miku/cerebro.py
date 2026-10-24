"""El cerebro de Miku: Claude Sonnet por «claude -p», con solo las herramientas de vista de Vortex.

Cada pregunta es un proceso de `claude -p` (modo no interactivo, salida en flujo) que se reanuda con
--resume para seguir la conversación. Seguridad, en capas (todas probadas contra un intento real
de usar Bash, Read, Write y WebFetch, que volvieron como error):
  - --tools "" quita todas las herramientas integradas;
  - --strict-mcp-config deja SOLO el servidor MCP «vortex» (nada de los MCP del usuario);
  - --allowedTools permite únicamente ver_pantalla, ventana_activa, ventanas y estado_sistema;
  - --restricted ignora los ajustes del usuario (hooks, permisos, plugins) y quita lo que corre comandos;
  - --permission-mode dontAsk: lo no permitido se rechaza sin preguntar.
La conversación se olvida al cerrar: se borra la transcripción que Claude Code guarda en ~/.claude.
"""

import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime
from pathlib import Path

from . import rutas

HERRAMIENTAS_PERMITIDAS = [
    "mcp__vortex__ver_pantalla", "mcp__vortex__ventana_activa", "mcp__vortex__ventanas", "mcp__vortex__estado_sistema",
]
# Las acciones (acciones_mcp.py): cada una pide permiso en la ventana de Miku antes de hacer nada.
ACCIONES = ["instalar_app", "quitar_app", "abrir", "correr_comando", "ajuste", "grabar", "mejores_momentos",
            "redactar_correo", "redactar_mensaje"]
HERRAMIENTAS_ACCIONES = [f"mcp__miku__{a}" for a in ACCIONES]
SERVIDOR_ACCIONES = Path(__file__).resolve().parent / "acciones_mcp.py"
# Internet (opcional, «internet» en miku.json): la búsqueda y la lectura de páginas de Claude, cada una con tu
# permiso en la ventana (el hook permiso_web.py corre antes de cada una).
HERRAMIENTAS_WEB = ["WebSearch", "WebFetch"]
HOOK_WEB = Path(__file__).resolve().parent / "permiso_web.py"
# Portapapeles e historial de la terminal existen en vortex-mcp, pero una asistente que habla en voz
# alta no debe leerlos; el resto es cinturón y tirantes sobre --tools "".
HERRAMIENTAS_PROHIBIDAS = [
    "mcp__vortex__portapapeles", "mcp__vortex__historial_terminal",
    "Bash", "Read", "Edit", "Write", "NotebookEdit", "WebFetch", "WebSearch", "Task", "Agent", "Skill", "ToolSearch",
]
# Variables que desviarían la facturación (clave de API) o saltarían el login de la suscripción.
VARIABLES_FUERA = ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "CLAUDE_CODE_SIMPLE")
SILENCIO_MAXIMO = 30.0      # segundos sin ninguna señal de Claude antes de darlo por colgado
DURACION_MAXIMA = 180.0


def encontrar_claude() -> str | None:
    """El claude del usuario. El instalador oficial lo deja en ~/.local/bin, que puede no estar en el PATH."""
    for d in (str(Path.home() / ".local/bin"), "/usr/local/bin", "/usr/bin"):
        ruta = os.path.join(d, "claude")
        if os.access(ruta, os.X_OK):
            return ruta
    return shutil.which("claude")


def encontrar_vortex() -> str | None:
    """El comando «vortex» de Vortexine's (activa Claude, abre su terminal…)."""
    for ruta in (os.environ.get("VORTEX_BIN"), shutil.which("vortex"), "/usr/local/bin/vortex", "/usr/bin/vortex"):
        if ruta and os.access(ruta, os.X_OK):
            return ruta
    return None


def claude_activado() -> bool | None:
    """¿Está activado Claude en Vortexine's? (`vortex claude activado` sale con 0 si sí; viene APAGADO de fábrica.)

    None = no hay comando «vortex» (otra distro): no se puede saber y se confía en que `claude` exista.
    """
    vortex = encontrar_vortex()
    if not vortex:
        return None
    try:
        r = subprocess.run([vortex, "claude", "activado"], capture_output=True, timeout=10, stdin=subprocess.DEVNULL)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return r.returncode == 0


MSG_ACTIVAR = "Para hablar conmigo necesito tu cuenta de Claude."
MSG_INSTALANDO = "Claude todavía no está instalado en tu usuario. Toca Activar Claude y lo dejo listo."
MSG_SESION = "No tengo sesión en Claude. Inicia sesión con tu cuenta para que pueda hablar contigo."


def encontrar_mcp() -> str | None:
    for ruta in (os.environ.get("VORTEX_MCP"), "/usr/local/bin/vortex-mcp", "/usr/bin/vortex-mcp"):
        if ruta and os.access(ruta, os.X_OK):
            return ruta
    return shutil.which("vortex-mcp")


def leer_persona(avatar: bool) -> str:
    nombre = "miku.txt" if avatar else "asistente.txt"
    try:
        return (rutas.PERSONAS / nombre).read_text(encoding="utf-8").strip()
    except OSError:
        return ("Eres el asistente de voz de Vortex OS. Responde en español, en una a tres frases cortas, "
                "sin markdown ni emojis, porque tu respuesta se lee en voz alta.")


def entorno_claude() -> dict:
    env = {k: v for k, v in os.environ.items() if k not in VARIABLES_FUERA}
    env.update({
        "CLAUDE_CODE_DISABLE_CLAUDE_MDS": "1",      # que no se cuele el CLAUDE.md de las carpetas del usuario
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY": "1",
        "DISABLE_AUTOUPDATER": "1",
        "CLAUDE_CODE_MAX_RETRIES": "2",             # sin internet, falla en ~3 s y no en minutos
        "MCP_TOOL_TIMEOUT": "300000",               # una acción espera hasta 2 min tu permiso y luego corre
    })
    env["PATH"] = os.pathsep.join([str(Path.home() / ".local/bin"), env.get("PATH", "")])
    return env


def construir_comando(claude: str, persona: str, texto: str, sesion: str | None, mcp: str | None,
                      autorizar: str | None = None, internet: bool = False) -> list[str]:
    web = internet and bool(autorizar)          # sin la ventana para pedir permiso, nada de internet
    cmd = [claude, "-p", "--model", "sonnet", "--effort", "low",
           "--output-format", "stream-json", "--verbose", "--include-partial-messages",
           "--append-system-prompt", persona,
           "--restricted", "--tools", ",".join(HERRAMIENTAS_WEB) if web else ""]
    if web:
        hook = f"{shlex.quote(sys.executable)} -I {shlex.quote(str(HOOK_WEB))}"
        cmd += ["--settings", json.dumps({"hooks": {"PreToolUse": [
            {"matcher": "|".join(HERRAMIENTAS_WEB), "hooks": [{"type": "command", "command": hook, "timeout": 150}]}]}})]
    cmd += ["--strict-mcp-config"]
    servidores, permitidas = {}, []
    if mcp:
        servidores["vortex"] = {"command": mcp}
        permitidas += HERRAMIENTAS_PERMITIDAS
    if autorizar:
        # «dontAsk» deja pasar estas herramientas sin preguntar en Claude: el permiso lo pide cada acción en la
        # ventana de Miku (el socket), y sin tu clic no hacen nada.
        servidores["miku"] = {"command": sys.executable, "args": ["-I", str(SERVIDOR_ACCIONES)],
                              "env": {"VORTEX_MIKU_AUTORIZAR": autorizar}}
        permitidas += HERRAMIENTAS_ACCIONES
    if servidores:
        cmd += ["--mcp-config", json.dumps({"mcpServers": servidores})]
    if web:
        permitidas += HERRAMIENTAS_WEB
    if permitidas:
        cmd += ["--allowedTools", *permitidas]
    prohibidas = [h for h in HERRAMIENTAS_PROHIBIDAS if not (web and h in HERRAMIENTAS_WEB)]
    cmd += ["--disallowedTools", *prohibidas,
            "--permission-mode", "dontAsk", "--disable-slash-commands", "--max-turns", "8"]
    if sesion:
        cmd += ["--resume", sesion]
    # «--» es obligatorio: --allowedTools y --disallowedTools aceptan varios valores y se comerían el
    # texto; también protege las frases que empiecen con «-».
    return cmd + ["--", texto]


# ---- Frases: el texto llega en trozos y cada frase completa se manda a la voz -----------------------

_ABREVIATURAS = {"sr", "sra", "srta", "dr", "dra", "etc", "ej", "aprox", "vs", "núm", "pág", "av", "ud", "uds"}
_FIN_FRASE = re.compile(r"[.!?…]+[\"»”’)\]]*(\s+)")


class DivisorFrases:
    """Parte el texto que llega en trozos en frases completas (para empezar a hablar con la primera)."""

    def __init__(self):
        self.buf = ""

    def alimentar(self, trozo: str) -> list[str]:
        self.buf += trozo
        frases, inicio = [], 0
        for m in _FIN_FRASE.finditer(self.buf):
            previa = self.buf[inicio:m.start()].rsplit(None, 1)
            ultima = previa[-1].lower().rstrip(".") if previa else ""
            if m.group().startswith(".") and ultima in _ABREVIATURAS:
                continue
            frase = self.buf[inicio:m.end()].strip()
            if frase:
                frases.append(frase)
            inicio = m.end()
        # Saltos de línea también cortan (listas, párrafos).
        resto = self.buf[inicio:]
        while "\n" in resto:
            antes, _, resto = resto.partition("\n")
            if antes.strip():
                frases.append(antes.strip())
        self.buf = resto
        return frases

    def vaciar(self) -> str:
        resto, self.buf = self.buf.strip(), ""
        return resto


# ---- Errores: siempre en español, con qué hacer -----------------------------------------------------

def mensaje_error(resultado: str, estado_api, limite: dict | None, stderr: str) -> tuple[str, str]:
    """(clave, mensaje en español) a partir de lo que dijo Claude Code."""
    r = f"{resultado or ''}\n{stderr or ''}"
    if limite and limite.get("status") == "rejected" or (estado_api == 429 and re.search(r"hit your|limit", r, re.I)):
        cuando = ""
        if limite and limite.get("resetsAt"):
            try:
                cuando = " Vuelvo a las " + datetime.fromtimestamp(int(limite["resetsAt"])).strftime("%H:%M") + "."
            except (ValueError, OSError, OverflowError):
                pass
        return "cuota", "Ya usé todo el límite de Claude por ahora." + cuando
    if re.search(r"not logged in|/login|oauth|invalid authentication|revoked|expired|credentials", r, re.I) or estado_api == 401:
        return "sesion", MSG_SESION
    if re.search(r"unable to connect|econn|enotfound|ehostunreach|etimedout|timed out|connection (refused|dropped|lost)|can't reach|no internet|network", r, re.I):
        return "internet", "No tengo internet, así que no puedo pensar ahora mismo."
    if estado_api in (500, 502, 503, 504, 529) or re.search(r"overloaded|api error: 5\d\d", r, re.I):
        return "saturado", "Claude está saturado en este momento. Prueba otra vez en un rato."
    if re.search(r"unknown option|unknown argument|--restricted|unrecognized", r, re.I):
        return "version", "Tu Claude Code es muy viejo para mí. Actualízalo en una terminal con: claude update"
    return "otro", "No pude hablar con Claude. Mira que tengas internet y sesión iniciada."


class Cerebro:
    """Una conversación. `preguntar()` corre en un hilo y avisa por `al_evento(tipo, etiqueta=..., **datos)`:

      ("inicio",)                          Claude arrancó
      ("texto", delta=str)                 un trozo de la respuesta
      ("herramienta", nombre=str)          va a mirar (pantalla, ventanas...)
      ("bloque_fin",)                      terminó un bloque de texto (hay que vaciar la última frase)
      ("listo", texto=str)                 respuesta completa
      ("error", clave=str, mensaje=str)    algo falló; mensaje listo para mostrar y decir. Las claves
                                           «activar» (Claude apagado o sin instalar) y «sesion» (sin cuenta
                                           iniciada) piden un botón en la ventana; el resto solo se dice.

    Si la pregunta se cancela (`cancelar()`), ya no avisa de nada.
    """

    def __init__(self, avatar: bool, al_evento):
        self.avatar = avatar
        self.al_evento = al_evento
        self.persona = leer_persona(avatar)
        self.sesion: str | None = None
        self.sesiones_creadas: set[str] = set()
        self.ultimo_init: dict = {}                     # herramientas y servidores MCP que Claude dijo tener (diagnóstico)
        self._proc: subprocess.Popen | None = None
        self._token: threading.Event | None = None      # se activa al cancelar la pregunta en curso
        self._lock = threading.Lock()
        self._sesion_comprobada = False
        self.claude = encontrar_claude()
        self.mcp = encontrar_mcp()
        self.autorizar: str | None = None                # socket del permiso de las acciones (lo pone el asistente)
        self.internet = False                            # búsqueda web y páginas, con permiso (lo pone el asistente)
        self.ocupado = threading.Event()                # una acción espera tu clic o está corriendo: no cortar a Claude
        if not self.mcp:
            self.persona += "\n\nEn esta sesión no tienes acceso a la pantalla: si te piden mirarla, explica que no puedes."

    def comprobar(self) -> tuple[str, str]:
        """(«ok», "") si se puede hablar con Claude; si no, (clave, mensaje en español) con qué hacer.

        Claude viene apagado en Vortexine's: se comprueba que esté activado (`vortex claude activado`), que
        el programa exista y que haya una sesión iniciada (`claude auth status`, que no gasta nada).
        """
        self.claude = encontrar_claude()
        if claude_activado() is False:
            self._sesion_comprobada = False
            return "activar", MSG_ACTIVAR
        if not self.claude:
            return "activar", MSG_INSTALANDO
        if not self._sesion_comprobada:
            if self.sesion_iniciada() is False:
                return "sesion", MSG_SESION
            self._sesion_comprobada = True
        return "ok", ""

    def sesion_iniciada(self) -> bool | None:
        """¿Hay sesión en Claude? None si no se pudo saber. `claude auth status` tarda ~0,1 s y no gasta nada."""
        if not self.claude:
            return None
        try:
            r = subprocess.run([self.claude, "auth", "status"], capture_output=True, timeout=10, env=entorno_claude(),
                               stdin=subprocess.DEVNULL)
        except (OSError, subprocess.TimeoutExpired):
            return None
        return r.returncode == 0

    def cancelar(self) -> None:
        if self._token is not None:
            self._token.set()
        self._matar()

    def _matar(self) -> None:
        with self._lock:
            p = self._proc
        if p and p.poll() is None:
            p.terminate()
            try:
                p.wait(timeout=2)
            except subprocess.TimeoutExpired:
                p.kill()

    def preguntar(self, texto: str, etiqueta=None, reintento: bool = False) -> None:
        """Hace una pregunta y avisa del resultado. `etiqueta` vuelve en cada evento (para descartar los viejos)."""
        token = self._token = threading.Event()

        def avisar(tipo, **datos):
            if not token.is_set():
                self.al_evento(tipo, etiqueta=etiqueta, **datos)

        clave, mensaje = self.comprobar()
        if clave != "ok":
            avisar("error", clave=clave, mensaje=mensaje)
            return
        rutas.CEREBRO_CWD.mkdir(parents=True, exist_ok=True)
        cmd = construir_comando(self.claude, self.persona, texto, self.sesion, self.mcp, self.autorizar, self.internet)
        env = entorno_claude()
        if self.autorizar:
            env["VORTEX_MIKU_AUTORIZAR"] = self.autorizar        # para el hook de internet
        try:
            p = subprocess.Popen(cmd, cwd=rutas.CEREBRO_CWD, env=env, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        except OSError:
            avisar("error", clave="activar", mensaje=MSG_INSTALANDO)
            return
        with self._lock:
            self._proc = p
        errores: list[str] = []
        threading.Thread(target=lambda: errores.append(p.stderr.read()), daemon=True).start()
        ultima = [time.monotonic()]
        inicio = time.monotonic()

        tiempo_en_acciones = [0.0]

        def vigilante():
            while p.poll() is None:
                time.sleep(1.0)
                ahora = time.monotonic()
                if self.ocupado.is_set():                # esperando tu permiso o corriendo una acción
                    ultima[0] = ahora
                    tiempo_en_acciones[0] += 1.0
                    continue
                if ahora - ultima[0] > SILENCIO_MAXIMO or ahora - inicio - tiempo_en_acciones[0] > DURACION_MAXIMA:
                    p.kill()
                    return
        threading.Thread(target=vigilante, daemon=True).start()

        resultado = None
        limite = None
        hubo_texto = False
        try:
            for linea in p.stdout:
                ultima[0] = time.monotonic()
                try:
                    ev = json.loads(linea)
                except ValueError:
                    continue
                tipo = ev.get("type")
                if tipo == "system" and ev.get("subtype") == "init":
                    self.ultimo_init = {k: ev.get(k) for k in ("tools", "mcp_servers", "model", "permissionMode")}
                    self.sesion = ev.get("session_id") or self.sesion
                    if self.sesion:
                        self.sesiones_creadas.add(self.sesion)
                    avisar("inicio")
                elif tipo == "stream_event":
                    e = ev.get("event", {})
                    et = e.get("type")
                    if et == "content_block_start" and e.get("content_block", {}).get("type") == "tool_use":
                        avisar("herramienta", nombre=e["content_block"].get("name", ""))
                    elif et == "content_block_delta" and e.get("delta", {}).get("type") == "text_delta":
                        hubo_texto = True
                        avisar("texto", delta=e["delta"].get("text", ""))
                    elif et == "content_block_stop":
                        avisar("bloque_fin")
                elif tipo == "rate_limit_event":
                    limite = ev.get("rate_limit_info")
                elif tipo == "result":
                    resultado = ev
                    self.sesion = ev.get("session_id") or self.sesion
        finally:
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
            with self._lock:
                self._proc = None
        if token.is_set():
            return
        stderr = errores[0] if errores else ""
        if resultado is None or resultado.get("is_error") or p.returncode not in (0, None):
            texto_r = (resultado or {}).get("result", "")
            if "No conversation found" in texto_r and self.sesion and not reintento:
                self.sesion = None                  # la conversación ya no existe: empieza otra
                return self.preguntar(texto, etiqueta=etiqueta, reintento=True)
            clave, msg = mensaje_error(texto_r, (resultado or {}).get("api_error_status"), limite, stderr)
            if clave == "sesion":
                self._sesion_comprobada = False
            if resultado is None and not hubo_texto and clave == "otro" and time.monotonic() - inicio >= SILENCIO_MAXIMO:
                clave, msg = "internet", "Claude tardó demasiado en contestar. Mira tu internet y vuelve a intentarlo."
            avisar("error", clave=clave, mensaje=msg)
            return
        avisar("listo", texto=resultado.get("result", ""))

    def olvidar(self) -> None:
        """Termina la conversación y borra su rastro (la transcripción que Claude Code guarda en ~/.claude)."""
        self.cancelar()
        base = Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude") / "projects"
        carpeta = base / re.sub(r"[^A-Za-z0-9]", "-", str(rutas.CEREBRO_CWD))
        for sid in self.sesiones_creadas:
            for sufijo in (".jsonl",):
                try:
                    (carpeta / f"{sid}{sufijo}").unlink()
                except OSError:
                    pass
            shutil.rmtree(carpeta / sid, ignore_errors=True)       # subcarpeta de la sesión, si la hubo
        try:
            carpeta.rmdir()                                       # solo si quedó vacía
        except OSError:
            pass
        self.sesiones_creadas.clear()
        self.sesion = None
