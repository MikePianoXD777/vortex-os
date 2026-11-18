//! JSON escrito igual que `json.dumps(..., ensure_ascii=False)` de Python (separadores `, ` y `: `):
//! el panel (QML) y el registro de avisos lo leen tal cual lo escribía la versión de Python.

/// Una cadena entre comillas, con los mismos escapes que Python (solo los de control, `"` y `\`).
pub fn cadena(s: &str, salida: &mut String) {
    salida.push('"');
    for c in s.chars() {
        match c {
            '"' => salida.push_str("\\\""),
            '\\' => salida.push_str("\\\\"),
            '\n' => salida.push_str("\\n"),
            '\r' => salida.push_str("\\r"),
            '\t' => salida.push_str("\\t"),
            '\u{8}' => salida.push_str("\\b"),
            '\u{c}' => salida.push_str("\\f"),
            c if (c as u32) < 0x20 => salida.push_str(&format!("\\u{:04x}", c as u32)),
            c => salida.push(c),
        }
    }
    salida.push('"');
}

#[cfg(test)]
mod pruebas {
    use super::cadena;

    #[test]
    fn escapes_como_python() {
        let mut s = String::new();
        cadena("a\"b\\c\n\t\u{1}é📅\u{7f}", &mut s);
        assert_eq!(s, "\"a\\\"b\\\\c\\n\\t\\u0001é📅\u{7f}\"");
    }
}
