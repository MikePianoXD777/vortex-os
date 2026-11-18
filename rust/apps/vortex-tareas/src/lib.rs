//! vortex-tareas (Vortex 2.5, en Rust desde la 4.0): tus tareas en la barra. Lee las tareas con fecha de tu
//! bóveda de Obsidian (la que tengas abierta) y las de `Tareas.md`; la barra cuenta las de hoy y mañana, el
//! panel las lista y avisa un día antes. Sin Obsidian, usa ~/Documentos/Tareas.md.
//!
//! Es la misma herramienta que `bin/vortex-tareas` (Python): mismas órdenes, misma salida (el JSON de `listar`
//! lo lee QML, byte por byte igual), mismas variables de entorno y el mismo registro de «ya avisé».
//! Gasta ~1 MB en vez de ~22 MB mientras `avisar` espera el clic en la notificación.

mod fecha;
mod json;
mod sys;

use fecha::{Fecha, Momento};
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

const USO: &str =
    "vortex-tareas (Vortex 2.5): tus tareas en la barra. Lee las tareas con fecha de tu bóveda de Obsidian
(la que tengas abierta) y las de `Tareas.md`; la barra cuenta las de hoy y mañana, el panel las lista
(clic en la cuenta, o lanzador → «Tareas») y avisa un día antes. Sin Obsidian, usa ~/Documentos/Tareas.md.

Las fechas se escriben como en el plugin Tasks de Obsidian (`- [ ] Maqueta de ciencias 📅 2026-11-03`),
o `due:: 2026-11-03`, o a la mexicana: `(3/11)`, «para el 3/11». Al marcarla hecha queda `- [x] … ✅ fecha`.

  vortex-tareas listar             JSON para el panel: {boveda, archivo, tareas: [...]}
  vortex-tareas agregar TEXTO [AAAA-MM-DD]
  vortex-tareas hecha ARCHIVO LINEA TEXTO   (TEXTO es la línea tal cual; si cambió, no toca nada)
  vortex-tareas abrir              abre Tareas.md en Obsidian (o en el editor), creándola si falta
  vortex-tareas avisar             avisa (una vez) de las de mañana después de las 5 pm y de las de hoy
";

const SALTAR: [&str; 4] = [".obsidian", ".trash", ".git", "node_modules"];

static TAREA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*[-*+] \[)( )(\] )(.*)$").unwrap());
static FECHAS: LazyLock<[Regex; 2]> = LazyLock::new(|| {
    [
        Regex::new(r"\s*📅\s*([0-9]{4})-([0-9]{2})-([0-9]{2})").unwrap(),
        Regex::new(r"\s*\[?due::\s*([0-9]{4})-([0-9]{2})-([0-9]{2})\]?").unwrap(),
    ]
});
// «(3/11)» o «para el 3/11»: entre paréntesis o con «para»/«entrega», para no confundir «1/2 taza» con una fecha
static FECHA_CORTA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*(?:\(|\b(?:para el|para|entrega(?: el)?)\s+)([0-9]{1,2})/([0-9]{1,2})(?:/([0-9]{2,4}))?\b\)?")
        .unwrap()
});
static EXTRAS_FECHA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[⏳🛫➕✅🔁]\s*[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap());
static PRIORIDADES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*[🔺⏫🔼🔽⏬]").unwrap());

// ====================================================================== el entorno

fn casa() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn estado_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map_or_else(|| casa().join(".local/state"), PathBuf::from)
        .join("vortex")
}

/// Como `str(Path(s))` de Python: sin `//` repetidos, sin `.` ni barra final.
fn normalizar(s: &str) -> String {
    let inicio = if s.starts_with("//") && !s.starts_with("///") {
        "//"
    } else if s.starts_with('/') {
        "/"
    } else {
        ""
    };
    let partes: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let resto = partes.join("/");
    if inicio.is_empty() && resto.is_empty() {
        ".".into()
    } else {
        format!("{inicio}{resto}")
    }
}

/// `Path(base) / nombre` con la base ya normalizada.
fn unir(base: &str, nombre: &str) -> String {
    if base == "." {
        normalizar(nombre)
    } else if base.ends_with('/') {
        format!("{base}{nombre}")
    } else {
        format!("{base}/{nombre}")
    }
}

/// La hora de ahora; `VORTEX_TAREAS_AHORA` (AAAA-MM-DDTHH:MM) la fija para las pruebas.
fn ahora() -> Result<Momento, String> {
    match std::env::var("VORTEX_TAREAS_AHORA") {
        Ok(f) if !f.is_empty() => {
            fecha::momento_de_iso(&f).ok_or_else(|| format!("VORTEX_TAREAS_AHORA inválida: {f}"))
        }
        _ => {
            let (a, m, d, h) = sys::hora_local();
            Ok(Momento {
                fecha: Fecha::nueva(i64::from(a), i64::from(m), i64::from(d))
                    .ok_or("hora del sistema inválida")?,
                hora: h,
            })
        }
    }
}

fn es_verdadero(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// La bóveda abierta en Obsidian (o la más reciente). `VORTEX_TAREAS_BOVEDA` manda (pruebas).
fn boveda() -> Option<String> {
    if let Some(b) = std::env::var_os("VORTEX_TAREAS_BOVEDA").filter(|b| !b.is_empty()) {
        return Some(normalizar(&b.to_string_lossy()));
    }
    let configs = [
        std::env::var_os("XDG_CONFIG_HOME")
            .map_or_else(|| casa().join(".config"), PathBuf::from)
            .join("obsidian/obsidian.json"),
        casa().join(".var/app/md.obsidian.Obsidian/config/obsidian/obsidian.json"), // Flatpak
    ];
    for c in configs {
        let Ok(texto) = fs::read_to_string(&c) else {
            continue;
        };
        let Ok(Value::Object(raiz)) = serde_json::from_str::<Value>(&texto) else {
            continue;
        };
        let Some(Value::Object(vaults)) = raiz.get("vaults") else {
            continue;
        };
        let mut orden: Vec<&Value> = vaults.values().collect();
        // como `sorted(key=(not open, -ts))` de Python: las abiertas primero, y las más recientes
        orden.sort_by(|a, b| {
            let clave = |v: &Value| {
                (
                    !es_verdadero(v.get("open")),
                    -v.get("ts").and_then(Value::as_f64).unwrap_or(0.0),
                )
            };
            let (ka, kb) = (clave(a), clave(b));
            ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
        });
        for v in orden {
            if let Some(Value::String(p)) = v.get("path")
                && !p.is_empty()
                && Path::new(p).is_dir()
            {
                return Some(normalizar(p));
            }
        }
    }
    None
}

fn archivo_tareas() -> String {
    match boveda() {
        Some(b) => unir(&b, "Tareas.md"),
        None => normalizar(&casa().join("Documentos/Tareas.md").to_string_lossy()),
    }
}

// ====================================================================== leer

/// (texto sin la fecha, fecha). «(3/11)» sin año es el próximo 3 de noviembre (o este, si es hoy).
fn fecha_de(texto: &str, hoy: Fecha) -> (String, Option<Fecha>) {
    for r in FECHAS.iter() {
        if let Some(c) = r.captures(texto) {
            let n = |i: usize| c[i].parse::<i64>().unwrap_or(0);
            if let Some(f) = Fecha::nueva(n(1), n(2), n(3)) {
                let m = c.get(0).unwrap();
                return (
                    format!("{}{}", &texto[..m.start()], &texto[m.end()..])
                        .trim()
                        .to_string(),
                    Some(f),
                );
            }
        }
    }
    if let Some(c) = FECHA_CORTA.captures(texto) {
        let n = |i: usize| c[i].parse::<i64>().unwrap_or(0);
        let (dia, mes) = (n(1), n(2));
        let mut anio = if c.get(3).is_some() { n(3) } else { hoy.anio };
        if anio < 100 {
            anio += 2000;
        }
        let mut f = Fecha::nueva(anio, mes, dia);
        if let Some(fe) = f
            && c.get(3).is_none()
            && fe.dias() < hoy.dias() - 60
        {
            f = Fecha::nueva(anio + 1, mes, dia); // 29 de febrero o año 10000: no hay fecha
        }
        if f.is_some() {
            let m = c.get(0).unwrap();
            let sin = format!("{}{}", &texto[..m.start()], &texto[m.end()..]);
            return (sin.trim_matches(|c| " -·,".contains(c)).to_string(), f);
        }
    }
    (texto.trim().to_string(), None)
}

/// Sin lo que el plugin Tasks agrega (⏳ 🛫 ➕ 🔁 prioridades) para que se lea bien en el panel.
fn limpiar(texto: &str) -> String {
    let t = EXTRAS_FECHA.replace_all(texto, "");
    PRIORIDADES.replace_all(&t, "").trim().to_string()
}

/// `str.splitlines()` de Python.
fn lineas_de(texto: &str) -> Vec<String> {
    let mut lineas = Vec::new();
    let mut actual = String::new();
    let mut cr = false;
    for c in texto.chars() {
        if cr {
            cr = false;
            if c == '\n' {
                continue;
            }
        }
        match c {
            '\r' => {
                lineas.push(std::mem::take(&mut actual));
                cr = true;
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => {
                lineas.push(std::mem::take(&mut actual));
            }
            c => actual.push(c),
        }
    }
    if !actual.is_empty() {
        lineas.push(actual);
    }
    lineas
}

/// `\r\n` y `\r` pasan a `\n`, como al leer un texto en Python.
fn saltos_universales(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// `Path.read_text()` de Python: UTF-8 estricto y saltos de línea universales.
fn leer_texto(ruta: &str) -> Result<String, String> {
    let bytes = fs::read(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    let s = String::from_utf8(bytes).map_err(|_| format!("{ruta}: no es UTF-8"))?;
    Ok(saltos_universales(&s))
}

fn md_de(dir: &str, archivos: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut subdirs = Vec::new();
    for e in rd.flatten() {
        let nombre = e.file_name().to_string_lossy().into_owned();
        let ruta = unir(dir, &nombre);
        if Path::new(&ruta).is_dir() {
            let enlace = e.file_type().is_ok_and(|t| t.is_symlink());
            if !SALTAR.contains(&nombre.as_str()) && !nombre.starts_with('.') && !enlace {
                subdirs.push(ruta);
            }
        } else if nombre.ends_with(".md") {
            archivos.push(ruta);
        }
    }
    for s in subdirs {
        md_de(&s, archivos);
    }
}

/// `Path.stem`
fn nota_de(ruta: &str) -> String {
    let nombre = ruta.rsplit('/').next().unwrap_or(ruta);
    match nombre.rfind('.') {
        Some(i) if i > 0 && i < nombre.len() - 1 => nombre[..i].to_string(),
        _ => nombre.to_string(),
    }
}

pub struct Tarea {
    texto: String,
    fecha: String,
    dias: Option<i64>,
    archivo: String,
    nota: String,
    linea: usize,
    original: String,
}

fn leer(hoy: Fecha) -> Vec<Tarea> {
    let propias = archivo_tareas();
    let mut archivos = Vec::new();
    if let Some(b) = boveda() {
        md_de(&b, &mut archivos);
    }
    if Path::new(&propias).exists() && !archivos.contains(&propias) {
        archivos.push(propias.clone());
    }
    let mut tareas = Vec::new();
    for a in &archivos {
        let Ok(bytes) = fs::read(a) else { continue };
        let texto = saltos_universales(&String::from_utf8_lossy(&bytes));
        for (i, linea) in lineas_de(&texto).iter().enumerate() {
            let Some(m) = TAREA.captures(linea) else { continue };
            let (texto, fecha) = fecha_de(&m[4], hoy);
            if fecha.is_none() && *a != propias {
                continue; // de las demás notas, solo las que tienen fecha
            }
            let texto = limpiar(&texto);
            if texto.is_empty() {
                continue;
            }
            tareas.push(Tarea {
                texto,
                fecha: fecha.map(|f| f.iso()).unwrap_or_default(),
                dias: fecha.map(|f| f.dias() - hoy.dias()),
                archivo: a.clone(),
                nota: nota_de(a),
                linea: i + 1,
                original: linea.clone(),
            });
        }
    }
    tareas.sort_by_cached_key(|t| (t.fecha.is_empty(), t.fecha.clone(), t.texto.to_lowercase()));
    tareas
}

fn listar() -> Result<(), String> {
    let hoy = ahora()?.fecha;
    let mut s = String::from("{\"boveda\": ");
    json::cadena(&boveda().unwrap_or_default(), &mut s);
    s.push_str(", \"archivo\": ");
    json::cadena(&archivo_tareas(), &mut s);
    s.push_str(", \"tareas\": [");
    for (i, t) in leer(hoy).iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str("{\"texto\": ");
        json::cadena(&t.texto, &mut s);
        s.push_str(", \"fecha\": ");
        json::cadena(&t.fecha, &mut s);
        s.push_str(", \"dias\": ");
        match t.dias {
            Some(d) => s.push_str(&d.to_string()),
            None => s.push_str("null"),
        }
        s.push_str(", \"archivo\": ");
        json::cadena(&t.archivo, &mut s);
        s.push_str(", \"nota\": ");
        json::cadena(&t.nota, &mut s);
        s.push_str(&format!(", \"linea\": {}, \"original\": ", t.linea));
        json::cadena(&t.original, &mut s);
        s.push('}');
    }
    s.push_str("]}\n");
    use std::io::Write;
    let _ = std::io::stdout().write_all(s.as_bytes());
    Ok(())
}

// ====================================================================== escribir

fn crear_archivo() -> Result<String, String> {
    let a = archivo_tareas();
    if let Some(padre) = Path::new(&a).parent() {
        fs::create_dir_all(padre).map_err(|e| format!("{}: {e}", padre.display()))?;
    }
    if !Path::new(&a).exists() {
        fs::write(
            &a,
            "---\ntags:\n  - tareas\n---\n# Tareas\n\nLas que agregas desde la barra de Vortexine's. \
             Las de otras notas también salen si llevan fecha (📅 AAAA-MM-DD).\n\n",
        )
        .map_err(|e| format!("{a}: {e}"))?;
    }
    Ok(a)
}

fn agregar(texto: &str, fecha: &str) -> Result<(), String> {
    let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if texto.is_empty() {
        return Err("la tarea está vacía".into());
    }
    if !fecha.is_empty() && fecha::de_iso(fecha).is_none() {
        return Err(format!("Invalid isoformat string: '{fecha}'"));
    }
    let a = crear_archivo()?;
    let mut previo = leer_texto(&a)?;
    let linea = if fecha.is_empty() {
        format!("- [ ] {texto}")
    } else {
        format!("- [ ] {texto} 📅 {fecha}")
    };
    if !previo.ends_with('\n') {
        previo.push('\n');
    }
    previo.push_str(&linea);
    previo.push('\n');
    fs::write(&a, previo).map_err(|e| format!("{a}: {e}"))
}

fn hecha(archivo: &str, linea: i64, original: &str) -> Result<(), String> {
    let texto = leer_texto(archivo)?;
    let mut lineas: Vec<String> = texto.split('\n').map(String::from).collect();
    if linea <= 0 || linea as usize > lineas.len() || lineas[linea as usize - 1] != original {
        return Err("la nota cambió: vuelve a abrir el panel".into());
    }
    let Some(m) = TAREA.captures(original) else {
        return Err("no es una tarea".into());
    };
    let hoy = ahora()?.fecha.iso();
    lineas[linea as usize - 1] = format!("{}x{}{} ✅ {hoy}", &m[1], &m[3], &m[4]);
    fs::write(archivo, lineas.join("\n")).map_err(|e| format!("{archivo}: {e}"))
}

/// `urllib.parse.quote(s, safe="")`
fn codificar(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn abrir() -> Result<(), String> {
    let a = crear_archivo()?;
    let destino = if boveda().is_some() {
        format!("obsidian://open?path={}", codificar(&a))
    } else {
        a
    };
    let mut orden = Command::new("xdg-open");
    orden.arg(destino).stdout(Stdio::null()).stderr(Stdio::null());
    sys::en_sesion_nueva(&mut orden);
    orden.spawn().map(drop).map_err(|e| format!("xdg-open: {e}"))
}

// ====================================================================== avisar

fn jugando() -> bool {
    estado_dir().join("modo-juego").exists()
}

/// `shutil.which`
fn en_path(programa: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|d| {
        fs::metadata(d.join(programa)).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}

/// Una vez por tarea y por día: las de mañana después de las 5 pm; las de hoy (y vencidas) desde las 7 am.
fn avisar() -> Result<(), String> {
    if jugando() || !en_path("notify-send") {
        return Ok(());
    }
    let momento = ahora()?;
    let hoy = momento.fecha;
    let estado_dir = estado_dir();
    let archivo = estado_dir.join("tareas-avisos.json");
    let dia = hoy.iso();
    let mut avisadas: Vec<String> = Vec::new();
    if let Some(Value::Object(e)) = fs::read_to_string(&archivo)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        && e.get("dia").and_then(Value::as_str) == Some(dia.as_str())
        && let Some(Value::Array(l)) = e.get("avisadas")
    {
        avisadas = l.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    }
    let mut vistas: HashSet<String> = avisadas.iter().cloned().collect();
    let mut avisar_de = Vec::new();
    for t in leer(hoy) {
        let Some(dias) = t.dias else { continue };
        let clave = format!("{}|{}", t.texto, t.fecha);
        let toca = (dias <= 0 && momento.hora >= 7) || (dias == 1 && momento.hora >= 17);
        if toca && !vistas.contains(&clave) {
            vistas.insert(clave.clone());
            avisadas.push(clave);
            avisar_de.push((t.texto, dias));
        }
    }
    if avisar_de.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(&estado_dir).map_err(|e| format!("{}: {e}", estado_dir.display()))?;
    let mut s = format!("{{\"dia\": \"{dia}\", \"avisadas\": [");
    for (i, c) in avisadas.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        json::cadena(c, &mut s);
    }
    s.push_str("]}");
    fs::write(&archivo, s).map_err(|e| format!("{}: {e}", archivo.display()))?;

    let hay_manana = avisar_de.iter().any(|(_, d)| *d == 1);
    let hay_hoy = avisar_de.iter().any(|(_, d)| *d <= 0);
    let solo_manana = hay_manana && !hay_hoy;
    let (titulo, cuerpo) = if avisar_de.len() > 1 {
        let cuerpo = avisar_de
            .iter()
            .take(5)
            .map(|(t, d)| {
                let marca = if *d == 1 {
                    " (mañana)"
                } else if *d < 0 {
                    " (vencida)"
                } else {
                    ""
                };
                format!("• {t}{marca}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let titulo = format!(
            "Tienes {} tareas {}",
            avisar_de.len(),
            if solo_manana { "para mañana" } else { "pendientes" }
        );
        (titulo, cuerpo)
    } else {
        let previo = if solo_manana {
            "Para mañana: "
        } else {
            "Para hoy: "
        };
        (
            format!("{previo}{}", avisar_de[0].0),
            "Te lo recuerdo desde ahora.".to_string(),
        )
    };
    let r = Command::new("notify-send")
        .args([
            "-a",
            "Vortexine's OS",
            "-i",
            "view-task",
            "-A",
            "ver=Ver mis tareas",
        ])
        .arg(titulo)
        .arg(cuerpo)
        .output()
        .map_err(|e| format!("notify-send: {e}"))?;
    if String::from_utf8_lossy(&r.stdout).trim() == "ver" {
        let serp = std::env::var("SERPANTINUM_DIR").unwrap_or_else(|_| "/usr/share/serpantinum/src".into());
        let mut orden = Command::new("bash");
        orden
            .arg(format!("{serp}/scripts/qs_manager.sh"))
            .args(["toggle", "tareas"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        sys::en_sesion_nueva(&mut orden);
        let _ = orden.spawn();
    }
    Ok(())
}

// ====================================================================== la línea de órdenes

/// Corre la orden y devuelve el código de salida (0 bien, 1 mal; los mensajes van a stderr).
pub fn principal(args: &[String]) -> u8 {
    let r = match args.first().map(String::as_str) {
        None | Some("listar") => listar(),
        Some("agregar") if args.len() >= 2 => agregar(&args[1], args.get(2).map_or("", String::as_str)),
        Some("hecha") if args.len() == 4 => match args[2].trim().parse::<i64>() {
            Ok(n) => hecha(&args[1], n, &args[3]),
            Err(_) => Err(format!("invalid literal for int() with base 10: '{}'", args[2])),
        },
        Some("abrir") => abrir(),
        Some("avisar") => avisar(),
        Some(_) => Err(USO.trim_end_matches('\n').to_string()),
    };
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(a: i64, m: i64, d: i64) -> Fecha {
        Fecha::nueva(a, m, d).unwrap()
    }

    #[test]
    fn fechas_del_texto() {
        let hoy = f(2026, 11, 2);
        let (t, fe) = fecha_de("Examen 📅 2026-11-02", hoy);
        assert_eq!((t.as_str(), fe), ("Examen", Some(f(2026, 11, 2))));
        let (t, fe) = fecha_de("Maqueta de ciencias (3/11)", hoy);
        assert_eq!((t.as_str(), fe), ("Maqueta de ciencias", Some(f(2026, 11, 3))));
        let (t, fe) = fecha_de("Problemas para el 6/11", hoy);
        assert_eq!((t.as_str(), fe), ("Problemas", Some(f(2026, 11, 6))));
        assert_eq!(
            fecha_de("Receta: 1/2 taza", hoy),
            ("Receta: 1/2 taza".to_string(), None)
        );
        // sin año y de hace más de 60 días: es del año que viene
        assert_eq!(fecha_de("algo (1/1)", hoy).1, Some(f(2027, 1, 1)));
        // 29 de febrero de un año que no es bisiesto: no hay fecha
        assert_eq!(fecha_de("algo (29/2/2027)", hoy).1, None);
        // una fecha larga inválida no impide probar con la siguiente forma
        let (t, fe) = fecha_de("x 📅 2026-13-40 due:: 2026-11-05", hoy);
        assert_eq!((t.as_str(), fe), ("x 📅 2026-13-40", Some(f(2026, 11, 5))));
    }

    #[test]
    fn limpia_lo_del_plugin() {
        assert_eq!(limpiar("Pagar ⏳ 2026-11-01 ⏫ ✅ 2026-11-02"), "Pagar");
    }

    #[test]
    fn rutas_como_pathlib() {
        assert_eq!(normalizar("/a//b/./c/"), "/a/b/c");
        assert_eq!(normalizar("./a/"), "a");
        assert_eq!(normalizar(""), ".");
        assert_eq!(normalizar("//a"), "//a");
        assert_eq!(normalizar("///a"), "/a");
        assert_eq!(nota_de("/x/Tareas.md"), "Tareas");
        assert_eq!(nota_de("/x/.md"), ".md");
        assert_eq!(nota_de("/x/a.b.md"), "a.b");
    }

    #[test]
    fn splitlines_de_python() {
        assert_eq!(lineas_de("a\nb\r\nc\u{2028}d\n"), ["a", "b", "c", "d"]);
        assert_eq!(lineas_de("a\n\nb"), ["a", "", "b"]);
        assert!(lineas_de("").is_empty());
    }

    #[test]
    fn codifica_como_quote() {
        assert_eq!(codificar("/a b/ñ.md"), "%2Fa%20b%2F%C3%B1.md");
    }
}
