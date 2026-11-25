//! 4.5: reabrir lo que cerraste sin querer (Super+Shift+R, o `vortex reabrir`).
//!
//! El gestor anota en `$XDG_RUNTIME_DIR/vortex/cerradas` la clase de cada app que se cierra del todo (su
//! última ventana) y que estuvo abierta al menos unos segundos: así no cuentan las ventanitas de arranque
//! ni los diálogos. `vortex-gestor-rs reabrir` saca la última, busca su `.desktop` y la vuelve a abrir;
//! cada vez que se aprieta, una más atrás (como las pestañas del navegador).

use std::collections::BTreeMap;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Cuántas se recuerdan.
const TOPE: usize = 20;
/// Lo que vivió menos que esto (en segundos) no se anota: ventanas de arranque, avisos, diálogos.
pub const VIDA_MINIMA: i64 = 3;

/// Lo que no es una app que la persona haya abierto: el propio shell y sus paneles.
pub fn se_anota(clase: &str) -> bool {
    let c = clase.trim().to_ascii_lowercase();
    !(c.is_empty()
        || c.contains("quickshell")
        || c.contains("serpantinum")
        || c.starts_with("xdg-desktop-portal")
        || c == "polkit-gnome-authentication-agent-1"
        || c == "hyprland-share-picker"
        || c == "hyprpolkitagent")
}

pub fn archivo(estado_dir: &Path) -> PathBuf {
    estado_dir.join("cerradas")
}

fn leer(arch: &Path) -> Vec<String> {
    std::fs::read_to_string(arch)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn escribir(arch: &Path, l: &[String]) {
    let mut t = l.join("\n");
    if !t.is_empty() {
        t.push('\n');
    }
    let tmp = arch.with_extension("tmp");
    if std::fs::write(&tmp, t).is_ok() {
        let _ = std::fs::rename(&tmp, arch);
    }
}

/// Anota una app cerrada (la más nueva va al final). Si ya estaba, se mueve al final.
pub fn anotar(estado_dir: &Path, clase: &str) {
    if !se_anota(clase) {
        return;
    }
    let _ = std::fs::create_dir_all(estado_dir);
    let arch = archivo(estado_dir);
    let mut l = leer(&arch);
    l.retain(|c| c != clase);
    l.push(clase.to_string());
    if l.len() > TOPE {
        l.drain(..l.len() - TOPE);
    }
    escribir(&arch, &l);
}

/// Saca la última cerrada (y la quita de la lista).
pub fn sacar(estado_dir: &Path) -> Option<String> {
    let arch = archivo(estado_dir);
    let mut l = leer(&arch);
    let c = l.pop()?;
    escribir(&arch, &l);
    Some(c)
}

/// Las carpetas `applications` donde buscar, en orden: las del usuario primero.
pub fn carpetas_apps() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let datos = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{home}/.local/share"));
    let sis = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut v = vec![PathBuf::from(datos).join("applications")];
    for d in sis.split(':').filter(|d| !d.is_empty()) {
        v.push(PathBuf::from(d).join("applications"));
    }
    // Las apps de Flatpak, por si XDG_DATA_DIRS no las trae.
    v.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));
    v.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    let mut vistos = std::collections::BTreeSet::new();
    v.retain(|p| vistos.insert(p.clone()));
    v
}

/// Las claves de la sección `[Desktop Entry]` de un `.desktop`.
fn entrada(texto: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut dentro = false;
    for l in texto.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            dentro = l == "[Desktop Entry]";
            continue;
        }
        if !dentro || l.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            m.entry(k.trim().to_string())
                .or_insert_with(|| v.trim().to_string());
        }
    }
    m
}

/// El `Exec=` partido en argumentos, sin los códigos de campo (%f, %U…), según la especificación.
pub fn partir_exec(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut actual = String::new();
    let mut hay = false;
    let mut comillas = false;
    let mut cs = exec.chars().peekable();
    while let Some(c) = cs.next() {
        match c {
            '"' => {
                comillas = !comillas;
                hay = true;
            }
            '\\' if comillas => {
                if let Some(n) = cs.next() {
                    actual.push(n);
                }
            }
            // «%%» es un %; %f %F %u %U %i %c %k y los viejos se quitan.
            '%' => {
                if cs.next() == Some('%') {
                    actual.push('%');
                }
            }
            c if c.is_whitespace() && !comillas => {
                if hay || !actual.is_empty() {
                    args.push(std::mem::take(&mut actual));
                    hay = false;
                }
            }
            c => actual.push(c),
        }
    }
    if hay || !actual.is_empty() {
        args.push(actual);
    }
    args.retain(|a| !a.is_empty());
    args
}

/// El comando para abrir la app de esa clase: del `.desktop` que la nombra (StartupWMClass, el nombre
/// del archivo, su última parte —org.gnome.Nautilus ↔ nautilus— o el programa de `Exec`).
pub fn buscar(clase: &str, carpetas: &[PathBuf]) -> Option<Vec<String>> {
    let c = clase.to_ascii_lowercase();
    // (prioridad, comando): gana la prioridad más baja; a igual prioridad, la primera carpeta.
    let mut mejor: Option<(u8, Vec<String>)> = None;
    for dir in carpetas {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut archivos: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "desktop"))
            .collect();
        archivos.sort();
        for p in archivos {
            let Ok(t) = std::fs::read_to_string(&p) else {
                continue;
            };
            let e = entrada(&t);
            if e.get("Hidden").is_some_and(|v| v == "true")
                || e.get("Type").is_some_and(|v| v != "Application")
            {
                continue;
            }
            let Some(exec) = e.get("Exec") else { continue };
            let args = partir_exec(exec);
            let Some(prog) = args.first() else { continue };
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let ultima = stem.rsplit('.').next().unwrap_or("").to_string();
            let base = Path::new(prog)
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let prio = if e
                .get("StartupWMClass")
                .is_some_and(|w| w.eq_ignore_ascii_case(&c))
            {
                0
            } else if stem == c {
                1
            } else if ultima == c {
                2
            } else if base == c {
                3
            } else {
                continue;
            };
            if mejor.as_ref().is_none_or(|(m, _)| prio < *m) {
                mejor = Some((prio, args));
            }
        }
    }
    mejor.map(|(_, a)| a)
}

fn en_path(prog: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|d| {
            let f = d.join(prog);
            f.is_file()
                && std::fs::metadata(&f).is_ok_and(|m| {
                    use std::os::unix::fs::PermissionsExt;
                    m.permissions().mode() & 0o111 != 0
                })
        })
    })
}

fn avisar(titulo: &str, texto: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "Vortexine's OS", "-i", "edit-undo", titulo, texto])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut h| h.wait());
}

fn lanzar(args: &[String]) -> bool {
    let Some((prog, resto)) = args.split_first() else {
        return false;
    };
    Command::new(prog)
        .args(resto)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Su propio grupo de procesos: no se va con este al terminar.
        .process_group(0)
        .spawn()
        .is_ok()
}

/// `vortex-gestor-rs reabrir`: abre la última app cerrada.
pub fn correr(estado_dir: &Path) -> i32 {
    // Si una ya no se sabe abrir (se desinstaló), se intenta con la anterior.
    while let Some(clase) = sacar(estado_dir) {
        let cmd = buscar(&clase, &carpetas_apps()).or_else(|| {
            let p = clase.to_ascii_lowercase();
            en_path(&p).then(|| vec![p])
        });
        match cmd {
            Some(a) if lanzar(&a) => {
                println!("{clase}");
                return 0;
            }
            _ => {
                avisar(
                    "No se pudo volver a abrir",
                    &format!("No encontré cómo abrir «{clase}». Pruebo con la anterior."),
                );
            }
        }
    }
    avisar(
        "No hay nada que reabrir",
        "Aquí aparecen las apps que cierres; con Super+Shift+R vuelves a abrir la última.",
    );
    1
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn exec_sin_codigos() {
        assert_eq!(partir_exec("firefox %u"), vec!["firefox"]);
        assert_eq!(
            partir_exec(r#"/usr/bin/flatpak run --branch=stable com.discordapp.Discord @@u %U @@"#),
            vec![
                "/usr/bin/flatpak",
                "run",
                "--branch=stable",
                "com.discordapp.Discord",
                "@@u",
                "@@"
            ]
        );
        assert_eq!(
            partir_exec(r#""/opt/Mi App/app" --x "a \"b\"" 100%%"#),
            vec!["/opt/Mi App/app", "--x", "a \"b\"", "100%"]
        );
    }

    #[test]
    fn lista_con_tope_y_sin_repetidos() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..25 {
            anotar(d.path(), &format!("app{i}"));
        }
        anotar(d.path(), "app10");
        anotar(d.path(), "org.quickshell");
        anotar(d.path(), "");
        let l = leer(&archivo(d.path()));
        assert_eq!(l.len(), TOPE);
        assert_eq!(l.last().unwrap(), "app10");
        assert_eq!(sacar(d.path()).as_deref(), Some("app10"));
        assert_eq!(sacar(d.path()).as_deref(), Some("app24"));
    }

    #[test]
    fn busca_el_desktop() {
        let d = tempfile::tempdir().unwrap();
        let w = |n: &str, t: &str| std::fs::write(d.path().join(n), t).unwrap();
        w(
            "org.gnome.Nautilus.desktop",
            "[Desktop Entry]\nType=Application\nExec=nautilus --new-window %U\n",
        );
        w(
            "vortex-player.desktop",
            "[Desktop Entry]\nType=Application\nExec=vortex-player\nStartupWMClass=VortexPlayer\n\
             [Desktop Action nueva]\nExec=otra\n",
        );
        w("kitty.desktop", "[Desktop Entry]\nType=Application\nExec=kitty\n");
        w(
            "oculta.desktop",
            "[Desktop Entry]\nType=Application\nExec=oculta\nHidden=true\n",
        );
        let c = vec![d.path().to_path_buf()];
        assert_eq!(
            buscar("org.gnome.Nautilus", &c).unwrap(),
            vec!["nautilus", "--new-window"]
        );
        assert_eq!(buscar("nautilus", &c).unwrap(), vec!["nautilus", "--new-window"]);
        assert_eq!(buscar("vortexplayer", &c).unwrap(), vec!["vortex-player"]);
        assert_eq!(buscar("kitty", &c).unwrap(), vec!["kitty"]);
        assert!(buscar("oculta", &c).is_none());
        assert!(buscar("nada", &c).is_none());
    }
}
