//! vortex-enlace en Rust: «bajar con copiar el enlace». LA ESCRIBE MIKE.
//!
//! Es la misma app que `bin/vortex-enlace` (Python): cuando copias el enlace de un video de YouTube,
//! TikTok, Instagram o X, sale un aviso con «Bajar video» y «Solo audio», y lo baja con yt-dlp.
//!
//!   vortex-enlace-rs                     escucha el portapapeles; si ya hay otro escuchando, sale
//!   vortex-enlace-rs revisar             (lo llama wl-paste en cada copia) guarda en cliphist y avisa
//!   vortex-enlace-rs ofrecer SITIO URL   el aviso con los botones (va aparte para no trabar a wl-paste)
//!   vortex-enlace-rs bajar video|audio URL
//!
//! Cada función de abajo es una del Python, con el mismo nombre. Abre `bin/vortex-enlace` al lado y ve
//! pasándolas una por una. Lee primero rust/apps/vortex-enlace/GUIA.md.

// Mientras haya huecos sin llenar, Rust se queja de cosas sin usar. Cuando termines, BORRA esta línea.
#![allow(unused)]

use std::path::PathBuf;
use std::process::Command;

const APP: &str = "Vortexine's OS";
const ICONO: &str = "folder-download";
/// El mismo enlace otra vez antes de 10 min no avisa.
const REPETIDO: f64 = 600.0;

/// La carpeta `~/.local/state/vortex` (o `$XDG_STATE_HOME/vortex`).
/// (Este ya está hecho, para que veas cómo se lee una variable de entorno.)
fn estado() -> PathBuf {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(d) => PathBuf::from(d),
        None => home().join(".local/state"),
    };
    base.join("vortex")
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// PASO 1. El sitio («YouTube», «TikTok», «Instagram» o «X») si lo copiado es UN solo enlace a un
/// video; si no, `None`. Python: `sitio()` y la lista `SITIOS` (copia las 4 expresiones regulares tal
/// cual; en Rust van con `regex::Regex::new`, y el `re.I` del Python se escribe `(?i)` al principio).
fn sitio(texto: &str) -> Option<&'static str> {
    todo!("paso 1")
}

/// PASO 2. ¿Está prendido el modo juego? (existe el archivo `estado()/modo-juego`). Una línea.
fn jugando() -> bool {
    todo!("paso 2")
}

/// PASO 3. Manda un aviso con notify-send y devuelve (id del aviso, botón elegido).
/// Python: `aviso()`. Arma el comando con `Command::new("notify-send")` y `.arg(...)`, córrelo con
/// `.output()` y parte la salida (`String::from_utf8_lossy(&salida.stdout)`) en palabras.
fn aviso(
    titulo: &str,
    cuerpo: &str,
    acciones: &[(&str, &str)],
    reemplaza: &str,
    progreso: Option<u32>,
    espera: u32,
) -> (String, String) {
    todo!("paso 3")
}

/// PASO 4. La carpeta de Videos o de Música (con `xdg-user-dir VIDEOS` o `MUSIC`).
fn carpeta(tipo: &str) -> PathBuf {
    todo!("paso 4")
}

/// PASO 5. Lo que hace `revisar()` en Python: lee TODO lo copiado de la entrada estándar
/// (`std::io::Read::read_to_end`), se lo pasa a `cliphist store`, y si es un enlace de video (y no
/// estás jugando, y no es el mismo de hace menos de 10 min: `enlace-ultimo.json`), lanza
/// `vortex-enlace-rs ofrecer SITIO URL` aparte (`std::env::current_exe()` te da tu propia ruta).
fn revisar() {
    todo!("paso 5")
}

/// PASO 6. El aviso «¿Bajar este video de …?» y, si eliges, `bajar()`. Python: `ofrecer()`.
fn ofrecer(nombre: &str, url: &str) {
    todo!("paso 6")
}

/// PASO 7. El mensaje de error en español. Python: `por_que()`. Es puro `if` con `contains`.
fn por_que(error: &str) -> &'static str {
    todo!("paso 7")
}

/// PASO 8 (el más largo). Baja con yt-dlp y va avisando el avance. Python: `bajar()`.
/// Pista: `Command::new("yt-dlp")...stdout(Stdio::piped()).spawn()` y lee las líneas con
/// `std::io::BufReader::new(hijo.stdout.take().unwrap()).lines()`.
fn bajar(tipo: &str, url: &str) {
    todo!("paso 8")
}

/// PASO 9. Un solo escucha por sesión. Python: `escuchar()`. Toma el candado con
/// `std::fs::File::try_lock()`; si ya lo tiene otro, sale. Luego corre
/// `wl-paste --type text --watch <tu-ruta> revisar` y ESPERA a que termine (`.status()`): así el candado
/// dura lo que dura el escucha (el Python hace `exec`, pero así es más fácil y gasta casi lo mismo).
fn escuchar() {
    todo!("paso 9")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => escuchar(),
        ["revisar"] => revisar(),
        ["ofrecer", nombre, url] => ofrecer(nombre, url),
        ["bajar", tipo @ ("video" | "audio"), url] => bajar(tipo, url),
        _ => {
            eprintln!("uso: vortex-enlace-rs [revisar | ofrecer SITIO URL | bajar video|audio URL]");
            std::process::exit(2);
        }
    }
}

// Pruebas chiquitas para ir viendo si vas bien: `cargo test -p vortex-enlace` (ve la GUIA).
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn paso_1_sitio() {
        assert_eq!(
            sitio("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("YouTube")
        );
        assert_eq!(sitio("https://youtu.be/dQw4w9WgXcQ"), Some("YouTube"));
        assert_eq!(
            sitio("https://www.tiktok.com/@alguien/video/1234567890"),
            Some("TikTok")
        );
        assert_eq!(sitio("https://www.instagram.com/reel/AbC123/"), Some("Instagram"));
        assert_eq!(sitio("https://x.com/alguien/status/123"), Some("X"));
        assert_eq!(
            sitio("  https://youtu.be/dQw4w9WgXcQ  "),
            Some("YouTube"),
            "con espacios alrededor sí"
        );
        assert_eq!(
            sitio("mira esto https://youtu.be/dQw4w9WgXcQ"),
            None,
            "un texto que trae un enlace, no"
        );
        assert_eq!(sitio("https://www.google.com"), None);
        assert_eq!(sitio(""), None);
    }

    #[test]
    fn paso_7_por_que() {
        assert_eq!(por_que("ERROR: Private video"), "El video es privado.");
        assert_eq!(
            por_que("ERROR: Sign in to confirm"),
            "Ese sitio pide iniciar sesión para verlo."
        );
        assert_eq!(
            por_que("ERROR: Video unavailable"),
            "El video ya no está disponible."
        );
        assert_eq!(
            por_que("ERROR: timed out"),
            "Revisa tu internet e inténtalo otra vez."
        );
        assert_eq!(
            por_que("algo raro"),
            "No se pudo bajar. Prueba de nuevo más tarde."
        );
    }
}
