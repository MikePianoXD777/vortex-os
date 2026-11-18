//! Textos de la app en español e inglés, en palabras sencillas (sin tecnicismos para la persona).
//!
//! El idioma sale de LC_ALL / LC_MESSAGES / LANG («es_MX.UTF-8» → español; cualquier otro → inglés).
//! `VORTEX_APP_INSTALLER_IDIOMA=es|en` lo fuerza (pruebas y capturas).
//!
//! ```
//! use vortex_app_installer::textos::{Idioma, tr};
//! assert_eq!(tr(Idioma::Es, "hecho_instalado", &[("nombre", "Firefox")]), "«Firefox» ya está instalado");
//! ```
//! `{n|cosa|cosas}` elige singular o plural según n.

use crate::textos_tabla::TEXTOS;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Idioma {
    Es,
    En,
}

impl Idioma {
    /// «es» o «en» (lo que se manda a Flathub como `locale`).
    pub fn codigo(self) -> &'static str {
        match self {
            Idioma::Es => "es",
            Idioma::En => "en",
        }
    }

    /// Del entorno: `VORTEX_APP_INSTALLER_IDIOMA`, y si no, el primer LC_ALL / LC_MESSAGES / LANG / LANGUAGE
    /// que no esté vacío.
    pub fn del_entorno() -> Idioma {
        let var = |n: &str| std::env::var(n).unwrap_or_default();
        Self::de_variables(
            &var("VORTEX_APP_INSTALLER_IDIOMA"),
            &[var("LC_ALL"), var("LC_MESSAGES"), var("LANG"), var("LANGUAGE")],
        )
    }

    /// La parte pura de [`Idioma::del_entorno`] (así se prueba sin tocar variables globales).
    pub fn de_variables(forzado: &str, locales: &[String]) -> Idioma {
        match forzado.trim().to_lowercase().as_str() {
            "es" => return Idioma::Es,
            "en" => return Idioma::En,
            _ => {}
        }
        for v in locales {
            if !v.is_empty() {
                return if v.to_lowercase().starts_with("es") {
                    Idioma::Es
                } else {
                    Idioma::En
                };
            }
        }
        Idioma::Es
    }
}

fn plural() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{(\w+)\|([^|{}]*)\|([^|{}]*)\}").expect("regex de plurales"))
}

/// El texto de `clave` en ese idioma, con los `{valor}` reemplazados y los plurales elegidos. Una clave que
/// no existe se devuelve tal cual (como la de Python).
pub fn tr(idioma: Idioma, clave: &str, valores: &[(&str, &str)]) -> String {
    let Some((_, es, en)) = TEXTOS.iter().find(|(k, _, _)| *k == clave) else {
        return clave.to_string();
    };
    let texto = if idioma == Idioma::Es { es } else { en };
    let texto = plural().replace_all(texto, |c: &regex::Captures| {
        let uno = valores.iter().any(|(k, v)| *k == &c[1] && *v == "1");
        c[if uno { 2 } else { 3 }].to_string()
    });
    let mut texto = texto.into_owned();
    for (k, v) in valores {
        texto = texto.replace(&format!("{{{k}}}"), v);
    }
    texto
}

/// Todos los textos del idioma, para la interfaz (QML los usa como `t.clave`).
pub fn todos(idioma: Idioma) -> Vec<(&'static str, &'static str)> {
    TEXTOS
        .iter()
        .map(|(k, es, en)| (*k, if idioma == Idioma::Es { *es } else { *en }))
        .collect()
}

/// Todas las claves, con sus dos textos (para las pruebas de que no falta ninguna).
pub fn tabla() -> &'static [(&'static str, &'static str, &'static str)] {
    TEXTOS
}
