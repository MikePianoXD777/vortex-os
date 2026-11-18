//! Identificadores de aplicación de Flathub (org.mozilla.firefox, com.valvesoftware.Steam…).
//!
//! Todo id que viene de fuera (de la interfaz, de una búsqueda, de un archivo) pasa por aquí antes de
//! usarse en una URL o en un comando: nada de espacios, comillas, «;», «$», barras ni nada que un shell
//! o una URL puedan interpretar. (De todos modos nunca se usa un shell.)

const MAXIMO: usize = 255;

fn es_segmento(s: &str) -> bool {
    let b = s.as_bytes();
    // Empieza por letra, número o «_» (no por «-»: un programa lo tomaría por una opción).
    match b.first() {
        Some(c) if c.is_ascii_alphanumeric() || *c == b'_' => {}
        _ => return false,
    }
    b.iter()
        .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

/// True si `app_id` tiene la forma de un id de Flathub: segmentos de letras, números, «_» y «-»
/// separados por puntos (al menos dos). Es más estricto que el de Flatpak a propósito.
pub fn es_id_valido(app_id: &str) -> bool {
    app_id.len() <= MAXIMO && app_id.split('.').count() >= 2 && app_id.split('.').all(es_segmento)
}

/// Devuelve `app_id` si es válido; si no, un error en español.
pub fn validar_id(app_id: &str) -> Result<&str, String> {
    if es_id_valido(app_id) {
        Ok(app_id)
    } else {
        let mostrado: String = format!("{app_id:?}").chars().take(60).collect();
        Err(format!(
            "«{mostrado}» no es un identificador de aplicación válido."
        ))
    }
}
