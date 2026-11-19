//! Lo de la ventana de Miku que no necesita Qt (y así se prueba con `cargo test`): qué modo toma (con avatar o
//! solo el panel de voz), la escala de la interfaz, los colores de la página web, su URL y cómo se le habla
//! (`Senal` → llamada de JavaScript). Lo dibuja `qt/` (QML con cxx-qt).

use crate::asistente::Senal;
use crate::rutas::Rutas;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use vortex_core::paleta::Paleta;

pub const RADIO: f64 = 18.0; // esquinas de las tarjetas
pub const ANCHO_PANEL: f64 = 420.0;
pub const ALTO_PANEL: f64 = 240.0;
pub const ANCHO_AVATAR: f64 = 460.0;
pub const ALTO_AVATAR: f64 = 760.0;
pub const APP_ID: &str = "vortex-miku"; // la clase de Wayland: lo que casan las reglas de Hyprland
pub const VERSION: &str = "1.3.0";

/// (¿con avatar?, nota si hubo que bajar a solo voz). Sin las bibliotecas 3D: solo voz.
pub fn elegir_modo(avatar_en_config: bool, vendor: &Path) -> (bool, String) {
    if !avatar_en_config {
        return (false, String::new());
    }
    if !vendor.join("three.module.min.js").is_file() {
        return (false, "Faltan las bibliotecas del 3D (three.js), así que sigo como asistente de voz. Reinstala el paquete vortex-miku.".into());
    }
    (true, String::new())
}

pub const SIN_WEBENGINE: &str =
    "No pude cargar QtWebEngine, así que sigo como asistente de voz. Instala qt6-webengine.";

/// Los ajustes del shell (solo lectura: general.uiScale).
pub fn ajustes_shell() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| vortex_core::rutas::home().join(".config"))
        .join("serpantinum/settings.json")
}

/// general.uiScale (1.0 si no está o no es razonable).
pub fn escala_ui(ruta: &Path) -> f64 {
    let v: Option<Value> = std::fs::read_to_string(ruta)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    match v.as_ref().and_then(|j| j["general"]["uiScale"].as_f64()) {
        Some(e) if (0.6..=1.6).contains(&e) => e,
        _ => 1.0,
    }
}

/// «#rrggbb» o «#aarrggbb» (Qt) -> «rgba(r, g, b, a)» para CSS.
pub fn a_css(color: &str) -> String {
    let h = color.trim_start_matches('#');
    let (alfa, h) = if h.len() == 8 {
        (
            u8::from_str_radix(&h[0..2], 16).unwrap_or(255) as f64 / 255.0,
            &h[2..],
        )
    } else {
        (1.0, h)
    };
    let canal = |i: usize| {
        h.get(i..i + 2)
            .and_then(|c| u8::from_str_radix(c, 16).ok())
            .unwrap_or(0)
    };
    format!("rgba({}, {}, {}, {:.2})", canal(0), canal(2), canal(4), alfa)
}

/// La paleta -> variables CSS de la página (--cristal, --texto, --acento...).
pub fn variables_css(p: &Paleta) -> Value {
    let g = |k: &str| p.get(k).cloned().unwrap_or_default();
    json!({
        "cristal": a_css(&g("flotante")), "campo": g("campo"), "borde": g("borde"), "borde-campo": g("bordeCampo"),
        "texto": g("texto"), "tenue": g("tenue"), "acento": g("azul"), "acento-hover": g("azulHover"),
        "sobre-acento": g("sobreAcento"), "relleno": g("relleno"), "rojo": g("peligro"),
        "radio": format!("{}px", RADIO),
    })
}

fn codificar(t: &str, dejar: &str) -> String {
    t.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) || dejar.as_bytes().contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `file:///ruta` con lo raro escapado.
pub fn url_archivo(p: &Path) -> String {
    format!("file://{}", codificar(&p.to_string_lossy(), "/"))
}

/// La página del avatar con lo que necesita en la dirección (modelo, bibliotecas, cuadros por segundo...).
pub fn url_pagina(rutas: &Rutas, modelo: Option<&Path>, fps: u32, extras: &[(String, String)]) -> String {
    let mut q: Vec<(String, String)> = vec![];
    if let Some(m) = modelo {
        q.push(("modelo".into(), url_archivo(m)));
    }
    q.push(("vendor".into(), format!("{}/", url_archivo(&rutas.vendor))));
    q.push(("fps".into(), fps.to_string()));
    q.extend(extras.iter().cloned());
    let q: Vec<String> = q
        .iter()
        .map(|(k, v)| format!("{k}={}", codificar(v, "")))
        .collect();
    format!("{}?{}", url_archivo(&rutas.web.join("index.html")), q.join("&"))
}

/// Ajustes de la página para pruebas (`VORTEX_MIKU_WEB_ENCUADRE`, `_RETRASO`, `_PRUEBA`).
pub fn extras_de_prueba() -> Vec<(String, String)> {
    ["encuadre", "retraso", "prueba"]
        .iter()
        .filter_map(|c| {
            std::env::var(format!("VORTEX_MIKU_WEB_{}", c.to_uppercase()))
                .ok()
                .filter(|v| !v.is_empty())
                .map(|v| (c.to_string(), v))
        })
        .collect()
}

/// La llamada de JavaScript de la página para una señal (`miku.estado(...)`), o `None` si la página no la usa.
pub fn js_de(s: &Senal) -> Option<String> {
    let r2 = |v: f64| (v * 100.0).round_ties_even() / 100.0;
    let (f, args): (&str, Vec<Value>) = match s {
        Senal::Estado(e, t) => ("estado", vec![json!(e), json!(t)]),
        Senal::Usuario(t) => ("usuario", vec![json!(t)]),
        Senal::Respuesta(t, fin) => ("respuesta", vec![json!(t), json!(fin)]),
        Senal::Aviso(t) => ("aviso", vec![json!(t)]),
        Senal::Nivel(v) => ("nivel", vec![json!(r2(*v))]),
        Senal::Boca(pista, hz) => ("visemas", vec![json!(pista), json!(hz)]),
        Senal::BocaFin => ("pararBoca", vec![]),
        Senal::Emocion(n, p) => ("emocion", vec![json!(n), json!(p)]),
        Senal::Sonrojo(v) => ("sonrojo", vec![json!(r2(*v))]),
        Senal::Afecto(v, n) => ("afecto", vec![json!(v), json!(n)]),
        Senal::Boda(e) => ("boda", vec![json!(e)]),
        Senal::Progreso(t, d, f) => (
            "progreso",
            vec![json!(t), json!(d), json!((f * 1000.0).round_ties_even() / 1000.0)],
        ),
        Senal::Acciones(l) => (
            "acciones",
            vec![json!(l.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>())],
        ),
        _ => return None,
    };
    Some(llamada(f, &args))
}

pub fn llamada(funcion: &str, args: &[Value]) -> String {
    format!(
        "window.miku && miku.{funcion}({})",
        args.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn colores_de_css_y_variables() {
        assert_eq!(a_css("#e60b0d1a"), "rgba(11, 13, 26, 0.90)");
        assert_eq!(a_css("#9ec5ff"), "rgba(158, 197, 255, 1.00)");
        let v = variables_css(&vortex_core::paleta::respaldo());
        assert_eq!(v["cristal"], "rgba(11, 13, 26, 0.90)");
        assert_eq!(
            (v["acento"].as_str(), v["rojo"].as_str(), v["radio"].as_str()),
            (Some("#9ec5ff"), Some("#ff9db0"), Some("18px"))
        );
    }

    #[test]
    fn la_escala_solo_vale_si_es_razonable() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("s.json");
        assert_eq!(escala_ui(&r), 1.0);
        for (t, e) in [
            (r#"{"general":{"uiScale":0.85}}"#, 0.85),
            (r#"{"general":{"uiScale":3}}"#, 1.0),
            (r#"{"general":{"uiScale":"x"}}"#, 1.0),
            ("no json", 1.0),
        ] {
            std::fs::write(&r, t).unwrap();
            assert_eq!(escala_ui(&r), e, "{t}");
        }
    }

    #[test]
    fn el_modo_baja_a_solo_voz_sin_las_bibliotecas_3d() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(elegir_modo(false, d.path()), (false, String::new()));
        let (a, nota) = elegir_modo(true, d.path());
        assert!(!a && nota.contains("three.js"));
        std::fs::write(d.path().join("three.module.min.js"), "").unwrap();
        assert_eq!(elegir_modo(true, d.path()), (true, String::new()));
    }

    #[test]
    fn la_url_de_la_pagina_escapa_y_lleva_lo_que_necesita() {
        let r = Rutas::en(Path::new("/tmp/x y"));
        let u = url_pagina(
            &r,
            Some(Path::new("/m/Mi modelo.vrm")),
            30,
            &[("prueba".into(), "1".into())],
        );
        assert!(
            u.starts_with(
                "file:///tmp/x%20y/raiz/web/index.html?modelo=file%3A%2F%2F%2Fm%2FMi%2520modelo.vrm&vendor="
            ),
            "{u}"
        );
        assert!(u.ends_with("&fps=30&prueba=1"), "{u}");
        assert!(!url_pagina(&r, None, 30, &[]).contains("modelo="));
    }

    #[test]
    fn las_senales_se_vuelven_llamadas_de_javascript() {
        assert_eq!(
            js_de(&Senal::Estado("reposo".into(), "Lista".into())).unwrap(),
            r#"window.miku && miku.estado("reposo", "Lista")"#
        );
        assert_eq!(
            js_de(&Senal::Nivel(0.456)).unwrap(),
            "window.miku && miku.nivel(0.46)"
        );
        assert_eq!(js_de(&Senal::BocaFin).unwrap(), "window.miku && miku.pararBoca()");
        assert_eq!(
            js_de(&Senal::Acciones(vec![("a".into(), "Activar".into())])).unwrap(),
            r#"window.miku && miku.acciones([["a","Activar"]])"#
        );
        assert_eq!(
            js_de(&Senal::Progreso("t".into(), "d".into(), 0.12345)).unwrap(),
            r#"window.miku && miku.progreso("t", "d", 0.123)"#
        );
        assert!(js_de(&Senal::Cerrar).is_none() && js_de(&Senal::Vista(String::new(), 0, 0)).is_none());
    }
}
