//! Cliente de Ollama: la IA que corre en tu PC (igual que `ollama.py`).
//!
//! Los modelos viven en `~/.ollama` y se comparten con Miku: si Ollama no está corriendo, primero se arranca
//! la unidad de usuario de Miku (`vortex-ollama.service`, del paquete vortex-miku) y, si no existe,
//! `ollama serve` como proceso aparte con la misma configuración.

use crate::http::{Cancelar, Destino, ErrHttp, Respuesta, pedir};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// `al_progreso(completado, total, estado)`.
pub type AlProgreso<'a> = &'a mut dyn FnMut(i64, i64, &str);

pub const URL_POR_DEFECTO: &str = "http://127.0.0.1:11434";
/// La unidad de Miku: `apps/miku/systemd/vortex-ollama.service`.
pub const UNIDAD: &str = "vortex-ollama.service";

/// Igual que la unidad de Miku, para que los dos arranques se porten igual.
pub const ENTORNO_SERVIDOR: [(&str, &str); 5] = [
    ("OLLAMA_VULKAN", "1"),
    ("OLLAMA_KEEP_ALIVE", "10m"),
    ("OLLAMA_CONTEXT_LENGTH", "8192"),
    ("OLLAMA_NUM_PARALLEL", "1"),
    ("OLLAMA_MAX_LOADED_MODELS", "1"),
];

/// Dónde puede estar instalada la unidad de usuario.
const DIRS_UNIDADES: [&str; 4] = [
    "/usr/lib/systemd/user",
    "/etc/systemd/user",
    "~/.config/systemd/user",
    "~/.local/share/systemd/user",
];

fn estado_legible(estado: &str) -> Option<&'static str> {
    Some(match estado {
        "pulling manifest" => "Buscando el modelo",
        "verifying sha256 digest" => "Revisando la descarga",
        "writing manifest" | "removing any unused layers" | "removing unused layers" => "Guardando",
        "success" => "Listo",
        _ => return None,
    })
}

/// Un fallo de Ollama con un mensaje en español para enseñárselo a la persona.
///
/// `clave`: «conexion» (no contesta), «no_instalado», «modelo» (no existe o no lo tienes), «red» (sin
/// internet al bajar), «cancelado» u «otro».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorOllama {
    pub mensaje: String,
    pub clave: String,
}

impl ErrorOllama {
    pub fn new(mensaje: impl Into<String>, clave: &str) -> Self {
        ErrorOllama {
            mensaje: mensaje.into(),
            clave: clave.into(),
        }
    }
    pub fn otro(mensaje: impl Into<String>) -> Self {
        Self::new(mensaje, "otro")
    }
    /// Se canceló a medias (con la señal de cancelar).
    pub fn cancelado() -> Self {
        Self::new("Se canceló.", "cancelado")
    }
    pub fn es_cancelado(&self) -> bool {
        self.clave == "cancelado"
    }
}

impl std::fmt::Display for ErrorOllama {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.mensaje)
    }
}

impl std::error::Error for ErrorOllama {}

fn capitalizar(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Pasa un error de Ollama (en inglés) a un `ErrorOllama` claro.
pub fn traducir(error: &str, modelo: &str) -> ErrorOllama {
    let bajo = error.to_lowercase();
    let nombre = if modelo.is_empty() {
        "ese modelo".to_string()
    } else {
        format!("«{modelo}»")
    };
    let tiene = |p: &str| bajo.contains(p);
    if tiene("file does not exist")
        || tiene("manifest unknown")
        || (tiene("pull model manifest") && tiene("not found"))
    {
        return ErrorOllama::new(
            format!(
                "No encontré {nombre} en la biblioteca de Ollama. Revisa que el nombre esté bien escrito."
            ),
            "modelo",
        );
    }
    if tiene("not found") && (tiene("model") || !modelo.is_empty()) {
        return ErrorOllama::new(
            format!("No tienes {nombre} en tu PC. Bájalo primero en «Modelos»."),
            "modelo",
        );
    }
    if tiene("does not support") && tiene("embed") {
        return ErrorOllama::new(
            format!(
                "{} no sirve para embeddings: elige uno para el vault, como nomic-embed-text.",
                capitalizar(&nombre)
            ),
            "modelo",
        );
    }
    if tiene("does not support thinking") {
        return ErrorOllama::new(
            format!("{} no sabe «pensar»: apaga esa opción.", capitalizar(&nombre)),
            "modelo",
        );
    }
    if tiene("does not support") && (tiene("image") || tiene("vision")) {
        return ErrorOllama::new(
            format!(
                "{} no puede ver imágenes: elige un modelo de visión.",
                capitalizar(&nombre)
            ),
            "modelo",
        );
    }
    if [
        "no such host",
        "dial tcp",
        "lookup",
        "network is unreachable",
        "tls",
        "i/o timeout",
        "connection reset",
        "eof",
    ]
    .iter()
    .any(|p| tiene(p))
    {
        return ErrorOllama::new(
            "No pude conectarme a internet para bajar el modelo. Revisa tu conexión y vuelve a intentar; \
             lo que ya se bajó no se pierde.",
            "red",
        );
    }
    if tiene("no space left") {
        return ErrorOllama::otro(
            "Ya no hay espacio en el disco para el modelo. Borra algún modelo o libera espacio.",
        );
    }
    if tiene("out of memory") || tiene("cudamalloc") || tiene("requires more system memory") {
        return ErrorOllama::new(
            format!(
                "{} no cabe en la memoria de tu PC. Prueba con uno más chico.",
                capitalizar(&nombre)
            ),
            "modelo",
        );
    }
    ErrorOllama::otro(format!("Ollama dijo: {error}"))
}

fn bytes(v: &Value) -> i64 {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// Separa lo que va dentro de `<think>…</think>` del resto, aunque las etiquetas lleguen partidas.
#[derive(Default)]
pub struct SeparadorPensar {
    dentro: bool,
    pendiente: String,
}

impl SeparadorPensar {
    const ABRE: &'static str = "<think>";
    const CIERRA: &'static str = "</think>";

    /// `[(es_pensamiento, texto), …]` con lo que ya se puede soltar.
    pub fn meter(&mut self, texto: &str) -> Vec<(bool, String)> {
        let mut salida = Vec::new();
        let mut buf = std::mem::take(&mut self.pendiente) + texto;
        while !buf.is_empty() {
            let etiqueta = if self.dentro { Self::CIERRA } else { Self::ABRE };
            if let Some(i) = buf.find(etiqueta) {
                if i > 0 {
                    salida.push((self.dentro, buf[..i].to_string()));
                }
                buf = buf[i + etiqueta.len()..].to_string();
                self.dentro = !self.dentro;
                continue;
            }
            // ¿El final podría ser el principio de la etiqueta? Guárdalo para el siguiente trozo.
            let mut corte = buf.len();
            for n in (1..=(etiqueta.len() - 1).min(buf.len())).rev() {
                if buf.is_char_boundary(buf.len() - n) && etiqueta.starts_with(&buf[buf.len() - n..]) {
                    corte = buf.len() - n;
                    break;
                }
            }
            if corte > 0 {
                salida.push((self.dentro, buf[..corte].to_string()));
            }
            self.pendiente = buf[corte..].to_string();
            break;
        }
        salida
    }

    pub fn terminar(&mut self) -> Vec<(bool, String)> {
        let resto = std::mem::take(&mut self.pendiente);
        if resto.is_empty() {
            vec![]
        } else {
            vec![(self.dentro, resto)]
        }
    }
}

/// Las estadísticas de la última respuesta del chat.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Estadisticas {
    pub tokens: i64,
    pub segundos: f64,
    pub tokens_por_segundo: f64,
}

/// Un modelo instalado (`/api/tags`).
#[derive(Debug, Clone, PartialEq)]
pub struct Modelo {
    pub nombre: String,
    pub bytes: i64,
    pub familia: String,
    pub parametros: String,
    pub cuantizacion: String,
    pub modificado: String,
    pub embeddings: bool,
}

/// Un modelo que está en memoria ahora (`/api/ps`).
#[derive(Debug, Clone, PartialEq)]
pub struct Cargado {
    pub nombre: String,
    pub bytes: i64,
    pub bytes_vram: i64,
    pub en_gpu_pct: i64,
    pub expira: String,
}

#[derive(Default)]
struct Arranque {
    unidad: bool,
    proceso: Option<Child>,
}

/// Habla con el servidor de Ollama (por defecto `http://127.0.0.1:11434`).
pub struct Ollama {
    pub url: String,
    pub puede_arrancar: bool,
    destino: Destino,
    ultimo_error: Mutex<String>,
    estadisticas: Mutex<Estadisticas>,
    arranque: Mutex<Arranque>,
}

/// ¿Está ese programa en el PATH?
pub fn en_path(programa: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p).map(|d| d.join(programa)).find(|c| {
            c.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

fn expandir(carpeta: &str) -> PathBuf {
    match carpeta.strip_prefix("~/") {
        Some(r) => vortex_core::rutas::home().join(r),
        None => Path::new(carpeta).to_path_buf(),
    }
}

impl Ollama {
    /// `url`: la de Ollama; si no, `VORTEX_LOCAL_AI_OLLAMA`; si no, la de siempre. `puede_arrancar`: si no, lo
    /// dice `VORTEX_LOCAL_AI_NO_ARRANCAR` (vacío o «0» = puede).
    pub fn new(url: Option<&str>, puede_arrancar: Option<bool>) -> Ollama {
        let entorno = std::env::var("VORTEX_LOCAL_AI_OLLAMA").ok();
        let mut url = url
            .filter(|u| !u.is_empty())
            .map(str::to_string)
            .or(entorno.filter(|u| !u.is_empty()))
            .unwrap_or_else(|| URL_POR_DEFECTO.to_string());
        url = url.trim_end_matches('/').to_string();
        if !url.contains("://") {
            url = format!("http://{url}");
        }
        let puede_arrancar = puede_arrancar.unwrap_or_else(|| {
            let v = std::env::var("VORTEX_LOCAL_AI_NO_ARRANCAR").unwrap_or_default();
            v.is_empty() || v == "0"
        });
        Ollama {
            destino: Destino::de_url(&url),
            url,
            puede_arrancar,
            ultimo_error: Mutex::new(String::new()),
            estadisticas: Mutex::new(Estadisticas::default()),
            arranque: Mutex::new(Arranque::default()),
        }
    }

    pub fn ultimo_error(&self) -> String {
        self.ultimo_error.lock().unwrap().clone()
    }

    pub fn ultimas_estadisticas(&self) -> Estadisticas {
        *self.estadisticas.lock().unwrap()
    }

    /// Para las pruebas: ¿esta instancia arrancó algo?
    pub fn arranco_algo(&self) -> bool {
        let a = self.arranque.lock().unwrap();
        a.unidad || a.proceso.is_some()
    }

    // ---------------------------------------------------------------- HTTP

    fn error_conexion(&self) -> ErrorOllama {
        if !self.instalado() && self.es_local() {
            return ErrorOllama::new(
                "Ollama no está instalado. Actualiza Vortexine's con: vortex actualizar",
                "no_instalado",
            );
        }
        ErrorOllama::new(
            format!(
                "No me pude conectar con Ollama ({}). Puede que esté apagado: ábrelo de nuevo \
                 o mira qué pasa con: systemctl --user status vortex-ollama",
                self.url
            ),
            "conexion",
        )
    }

    fn error_http(&self, codigo: u16, cuerpo: &[u8], modelo: &str) -> ErrorOllama {
        let mensaje = match serde_json::from_slice::<Value>(if cuerpo.is_empty() { b"{}" } else { cuerpo }) {
            Ok(v) => v.get("error").and_then(Value::as_str).unwrap_or("").to_string(),
            Err(_) => String::from_utf8_lossy(cuerpo).trim().to_string(),
        };
        if mensaje.is_empty() {
            if codigo == 404 {
                return traducir("model not found", modelo);
            }
            return traducir(&format!("error {codigo}"), modelo);
        }
        traducir(&mensaje, modelo)
    }

    fn json(
        &self,
        ruta: &str,
        datos: Option<&Value>,
        metodo: Option<&str>,
        espera: f64,
        modelo: &str,
    ) -> Result<Value, ErrorOllama> {
        let cuerpo = datos.map(|d| d.to_string().into_bytes());
        let metodo = metodo.unwrap_or(if cuerpo.is_some() { "POST" } else { "GET" });
        let (estado, crudo) = pedir(
            &self.destino,
            metodo,
            ruta,
            cuerpo.as_deref(),
            Duration::from_secs_f64(espera),
        )
        .map_err(|_| self.error_conexion())?;
        if estado >= 400 {
            return Err(self.error_http(estado, &crudo, modelo));
        }
        serde_json::from_slice(if crudo.is_empty() { b"{}" } else { &crudo })
            .map_err(|_| ErrorOllama::otro("Ollama contestó algo que no entiendo."))
    }

    /// POST que entrega una línea JSON por evento. Si `cancelar` se activa, corta ya.
    fn flujo(
        &self,
        ruta: &str,
        datos: &Value,
        cancelar: Option<&Cancelar>,
        espera: f64,
        modelo: &str,
        mut por_evento: impl FnMut(&Value) -> Result<(), ErrorOllama>,
    ) -> Result<(), ErrorOllama> {
        let esta_cancelado = || cancelar.is_some_and(Cancelar::is_set);
        let cuerpo = datos.to_string().into_bytes();
        let mut r = match Respuesta::enviar(
            &self.destino,
            "POST",
            ruta,
            Some(&cuerpo),
            Duration::from_secs_f64(espera),
            cancelar,
        ) {
            Ok(r) => r,
            Err(ErrHttp::Cancelado) => return Err(ErrorOllama::cancelado()),
            Err(_) => {
                return Err(if esta_cancelado() {
                    ErrorOllama::cancelado()
                } else {
                    self.error_conexion()
                });
            }
        };
        if r.estado >= 400 {
            let estado = r.estado;
            let cuerpo = r.todo(None).unwrap_or_default();
            return Err(self.error_http(estado, &cuerpo, modelo));
        }
        loop {
            if esta_cancelado() {
                return Err(ErrorOllama::cancelado());
            }
            let linea = match r.linea(cancelar) {
                Ok(Some(l)) => l,
                Ok(None) => {
                    return if esta_cancelado() {
                        Err(ErrorOllama::cancelado())
                    } else {
                        Ok(())
                    };
                }
                Err(ErrHttp::Cancelado) => return Err(ErrorOllama::cancelado()),
                Err(_) => {
                    return Err(ErrorOllama::new(
                        "Se cortó la conexión con Ollama a medias. Vuelve a intentar.",
                        "conexion",
                    ));
                }
            };
            let texto = String::from_utf8_lossy(&linea);
            let texto = texto.trim();
            if texto.is_empty() {
                continue;
            }
            let Ok(evento) = serde_json::from_str::<Value>(texto) else {
                continue;
            };
            if let Some(e) = evento
                .get("error")
                .filter(|e| !e.is_null() && e.as_str() != Some(""))
            {
                return Err(traducir(e.as_str().unwrap_or(&e.to_string()), modelo));
            }
            por_evento(&evento)?;
        }
    }

    // ---------------------------------------------------------------- servidor

    /// ¿Está el comando `ollama`?
    pub fn instalado(&self) -> bool {
        en_path("ollama").is_some()
    }

    fn es_local(&self) -> bool {
        ["127.0.0.1", "localhost", "::1", "0.0.0.0"].contains(&self.destino.host.as_str())
    }

    pub fn disponible(&self) -> bool {
        self.json("/api/version", None, None, 1.5, "").is_ok()
    }

    pub fn version(&self) -> Option<String> {
        self.json("/api/version", None, None, 1.5, "")
            .ok()
            .and_then(|v| v.get("version").and_then(Value::as_str).map(str::to_string))
    }

    fn hay_unidad() -> bool {
        DIRS_UNIDADES.iter().any(|c| expandir(c).join(UNIDAD).is_file())
    }

    /// Deja Ollama contestando. `true` si ya contesta o si lo pude arrancar a tiempo.
    ///
    /// Primero la unidad de usuario de Miku (`vortex-ollama.service`, mismo `~/.ollama`); si no existe o
    /// falla, `ollama serve` como proceso aparte (sin atarse a esta app). Solo arranca algo si la URL es de
    /// esta PC. Si no se puede, `ultimo_error` dice por qué.
    pub fn asegurar_servidor(&self, espera: Duration) -> bool {
        self.ultimo_error.lock().unwrap().clear();
        let falla = |t: String| {
            *self.ultimo_error.lock().unwrap() = t;
            false
        };
        if self.disponible() {
            return true;
        }
        if !self.puede_arrancar {
            return falla("Ollama no contesta (y esta instancia no tiene permiso de arrancarlo).".into());
        }
        if !self.es_local() {
            return falla(format!(
                "Ollama no contesta en {} y no está en esta PC: no lo puedo arrancar.",
                self.url
            ));
        }
        if !self.instalado() {
            return falla("Ollama no está instalado. Actualiza Vortexine's con: vortex actualizar".into());
        }
        let mut arrancado = false;
        // La unidad escucha en 127.0.0.1:11434: solo sirve si esa es nuestra URL.
        if self.destino.puerto == 11434 && Self::hay_unidad() {
            arrancado = Command::new("systemctl")
                .args(["--user", "start", UNIDAD])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if arrancado {
                self.arranque.lock().unwrap().unidad = true;
            }
        }
        if !arrancado {
            use std::os::unix::process::CommandExt;
            let mut orden = Command::new("ollama");
            orden
                .arg("serve")
                .env(
                    "OLLAMA_HOST",
                    format!("{}:{}", self.destino.host, self.destino.puerto),
                )
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0);
            for (k, v) in ENTORNO_SERVIDOR {
                orden.env(k, v);
            }
            match orden.spawn() {
                Ok(h) => self.arranque.lock().unwrap().proceso = Some(h),
                Err(e) => return falla(format!("No pude arrancar Ollama: {e}")),
            }
        }
        let limite = Instant::now() + espera;
        while Instant::now() < limite {
            if self.disponible() {
                return true;
            }
            if let Some(p) = self.arranque.lock().unwrap().proceso.as_mut()
                && p.try_wait().ok().flatten().is_some()
            {
                return falla("Ollama se cerró al arrancar. Mira qué pasa con: ollama serve".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        falla(
            "Ollama tardó demasiado en arrancar. Mira qué pasa con: systemctl --user status vortex-ollama"
                .into(),
        )
    }

    /// Para el Ollama que arrancó esta instancia. Si ya estaba corriendo antes, no lo toca.
    pub fn parar_propio(&self) {
        let (unidad, proceso) = {
            let mut a = self.arranque.lock().unwrap();
            (std::mem::take(&mut a.unidad), a.proceso.take())
        };
        if unidad {
            let _ = Command::new("systemctl")
                .args(["--user", "stop", "--no-block", UNIDAD])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        if let Some(mut p) = proceso {
            if p.try_wait().ok().flatten().is_none() {
                let _ = p.kill();
            }
            let _ = p.wait();
        }
    }

    // ---------------------------------------------------------------- modelos

    /// Los modelos instalados (`/api/tags`), por nombre.
    pub fn modelos(&self) -> Result<Vec<Modelo>, ErrorOllama> {
        let v = self.json("/api/tags", None, None, 10.0, "")?;
        let mut salida = Vec::new();
        for m in v.get("models").and_then(Value::as_array).into_iter().flatten() {
            let det = m.get("details").cloned().unwrap_or(Value::Null);
            let texto = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let nombre = m
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.is_empty())
                .or_else(|| m.get("model").and_then(Value::as_str))
                .unwrap_or("")
                .to_string();
            let familia = texto(&det, "family");
            let familias: Vec<String> = match det.get("families").and_then(Value::as_array) {
                Some(l) if !l.is_empty() => l
                    .iter()
                    .map(|f| f.as_str().unwrap_or("").to_lowercase())
                    .collect(),
                _ => vec![familia.to_lowercase()],
            };
            salida.push(Modelo {
                embeddings: nombre.to_lowercase().contains("embed")
                    || familias.iter().any(|f| f.contains("bert")),
                bytes: m.get("size").map(bytes).unwrap_or(0),
                parametros: texto(&det, "parameter_size"),
                cuantizacion: texto(&det, "quantization_level"),
                modificado: texto(m, "modified_at"),
                familia,
                nombre,
            });
        }
        salida.sort_by(|a, b| a.nombre.cmp(&b.nombre));
        Ok(salida)
    }

    /// Los modelos que están en memoria ahora (`/api/ps`).
    pub fn cargados(&self) -> Result<Vec<Cargado>, ErrorOllama> {
        let v = self.json("/api/ps", None, None, 10.0, "")?;
        let mut salida = Vec::new();
        for m in v.get("models").and_then(Value::as_array).into_iter().flatten() {
            let total = m.get("size").map(bytes).unwrap_or(0);
            let vram = m.get("size_vram").map(bytes).unwrap_or(0);
            salida.push(Cargado {
                nombre: m
                    .get("name")
                    .and_then(Value::as_str)
                    .or_else(|| m.get("model").and_then(Value::as_str))
                    .unwrap_or("")
                    .to_string(),
                bytes: total,
                bytes_vram: vram,
                en_gpu_pct: if total > 0 {
                    (100.0 * vram as f64 / total as f64).round() as i64
                } else {
                    0
                },
                expira: m
                    .get("expires_at")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
        }
        Ok(salida)
    }

    /// Baja un modelo (`/api/pull`). `al_progreso(completado, total, estado)` con los bytes de todas las capas.
    pub fn bajar(
        &self,
        nombre: &str,
        mut al_progreso: Option<AlProgreso>,
        cancelar: Option<&Cancelar>,
    ) -> Result<(), ErrorOllama> {
        let nombre = nombre.trim();
        if nombre.is_empty() {
            return Err(ErrorOllama::new("Falta el nombre del modelo.", "modelo"));
        }
        let mut capas: Vec<(String, i64, i64)> = Vec::new();
        let mut listo = false;
        self.flujo(
            "/api/pull",
            &json!({"model": nombre, "stream": true}),
            cancelar,
            120.0,
            nombre,
            |evento| {
                let estado_en = evento.get("status").and_then(Value::as_str).unwrap_or("");
                let estado = if let Some(digest) = evento.get("digest").and_then(Value::as_str) {
                    let total = evento.get("total").map(bytes).unwrap_or(0);
                    let completado = evento.get("completed").map(bytes).unwrap_or(0);
                    match capas.iter_mut().find(|c| c.0 == digest) {
                        Some(c) => {
                            c.1 = c.1.max(completado);
                            c.2 = c.2.max(total);
                        }
                        None => capas.push((digest.to_string(), completado, total)),
                    }
                    "Bajando".to_string()
                } else {
                    match estado_legible(estado_en) {
                        Some(e) => e.to_string(),
                        None if estado_en.is_empty() => "Bajando".to_string(),
                        None => capitalizar_primera(estado_en),
                    }
                };
                if estado_en == "success" {
                    listo = true;
                }
                if let Some(f) = al_progreso.as_mut() {
                    let mut completado: i64 = capas.iter().map(|c| c.1).sum();
                    let total: i64 = capas.iter().map(|c| c.2).sum();
                    if listo {
                        completado = total;
                    }
                    f(completado, total, &estado);
                }
                Ok(())
            },
        )?;
        if !listo {
            return Err(ErrorOllama::new(
                format!(
                    "La descarga de «{nombre}» se cortó antes de terminar. Vuelve a intentar: \
                     sigue donde se quedó."
                ),
                "red",
            ));
        }
        Ok(())
    }

    /// Borra un modelo instalado (`/api/delete`).
    pub fn borrar(&self, nombre: &str) -> Result<(), ErrorOllama> {
        self.json(
            "/api/delete",
            Some(&json!({"model": nombre})),
            Some("DELETE"),
            10.0,
            nombre,
        )
        .map(|_| ())
    }

    /// Saca el modelo de la memoria ya (`keep_alive=0`), sin borrarlo.
    pub fn descargar_de_gpu(&self, modelo: &str) {
        let _ = self.json(
            "/api/generate",
            Some(&json!({"model": modelo, "keep_alive": 0})),
            None,
            5.0,
            modelo,
        );
    }

    // ---------------------------------------------------------------- chat y embeddings

    /// Platica con un modelo (`/api/chat`) en streaming. Regresa la respuesta sin el pensamiento.
    ///
    /// `al_trozo(texto)` recibe cada trozo de la respuesta; `al_pensar(texto)` cada trozo del pensamiento
    /// (`<think>…</think>` o el campo «thinking»). Si se cancela, regresa lo que llevaba.
    pub fn chat(
        &self,
        modelo: &str,
        mensajes: &[Value],
        mut al_trozo: Option<&mut dyn FnMut(&str)>,
        cancelar: Option<&Cancelar>,
        opciones: Option<&Value>,
        mut al_pensar: Option<&mut dyn FnMut(&str)>,
    ) -> Result<String, ErrorOllama> {
        let mut datos = json!({"model": modelo, "messages": mensajes, "stream": true});
        if let Some(o) = opciones.filter(|o| o.as_object().is_some_and(|m| !m.is_empty())) {
            datos["options"] = o.clone();
        }
        let mut respuesta = String::new();
        let mut separador = SeparadorPensar::default();
        *self.estadisticas.lock().unwrap() = Estadisticas::default();

        fn soltar(
            partes: Vec<(bool, String)>,
            respuesta: &mut String,
            al_trozo: &mut Option<&mut dyn FnMut(&str)>,
            al_pensar: &mut Option<&mut dyn FnMut(&str)>,
        ) {
            for (es_pensamiento, texto) in partes {
                if texto.is_empty() {
                    continue;
                }
                if es_pensamiento {
                    if let Some(f) = al_pensar.as_mut() {
                        f(&texto);
                    }
                } else {
                    respuesta.push_str(&texto);
                    if let Some(f) = al_trozo.as_mut() {
                        f(&texto);
                    }
                }
            }
        }

        let resultado = self.flujo("/api/chat", &datos, cancelar, 600.0, modelo, |evento| {
            let mensaje = evento.get("message").cloned().unwrap_or(Value::Null);
            if let Some(p) = mensaje
                .get("thinking")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
                && let Some(f) = al_pensar.as_mut()
            {
                f(p);
            }
            if let Some(c) = mensaje
                .get("content")
                .and_then(Value::as_str)
                .filter(|c| !c.is_empty())
            {
                soltar(separador.meter(c), &mut respuesta, &mut al_trozo, &mut al_pensar);
            }
            if evento.get("done").and_then(Value::as_bool) == Some(true) {
                let tokens = evento.get("eval_count").map(bytes).unwrap_or(0);
                let segundos = evento.get("eval_duration").map(bytes).unwrap_or(0) as f64 / 1e9;
                *self.estadisticas.lock().unwrap() = Estadisticas {
                    tokens,
                    segundos: (segundos * 100.0).round() / 100.0,
                    tokens_por_segundo: if segundos > 0.0 {
                        (tokens as f64 / segundos * 10.0).round() / 10.0
                    } else {
                        0.0
                    },
                };
            }
            Ok(())
        });
        match resultado {
            Ok(()) => {}
            Err(e) if e.es_cancelado() => {}
            Err(e) => return Err(e),
        }
        soltar(
            separador.terminar(),
            &mut respuesta,
            &mut al_trozo,
            &mut al_pensar,
        );
        Ok(respuesta.trim().to_string())
    }

    /// Los vectores de cada texto (`/api/embed`; si el Ollama es viejo, `/api/embeddings` uno por uno).
    pub fn embeber(&self, modelo: &str, textos: &[String]) -> Result<Vec<Vec<f32>>, ErrorOllama> {
        if textos.is_empty() {
            return Ok(vec![]);
        }
        let vector = |v: &Value| -> Vec<f32> {
            v.as_array()
                .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())
                .unwrap_or_default()
        };
        let primero = self
            .json(
                "/api/embed",
                Some(&json!({"model": modelo, "input": textos})),
                None,
                300.0,
                modelo,
            )
            .and_then(|datos| match datos.get("embeddings").and_then(Value::as_array) {
                Some(v) if v.len() == textos.len() => Ok(v.iter().map(vector).collect()),
                _ => Err(ErrorOllama::otro("Ollama no regresó los embeddings.")),
            });
        match primero {
            Ok(v) => Ok(v),
            Err(e) => {
                if e.clave != "modelo" || e.mensaje.to_lowercase().contains("embeddings") {
                    return Err(e);
                }
                // Un 404 puede ser «no tienes el modelo» o un Ollama sin /api/embed: prueba la ruta vieja.
                let mut salida = Vec::new();
                for t in textos {
                    match self.json(
                        "/api/embeddings",
                        Some(&json!({"model": modelo, "prompt": t})),
                        None,
                        300.0,
                        modelo,
                    ) {
                        Ok(d) => salida.push(d.get("embedding").map(vector).unwrap_or_default()),
                        Err(_) => return Err(e),
                    }
                }
                Ok(salida)
            }
        }
    }
}

fn capitalizar_primera(t: &str) -> String {
    // str.capitalize() de Python: primera en mayúscula y el resto en minúscula.
    capitalizar(t)
}
