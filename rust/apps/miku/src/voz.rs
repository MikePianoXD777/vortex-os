//! La voz de Miku: el catálogo de voces, el proceso aparte que las dice y cómo suena cada frase.
//!
//! Supertonic 3 (la de fábrica) se dice en Rust (`miku-voz`, del crate `apps/miku/voz`, sobre onnxruntime);
//! ElevenLabs sigue en `miku/voz_worker.py` y la voz natural (Qwen3-TTS, PyTorch en la GPU) en
//! `miku/voz_natural.py`. Este módulo las lanza como proceso aparte con el mismo protocolo de líneas de JSON:
//!
//!   entra   {"id": 1, "texto": "Hola."}                 (o {"cmd": "salir"})
//!   sale    {"listo": true, "voz": "supertonic-f1"}     al cargar el modelo
//!           {"id": 1, "wav": ".../voz-1.wav", "seg": 1.8, "visemas": [[0,0,0,0,0], ...]}
//!           {"id": 1, "error": "texto del problema"}    (o sin id: no pudo cargar)

use crate::config::Config;
use crate::rutas::Rutas;
use crate::{descargas, sistema};
use regex::Regex;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const VOZ_DEFECTO: &str = "supertonic-f1";

// Lo que se le suma a la persona de la IA según la voz.
pub const PERSONA_ELEVENLABS: &str = "Tu voz es natural y expresiva: puedes poner etiquetas de emoción en inglés entre corchetes justo antes de la parte que afectan, por ejemplo [sighs], [whispers], [softly], [sad], [laughs softly], [nervous], [hesitates], [curious] o [excited]. Usa como mucho una o dos por respuesta y solo cuando de verdad se sienta esa emoción; no las expliques ni las escribas de otra forma. No se muestran en pantalla: solo cambian cómo suena tu voz.";
pub const PERSONA_NATURAL: &str = "Tu voz es natural y expresiva, y suena como escribes: deja que se note lo que sientes con interjecciones («ay», «mmm», «oye»), puntos suspensivos para las pausas y signos de exclamación o de pregunta cuando toque, sin exagerar. Si quieres, pon antes de una frase una etiqueta en inglés entre corchetes como [sighs], [sad], [laughs softly] o [excited]: no se lee en voz alta ni se muestra, solo cambia tu cara.";

fn etiqueta() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    // `(?!\()` (un enlace de markdown) se resuelve a mano: la crate no tiene lookahead.
    C.get_or_init(|| Regex::new(r"\[\s*([a-zA-Z][a-zA-Z ,'-]{1,30})\s*\](\s*)").unwrap())
}

/// Las etiquetas `[sighs]` del texto, que no sean el principio de un enlace `[texto](url)`.
fn buscar_etiquetas(texto: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut v = vec![];
    for c in etiqueta().captures_iter(texto) {
        let todo = c.get(0).unwrap();
        let cuerpo_fin = c.get(1).unwrap().end();
        // «[x](…)»: el paréntesis va pegado al corchete de cierre.
        let tras = texto[todo.start()..]
            .find(']')
            .map(|i| todo.start() + i + 1)
            .unwrap_or(0);
        if texto[tras..].starts_with('(') {
            continue;
        }
        let _ = cuerpo_fin;
        v.push((todo.range(), c[1].trim().to_lowercase()));
    }
    v
}

/// El texto sin [etiquetas] de emoción (para el globo, el oído y Supertonic).
pub fn quitar_etiquetas(texto: &str) -> String {
    let mut o = String::new();
    let mut desde = 0;
    for (r, _) in buscar_etiquetas(texto) {
        o += &texto[desde..r.start];
        desde = r.end;
    }
    o += &texto[desde..];
    static ESP: OnceLock<Regex> = OnceLock::new();
    ESP.get_or_init(|| Regex::new(r"\s{2,}").unwrap())
        .replace_all(&o, " ")
        .trim()
        .to_string()
}

pub fn etiquetas_de(texto: &str) -> Vec<String> {
    buscar_etiquetas(texto).into_iter().map(|(_, e)| e).collect()
}

const TRISTES: [&str; 6] = ["sighs", "sad", "exhales", "crying", "softly", "whispers"];
const ALEGRES: [&str; 5] = ["laughs", "laughs softly", "giggles", "excited", "happy"];
const SORPRESA: [&str; 4] = ["surprised", "curious", "nervous", "hesitates"];

fn re_triste() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| {
        Regex::new(r"(?i)\b(lo siento|perd[oó]n|no puedo|no pude|error|problema|fall[oó]|no s[eé])\b")
            .unwrap()
    })
}

fn re_alegre() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| {
        Regex::new(
            r"(?i)[!¡]|\b(genial|qué bien|me alegra|me encanta|me gusta|claro que s[ií]|listo|de nada)\b",
        )
        .unwrap()
    })
}

/// Una emoción suave para la frase que va a decir (Miku es apagada: nunca pasa de una sonrisa leve). Si la
/// frase trae etiquetas de ElevenLabs ([sighs], [laughs softly]...), mandan ellas.
pub fn emocion_de(frase: &str) -> (&'static str, f64) {
    let e = etiquetas_de(frase);
    let hay = |l: &[&str]| e.iter().any(|x| l.contains(&x.as_str()));
    if hay(&ALEGRES) {
        return ("happy", 0.45);
    }
    if hay(&SORPRESA) {
        return ("surprised", 0.3);
    }
    if hay(&TRISTES) {
        return ("sad", 0.4);
    }
    let f = quitar_etiquetas(frase);
    if re_triste().is_match(&f) {
        return ("sad", 0.3);
    }
    if re_alegre().is_match(&f) {
        return ("happy", 0.35);
    }
    ("relaxed", 0.3)
}

/// Programa que suena un WAV: `pw-play` (PipeWire) y, si falta, `paplay`, `aplay` o `ffplay`.
pub fn reproductor() -> Option<Vec<String>> {
    [
        vec!["pw-play"],
        vec!["paplay"],
        vec!["aplay", "-q"],
        vec!["ffplay", "-nodisp", "-autoexit", "-loglevel", "quiet"],
    ]
    .into_iter()
    .find(|c| sistema::cual(c[0]).is_some())
    .map(|c| c.into_iter().map(String::from).collect())
}

// ---- El catálogo ------------------------------------------------------------------------------------

/// Las voces de `voces.json`. Si falta o está roto, solo la de fábrica.
pub fn catalogo(rutas: &Rutas) -> Vec<Value> {
    let leidas = std::fs::read_to_string(&rutas.catalogo_voces)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|d| d["voces"].as_array().cloned())
        .map(|v| {
            v.into_iter()
                .filter(|x| x.is_object() && x["id"].as_str().is_some_and(|i| !i.is_empty()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if leidas.is_empty() {
        return vec![
            json!({"id": VOZ_DEFECTO, "nombre": "Miku (F1)", "motor": "supertonic", "estilo_voz": "F1", "incluida": true}),
        ];
    }
    leidas
}

pub fn buscar(rutas: &Rutas, id: &str) -> Option<Value> {
    catalogo(rutas).into_iter().find(|v| v["id"] == id)
}

pub fn archivo_estilo(rutas: &Rutas, voz: &Value) -> PathBuf {
    rutas
        .voz
        .join("voice_styles")
        .join(format!("{}.json", voz["estilo_voz"].as_str().unwrap_or("F1")))
}

/// La clave de ElevenLabs: la variable, el llavero (`secret-tool`) o `~/.config/vortex/elevenlabs.clave`.
pub fn leer_clave_elevenlabs(rutas: &Rutas) -> String {
    if let Some(c) = std::env::var("ELEVENLABS_API_KEY")
        .ok()
        .filter(|c| !c.trim().is_empty())
    {
        return c.trim().into();
    }
    if sistema::cual("secret-tool").is_some()
        && let Ok(r) = Command::new("secret-tool")
            .args(["lookup", "service", "vortex-miku", "cuenta", "elevenlabs"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        && r.status.success()
        && !String::from_utf8_lossy(&r.stdout).trim().is_empty()
    {
        return String::from_utf8_lossy(&r.stdout).trim().into();
    }
    std::fs::read_to_string(rutas.config_dir.join("elevenlabs.clave"))
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

/// ¿Se puede hablar ya con esta voz? Supertonic necesita el modelo y su estilo; ElevenLabs, clave y Voice ID;
/// la natural, su entorno de Python.
pub fn disponible(rutas: &Rutas, cfg: &Config, voz: &Value) -> bool {
    match voz["motor"].as_str() {
        Some("elevenlabs") => !cfg.elevenlabs_voz.is_empty() && !leer_clave_elevenlabs(rutas).is_empty(),
        Some("qwen") => rutas.natural_listo().is_file() && rutas.natural_python().is_file(),
        _ => descargas::paquete_listo(rutas, "voz") && archivo_estilo(rutas, voz).is_file(),
    }
}

/// Dónde corre la voz natural: «gpu» (PyTorch en una NVIDIA serie 20+), «cuda» (qwentts.cpp en una NVIDIA
/// serie 10+, la misma voz) o «cpu» (lenta). Lo guarda «vortex-miku natural instalar».
pub fn modo_natural(rutas: &Rutas) -> String {
    match std::fs::read_to_string(rutas.natural_modo()).map(|t| t.trim().to_string()) {
        Ok(m) if ["gpu", "cuda", "cpu"].contains(&m.as_str()) => m,
        _ => "gpu".into(),
    }
}

// ---- El proceso de la voz ---------------------------------------------------------------------------

/// Lo que cuenta el proceso de la voz.
#[derive(Debug, Clone, PartialEq)]
pub enum MensajeVoz {
    Listo {
        modo: String,
    },
    Wav {
        id: u64,
        wav: String,
        seg: f64,
        visemas: Vec<Vec<i64>>,
        aviso: String,
    },
    /// No pudo cargar el modelo (sin `id`).
    ErrorCarga(String),
    /// Una frase falló (con `id`).
    ErrorFrase {
        id: u64,
        mensaje: String,
    },
    /// El proceso terminó.
    Salio,
}

pub fn leer_mensaje(linea: &str) -> Option<MensajeVoz> {
    let m: Value = serde_json::from_str(linea.trim()).ok()?;
    if m["listo"].as_bool() == Some(true) {
        return Some(MensajeVoz::Listo {
            modo: m["modo"].as_str().unwrap_or("").into(),
        });
    }
    let id = m["id"].as_u64();
    if m.get("wav").is_some() {
        return Some(MensajeVoz::Wav {
            id: id.unwrap_or(0),
            wav: m["wav"].as_str()?.into(),
            seg: m["seg"].as_f64().unwrap_or(0.0),
            visemas: m["visemas"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .map(|c| {
                            c.as_array()
                                .map(|x| x.iter().filter_map(Value::as_i64).collect())
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .unwrap_or_default(),
            aviso: m["aviso"].as_str().unwrap_or("").into(),
        });
    }
    if let Some(e) = m["error"].as_str().filter(|e| !e.is_empty()) {
        return Some(match id {
            None => MensajeVoz::ErrorCarga(e.into()),
            Some(id) => MensajeVoz::ErrorFrase {
                id,
                mensaje: e.into(),
            },
        });
    }
    None
}

/// La voz en Rust (`miku-voz`, Supertonic sobre onnxruntime), si está: `VORTEX_MIKU_VOZ`, junto a este
/// binario o en `/usr/lib/vortex`. `VORTEX_MIKU_VOZ_PYTHON=1` vuelve a la de Python (`miku/voz_worker.py`).
pub fn voz_rust() -> Option<PathBuf> {
    if std::env::var_os("VORTEX_MIKU_VOZ_PYTHON").is_some_and(|v| !v.is_empty()) {
        return None;
    }
    if let Some(p) = std::env::var_os("VORTEX_MIKU_VOZ").filter(|v| !v.is_empty()) {
        return Some(p.into());
    }
    let junto = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("miku-voz")));
    [junto, Some(PathBuf::from("/usr/lib/vortex/miku-voz"))]
        .into_iter()
        .flatten()
        .find(|p| sistema::es_ejecutable(p))
}

/// Cómo se lanza la voz `id`: (programa, argumentos, variables de entorno).
pub fn comando_de_voz(
    rutas: &Rutas,
    voz: &Value,
    avatar: bool,
) -> (PathBuf, Vec<String>, Vec<(String, String)>) {
    comando_con(rutas, voz, avatar, voz_rust())
}

/// Como `comando_de_voz`, con la voz de Rust que se le diga (`None` = la de Python).
pub fn comando_con(
    rutas: &Rutas,
    voz: &Value,
    avatar: bool,
    rust: Option<PathBuf>,
) -> (PathBuf, Vec<String>, Vec<(String, String)>) {
    let id = voz["id"].as_str().unwrap_or(VOZ_DEFECTO).to_string();
    if voz["motor"].as_str() == Some("qwen") {
        // La voz natural corre con el Python de su entorno (PyTorch y Qwen3-TTS no están en el sistema).
        let script = rutas.raiz.join("miku/voz_natural.py");
        return (
            rutas.natural_python(),
            vec!["-I".into(), script.to_string_lossy().into(), "--voz".into(), id],
            vec![("HF_HUB_OFFLINE".into(), "1".into())],
        );
    }
    let estilo = if avatar { "miku" } else { "normal" };
    // Supertonic: en Rust si está. ElevenLabs (por internet) sigue en voz_worker.py.
    if let Some(b) = rust.filter(|_| voz["motor"].as_str() != Some("elevenlabs")) {
        return (
            b,
            vec!["--voz".into(), id, "--estilo".into(), estilo.into()],
            vec![],
        );
    }
    let script = rutas.raiz.join("miku/voz_worker.py");
    let python = sistema::cual("python3").unwrap_or_else(|| "python3".into());
    (
        python,
        vec![
            "-I".into(),
            script.to_string_lossy().into(),
            "--voz".into(),
            id,
            "--estilo".into(),
            estilo.into(),
        ],
        vec![],
    )
}

/// El proceso de la voz en marcha: se le pide una frase con `decir` y lo que conteste llega a `al_mensaje`.
pub struct ProcesoVoz {
    hijo: Mutex<Child>,
    entrada: Mutex<Option<ChildStdin>>,
    cerrado: Arc<AtomicBool>,
    pub id_voz: String,
}

impl ProcesoVoz {
    pub fn lanzar(
        programa: &std::path::Path,
        args: &[String],
        env: &[(String, String)],
        id_voz: &str,
        al_mensaje: Arc<dyn Fn(MensajeVoz) + Send + Sync>,
    ) -> std::io::Result<Self> {
        let mut c = Command::new(programa);
        c.args(args)
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut h = c.spawn()?;
        let salida = h.stdout.take().unwrap();
        let entrada = h.stdin.take();
        let cerrado = Arc::new(AtomicBool::new(false));
        let aviso = al_mensaje.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(salida).lines().map_while(Result::ok) {
                if let Some(m) = leer_mensaje(&l) {
                    aviso(m);
                }
            }
            al_mensaje(MensajeVoz::Salio);
        });
        Ok(Self {
            hijo: Mutex::new(h),
            entrada: Mutex::new(entrada),
            cerrado,
            id_voz: id_voz.into(),
        })
    }

    /// Manda una frase a la voz. Suena cuando le toque.
    pub fn decir(&self, id: u64, texto: &str) {
        if let Some(e) = self.entrada.lock().unwrap().as_mut() {
            let _ = writeln!(e, "{}", json!({"id": id, "texto": texto}));
            let _ = e.flush();
        }
    }

    pub fn vivo(&self) -> bool {
        !self.cerrado.load(Ordering::SeqCst) && matches!(self.hijo.lock().unwrap().try_wait(), Ok(None))
    }

    /// Se va sin esperar a nadie (cambiar de voz).
    pub fn matar(&self) {
        self.cerrado.store(true, Ordering::SeqCst);
        let _ = self.hijo.lock().unwrap().kill();
        let _ = self.hijo.lock().unwrap().wait();
    }

    /// Le dice que salga y, si no lo hace en 1,5 s, lo termina.
    pub fn cerrar(&self) {
        self.cerrado.store(true, Ordering::SeqCst);
        if let Some(mut e) = self.entrada.lock().unwrap().take() {
            let _ = writeln!(e, "{}", json!({"cmd": "salir"}));
        }
        let mut h = self.hijo.lock().unwrap();
        let limite = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < limite {
            if matches!(h.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        sistema::matar(h.id(), 15);
        let limite = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < limite {
            if matches!(h.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = h.kill();
        let _ = h.wait();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_etiquetas_no_se_ven_ni_se_leen() {
        assert_eq!(
            quitar_etiquetas("[sighs] Ay, qué cansada estoy [softly] hoy."),
            "Ay, qué cansada estoy hoy."
        );
        assert_eq!(
            quitar_etiquetas("Mira [este enlace](https://x.org) ahora"),
            "Mira [este enlace](https://x.org) ahora",
            "un enlace no es etiqueta"
        );
        assert_eq!(
            etiquetas_de("[Laughs Softly] jeje [x]"),
            vec!["laughs softly".to_string()]
        );
        assert_eq!(quitar_etiquetas("sin nada"), "sin nada");
    }

    #[test]
    fn la_cara_de_cada_frase() {
        assert_eq!(emocion_de("[excited] ¡Lo logramos!"), ("happy", 0.45));
        assert_eq!(emocion_de("[curious] ¿Y eso?"), ("surprised", 0.3));
        assert_eq!(emocion_de("[sighs] bueno"), ("sad", 0.4));
        assert_eq!(emocion_de("Lo siento, no pude abrirlo."), ("sad", 0.3));
        assert_eq!(emocion_de("¡Listo!"), ("happy", 0.35));
        assert_eq!(emocion_de("Son las tres."), ("relaxed", 0.3));
    }

    #[test]
    fn el_catalogo_y_lo_disponible() {
        let d = tempfile::tempdir().unwrap();
        let r = Rutas::en(d.path());
        // Sin voces.json: solo la de fábrica.
        let c = catalogo(&r);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0]["id"], VOZ_DEFECTO);
        std::fs::create_dir_all(&r.raiz).unwrap();
        std::fs::write(&r.catalogo_voces, r#"{"voces": [{"id": "a", "motor": "supertonic", "estilo_voz": "F2"}, {"id": "n", "motor": "qwen"}, {"id": "e", "motor": "elevenlabs"}, 5]}"#).unwrap();
        assert_eq!(catalogo(&r).len(), 3);
        let cfg = Config::default();
        let a = buscar(&r, "a").unwrap();
        assert!(!disponible(&r, &cfg, &a), "sin el modelo no");
        assert!(!disponible(&r, &cfg, &buscar(&r, "n").unwrap()));
        assert!(buscar(&r, "no").is_none());
        // La natural, con su entorno puesto.
        std::fs::create_dir_all(r.natural_python().parent().unwrap()).unwrap();
        std::fs::write(r.natural_python(), "").unwrap();
        std::fs::write(r.natural_listo(), "").unwrap();
        assert!(disponible(&r, &cfg, &buscar(&r, "n").unwrap()));
        assert_eq!(modo_natural(&r), "gpu");
        std::fs::write(r.natural_modo(), "cpu\n").unwrap();
        assert_eq!(modo_natural(&r), "cpu");
        // ElevenLabs necesita el Voice ID y la clave.
        let e = buscar(&r, "e").unwrap();
        std::fs::create_dir_all(&r.config_dir).unwrap();
        std::fs::write(r.config_dir.join("elevenlabs.clave"), "clave-de-mentira\n").unwrap();
        assert!(!disponible(&r, &cfg, &e));
        assert!(disponible(
            &r,
            &Config {
                elevenlabs_voz: "abc".into(),
                ..cfg
            },
            &e
        ));
    }

    #[test]
    fn el_protocolo_del_proceso() {
        assert_eq!(
            leer_mensaje(r#"{"listo": true, "voz": "x", "modo": "cpu"}"#),
            Some(MensajeVoz::Listo { modo: "cpu".into() })
        );
        assert_eq!(
            leer_mensaje(
                r#"{"id": 3, "wav": "/a.wav", "seg": 1.5, "visemas": [[0,1,2,3,4],[5,6,7,8,9]], "aviso": "uy"}"#
            ),
            Some(MensajeVoz::Wav {
                id: 3,
                wav: "/a.wav".into(),
                seg: 1.5,
                visemas: vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]],
                aviso: "uy".into()
            })
        );
        assert_eq!(
            leer_mensaje(r#"{"error": "no hay modelo"}"#),
            Some(MensajeVoz::ErrorCarga("no hay modelo".into()))
        );
        assert_eq!(
            leer_mensaje(r#"{"id": 2, "error": "mala frase"}"#),
            Some(MensajeVoz::ErrorFrase {
                id: 2,
                mensaje: "mala frase".into()
            })
        );
        assert_eq!(leer_mensaje("basura"), None);
    }

    #[test]
    fn como_se_lanza_cada_voz() {
        let d = tempfile::tempdir().unwrap();
        let r = Rutas::en(d.path());
        let (_, args, env) = comando_con(
            &r,
            &json!({"id": "supertonic-f1", "motor": "supertonic"}),
            true,
            None,
        );
        assert_eq!(&args[0], "-I");
        assert!(args[1].ends_with("miku/voz_worker.py"));
        assert_eq!(&args[2..], ["--voz", "supertonic-f1", "--estilo", "miku"]);
        assert!(env.is_empty());
        let (_, args, _) = comando_con(&r, &json!({"id": "supertonic-f2"}), false, None);
        assert_eq!(args.last().unwrap(), "normal");
        // Con la voz de Rust: Supertonic va en Rust; ElevenLabs y la natural, no.
        let rs = Some(PathBuf::from("/x/miku-voz"));
        let (b, args, env) = comando_con(
            &r,
            &json!({"id": "supertonic-f2", "motor": "supertonic"}),
            true,
            rs.clone(),
        );
        assert_eq!(b, PathBuf::from("/x/miku-voz"));
        assert_eq!(args, ["--voz", "supertonic-f2", "--estilo", "miku"]);
        assert!(env.is_empty());
        let (b, args, _) = comando_con(&r, &json!({"id": "e", "motor": "elevenlabs"}), true, rs.clone());
        assert_ne!(b, PathBuf::from("/x/miku-voz"));
        assert!(args[1].ends_with("miku/voz_worker.py"));
        let (py, args, env) = comando_con(&r, &json!({"id": "miku-natural", "motor": "qwen"}), true, rs);
        assert_eq!(py, r.natural_python());
        assert!(args[1].ends_with("miku/voz_natural.py"));
        assert_eq!(env, vec![("HF_HUB_OFFLINE".to_string(), "1".to_string())]);
    }

    #[test]
    fn habla_con_un_proceso_de_mentira() {
        let d = tempfile::tempdir().unwrap();
        let guion = d.path().join("voz.py");
        std::fs::write(&guion, r#"
import json, sys
print(json.dumps({"listo": True, "voz": "x"}), flush=True)
for l in sys.stdin:
    p = json.loads(l)
    if p.get("cmd") == "salir":
        break
    if "malo" in p["texto"]:
        print(json.dumps({"id": p["id"], "error": "no pude"}), flush=True)
    else:
        print(json.dumps({"id": p["id"], "wav": "/tmp/" + p["texto"] + ".wav", "seg": 0.5, "visemas": [[1,2,3,4,5]]}), flush=True)
"#).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        let p = ProcesoVoz::lanzar(
            &sistema::cual("python3").unwrap(),
            &["-I".into(), guion.to_string_lossy().into()],
            &[],
            "x",
            Arc::new(move |m| {
                let _ = tx.lock().unwrap().send(m);
            }),
        )
        .unwrap();
        let t = Duration::from_secs(10);
        assert_eq!(
            rx.recv_timeout(t).unwrap(),
            MensajeVoz::Listo { modo: String::new() }
        );
        p.decir(1, "hola");
        p.decir(2, "malo");
        assert!(matches!(rx.recv_timeout(t).unwrap(), MensajeVoz::Wav { id: 1, seg, .. } if seg == 0.5));
        assert_eq!(
            rx.recv_timeout(t).unwrap(),
            MensajeVoz::ErrorFrase {
                id: 2,
                mensaje: "no pude".into()
            }
        );
        assert!(p.vivo());
        p.cerrar();
        assert_eq!(rx.recv_timeout(t).unwrap(), MensajeVoz::Salio);
        assert!(!p.vivo());
    }
}
