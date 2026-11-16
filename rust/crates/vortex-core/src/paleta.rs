//! La paleta de Vortexine: los colores del fondo de pantalla, iguales a los de `colores.py`.
//!
//! El escritorio (Serpantinum) genera con matugen la paleta del fondo en `qs_colors.json` (claves estilo
//! Catppuccin: base, mantle, text, blue…). Aquí se convierte en los colores que usa la interfaz
//! (`Tema.qml`: fondo, tarjeta, texto, azul…), asegurando contraste de texto de 4.5:1 o más (7:1 el texto
//! principal). Si el archivo no existe, está roto o le faltan claves, se usa [`RESPALDO`].
//!
//! La salida es **idéntica** a la de `colores.py` (lo comprueba `tests/paleta_igual_que_python.rs` con
//! paletas sacadas de la app de Python).

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Nombre del color (los de `Tema.qml`) → «#rrggbb» (o «#aarrggbb» en `velo` y `flotante`).
pub type Paleta = BTreeMap<String, String>;

/// La paleta fija de Vortexine's: la que se usa sin paleta del fondo (Windows, otro escritorio…).
pub const RESPALDO: [(&str, &str); 27] = [
    ("fondo", "#0b0d1a"),
    ("barra", "#11142a"),
    ("tarjeta", "#141833"),
    ("tarjetaHover", "#191e40"),
    ("campo", "#161a36"),
    ("borde", "#232850"),
    ("bordeCampo", "#2c3366"),
    ("bordeHover", "#3d4896"),
    ("texto", "#e8ebff"),
    ("suave", "#c3c8ea"),
    ("tenue", "#a3aad6"),
    ("azul", "#9ec5ff"),
    ("azulHover", "#b8d6ff"),
    ("azulPresion", "#86b2f2"),
    ("violeta", "#c6a8ff"),
    ("sobreAcento", "#0b0d1a"),
    ("peligro", "#ff9db0"),
    ("exito", "#8fe3b4"),
    ("realce", "#1a1f45"),
    ("realceFuerte", "#232a5c"),
    ("relleno", "#1f2552"),
    ("barraSel", "#222a5e"),
    ("barraHover", "#191e45"),
    ("peligroFondo", "#2b1a33"),
    ("peligroBorde", "#6b3550"),
    ("velo", "#cc060812"),
    ("flotante", "#e60b0d1a"),
];

/// La paleta de respaldo como mapa.
pub fn respaldo() -> Paleta {
    RESPALDO
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn respaldo_de(clave: &str) -> &'static str {
    RESPALDO
        .iter()
        .find(|(k, _)| *k == clave)
        .map(|(_, v)| *v)
        .unwrap_or("#000000")
}

// ---- Aritmética de color (la misma de colores.py, paso por paso) --------------------------------------

type Rgb = (f64, f64, f64);

/// «#rrggbb» o «#aarrggbb» (el alfa se ignora) → canales de 0 a 1.
fn rgb(h: &str) -> Option<Rgb> {
    let h = h.strip_prefix('#').unwrap_or(h);
    let h = if h.len() == 8 { h.get(2..)? } else { h };
    if h.len() < 6 {
        return None;
    }
    let canal = |i: usize| -> Option<f64> {
        let par = h.get(i..i + 2)?;
        if !par.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        u8::from_str_radix(par, 16).ok().map(|v| f64::from(v) / 255.0)
    };
    Some((canal(0)?, canal(2)?, canal(4)?))
}

/// `round()` de Python: a la mitad redondea al par (0.5 → 0, 1.5 → 2), no hacia afuera como Rust.
fn redondear_como_python(x: f64) -> f64 {
    let piso = x.floor();
    let resto = x - piso;
    let sube = resto > 0.5 || (resto == 0.5 && piso % 2.0 != 0.0);
    if sube { piso + 1.0 } else { piso }
}

fn hex(c: Rgb) -> String {
    let b = |v: f64| redondear_como_python(v * 255.0).clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.0), b(c.1), b(c.2))
}

/// a + (b − a) · t, por canal. Con un color inválido devuelve `a` tal cual.
pub fn mezclar(a: &str, b: &str, t: f64) -> String {
    match (rgb(a), rgb(b)) {
        (Some(x), Some(y)) => hex((
            x.0 + (y.0 - x.0) * t,
            x.1 + (y.1 - x.1) * t,
            x.2 + (y.2 - x.2) * t,
        )),
        _ => a.to_string(),
    }
}

/// Luminancia relativa (WCAG).
pub fn luminancia(h: &str) -> f64 {
    let lin = |v: f64| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = rgb(h).unwrap_or((0.0, 0.0, 0.0));
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// Contraste WCAG entre dos colores (1 a 21).
pub fn contraste(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminancia(a), luminancia(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn peor_contraste(color: &str, fondos: &[&str]) -> f64 {
    fondos
        .iter()
        .map(|f| contraste(color, f))
        .fold(f64::INFINITY, f64::min)
}

/// Acerca `color` al blanco o al negro (el que contraste más con los fondos) hasta llegar al contraste.
pub fn asegurar(color: &str, fondos: &[&str], minimo: f64) -> String {
    // El fondo con el que peor contrasta (el primero, si hay empate: como min() de Python).
    let mut peor = fondos[0];
    for f in &fondos[1..] {
        if contraste(color, f) < contraste(color, peor) {
            peor = f;
        }
    }
    let destino = if luminancia(peor) < 0.4 {
        "#ffffff"
    } else {
        "#000000"
    };
    let mut nuevo = color.to_string();
    for paso in 1..=20 {
        if peor_contraste(&nuevo, fondos) >= minimo {
            return nuevo;
        }
        nuevo = mezclar(color, destino, f64::from(paso) * 0.05);
    }
    nuevo
}

/// El primer candidato que dé 4.5:1 sobre el acento; si ninguno, negro o blanco según contraste.
pub fn texto_sobre(acento: &str, candidatos: &[&str]) -> String {
    if let Some(c) = candidatos.iter().find(|c| contraste(c, acento) >= 4.5) {
        return c.to_string();
    }
    if contraste("#000000", acento) >= contraste("#ffffff", acento) {
        "#000000"
    } else {
        "#ffffff"
    }
    .into()
}

// ---- matugen → Tema -------------------------------------------------------------------------------

fn valido<'a>(j: &'a Map<String, Value>, clave: &str) -> Option<&'a str> {
    let v = j.get(clave)?.as_str()?;
    (v.starts_with('#') && (v.len() == 7 || v.len() == 9) && rgb(v).is_some()).then_some(v)
}

/// Paleta de matugen (`qs_colors.json`) → colores de la interfaz. `None` si le faltan claves esenciales.
pub fn desde_matugen(j: &Map<String, Value>) -> Option<Paleta> {
    let g = |k: &str, otro: Option<&str>| valido(j, k).or_else(|| otro.and_then(|o| valido(j, o)));
    let base = g("base", Some("crust"))?;
    let mantle = g("mantle", Some("base"))?;
    let surface0 = g("surface0", None)?;
    let surface1 = g("surface1", Some("surface0"))?;
    let surface2 = g("surface2", Some("surface1"))?;
    let texto = g("text", None)?;
    let subtexto = g("subtext0", Some("text"))?;
    let azul = g("blue", None)?;
    let crust = g("crust", Some("base"));
    let violeta = g("peach", None).or_else(|| g("mauve", None)).unwrap_or(azul);
    let peligro = g("red", None).unwrap_or_else(|| respaldo_de("peligro"));
    let exito = g("green", None).unwrap_or_else(|| respaldo_de("exito"));

    let fondos = [base, mantle, surface0, surface1]; // sobre lo que se lee texto
    let texto = asegurar(texto, &fondos, 7.0);
    let suave = asegurar(&mezclar(&texto, subtexto, 0.45), &fondos, 4.5);
    let tenue = asegurar(subtexto, &fondos, 4.5);
    let sobre_base = [base, surface0];
    let azul = asegurar(azul, &sobre_base, 4.5);
    let violeta = asegurar(violeta, &sobre_base, 4.5);
    let peligro = asegurar(peligro, &sobre_base, 4.5);
    let exito = asegurar(exito, &sobre_base, 4.5);
    let oscuro = luminancia(base) < 0.4;
    let mut azul_hover = mezclar(&azul, if oscuro { "#ffffff" } else { "#000000" }, 0.22);
    let mut azul_presion = mezclar(&azul, if oscuro { "#000000" } else { "#ffffff" }, 0.14);

    // El texto del botón se tiene que leer sobre el azul normal, el de «hover» y el de «presionado»; si
    // ningún candidato lo logra en los tres, los estados se acercan al azul normal.
    let mut candidatos: Vec<&str> = Vec::new();
    if let Some(c) = crust {
        candidatos.push(c);
    }
    candidatos.extend([base, "#000000", "#ffffff"]);
    let estados = [azul.as_str(), azul_hover.as_str(), azul_presion.as_str()];
    let sobre = match candidatos.iter().find(|c| peor_contraste(c, &estados) >= 4.5) {
        Some(c) => c.to_string(),
        None => {
            let sobre = texto_sobre(&azul, &candidatos);
            for t in [0.75, 0.5, 0.25, 0.0] {
                azul_hover = if t != 0.0 {
                    mezclar(&azul, &azul_hover, t)
                } else {
                    azul.clone()
                };
                azul_presion = if t != 0.0 {
                    mezclar(&azul, &azul_presion, t)
                } else {
                    azul.clone()
                };
                if peor_contraste(&sobre, &[azul_hover.as_str(), azul_presion.as_str()]) >= 4.5 {
                    break;
                }
            }
            sobre
        }
    };

    let base6 = base.trim_start_matches('#');
    let base6 = &base6[base6.len().saturating_sub(6)..];
    let pares: Vec<(&str, String)> = vec![
        ("fondo", base.into()),
        ("barra", mantle.into()),
        ("tarjeta", surface0.into()),
        ("tarjetaHover", mezclar(surface0, surface1, 0.6)),
        ("campo", surface1.into()),
        ("borde", surface2.into()),
        ("bordeCampo", mezclar(surface2, subtexto, 0.25)),
        ("bordeHover", mezclar(surface2, &azul, 0.45)),
        ("texto", texto.clone()),
        ("suave", suave),
        ("tenue", tenue),
        ("azulHover", azul_hover),
        ("azulPresion", azul_presion),
        ("violeta", violeta),
        ("sobreAcento", sobre),
        ("realce", mezclar(surface0, &azul, 0.10)),
        ("realceFuerte", mezclar(surface0, &azul, 0.22)),
        ("relleno", mezclar(surface0, &texto, 0.10)),
        ("barraSel", mezclar(mantle, &azul, 0.20)),
        ("barraHover", mezclar(mantle, &azul, 0.09)),
        ("peligroFondo", mezclar(surface0, &peligro, 0.14)),
        ("peligroBorde", mezclar(surface0, &peligro, 0.45)),
        ("peligro", peligro),
        ("exito", exito),
        (
            "velo",
            format!("#cc{}", mezclar(base, "#000000", 0.35).trim_start_matches('#')),
        ),
        ("flotante", format!("#e6{base6}")),
        ("azul", azul),
    ];
    Some(pares.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// La paleta de un archivo de matugen, o el respaldo si no existe, está roto o le faltan claves.
pub fn leer(ruta: &Path) -> Paleta {
    fs::read_to_string(ruta)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().and_then(desde_matugen))
        .unwrap_or_else(respaldo)
}

/// La paleta de ahora, de `rutas::paleta_matugen()`.
pub fn actual() -> Paleta {
    leer(&crate::rutas::paleta_matugen())
}

/// Vigila el archivo de matugen sin hilos ni dependencias: se le llama seguido (cada 300 ms, desde un
/// temporizador de la interfaz o un hilo) y dice si la paleta cambió.
///
/// matugen reescribe el archivo en varios pasos: un cambio solo cuenta cuando el archivo se queda quieto
/// entre dos revisiones, así nunca se lee a medias (y no parpadea a la paleta de respaldo).
pub struct Vigia {
    ruta: PathBuf,
    firma: Option<(SystemTime, u64)>,
    pendiente: Option<(SystemTime, u64)>,
    actual: Paleta,
}

impl Vigia {
    pub fn new(ruta: PathBuf) -> Self {
        let firma = firma(&ruta);
        let actual = leer(&ruta);
        Vigia {
            ruta,
            firma,
            pendiente: None,
            actual,
        }
    }

    /// El de siempre: `rutas::paleta_matugen()`.
    pub fn de_matugen() -> Self {
        Self::new(crate::rutas::paleta_matugen())
    }

    pub fn actual(&self) -> &Paleta {
        &self.actual
    }

    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// `Some(paleta)` si cambió desde la última vez.
    pub fn revisar(&mut self) -> Option<&Paleta> {
        let ahora = firma(&self.ruta);
        if ahora == self.firma {
            self.pendiente = None;
            return None;
        }
        if self.pendiente != Some(ahora.unwrap_or((SystemTime::UNIX_EPOCH, u64::MAX))) {
            self.pendiente = Some(ahora.unwrap_or((SystemTime::UNIX_EPOCH, u64::MAX)));
            return None; // se espera a la siguiente vuelta: que matugen termine de escribir
        }
        self.pendiente = None;
        self.firma = ahora;
        let nueva = leer(&self.ruta);
        if nueva == self.actual {
            return None;
        }
        self.actual = nueva;
        Some(&self.actual)
    }
}

fn firma(ruta: &Path) -> Option<(SystemTime, u64)> {
    let m = fs::metadata(ruta).ok()?;
    Some((m.modified().ok()?, m.len()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn redondeo_igual_que_python() {
        assert_eq!(redondear_como_python(0.5), 0.0);
        assert_eq!(redondear_como_python(1.5), 2.0);
        assert_eq!(redondear_como_python(2.5), 2.0);
        assert_eq!(redondear_como_python(127.5), 128.0);
        assert_eq!(redondear_como_python(127.4), 127.0);
    }

    #[test]
    fn colores_invalidos() {
        assert!(rgb("#12345").is_none());
        assert!(rgb("#zzzzzz").is_none());
        assert_eq!(rgb("#ff000000"), Some((0.0, 0.0, 0.0)));
    }

    #[test]
    fn sin_archivo_es_el_respaldo() {
        assert_eq!(leer(Path::new("/no/existe/qs_colors.json")), respaldo());
    }

    #[test]
    fn el_vigia_espera_a_que_el_archivo_se_quede_quieto() {
        let c = crate::carpeta_de_prueba("vigia");
        let r = c.join("qs_colors.json");
        let mut v = Vigia::new(r.clone());
        assert_eq!(v.actual(), &respaldo());
        assert!(v.revisar().is_none());
        std::fs::write(
            &r,
            r##"{"base":"#1e1e2e","mantle":"#181825","surface0":"#313244","surface1":"#45475a",
                "surface2":"#585b70","text":"#cdd6f4","subtext0":"#a6adc8","blue":"#89b4fa"}"##,
        )
        .unwrap();
        assert!(v.revisar().is_none(), "la primera vez que lo ve cambiado, espera");
        let nueva = v.revisar().expect("quieto dos vueltas: ya cuenta").clone();
        assert_eq!(nueva["fondo"], "#1e1e2e");
        assert!(v.revisar().is_none(), "sin cambios, nada");
        std::fs::remove_file(&r).unwrap();
        v.revisar();
        assert_eq!(
            v.revisar().cloned(),
            Some(respaldo()),
            "si lo borran, vuelve al respaldo"
        );
    }
}
