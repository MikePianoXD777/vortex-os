//! vortex-enlace en Rust: «bajar con copiar el enlace». LA ESCRIBE MIKE.
//!
//! Es la misma app que `bin/vortex-enlace` (Python): cuando copias el enlace de un video de YouTube,
//! TikTok, Instagram o X, sale un aviso con «Bajar video» y «Solo audio», y lo baja con yt-dlp.
//!
//!   vortex-enlace-rs                     escucha el portapapeles; si ya hay otro escuchando, sale
//!   vortex-enlace-rs revisar             (lo llama wl-paste en cada copia) guarda en cliphist y avisa
//!   vortex-enlace-rs ofrecer SITIO URL   el aviso con los botones (va aparte para no trabar a wl-paste)
//!   vortex-enlace-rs bajar video|audio URL

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;

const APP: &str = "Vortexine's OS";
const ICONO: &str = "folder-download";
/// El mismo enlace otra vez antes de 10 min no avisa.
const REPETIDO: f64 = 600.0;

static RE_YOUTUBE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^https?://(?:www\.|m\.|music\.)?(?:youtube\.com/(?:watch\?.*v=|shorts/|live/)|youtu\.be/)[\w-]{6,}").expect("expresión regular de YouTube")
});
static RE_TIKTOK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^https?://(?:(?:www|m)\.tiktok\.com/@[^/]+/(?:video|photo)/\d+|(?:vm|vt)\.tiktok\.com/\w+)",
    )
    .expect("expresión regular de TikTok")
});
static RE_INSTAGRAM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^https?://(?:www\.)?instagram\.com/(?:[\w.]+/)?(?:reels?|p|tv)/[\w-]+")
        .expect("expresión regular de Instagram")
});
static RE_X: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^https?://(?:www\.|mobile\.)?(?:x|twitter)\.com/\w+/status/\d+")
        .expect("expresión regular de X")
});
static RE_VORTEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^VORTEX\s+([\d.]+)%").expect("expresión regular de avance"));

/// La carpeta `~/.local/state/vortex` (o `$XDG_STATE_HOME/vortex`).
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
/// video; si no, `None`. Python: `sitio()` y la lista `SITIOS`.
fn sitio(texto: &str) -> Option<&'static str> {
    let texto = texto.trim();
    if texto.is_empty() || texto.len() > 500 || texto.chars().any(char::is_whitespace) {
        return None;
    }
    if RE_YOUTUBE.is_match(texto) {
        Some("YouTube")
    } else if RE_TIKTOK.is_match(texto) {
        Some("TikTok")
    } else if RE_INSTAGRAM.is_match(texto) {
        Some("Instagram")
    } else if RE_X.is_match(texto) {
        Some("X")
    } else {
        None
    }
}

/// PASO 2. ¿Está prendido el modo juego? (existe el archivo `estado()/modo-juego`).
fn jugando() -> bool {
    estado().join("modo-juego").exists()
}

/// PASO 3. Manda un aviso con notify-send y devuelve (id del aviso, botón elegido).
fn aviso(
    titulo: &str,
    cuerpo: &str,
    acciones: &[(&str, &str)],
    reemplaza: &str,
    progreso: Option<u32>,
    espera: u32,
) -> (String, String) {
    let mut cmd = Command::new("notify-send");
    cmd.args(["-a", APP, "-i", ICONO, "-p"]);
    if !reemplaza.is_empty() {
        cmd.args(["-r", reemplaza]);
    }
    if let Some(p) = progreso {
        cmd.args(["-h", &format!("int:value:{p}")]);
    }
    if espera > 0 {
        cmd.args(["-t", &espera.to_string()]);
    }
    for (clave, texto) in acciones {
        cmd.args(["-A", &format!("{clave}={texto}")]);
    }
    cmd.arg(titulo);
    cmd.arg(cuerpo);

    let salida = match cmd.output() {
        Ok(s) => s,
        Err(_) => return (String::new(), String::new()),
    };
    let texto = String::from_utf8_lossy(&salida.stdout);
    let mut palabras = texto.split_whitespace();
    let id = palabras.next().unwrap_or("").to_string();
    let boton = palabras.next().unwrap_or("").to_string();
    (id, boton)
}

/// PASO 4. La carpeta de Videos o de Música (con `xdg-user-dir VIDEOS` o `MUSIC`).
fn carpeta(tipo: &str) -> PathBuf {
    let (clave, nombre) = if tipo == "audio" {
        ("MUSIC", "Música")
    } else {
        ("VIDEOS", "Videos")
    };
    let d = Command::new("xdg-user-dir")
        .arg(clave)
        .output()
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
        .unwrap_or_default();
    let h = home();
    let h_str = h.to_string_lossy();
    if !d.is_empty() && d != h_str {
        PathBuf::from(d)
    } else {
        h.join(nombre)
    }
}

/// PASO 5. Lo que hace `revisar()` en Python: lee TODO lo copiado de la entrada estándar,
/// se lo pasa a `cliphist store`, y si es un enlace de video (y no estás jugando, y no es el mismo
/// de hace menos de 10 min), lanza `vortex-enlace-rs ofrecer SITIO URL` aparte.
fn revisar() {
    let mut datos = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut datos);

    // Guarda el texto en el historial del portapapeles.
    if let Ok(mut hijo) = Command::new("cliphist")
        .arg("store")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(mut entrada) = hijo.stdin.take() {
            let _ = entrada.write_all(&datos);
        }
        let _ = hijo.wait();
    }

    let limite = datos.len().min(4096);
    let texto = String::from_utf8_lossy(&datos[..limite]);
    let Some(nombre) = sitio(&texto) else {
        return;
    };
    if jugando() {
        return;
    }

    let url = texto.trim();
    let est = estado();
    let archivo = est.join("enlace-ultimo.json");
    let ahora = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(_) => 0.0,
    };

    if let Ok(contenido) = std::fs::read_to_string(&archivo)
        && let Ok(ultimo) = serde_json::from_str::<serde_json::Value>(&contenido)
    {
        let ultimo_url = ultimo.get("url").and_then(|v| v.as_str());
        let cuando = ultimo.get("cuando").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if ultimo_url == Some(url) && (ahora - cuando) < REPETIDO {
            return;
        }
    }

    let _ = std::fs::create_dir_all(&est);
    let nuevo = serde_json::json!({
        "url": url,
        "cuando": ahora,
    });
    if let Ok(json_str) = serde_json::to_string(&nuevo) {
        let _ = std::fs::write(&archivo, json_str);
    }

    // wl-paste espera a que terminemos: el aviso (que espera tu clic) va aparte
    let mi_ruta = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from(std::env::args().next().unwrap_or_default()));
    let _ = Command::new(mi_ruta)
        .args(["ofrecer", nombre, url])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
}

/// PASO 6. El aviso «¿Bajar este video de …?» y, si eliges, `bajar()`. Python: `ofrecer()`.
fn ofrecer(nombre: &str, url: &str) {
    let titulo = format!("¿Bajar este video de {nombre}?");
    let (_, eleccion) = aviso(
        &titulo,
        "Copiaste su enlace.",
        &[("video", "Bajar video"), ("audio", "Solo audio")],
        "",
        None,
        15000,
    );
    if eleccion == "video" || eleccion == "audio" {
        bajar(&eleccion, url);
    }
}

/// PASO 7. El mensaje de error en español. Python: `por_que()`.
fn por_que(error: &str) -> &'static str {
    let e = error.to_lowercase();
    if e.contains("private") || e.contains("privado") {
        "El video es privado."
    } else if e.contains("login") || e.contains("sign in") || e.contains("cookies") {
        "Ese sitio pide iniciar sesión para verlo."
    } else if e.contains("unavailable")
        || e.contains("removed")
        || e.contains("not available")
        || e.contains("404")
        || e.contains("not found")
    {
        "El video ya no está disponible."
    } else if e.contains("unable to download")
        || e.contains("name resolution")
        || e.contains("timed out")
        || e.contains("network")
    {
        "Revisa tu internet e inténtalo otra vez."
    } else {
        "No se pudo bajar. Prueba de nuevo más tarde."
    }
}

/// Comprueba si un comando ejecutable existe en alguna carpeta del PATH.
fn tiene_comando(cmd: &str) -> bool {
    if let Some(path) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&path) {
            let ruta = p.join(cmd);
            if ruta.is_file() {
                return true;
            }
        }
    }
    false
}

/// PASO 8. Baja con yt-dlp y va avisando el avance. Python: `bajar()`.
fn bajar(tipo: &str, url: &str) {
    if !tiene_comando("yt-dlp") {
        aviso(
            "No se pudo bajar",
            "Falta yt-dlp: corre «vortex actualizar».",
            &[],
            "",
            None,
            0,
        );
        return;
    }

    let destino = carpeta(tipo);
    let _ = std::fs::create_dir_all(&destino);

    let temp_nombre = format!(
        "vortex-enlace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let final_path = std::env::temp_dir().join(temp_nombre);
    // (Claude) create_new: si ya existe algo con ese nombre (otro usuario de la PC pudo dejar ahí un enlace
    // a otro archivo), no lo abre; y 0o600, que nadie más lo lea. Es lo que hace mkstemp en el Python.
    if OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&final_path)
        .is_err()
    {
        aviso(
            &format!(
                "No se pudo bajar {}",
                if tipo == "audio" {
                    "la canción"
                } else {
                    "el video"
                }
            ),
            "No pude crear un archivo temporal.",
            &[],
            "",
            None,
            0,
        );
        return;
    }

    let final_str = final_path.to_string_lossy();
    let destino_str = destino.to_string_lossy();

    let mut cmd = Command::new("yt-dlp");
    cmd.args([
        "--no-playlist",
        "--newline",
        "--no-colors",
        "--trim-filenames",
        "120",
        "--progress-template",
        "download:VORTEX %(progress._percent_str)s",
        "--print-to-file",
        "after_move:filepath",
        &final_str,
        "-P",
        &destino_str,
    ]);

    if tipo == "audio" {
        cmd.args([
            "-x",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "0",
            "--embed-thumbnail",
            "--embed-metadata",
            "--convert-thumbnails",
            "jpg",
            "-o",
            "%(artist,creator,uploader&{} - |)s%(track,title)s.%(ext)s",
        ]);
    } else {
        cmd.args([
            "-f",
            "bv*[height<=1080][ext=mp4]+ba[ext=m4a]/b[height<=1080][ext=mp4]/bv*[height<=1080]+ba/b",
            "--merge-output-format",
            "mp4",
            "--embed-metadata",
            "-o",
            "%(title)s.%(ext)s",
        ]);
    }

    cmd.arg("--");
    cmd.arg(url);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let que = if tipo == "audio" {
        "la canción"
    } else {
        "el video"
    };
    let (mut aviso_id, _) = aviso(&format!("Bajando {que}…"), url, &[], "", Some(0), 0);
    let mut ultimo = -10i32;

    let mut hijo = match cmd.spawn() {
        Ok(h) => h,
        Err(_) => {
            let _ = std::fs::remove_file(&final_path);
            let _ = aviso(
                &format!("No se pudo bajar {que}"),
                "No se pudo iniciar yt-dlp.",
                &[],
                &aviso_id,
                None,
                0,
            );
            return;
        }
    };

    let hijo_stdout = hijo.stdout.take();
    let hijo_stderr = hijo.stderr.take();

    let hilo_stderr = std::thread::spawn(move || {
        let mut errores = Vec::new();
        if let Some(err) = hijo_stderr {
            let reader = BufReader::new(err);
            for linea in reader.lines().map_while(Result::ok) {
                let recortada = linea.trim().to_string();
                if recortada.starts_with("ERROR") {
                    errores.push(recortada);
                }
            }
        }
        errores
    });

    if let Some(salida) = hijo_stdout {
        let reader = BufReader::new(salida);
        for linea in reader.lines().map_while(Result::ok) {
            let recortada = linea.trim();
            if let Some(caps) = RE_VORTEX.captures(recortada)
                && let Some(m) = caps.get(1)
                && let Ok(val) = m.as_str().parse::<f64>()
            {
                let pct = val as i32;
                if pct >= ultimo + 10 {
                    ultimo = pct;
                    let (nuevo_id, _) = aviso(
                        &format!("Bajando {que}… {pct} %"),
                        url,
                        &[],
                        &aviso_id,
                        Some(pct.max(0) as u32),
                        0,
                    );
                    aviso_id = nuevo_id;
                }
            }
        }
    }

    let estado = hijo.wait();
    let errores = hilo_stderr.join().unwrap_or_default();

    let ruta = std::fs::read_to_string(&final_path)
        .map(|s| s.trim().lines().last().unwrap_or("").trim().to_string())
        .unwrap_or_default();
    let _ = std::fs::remove_file(&final_path);

    let ruta_path = Path::new(&ruta);
    let exito = estado.map(|st| st.success()).unwrap_or(false) && !ruta.is_empty() && ruta_path.exists();

    if !exito {
        let msg_error = por_que(&errores.join(" "));
        let _ = aviso(
            &format!("No se pudo bajar {que}"),
            msg_error,
            &[],
            &aviso_id,
            None,
            0,
        );
        return;
    }

    let nombre = ruta_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let carpeta_padre = ruta_path.parent();
    let nombre_padre = carpeta_padre
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let cuerpo = format!("Está en tu carpeta {nombre_padre}.");

    let (_, eleccion) = aviso(
        &format!("Listo: {nombre}"),
        &cuerpo,
        &[("abrir", "Abrir"), ("carpeta", "Ver en la carpeta")],
        &aviso_id,
        None,
        20000,
    );

    if eleccion == "abrir" {
        let _ = Command::new("xdg-open")
            .arg(&ruta)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
    } else if eleccion == "carpeta"
        && let Some(padre) = carpeta_padre
    {
        let _ = Command::new("xdg-open")
            .arg(padre)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
    }
}

/// PASO 9. Un solo escucha por sesión. Python: `escuchar()`. Toma el candado con
/// `std::fs::File::try_lock()`; si ya lo tiene otro, sale. Luego corre
/// `wl-paste --type text --watch <tu-ruta> revisar` y ESPERA a que termine (`.status()`).
fn escuchar() {
    let run = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(d) => PathBuf::from(d),
        None => std::env::temp_dir(),
    };
    let lock_path = run.join("vortex-enlace.lock");

    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    opts.mode(0o600);

    let Ok(archivo_candado) = opts.open(&lock_path) else {
        return;
    };

    if archivo_candado.try_lock().is_err() {
        return;
    }

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };

    let _ = Command::new("wl-paste")
        .args(["--type", "text", "--watch"])
        .arg(&exe)
        .arg("revisar")
        .status();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => escuchar(),
        ["revisar"] => revisar(),
        ["ofrecer", nombre, url] => ofrecer(nombre, url),
        ["bajar", tipo @ ("video" | "audio"), url] => bajar(tipo, url),
        ["sitio", url] => println!("{}", sitio(url).unwrap_or("None")),
        _ => {
            eprintln!(
                "uso: vortex-enlace-rs [revisar | ofrecer SITIO URL | bajar video|audio URL | sitio URL]"
            );
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
