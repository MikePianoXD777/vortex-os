//! Utilidades de texto que usa la interfaz (las de `puente.py`): sin acentos, tamaños, fechas cortas y el
//! Markdown con los `[[enlaces]]` listos para `Text.MarkdownText`.

use crate::tiempo;
use regex::Regex;
use std::sync::LazyLock;

/// La letra sin su acento (lo que queda de una letra cuando se descompone con NFD y se quitan las marcas).
/// Cubre el latín de Europa y América (español, portugués, francés, alemán, italiano, …).
fn sin_marca(c: char) -> char {
    match c {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'A',
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => 'C',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'Ď' => 'D',
        'ď' => 'd',
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => 'E',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => 'G',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'Ĥ' => 'H',
        'ĥ' => 'h',
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => 'I',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'Ĵ' => 'J',
        'ĵ' => 'j',
        'Ķ' => 'K',
        'ķ' => 'k',
        'Ĺ' | 'Ļ' | 'Ľ' => 'L',
        'ĺ' | 'ļ' | 'ľ' => 'l',
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => 'N',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ō' | 'Ŏ' | 'Ő' => 'O',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'Ŕ' | 'Ŗ' | 'Ř' => 'R',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => 'S',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'Ţ' | 'Ť' => 'T',
        'ţ' | 'ť' => 't',
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => 'U',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'Ŵ' => 'W',
        'ŵ' => 'w',
        'Ý' | 'Ŷ' | 'Ÿ' => 'Y',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'Ź' | 'Ż' | 'Ž' => 'Z',
        'ź' | 'ż' | 'ž' => 'z',
        _ => c,
    }
}

/// Las marcas de acento sueltas (U+0300–U+036F), que NFD separa de la letra.
fn es_marca(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c)
}

/// Minúsculas y sin acentos, para buscar («Canción» → «cancion»). Como `_normal` de `vault.py`.
pub fn normal(texto: &str) -> String {
    texto
        .to_lowercase()
        .chars()
        .filter(|c| !es_marca(*c))
        .map(sin_marca)
        .collect()
}

/// Como `normal`, pero también dice de qué posición (en letras) del original salió cada letra.
pub fn normal_con_mapa(texto: &str) -> (Vec<char>, Vec<usize>) {
    let (mut letras, mut mapa) = (Vec::new(), Vec::new());
    for (i, c) in texto.chars().enumerate() {
        for l in c.to_lowercase() {
            if es_marca(l) {
                continue;
            }
            letras.push(sin_marca(l));
            mapa.push(i);
        }
    }
    (letras, mapa)
}

/// Sin acentos y en minúsculas (la `sin_acentos` de `puente.py`).
pub fn sin_acentos(texto: &str) -> String {
    normal(texto)
}

/// Bytes en texto corto: «4.7 GB», «820 MB».
pub fn tamano(n: f64) -> String {
    let mut n = n.max(0.0);
    for unidad in ["B", "KB", "MB", "GB", "TB"] {
        if n < 1024.0 || unidad == "TB" {
            return if ["B", "KB", "MB"].contains(&unidad) {
                format!("{} {unidad}", redondear0(n))
            } else {
                format!("{n:.1} {unidad}")
            };
        }
        n /= 1024.0;
    }
    format!("{n:.1} TB")
}

/// `f"{n:.0f}"` de Python (redondea a par en los empates).
fn redondear0(n: f64) -> String {
    let r = n.round();
    let r = if (n - n.trunc() - 0.5).abs() < f64::EPSILON && r % 2.0 != 0.0 {
        r - 1.0
    } else {
        r
    };
    format!("{r:.0}")
}

const MESES: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];
const DIAS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];

/// «14:05» si es de hoy, «Ayer», el día de la semana, o «3 nov» (con el año si no es el de ahora).
pub fn fecha_corta(epoch: f64) -> String {
    if epoch == 0.0 {
        return String::new();
    }
    let t = tiempo::local(epoch);
    let hoy = tiempo::local(tiempo::ahora());
    let dias = (tiempo::medianoche(&hoy) / 86400.0).floor() as i64
        - (tiempo::medianoche(&t) / 86400.0).floor() as i64;
    if dias <= 0 {
        return format!("{:02}:{:02}", t.hora, t.minuto);
    }
    if dias == 1 {
        return "Ayer".into();
    }
    if dias < 7 {
        return DIAS[t.dia_semana as usize].into();
    }
    let texto = format!("{} {}", t.dia, MESES[t.mes as usize - 1]);
    if t.anio == hoy.anio {
        texto
    } else {
        format!("{texto} {}", t.anio)
    }
}

/// Como `urllib.parse.quote`: todo menos letras, números, `_.-~` y `/` va en %XX.
pub fn quote(texto: &str) -> String {
    let mut s = String::new();
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~/".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

static ENLACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(!?)\[\[([^\]\n]+?)\]\]").unwrap());
static CODIGO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```|`[^`\n]*`").unwrap());

/// Los `[[enlaces]]` de un texto, en orden: lo que queda entre los corchetes (sin el `!` de incrustar).
pub fn enlaces_simples(texto: &str) -> Vec<String> {
    ENLACE.captures_iter(texto).map(|c| c[2].to_string()).collect()
}

/// Markdown listo para `Text.MarkdownText`: sin frontmatter y con `[[Nota|alias]]` → `[alias](nota:Nota)`.
pub fn markdown_con_enlaces(texto: &str) -> String {
    let mut texto = texto.to_string();
    if texto.starts_with("---\n")
        && let Some(fin) = texto[4..].find("\n---").map(|i| i + 4)
    {
        texto = texto[fin + 4..].trim_start_matches('\n').to_string();
    }
    let cambiar = |s: &str| -> String {
        ENLACE
            .replace_all(s, |c: &regex::Captures| {
                let incrustado = &c[1];
                let (destino, alias) = c[2].split_once('|').unwrap_or((&c[2], ""));
                let visible = if alias.trim().is_empty() {
                    destino.trim()
                } else {
                    alias.trim()
                };
                format!(
                    "{}[{visible}](nota:{})",
                    if incrustado.is_empty() { "" } else { "📎 " },
                    quote(destino.trim())
                )
            })
            .to_string()
    };
    // Fuera de los bloques de código: ahí [[x]] es texto.
    let mut salida = String::new();
    let mut desde = 0;
    for m in CODIGO.find_iter(&texto) {
        salida += &cambiar(&texto[desde..m.start()]);
        salida += m.as_str();
        desde = m.end();
    }
    salida += &cambiar(&texto[desde..]);
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn acentos() {
        assert_eq!(normal("Canción ÁÉÍÓÚ ñandú"), "cancion aeiou nandu");
        assert_eq!(normal("e\u{301}"), "e");
    }

    #[test]
    fn tamanos() {
        assert_eq!(tamano(4.7e9), "4.4 GB");
        assert_eq!(tamano(820.0 * 1024.0 * 1024.0), "820 MB");
        assert_eq!(tamano(0.0), "0 B");
    }

    #[test]
    fn markdown() {
        let t = "---\ntags: [x]\n---\nMira [[Ideas]] y [[Ideas#Juegos|mis juegos]] ![[foto.png]] `[[no]]`";
        let m = markdown_con_enlaces(t);
        assert_eq!(
            m,
            "Mira [Ideas](nota:Ideas) y [mis juegos](nota:Ideas%23Juegos) 📎 [foto.png](nota:foto.png) `[[no]]`"
        );
        assert_eq!(quote("Nota que falta"), "Nota%20que%20falta");
    }
}
