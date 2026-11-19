//! Del texto de Miku al que lee el modelo: sin markdown ni emojis, normalizado como lo espera Supertonic y
//! partido en trozos que el modelo aguanta (igual que `texto_hablado`, `preparar_texto` y `trozos` de
//! `motor_voz.py`).

use regex::Regex;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub const IDIOMA: &str = "es";
/// Caracteres por pasada del modelo; una frase más larga se parte.
pub const MAX_TROZO: usize = 300;

fn re(c: &'static OnceLock<Regex>, patron: &str) -> &'static Regex {
    c.get_or_init(|| Regex::new(patron).unwrap())
}

/// La puntuación que se queda (las categorías P de Unicode que salen en un texto en español). Los símbolos
/// (`$ + < = > ^ ~ |` y los emojis) se quitan, como en Python.
fn es_puntuacion(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_punctuation() && !"$+<=>^`|~".contains(c);
    }
    matches!(c, '¡' | '¿' | '«' | '»' | '·' | '§' | '¶')
        || ('\u{2010}'..='\u{2027}').contains(&c)
        || ('\u{2030}'..='\u{2043}').contains(&c)
        || ('\u{2045}'..='\u{2051}').contains(&c)
        || ('\u{3001}'..='\u{3003}').contains(&c)
        || ('\u{3008}'..='\u{3011}').contains(&c)
}

/// El texto listo para decirlo: sin bloques de código, enlaces, markdown ni emojis.
pub fn texto_hablado(texto: &str) -> String {
    static BLOQUE: OnceLock<Regex> = OnceLock::new();
    static CODIGO: OnceLock<Regex> = OnceLock::new();
    static ENLACE: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    static MARCAS: OnceLock<Regex> = OnceLock::new();
    static VINETA: OnceLock<Regex> = OnceLock::new();
    static ESPACIOS: OnceLock<Regex> = OnceLock::new();
    let t = re(&BLOQUE, r"(?s)```.*?```").replace_all(texto, " ");
    let t = re(&CODIGO, r"`([^`]*)`").replace_all(&t, "$1");
    let t = re(&ENLACE, r"\[([^\]]*)\]\([^)]*\)").replace_all(&t, "$1");
    let t = re(&URL, r"https?://\S+").replace_all(&t, "un enlace");
    let t = re(&MARCAS, r"[*_#>~|]+").replace_all(&t, " ");
    let t = re(&VINETA, r"(?m)^\s*[-•]\s+").replace_all(&t, "");
    let t: String = t
        .chars()
        .filter(|&c| c.is_alphabetic() || c.is_numeric() || c.is_whitespace() || es_puntuacion(c))
        .collect();
    let t = t.replace('…', "...");
    re(&ESPACIOS, r"\s+").replace_all(&t, " ").trim().to_string()
}

const SIMBOLOS: &[(char, &str)] = &[
    ('–', "-"),
    ('‑', "-"),
    ('—', "-"),
    ('¯', " "),
    ('_', " "),
    ('“', "\""),
    ('”', "\""),
    ('‘', "'"),
    ('’', "'"),
    ('´', "'"),
    ('`', "'"),
    ('[', " "),
    (']', " "),
    ('|', " "),
    ('/', " "),
    ('#', " "),
    ('→', " "),
    ('←', " "),
];

/// ¿Termina en un signo que cierra la frase? Si no, se le pone punto.
fn tiene_final(t: &str) -> bool {
    t.chars()
        .last()
        .is_some_and(|c| ".!?;:,'\")]}…。」』】〉》›»".contains(c))
}

/// La normalización que espera el modelo (las mismas reglas que el SDK de Supertone), envuelta en `<es>…</es>`.
pub fn preparar_texto(texto: &str) -> String {
    static ESPACIOS: OnceLock<Regex> = OnceLock::new();
    let mut t: String = texto.nfkd().collect();
    for (viejo, nuevo) in SIMBOLOS {
        if t.contains(*viejo) {
            t = t.replace(*viejo, nuevo);
        }
    }
    t.retain(|c| !matches!(c, '♥' | '☆' | '♡' | '©' | '\\'));
    for signo in [",", ".", "!", "?", ";", ":", "'"] {
        t = t.replace(&format!(" {signo}"), signo);
    }
    // Comillas repetidas ("" o '' o ``) se quedan en una.
    let mut sin_repetir = String::with_capacity(t.len());
    let mut anterior = None;
    for c in t.chars() {
        if matches!(c, '"' | '\'' | '`') && anterior == Some(c) {
            continue;
        }
        sin_repetir.push(c);
        anterior = Some(c);
    }
    let mut t = re(&ESPACIOS, r"\s+")
        .replace_all(&sin_repetir, " ")
        .trim()
        .to_string();
    if !tiene_final(&t) {
        t.push('.');
    }
    format!("<{IDIOMA}>{t}</{IDIOMA}>")
}

/// Las frases del texto: corta en los espacios que siguen a `. ! ? …`.
fn frases(texto: &str) -> Vec<String> {
    let mut v = vec![];
    let mut actual = String::new();
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() && actual.ends_with(['.', '!', '?', '…']) {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            v.push(std::mem::take(&mut actual));
            continue;
        }
        actual.push(c);
    }
    v.push(actual);
    v
}

/// Parte un texto largo en trozos de hasta `maximo` caracteres, cortando en el final de una frase (una frase
/// interminable, en una coma o un espacio).
pub fn trozos(texto: &str, maximo: usize) -> Vec<String> {
    let mut salida: Vec<String> = vec![];
    let mut actual = String::new();
    for frase in frases(texto) {
        let mut frase: Vec<char> = frase.chars().collect();
        while frase.len() > maximo {
            let antes = &frase[..maximo];
            let corte = antes
                .iter()
                .rposition(|&c| c == ',' || c == ' ')
                .filter(|&p| p > 0)
                .unwrap_or(maximo);
            let fin = (corte + 1).min(frase.len());
            salida.push(frase[..fin].iter().collect::<String>().trim().to_string());
            frase = frase[fin..].iter().collect::<String>().trim().chars().collect();
        }
        let frase: String = frase.into_iter().collect();
        if !actual.is_empty() && actual.chars().count() + 1 + frase.chars().count() > maximo {
            salida.push(std::mem::take(&mut actual));
        }
        actual = format!("{actual} {frase}").trim().to_string();
    }
    if !actual.is_empty() {
        salida.push(actual);
    }
    salida.retain(|s| !s.is_empty());
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn quita_markdown_y_emojis() {
        assert_eq!(
            texto_hablado("**Hola** 😊 mira [esto](https://x.com) y `ls`… ```\ncódigo\n```fin $5"),
            "Hola mira esto y ls... fin 5"
        );
        assert_eq!(texto_hablado("ve a https://vortex.mx ya"), "ve a un enlace ya");
        assert_eq!(texto_hablado("- uno\n- dos"), "uno dos");
        assert_eq!(texto_hablado("¿Qué tal? ¡Bien!"), "¿Qué tal? ¡Bien!");
    }

    #[test]
    fn prepara_como_el_sdk() {
        assert_eq!(
            preparar_texto("Hola , ¿qué tal"),
            "<es>Hola, ¿que\u{301} tal.</es>"
        );
        assert_eq!(preparar_texto("—Sí ''ya''"), "<es>-Si\u{301}'ya'</es>");
        assert_eq!(preparar_texto("ok!"), "<es>ok!</es>");
    }

    #[test]
    fn parte_en_trozos() {
        assert_eq!(trozos("Uno. Dos. Tres.", 300), vec!["Uno. Dos. Tres."]);
        assert_eq!(trozos("Uno. Dos. Tres.", 9), vec!["Uno. Dos.", "Tres."]);
        let largo = "palabra ".repeat(10);
        let t = trozos(largo.trim(), 20);
        assert!(t.iter().all(|x| x.chars().count() <= 20), "{t:?}");
        assert_eq!(t.join(" "), largo.trim());
        assert!(trozos("", 300).is_empty());
    }
}
