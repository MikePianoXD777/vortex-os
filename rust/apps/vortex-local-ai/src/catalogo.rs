//! El catálogo de la Vortexine: los modelos que recomendamos bajar, y cuáles le caben a tu PC (igual que
//! `catalogo.py`).
//!
//! Los nombres y tamaños salen de ollama.com/library (páginas de «tags», revisadas el 07/11/2026). `gb` es
//! la descarga en Linux. `vram_gb` es lo que necesita para ir completo en la tarjeta de video con el contexto
//! de 8K de Vortexine's. Los modelos «moe» (mezcla de expertos) aguantan mucho mejor ir en parte en la CPU.

use crate::ollama::en_path;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub const FUENTE: &str = "ollama.com/library (tags revisadas el 07/11/2026)";

/// `(id, nombre bonito)` de cada tipo, en el orden de la interfaz.
pub const TIPOS: [(&str, &str); 5] = [
    ("chat", "Platicar"),
    ("codigo", "Programar"),
    ("razonamiento", "Pensar a fondo"),
    ("vision", "Ver imágenes"),
    ("vault", "Para tu vault"),
];

/// El modelo de embeddings para «Preguntar a mi vault». Qwen3 Embedding entiende bien el español.
pub const EMBEDDINGS: &str = "qwen3-embedding:0.6b";

/// Con menos que esto no cabe entero ni el modelo más chico: mejor usar el procesador.
pub const VRAM_MINIMA_MB: i64 = 3380;
/// En el procesador manda la velocidad: los modelos que bajan más de esto ya se sienten lentos.
pub const CPU_MAX_GB: f64 = 3.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Entrada {
    pub id: &'static str,
    pub nombre: &'static str,
    pub descripcion: &'static str,
    pub para: &'static str,
    pub gb: f64,
    pub vram_gb: f64,
    pub contexto: &'static str,
    pub piensa: bool,
    pub moe: bool,
    pub vision: bool,
}

#[allow(clippy::too_many_arguments)]
const fn m(
    id: &'static str,
    nombre: &'static str,
    descripcion: &'static str,
    para: &'static str,
    gb: f64,
    vram_gb: f64,
    contexto: &'static str,
    piensa: bool,
    moe: bool,
    vision: bool,
) -> Entrada {
    Entrada {
        id,
        nombre,
        descripcion,
        para,
        gb,
        vram_gb,
        contexto,
        piensa,
        moe,
        vision,
    }
}

pub const CATALOGO: [Entrada; 18] = [
    // ---- Platicar
    m(
        "llama3.2:3b",
        "Llama 3.2 3B",
        "Chiquito y ligero: va en casi cualquier PC, hasta sin tarjeta de video. Bueno para dudas sencillas.",
        "chat",
        2.0,
        3.0,
        "128K",
        false,
        false,
        false,
    ),
    m(
        "qwen3.5:2b",
        "Qwen 3.5 2B",
        "Muy rápido y entiende imágenes. Para PCs modestas o laptops.",
        "chat",
        2.7,
        3.5,
        "256K",
        true,
        false,
        true,
    ),
    m(
        "qwen3.5:4b",
        "Qwen 3.5 4B",
        "Listo y ligero: habla bien español, ve imágenes y va bien hasta sin tarjeta de video.",
        "chat",
        3.3,
        4.5,
        "256K",
        true,
        false,
        true,
    ),
    m(
        "gemma4:e2b",
        "Gemma 4 E2B",
        "El Gemma de Google en tamaño mini: ve imágenes y escucha audio, y va bien en laptops.",
        "chat",
        4.6,
        5.5,
        "128K",
        true,
        false,
        true,
    ),
    m(
        "qwen3:8b",
        "Qwen 3 8B",
        "Un todoterreno muy probado: platica, explica y piensa antes de contestar si se lo pides.",
        "chat",
        5.2,
        6.5,
        "40K",
        true,
        false,
        false,
    ),
    m(
        "qwen3.5:9b",
        "Qwen 3.5 9B",
        "El recomendado para la mayoría: listo, rápido, habla muy bien español y ve imágenes.",
        "chat",
        6.6,
        8.0,
        "256K",
        true,
        false,
        true,
    ),
    m(
        "gemma4:e4b",
        "Gemma 4 E4B",
        "El Gemma de Google mediano: buena plática, ve imágenes y escucha audio.",
        "chat",
        6.6,
        8.0,
        "128K",
        true,
        false,
        true,
    ),
    m(
        "gemma4:12b",
        "Gemma 4 12B",
        "Más inteligente que los medianos; ideal si tienes una tarjeta de 10-12 GB.",
        "chat",
        8.0,
        10.0,
        "256K",
        true,
        false,
        true,
    ),
    m(
        "qwen3.6:35b-a3b",
        "Qwen 3.6 35B-A3B",
        "Un modelo grande pero rápido (usa solo una parte a la vez). Para PCs con mucha memoria.",
        "chat",
        23.0,
        26.0,
        "256K",
        true,
        true,
        true,
    ),
    // ---- Pensar a fondo
    m(
        "deepseek-r1:8b",
        "DeepSeek R1 8B",
        "Piensa paso a paso antes de contestar: bueno para mate y problemas difíciles.",
        "razonamiento",
        5.2,
        6.5,
        "128K",
        true,
        false,
        false,
    ),
    m(
        "gpt-oss:20b",
        "GPT-OSS 20B",
        "El modelo abierto de OpenAI: razona muy bien y aguanta ir en parte en la CPU.",
        "razonamiento",
        14.0,
        16.0,
        "128K",
        true,
        true,
        false,
    ),
    m(
        "qwen3.6:27b",
        "Qwen 3.6 27B",
        "De los más listos que puedes correr en casa. Necesita una tarjeta de video grande.",
        "razonamiento",
        17.0,
        20.0,
        "256K",
        true,
        false,
        true,
    ),
    // ---- Programar
    m(
        "qwen2.5-coder:7b",
        "Qwen 2.5 Coder 7B",
        "Para programar: escribe, explica y arregla código. Ligero.",
        "codigo",
        4.7,
        6.0,
        "32K",
        false,
        false,
        false,
    ),
    m(
        "qwen3-coder:30b",
        "Qwen 3 Coder 30B",
        "El mejor para programar en tu PC: entiende proyectos grandes. Rápido aunque no quepa completo.",
        "codigo",
        19.0,
        22.0,
        "256K",
        false,
        true,
        false,
    ),
    // ---- Ver imágenes
    m(
        "qwen3-vl:4b",
        "Qwen 3 VL 4B",
        "Mira fotos y capturas de pantalla y te dice qué hay, o lee el texto que tienen.",
        "vision",
        3.3,
        4.5,
        "256K",
        true,
        false,
        true,
    ),
    m(
        "qwen3-vl:8b",
        "Qwen 3 VL 8B",
        "Como el 4B pero más fino: entiende mejor gráficas, documentos y fotos complicadas.",
        "vision",
        6.2,
        8.0,
        "256K",
        true,
        false,
        true,
    ),
    // ---- Para tu vault (embeddings: no platican, convierten tus notas en números para buscarlas)
    m(
        "qwen3-embedding:0.6b",
        "Qwen 3 Embedding 0.6B",
        "Le enseña a la IA a buscar en tus notas. Entiende español. Es el que usa «Preguntar a mi vault».",
        "vault",
        0.64,
        1.2,
        "32K",
        false,
        false,
        false,
    ),
    m(
        "nomic-embed-text",
        "Nomic Embed Text",
        "Otro buscador de notas, más chiquito; funciona mejor con notas en inglés.",
        "vault",
        0.27,
        0.6,
        "2K",
        false,
        false,
        false,
    ),
];

/// El nombre bonito de un tipo («chat» → «Platicar»).
pub fn nombre_de_tipo(tipo: &str) -> Option<&'static str> {
    TIPOS.iter().find(|t| t.0 == tipo).map(|t| t.1)
}

/// Todo el catálogo, en orden.
pub fn todos() -> impl Iterator<Item = &'static Entrada> {
    CATALOGO.iter()
}

/// El modelo del catálogo con ese id (acepta «x» como «x:latest»).
pub fn por_id(id: &str) -> Option<&'static Entrada> {
    if id.is_empty() {
        return None;
    }
    todos().find(|m| m.id == id || format!("{}:latest", m.id) == id || m.id == format!("{id}:latest"))
}

/// Los modelos del catálogo de un tipo («chat», «codigo», …).
pub fn de_tipo(para: &str) -> Vec<&'static Entrada> {
    todos().filter(|m| m.para == para).collect()
}

// ------------------------------------------------------------------ tu memoria de video

/// La memoria con la que puede correr los modelos.
#[derive(Debug, Clone, PartialEq)]
pub struct Vram {
    pub nombre: String,
    pub total_mb: i64,
    pub libre_mb: i64,
    /// «nvidia», «amd», «intel» o «cpu».
    pub tipo: String,
    pub ram_mb: i64,
    pub de_entrada: bool,
}

/// `(total_mb, libre_mb)` de la RAM.
fn ram() -> (i64, i64) {
    ram_de(Path::new("/proc/meminfo"))
}

fn ram_de(ruta: &Path) -> (i64, i64) {
    let (mut total, mut libre) = (0, 0);
    if let Ok(t) = std::fs::read_to_string(ruta) {
        for linea in t.lines() {
            if let Some((clave, valor)) = linea.split_once(':') {
                let kb = valor
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<i64>().ok());
                match (clave, kb) {
                    ("MemTotal", Some(kb)) => total = kb / 1024,
                    ("MemAvailable", Some(kb)) => libre = kb / 1024,
                    _ => {}
                }
            }
        }
    }
    (total, libre)
}

/// Corre un programa y regresa `(código, salida)`; `None` si no existe o tarda más de `espera`.
fn correr(programa: &str, args: &[&str], espera: Duration) -> Option<(i32, String)> {
    en_path(programa)?;
    let mut hijo = Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // La salida se lee en otro hilo para que un programa que escribe mucho no se quede esperando.
    let mut salida = hijo.stdout.take()?;
    let lector = std::thread::spawn(move || {
        let mut t = String::new();
        let _ = std::io::Read::read_to_string(&mut salida, &mut t);
        t
    });
    let limite = std::time::Instant::now() + espera;
    let codigo = loop {
        match hijo.try_wait() {
            Ok(Some(s)) => break s.code().unwrap_or(-1),
            Ok(None) if std::time::Instant::now() < limite => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return None;
            }
        }
    };
    Some((codigo, lector.join().unwrap_or_default()))
}

/// La pregunta por el motor de la IA (CUDA en NVIDIA) si toca hacerla, o `None`. Solo la hace Local AI.
pub fn motor_pendiente() -> Option<String> {
    let (codigo, salida) = correr("vortex-ia", &["motor", "--pendiente"], Duration::from_secs(15))?;
    if codigo != 0 {
        return None;
    }
    let datos: Value = serde_json::from_str(&salida).ok()?;
    if datos.get("preguntar").is_some_and(verdadero) {
        let texto = datos.get("texto").and_then(Value::as_str).unwrap_or("");
        if !texto.is_empty() {
            return Some(texto.to_string());
        }
    }
    None
}

fn verdadero(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Null => false,
    }
}

/// La respuesta a esa pregunta: «ok», «cancelado» (sin contraseña: se vuelve a preguntar) o «error».
pub fn elegir_motor(si: bool) -> String {
    let arg = if si { "--si" } else { "--no" };
    // Sin límite de tiempo: puede pedir la contraseña y bajar CUDA.
    let Ok(estado) = en_path("vortex-ia")
        .map(|_| {
            Command::new("vortex-ia")
                .args(["motor", arg])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        })
        .unwrap_or(Err(std::io::Error::other("no está")))
    else {
        return "error".into();
    };
    match estado.code() {
        Some(0) => "ok",
        Some(2) => "cancelado",
        _ => "error",
    }
    .into()
}

fn numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
}

/// Lo que diga `vortex-ia modelos --json`, si existe (lo hace otra parte de Vortexine's).
fn de_vortex_ia(ram_mb: i64) -> Option<Vram> {
    let (codigo, salida) = correr("vortex-ia", &["modelos", "--json"], Duration::from_secs(8))?;
    if codigo != 0 {
        return None;
    }
    let datos: Value = serde_json::from_str(&salida).ok()?;
    if !datos.is_object() {
        return None;
    }
    let maquina = datos.get("maquina").filter(|m| m.is_object());
    let candidatos = [
        Some(&datos),
        datos.get("vram"),
        datos.get("gpu"),
        datos.get("tarjeta"),
        maquina.and_then(|m| m.get("gpu")),
    ];
    for c in candidatos.into_iter().flatten() {
        let Some(obj) = c.as_object() else { continue };
        let mut total = obj.get("total_mb").and_then(numero).unwrap_or(0.0);
        let mut libre = obj.get("libre_mb").and_then(numero).unwrap_or(0.0);
        if total == 0.0
            && let Some(gb) = obj.get("vram_gb").filter(|v| verdadero(v))
        {
            // vortex-ia da {"maquina": {"gpu": {"tipo", "nombre", "vram_gb", "libre_gb", "de_entrada"}}}
            let gb = numero(gb)?;
            total = (gb * 1024.0).trunc();
            libre = (obj.get("libre_gb").and_then(numero).unwrap_or(0.0) * 1024.0).trunc();
        }
        if total != 0.0 {
            let tipo = obj
                .get("tipo")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .unwrap_or("cpu")
                .to_lowercase();
            let nombre = obj
                .get("nombre")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .unwrap_or("Tarjeta de video");
            return Some(Vram {
                nombre: nombre.to_string(),
                total_mb: total as i64,
                libre_mb: if libre != 0.0 { libre as i64 } else { total as i64 },
                tipo: if ["nvidia", "amd", "intel", "cpu"].contains(&tipo.as_str()) {
                    tipo
                } else {
                    "cpu".into()
                },
                ram_mb,
                de_entrada: obj.get("de_entrada").is_some_and(verdadero),
            });
        }
    }
    None
}

fn de_nvidia(ram_mb: i64) -> Option<Vram> {
    let (codigo, salida) = correr(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ],
        Duration::from_secs(8),
    )?;
    let mut mejor: Option<Vram> = None;
    if codigo != 0 {
        return None;
    }
    for linea in salida.lines() {
        let p: Vec<&str> = linea.split(',').map(str::trim).collect();
        if p.len() < 3 {
            continue;
        }
        let (Ok(total), Ok(libre)) = (p[1].parse::<f64>(), p[2].parse::<f64>()) else {
            continue;
        };
        if mejor.as_ref().is_none_or(|m| total as i64 > m.total_mb) {
            mejor = Some(Vram {
                nombre: p[0].to_string(),
                total_mb: total as i64,
                libre_mb: libre as i64,
                tipo: "nvidia".into(),
                ram_mb,
                de_entrada: false,
            });
        }
    }
    mejor
}

/// AMD (y las Intel Arc) dicen su VRAM en `/sys/class/drm/cardN/device/mem_info_vram_*`.
pub fn de_sysfs(ram_mb: i64, ram_libre: i64, drm: &Path) -> Option<Vram> {
    let mut mejor: Option<Vram> = None;
    let mut integrada: Option<&str> = None;
    let mut tarjetas: Vec<_> = std::fs::read_dir(drm)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| {
            n.strip_prefix("card")
                .is_some_and(|r| r.chars().next().is_some_and(|c| c.is_ascii_digit()))
                && !n.contains('-') // card0-HDMI-A-1: son las salidas, no la tarjeta
        })
        .collect();
    tarjetas.sort();
    for t in tarjetas {
        let dispositivo = drm.join(&t).join("device");
        let Ok(vendedor) = std::fs::read_to_string(dispositivo.join("vendor")) else {
            continue;
        };
        let tipo = match vendedor.trim() {
            "0x10de" => "nvidia",
            "0x1002" => "amd",
            "0x8086" => "intel",
            _ => continue,
        };
        let leer = |n: &str| -> Option<i64> {
            std::fs::read_to_string(dispositivo.join(n))
                .ok()?
                .trim()
                .parse()
                .ok()
        };
        let (Some(total), Some(usada)) = (leer("mem_info_vram_total"), leer("mem_info_vram_used")) else {
            if tipo == "intel" {
                integrada = Some("intel");
            }
            continue;
        };
        let (total, usada) = (total / (1024 * 1024), usada / (1024 * 1024));
        if total < 1024 {
            // una integrada con su pedacito reservado: usa la RAM compartida
            integrada = integrada.or(Some(tipo));
            continue;
        }
        let nombre = match tipo {
            "amd" => "Tarjeta AMD",
            "intel" => "Tarjeta Intel",
            _ => "Tarjeta NVIDIA",
        };
        if mejor.as_ref().is_none_or(|m| total > m.total_mb) {
            mejor = Some(Vram {
                nombre: nombre.into(),
                total_mb: total,
                libre_mb: (total - usada).max(0),
                tipo: tipo.into(),
                ram_mb,
                de_entrada: false,
            });
        }
    }
    if mejor.is_none()
        && let Some(tipo) = integrada
        && ram_mb > 0
    {
        // La integrada comparte la RAM: Ollama (Vulkan) puede usar más o menos la mitad.
        let bonito = if tipo == "amd" {
            "AMD".to_string()
        } else {
            let mut c = tipo.chars();
            c.next()
                .map(|p| p.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };
        return Some(Vram {
            nombre: format!("Gráficos integrados {bonito}"),
            total_mb: ram_mb / 2,
            libre_mb: ram_libre / 2,
            tipo: tipo.into(),
            ram_mb,
            de_entrada: false,
        });
    }
    mejor
}

/// La memoria con la que puede correr los modelos.
///
/// Primero `vortex-ia modelos --json` (si existe), luego `nvidia-smi`, luego `/sys/class/drm` (AMD, Intel
/// Arc), y si no hay tarjeta o se queda corta para la IA, la RAM (tipo «cpu»). `VORTEX_LOCAL_AI_VRAM_MB=<n>` la
/// fija (para pruebas).
pub fn vram_disponible() -> Vram {
    let (ram_mb, ram_libre) = ram();
    if let Some(mb) = std::env::var("VORTEX_LOCAL_AI_VRAM_MB")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
    {
        return Vram {
            nombre: "Tarjeta de prueba".into(),
            total_mb: mb,
            libre_mb: mb,
            tipo: "nvidia".into(),
            ram_mb,
            de_entrada: false,
        };
    }
    let buscadores: [&dyn Fn() -> Option<Vram>; 3] =
        [&|| de_vortex_ia(ram_mb), &|| de_nvidia(ram_mb), &|| {
            de_sysfs(ram_mb, ram_libre, Path::new("/sys/class/drm"))
        }];
    for buscar in buscadores {
        if let Some(r) = buscar() {
            if r.de_entrada || r.total_mb < VRAM_MINIMA_MB {
                return Vram {
                    nombre: "Procesador (tu tarjeta de video se queda corta)".into(),
                    total_mb: ram_mb,
                    libre_mb: ram_libre,
                    tipo: "cpu".into(),
                    ram_mb,
                    de_entrada: true,
                };
            }
            return r;
        }
    }
    Vram {
        nombre: "Procesador (sin tarjeta de video)".into(),
        total_mb: ram_mb,
        libre_mb: ram_libre,
        tipo: "cpu".into(),
        ram_mb,
        de_entrada: false,
    }
}

// ------------------------------------------------------------------ ¿te cabe?

/// Lo que ocupa un modelo, venga de donde venga.
#[derive(Debug, Clone, Copy)]
pub enum Medida<'a> {
    /// Un id o nombre de Ollama («qwen3.5:9b»).
    Id(&'a str),
    /// Una entrada del catálogo.
    Entrada(&'a Entrada),
    /// Un modelo instalado, por lo que pesa en disco.
    Bytes(i64),
}

/// `(GB que necesita, es_moe)`.
pub fn necesita_gb(modelo: Medida) -> (f64, bool) {
    match modelo {
        Medida::Id(id) => por_id(id).map_or((0.0, false), |e| (e.vram_gb, e.moe)),
        Medida::Entrada(e) => (e.vram_gb, e.moe),
        Medida::Bytes(b) if b > 0 => (b as f64 / 1e9 * 1.15 + 0.8, false),
        Medida::Bytes(_) => (0.0, false),
    }
}

/// «si» (va completo y holgado), «justo» (con poco margen, o una parte en la CPU y más lento) o «no».
pub fn cabe(modelo: Medida, vram: &Vram) -> &'static str {
    let (necesita, moe) = necesita_gb(modelo);
    let total = vram.total_mb as f64 / 1024.0;
    let ram = vram.ram_mb as f64 / 1024.0;
    if necesita <= 0.0 || total <= 0.0 {
        return "no";
    }
    if vram.tipo == "cpu" {
        // Sin tarjeta todo va en la RAM: deja espacio para el sistema y tus apps.
        if necesita <= total * 0.5 {
            return "si";
        }
        return if necesita <= total * 0.75 { "justo" } else { "no" };
    }
    if necesita <= total - 0.3 {
        return "si";
    }
    // Lo que no cabe en la tarjeta se va a la RAM. Un moe lo aguanta bien; uno normal se pone lento rápido.
    let extra = (ram * 0.5).min(total * if moe { 1.5 } else { 0.3 });
    if necesita <= total + extra { "justo" } else { "no" }
}

/// El mejor modelo para platicar que te cabe completo; si ninguno, el más chico. Sin gráfica, uno ligero.
pub fn recomendado(vram: &Vram) -> &'static Entrada {
    let platica = de_tipo("chat");
    let mut caben: Vec<&Entrada> = platica
        .iter()
        .copied()
        .filter(|m| cabe(Medida::Entrada(m), vram) == "si")
        .collect();
    if vram.tipo == "cpu" {
        caben.retain(|m| m.gb <= CPU_MAX_GB);
    }
    // max() de Python se queda con el primero si hay empate.
    let mejor = caben.iter().copied().fold(None::<&Entrada>, |a, m| match a {
        Some(x) if x.vram_gb >= m.vram_gb => Some(x),
        _ => Some(m),
    });
    mejor.unwrap_or_else(|| {
        platica
            .iter()
            .copied()
            .fold(platica[0], |a, m| if m.vram_gb < a.vram_gb { m } else { a })
    })
}
