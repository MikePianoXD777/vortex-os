//! Con qué IA piensa Miku («ia» en miku.json): Claude (`claude -p`, `cerebro.rs`) o cualquiera compatible con
//! OpenAI: `local` (Ollama en tu PC, sin cuenta ni internet), `gemini`, `openai` u `otra`. La clave va en el
//! llavero (`secret-tool`) o, si no hay, en `~/.config/vortex/ia-<proveedor>.clave` (0600). Nunca en miku.json.

use crate::config::Config;
use crate::http::{self, Cortador, Respuesta};
use crate::rutas::Rutas;
use crate::sistema;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MODELO_LOCAL: &str = "qwen3.5:4b";
pub const TAMANO_LOCAL: &str = "3,3 GB";
/// 4096 (lo que pone Ollama solo) se llena en una plática corta y Ollama corta el principio: las
/// instrucciones de Miku. Va también en `systemd/vortex-ollama.service`.
pub const CONTEXTO_LOCAL: &str = "8192";
pub const UNIDAD_OLLAMA: &str = "vortex-ollama.service";

/// Error de la IA: el texto ya está en español y dice qué hacer. `clave` es para la ventana.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorIA {
    pub clave: String,
    pub mensaje: String,
}

impl ErrorIA {
    pub fn nuevo(clave: &str, mensaje: impl Into<String>) -> Self {
        Self { clave: clave.into(), mensaje: mensaje.into() }
    }
}

impl std::fmt::Display for ErrorIA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.mensaje)
    }
}
impl std::error::Error for ErrorIA {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clave {
    No,
    Si,
    Opcional,
}

#[derive(Debug, Clone)]
pub struct Proveedor {
    pub id: String,
    pub nombre: String,
    pub url: String,
    pub modelo: String,
    pub clave: Clave,
    pub donde: String,
    pub extra: Map<String, Value>,
}

pub fn proveedor_base(id: &str, ollama: &str) -> Proveedor {
    let (nombre, url, modelo, clave, donde) = match id {
        "local" => ("IA local", format!("{ollama}/v1"), MODELO_LOCAL, Clave::No, ""),
        "gemini" => (
            "Gemini",
            "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            "gemini-flash-latest",
            Clave::Si,
            "aistudio.google.com/apikey",
        ),
        "openai" => ("ChatGPT", "https://api.openai.com/v1".to_string(), "gpt-5-mini", Clave::Si, "platform.openai.com/api-keys"),
        "otra" => ("tu IA", String::new(), "", Clave::Opcional, ""),
        _ => ("Claude", String::new(), "", Clave::No, ""),
    };
    // Sin «pensar» en voz baja antes de contestar: en una GPU de 4 GB eso son segundos de silencio. Sin pensar,
    // Qwen recomienda temperatura 0.7 y top_p 0.8.
    let extra = if id == "local" {
        let mut m = Map::new();
        m.insert("reasoning_effort".into(), json!("none"));
        m.insert("temperature".into(), json!(0.7));
        m.insert("top_p".into(), json!(0.8));
        m
    } else {
        Map::new()
    };
    Proveedor { id: id.into(), nombre: nombre.into(), url, modelo: modelo.into(), clave, donde: donde.into(), extra }
}

/// `ia_url` / `ia_modelo` mandan si vienen.
pub fn elegido(cfg: &Config, ollama: &str) -> Proveedor {
    let id = if crate::config::IAS.contains(&cfg.ia.as_str()) { cfg.ia.as_str() } else { "claude" };
    let mut p = proveedor_base(id, ollama);
    if !cfg.ia_url.is_empty() && id == "otra" {
        p.url = cfg.ia_url.clone();
    }
    if !cfg.ia_modelo.is_empty() && id != "claude" {
        p.modelo = cfg.ia_modelo.clone();
    }
    p.url = p.url.trim_end_matches('/').to_string();
    p
}

/// Sin lo que algunas terminales meten al pegar (ESC[200~ … ESC[201~), ni espacios ni invisibles.
pub fn limpiar_clave(clave: &str) -> String {
    let re = Regex::new(r"\x1b?\[20[01]~").unwrap();
    re.replace_all(clave, "").chars().filter(|c| !c.is_control() && !c.is_whitespace()).collect()
}

/// Explica un error HTTP en español.
pub fn explicar(p: &Proveedor, codigo: u16, cuerpo: &str) -> ErrorIA {
    let n = &p.nombre;
    let clave_mala = Regex::new(r"(?i)api[ _-]?key").unwrap();
    if codigo == 401 || codigo == 403 || (codigo == 400 && clave_mala.is_match(cuerpo)) {
        return ErrorIA::nuevo("clave", format!("{n} no aceptó tu clave. Ponla otra vez en una terminal con: vortex-miku ia clave"));
    }
    if codigo == 404 && p.id == "local" {
        return ErrorIA::nuevo("modelo", format!("Para pensar sin internet necesito bajar mi IA local una sola vez ({TAMANO_LOCAL})."));
    }
    if codigo == 404 {
        return ErrorIA::nuevo("otro", format!("{n} no tiene el modelo «{}». Cámbialo con: vortex-miku ia {} MODELO", p.modelo, p.id));
    }
    if codigo == 429 {
        return ErrorIA::nuevo("cuota", format!("Ya usé todo el límite de {n} por ahora. Prueba otra vez en un rato."));
    }
    if codigo >= 500 {
        return ErrorIA::nuevo("saturado", format!("Hay demasiada gente usando {n} en este momento. Prueba otra vez en un rato."));
    }
    let corto: String = cuerpo.chars().take(160).collect();
    ErrorIA::nuevo("otro", format!("{n} contestó con un error ({codigo}): {corto}"))
}

#[derive(Default)]
struct Propio {
    unidad: bool,
    proceso: Option<Child>,
}

pub struct Ia {
    pub rutas: Rutas,
    pub ollama: String,
    /// Usar `secret-tool` para las claves (las pruebas lo apagan: no tocan el llavero de nadie).
    pub llavero: bool,
    propio: Mutex<Propio>,
    precargado: Mutex<Option<Instant>>,
}

impl Ia {
    pub fn nueva(rutas: Rutas) -> Self {
        Self {
            rutas,
            ollama: std::env::var("VORTEX_MIKU_OLLAMA")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "http://127.0.0.1:11434".into())
                .trim_end_matches('/')
                .to_string(),
            llavero: true,
            propio: Mutex::new(Propio::default()),
            precargado: Mutex::new(None),
        }
    }

    pub fn elegido(&self, cfg: &Config) -> Proveedor {
        elegido(cfg, &self.ollama)
    }

    // ---- La clave --------------------------------------------------------------------------------

    fn llavero_args(proveedor: &str) -> Vec<String> {
        ["service", "vortex-miku", "cuenta"].iter().map(|s| s.to_string()).chain([format!("ia-{proveedor}")]).collect()
    }

    pub fn archivo_clave(&self, proveedor: &str) -> std::path::PathBuf {
        self.rutas.config_dir.join(format!("ia-{proveedor}.clave"))
    }

    pub fn leer_clave(&self, proveedor: &str) -> String {
        if let Ok(c) = std::env::var("VORTEX_MIKU_IA_CLAVE")
            && !c.is_empty()
        {
            return limpiar_clave(&c);
        }
        if self.llavero && sistema::cual("secret-tool").is_some()
            && let Ok(o) = Command::new("secret-tool")
                .arg("lookup")
                .args(Self::llavero_args(proveedor))
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
            && o.status.success()
        {
            let c = limpiar_clave(&String::from_utf8_lossy(&o.stdout));
            if !c.is_empty() {
                return c;
            }
        }
        std::fs::read_to_string(self.archivo_clave(proveedor)).map(|t| limpiar_clave(&t)).unwrap_or_default()
    }

    /// Guarda la clave y dice dónde quedó («llavero» o la ruta del archivo).
    pub fn guardar_clave(&self, proveedor: &str, clave: &str) -> std::io::Result<String> {
        let clave = limpiar_clave(clave);
        let archivo = self.archivo_clave(proveedor);
        if self.llavero && sistema::cual("secret-tool").is_some() {
            let nombre = proveedor_base(proveedor, &self.ollama).nombre;
            if let Ok(mut h) = Command::new("secret-tool")
                .args(["store", "--label", &format!("{nombre} (Miku de Vortexine's OS)")])
                .args(Self::llavero_args(proveedor))
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut e) = h.stdin.take() {
                    let _ = e.write_all(clave.as_bytes());
                }
                if h.wait().map(|s| s.success()).unwrap_or(false) {
                    let _ = std::fs::remove_file(&archivo);
                    return Ok("llavero".into());
                }
            }
        }
        std::fs::create_dir_all(&self.rutas.config_dir)?;
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&archivo)?;
            writeln!(f, "{clave}")?;
        }
        sistema::permisos(&archivo, 0o600)?;
        Ok(archivo.display().to_string())
    }

    pub fn borrar_clave(&self, proveedor: &str) {
        if self.llavero && sistema::cual("secret-tool").is_some() {
            let _ = Command::new("secret-tool")
                .arg("clear")
                .args(Self::llavero_args(proveedor))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = std::fs::remove_file(self.archivo_clave(proveedor));
    }

    // ---- Ollama (la IA local) -----------------------------------------------------------------------

    fn ollama_json(&self, ruta: &str, datos: Option<&Value>, timeout: u64) -> Result<Value, String> {
        let url = format!("{}{ruta}", self.ollama);
        let (estado, cuerpo) = match datos {
            Some(d) => http::json_a_json(&url, &[], d, timeout)?,
            None => http::obtener(&url, timeout)?,
        };
        if estado >= 400 {
            return Err(format!("HTTP {estado}"));
        }
        Ok(serde_json::from_str(&cuerpo).unwrap_or(json!({})))
    }

    pub fn ollama_instalado(&self) -> bool {
        sistema::cual("ollama").is_some()
    }

    pub fn ollama_corriendo(&self) -> bool {
        self.ollama_json("/api/version", None, 2).is_ok()
    }

    /// Deja corriendo Ollama para tu usuario. Primero la unidad de usuario del paquete; si no está,
    /// `ollama serve` aparte de Miku. `true` si ya contesta.
    pub fn arrancar_ollama(&self, espera: Duration) -> bool {
        if self.ollama_corriendo() {
            return true;
        }
        if !self.ollama_instalado() {
            return false;
        }
        let unidad = Command::new("systemctl")
            .args(["--user", "start", UNIDAD_OLLAMA])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if unidad {
            self.propio.lock().unwrap().unidad = true;
        } else {
            let host = self.ollama.split("://").last().unwrap_or("127.0.0.1:11434").to_string();
            match Command::new("ollama")
                .arg("serve")
                .env("OLLAMA_HOST", host)
                .env("OLLAMA_VULKAN", "1")
                .env("OLLAMA_KEEP_ALIVE", "10m")
                .env("OLLAMA_CONTEXT_LENGTH", CONTEXTO_LOCAL)
                .env("OLLAMA_NUM_PARALLEL", "1")
                .env("OLLAMA_MAX_LOADED_MODELS", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(h) => self.propio.lock().unwrap().proceso = Some(h),
                Err(_) => return false,
            }
        }
        let limite = Instant::now() + espera;
        while Instant::now() < limite {
            if self.ollama_corriendo() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        false
    }

    pub fn modelo_local_listo(&self, modelo: &str) -> bool {
        self.ollama_json("/api/show", Some(&json!({"model": modelo})), 5).is_ok()
    }

    /// Sube el modelo a la GPU aparte (la primera respuesta tardaba ~20 s en cargarlo). La ventana lo llama
    /// cuando su voz ya cargó: la voz natural necesita su memoria de GPU de una pieza y Ollama no.
    pub fn precargar(self: &std::sync::Arc<Self>, modelo: &str) {
        {
            let mut p = self.precargado.lock().unwrap();
            if p.is_some_and(|t| t.elapsed() < Duration::from_secs(300)) {
                return;
            }
            *p = Some(Instant::now());
        }
        let (yo, modelo) = (self.clone(), modelo.to_string());
        std::thread::spawn(move || {
            if yo.arrancar_ollama(Duration::from_secs(20)) && yo.modelo_local_listo(&modelo) {
                let _ = yo.ollama_json("/api/generate", Some(&json!({"model": modelo, "keep_alive": "10m"})), 300);
            }
        });
    }

    /// Saca el modelo de la GPU ya (al cerrar Miku: si no, sigue ahí 10 min y la voz natural no cabe).
    pub fn soltar(&self, modelo: &str) {
        *self.precargado.lock().unwrap() = None;
        let _ = self.ollama_json("/api/generate", Some(&json!({"model": modelo, "keep_alive": 0})), 3);
    }

    /// Al cerrar Miku: para el Ollama que arrancó ella (sin Miku abierta no queda nada corriendo).
    pub fn parar_ollama_propio(&self) {
        let mut p = self.propio.lock().unwrap();
        if std::mem::take(&mut p.unidad) {
            let _ = Command::new("systemctl")
                .args(["--user", "stop", "--no-block", UNIDAD_OLLAMA])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        if let Some(mut h) = p.proceso.take() {
            let _ = h.kill();
            let _ = h.wait();
        }
    }

    /// Revisa que la IA local pueda contestar ya. Si no, `ErrorIA` con qué hacer («modelo»: falta bajarlo).
    pub fn preparar_local(&self, modelo: &str) -> Result<(), ErrorIA> {
        if !self.ollama_instalado() {
            return Err(ErrorIA::nuevo("otro", "Me falta Ollama para pensar en tu PC. Actualiza Vortexine's en una terminal con: vortex actualizar"));
        }
        if !self.arrancar_ollama(Duration::from_secs(20)) {
            return Err(ErrorIA::nuevo("otro", "No pude arrancar la IA local (Ollama). Mira qué pasa con: systemctl --user status vortex-ollama"));
        }
        if !self.modelo_local_listo(modelo) {
            return Err(ErrorIA::nuevo("modelo", format!("Para pensar sin internet necesito bajar mi IA local una sola vez ({TAMANO_LOCAL}).")));
        }
        Ok(())
    }

    /// Baja el modelo con Ollama. `progreso(detalle, fracción)`; `cancelar` corta la descarga.
    pub fn bajar_modelo(&self, modelo: &str, progreso: &dyn Fn(&str, f64), cancelar: &AtomicBool) -> Result<(), ErrorIA> {
        if !self.arrancar_ollama(Duration::from_secs(20)) {
            return Err(ErrorIA::nuevo("otro", "No pude arrancar la IA local (Ollama) para bajar el modelo."));
        }
        let cuerpo = json!({"model": modelo, "stream": true}).to_string().into_bytes();
        let mut r = http::pedir(
            "POST",
            &format!("{}/api/pull", self.ollama),
            &[("Content-Type", "application/json".into())],
            Some(cuerpo),
            60,
        )
        .map_err(|e| ErrorIA::nuevo("internet", format!("Se cortó la descarga de la IA local ({e}). Mira tu internet y vuelve a intentarlo.")))?;
        if r.estado >= 400 {
            return Err(ErrorIA::nuevo("otro", format!("Ollama no quiso bajar «{modelo}» (error {}).", r.estado)));
        }
        let mut capas: HashMap<String, (u64, u64)> = HashMap::new();
        while let Some(l) = r.linea() {
            if cancelar.load(Ordering::SeqCst) {
                return Err(ErrorIA::nuevo("cancelado", "Cancelé la descarga de la IA local."));
            }
            let Ok(ev) = serde_json::from_slice::<Value>(&l) else { continue };
            if let Some(e) = ev.get("error").and_then(Value::as_str).filter(|e| !e.is_empty()) {
                return Err(ErrorIA::nuevo("otro", format!("No pude bajar la IA local: {e}")));
            }
            if let (Some(d), Some(t)) = (ev.get("digest").and_then(Value::as_str), ev.get("total").and_then(Value::as_u64))
                && t > 0
            {
                capas.insert(d.into(), (ev.get("completed").and_then(Value::as_u64).unwrap_or(0), t));
            }
            let hecho: u64 = capas.values().map(|c| c.0).sum();
            let total: u64 = capas.values().map(|c| c.1).sum();
            if total > 0 {
                progreso(&format!("{:.1} de {:.1} GB", hecho as f64 / 1e9, total as f64 / 1e9), (hecho as f64 / total as f64).min(1.0));
            }
            if ev.get("status").and_then(Value::as_str) == Some("success") {
                progreso("Lista", 1.0);
                return Ok(());
            }
        }
        Err(ErrorIA::nuevo("otro", "La descarga de la IA local terminó a medias. Vuelve a intentarlo."))
    }

    // ---- La API compatible con OpenAI ------------------------------------------------------------

    /// POST /chat/completions. Devuelve la respuesta abierta (quien llama la lee). `al_abrir` recibe el
    /// cortador en cuanto existe, para cancelar desde otro hilo.
    pub fn pedir(&self, p: &Proveedor, cuerpo: &Value, inactivo: u64, al_abrir: Option<&dyn Fn(Cortador)>) -> Result<Respuesta, ErrorIA> {
        let mut cab: Vec<(&str, String)> = vec![("Content-Type", "application/json".into())];
        if p.clave != Clave::No {
            let c = self.leer_clave(&p.id);
            if !c.is_empty() {
                cab.push(("Authorization", format!("Bearer {c}")));
            }
        }
        let mut datos = cuerpo.as_object().cloned().unwrap_or_default();
        datos.insert("model".into(), json!(p.modelo));
        for (k, v) in &p.extra {
            datos.insert(k.clone(), v.clone());
        }
        let falla_red = |e: String| {
            if p.id == "local" {
                ErrorIA::nuevo("otro", "La IA local no contesta. Mira que Ollama esté corriendo: systemctl --user status vortex-ollama")
            } else {
                ErrorIA::nuevo("internet", format!("No pude llegar a {} ({e}). Mira tu internet.", p.nombre))
            }
        };
        let mut r = http::pedir(
            "POST",
            &format!("{}/chat/completions", p.url),
            &cab,
            Some(Value::Object(datos).to_string().into_bytes()),
            inactivo,
        )
        .map_err(falla_red)?;
        if r.estado >= 400 {
            let texto = String::from_utf8_lossy(&r.leer_todo()).into_owned();
            return Err(explicar(p, r.estado, &texto));
        }
        if let Some(f) = al_abrir {
            f(r.cortador());
        }
        Ok(r)
    }

    /// Una respuesta entera, sin flujo ni herramientas (la usa `momentos`).
    pub fn completar(&self, p: &Proveedor, mensajes: &Value, timeout: u64) -> Result<String, ErrorIA> {
        let mut r = self.pedir(p, &json!({"messages": mensajes, "stream": false}), timeout, None)?;
        let datos: Value = serde_json::from_slice(&r.leer_todo()).unwrap_or(json!({}));
        Ok(datos["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
    }

    /// («ok», "") si se puede hablar con este proveedor; si no, (clave, mensaje en español).
    pub fn comprobar(&self, p: &Proveedor) -> (String, String) {
        if p.url.is_empty() || p.modelo.is_empty() {
            return ("otro".into(), "Me falta la dirección o el modelo de tu IA. Ponlos con: vortex-miku ia otra URL MODELO".into());
        }
        if p.clave == Clave::Si && self.leer_clave(&p.id).is_empty() {
            return (
                "clave".into(),
                format!("Para hablar conmigo con {} necesito tu clave ({}). Ponla en una terminal con: vortex-miku ia clave", p.nombre, p.donde),
            );
        }
        if p.id == "local"
            && let Err(e) = self.preparar_local(&p.modelo)
        {
            return (e.clave, e.mensaje);
        }
        ("ok".into(), String::new())
    }
}
