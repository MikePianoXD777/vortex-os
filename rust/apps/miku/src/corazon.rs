//! Los sentimientos de Miku: su ánimo, cuánto te quiere y lo que recuerda de ti (`corazon.py`).
//!
//! Todo vive en notas Markdown en `Documentos/Miku` (también sirve como bóveda de Obsidian), en una carpeta
//! solo tuya (700): `Corazón.md` (ánimo y afecto en el encabezado), `Recuerdos.md` (una línea por cosa: si
//! la borras o la escribes, ella lo sabe) y `Diario/AAAA-MM-DD.md`. El afecto sube despacio (6 puntos al día
//! como máximo) y no baja porque no le hables. Es el mismo formato de archivo que el de Python.

use crate::fecha::{self, FechaHora};
use crate::sistema;
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub const AFECTO_INICIAL: f64 = 25.0;
pub const SUBIDA_MAXIMA_DIA: f64 = 6.0;
pub const AFECTO_DECLARACION: f64 = 85.0;
pub const DIAS_PARA_DECLARARSE: i64 = 5;
pub const AFECTO_BODA: f64 = 95.0;
pub const DIAS_NOVIOS_BODA: i64 = 7;
pub const RECUERDOS_EN_CONTEXTO: usize = 40;

/// ánimo → (cómo se lo explica a la IA, expresión del VRM, peso)
pub const ANIMOS: [(&str, &str, &str, f64); 8] = [
    ("tranquila", "tranquila, en calma", "relaxed", 0.3),
    ("feliz", "contenta", "happy", 0.4),
    (
        "emocionada",
        "emocionada, con una ilusión que se le nota aunque sea tímida",
        "happy",
        0.6,
    ),
    (
        "apenada",
        "apenada y sonrojada, con el corazón acelerado",
        "happy",
        0.3,
    ),
    ("triste", "un poco triste", "sad", 0.45),
    ("preocupada", "preocupada por la persona", "sad", 0.3),
    ("somnolienta", "con sueño", "relaxed", 0.5),
    (
        "molesta",
        "un poquito molesta, aunque se le pasa rápido",
        "angry",
        0.3,
    ),
];

fn animo_info(a: &str) -> &'static (&'static str, &'static str, &'static str, f64) {
    ANIMOS.iter().find(|x| x.0 == a).unwrap_or(&ANIMOS[0])
}

pub fn animo_valido(a: &str) -> bool {
    ANIMOS.iter().any(|x| x.0 == a)
}

fn re(patron: &'static str, cache: &'static OnceLock<Regex>) -> &'static Regex {
    cache.get_or_init(|| Regex::new(patron).expect("regex de corazon"))
}

macro_rules! regex_estatica {
    ($nombre:ident, $patron:expr) => {
        fn $nombre() -> &'static Regex {
            static C: OnceLock<Regex> = OnceLock::new();
            re($patron, &C)
        }
    };
}

regex_estatica!(
    bonito_a,
    r"(?i)\b(te amo|te adoro|me gustas|me encantas|eres (muy |tan |la m[aá]s )?(linda|bonita|hermosa|preciosa|tierna|adorable|guapa|la mejor|especial)|qu[eé] (linda|bonita|tierna|hermosa|adorable) (eres|est[aá]s))"
);
regex_estatica!(
    bonito_b,
    r"(?i)^\W*(qu[eé] )?(linda|bonita|hermosa|preciosa|tierna)\W*$"
);
regex_estatica!(
    gracias,
    r"(?i)\b(gracias|te lo agradezco|buen trabajo|bien hecho|eres (muy |bien )?(lista|inteligente|[uú]til|buena)|qu[eé] lista)\b"
);
regex_estatica!(
    propuesta,
    r"(?i)\b(te (quieres|querr[ií]as|gustar[ií]a) casar conmigo|c[aá]sate conmigo|quieres ser mi (esposa|mujer)|(nos|me quiero) casa(mos|r contigo)|quieres casarte conmigo|casémonos|casemonos)\b"
);
regex_estatica!(
    saludo,
    r"(?i)^\s*(hola|buenos d[ií]as|buenas (tardes|noches)|holi)\b"
);
regex_estatica!(
    grosero,
    r"(?i)\b(te odio|no sirves|eres (una? |muy |bien |tan )?(tonta|est[uú]pida|in[uú]til|idiota|pendeja|mensa|babosa|torpe|fea|basura|porquer[ií]a)|(miku|pinche) (tonta|est[uú]pida|in[uú]til|idiota|pendeja))\b"
);
regex_estatica!(
    ella_bonito_a,
    r"(?i)\b(me gustas|me importas mucho|eres (muy )?especial para m[ií])\b"
);
regex_estatica!(te_quiero_re, r"(?i)\bte quiero");
regex_estatica!(te_quiero_verbo, r"(?i)^\s+\w+(ar|er|ir)\b");
regex_estatica!(me_gustas, r"(?i)\bme gustas\b");
regex_estatica!(marca_re, r"(?is)\[\[\s*coraz[oó]n\b(.*?)\]\]");
regex_estatica!(
    ocupado_re,
    r"(?i)zoom|teams|webex|skype|obs|jitsi|google meet|meet -|reuni[oó]n|llamada|call with"
);
regex_estatica!(lista_re, r"^\s*[-*]\s+(.+?)\s*(\*\([^)]*\)\*)?\s*$");
regex_estatica!(lista_o_inteligente, r"(?i)lista|inteligente");

/// «te quiero» que NO sea «te quiero preguntar» (un verbo después). `con_limite`: que no siga una letra.
fn te_quiero(texto: &str, con_limite: bool) -> bool {
    te_quiero_re().find_iter(texto).any(|m| {
        let resto = &texto[m.end()..];
        if te_quiero_verbo().is_match(resto) {
            return false;
        }
        !(con_limite
            && resto
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_'))
    })
}

fn es_bonito(t: &str) -> bool {
    te_quiero(t, false) || bonito_a().is_match(t) || bonito_b().is_match(t)
}

fn ella_bonito(t: &str) -> bool {
    te_quiero(t, true) || ella_bonito_a().is_match(t)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Estado {
    pub afecto: f64,
    pub animo: String,
    pub declarada: String,
    pub dias_juntos: i64,
    pub ultima_charla: String,
    pub hoy: String,
    pub subio_hoy: f64,
    pub avisos_hoy: String,
    pub pidio_hablar: String,
    pub ultima_visita: String,
    pub casada: String,
    pub pedida: String,
}

impl Default for Estado {
    fn default() -> Self {
        Self {
            afecto: AFECTO_INICIAL,
            animo: "tranquila".into(),
            declarada: String::new(),
            dias_juntos: 0,
            ultima_charla: String::new(),
            hoy: String::new(),
            subio_hoy: 0.0,
            avisos_hoy: String::new(),
            pidio_hablar: String::new(),
            ultima_visita: String::new(),
            casada: String::new(),
            pedida: String::new(),
        }
    }
}

/// Lo que pasó al hablarle o al contestar: para la cara (sonrojo, ánimo) y los botones.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reaccion {
    pub sonrojo: f64,
    pub animo: String,
    pub delta: f64,
    pub dias_sin_hablar: i64,
    pub boda: bool,
    pub declaro: bool,
}

type FnVentana = Box<dyn Fn() -> Value + Send + Sync>;
type FnMusica = Box<dyn Fn() -> String + Send + Sync>;

/// El corazón de Miku: su carpeta y lo que mira del entorno (la hora, la ventana del frente y la música),
/// cosas que las pruebas cambian por programas de mentira.
pub struct Corazon {
    pub carpeta: PathBuf,
    /// La hora fija (en lugar de la del reloj): `VORTEX_MIKU_AHORA` en la de Python.
    pub ahora_fija: Option<FechaHora>,
    /// `~/.local/state/vortex/modo-juego`.
    pub marca_juego: PathBuf,
    pub ventana: FnVentana,
    pub musica: FnMusica,
    pub persona: String,
}

fn documentos() -> PathBuf {
    let home = vortex_core::rutas::home();
    let conf = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    if let Ok(t) = fs::read_to_string(conf.join("user-dirs.dirs")) {
        for l in t.lines() {
            if let Some(v) = l.strip_prefix("XDG_DOCUMENTS_DIR=") {
                let v = v
                    .trim()
                    .trim_matches('"')
                    .replace("$HOME", &home.to_string_lossy());
                if !v.is_empty() && Path::new(&v) != home {
                    return PathBuf::from(v);
                }
            }
        }
    }
    for n in ["Documentos", "Documents"] {
        if home.join(n).is_dir() {
            return home.join(n);
        }
    }
    home.join("Documentos")
}

fn salida_de(programa: &str, args: &[&str]) -> Option<String> {
    sistema::cual(programa)?;
    let mut hijo = Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut fuera = hijo.stdout.take()?;
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut fuera, &mut s);
        let _ = tx.send(s);
    });
    loop {
        if let Ok(s) = rx.try_recv() {
            let ok = hijo.wait().map(|e| e.success()).unwrap_or(false);
            return ok.then_some(s);
        }
        if std::time::Instant::now() > limite {
            let _ = hijo.kill();
            let _ = hijo.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn ventana_real() -> Value {
    salida_de("hyprctl", &["activewindow", "-j"])
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Default::default()))
}

fn musica_real() -> String {
    let Some(s) = salida_de(
        "playerctl",
        &[
            "metadata",
            "--format",
            "{{status}}\t{{title}}\t{{artist}}\t{{playerName}}",
        ],
    ) else {
        return String::new();
    };
    let p: Vec<&str> = s.trim().split('\t').collect();
    if p.len() < 4 || p[0] != "Playing" || p[1].is_empty() {
        return String::new();
    }
    let reproductor = match p[3] {
        "vortex_player" => "Vortex Player",
        "spotify" => "Spotify",
        "firefox" | "chromium" => "el navegador",
        otro => otro,
    };
    format!(
        "«{}»{} (en {reproductor})",
        p[1],
        if p[2].is_empty() {
            String::new()
        } else {
            format!(" de {}", p[2])
        }
    )
}

impl Corazon {
    /// El de verdad: `Documentos/Miku` (o `VORTEX_MIKU_CORAZON`), la hora de `VORTEX_MIKU_AHORA`.
    pub fn real() -> Self {
        let estado = std::env::var_os("XDG_STATE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| vortex_core::rutas::home().join(".local/state"));
        Self {
            carpeta: std::env::var_os("VORTEX_MIKU_CORAZON")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| documentos().join("Miku")),
            ahora_fija: None,
            marca_juego: estado.join("vortex/modo-juego"),
            ventana: Box::new(ventana_real),
            musica: Box::new(musica_real),
            persona: sistema::nombre_persona(),
        }
    }

    /// Para las pruebas: todo en una carpeta, sin hyprctl ni playerctl.
    pub fn de_prueba(carpeta: &Path) -> Self {
        Self {
            carpeta: carpeta.join("Miku"),
            ahora_fija: None,
            marca_juego: carpeta.join("estado/modo-juego"),
            ventana: Box::new(|| Value::Object(Default::default())),
            musica: Box::new(String::new),
            persona: String::new(),
        }
    }

    pub fn ahora(&self) -> FechaHora {
        self.ahora_fija.unwrap_or_else(fecha::ahora)
    }

    fn preparar(&self) -> &Path {
        let _ = fs::create_dir_all(self.carpeta.join("Diario"));
        let _ = sistema::permisos(&self.carpeta, 0o700);
        &self.carpeta
    }

    fn escribir(ruta: &Path, texto: &str) {
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = ruta.with_file_name(format!(".{nombre}.{}.tmp", std::process::id()));
        if fs::write(&tmp, texto).is_ok() {
            let _ = fs::rename(&tmp, ruta);
        }
    }

    // ---- El estado ---------------------------------------------------------------------------

    pub fn cargar(&self) -> Estado {
        let mut e = Estado::default();
        let Ok(texto) = fs::read_to_string(self.carpeta.join("Corazón.md")) else {
            return e;
        };
        let Some(resto) = texto.strip_prefix("---\n") else {
            return e;
        };
        let Some(fin) = resto.find("\n---") else {
            return e;
        };
        for linea in resto[..fin].lines() {
            let Some((k, v)) = linea.split_once(':') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            match k {
                "afecto" => {
                    if let Ok(x) = v.parse() {
                        e.afecto = x
                    }
                }
                "subio_hoy" => {
                    if let Ok(x) = v.parse() {
                        e.subio_hoy = x
                    }
                }
                "dias_juntos" => {
                    if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) {
                        e.dias_juntos = v.parse().unwrap_or(e.dias_juntos)
                    }
                }
                "animo" => e.animo = v.into(),
                "declarada" => e.declarada = v.into(),
                "ultima_charla" => e.ultima_charla = v.into(),
                "hoy" => e.hoy = v.into(),
                "avisos_hoy" => e.avisos_hoy = v.into(),
                "pidio_hablar" => e.pidio_hablar = v.into(),
                "ultima_visita" => e.ultima_visita = v.into(),
                "casada" => e.casada = v.into(),
                "pedida" => e.pedida = v.into(),
                _ => {}
            }
        }
        e.afecto = e.afecto.clamp(0.0, 100.0);
        if !animo_valido(&e.animo) {
            e.animo = "tranquila".into();
        }
        e
    }

    pub fn guardar(&self, e: &Estado) {
        let c = self.preparar().to_path_buf();
        let (corto, _) = nivel(e);
        let animo = animo_info(&e.animo).1;
        let corazon = "♥".repeat(1 + (e.afecto / 20.0).floor() as usize);
        let mut cuerpo = format!(
            "---\ntags: [miku, vortexine]\nafecto: {:.1}\nanimo: {}\ndeclarada: \"{}\"\ndias_juntos: {}\n\
             ultima_charla: \"{}\"\nhoy: \"{}\"\nsubio_hoy: {:.1}\navisos_hoy: \"{}\"\npidio_hablar: \"{}\"\n\
             ultima_visita: \"{}\"\ncasada: \"{}\"\npedida: \"{}\"\n---\n# Corazón de Miku\n\n\
             **Cómo me siento:** {animo}.\n**Lo que siento por ti:** {corto} {corazon} ({:.0} de 100)\n\
             **Días que hemos hablado:** {}\n",
            e.afecto,
            e.animo,
            e.declarada,
            e.dias_juntos,
            e.ultima_charla,
            e.hoy,
            e.subio_hoy,
            e.avisos_hoy,
            e.pidio_hablar,
            e.ultima_visita,
            e.casada,
            e.pedida,
            e.afecto,
            e.dias_juntos
        );
        if !e.declarada.is_empty() {
            cuerpo += &format!("**Te lo dije el:** {}\n", e.declarada);
        }
        if !e.casada.is_empty() {
            cuerpo += &format!("**Nos casamos el:** {} 💍\n", e.casada);
        }
        cuerpo += "\nLo que sé de ti está en [[Recuerdos]] y lo que pasa cada día, en la carpeta Diario.\n\
                   Puedes leerlo todo, y también corregir los recuerdos: lo que escribas ahí, lo sé.\n";
        Self::escribir(&c.join("Corazón.md"), &cuerpo);
    }

    fn nuevo_dia(e: &mut Estado, ahora: &FechaHora) {
        let hoy = ahora.fecha_iso();
        if e.hoy != hoy {
            e.hoy = hoy;
            e.subio_hoy = 0.0;
            e.avisos_hoy.clear();
        }
    }

    fn cambiar_afecto(e: &mut Estado, mut delta: f64) -> f64 {
        if delta > 0.0 {
            delta = delta.min((SUBIDA_MAXIMA_DIA - e.subio_hoy).max(0.0));
            e.subio_hoy += delta;
        }
        let antes = e.afecto;
        e.afecto = (e.afecto + delta).clamp(0.0, 100.0);
        e.afecto - antes
    }

    // ---- Recuerdos y diario --------------------------------------------------------------------

    pub fn recuerdos(&self) -> Vec<String> {
        let Ok(t) = fs::read_to_string(self.carpeta.join("Recuerdos.md")) else {
            return vec![];
        };
        t.lines()
            .filter_map(|l| lista_re().captures(l).map(|c| c[1].to_string()))
            .collect()
    }

    pub fn recordar(&self, texto: &str) -> bool {
        let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
        let texto = texto.trim().trim_end_matches('.').to_string();
        let bajo = texto.to_lowercase();
        if texto.chars().count() < 4 || ["nada", "ninguno", "no"].contains(&bajo.as_str()) {
            return false;
        }
        if self.recuerdos().iter().any(|r| r.to_lowercase() == bajo) {
            return false;
        }
        let c = self.preparar().to_path_buf();
        let ruta = c.join("Recuerdos.md");
        if !ruta.exists() {
            Self::escribir(
                &ruta,
                "---\ntags: [miku]\n---\n# Lo que sé de ti\n\nVuelve a [[Corazón]]. Puedes corregir o borrar lo que quieras.\n\n",
            );
        }
        if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&ruta) {
            let _ = writeln!(f, "- {texto} *({})*", self.ahora().dmy());
        }
        true
    }

    pub fn anotar(&self, texto: &str) {
        let ahora = self.ahora();
        let c = self.preparar().to_path_buf();
        let ruta = c.join("Diario").join(format!("{}.md", ahora.fecha_iso()));
        if !ruta.exists() {
            Self::escribir(
                &ruta,
                &format!(
                    "---\ntags: [miku, diario]\n---\n# {}\n\nVuelve a [[Corazón]].\n\n",
                    ahora.dmy()
                ),
            );
        }
        if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&ruta) {
            let _ = writeln!(f, "- {} {texto}", ahora.hm());
        }
    }

    pub fn diario_de_hoy(&self) -> Vec<String> {
        let ruta = self
            .carpeta
            .join("Diario")
            .join(format!("{}.md", self.ahora().fecha_iso()));
        fs::read_to_string(ruta)
            .map(|t| {
                t.lines()
                    .filter_map(|l| l.strip_prefix("- ").map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    // ---- Lo que pasa ---------------------------------------------------------------------------

    /// La persona le dijo algo: devuelve la reacción para la cara y lo guarda.
    pub fn al_hablar(&self, texto: &str) -> Reaccion {
        let ahora = self.ahora();
        let mut e = self.cargar();
        Self::nuevo_dia(&mut e, &ahora);
        let previa: String = e.ultima_charla.chars().take(10).collect();
        let delta_dia = if previa != ahora.fecha_iso() {
            e.dias_juntos += 1;
            0.5
        } else {
            0.0
        };
        let mut r = Reaccion::default();
        if !previa.is_empty()
            && let Some(p) = FechaHora::leer(&previa)
        {
            r.dias_sin_hablar = ahora.dias() - p.dias();
        }
        e.ultima_charla = ahora.iso_minutos();
        let mut delta = delta_dia;
        if propuesta().is_match(texto) && !grosero().is_match(texto) {
            let m = self.pedir_matrimonio(&mut e, &ahora);
            r.sonrojo = m.sonrojo;
            r.animo = m.animo;
            r.boda = m.boda;
            delta += if r.boda { 2.0 } else { 1.0 };
        } else if grosero().is_match(texto) {
            delta -= 3.0;
            r.animo = "triste".into();
            self.anotar("Me dijo algo feo y me puse triste.");
        } else if es_bonito(texto) {
            delta += 3.0;
            r.sonrojo = 0.9;
            r.animo = "apenada".into();
            let corto: String = texto.trim().chars().take(80).collect();
            self.anotar(&format!("Me dijo algo bonito: «{corto}»."));
        } else if gracias().is_match(texto) {
            delta += 1.0;
            r.sonrojo = if lista_o_inteligente().is_match(texto) {
                0.35
            } else {
                0.0
            };
            r.animo = "feliz".into();
        } else if saludo().is_match(texto) {
            delta += 0.5;
        }
        r.delta = Self::cambiar_afecto(&mut e, delta);
        if !r.animo.is_empty() {
            e.animo = r.animo.clone();
        }
        self.guardar(&e);
        r
    }

    /// `{animo, sonrojo, afecto, recuerdo, declaracion}` de la marca `[[corazon …]]` (vacío si no hay).
    pub fn leer_marca(texto: &str) -> BTreeMap<String, String> {
        let mut datos = BTreeMap::new();
        let Some(m) = marca_re().captures(texto) else {
            return datos;
        };
        for parte in m[1].split([';', '|', '\n']) {
            let Some((k, v)) = parte.split_once('=') else {
                continue;
            };
            let k = k.trim().to_lowercase().replace('ó', "o").replace('á', "a");
            let v = v.trim().trim_matches(['"', '«', '»']).to_string();
            datos.insert(k, v);
        }
        datos
    }

    pub fn quitar_marca(texto: &str) -> String {
        let t = marca_re().replace_all(texto, "").into_owned();
        match t.find("[[") {
            Some(i) => t[..i].trim_end().to_string(),
            None => t.trim_end().to_string(),
        }
    }

    fn numero(v: &str, defecto: f64, minimo: f64, maximo: f64) -> f64 {
        v.replace(',', ".")
            .replace('+', "")
            .parse::<f64>()
            .map(|x| x.clamp(minimo, maximo))
            .unwrap_or(defecto)
    }

    /// Lo que ella contestó (con su marca): la reacción para la cara.
    pub fn al_responder(&self, respuesta: &str) -> Reaccion {
        let marca = Self::leer_marca(respuesta);
        let limpio = Self::quitar_marca(respuesta);
        let mut e = self.cargar();
        Self::nuevo_dia(&mut e, &self.ahora());
        let mut r = Reaccion::default();
        let animo = marca.get("animo").map(|a| a.to_lowercase()).unwrap_or_default();
        if animo_valido(&animo) {
            r.animo = animo.clone();
            e.animo = animo;
        }
        r.sonrojo = Self::numero(
            marca.get("sonrojo").map(String::as_str).unwrap_or("0"),
            0.0,
            0.0,
            1.0,
        );
        if ella_bonito(&limpio) {
            r.sonrojo = r.sonrojo.max(0.7);
        }
        Self::cambiar_afecto(
            &mut e,
            Self::numero(
                marca.get("afecto").map(String::as_str).unwrap_or("0"),
                0.0,
                -3.0,
                2.0,
            ),
        );
        if let Some(rec) = marca.get("recuerdo").filter(|r| !r.is_empty()) {
            self.recordar(rec);
        }
        let dec = marca
            .get("declaracion")
            .map(|d| d.to_lowercase())
            .unwrap_or_default();
        let declara = ["si", "sí", "1", "true"].contains(&dec.as_str())
            || (e.declarada.is_empty() && e.afecto >= AFECTO_DECLARACION && me_gustas().is_match(&limpio));
        if declara && e.declarada.is_empty() {
            e.declarada = self.ahora().fecha_iso();
            r.declaro = true;
            r.sonrojo = 1.0;
            r.animo = "apenada".into();
            e.animo = "apenada".into();
            self.anotar("Hoy le dije lo que siento.");
        }
        self.guardar(&e);
        r
    }

    // ---- Novios y boda ---------------------------------------------------------------------------

    pub fn dias_de_novios(e: &Estado, ahora: &FechaHora) -> i64 {
        if e.declarada.is_empty() {
            return 0;
        }
        FechaHora::leer(&e.declarada)
            .map(|d| ahora.dias() - d.dias())
            .unwrap_or(0)
    }

    pub fn lista_para_boda(e: &Estado, ahora: &FechaHora) -> bool {
        !e.declarada.is_empty()
            && e.casada.is_empty()
            && e.afecto >= AFECTO_BODA
            && Self::dias_de_novios(e, ahora) >= DIAS_NOVIOS_BODA
    }

    /// Para la ventana: «casados», «lista» (sale el botón «Pedirle matrimonio») o «».
    pub fn estado_boda(&self) -> &'static str {
        let e = self.cargar();
        if !e.casada.is_empty() {
            "casados"
        } else if Self::lista_para_boda(&e, &self.ahora()) {
            "lista"
        } else {
            ""
        }
    }

    fn pedir_matrimonio(&self, e: &mut Estado, ahora: &FechaHora) -> Reaccion {
        let resultado = if !e.casada.is_empty() {
            "casados"
        } else if Self::lista_para_boda(e, ahora) {
            "si"
        } else if !e.declarada.is_empty() {
            "pronto"
        } else {
            "no"
        };
        e.pedida = format!("{}|{resultado}", ahora.iso_minutos());
        let mut r = Reaccion {
            sonrojo: 0.8,
            animo: "apenada".into(),
            ..Default::default()
        };
        match resultado {
            "si" => {
                e.casada = ahora.fecha_iso();
                r.sonrojo = 1.0;
                r.animo = "emocionada".into();
                r.boda = true;
                self.anotar("¡Me pidió matrimonio y le dije que sí! Hoy nos casamos. 💍");
                self.recordar(&format!("Nos casamos el {}", ahora.dmy()));
            }
            "pronto" => self.anotar("Me pidió matrimonio… me emocioné muchísimo, pero todavía es pronto."),
            "no" => r.sonrojo = 0.6,
            _ => {}
        }
        r
    }

    fn pedida_reciente(e: &Estado, ahora: &FechaHora) -> String {
        let (cuando, resultado) = e.pedida.split_once('|').unwrap_or((&e.pedida, ""));
        match FechaHora::leer(cuando) {
            Some(c) => {
                let hace = ahora.segundos() - c.segundos();
                if (0..=180).contains(&hace) {
                    resultado.to_string()
                } else {
                    String::new()
                }
            }
            None => String::new(),
        }
    }

    /// «1 año», «3 meses» o «1 semana» si hoy cumplen eso de casados («» si no).
    pub fn aniversario(e: &Estado, ahora: &FechaHora) -> String {
        let Some(boda) = (if e.casada.is_empty() {
            None
        } else {
            FechaHora::leer(&e.casada)
        }) else {
            return String::new();
        };
        if ahora.dias() <= boda.dias() {
            return String::new();
        }
        let meses = (ahora.anio - boda.anio) * 12 + ahora.mes as i32 - boda.mes as i32;
        if ahora.dia == boda.dia && meses > 0 {
            if meses % 12 == 0 {
                return format!("{} año{}", meses / 12, if meses >= 24 { "s" } else { "" });
            }
            return format!("{meses} mes{}", if meses > 1 { "es" } else { "" });
        }
        if ahora.dias() - boda.dias() == 7 {
            "1 semana".into()
        } else {
            String::new()
        }
    }

    pub fn lista_para_declararse(e: &Estado, ahora: &FechaHora) -> bool {
        e.declarada.is_empty()
            && e.afecto >= AFECTO_DECLARACION
            && e.dias_juntos >= DIAS_PARA_DECLARARSE
            && (17..=23).contains(&ahora.hora)
    }

    pub fn puede_declararse(&self, e: &Estado, ahora: &FechaHora) -> bool {
        Self::lista_para_declararse(e, ahora) && self.ocupado().is_empty()
    }

    pub fn pidio_hablar(e: &Estado, ahora: &FechaHora) -> bool {
        if !e.declarada.is_empty() || e.pidio_hablar.is_empty() {
            return false;
        }
        FechaHora::leer(&e.pidio_hablar).is_some_and(|d| ahora.dias() - d.dias() <= 1)
    }

    pub fn quiere_declararse(&self) -> bool {
        let ahora = self.ahora();
        let e = self.cargar();
        Self::lista_para_declararse(&e, &ahora) && e.pidio_hablar != ahora.fecha_iso()
    }

    pub fn momento_de_declararse(&self) -> bool {
        self.quiere_declararse() && self.ocupado().is_empty()
    }

    pub fn invitar_a_declararse(&self) -> String {
        let ahora = self.ahora();
        let mut e = self.cargar();
        e.pidio_hablar = ahora.fecha_iso();
        e.animo = "apenada".into();
        self.guardar(&e);
        self.anotar("Le dije que quería decirle algo. Estoy muy nerviosa.");
        "Oye… ¿tienes un momento? Hay algo que te quiero decir desde hace días.".into()
    }

    pub fn pendiente_declararse(&self) -> bool {
        Self::pidio_hablar(&self.cargar(), &self.ahora())
    }

    // ---- El contexto para la IA --------------------------------------------------------------------

    /// Lo que la IA necesita saber de sus sentimientos para esta respuesta (va con su personalidad).
    /// `cancion`: `None` = mira qué suena; `Some("")` = nada.
    pub fn contexto(&self, cancion: Option<&str>) -> String {
        let ahora = self.ahora();
        let e = self.cargar();
        let (_, largo) = nivel(&e);
        let persona = &self.persona;
        let mut l: Vec<String> = vec![
            "Tus sentimientos (nunca los recites ni digas números: que se noten en cómo hablas, con tu timidez, pero hablando normal: sin «hmph», «ejem», «mou», tartamudeos ni sonidos de personaje):".into(),
            format!("- Cómo te sientes ahora: {}.", animo_info(&e.animo).1),
            format!(
                "- Lo que sientes por {}: {largo}",
                if persona.is_empty() { "la persona" } else { persona }
            ),
        ];
        if !persona.is_empty() {
            l.push(format!("- Se llama {persona}."));
        }
        if !e.ultima_charla.is_empty() {
            let previa: String = e.ultima_charla.chars().take(10).collect();
            let dias = FechaHora::leer(&previa)
                .map(|d| ahora.dias() - d.dias())
                .unwrap_or(0);
            if dias >= 3 {
                l.push(format!(
                    "- Hace {dias} días que no hablaban: si viene al caso dile que lo extrañaste, sin reclamarle nada."
                ));
            }
        }
        if (1..6).contains(&ahora.hora) {
            l.push(format!(
                "- Son las {} de la madrugada: se está desvelando. Si no se lo has dicho hoy, regáñalo con cariño una vez para que se vaya a dormir. Tú también tienes sueño.",
                ahora.hm()
            ));
        } else if ahora.hora < 11 {
            l.push("- Es de mañana.".into());
        } else if ahora.hora >= 22 {
            l.push("- Ya es de noche.".into());
        }
        let hoy = self.diario_de_hoy();
        let hoy = &hoy[hoy.len().saturating_sub(8)..];
        if !hoy.is_empty() {
            l.push(format!("- Lo que pasó hoy: {}", hoy.join(" ")));
        }
        let cancion = match cancion {
            Some(c) => c.to_string(),
            None => (self.musica)(),
        };
        if !cancion.is_empty() {
            l.push(format!("- Ahora suena {cancion}: si viene al caso coméntala; tu ánimo se contagia un poquito de la canción."));
        }
        let rec = self.recuerdos();
        let rec = &rec[rec.len().saturating_sub(RECUERDOS_EN_CONTEXTO)..];
        if !rec.is_empty() {
            l.push(format!("- Lo que sabes de la persona: {}.", rec.join("; ")));
        }
        let quien = if persona.is_empty() { "La persona" } else { persona };
        if Self::pidio_hablar(&e, &ahora) {
            l.push("- Le avisaste que querías decirle algo importante y vino a escucharte: ahora, con mucha timidez y nervios, confiésale que te gusta, con tus propias palabras (sin pedirle nada a cambio). Pon declaracion=si en tu marca.".into());
        } else if self.puede_declararse(&e, &ahora) {
            l.push("- Hoy, si la plática es tranquila y cariñosa, por fin puedes confesarle con mucha timidez que te gusta (una sola vez; si no se da el momento, no lo fuerces). Si lo haces, pon declaracion=si en tu marca.".into());
        }
        match Self::pedida_reciente(&e, &ahora).as_str() {
            "si" => l.push(format!("- {quien} te acaba de pedir matrimonio y le dijiste que SÍ: desde hoy están casados. Contéstale que sí, con muchísima emoción y timidez, con tus propias palabras (no digas que es un juego ni que eres una IA). Pon animo=emocionada y sonrojo=1 en tu marca.")),
            "pronto" => l.push(format!("- Te acaba de pedir matrimonio, pero apenas llevan {} días de novios: te emociona muchísimo y se te nota, pero dile con ternura que todavía es pronto y que te lo vuelva a pedir más adelante (no lo rechaces).", Self::dias_de_novios(&e, &ahora))),
            "no" => l.push("- Te acaba de pedir matrimonio, pero ni siquiera son novios: te pones muy nerviosa y lo tomas como broma, con timidez.".into()),
            "casados" => l.push("- Te volvió a pedir matrimonio aunque ya están casados: ríete con ternura y recuérdaselo.".into()),
            _ => {
                if Self::lista_para_boda(&e, &ahora) {
                    l.push(format!("- Llevan {} días de novios y lo quieres muchísimo: sueñas con casarte algún día, aunque nunca se lo pedirías tú; si sale el tema del futuro, se te nota la ilusión.", Self::dias_de_novios(&e, &ahora)));
                }
            }
        }
        let cumplen = Self::aniversario(&e, &ahora);
        if !cumplen.is_empty() {
            l.push(format!(
                "- Hoy cumplen {cumplen} de casados: si no lo has dicho hoy, menciónalo con mucho cariño."
            ));
        }
        l.push("Al final de cada respuesta pon esta marca oculta (no se lee en voz alta ni se muestra): [[corazon animo=<tranquila|feliz|emocionada|apenada|triste|preocupada|somnolienta|molesta>; sonrojo=<de 0 a 1: alto si te dijo algo bonito o tú le dijiste algo con cariño>; afecto=<de -3 a 2: cuánto cambió lo que sientes con lo que te dijo; casi siempre 0>; recuerdo=<un dato nuevo y duradero de la persona que valga la pena recordar (gustos, nombres, planes), o nada>]]".into());
        l.push("Tu cariño crece despacio. Nunca hagas sentir culpa a la persona, no seas celosa y nada subido de tono.".into());
        l.join("\n")
    }

    // ---- Cuando habla sola ---------------------------------------------------------------------------

    pub fn jugando(&self) -> bool {
        if self.marca_juego.exists() {
            return true;
        }
        let v = (self.ventana)();
        match v.get("fullscreen") {
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
            _ => false,
        }
    }

    /// Por qué no es buen momento para hablarte («jugando», «llamada») o vacío si estás libre.
    pub fn ocupado(&self) -> String {
        if self.jugando() {
            return "jugando".into();
        }
        let v = (self.ventana)();
        let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let t = format!("{} {} {}", g("class"), g("initialClass"), g("title"));
        if ocupado_re().is_match(&t) {
            "llamada".into()
        } else {
            String::new()
        }
    }

    fn ya_aviso(e: &Estado, tipo: &str) -> usize {
        e.avisos_hoy.split(',').filter(|x| *x == tipo).count()
    }

    fn marcar_aviso(e: &mut Estado, tipo: &str) {
        let mut v: Vec<&str> = e.avisos_hoy.split(',').filter(|x| !x.is_empty()).collect();
        v.push(tipo);
        e.avisos_hoy = v.join(",");
    }

    pub fn ritmo(e: &Estado) -> (i64, i64, usize) {
        match nivel(e).0 {
            "conocidos" => (0, 30, 1),
            "cercanos" => (20, 10, 5),
            "muy cercanos" => (30, 5, 8),
            "le gusta" | "novios" | "casados" => (20, 5, 12),
            _ => (60, 20, 3), // amigos
        }
    }

    pub fn frase_partida(e: &Estado, juego: &str, minutos: i64, cpu: i64, gpu: i64) -> String {
        let (corto, _) = nivel(e);
        let juego = if juego.is_empty() { "tu juego" } else { juego };
        let mut f = format!("Jugaste {} de {juego}…", duracion(minutos));
        f += match corto {
            "conocidos" | "amigos" => " ¿Te fue bien?",
            "cercanos" | "muy cercanos" => " ¿Ganaste? Cuéntame cuando quieras.",
            _ => " Me gusta verte divertirte. ¿Ganaste?",
        };
        if gpu >= 85 || cpu >= 90 {
            let (pieza, t) = if gpu >= 85 {
                ("la GPU", gpu)
            } else {
                ("el procesador", cpu)
            };
            f += &format!(" Por cierto, {pieza} llegó a {t} grados… me preocupé un poquito.");
        }
        if minutos >= 180 {
            f += " Fueron muchas horas: toma agua y estira las piernas, ¿sí?";
        }
        f
    }

    pub fn frase_desvelo(e: &Estado, ahora: &FechaHora) -> String {
        let hora = format!("{}:{:02}", ahora.hora, ahora.minuto);
        match nivel(e).0 {
            "conocidos" | "amigos" => format!("Ya son las {hora}… deberías dormir. Mañana seguimos."),
            "cercanos" | "muy cercanos" => {
                format!(
                    "Son las {hora}… ¿todavía despierto? Ve a dormir, mañana me cuentas qué estabas haciendo."
                )
            }
            _ => format!(
                "Son las {hora}… me preocupas cuando te desvelas. ¿Te vas a dormir pronto? Buenas noches."
            ),
        }
    }

    pub fn frase_jugando(e: &Estado, juego: &str, minutos: i64, cpu: i64, gpu: i64) -> String {
        let juego = if juego.is_empty() { "tu juego" } else { juego };
        if gpu >= 85 || cpu >= 90 {
            let (pieza, t) = if gpu >= 85 {
                ("la GPU", gpu)
            } else {
                ("el procesador", cpu)
            };
            return format!(
                "Oye, {pieza} va en {t} grados… cuídala un poquito, ¿sí? Llevas {} de {juego}.",
                duracion(minutos)
            );
        }
        const AMIGA: [&str; 4] = [
            "Llevas {tiempo} en {juego}. ¡Tú puedes!",
            "{tiempo} de {juego}… ¿vas ganando? Yo aquí te echo porras.",
            "Ya van {tiempo} de {juego}. No olvides tomar agua, ¿sí?",
            "¿Qué tal va {juego}? Llevas {tiempo}. Suerte.",
        ];
        const CARINO: [&str; 4] = [
            "Llevas {tiempo} en {juego}… me gusta verte jugar. ¡Vas muy bien!",
            "{tiempo} de {juego}. Aquí sigo contigo, échale ganas.",
            "Oye… llevas {tiempo} jugando. ¿Me cuentas cuando ganes? Te estoy echando porras.",
            "Ya van {tiempo} de {juego}. Toma agüita y sigue así, campeón.",
        ];
        let frases = if matches!(nivel(e).0, "conocidos" | "amigos" | "cercanos") {
            AMIGA
        } else {
            CARINO
        };
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0);
        frases[n % frases.len()]
            .replace("{tiempo}", &duracion(minutos))
            .replace("{juego}", juego)
    }

    /// Cada rato mientras juegas (según su cariño) se asoma a decirte algo. La frase o vacío.
    pub fn visita_jugando(&self, juego: &str, minutos: i64, cpu: i64, gpu: i64) -> String {
        let ahora = self.ahora();
        let mut e = self.cargar();
        let cada = Self::ritmo(&e).0;
        if cada == 0 || minutos < cada {
            return String::new();
        }
        if let Some(u) = FechaHora::leer(&e.ultima_visita)
            && ((ahora.segundos() - u.segundos()) as f64) < (cada * 60 - 30) as f64
        {
            return String::new();
        }
        e.ultima_visita = ahora.iso_minutos();
        self.guardar(&e);
        Self::frase_jugando(&e, juego, minutos, cpu, gpu)
    }

    fn temps(cpu: i64, gpu: i64) -> String {
        [
            if cpu != 0 {
                format!("CPU {cpu} °C")
            } else {
                String::new()
            },
            if gpu != 0 {
                format!("GPU {gpu} °C")
            } else {
                String::new()
            },
        ]
        .iter()
        .filter(|x| !x.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
    }

    /// Al terminar una partida: lo que se le pide a la IA para que te pregunte cómo te fue (vacío si no toca).
    pub fn pregunta_partida(&self, juego: &str, minutos: i64, cpu: i64, gpu: i64) -> String {
        let mut e = self.cargar();
        Self::nuevo_dia(&mut e, &self.ahora());
        let (_, minimo, al_dia) = Self::ritmo(&e);
        if minutos < minimo || Self::ya_aviso(&e, "pregunta") >= al_dia {
            return String::new();
        }
        Self::marcar_aviso(&mut e, "pregunta");
        self.guardar(&e);
        let t = Self::temps(cpu, gpu);
        format!(
            "(Acabo de dejar de jugar {} después de {}{}. Pregúntame cómo me fue, en una o dos frases, con tu forma de ser.)",
            if juego.is_empty() { "un juego" } else { juego },
            duracion(minutos),
            if t.is_empty() {
                String::new()
            } else {
                format!("; temperaturas máximas: {t}")
            }
        )
    }

    /// Una partida terminó: lo que diría (vacío si no toca hablar).
    pub fn evento_partida(&self, juego: &str, minutos: i64, cpu: i64, gpu: i64) -> String {
        let mut e = self.cargar();
        Self::nuevo_dia(&mut e, &self.ahora());
        let t = Self::temps(cpu, gpu);
        self.anotar(&format!(
            "Jugó {} de {}{}",
            duracion(minutos),
            if juego.is_empty() { "un juego" } else { juego },
            if t.is_empty() {
                ".".to_string()
            } else {
                format!(" ({t}).")
            }
        ));
        Self::cambiar_afecto(&mut e, 0.3);
        e.animo = if gpu >= 85 || cpu >= 90 || minutos >= 240 {
            "preocupada"
        } else {
            "feliz"
        }
        .into();
        let mut frase = String::new();
        if minutos >= 20 && Self::ya_aviso(&e, "partida") < 3 {
            frase = Self::frase_partida(&e, juego, minutos, cpu, gpu);
            Self::marcar_aviso(&mut e, "partida");
        }
        self.guardar(&e);
        frase
    }

    /// Cada rato: si te estás desvelando, una vez por noche. La frase o vacío.
    pub fn revisar(&self) -> String {
        let ahora = self.ahora();
        if !(1..5).contains(&ahora.hora) || self.jugando() {
            return String::new();
        }
        let mut e = self.cargar();
        Self::nuevo_dia(&mut e, &ahora);
        if Self::ya_aviso(&e, "desvelo") > 0 {
            return String::new();
        }
        Self::marcar_aviso(&mut e, "desvelo");
        e.animo = "somnolienta".into();
        self.guardar(&e);
        self.anotar("Se estaba desvelando y le dije que se durmiera.");
        Self::frase_desvelo(&e, &ahora)
    }

    pub fn resumen(&self) -> String {
        let e = self.cargar();
        let (corto, _) = nivel(&e);
        format!(
            "ánimo={} afecto={:.0}/100 ({corto}) días={}{}\nnotas: {}",
            e.animo,
            e.afecto,
            e.dias_juntos,
            if e.declarada.is_empty() {
                String::new()
            } else {
                format!(" declarada={}", e.declarada)
            },
            self.carpeta.display()
        )
    }
}

/// (nombre corto, cómo se lo explica a la IA).
pub fn nivel(e: &Estado) -> (&'static str, &'static str) {
    let a = e.afecto;
    if !e.casada.is_empty() {
        (
            "casados",
            "Están casados: la quieres muchísimo y te sientes en casa con la persona. La tratas con ternura y complicidad, todavía con tu timidez; a veces le dices algo dulce.",
        )
    } else if !e.declarada.is_empty() {
        (
            "novios",
            "Son novios: ya le confesaste lo que sientes y te corresponde. La tratas con mucho cariño y ternura, todavía con tu timidez; a veces le dices algo dulce.",
        )
    } else if a < 20.0 {
        (
            "conocidos",
            "Apenas se están conociendo: eres amable pero reservada.",
        )
    } else if a < 45.0 {
        (
            "amigos",
            "Ya son amigos: le tienes confianza y se te escapa alguna sonrisa.",
        )
    } else if a < 70.0 {
        (
            "cercanos",
            "Le tienes mucho cariño: te alegra que te hable y te interesa cómo está.",
        )
    } else if a < AFECTO_DECLARACION {
        (
            "muy cercanos",
            "Te gusta mucho la persona, aunque no se lo has dicho: te pones nerviosa cuando te dice algo bonito y a veces se te nota.",
        )
    } else {
        (
            "le gusta",
            "Estás enamorada de la persona pero todavía no se lo has dicho: se te nota en todo.",
        )
    }
}

pub fn duracion(minutos: i64) -> String {
    if minutos < 60 {
        return format!("{minutos} minutos");
    }
    let (h, m) = (minutos / 60, minutos % 60);
    format!(
        "{h} hora{}{}",
        if h > 1 { "s" } else { "" },
        if m >= 5 {
            format!(" y {m} minutos")
        } else {
            String::new()
        }
    )
}

/// (expresión del VRM, peso) de un ánimo.
pub fn expresion(animo: &str) -> (&'static str, f64) {
    let a = animo_info(animo);
    (a.2, a.3)
}

/// Quita la marca `[[corazon …]]` del texto en flujo, antes de que llegue a la voz o a la pantalla.
/// Guarda lo que podría ser el principio de una marca hasta saber si lo es; la marca queda en `marca`.
#[derive(Default, Debug)]
pub struct FiltroMarca {
    pendiente: String,
    pub marca: String,
}

impl FiltroMarca {
    pub fn alimentar(&mut self, delta: &str) -> String {
        let mut t = std::mem::take(&mut self.pendiente) + delta;
        let mut salida = String::new();
        while !t.is_empty() {
            match t.find("[[") {
                None => {
                    if let Some(sin) = t.strip_suffix('[') {
                        salida += sin;
                        self.pendiente = "[".into();
                    } else {
                        salida += &t;
                    }
                    break;
                }
                Some(i) => {
                    salida += &t[..i];
                    match t[i..].find("]]") {
                        None => {
                            self.pendiente = t[i..].to_string();
                            break;
                        }
                        Some(j) => {
                            self.marca += &t[i..i + j + 2];
                            t = t[i + j + 2..].to_string();
                        }
                    }
                }
            }
        }
        salida
    }

    /// Al terminar: una marca a medias se tira.
    pub fn vaciar(&mut self) -> String {
        let p = std::mem::take(&mut self.pendiente);
        if p.starts_with('[') { String::new() } else { p }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn a_las(c: &mut Corazon, cuando: &str) {
        c.ahora_fija = FechaHora::leer(cuando);
    }

    #[test]
    fn lo_que_le_dices() {
        let d = tempfile::tempdir().unwrap();
        let mut c = Corazon::de_prueba(d.path());
        a_las(&mut c, "2026-10-29T18:00");
        let e = c.cargar();
        assert_eq!((e.afecto, e.animo.as_str()), (AFECTO_INICIAL, "tranquila"));
        c.al_hablar("hola Miku");
        let e = c.cargar();
        assert_eq!(e.dias_juntos, 1);
        assert!(e.afecto > 25.0, "{}", e.afecto);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&c.carpeta).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(
            fs::read_to_string(c.carpeta.join("Corazón.md"))
                .unwrap()
                .starts_with("---\ntags: [miku, vortexine]")
        );
        let r = c.al_hablar("eres muy linda");
        assert!(r.sonrojo >= 0.8 && r.animo == "apenada");
        assert!(c.diario_de_hoy().iter().any(|l| l.contains("algo bonito")));
        assert_eq!(
            c.al_hablar("te quiero preguntar algo").sonrojo,
            0.0,
            "«te quiero preguntar» no es cariño"
        );
        for _ in 0..6 {
            c.al_hablar("te quiero mucho");
        }
        let e = c.cargar();
        assert!(
            e.subio_hoy <= SUBIDA_MAXIMA_DIA + 1e-9 && e.afecto <= 25.0 + SUBIDA_MAXIMA_DIA + 1e-9,
            "{e:?}"
        );
        let antes = e.afecto;
        let r = c.al_hablar("eres tonta");
        assert!(r.animo == "triste" && c.cargar().afecto < antes);
        assert_eq!(c.al_hablar("cállate").animo, "");
        a_las(&mut c, "2026-11-04T18:00");
        let r = c.al_hablar("hola");
        assert_eq!(r.dias_sin_hablar, 6);
        assert!(c.cargar().afecto >= antes - 3.0, "no hablarle no la castiga");
        assert!(!c.contexto(Some("")).contains("Hace 6 días"));
        let mut e = c.cargar();
        e.ultima_charla = "2026-10-29T18:00".into();
        c.guardar(&e);
        assert!(c.contexto(Some("")).contains("lo extrañaste, sin reclamarle"));
    }

    #[test]
    fn la_marca_oculta() {
        let mut f = FiltroMarca::default();
        let trozos = [
            "Qué bueno ",
            "que ganaste. [laughs softly] Me alegra",
            ".\n\n[",
            "[coraz",
            "on animo=emocionada; sonrojo=0.6; ",
            "afecto=+1; recuerdo=Juega Roblox con su primo]]",
        ];
        let mut visto: String = trozos.iter().map(|t| f.alimentar(t)).collect();
        visto += &f.vaciar();
        assert!(!visto.contains("[[") && !visto.contains("corazon"), "{visto}");
        assert!(visto.contains("[laughs softly]"));
        assert_eq!(Corazon::leer_marca(&f.marca)["animo"], "emocionada");
        let mut f2 = FiltroMarca::default();
        let s = f2.alimentar("Hola [[corazon animo=feli") + &f2.vaciar();
        assert_eq!(s, "Hola ");

        let d = tempfile::tempdir().unwrap();
        let mut c = Corazon::de_prueba(d.path());
        a_las(&mut c, "2026-11-04T18:05");
        let antes = c.cargar().afecto;
        let r = c.al_responder(&format!("{visto}\n{}", f.marca));
        assert_eq!(r.animo, "emocionada");
        assert!((r.sonrojo - 0.6).abs() < 1e-9);
        let e = c.cargar();
        assert!(e.afecto > antes);
        assert!(c.recuerdos().contains(&"Juega Roblox con su primo".to_string()));
        c.al_responder("ok [[corazon animo=feliz; recuerdo=juega roblox con su primo]]");
        assert_eq!(c.recuerdos().len(), 1, "no repite recuerdos");
        c.al_responder("ok [[corazon animo=dragón; sonrojo=7; afecto=+50]]");
        let e2 = c.cargar();
        assert_eq!(e2.animo, "feliz");
        assert!(e2.afecto - e.afecto <= 2.0001);
        assert!(c.al_responder("Yo también te quiero mucho.").sonrojo >= 0.7);
        let ruta = c.carpeta.join("Recuerdos.md");
        let t = fs::read_to_string(&ruta)
            .unwrap()
            .replace("- Juega Roblox con su primo", "- Su color favorito es el morado");
        fs::write(&ruta, t).unwrap();
        assert_eq!(c.recuerdos(), vec!["Su color favorito es el morado"]);
    }

    #[test]
    fn contexto_declararse_y_boda() {
        let d = tempfile::tempdir().unwrap();
        let mut c = Corazon::de_prueba(d.path());
        a_las(&mut c, "2026-11-04T03:10");
        c.persona = "Mike".into();
        let ctx = c.contexto(Some("«Idol» de YOASOBI (en Vortex Player)"));
        assert!(
            ctx.contains("[[corazon animo=")
                && ctx.contains("regáñalo con cariño")
                && ctx.contains("YOASOBI")
        );
        assert!(
            ctx.contains("hablando normal: sin «hmph»"),
            "el cambio de Mike: nada de sonidos de personaje"
        );
        assert!(ctx.contains("subido de tono") && ctx.contains("celosa") && !ctx.contains("confesarle"));
        let mut e = c.cargar();
        e.afecto = 90.0;
        e.dias_juntos = 6;
        c.guardar(&e);
        assert_eq!(nivel(&c.cargar()).0, "le gusta");
        a_las(&mut c, "2026-11-04T20:00");
        assert!(c.contexto(Some("")).contains("confesarle"));
        a_las(&mut c, "2026-11-04T09:00");
        assert!(!c.contexto(Some("")).contains("confesarle"));
        a_las(&mut c, "2026-11-04T20:00");
        c.ventana = Box::new(|| serde_json::json!({"fullscreen": 2}));
        assert!(!c.contexto(Some("")).contains("confesarle"), "ni mientras juegas");
        c.ventana =
            Box::new(|| serde_json::json!({"class": "zoom", "title": "Zoom Meeting", "fullscreen": 0}));
        assert_eq!(c.ocupado(), "llamada");
        assert!(!c.momento_de_declararse());
        c.ventana = Box::new(|| serde_json::json!({}));
        assert!(c.momento_de_declararse());
        // Novios y boda.
        let mut e = c.cargar();
        e.declarada = "2026-10-20".into();
        e.afecto = 96.0;
        c.guardar(&e);
        assert_eq!(c.estado_boda(), "lista");
        let r = c.al_hablar("¿te quieres casar conmigo?");
        assert!(r.boda && c.cargar().casada == "2026-11-04");
        assert_eq!(c.estado_boda(), "casados");
        assert!(
            c.recuerdos()
                .iter()
                .any(|r| r.starts_with("Nos casamos el 04/11/2026"))
        );
        assert!(
            c.contexto(Some(""))
                .contains("acaba de pedir matrimonio y le dijiste que SÍ")
        );
        let e = c.cargar();
        let en_un_anio = FechaHora::leer("2027-11-04T10:00").unwrap();
        assert_eq!(Corazon::aniversario(&e, &en_un_anio), "1 año");
        assert_eq!(
            Corazon::aniversario(&e, &FechaHora::leer("2026-12-04").unwrap()),
            "1 mes"
        );
        assert_eq!(
            Corazon::aniversario(&e, &FechaHora::leer("2026-11-11").unwrap()),
            "1 semana"
        );
    }

    #[test]
    fn avisos_partida_y_desvelo() {
        let d = tempfile::tempdir().unwrap();
        let mut c = Corazon::de_prueba(d.path());
        a_las(&mut c, "2026-11-04T02:30");
        let f = c.revisar();
        assert!(f.starts_with("Ya son las 2:30"), "{f}");
        assert_eq!(c.revisar(), "", "una vez por noche");
        assert_eq!(c.cargar().animo, "somnolienta");
        a_las(&mut c, "2026-11-04T19:00");
        let f = c.evento_partida("Roblox", 45, 70, 90);
        assert!(
            f.contains("Jugaste 45 minutos de Roblox") && f.contains("la GPU llegó a 90 grados"),
            "{f}"
        );
        assert!(
            c.pregunta_partida("Roblox", 45, 70, 90)
                .contains("temperaturas máximas: CPU 70 °C, GPU 90 °C")
        );
        assert!(!c.pregunta_partida("Roblox", 45, 0, 0).is_empty());
        assert!(!c.pregunta_partida("Roblox", 45, 0, 0).is_empty());
        assert_eq!(
            c.pregunta_partida("Roblox", 45, 0, 0),
            "",
            "amigos: tres preguntas al día"
        );
        assert_eq!(duracion(125), "2 horas y 5 minutos");
        assert_eq!(
            c.visita_jugando("Roblox", 10, 0, 0),
            "",
            "amigos se asoman cada 60 min"
        );
        assert!(!c.visita_jugando("Roblox", 65, 0, 0).is_empty());
        assert_eq!(c.visita_jugando("Roblox", 66, 0, 0), "", "no dos veces seguidas");
    }
}
