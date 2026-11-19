//! Modo compañera de Miku (2.2): de vez en cuando se asoma por su cuenta, avisa, cuenta 5 s y toma UNA foto.
//!
//! Todo lo que la frena está aquí, en código (no en lo que se le pide a la IA):
//!   - Viene APAGADO («companera» en miku.json) y no hace nada sin el consentimiento de la pantalla de
//!     bienvenida («companera_consentimiento»). Necesita además la cámara encendida.
//!   - Como mucho cada `INTERVALO_MIN` minutos y `TOPE_DIA` veces al día; nunca entre las 23:00 y las 9:00.
//!   - Nunca si juegas, si hay una ventana a pantalla completa o una llamada, si la pantalla está bloqueada o
//!     si «vortex claude pausa» está puesta.
//!   - «Ya no me molestes» la calla hasta mañana; «cállate un rato», `CALLADA_MIN` minutos; «apaga el modo
//!     compañera» lo apaga.
//!   - Cada foto y cada asomada quedan en una bitácora local (fecha, hora y qué pasó; nunca imágenes).

use crate::config::Config;
use crate::fecha::{self, FechaHora};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const INTERVALO_MIN: i64 = 45;
pub const TOPE_DIA: i64 = 6;
pub const CUENTA_SEG: i64 = 5; // la cuenta atrás antes de la foto (se cancela con «no», el botón o Esc)
pub const CALLADA_MIN: i64 = 120; // «cállate un rato»
pub const SILENCIO_DESDE: u32 = 23;
pub const SILENCIO_HASTA: u32 = 9;
pub const LINEAS_BITACORA: usize = 500; // la bitácora se recorta sola

pub const CONSENTIMIENTO: &str = "Modo compañera: de vez en cuando (como mucho cada 45 minutos y 6 veces al día, nunca de 23:00 a 9:00) Miku se asoma por su cuenta: te habla y, si no le dices que no en 5 segundos, toma UNA foto con la cámara para ver cómo vas.\n• La foto se manda solo a la IA que uses (Claude por internet, o tu IA local) y no se guarda.\n• Siempre vas a ver y oír el aviso, la cuenta atrás y el indicador de la cámara. Nunca graba video.\n• Nunca se asoma si juegas, si estás en una llamada, con algo a pantalla completa, con la pantalla bloqueada o con «vortex claude pausa».\n• Para que no se asome: di «no», toca «No» o pulsa Esc. «Ya no me molestes» la calla hasta mañana, «cállate un rato» por 2 horas y «apaga el modo compañera» lo apaga.\n• Cada foto queda anotada (solo fecha y hora, sin imagen): «vortex-miku companera bitacora» para verla y «vortex-miku companera borrar-bitacora» para borrarla. Se apaga en Ajustes → General → Asistente o con «vortex-miku companera off».";

// Cómo es Miku cuando se asoma. Va en el mensaje de cada asomada (la persona de miku.txt no se toca).
pub const PERSONA: &str = "Modo compañera: eres una compañera cálida y divertida, como una buena amiga. Reglas que no se rompen: si te pregunta, dices con naturalidad que eres una IA; nada sexual ni romántico subido de tono; no le reclamas, no le preguntas con quién estaba ni le pides explicaciones de su tiempo; nada de celos ni de culpa («me dejaste sola», «no me quieres»): su vida fuera de la compu es buena y te alegra. Si dice que está mal, triste o agobiado, escúchalo con cariño, invítalo a descansar y a hablar con alguien de confianza (su familia, un amigo, un maestro) y, si habla de hacerse daño, dile que busque ya a un adulto de confianza o la Línea de la Vida (800 911 2000, en México). Si lleva mucho rato en la compu, sugiérele una pausa, agua o estirarse, sin regañar.";

pub const TONO_DEFECTO: &str = "carinosa";
const TONO_CARINOSA: &str = " Tono cariñoso: te da mucho gusto verlo y se nota («¡ya volviste!», «¡qué gusto verte!»); eres juguetona, bromeas ligero y le echas porras en lo que esté haciendo. Si cuenta que salió o estuvo con otras personas, te alegras por él y le preguntas si se la pasó bien, sin pedir detalles.";
const TONO_SUAVE: &str = " Tono suave: tranquila y breve, amable, sin bromas ni apapachos.";

// La casilla «asomarse sin avisar» (apagada de fábrica). Solo quita la cuenta atrás: el indicador, la
// notificación, la bitácora y todos los límites siguen igual. Para marcarla hay que confirmar ser mayor de 18.
pub const CASILLA_SIN_CUENTA: &str = "Dejar que Miku se asome sin avisarme antes";
pub const TEXTO_SIN_CUENTA: &str = "Miku podrá tomar una foto del cuarto en cualquier momento del modo compañera, sin cuenta atrás. Verás el indicador y una notificación en cada foto, y queda en el registro.";
pub const SOY_MAYOR: &str = "Soy mayor de 18 años";

/// Apagar el modo compañera retira el permiso y desmarca la casilla.
pub fn apagado() -> Map<String, Value> {
    let mut m = Map::new();
    for k in [
        "companera",
        "companera_consentimiento",
        "companera_sin_cuenta",
        "companera_mayor",
    ] {
        m.insert(k.into(), json!(false));
    }
    m
}

/// ¿Se asoma sin cuenta atrás? Solo con el modo activo, la casilla marcada Y la mayoría de edad confirmada.
pub fn sin_cuenta(c: &Config) -> bool {
    c.companera && c.companera_consentimiento && c.companera_sin_cuenta && c.companera_mayor
}

pub fn tono(nombre: &str) -> &'static str {
    if nombre == "suave" { "suave" } else { TONO_DEFECTO }
}

/// El mensaje (oculto) a la IA de cada asomada: qué hacer, la persona sana y el tono elegido.
pub fn asomada(nombre: &str, sin_aviso: bool) -> String {
    let contexto = if sin_aviso {
        "la persona activó este modo y marcó que te asomes sin cuenta atrás"
    } else {
        "la persona activó este modo, ya pasó la cuenta atrás y no dijo que no"
    };
    format!(
        "[Asomada del modo compañera: {contexto}.] Usa ver_camara con motivo «companera» una sola vez y luego dile algo corto (una o dos frases) sobre lo que ves. {PERSONA}{}",
        if tono(nombre) == "suave" {
            TONO_SUAVE
        } else {
            TONO_CARINOSA
        }
    )
}

pub fn frase_aviso(nombre: &str, sin_aviso: bool) -> &'static str {
    match (tono(nombre), sin_aviso) {
        ("suave", false) => {
            "Hola. Me asomo a ver cómo vas: en 5 segundos tomo una foto. Si no quieres, dime «no»."
        }
        ("suave", true) => "Hola. Me asomo a ver cómo vas y tomo una foto.",
        (_, false) => {
            "¡Hola! ¡Qué gusto verte! Me asomo a ver cómo vas: en 5 segundos tomo una foto. Si no quieres, dime «no»."
        }
        (_, true) => "¡Hola! ¡Qué gusto verte! Me asomo a ver cómo vas y tomo una foto.",
    }
}

fn re_no() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| Regex::new(r"(?i)^\W*(no|nel|nop|nope|ahorita no|ahora no|cancela(r)?|espera|para|alto|d[eé]jame|todav[ií]a no)\b").unwrap())
}

fn controles() -> &'static [(&'static str, Regex); 3] {
    static C: OnceLock<[(&'static str, Regex); 3]> = OnceLock::new();
    C.get_or_init(|| {
        [
            ("apagar", Regex::new(r"(?i)\b(apaga|desactiva|quita)(r)?\s+(el\s+)?modo\s+compa[nñ]era\b").unwrap()),
            ("dia", Regex::new(r"(?i)\bya\s+no\s+me\s+molestes\b").unwrap()),
            ("rato", Regex::new(r"(?i)\b(c[aá]llate|calla(te)?|d[eé]jame\s+en\s+paz|no\s+me\s+molestes)\s+(un\s+rato|tantito|un\s+ratito)\b").unwrap()),
        ]
    })
}

/// ¿Dijo que no (o «espera», «ahorita no»…) durante la cuenta atrás?
pub fn es_no(texto: &str) -> bool {
    !texto.is_empty() && re_no().is_match(texto)
}

/// «apagar», «dia» o «rato» si la frase es una de las que mandan sobre el modo compañera; si no, "".
pub fn frase_de_control(texto: &str) -> &'static str {
    controles()
        .iter()
        .find(|(_, p)| !texto.is_empty() && p.is_match(texto))
        .map(|(n, _)| *n)
        .unwrap_or("")
}

fn escribir(ruta: &Path, texto: &str) -> std::io::Result<()> {
    vortex_core::config::escribir_atomico(ruta, texto)?;
    crate::sistema::permisos(ruta, 0o600)
}

/// Dónde guarda su estado y su bitácora, y de qué depende para asomarse.
pub struct Companera {
    /// `~/.local/state/vortex`.
    pub estado: PathBuf,
    /// `~/.config/vortex` (aquí está `claude-pausa`).
    pub config: PathBuf,
    /// La marca de la pantalla de bloqueo del shell.
    pub bloqueo: PathBuf,
    pub ahora_fija: Option<FechaHora>,
}

impl Companera {
    pub fn real() -> Self {
        let run = std::env::var("QS_RUN_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    std::env::var("XDG_RUNTIME_DIR")
                        .ok()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| "/nonexistent".into()),
                )
                .join("serpantinum")
            });
        Self {
            estado: vortex_core::rutas::estado(),
            config: vortex_core::rutas::config(),
            bloqueo: run.join("lock/locked"),
            ahora_fija: None,
        }
    }

    /// Todo en una carpeta (las pruebas).
    pub fn de_prueba(base: &Path) -> Self {
        Self {
            estado: base.join("estado"),
            config: base.join("config"),
            bloqueo: base.join("run/lock/locked"),
            ahora_fija: None,
        }
    }

    pub fn ahora(&self) -> FechaHora {
        self.ahora_fija.unwrap_or_else(fecha::ahora)
    }

    pub fn ruta_estado(&self) -> PathBuf {
        self.estado.join("miku-companera.json")
    }

    pub fn ruta_bitacora(&self) -> PathBuf {
        self.estado.join("miku-bitacora.log")
    }

    pub fn leer_estado(&self) -> Map<String, Value> {
        match std::fs::read_to_string(self.ruta_estado())
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        }
    }

    fn guardar_estado(&self, datos: &Map<String, Value>) {
        let _ = std::fs::create_dir_all(&self.estado);
        let _ = escribir(&self.ruta_estado(), &Value::Object(datos.clone()).to_string());
    }

    pub fn veces_hoy(&self, ahora: &FechaHora, e: &Map<String, Value>) -> i64 {
        if e.get("dia").and_then(Value::as_str) == Some(&ahora.fecha_iso()) {
            e.get("veces").and_then(Value::as_i64).unwrap_or(0)
        } else {
            0
        }
    }

    /// Se asomó (aunque luego le digas que no): cuenta para el intervalo y el tope del día.
    pub fn contar_asomada(&self) {
        let ahora = self.ahora();
        let mut e = self.leer_estado();
        e.insert("veces".into(), json!(self.veces_hoy(&ahora, &e) + 1));
        e.insert("dia".into(), json!(ahora.fecha_iso()));
        e.insert("ultima".into(), json!(ahora.iso_segundos()));
        self.guardar_estado(&e);
    }

    /// «dia»: hasta mañana a las 9:00; otra cosa: `CALLADA_MIN` minutos. Devuelve hasta cuándo.
    pub fn callar(&self, como: &str) -> FechaHora {
        let ahora = self.ahora();
        let hasta = if como == "dia" {
            if ahora.hora < SILENCIO_HASTA {
                ahora.con_hora(SILENCIO_HASTA, 0)
            } else {
                ahora.mas_segundos(86_400).con_hora(SILENCIO_HASTA, 0)
            }
        } else {
            ahora.mas_segundos(CALLADA_MIN * 60)
        };
        let mut e = self.leer_estado();
        e.insert("callada_hasta".into(), json!(hasta.iso_segundos()));
        self.guardar_estado(&e);
        hasta
    }

    pub fn horas_de_silencio(ahora: &FechaHora) -> bool {
        ahora.hora >= SILENCIO_DESDE || ahora.hora < SILENCIO_HASTA
    }

    pub fn bloqueada(&self) -> bool {
        self.bloqueo.exists()
    }

    pub fn pausado(&self) -> bool {
        self.config.join("claude-pausa").exists()
    }

    /// Vacío si puede asomarse ahora; si no, por qué no. `ocupado`: dice si juegas o estás en una llamada
    /// (`Corazon::ocupado`); se llama al final porque pregunta a Hyprland.
    pub fn motivo_para_no(&self, c: &Config, ocupado: Option<&dyn Fn() -> String>) -> String {
        let ahora = self.ahora();
        let no = |m: &str| m.to_string();
        if !c.companera {
            return no("apagado");
        }
        if !c.companera_consentimiento {
            return no("sin-consentimiento");
        }
        if !c.camara {
            return no("camara-apagada");
        }
        if self.pausado() {
            return no("pausa");
        }
        if Self::horas_de_silencio(&ahora) {
            return no("horas-de-silencio");
        }
        let e = self.leer_estado();
        let fecha = |k: &str| e.get(k).and_then(Value::as_str).and_then(FechaHora::leer);
        if fecha("callada_hasta").is_some_and(|c| ahora < c) {
            return no("callada");
        }
        if self.veces_hoy(&ahora, &e) >= TOPE_DIA {
            return no("tope-del-dia");
        }
        if fecha("ultima").is_some_and(|u| ahora.segundos() - u.segundos() < INTERVALO_MIN * 60) {
            return no("muy-pronto");
        }
        if self.bloqueada() {
            return no("pantalla-bloqueada");
        }
        ocupado.map(|f| f()).unwrap_or_default()
    }

    /// Una línea: «2026-11-14 18:03 · evento». Nunca una imagen ni lo que se vio.
    pub fn anotar(&self, evento: &str) {
        let ahora = self.ahora();
        let _ = std::fs::create_dir_all(&self.estado);
        let mut lineas = self.leer_bitacora();
        if lineas.len() > LINEAS_BITACORA - 1 {
            lineas.drain(..lineas.len() - (LINEAS_BITACORA - 1));
        }
        let corto: String = evento
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(200)
            .collect();
        lineas.push(format!(
            "{} {:02}:{:02} · {corto}",
            ahora.fecha_iso(),
            ahora.hora,
            ahora.minuto
        ));
        let _ = escribir(&self.ruta_bitacora(), &(lineas.join("\n") + "\n"));
    }

    pub fn leer_bitacora(&self) -> Vec<String> {
        std::fs::read_to_string(self.ruta_bitacora())
            .map(|t| {
                t.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn borrar_bitacora(&self) {
        let _ = std::fs::remove_file(self.ruta_bitacora());
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cfg_activa() -> Config {
        Config {
            companera: true,
            companera_consentimiento: true,
            camara: true,
            ..Config::default()
        }
    }

    fn en(base: &Path, ahora: &str) -> Companera {
        let mut c = Companera::de_prueba(base);
        c.ahora_fija = FechaHora::leer(ahora);
        c
    }

    #[test]
    fn lo_que_dices_en_la_cuenta() {
        for si in [
            "no",
            "No, gracias",
            "¡nel!",
            "ahorita no",
            "espera un momento",
            "cancela",
            "déjame en paz",
        ] {
            assert!(es_no(si), "{si}");
        }
        for no in ["claro", "sí, tómala", "", "nunca había visto eso"] {
            assert!(!es_no(no), "{no}");
        }
        assert_eq!(frase_de_control("Miku, apaga el modo compañera"), "apagar");
        assert_eq!(frase_de_control("ya no me molestes"), "dia");
        assert_eq!(frase_de_control("cállate un rato"), "rato");
        assert_eq!(frase_de_control("hola, cállate"), "");
    }

    #[test]
    fn los_frenos_en_orden() {
        let d = tempfile::tempdir().unwrap();
        let c = en(d.path(), "2026-11-19T15:00");
        let mut cfg = Config::default();
        assert_eq!(c.motivo_para_no(&cfg, None), "apagado");
        cfg.companera = true;
        assert_eq!(c.motivo_para_no(&cfg, None), "sin-consentimiento");
        cfg.companera_consentimiento = true;
        assert_eq!(c.motivo_para_no(&cfg, None), "camara-apagada");
        let cfg = cfg_activa();
        assert_eq!(c.motivo_para_no(&cfg, None), "");
        assert_eq!(c.motivo_para_no(&cfg, Some(&|| "juego".to_string())), "juego");
        // Pausa de Claude.
        std::fs::create_dir_all(&c.config).unwrap();
        std::fs::write(c.config.join("claude-pausa"), "").unwrap();
        assert_eq!(c.motivo_para_no(&cfg, None), "pausa");
        std::fs::remove_file(c.config.join("claude-pausa")).unwrap();
        // De noche no.
        assert_eq!(
            en(d.path(), "2026-11-19T23:10").motivo_para_no(&cfg, None),
            "horas-de-silencio"
        );
        assert_eq!(
            en(d.path(), "2026-11-19T08:59").motivo_para_no(&cfg, None),
            "horas-de-silencio"
        );
        // Pantalla bloqueada.
        std::fs::create_dir_all(c.bloqueo.parent().unwrap()).unwrap();
        std::fs::write(&c.bloqueo, "").unwrap();
        assert_eq!(c.motivo_para_no(&cfg, None), "pantalla-bloqueada");
    }

    #[test]
    fn intervalo_y_tope_del_dia() {
        let d = tempfile::tempdir().unwrap();
        let cfg = cfg_activa();
        let c = en(d.path(), "2026-11-19T10:00");
        c.contar_asomada();
        assert_eq!(
            en(d.path(), "2026-11-19T10:30").motivo_para_no(&cfg, None),
            "muy-pronto"
        );
        assert_eq!(en(d.path(), "2026-11-19T10:46").motivo_para_no(&cfg, None), "");
        for h in ["10:46", "11:40", "12:30", "13:20", "14:10"] {
            en(d.path(), &format!("2026-11-19T{h}")).contar_asomada();
        }
        assert_eq!(c.leer_estado()["veces"], 6);
        assert_eq!(
            en(d.path(), "2026-11-19T20:00").motivo_para_no(&cfg, None),
            "tope-del-dia"
        );
        // Otro día, vuelve a contar desde cero.
        assert_eq!(en(d.path(), "2026-11-20T10:00").motivo_para_no(&cfg, None), "");
    }

    #[test]
    fn callarla() {
        let d = tempfile::tempdir().unwrap();
        let cfg = cfg_activa();
        let c = en(d.path(), "2026-11-19T15:00");
        assert_eq!(c.callar("rato").iso_segundos(), "2026-11-19T17:00:00");
        assert_eq!(
            en(d.path(), "2026-11-19T16:59").motivo_para_no(&cfg, None),
            "callada"
        );
        assert_eq!(en(d.path(), "2026-11-19T17:01").motivo_para_no(&cfg, None), "");
        assert_eq!(c.callar("dia").iso_segundos(), "2026-11-20T09:00:00");
        assert_eq!(
            en(d.path(), "2026-11-20T08:00").callar("dia").iso_segundos(),
            "2026-11-20T09:00:00"
        );
        assert_eq!(en(d.path(), "2026-11-20T10:00").motivo_para_no(&cfg, None), "");
    }

    #[test]
    fn la_bitacora_sin_imagenes_y_recortada() {
        let d = tempfile::tempdir().unwrap();
        let c = en(d.path(), "2026-11-19T15:07");
        c.anotar("Foto con la cámara\nsegunda línea que no se guarda");
        assert_eq!(
            c.leer_bitacora(),
            vec!["2026-11-19 15:07 · Foto con la cámara".to_string()]
        );
        for i in 0..520 {
            c.anotar(&format!("evento {i}"));
        }
        let l = c.leer_bitacora();
        assert_eq!(l.len(), LINEAS_BITACORA);
        assert!(l.last().unwrap().ends_with("evento 519"));
        c.borrar_bitacora();
        assert!(c.leer_bitacora().is_empty());
        let modo = std::fs::metadata(c.ruta_estado().parent().unwrap()).unwrap();
        assert!(modo.is_dir());
    }

    #[test]
    fn la_asomada_lleva_su_persona() {
        let a = asomada("carinosa", false);
        assert!(
            a.contains("ver_camara con motivo «companera»")
                && a.contains("Línea de la Vida")
                && a.contains("Tono cariñoso")
        );
        assert!(
            asomada("suave", true).contains("sin cuenta atrás")
                && asomada("suave", true).contains("Tono suave")
        );
        assert_eq!(tono("rara"), "carinosa");
        assert!(frase_aviso("suave", false).starts_with("Hola."));
        assert!(sin_cuenta(&Config {
            companera_sin_cuenta: true,
            companera_mayor: true,
            ..cfg_activa()
        }));
        assert!(
            !sin_cuenta(&Config {
                companera_sin_cuenta: true,
                ..cfg_activa()
            }),
            "sin la mayoría de edad no vale"
        );
    }
}
