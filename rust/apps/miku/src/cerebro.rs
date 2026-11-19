//! El cerebro de Miku con Claude: Sonnet por `claude -p`, con solo las herramientas de vista de Vortex.
//!
//! Cada pregunta es un proceso de `claude -p` (modo no interactivo, salida en flujo) que se reanuda con
//! `--resume` para seguir la conversación. Seguridad, en capas: `--tools ""` quita las herramientas
//! integradas; `--strict-mcp-config` deja SOLO el servidor MCP «vortex» (y el de acciones de Miku);
//! `--allowedTools` permite únicamente ver_pantalla, ventana_activa, ventanas y estado_sistema (y las
//! acciones, que piden permiso en la ventana); `--restricted` ignora los ajustes del usuario;
//! `--permission-mode dontAsk` rechaza sin preguntar lo no permitido. La conversación se olvida al cerrar:
//! se borra la transcripción que Claude Code guarda en `~/.claude`.

use crate::eventos::{AlEvento, Cerebro as CerebroT, Evento};
use crate::rutas::Rutas;
use crate::sistema;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const HERRAMIENTAS_PERMITIDAS: [&str; 4] = [
    "mcp__vortex__ver_pantalla",
    "mcp__vortex__ventana_activa",
    "mcp__vortex__ventanas",
    "mcp__vortex__estado_sistema",
];
pub const ACCIONES: [&str; 12] = [
    "instalar_app",
    "quitar_app",
    "abrir",
    "correr_comando",
    "ajuste",
    "grabar",
    "mejores_momentos",
    "redactar_correo",
    "redactar_mensaje",
    "ver_camara",
    "escribir",
    "clic",
];
pub const HERRAMIENTAS_WEB: [&str; 2] = ["WebSearch", "WebFetch"];
pub const HERRAMIENTAS_PROHIBIDAS: [&str; 13] = [
    "mcp__vortex__portapapeles",
    "mcp__vortex__historial_terminal",
    "Bash",
    "Read",
    "Edit",
    "Write",
    "NotebookEdit",
    "WebFetch",
    "WebSearch",
    "Task",
    "Agent",
    "Skill",
    "ToolSearch",
];
pub const VARIABLES_FUERA: [&str; 4] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CLAUDE_CODE_SIMPLE",
];
pub const SILENCIO_MAXIMO: f64 = 30.0;
pub const DURACION_MAXIMA: f64 = 180.0;

pub const MSG_ACTIVAR: &str = "Para hablar conmigo necesito tu cuenta de Claude.";
pub const MSG_INSTALANDO: &str =
    "Claude todavía no está instalado en tu usuario. Toca Activar Claude y lo dejo listo.";
pub const MSG_SESION: &str =
    "No tengo sesión en Claude. Inicia sesión con tu cuenta para que pueda hablar contigo.";

fn home() -> PathBuf {
    vortex_core::rutas::home()
}

/// El `claude` del usuario. El instalador oficial lo deja en `~/.local/bin`, que puede no estar en el PATH.
pub fn encontrar_claude() -> Option<PathBuf> {
    [
        home().join(".local/bin"),
        "/usr/local/bin".into(),
        "/usr/bin".into(),
    ]
    .into_iter()
    .map(|d| d.join("claude"))
    .find(|r| sistema::es_ejecutable(r))
    .or_else(|| sistema::cual("claude"))
}

/// El comando «vortex» de Vortexine's (activa Claude, abre su terminal…).
pub fn encontrar_vortex() -> Option<PathBuf> {
    std::env::var("VORTEX_BIN")
        .ok()
        .map(PathBuf::from)
        .into_iter()
        .chain(sistema::cual("vortex"))
        .chain(["/usr/local/bin/vortex".into(), "/usr/bin/vortex".into()])
        .find(|r| sistema::es_ejecutable(r))
}

/// ¿Está activado Claude en Vortexine's? `None` = no hay comando «vortex» (otra distro): no se puede saber.
pub fn claude_activado() -> Option<bool> {
    let v = encontrar_vortex()?;
    let r = Command::new(v)
        .args(["claude", "activado"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    Some(r.success())
}

pub fn encontrar_mcp() -> Option<PathBuf> {
    std::env::var("VORTEX_MCP")
        .ok()
        .map(PathBuf::from)
        .into_iter()
        .chain(["/usr/local/bin/vortex-mcp".into(), "/usr/bin/vortex-mcp".into()])
        .find(|r| sistema::es_ejecutable(r))
        .or_else(|| sistema::cual("vortex-mcp"))
}

/// El binario auxiliar (`miku-aux`): el servidor MCP de las acciones, el hook de internet y los mejores momentos.
pub fn encontrar_aux() -> PathBuf {
    if let Some(a) = std::env::var_os("VORTEX_MIKU_AUX").filter(|v| !v.is_empty()) {
        return a.into();
    }
    let junto = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("miku-aux")));
    junto
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("/usr/lib/vortex/miku-aux"))
}

pub fn leer_persona(rutas: &Rutas, avatar: bool) -> String {
    let nombre = if avatar { "miku.txt" } else { "asistente.txt" };
    std::fs::read_to_string(rutas.personas.join(nombre)).map(|t| t.trim().to_string()).unwrap_or_else(|_| {
        "Eres el asistente de voz de Vortex OS. Responde en español, en una a tres frases cortas, sin markdown ni emojis, porque tu respuesta se lee en voz alta.".into()
    })
}

/// Entre comillas simples para el shell (como `shlex.quote`).
pub fn comillas(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c))
    {
        return s.into();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// Cambios al entorno de `claude`: (quitar, poner).
pub fn entorno_claude() -> (Vec<&'static str>, Vec<(String, String)>) {
    let path = format!(
        "{}:{}",
        home().join(".local/bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (
        VARIABLES_FUERA.to_vec(),
        vec![
            ("CLAUDE_CODE_DISABLE_CLAUDE_MDS".into(), "1".into()),
            ("CLAUDE_CODE_DISABLE_AUTO_MEMORY".into(), "1".into()),
            ("DISABLE_AUTOUPDATER".into(), "1".into()),
            ("CLAUDE_CODE_MAX_RETRIES".into(), "2".into()),
            ("MCP_TOOL_TIMEOUT".into(), "300000".into()),
            ("PATH".into(), path),
        ],
    )
}

/// Los argumentos de `claude -p …` (sin el programa).
pub fn construir_comando(
    persona: &str,
    texto: &str,
    sesion: Option<&str>,
    mcp: Option<&Path>,
    autorizar: Option<&str>,
    internet: bool,
    aux: &Path,
) -> Vec<String> {
    let web = internet && autorizar.is_some();
    let mut c: Vec<String> = [
        "-p",
        "--model",
        "sonnet",
        "--effort",
        "low",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--append-system-prompt",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    c.push(persona.into());
    c.extend([
        "--restricted".into(),
        "--tools".into(),
        if web {
            HERRAMIENTAS_WEB.join(",")
        } else {
            String::new()
        },
    ]);
    if web {
        let hook = format!("{} hook-web", comillas(&aux.to_string_lossy()));
        c.extend([
            "--settings".into(),
            json!({"hooks": {"PreToolUse": [{"matcher": HERRAMIENTAS_WEB.join("|"), "hooks": [{"type": "command", "command": hook, "timeout": 150}]}]}}).to_string(),
        ]);
    }
    c.push("--strict-mcp-config".into());
    let mut servidores = serde_json::Map::new();
    let mut permitidas: Vec<String> = vec![];
    if let Some(m) = mcp {
        servidores.insert("vortex".into(), json!({"command": m}));
        permitidas.extend(HERRAMIENTAS_PERMITIDAS.iter().map(|s| s.to_string()));
    }
    if let Some(a) = autorizar {
        // «dontAsk» deja pasar estas herramientas sin preguntar en Claude: el permiso lo pide cada acción en
        // la ventana de Miku (el socket), y sin tu clic no hacen nada.
        servidores.insert(
            "miku".into(),
            json!({"command": aux, "args": ["mcp"], "env": {"VORTEX_MIKU_AUTORIZAR": a}}),
        );
        permitidas.extend(ACCIONES.iter().map(|x| format!("mcp__miku__{x}")));
    }
    if !servidores.is_empty() {
        c.extend([
            "--mcp-config".into(),
            json!({"mcpServers": servidores}).to_string(),
        ]);
    }
    if web {
        permitidas.extend(HERRAMIENTAS_WEB.iter().map(|s| s.to_string()));
    }
    if !permitidas.is_empty() {
        c.push("--allowedTools".into());
        c.extend(permitidas);
    }
    c.push("--disallowedTools".into());
    c.extend(
        HERRAMIENTAS_PROHIBIDAS
            .iter()
            .filter(|h| !(web && HERRAMIENTAS_WEB.contains(h)))
            .map(|s| s.to_string()),
    );
    c.extend(
        [
            "--permission-mode",
            "dontAsk",
            "--disable-slash-commands",
            "--max-turns",
            "8",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    if let Some(s) = sesion {
        c.extend(["--resume".into(), s.into()]);
    }
    // «--» es obligatorio: --allowedTools y --disallowedTools aceptan varios valores y se comerían el texto.
    c.extend(["--".into(), texto.into()]);
    c
}

// ---- Frases: el texto llega en trozos y cada frase completa se manda a la voz ------------------------

const ABREVIATURAS: [&str; 14] = [
    "sr", "sra", "srta", "dr", "dra", "etc", "ej", "aprox", "vs", "núm", "pág", "av", "ud", "uds",
];

fn fin_frase() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"[.!?…]+["»”’)\]]*(\s+)"#).unwrap())
}

/// Parte el texto que llega en trozos en frases completas (para empezar a hablar con la primera).
#[derive(Default, Debug)]
pub struct DivisorFrases {
    buf: String,
}

impl DivisorFrases {
    pub fn alimentar(&mut self, trozo: &str) -> Vec<String> {
        self.buf.push_str(trozo);
        let (mut frases, mut inicio) = (vec![], 0usize);
        for m in fin_frase().find_iter(&self.buf) {
            let previa = &self.buf[inicio..m.start()];
            let ultima = previa.split_whitespace().last().unwrap_or("").to_lowercase();
            let ultima = ultima.trim_end_matches('.');
            if m.as_str().starts_with('.') && ABREVIATURAS.contains(&ultima) {
                continue;
            }
            let frase = self.buf[inicio..m.end()].trim();
            if !frase.is_empty() {
                frases.push(frase.to_string());
            }
            inicio = m.end();
        }
        // Saltos de línea también cortan (listas, párrafos).
        let mut resto = self.buf[inicio..].to_string();
        while let Some(i) = resto.find('\n') {
            let antes = resto[..i].trim().to_string();
            if !antes.is_empty() {
                frases.push(antes);
            }
            resto = resto[i + 1..].to_string();
        }
        self.buf = resto;
        frases
    }

    pub fn vaciar(&mut self) -> String {
        std::mem::take(&mut self.buf).trim().to_string()
    }
}

// ---- Errores: siempre en español, con qué hacer --------------------------------------------------------

pub fn mensaje_error(
    resultado: &str,
    estado_api: Option<i64>,
    limite: Option<&Value>,
    stderr: &str,
) -> (String, String) {
    let r = format!("{resultado}\n{stderr}");
    let m = |p: &str| Regex::new(&format!("(?i){p}")).unwrap().is_match(&r);
    let rechazado = limite.and_then(|l| l.get("status")).and_then(Value::as_str) == Some("rejected");
    if rechazado || (estado_api == Some(429) && m("hit your|limit")) {
        let mut cuando = String::new();
        if let Some(t) = limite
            .and_then(|l| l.get("resetsAt"))
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
        {
            cuando = format!(" Vuelvo a las {}.", crate::fecha::desde_epoch_local(t).hm());
        }
        return (
            "cuota".into(),
            format!("Ya usé todo el límite de Claude por ahora.{cuando}"),
        );
    }
    if m("not logged in|/login|oauth|invalid authentication|revoked|expired|credentials")
        || estado_api == Some(401)
    {
        return ("sesion".into(), MSG_SESION.into());
    }
    if m(
        r"unable to connect|econn|enotfound|ehostunreach|etimedout|timed out|connection (refused|dropped|lost)|can't reach|no internet|network",
    ) {
        return (
            "internet".into(),
            "No tengo internet, así que no puedo pensar ahora mismo.".into(),
        );
    }
    if matches!(estado_api, Some(500 | 502 | 503 | 504 | 529)) || m(r"overloaded|api error: 5\d\d") {
        return (
            "saturado".into(),
            "Claude está saturado en este momento. Prueba otra vez en un rato.".into(),
        );
    }
    if m("unknown option|unknown argument|--restricted|unrecognized") {
        return (
            "version".into(),
            "Tu Claude Code es muy viejo para mí. Actualízalo en una terminal con: claude update".into(),
        );
    }
    (
        "otro".into(),
        "No pude hablar con Claude. Mira que tengas internet y sesión iniciada.".into(),
    )
}

#[derive(Default)]
struct Estado {
    sesion: Option<String>,
    sesiones_creadas: HashSet<String>,
    pid: Option<u32>,
    token: Option<Arc<AtomicBool>>,
    ultimo_init: Value,
}

pub struct Cerebro {
    pub avatar: bool,
    al_evento: AlEvento,
    rutas: Rutas,
    persona: Mutex<String>,
    sentimientos: Mutex<String>,
    estado: Mutex<Estado>,
    sesion_comprobada: AtomicBool,
    /// Preguntar a `vortex claude activado` (las pruebas no lo hacen: no miran el estado real de nadie).
    pub revisar_activado: AtomicBool,
    /// Un `claude` fijo (las pruebas); si no, se busca.
    pub claude_fijo: Mutex<Option<PathBuf>>,
    pub mcp: Mutex<Option<PathBuf>>,
    pub aux: Mutex<PathBuf>,
    autorizar: Mutex<Option<String>>,
    internet: AtomicBool,
    ocupado: Arc<AtomicBool>,
}

impl Cerebro {
    pub fn nuevo(rutas: &Rutas, avatar: bool, al_evento: AlEvento) -> Self {
        let mcp = encontrar_mcp();
        let mut persona = leer_persona(rutas, avatar);
        if mcp.is_none() {
            persona += "\n\nEn esta sesión no tienes acceso a la pantalla: si te piden mirarla, explica que no puedes.";
        }
        Self {
            avatar,
            al_evento,
            rutas: rutas.clone(),
            persona: Mutex::new(persona),
            sentimientos: Mutex::new(String::new()),
            estado: Mutex::new(Estado::default()),
            sesion_comprobada: AtomicBool::new(false),
            revisar_activado: AtomicBool::new(true),
            claude_fijo: Mutex::new(None),
            mcp: Mutex::new(mcp),
            aux: Mutex::new(encontrar_aux()),
            autorizar: Mutex::new(None),
            internet: AtomicBool::new(false),
            ocupado: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn persona(&self) -> String {
        self.persona.lock().unwrap().clone()
    }

    pub fn sesion(&self) -> Option<String> {
        self.estado.lock().unwrap().sesion.clone()
    }

    pub fn ultimo_init(&self) -> Value {
        self.estado.lock().unwrap().ultimo_init.clone()
    }

    fn claude(&self) -> Option<PathBuf> {
        self.claude_fijo.lock().unwrap().clone().or_else(encontrar_claude)
    }

    /// ¿Hay sesión en Claude? `None` si no se pudo saber. `claude auth status` tarda ~0,1 s y no gasta nada.
    pub fn sesion_iniciada(&self) -> Option<bool> {
        let c = self.claude()?;
        let (quitar, poner) = entorno_claude();
        let mut cmd = Command::new(c);
        cmd.args(["auth", "status"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for q in quitar {
            cmd.env_remove(q);
        }
        cmd.envs(poner);
        cmd.status().ok().map(|s| s.success())
    }

    fn matar(&self) {
        let pid = self.estado.lock().unwrap().pid;
        if let Some(p) = pid {
            sistema::matar(p, libc::SIGTERM);
        }
    }

    fn preguntar_int(&self, texto: &str, etiqueta: u64, reintento: bool) {
        let token = Arc::new(AtomicBool::new(false));
        self.estado.lock().unwrap().token = Some(token.clone());
        let avisar = |ev: Evento| {
            if !token.load(Ordering::SeqCst) {
                (self.al_evento)(etiqueta, ev);
            }
        };
        let (clave, mensaje) = self.comprobar();
        if clave != "ok" {
            avisar(Evento::Error { clave, mensaje });
            return;
        }
        let _ = std::fs::create_dir_all(&self.rutas.cerebro_cwd);
        let claude = self.claude().expect("comprobar() ya vio que existe");
        let sesion = self.sesion();
        let autorizar = self.autorizar.lock().unwrap().clone();
        let persona = format!(
            "{}{}",
            self.persona.lock().unwrap(),
            self.sentimientos.lock().unwrap()
        );
        let args = construir_comando(
            &persona,
            texto,
            sesion.as_deref(),
            self.mcp.lock().unwrap().as_deref(),
            autorizar.as_deref(),
            self.internet.load(Ordering::SeqCst),
            &self.aux.lock().unwrap(),
        );
        let (quitar, poner) = entorno_claude();
        let mut cmd = Command::new(&claude);
        cmd.args(&args).current_dir(&self.rutas.cerebro_cwd);
        for q in quitar {
            cmd.env_remove(q);
        }
        cmd.envs(poner);
        if let Some(a) = &autorizar {
            cmd.env("VORTEX_MIKU_AUTORIZAR", a); // para el hook de internet
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let Ok(mut hijo) = cmd.spawn() else {
            avisar(Evento::Error {
                clave: "activar".into(),
                mensaje: MSG_INSTALANDO.into(),
            });
            return;
        };
        self.estado.lock().unwrap().pid = Some(hijo.id());
        let errores = Arc::new(Mutex::new(String::new()));
        if let Some(mut e) = hijo.stderr.take() {
            let errores = errores.clone();
            std::thread::spawn(move || {
                let mut s = String::new();
                let _ = e.read_to_string(&mut s);
                *errores.lock().unwrap() = s;
            });
        }
        let inicio = Instant::now();
        let ultima = Arc::new(Mutex::new(Instant::now()));
        let terminado = Arc::new(AtomicBool::new(false));
        let en_acciones = Arc::new(Mutex::new(0.0f64));
        {
            let (ultima, terminado, en_acciones, ocupado, pid) = (
                ultima.clone(),
                terminado.clone(),
                en_acciones.clone(),
                self.ocupado.clone(),
                hijo.id(),
            );
            std::thread::spawn(move || {
                while !terminado.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(250));
                    let ahora = Instant::now();
                    if ocupado.load(Ordering::SeqCst) {
                        // Esperando tu permiso o corriendo una acción: no se corta a Claude.
                        *ultima.lock().unwrap() = ahora;
                        *en_acciones.lock().unwrap() += 0.25;
                        continue;
                    }
                    let sin_senal = ahora.duration_since(*ultima.lock().unwrap()).as_secs_f64();
                    let total = ahora.duration_since(inicio).as_secs_f64() - *en_acciones.lock().unwrap();
                    if sin_senal > silencio_maximo() || total > DURACION_MAXIMA {
                        sistema::matar(pid, libc::SIGKILL);
                        return;
                    }
                }
            });
        }
        let (mut resultado, mut limite, mut hubo_texto): (Option<Value>, Option<Value>, bool) =
            (None, None, false);
        let lector = BufReader::new(hijo.stdout.take().expect("stdout de claude"));
        for linea in lector.lines().map_while(Result::ok) {
            *ultima.lock().unwrap() = Instant::now();
            let Ok(ev) = serde_json::from_str::<Value>(&linea) else {
                continue;
            };
            match ev["type"].as_str() {
                Some("system") if ev["subtype"] == "init" => {
                    let mut e = self.estado.lock().unwrap();
                    e.ultimo_init = json!({"tools": ev["tools"], "mcp_servers": ev["mcp_servers"], "model": ev["model"], "permissionMode": ev["permissionMode"]});
                    if let Some(s) = ev["session_id"].as_str() {
                        e.sesion = Some(s.into());
                        e.sesiones_creadas.insert(s.into());
                    }
                    drop(e);
                    avisar(Evento::Inicio);
                }
                Some("stream_event") => {
                    let e = &ev["event"];
                    match e["type"].as_str() {
                        Some("content_block_start") if e["content_block"]["type"] == "tool_use" => {
                            avisar(Evento::Herramienta(
                                e["content_block"]["name"].as_str().unwrap_or("").into(),
                            ));
                        }
                        Some("content_block_delta") if e["delta"]["type"] == "text_delta" => {
                            hubo_texto = true;
                            avisar(Evento::Texto(e["delta"]["text"].as_str().unwrap_or("").into()));
                        }
                        Some("content_block_stop") => avisar(Evento::BloqueFin),
                        _ => {}
                    }
                }
                Some("rate_limit_event") => limite = Some(ev["rate_limit_info"].clone()),
                Some("result") => {
                    if let Some(s) = ev["session_id"].as_str() {
                        self.estado.lock().unwrap().sesion = Some(s.into());
                    }
                    resultado = Some(ev);
                }
                _ => {}
            }
        }
        let codigo = hijo.wait().ok().and_then(|s| s.code());
        terminado.store(true, Ordering::SeqCst);
        {
            let mut e = self.estado.lock().unwrap();
            e.pid = None;
        }
        if token.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(Duration::from_millis(30)); // que el hilo del stderr termine de guardar
        let stderr = errores.lock().unwrap().clone();
        let fallo = match &resultado {
            None => true,
            Some(r) => r["is_error"].as_bool().unwrap_or(false) || codigo != Some(0),
        };
        if fallo {
            let texto_r = resultado
                .as_ref()
                .and_then(|r| r["result"].as_str())
                .unwrap_or("")
                .to_string();
            if texto_r.contains("No conversation found") && self.sesion().is_some() && !reintento {
                self.estado.lock().unwrap().sesion = None; // la conversación ya no existe: empieza otra
                return self.preguntar_int(texto, etiqueta, true);
            }
            let api = resultado.as_ref().and_then(|r| r["api_error_status"].as_i64());
            let (mut clave, mut msg) = mensaje_error(&texto_r, api, limite.as_ref(), &stderr);
            if clave == "sesion" {
                self.sesion_comprobada.store(false, Ordering::SeqCst);
            }
            if resultado.is_none()
                && !hubo_texto
                && clave == "otro"
                && inicio.elapsed().as_secs_f64() >= silencio_maximo()
            {
                clave = "internet".into();
                msg = "Claude tardó demasiado en contestar. Mira tu internet y vuelve a intentarlo.".into();
            }
            avisar(Evento::Error { clave, mensaje: msg });
            return;
        }
        avisar(Evento::Listo(
            resultado
                .and_then(|r| r["result"].as_str().map(String::from))
                .unwrap_or_default(),
        ));
    }
}

/// `SILENCIO_MAXIMO`, o lo que ponga `VORTEX_MIKU_SILENCIO_MAXIMO` (las pruebas no esperan 30 s).
fn silencio_maximo() -> f64 {
    std::env::var("VORTEX_MIKU_SILENCIO_MAXIMO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SILENCIO_MAXIMO)
}

impl CerebroT for Cerebro {
    fn comprobar(&self) -> (String, String) {
        if self.revisar_activado.load(Ordering::SeqCst) && claude_activado() == Some(false) {
            self.sesion_comprobada.store(false, Ordering::SeqCst);
            return ("activar".into(), MSG_ACTIVAR.into());
        }
        if self.claude().is_none() {
            return ("activar".into(), MSG_INSTALANDO.into());
        }
        if !self.sesion_comprobada.load(Ordering::SeqCst) {
            if self.sesion_iniciada() == Some(false) {
                return ("sesion".into(), MSG_SESION.into());
            }
            self.sesion_comprobada.store(true, Ordering::SeqCst);
        }
        ("ok".into(), String::new())
    }

    fn preguntar(&self, texto: &str, etiqueta: u64) {
        self.preguntar_int(texto, etiqueta, false);
    }

    fn cancelar(&self) {
        if let Some(t) = &self.estado.lock().unwrap().token {
            t.store(true, Ordering::SeqCst);
        }
        self.matar();
    }

    fn olvidar(&self) {
        self.cancelar();
        let base = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".claude"))
            .join("projects");
        let nombre: String = self
            .rutas
            .cerebro_cwd
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let carpeta = base.join(nombre);
        let mut e = self.estado.lock().unwrap();
        for sid in e.sesiones_creadas.drain() {
            let _ = std::fs::remove_file(carpeta.join(format!("{sid}.jsonl")));
            let _ = std::fs::remove_dir_all(carpeta.join(&sid));
        }
        let _ = std::fs::remove_dir(&carpeta); // solo si quedó vacía
        e.sesion = None;
    }

    fn poner_sentimientos(&self, texto: &str) {
        *self.sentimientos.lock().unwrap() = texto.into();
    }

    fn poner_autorizar(&self, ruta: &str) {
        *self.autorizar.lock().unwrap() = Some(ruta.into());
    }

    fn poner_internet(&self, si: bool) {
        self.internet.store(si, Ordering::SeqCst);
    }

    fn ocupado(&self) -> Arc<AtomicBool> {
        self.ocupado.clone()
    }

    fn agregar_persona(&self, texto: &str) {
        let mut p = self.persona.lock().unwrap();
        p.push_str("\n\n");
        p.push_str(texto);
    }
}
