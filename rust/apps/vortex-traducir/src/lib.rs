//! vortex-traducir (4.0): traduce lo que tienes seleccionado, en cualquier app, con la IA local (Ollama, la
//! misma de Vortexine's Local AI y de Miku). Super+Alt+T: lo seleccionado (o, si no hay, lo copiado) sale
//! traducido en un aviso con «Copiar». Si está en español va al inglés; si no, al español. Sin internet.
//!
//! Elige el modelo que pusiste en Local AI o, si no, el más chico que tengas (para que tarde poco), y lo
//! suelta de la memoria a los 2 min. Si Ollama no corre lo arranca (`vortex-ollama.service`).
//!
//!   vortex-traducir            traduce la selección y avisa
//!   vortex-traducir TEXTO…     traduce ese texto y lo imprime (sin avisos)

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const APP: &str = "Traducir";
/// Lo más largo que se traduce de una vez (lo demás se corta con «…»).
pub const LIMITE: usize = 4000;

/// A qué idioma va.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    Espanol,
    Ingles,
}

impl Destino {
    pub fn titulo(self) -> &'static str {
        match self {
            Destino::Espanol => "Traducción al español",
            Destino::Ingles => "Traducción al inglés",
        }
    }
    fn idioma(self) -> &'static str {
        match self {
            Destino::Espanol => "español",
            Destino::Ingles => "inglés",
        }
    }
}

const PALABRAS_ES: &[&str] = &[
    "de", "la", "que", "el", "en", "y", "los", "se", "del", "las", "un", "por", "con", "no", "una", "su",
    "para", "es", "al", "lo", "como", "más", "pero", "sus", "le", "ya", "o", "este", "sí", "porque", "esta",
    "entre", "cuando", "muy", "sin", "sobre", "también", "me", "hasta", "hay", "donde", "quien", "desde",
    "todo", "nos", "qué", "yo", "tu", "tú", "mi", "esto", "eso", "está", "están", "son", "fue", "ser",
    "tiene", "hola", "gracias",
];
const PALABRAS_EN: &[&str] = &[
    "the", "and", "of", "to", "is", "in", "that", "it", "you", "for", "with", "on", "are", "this", "be",
    "was", "have", "not", "but", "what", "can", "your", "at", "from", "they", "we", "an", "will", "my",
    "all", "there", "if", "do", "so", "about", "which", "would", "has", "been", "their", "were", "how",
    "when", "i", "hello",
];

/// Español → inglés; cualquier otro idioma → español. Cuenta palabras comunes de cada idioma (y la ñ, las
/// tildes y los signos de apertura, que solo tiene el español).
pub fn destino_de(texto: &str) -> Destino {
    let minus = texto.to_lowercase();
    let (mut es, mut en, mut palabras) = (0usize, 0usize, 0usize);
    for p in minus
        .split(|c: char| !c.is_alphabetic())
        .filter(|p| !p.is_empty())
    {
        palabras += 1;
        es += usize::from(PALABRAS_ES.contains(&p));
        en += usize::from(PALABRAS_EN.contains(&p));
    }
    es += minus.chars().filter(|c| "ñ¿¡áéíóú".contains(*c)).count() * 2;
    // Que haya bastantes (el francés o el italiano también dicen «que», «tu», «de»).
    if es > en && es * 3 >= palabras {
        Destino::Ingles
    } else {
        Destino::Espanol
    }
}

/// Lo que se manda: sin espacios de más y como mucho [`LIMITE`] caracteres.
pub fn preparar(texto: &str) -> String {
    let t = texto.trim();
    if t.chars().count() <= LIMITE {
        return t.to_string();
    }
    let mut s: String = t.chars().take(LIMITE).collect();
    s.push('…');
    s
}

/// ¿Es un modelo de embeddings (no sabe conversar)?
pub fn es_embeddings(nombre: &str) -> bool {
    let n = nombre.to_lowercase();
    [
        "embed",
        "bge-",
        "bge:",
        "minilm",
        "nomic",
        "snowflake-arctic",
        "mxbai",
        "granite-embedding",
    ]
    .iter()
    .any(|e| n.contains(e))
}

/// El modelo: el preferido si lo tienes; si no, el más chico de los que conversan (tarda menos).
pub fn elegir_modelo(tags: &Value, preferido: Option<&str>) -> Option<String> {
    let lista = tags.get("models")?.as_array()?;
    let nombres: Vec<(&str, u64)> = lista
        .iter()
        .filter_map(|m| {
            let n = m.get("name").or_else(|| m.get("model"))?.as_str()?;
            Some((n, m.get("size").and_then(Value::as_u64).unwrap_or(u64::MAX)))
        })
        .filter(|(n, _)| !es_embeddings(n))
        .collect();
    if let Some(p) = preferido.filter(|p| !p.is_empty())
        && let Some((n, _)) = nombres
            .iter()
            .find(|(n, _)| *n == p || n.strip_suffix(":latest") == Some(p))
    {
        return Some((*n).to_string());
    }
    nombres
        .iter()
        .min_by_key(|(_, b)| *b)
        .map(|(n, _)| (*n).to_string())
}

/// La petición a `/api/chat` (sin streaming, sin «pensar», y que lo suelte de la memoria a los 2 min).
pub fn peticion(modelo: &str, texto: &str, a: Destino, pensar: bool) -> Value {
    let sistema = format!(
        "Eres un traductor. Traduce al {} el texto que te den, con naturalidad y conservando el formato \
         (saltos de línea, listas, nombres propios). Responde SOLO con la traducción: sin comillas, sin notas, \
         sin explicar nada.",
        a.idioma()
    );
    let mut p = json!({
        "model": modelo,
        "stream": false,
        "keep_alive": "2m",
        "options": {"temperature": 0.2},
        "messages": [{"role": "system", "content": sistema}, {"role": "user", "content": texto}],
    });
    if !pensar {
        p["think"] = json!(false);
    }
    p
}

/// La respuesta sin el pensamiento (`<think>…</think>`), sin espacios ni comillas alrededor.
pub fn limpiar(respuesta: &str) -> String {
    let mut s = respuesta.to_string();
    while let (Some(a), Some(b)) = (s.find("<think>"), s.find("</think>")) {
        if b < a {
            break;
        }
        s.replace_range(a..b + "</think>".len(), "");
    }
    let s = s.trim();
    let pares = [('"', '"'), ('“', '”'), ('«', '»'), ('\'', '\'')];
    for (i, f) in pares {
        if let Some(x) = s.strip_prefix(i).and_then(|x| x.strip_suffix(f))
            && !x.contains(i)
        {
            return x.trim().to_string();
        }
    }
    s.to_string()
}

/// Para el cuerpo de un aviso (el panel de avisos entiende un poco de HTML).
pub fn escapar(texto: &str) -> String {
    texto
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ====================================================================== Ollama por HTTP (con curl)

pub fn url_ollama() -> String {
    ["VORTEX_TRADUCIR_OLLAMA", "VORTEX_LOCAL_AI_OLLAMA"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|u| !u.is_empty()))
        .unwrap_or_else(|| "http://127.0.0.1:11434".into())
        .trim_end_matches('/')
        .to_string()
}

/// GET o POST (si hay cuerpo) con curl, que siempre está; el cuerpo va por la entrada estándar.
fn curl(url: &str, cuerpo: Option<&str>, segundos: u32) -> Result<String, String> {
    let mut c = Command::new("curl");
    c.args([
        "-sS",
        "--max-time",
        &segundos.to_string(),
        "-H",
        "Content-Type: application/json",
        "-H",
        "Expect:",
    ]);
    if cuerpo.is_some() {
        c.args(["--data-binary", "@-"]);
    }
    c.arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut h = c.spawn().map_err(|e| format!("no pude correr curl: {e}"))?;
    if let (Some(cuerpo), Some(mut e)) = (cuerpo, h.stdin.take()) {
        let _ = e.write_all(cuerpo.as_bytes());
    }
    let o = h.wait_with_output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Los modelos que tienes (`/api/tags`), o `None` si Ollama no contesta.
pub fn modelos(url: &str) -> Option<Value> {
    serde_json::from_str(&curl(&format!("{url}/api/tags"), None, 2).ok()?).ok()
}

/// Traduce con Ollama. Si el modelo no acepta `think`, lo vuelve a pedir sin él.
pub fn traducir(url: &str, modelo: &str, texto: &str, a: Destino) -> Result<String, String> {
    let mut pensar = false;
    loop {
        let cuerpo = peticion(modelo, texto, a, pensar).to_string();
        let r = curl(&format!("{url}/api/chat"), Some(&cuerpo), 180)?;
        let v: Value =
            serde_json::from_str(&r).map_err(|_| "Ollama respondió algo que no entiendo".to_string())?;
        if let Some(e) = v.get("error").and_then(Value::as_str) {
            if !pensar && e.to_lowercase().contains("think") {
                pensar = true;
                continue;
            }
            return Err(e.to_string());
        }
        let t = limpiar(
            v.pointer("/message/content")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        return if t.is_empty() {
            Err("el modelo no respondió nada".into())
        } else {
            Ok(t)
        };
    }
}

/// El modelo que elegiste en Vortexine's Local AI (su config), si hay.
fn modelo_de_local_ai() -> Option<String> {
    // `rutas::datos()` es `~/.local/share/vortex`: la de Local AI está al lado.
    let datos = vortex_core::rutas::datos();
    let c = datos
        .parent()
        .unwrap_or(&datos)
        .join("vortex-local-ai/config.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(c).ok()?).ok()?;
    ["modelo", "modelo_chat", "ultimo_modelo"]
        .iter()
        .find_map(|k| v.get(*k)?.as_str().map(String::from))
}

/// Que Ollama conteste: si no, lo arranca (la unidad de Miku o `ollama serve`) y espera hasta 20 s.
fn asegurar_ollama(url: &str) -> Option<Value> {
    if let Some(t) = modelos(url) {
        return Some(t);
    }
    let local = url.contains("127.0.0.1") || url.contains("localhost");
    if !local || !en_path("ollama") {
        return None;
    }
    let unidad = Command::new("systemctl")
        .args(["--user", "start", "vortex-ollama.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !unidad {
        let _ = Command::new("setsid")
            .args(["-f", "ollama", "serve"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let fin = Instant::now() + Duration::from_secs(20);
    while Instant::now() < fin {
        std::thread::sleep(Duration::from_millis(500));
        if let Some(t) = modelos(url) {
            return Some(t);
        }
    }
    None
}

fn en_path(programa: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(programa).is_file()))
}

// ====================================================================== la selección y los avisos

fn salida(programa: &str, args: &[&str]) -> String {
    Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Lo seleccionado (selección primaria) o, si no hay, lo copiado.
fn seleccion() -> String {
    let p = salida("wl-paste", &["--primary", "--no-newline", "--type", "text"]);
    if !p.trim().is_empty() {
        return p;
    }
    salida("wl-paste", &["--no-newline", "--type", "text"])
}

/// Cierra un aviso por su id (el panel de avisos del shell no redibuja uno reemplazado con `-r`: se cierra
/// el anterior y sale otro).
pub fn cerrar_aviso(id: &str) {
    let _ = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.Notifications",
            "--object-path",
            "/org/freedesktop/Notifications",
            "--method",
            "org.freedesktop.Notifications.CloseNotification",
            id,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Un aviso; con `reemplaza` cierra antes ese otro. Devuelve su id (`notify-send -p`).
fn avisar(titulo: &str, cuerpo: &str, reemplaza: Option<&str>) -> Option<String> {
    if let Some(id) = reemplaza {
        cerrar_aviso(id);
    }
    let mut c = Command::new("notify-send");
    c.args(["-a", APP, "-i", "accessories-dictionary", "-p"]);
    c.args([titulo, cuerpo]);
    let id = salida_de(c);
    (!id.trim().is_empty()).then(|| id.trim().to_string())
}

fn salida_de(mut c: Command) -> String {
    c.stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// El aviso con la traducción y «Copiar» (espera a que lo cierres o lo uses).
fn ensenar(titulo: &str, traduccion: &str, reemplaza: Option<&str>) {
    let mut c = Command::new("notify-send");
    c.args(["-a", APP, "-i", "accessories-dictionary", "-A", "copiar=Copiar"]);
    if let Some(id) = reemplaza {
        cerrar_aviso(id);
    }
    c.args([titulo, &escapar(traduccion)]);
    if salida_de(c).trim() == "copiar"
        && let Ok(mut h) = Command::new("wl-copy").stdin(Stdio::piped()).spawn()
    {
        if let Some(mut e) = h.stdin.take() {
            let _ = e.write_all(traduccion.as_bytes());
        }
        let _ = h.wait();
    }
}

pub fn principal() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let url = url_ollama();
    // Con texto en los argumentos: solo imprime (para la terminal y las pruebas).
    if !args.is_empty() {
        let texto = preparar(&args.join(" "));
        let Some(tags) = asegurar_ollama(&url) else {
            eprintln!("Ollama no contesta: abre Vortexine's Local AI.");
            return 1;
        };
        let Some(modelo) = elegir_modelo(&tags, modelo_de_local_ai().as_deref()) else {
            eprintln!("No tienes ningún modelo: baja uno en Vortexine's Local AI.");
            return 1;
        };
        return match traducir(&url, &modelo, &texto, destino_de(&texto)) {
            Ok(t) => {
                println!("{t}");
                0
            }
            Err(e) => {
                eprintln!("No pude traducir: {e}");
                1
            }
        };
    }

    let texto = preparar(&seleccion());
    if texto.is_empty() {
        avisar(
            "Selecciona un texto",
            "Selecciona (o copia) lo que quieras traducir y aprieta Super+Alt+T.",
            None,
        );
        return 1;
    }
    let a = destino_de(&texto);
    let id = avisar(
        &format!("{}…", a.titulo()),
        "Un momento: la IA local lo está traduciendo.",
        None,
    );
    let Some(tags) = asegurar_ollama(&url) else {
        avisar(
            "Para traducir hace falta la IA local",
            "Abre Vortexine's Local AI y baja un modelo (con uno chico basta). Todo se queda en tu PC.",
            id.as_deref(),
        );
        return 1;
    };
    let Some(modelo) = elegir_modelo(&tags, modelo_de_local_ai().as_deref()) else {
        avisar(
            "Para traducir hace falta un modelo",
            "Abre Vortexine's Local AI y baja uno (con uno chico basta). Todo se queda en tu PC.",
            id.as_deref(),
        );
        return 1;
    };
    match traducir(&url, &modelo, &texto, a) {
        Ok(t) => {
            ensenar(a.titulo(), &t, id.as_deref());
            0
        }
        Err(e) => {
            avisar("No pude traducir", &escapar(&e), id.as_deref());
            1
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn a_que_idioma() {
        assert_eq!(
            destino_de("Hola, ¿cómo estás? Mañana vamos al cine"),
            Destino::Ingles
        );
        assert_eq!(
            destino_de("The quick brown fox jumps over the lazy dog"),
            Destino::Espanol
        );
        assert_eq!(destino_de("Je ne sais pas ce que tu veux dire"), Destino::Espanol);
        assert_eq!(destino_de("Que es esto"), Destino::Ingles);
        assert_eq!(destino_de("What is this"), Destino::Espanol);
        assert_eq!(destino_de(""), Destino::Espanol);
    }

    #[test]
    fn el_modelo() {
        let tags = json!({"models": [
            {"name": "nomic-embed-text:latest", "size": 270_000_000u64},
            {"name": "qwen3:8b", "size": 5_200_000_000u64},
            {"name": "llama3.2:3b", "size": 2_000_000_000u64},
        ]});
        assert_eq!(elegir_modelo(&tags, None).as_deref(), Some("llama3.2:3b"));
        assert_eq!(
            elegir_modelo(&tags, Some("qwen3:8b")).as_deref(),
            Some("qwen3:8b")
        );
        assert_eq!(
            elegir_modelo(&tags, Some("no-lo-tengo")).as_deref(),
            Some("llama3.2:3b")
        );
        let solo = json!({"models": [{"name": "gemma3:latest", "size": 3u64}]});
        assert_eq!(
            elegir_modelo(&solo, Some("gemma3")).as_deref(),
            Some("gemma3:latest")
        );
        assert_eq!(
            elegir_modelo(&json!({"models": [{"name": "bge-m3:latest"}]}), None),
            None
        );
        assert_eq!(elegir_modelo(&json!({}), None), None);
    }

    #[test]
    fn la_respuesta() {
        assert_eq!(
            limpiar("<think>\nlo pienso\n</think>\n\nHello there"),
            "Hello there"
        );
        assert_eq!(limpiar("  «Hola mundo»  "), "Hola mundo");
        assert_eq!(limpiar("\"Hi\""), "Hi");
        assert_eq!(limpiar("\"a\" y \"b\""), "\"a\" y \"b\"");
        assert_eq!(limpiar("línea 1\nlínea 2"), "línea 1\nlínea 2");
    }

    #[test]
    fn la_peticion() {
        let p = peticion("m", "hola", Destino::Ingles, false);
        assert_eq!(p["model"], "m");
        assert_eq!(p["stream"], false);
        assert_eq!(p["think"], false);
        assert!(
            p["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("al inglés")
        );
        assert_eq!(p["messages"][1]["content"], "hola");
        assert!(peticion("m", "x", Destino::Espanol, true).get("think").is_none());
    }

    #[test]
    fn corta_y_escapa() {
        let largo = "a".repeat(LIMITE + 10);
        assert_eq!(preparar(&largo).chars().count(), LIMITE + 1);
        assert_eq!(preparar("  hola \n"), "hola");
        assert_eq!(escapar("<b>x & y</b>"), "&lt;b&gt;x &amp; y&lt;/b&gt;");
    }
}
