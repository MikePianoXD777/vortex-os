//! El catálogo de Vortexine's Shop: todo lo que hay en Flathub, leído de su API pública.
//!
//! ```text
//! https://flathub.org/api/v2/   (documentada en https://flathub.org/api/v2/openapi.json)
//!
//! GET  /collection/trending|popular?page=&per_page=&locale=   listas de apps (Meilisearch)
//! GET  /collection/category                                    ids de las categorías
//! GET  /collection/category/{id}?page=&per_page=&sort_by=&locale=
//! POST /search?locale=   {"query", "hits_per_page", "page"}   búsqueda en todo el catálogo
//! GET  /appstream/{app_id}?locale=                             ficha completa (descripción, capturas)
//! GET  /summary/{app_id}                                       tamaño de descarga
//! GET  /app-picks/app-of-the-day/{AAAA-MM-DD}                  la app del día y las de la semana
//! GET  /app-picks/apps-of-the-week/{AAAA-MM-DD}
//! ```
//!
//! La categoría «IA» no existe en Flathub: es una lista de Vortexine's ([`IA`]) que se arma con las fichas
//! de `/appstream` y sale primero, destacada.
//!
//! El idioma se pide con `locale` y Flathub traduce lo que tenga traducido.
//!
//! Red: el Python usa `urllib`; aquí se llama a `curl` (ya viene en el sistema y trae TLS), con timeout de
//! 12 s, User-Agent `vortexines-shop/VERSIÓN` y caché en disco (un JSON por petición): vale 1 h para
//! listas y búsquedas, 24 h para fichas. Si no hay red y hay caché, se usa aunque esté vieja; si no hay
//! nada, `SinRed`. Para probar sin internet, `VORTEX_SHOP_API` apunta a otro servidor con la misma forma.

use crate::AGENTE;
use crate::errores::{Error, Resultado};
use crate::ids::es_id_valido;
use crate::proceso::{self, Entorno, Fallo};
use crate::texto::html_a_texto;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

pub const API_POR_DEFECTO: &str = "https://flathub.org/api/v2";
/// Segundos de timeout por petición.
pub const ESPERA: f64 = 12.0;
/// 1 h: listas, categorías por página y búsquedas.
pub const TTL_LISTAS: f64 = 3600.0;
/// 24 h: fichas, tamaños y la lista de categorías.
pub const TTL_DETALLES: f64 = 24.0 * 3600.0;
/// Apps por página (en categorías y búsquedas).
pub const POR_PAGINA: usize = 30;
pub const MINIMO_DESTACADOS: usize = 6;
const MAXIMO_RESPUESTA: usize = 16 * 1024 * 1024;

/// Lo que Vortexine's recomienda (verificado: todos existen en Flathub). Mismo orden en pantalla.
pub const RECOMENDADAS: [&str; 25] = [
    "com.valvesoftware.Steam",
    "org.prismlauncher.PrismLauncher",
    "org.vinegarhq.Sober",
    "com.heroicgameslauncher.hgl",
    "com.usebottles.bottles",
    "net.lutris.Lutris",
    "net.davidotek.pupgui2",
    "sh.ppy.osu",
    "org.pegasus_frontend.Pegasus",
    "org.libretro.RetroArch",
    "com.discordapp.Discord",
    "dev.vencord.Vesktop",
    "org.telegram.desktop",
    "us.zoom.Zoom",
    "org.mozilla.firefox",
    "com.brave.Browser",
    "com.spotify.Client",
    "org.videolan.VLC",
    "com.obsproject.Studio",
    "org.kde.kdenlive",
    "org.gimp.GIMP",
    "org.libreoffice.LibreOffice",
    "org.b3log.siyuan",
    "com.github.wwmm.easyeffects",
    "com.github.tchx84.Flatseal",
];

/// La categoría «IA»: apps para correr IA en tu PC, sin cuenta y sin internet después de bajarlas. No es
/// de Flathub: la arma la tienda con esta lista, en este orden, y sale primero en la barra lateral.
pub const CATEGORIA_IA: &str = "ia";

/// Una app de la categoría «IA» con lo que le pone Vortexine's.
pub struct DatosIa {
    pub id: &'static str,
    /// Lo que se ve en la tarjeta y en la ficha, para alguien que no sabe nada de IA.
    pub resumen: &'static str,
    /// Qué gráfica le conviene (sale en la ficha, en «Para tu PC»).
    pub grafica: &'static str,
    /// Un dato extra para la ficha, si hace falta.
    pub nota: &'static str,
}

pub const IA: [DatosIa; 8] = [
    DatosIa {
        id: "com.jeffser.Alpaca",
        resumen: "Platica con una IA en tu PC, sin internet. Puede usar el Ollama que ya trae Vortexine's.",
        grafica: "Corre en cualquier PC; con una gráfica NVIDIA o AMD responde mucho más rápido.",
        nota: "Para usar el Ollama de Vortexine's, en Alpaca agrega una instancia «Ollama» con la dirección http://localhost:11434. El Ollama propio de Alpaca es un complemento aparte de casi 2 GB: no hace falta.",
    },
    DatosIa {
        id: "ai.jan.Jan",
        resumen: "Un asistente como ChatGPT que corre completo en tu PC. Los modelos se bajan desde la app.",
        grafica: "Funciona solo con el procesador; con una gráfica NVIDIA (o AMD con Vulkan) va mucho más rápido.",
        nota: "Trae su propio motor de IA: no usa el Ollama del sistema.",
    },
    DatosIa {
        id: "ai.lmstudio.lm-studio",
        resumen: "Busca, baja y prueba modelos de IA con un clic; te avisa cuáles caben en tu PC.",
        grafica: "Mejor con 16 GB de RAM y una gráfica de 8 GB o más; con menos, usa modelos chicos.",
        nota: "Es gratis, pero de código cerrado. Trae su propio motor: no usa el Ollama del sistema.",
    },
    DatosIa {
        id: "io.gpt4all.gpt4all",
        resumen: "Chatea con una IA en tu PC y hazle preguntas sobre tus propios documentos.",
        grafica: "Corre bien hasta sin gráfica; con una NVIDIA o AMD (Vulkan) va más rápido.",
        nota: "Trae su propio motor: no usa el Ollama del sistema.",
    },
    DatosIa {
        id: "dev.ibrahimcetin.reins",
        resumen: "Un chat ligero y sencillo para el Ollama que ya trae Vortexine's.",
        grafica: "La rapidez la pone Ollama: con una gráfica NVIDIA o AMD responde más rápido.",
        nota: "Pesa muy poco porque no trae motor propio: usa el Ollama del sistema (http://localhost:11434).",
    },
    DatosIa {
        id: "net.mkiol.SpeechNote",
        resumen: "Dicta y la IA escribe lo que dices; también lee textos en voz alta y traduce, sin internet.",
        grafica: "Funciona sin gráfica; con una NVIDIA el dictado es más rápido.",
        nota: "Los idiomas y las voces se bajan dentro de la app, la primera vez.",
    },
    DatosIa {
        id: "org.upscayl.Upscayl",
        resumen: "Agranda tus fotos y dibujos con IA sin que se vean pixeleados.",
        grafica: "Necesita una gráfica con Vulkan: casi todas las NVIDIA, AMD e Intel recientes.",
        nota: "",
    },
    DatosIa {
        id: "io.gitlab.theevilskeleton.Upscaler",
        resumen: "Mejora y agranda imágenes con IA. Sencilla y muy ligera.",
        grafica: "Usa la gráfica (Vulkan); con una integrada también funciona, solo que más lento.",
        nota: "",
    },
];
pub const INTRO_IA: &str = "Corre modelos de IA en tu PC: sin cuenta y sin internet después de bajarlos. Para ver cuáles te caben, abre una terminal y escribe:";
pub const COMANDO_IA: &str = "vortex-ia modelos";

/// Categorías de Flathub (las que devuelve `/collection/category`) con su nombre y su ícono simbólico.
/// El orden es el de la pantalla; las que Flathub agregue después salen al final con su id.
const CATEGORIAS: [(&str, &str, &str); 11] = [
    ("game", "Juegos", "applications-games-symbolic"),
    ("audiovideo", "Audio y vídeo", "applications-multimedia-symbolic"),
    ("graphics", "Gráficos y fotos", "applications-graphics-symbolic"),
    ("development", "Programación", "applications-engineering-symbolic"),
    ("office", "Oficina", "applications-office-symbolic"),
    ("network", "Internet y chat", "applications-internet-symbolic"),
    ("utility", "Utilidades", "applications-utilities-symbolic"),
    ("system", "Sistema", "applications-system-symbolic"),
    ("education", "Educación", "applications-education-symbolic"),
    ("science", "Ciencia", "applications-science-symbolic"),
    ("healthfitness", "Salud y deporte", "applications-health-symbolic"),
];
const CATEGORIA_NUEVA: &str = "applications-other-symbolic";

/// Categorías de la ficha (las del .desktop de la app) en español. Las que no están aquí no se muestran.
const ETIQUETAS: &[(&str, &str)] = &[
    ("AudioVideo", "Audio y vídeo"),
    ("Audio", "Audio"),
    ("Video", "Vídeo"),
    ("Development", "Programación"),
    ("Education", "Educación"),
    ("Game", "Juegos"),
    ("Graphics", "Gráficos"),
    ("Network", "Internet"),
    ("Office", "Oficina"),
    ("Science", "Ciencia"),
    ("Settings", "Configuración"),
    ("System", "Sistema"),
    ("Utility", "Utilidades"),
    ("ActionGame", "Acción"),
    ("AdventureGame", "Aventura"),
    ("ArcadeGame", "Arcade"),
    ("BoardGame", "Juegos de mesa"),
    ("BlocksGame", "Bloques"),
    ("CardGame", "Cartas"),
    ("KidsGame", "Infantil"),
    ("LogicGame", "Lógica"),
    ("RolePlaying", "Rol"),
    ("Shooter", "Disparos"),
    ("Simulation", "Simulación"),
    ("SportsGame", "Deportes"),
    ("StrategyGame", "Estrategia"),
    ("Emulator", "Emuladores"),
    ("Music", "Música"),
    ("Player", "Reproductores"),
    ("Recorder", "Grabación"),
    ("Midi", "MIDI"),
    ("Mixer", "Mezcla"),
    ("Sequencer", "Secuenciadores"),
    ("Tuner", "Afinadores"),
    ("AudioVideoEditing", "Edición de audio y vídeo"),
    ("Photography", "Fotografía"),
    ("RasterGraphics", "Imágenes"),
    ("VectorGraphics", "Gráficos vectoriales"),
    ("3DGraphics", "Gráficos 3D"),
    ("Scanning", "Escáner"),
    ("OCR", "Reconocimiento de texto"),
    ("Viewer", "Visores"),
    ("Chat", "Chat"),
    ("InstantMessaging", "Mensajería"),
    ("Email", "Correo"),
    ("WebBrowser", "Navegadores"),
    ("FileTransfer", "Descargas"),
    ("P2P", "P2P"),
    ("VideoConference", "Videollamadas"),
    ("Telephony", "Llamadas"),
    ("IRCClient", "IRC"),
    ("News", "Noticias"),
    ("RemoteAccess", "Acceso remoto"),
    ("Feed", "Noticias"),
    ("TextEditor", "Editores de texto"),
    ("IDE", "Entornos de desarrollo"),
    ("Debugger", "Depuración"),
    ("WebDevelopment", "Desarrollo web"),
    ("Calculator", "Calculadoras"),
    ("Math", "Matemáticas"),
    ("Calendar", "Calendario"),
    ("Spreadsheet", "Hojas de cálculo"),
    ("WordProcessor", "Procesadores de texto"),
    ("Presentation", "Presentaciones"),
    ("Database", "Bases de datos"),
    ("Dictionary", "Diccionarios"),
    ("Archiving", "Archivos comprimidos"),
    ("FileManager", "Archivos"),
    ("TerminalEmulator", "Terminales"),
    ("Monitor", "Monitores"),
    ("Security", "Seguridad"),
    ("Languages", "Idiomas"),
    ("Astronomy", "Astronomía"),
    ("Biology", "Biología"),
    ("Chemistry", "Química"),
    ("Physics", "Física"),
    ("Geography", "Geografía"),
    ("Maps", "Mapas"),
    ("Art", "Arte"),
    ("Electronics", "Electrónica"),
    ("Engineering", "Ingeniería"),
];

/// Tipos que el índice de Flathub puede traer y que no son una app que se pueda abrir.
const NO_APPS: [&str; 5] = ["runtime", "addon", "extension", "localization", "generic"];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct App {
    pub id: String,
    pub nombre: String,
    pub resumen: String,
    pub icono_url: String,
    pub desarrollador: String,
    pub verificado: bool,
    pub descargas_mes: Option<i64>,
    /// Solo las de la categoría «IA»: qué gráfica le conviene.
    pub grafica: String,
    /// Y un dato extra para la ficha.
    pub nota: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Categoria {
    pub id: String,
    pub nombre: String,
    pub icono: String,
    /// Texto de arriba de la página de la categoría.
    pub intro: String,
    /// Un comando para la terminal que va debajo del texto.
    pub comando: String,
    /// Sobresale en la barra lateral y en el inicio.
    pub destacada: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppDetalle {
    pub app: App,
    pub descripcion: String,
    pub capturas: Vec<String>,
    pub licencia: String,
    pub web: Option<String>,
    pub version: Option<String>,
    pub tamano_descarga: Option<i64>,
    pub categorias: Vec<String>,
}

impl std::ops::Deref for AppDetalle {
    type Target = App;
    fn deref(&self) -> &App {
        &self.app
    }
}

impl std::ops::DerefMut for AppDetalle {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.app
    }
}

enum Descarga {
    /// El servidor dice que eso no existe (404 o 422).
    NoExiste,
    /// No se pudo hablar con el servidor (sin red, timeout, error 5xx, respuesta ilegible).
    Fallo,
}

fn texto(v: Option<&Value>) -> String {
    v.and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn entero(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => {
            let s = s.trim();
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse().ok())
                .flatten()
        }
        _ => None,
    }
}

fn verdadero(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.trim().eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn url_web(v: Option<&Value>) -> String {
    let t = texto(v);
    let min = t.to_ascii_lowercase();
    if min.starts_with("http://") || min.starts_with("https://") {
        t
    } else {
        String::new()
    }
}

/// La licencia SPDX de AppStream, legible. «LicenseRef-proprietary» (con o sin «=url») es software
/// propietario; las demás `LicenseRef-*` se muestran sin el prefijo.
fn licencia(v: Option<&Value>) -> String {
    let t = texto(v);
    if t.is_empty() {
        return "Sin información".into();
    }
    let t = t.split('=').next().unwrap_or("");
    if t.to_ascii_lowercase().starts_with("licenseref-proprietary") {
        return "Propietaria".into();
    }
    t.replace("LicenseRef-", "")
}

/// Una app de las listas y búsquedas de Flathub. `None` si el elemento no es una app usable.
fn app_de_hit(hit: &Value) -> Option<App> {
    let o = hit.as_object()?;
    let id = o.get("app_id")?.as_str()?;
    if !es_id_valido(id)
        || o.get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| NO_APPS.contains(&t))
    {
        return None;
    }
    let nombre = texto(o.get("name"));
    Some(App {
        id: id.to_string(),
        nombre: if nombre.is_empty() { id.to_string() } else { nombre },
        resumen: texto(o.get("summary")),
        icono_url: url_web(o.get("icon")),
        desarrollador: texto(o.get("developer_name")),
        verificado: verdadero(o.get("verification_verified")),
        descargas_mes: entero(o.get("installs_last_month")),
        ..App::default()
    })
}

/// Una app a partir de su ficha de `/appstream` (no trae el número de descargas).
fn app_de_appstream(app_id: &str, ficha: &Value) -> App {
    let meta = ficha.get("metadata").filter(|m| m.is_object());
    let nombre = texto(ficha.get("name"));
    App {
        id: app_id.to_string(),
        nombre: if nombre.is_empty() {
            app_id.to_string()
        } else {
            nombre
        },
        resumen: texto(ficha.get("summary")),
        icono_url: url_web(ficha.get("icon")),
        desarrollador: texto(ficha.get("developer_name")),
        verificado: verdadero(meta.and_then(|m| m.get("flathub::verification::verified"))),
        ..App::default()
    }
}

/// De los tamaños de una captura, el más grande que no pase de 1280 px de ancho (o el más chico si todos
/// pasan): la tienda no necesita bajar el original de 4K.
fn captura(sizes: Option<&Value>) -> String {
    let mut candidatas: Vec<(i64, String)> = Vec::new();
    for tam in sizes.and_then(Value::as_array).into_iter().flatten() {
        if !tam.is_object() {
            continue;
        }
        let src = url_web(tam.get("src"));
        let ancho = entero(tam.get("width")).unwrap_or(0);
        if !src.is_empty() {
            candidatas.push((ancho, src));
        }
    }
    let dentro = candidatas.iter().filter(|c| c.0 <= 1280).max();
    match dentro {
        Some(c) => c.1.clone(),
        None => candidatas.iter().min().map(|c| c.1.clone()).unwrap_or_default(),
    }
}

/// La versión de la publicación más reciente.
fn version(releases: Option<&Value>) -> Option<String> {
    let mut mejor: Option<String> = None;
    let mut marca = -1;
    for rel in releases.and_then(Value::as_array).into_iter().flatten() {
        let v = texto(rel.get("version"));
        if !rel.is_object() || v.is_empty() {
            continue;
        }
        let t = entero(rel.get("timestamp")).unwrap_or(-1);
        if mejor.is_none() || t > marca {
            mejor = Some(v);
            marca = t;
        }
    }
    mejor
}

/// La categoría «IA» (no es de Flathub: la arma la tienda con la lista [`IA`]).
pub fn categoria_ia() -> Categoria {
    Categoria {
        id: CATEGORIA_IA.into(),
        nombre: "IA".into(),
        icono: "vortex-ia-symbolic".into(),
        intro: INTRO_IA.into(),
        comando: COMANDO_IA.into(),
        destacada: true,
    }
}

/// La app con el resumen en español, la gráfica y la nota de la lista IA (si es de esa lista).
pub fn con_datos_ia(mut app: App) -> App {
    if let Some(d) = IA.iter().find(|d| d.id == app.id) {
        app.resumen = d.resumen.into();
        app.grafica = d.grafica.into();
        app.nota = d.nota.into();
    }
    app
}

fn id_categoria_valido(t: &str) -> bool {
    (1..=40).contains(&t.len())
        && t.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn idioma_valido(t: &str) -> bool {
    let (base, resto) = match t.find(['_', '-']) {
        Some(i) => (&t[..i], Some(&t[i + 1..])),
        None => (t, None),
    };
    (2..=3).contains(&base.len())
        && base.bytes().all(|b| b.is_ascii_alphabetic())
        && resto.is_none_or(|r| (2..=8).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// Como `urllib.parse.quote(t, safe="")`.
pub fn codificar(t: &str) -> String {
    let mut s = String::new();
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

fn codificar_mas(t: &str) -> String {
    codificar(t).replace("%20", "+")
}

/// Un hash de 128 bits (dos FNV-1a) en hexadecimal: el nombre del archivo de caché.
fn huella(t: &str) -> String {
    let (mut a, mut b) = (0xcbf29ce484222325_u64, 0x84222325cbf29ce4_u64);
    for byte in t.bytes() {
        a = (a ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        b = (b ^ u64::from(byte)).wrapping_mul(0x1000193).rotate_left(7);
    }
    format!("{a:016x}{b:016x}")
}

/// AAAA-MM-DD (UTC) de un momento en segundos desde 1970.
pub fn fecha_utc(segundos: f64) -> String {
    let dias = (segundos / 86400.0).floor() as i64;
    // algoritmo «civil_from_days» de Howard Hinnant
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", if m <= 2 { y + 1 } else { y })
}

fn ahora_real() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Lo que hay en Flathub. Todos los métodos pueden dar `SinRed` (sin internet y sin caché); `detalle`
/// también `NoEncontrada` si Flathub no tiene esa app.
pub struct Catalogo {
    pub idioma: String,
    pub api: String,
    pub cache_dir: PathBuf,
    /// Segundos de timeout por petición (las pruebas lo bajan).
    pub espera: f64,
    entorno: Entorno,
    reloj: Arc<dyn Fn() -> f64 + Send + Sync>,
}

impl Catalogo {
    pub const POR_PAGINA: usize = POR_PAGINA;

    /// `cache_dir`, `idioma` y `api` en `None` toman lo de siempre (`~/.cache/vortexines-shop`, «es» y
    /// `VORTEX_SHOP_API` o la API de Flathub).
    pub fn nuevo(cache_dir: Option<&Path>, idioma: &str, api: Option<&str>) -> Self {
        let idioma = idioma.trim();
        let idioma = if idioma_valido(idioma) { idioma } else { "es" }.to_string();
        let entorno_api = std::env::var("VORTEX_SHOP_API").ok();
        let base = api
            .map(str::to_string)
            .or(entorno_api)
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| API_POR_DEFECTO.into());
        let min = base.to_ascii_lowercase();
        let base = if min.starts_with("http://") || min.starts_with("https://") {
            base
        } else {
            API_POR_DEFECTO.into()
        };
        let cache_dir = cache_dir.map(Path::to_path_buf).unwrap_or_else(|| {
            let raiz = std::env::var_os("XDG_CACHE_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")
                });
            raiz.join("vortexines-shop")
        });
        Catalogo {
            idioma,
            api: base.trim_end_matches('/').to_string(),
            cache_dir,
            espera: ESPERA,
            entorno: Entorno::default(),
            reloj: Arc::new(ahora_real),
        }
    }

    /// Cambia el reloj (las pruebas simulan que pasó el tiempo).
    pub fn con_reloj(mut self, reloj: impl Fn() -> f64 + Send + Sync + 'static) -> Self {
        self.reloj = Arc::new(reloj);
        self
    }

    pub fn con_entorno(mut self, e: Entorno) -> Self {
        self.entorno = e;
        self
    }

    // ------------------------------------------------------------------ red y caché

    fn archivo_cache(&self, metodo: &str, url: &str, cuerpo: Option<&Value>) -> PathBuf {
        // `Value` guarda las claves ordenadas: es el «sort_keys» de Python.
        let h = format!(
            "{metodo} {url} {}",
            cuerpo.map(Value::to_string).unwrap_or_default()
        );
        self.cache_dir.join(format!("{}.json", huella(&h)))
    }

    /// `(momento, datos)` o `None` si no hay caché o está dañada.
    fn leer_cache(archivo: &Path) -> Option<(f64, Value)> {
        let t = std::fs::read_to_string(archivo).ok()?;
        let v: Value = serde_json::from_str(&t).ok()?;
        Some((v.get("t")?.as_f64()?, v.get("datos")?.clone()))
    }

    fn guardar_cache(&self, archivo: &Path, ahora: f64, datos: &Value) {
        // Se escribe en un archivo aparte y se renombra: otro hilo (o un corte de luz) nunca ve un JSON a medias.
        let tmp = archivo.with_extension(format!(
            "{}.{:?}.tmp",
            std::process::id(),
            std::thread::current().id()
        ));
        let hecho = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(&self.cache_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&self.cache_dir, std::fs::Permissions::from_mode(0o700));
            }
            std::fs::write(&tmp, json!({"t": ahora, "datos": datos}).to_string())?;
            std::fs::rename(&tmp, archivo)
        })();
        if hecho.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    fn descargar(&self, url: &str, cuerpo: Option<&Value>) -> Result<Value, Descarga> {
        let mut cmd = Command::new(self.entorno.buscar("curl").unwrap_or_else(|| "curl".into()));
        cmd.args(["-q", "-sS", "--proto", "=http,https", "--max-time"])
            .arg(format!("{:.3}", self.espera))
            .args(["--max-filesize", &MAXIMO_RESPUESTA.to_string()])
            .args(["-A", AGENTE, "-H", "Accept: application/json"])
            .args(["-w", "\n%{http_code}"]);
        let entrada = cuerpo.map(Value::to_string);
        if entrada.is_some() {
            cmd.args(["-H", "Content-Type: application/json", "--data-binary", "@-"]);
        }
        cmd.arg(url);
        self.entorno.aplicar(&mut cmd);
        let limite = Duration::from_secs_f64(self.espera + 5.0);
        let salida = match proceso::correr(cmd, entrada.as_deref().map(str::as_bytes), limite) {
            Ok(s) if s.bien() => s.stdout,
            Ok(_) | Err(Fallo::NoArranca(_) | Fallo::Tiempo) => return Err(Descarga::Fallo),
        };
        let (crudo, codigo) = salida.rsplit_once('\n').ok_or(Descarga::Fallo)?;
        let codigo: u16 = codigo.trim().parse().map_err(|_| Descarga::Fallo)?;
        match codigo {
            404 | 422 => return Err(Descarga::NoExiste),
            200..=299 => {}
            _ => return Err(Descarga::Fallo),
        }
        if crudo.len() > MAXIMO_RESPUESTA {
            return Err(Descarga::Fallo);
        }
        serde_json::from_str(crudo).map_err(|_| Descarga::Fallo)
    }

    /// JSON de la ruta: de la caché si es reciente; si no, de la red (y se guarda). Con la red caída se
    /// usa la caché aunque esté vieja. `NoEncontrada` si el servidor dice que no existe; `SinRed` si no
    /// hay red ni caché.
    fn pedir(
        &self,
        ruta: &str,
        ttl: f64,
        params: &[(&str, String)],
        cuerpo: Option<&Value>,
    ) -> Resultado<Value> {
        let mut url = format!("{}{ruta}", self.api);
        if !params.is_empty() {
            let q: Vec<String> = params
                .iter()
                .map(|(k, v)| format!("{}={}", codificar_mas(k), codificar_mas(v)))
                .collect();
            url = format!("{url}?{}", q.join("&"));
        }
        let archivo = self.archivo_cache(if cuerpo.is_some() { "POST" } else { "GET" }, &url, cuerpo);
        let guardado = Self::leer_cache(&archivo);
        let ahora = (self.reloj)();
        if let Some((t, datos)) = &guardado
            && (0.0..ttl).contains(&(ahora - t))
        {
            return Ok(datos.clone());
        }
        match self.descargar(&url, cuerpo) {
            Ok(datos) => {
                self.guardar_cache(&archivo, ahora, &datos);
                Ok(datos)
            }
            Err(Descarga::NoExiste) => Err(Error::no_encontrada("Flathub no tiene lo que buscabas.")),
            Err(Descarga::Fallo) => match guardado {
                Some((_, datos)) => Ok(datos),
                None => Err(Error::sin_red()),
            },
        }
    }

    // ------------------------------------------------------------------ listas

    /// Las apps de una respuesta de lista (campo «hits»), sin repetidas ni basura.
    fn apps(datos: &Value) -> Vec<App> {
        let mut vistas = std::collections::HashSet::new();
        let mut salida = Vec::new();
        for hit in datos.get("hits").and_then(Value::as_array).into_iter().flatten() {
            if let Some(app) = app_de_hit(hit)
                && vistas.insert(app.id.clone())
            {
                salida.push(app);
            }
        }
        salida
    }

    fn coleccion(&self, nombre: &str, cantidad: usize) -> Resultado<Vec<App>> {
        let datos = self.pedir(
            &format!("/collection/{nombre}"),
            TTL_LISTAS,
            &[
                ("page", "1".into()),
                ("per_page", cantidad.to_string()),
                ("locale", self.idioma.clone()),
            ],
            None,
        )?;
        Ok(Self::apps(&datos))
    }

    /// App a partir de su id (ficha de `/appstream`). `NoEncontrada` si Flathub no la tiene.
    fn resolver(&self, app_id: &str) -> Resultado<App> {
        let ficha = self.pedir(
            &format!("/appstream/{}", codificar(app_id)),
            TTL_DETALLES,
            &[("locale", self.idioma.clone())],
            None,
        )?;
        if !ficha.is_object() {
            return Err(Error::no_encontrada("Flathub no tiene esa aplicación."));
        }
        Ok(app_de_appstream(app_id, &ficha))
    }

    /// Las apps de esos ids, en el mismo orden, pidiendo las fichas en paralelo. Las que Flathub ya no
    /// tiene se saltan. `SinRed` solo si no se pudo conseguir ninguna por falta de red.
    fn resolver_varias(&self, ids: &[&str]) -> Resultado<Vec<App>> {
        let resultados: Vec<std::sync::Mutex<Option<Resultado<App>>>> =
            ids.iter().map(|_| std::sync::Mutex::new(None)).collect();
        let siguiente = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..ids.len().min(8) {
                s.spawn(|| {
                    loop {
                        let i = siguiente.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let Some(id) = ids.get(i) else { break };
                        let r = self.resolver(id);
                        *resultados[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                    }
                });
            }
        });
        let mut apps = Vec::new();
        let mut error: Option<Error> = None;
        for r in resultados {
            match r.into_inner().unwrap_or_else(|e| e.into_inner()) {
                Some(Ok(a)) => apps.push(a),
                Some(Err(e)) if e.es_sin_red() => error = error.or(Some(e)),
                _ => {}
            }
        }
        if apps.is_empty()
            && let Some(e) = error
        {
            return Err(e);
        }
        Ok(apps)
    }

    /// La app del día y las de la semana de Flathub; si faltan, completa con las que más suben.
    pub fn destacados(&self) -> Resultado<Vec<App>> {
        let hoy = fecha_utc((self.reloj)());
        let mut ids: Vec<String> = Vec::new();
        let mut selecciones = || -> Resultado<()> {
            let dia = self.pedir(&format!("/app-picks/app-of-the-day/{hoy}"), TTL_LISTAS, &[], None)?;
            if let Some(id) = dia.get("app_id").and_then(Value::as_str)
                && es_id_valido(id)
            {
                ids.push(id.to_string());
            }
            let semana = self.pedir(
                &format!("/app-picks/apps-of-the-week/{hoy}"),
                TTL_LISTAS,
                &[],
                None,
            )?;
            let mut elegidas: Vec<&Value> = semana
                .get("apps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|e| e.is_object())
                .collect();
            elegidas.sort_by_key(|e| entero(e.get("position")).unwrap_or(0));
            for e in elegidas {
                if let Some(id) = e.get("app_id").and_then(Value::as_str)
                    && es_id_valido(id)
                    && !ids.iter().any(|i| i == id)
                {
                    ids.push(id.to_string());
                }
            }
            Ok(())
        };
        // Sin las selecciones del día, las populares de abajo hacen de destacados.
        let _ = selecciones();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut apps = if refs.is_empty() {
            Vec::new()
        } else {
            self.resolver_varias(&refs).unwrap_or_default()
        };
        if apps.len() < MINIMO_DESTACADOS {
            let tendencia = self.coleccion("trending", MINIMO_DESTACADOS * 2)?;
            let faltan = MINIMO_DESTACADOS - apps.len();
            let ya: Vec<String> = apps.iter().map(|a| a.id.clone()).collect();
            apps.extend(tendencia.into_iter().filter(|a| !ya.contains(&a.id)).take(faltan));
        }
        Ok(apps)
    }

    /// La lista de Vortexine's ([`RECOMENDADAS`]), con los datos de Flathub. Las que Flathub ya no tenga no salen.
    pub fn recomendadas(&self) -> Resultado<Vec<App>> {
        self.resolver_varias(&RECOMENDADAS)
    }

    /// Las apps de la categoría «IA», con los datos de Flathub y el resumen, la gráfica y la nota de
    /// Vortexine's. Las que Flathub ya no tenga no salen.
    pub fn apps_ia(&self) -> Resultado<Vec<App>> {
        let ids: Vec<&str> = IA.iter().map(|d| d.id).collect();
        Ok(self
            .resolver_varias(&ids)?
            .into_iter()
            .map(con_datos_ia)
            .collect())
    }

    /// La categoría «IA» de Vortexine's y, detrás, las de Flathub, en español y en el orden de la pantalla.
    pub fn categorias(&self) -> Resultado<Vec<Categoria>> {
        let datos = self.pedir("/collection/category", TTL_DETALLES, &[], None)?;
        let ids: Vec<String> = datos
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim().to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let mut salida = vec![categoria_ia()];
        for (id, nombre, icono) in CATEGORIAS {
            if ids.iter().any(|i| i == id) {
                salida.push(Categoria {
                    id: id.into(),
                    nombre: nombre.into(),
                    icono: icono.into(),
                    ..Categoria::default()
                });
            }
        }
        for i in &ids {
            if !CATEGORIAS.iter().any(|c| c.0 == i) && i != CATEGORIA_IA && id_categoria_valido(i) {
                let mut cap = i.clone();
                cap[..1].make_ascii_uppercase();
                salida.push(Categoria {
                    id: i.clone(),
                    nombre: cap,
                    icono: CATEGORIA_NUEVA.into(),
                    ..Categoria::default()
                });
            }
        }
        Ok(salida)
    }

    /// Una página (`POR_PAGINA` apps) de una categoría, de las más instaladas a las menos. La lista viene
    /// vacía cuando la página no existe o la categoría tampoco.
    pub fn por_categoria(&self, cat_id: &str, pagina: i64) -> Resultado<Vec<App>> {
        let cat_id = cat_id.trim().to_lowercase();
        if !id_categoria_valido(&cat_id) {
            return Ok(Vec::new());
        }
        let pagina = pagina.max(1);
        if cat_id == CATEGORIA_IA {
            // la de Vortexine's: una sola página, en el orden de la lista
            return if pagina == 1 {
                self.apps_ia()
            } else {
                Ok(Vec::new())
            };
        }
        let datos = self.pedir(
            &format!("/collection/category/{}", codificar(&cat_id)),
            TTL_LISTAS,
            &[
                ("page", pagina.to_string()),
                ("per_page", POR_PAGINA.to_string()),
                ("sort_by", "installs_last_month".into()),
                ("locale", self.idioma.clone()),
            ],
            None,
        );
        match datos {
            Ok(d) => Ok(Self::apps(&d)),
            Err(Error::NoEncontrada(_)) => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Las apps que coinciden con lo escrito (nombre, resumen, palabras clave), las mejores primero.
    pub fn buscar(&self, texto: &str) -> Resultado<Vec<App>> {
        let t: String = texto.split_whitespace().collect::<Vec<_>>().join(" ");
        let t: String = t.chars().take(100).collect();
        if t.is_empty() {
            return Ok(Vec::new());
        }
        let datos = self.pedir(
            "/search",
            TTL_LISTAS,
            &[("locale", self.idioma.clone())],
            Some(&json!({"query": t, "hits_per_page": POR_PAGINA, "page": 1})),
        )?;
        Ok(Self::apps(&datos))
    }

    // ------------------------------------------------------------------ ficha

    /// La ficha completa de una app. `NoEncontrada` si el id no es válido o Flathub no la tiene.
    pub fn detalle(&self, app_id: &str) -> Resultado<AppDetalle> {
        if !es_id_valido(app_id) {
            return Err(Error::no_encontrada("Esa aplicación no existe."));
        }
        let seguro = codificar(app_id);
        let (ficha, resumen) = std::thread::scope(|s| {
            // El tamaño es un extra: sin él la ficha sale igual.
            let tamano = s.spawn(|| {
                self.pedir(&format!("/summary/{seguro}"), TTL_DETALLES, &[], None)
                    .ok()
            });
            let ficha = self.pedir(
                &format!("/appstream/{seguro}"),
                TTL_DETALLES,
                &[("locale", self.idioma.clone())],
                None,
            );
            (ficha, tamano.join().ok().flatten())
        });
        let ficha = ficha?;
        if !ficha.is_object() {
            return Err(Error::no_encontrada("Flathub no tiene esa aplicación."));
        }
        let base = app_de_appstream(app_id, &ficha);
        let urls = ficha.get("urls").filter(|u| u.is_object());
        let mut capturas: Vec<String> = Vec::new();
        for cap in ficha
            .get("screenshots")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let url = if cap.is_object() {
                captura(cap.get("sizes"))
            } else {
                String::new()
            };
            if !url.is_empty() && !capturas.contains(&url) {
                capturas.push(url);
            }
        }
        let mut etiquetas: Vec<String> = Vec::new();
        for cat in ficha
            .get("categories")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let nombre = cat
                .as_str()
                .and_then(|c| ETIQUETAS.iter().find(|e| e.0 == c))
                .map(|e| e.1.to_string());
            if let Some(n) = nombre
                && !etiquetas.contains(&n)
            {
                etiquetas.push(n);
            }
        }
        let tam = resumen.as_ref().and_then(|r| entero(r.get("download_size")));
        let descripcion = html_a_texto(ficha.get("description").and_then(Value::as_str).unwrap_or(""));
        let web = url_web(urls.and_then(|u| u.get("homepage")));
        let detalle = AppDetalle {
            descripcion: if descripcion.is_empty() {
                base.resumen.clone()
            } else {
                descripcion
            },
            app: base,
            capturas: capturas.into_iter().take(12).collect(),
            licencia: licencia(ficha.get("project_license")),
            web: (!web.is_empty()).then_some(web),
            version: version(ficha.get("releases")),
            tamano_descarga: tam.filter(|t| *t > 0),
            categorias: etiquetas,
        };
        Ok(AppDetalle {
            app: con_datos_ia(detalle.app.clone()),
            ..detalle
        })
    }
}
