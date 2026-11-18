//! vortex-memoria en Rust: el aviso antes de quedarte sin memoria. LA ESCRIBE MIKE.
//!
//! Es la misma app que `bin/vortex-memoria` (Python). Dice qué app se está comiendo la memoria y da un
//! botón para cerrarla. La lanza vortex-gestor cuando queda poca memoria (y al abrir un juego). Justo en
//! ese momento es cuando más importa que arranque rápido y gaste poco: por eso la pasamos a Rust.
//!
//!   vortex-memoria-rs                          avisa
//!   vortex-memoria-rs --ver                    solo dice qué avisaría (no abre nada)
//!   vortex-memoria-rs --juego CLASE [PID] [--ver]
//!                                              al abrir un juego: si le queda poca memoria, ofrece cerrar
//!                                              lo que más gasta (menos el juego y su lanzador)
//!
//! Cada función de abajo es una del Python, con el mismo nombre. Abre `bin/vortex-memoria` al lado y ve
//! pasándolas una por una, EN ORDEN (los pasos 1 a 5 tienen pruebas chiquitas). Lee primero la GUIA.md.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

/// Con menos que esto (300 MB), cerrarla no arregla nada: no se ofrece.
const MINIMO_KB: u64 = 300 * 1024;
/// Para jugar: menos de 3 GB libres (o del 40 % de la RAM, en equipos de 4 GB) ya es ir justo.
const JUEGO_MAX_KB: u64 = 3 * 1024 * 1024;
const JUEGO_PARTE: f64 = 0.4;
/// Cuántas apps ofrece cerrar al jugar, como mucho.
const JUEGO_APPS: usize = 3;
/// Cerrarlas cerraría el juego o te dejaría sin lanzador: nunca se ofrecen (Python: `LANZADORES`).
const LANZADORES: &str = r"(?i)(^steam$|steamwebhelper|lutris|heroic|prismlauncher|minecraft|bottles|gamescope|sober|vinegar|^steam_app_|\.exe$|vortex-task-manager)";

/// Una app con ventana. En Python es un diccionario (`{"clase": ..., "kb": ..., "direcciones": [...]}`);
/// en Rust se usa un `struct`, que es como un diccionario con campos fijos.
#[derive(Debug, Clone, Default)]
struct App {
    /// Su cgroup (lo que devuelve `grupo_de`): sirve para no cerrar el juego.
    grupo: String,
    /// La clase de su ventana (`com.discordapp.Discord`).
    clase: String,
    /// Cuánta memoria usa, en KB.
    kb: u64,
    /// Las direcciones de sus ventanas (`0x55...`), para cerrarlas.
    direcciones: Vec<String>,
    /// El nombre bonito (`Discord`). Vacío hasta que se lo pongas con `nombre_de`.
    nombre: String,
}

// ---------------------------------------------------------------------------------------------------
// Ya hechos (para que veas cómo se hacen y no te atores en lo aburrido).

/// La carpeta /proc (las pruebas la cambian por una falsa con `VORTEX_MEMORIA_PROC`).
fn proc_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("VORTEX_MEMORIA_PROC").unwrap_or_else(|| "/proc".into()))
}

/// Los segundos que espera el juego a cargar antes de medir (`VORTEX_MEMORIA_ESPERA`, 15 si no).
fn espera_s() -> f64 {
    std::env::var("VORTEX_MEMORIA_ESPERA")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(15.0)
}

/// Tu número de usuario (el `os.getuid()` de Python). Rust no lo trae sin `unsafe`, pero /proc/self es
/// tuyo, así que su dueño eres tú. (Ojo: el /proc DE VERDAD, no el falso de las pruebas.)
fn mi_uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(u32::MAX)
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
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

// ---------------------------------------------------------------------------------------------------
// Pasos implementados.

/// PASO 1. Los KB en texto: «1.5 GB» si son 104858 KB o más, si no «300 MB». Python: `gb()`.
fn gb(kb: u64) -> String {
    if kb >= 104_858 {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    } else {
        format!("{} MB", kb / 1_024)
    }
}

/// PASO 2. El número que va después de «Clave:» en un texto como el de /proc/meminfo o /proc/PID/status
/// («MemAvailable:    600000 kB» → 600000), o 0 si no está.
fn leer_clave(texto: &str, clave: &str) -> u64 {
    for line in texto.lines() {
        if let Some(rest) = line.strip_prefix(clave)
            && let Some(rest) = rest.strip_prefix(':')
            && let Some(num_str) = rest.split_whitespace().next()
            && let Ok(val) = num_str.parse::<u64>()
        {
            return val;
        }
    }
    0
}

/// Un número de /proc/meminfo (ya hecho: usa tu paso 2).
fn meminfo(clave: &str) -> u64 {
    let texto = std::fs::read_to_string(proc_dir().join("meminfo")).unwrap_or_default();
    leer_clave(&texto, clave)
}

/// PASO 3. ¿Le queda poca memoria al juego? Python: `justa_para_jugar()`.
fn justa_para_jugar(queda: u64, total: u64) -> bool {
    if queda == 0 {
        return false;
    }
    let limite = JUEGO_MAX_KB.min((total as f64 * JUEGO_PARTE) as u64);
    queda < limite
}

/// PASO 4. «Discord», «Discord y Chrome», «Discord, Chrome y Spotify». Python: `juntar()`.
fn juntar(nombres: &[String]) -> String {
    match nombres.len() {
        0 => String::new(),
        1 => nombres[0].clone(),
        _ => {
            let mut result = nombres[..nombres.len() - 1].join(", ");
            result.push_str(" y ");
            result.push_str(&nombres[nombres.len() - 1]);
            result
        }
    }
}

/// PASO 5a. El texto del aviso normal. Python: `texto_aviso()`.
fn texto_aviso(app: Option<&App>, queda: u64) -> String {
    let mut s = format!("Quedan {}.", gb(queda));
    match app {
        Some(a) => {
            s.push_str(&format!(" {} es lo que más usa: {}.", a.nombre, gb(a.kb)));
        }
        None => {
            s.push_str(" Mira qué la está usando antes de que algo se cierre solo.");
        }
    }
    s
}

/// PASO 5b. El texto del aviso al jugar. Python: `texto_juego()`.
fn texto_juego(juego: &str, apps: &[App], queda: u64) -> String {
    let total_kb: u64 = apps.iter().map(|a| a.kb).sum();
    let nombres: Vec<String> = apps.iter().map(|a| a.nombre.clone()).collect();
    let apps_str = juntar(&nombres);
    format!(
        "Cierra {} y {} tendrá unos {} más (ahora le quedan {}). Así va con menos tirones.",
        apps_str,
        juego,
        gb(total_kb),
        gb(queda)
    )
}

/// PASO 6. El cgroup de un proceso. Python: `grupo_de()`.
fn grupo_de(pid: u32) -> String {
    let ruta = proc_dir().join(pid.to_string()).join("cgroup");
    let contenido = std::fs::read_to_string(ruta).unwrap_or_default();
    let hoja = contenido.trim().rsplit('/').next().unwrap_or("").to_string();
    if hoja.starts_with("app-") && hoja.ends_with(".scope") {
        hoja
    } else {
        format!("pid:{pid}")
    }
}

/// PASO 7. Lo que de verdad usa un proceso: suma de RssAnon, RssShmem y VmSwap. Python: `uso_kb()`.
fn uso_kb(pid: u32) -> u64 {
    let ruta = proc_dir().join(pid.to_string()).join("status");
    let texto = std::fs::read_to_string(ruta).unwrap_or_default();
    leer_clave(&texto, "RssAnon") + leer_clave(&texto, "RssShmem") + leer_clave(&texto, "VmSwap")
}

/// PASO 8. Cuánto usa cada grupo, sumando sus procesos (solo los tuyos). Python: `usos_por_grupo()`.
fn usos_por_grupo() -> HashMap<String, u64> {
    use std::os::unix::fs::MetadataExt;
    let uid = mi_uid();
    let mut usos: HashMap<String, u64> = HashMap::new();
    let Ok(entradas) = std::fs::read_dir(proc_dir()) else {
        return usos;
    };
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name();
        let Some(nombre_str) = nombre.to_str() else {
            continue;
        };
        let Ok(pid) = nombre_str.parse::<u32>() else {
            continue;
        };
        let Ok(meta) = entrada.metadata() else {
            continue;
        };
        if meta.uid() != uid {
            continue;
        }
        let grupo = grupo_de(pid);
        *usos.entry(grupo).or_insert(0) += uso_kb(pid);
    }
    usos
}

/// PASO 9. Las ventanas abiertas (`hyprctl clients -j`). Python: `ventanas()`.
fn ventanas() -> Vec<Value> {
    let Ok(salida) = Command::new("hyprctl").args(["clients", "-j"]).output() else {
        return Vec::new();
    };
    let Ok(texto) = std::str::from_utf8(&salida.stdout) else {
        return Vec::new();
    };
    let Ok(lista) = serde_json::from_str::<Value>(texto) else {
        return Vec::new();
    };
    let Some(arr) = lista.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter(|v| {
            v.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) > 1
                && v.get("mapped").and_then(|m| m.as_bool()).unwrap_or(true)
        })
        .cloned()
        .collect()
}

/// PASO 10. El nombre de la app como sale en el lanzador. Python: `nombre_de()`.
fn nombre_de(clase: &str) -> String {
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| home().join(".local/share").to_string_lossy().into_owned());
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    let mut carpetas = vec![xdg_data_home];
    carpetas.extend(xdg_data_dirs.split(':').map(|s| s.to_string()));
    carpetas.push("/var/lib/flatpak/exports/share".to_string());
    carpetas.push(
        home()
            .join(".local/share/flatpak/exports/share")
            .to_string_lossy()
            .into_owned(),
    );

    let buscada = clase.to_lowercase();
    for carpeta in &carpetas {
        let dir = std::path::Path::new(carpeta).join("applications");
        let entradas = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entrada in entradas.flatten() {
            let ruta = entrada.path();
            if ruta.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let texto = match std::fs::read_to_string(&ruta) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let mut nombre = String::new();
            let mut wm = String::new();
            for linea in texto.lines() {
                if linea.starts_with('[') && linea != "[Desktop Entry]" {
                    break;
                }
                if linea.starts_with("Name=") && nombre.is_empty() {
                    nombre = linea[5..].trim().to_string();
                } else if let Some(stripped) = linea.strip_prefix("StartupWMClass=") {
                    wm = stripped.trim().to_lowercase();
                }
            }
            let stem = ruta
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !nombre.is_empty() && (stem == buscada || wm == buscada) {
                return nombre;
            }
        }
    }
    // No lo encontró: usa la última parte de la clase
    let base = clase.rsplit('.').next().unwrap_or("Esa app");
    let base = base.replace('-', " ");
    let base = base.trim();
    if base.is_empty() {
        return "Esa app".to_string();
    }
    let mut chars = base.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

/// PASO 11. Las apps con ventana, agrupadas por cgroup. Python: `apps_con_ventana()`.
fn apps_con_ventana() -> Vec<App> {
    let usos = usos_por_grupo();
    let mut apps: Vec<App> = Vec::new();
    for v in ventanas() {
        let pid = v["pid"].as_u64().unwrap_or(0) as u32;
        let clase = v["class"].as_str().unwrap_or("").to_string();
        let direccion = v["address"].as_str().unwrap_or("").to_string();
        let grupo = grupo_de(pid);
        let kb = usos.get(&grupo).copied().unwrap_or(0);
        if let Some(app) = apps.iter_mut().find(|a| a.grupo == grupo) {
            app.direcciones.push(direccion);
        } else {
            apps.push(App {
                grupo,
                clase,
                kb,
                direcciones: vec![direccion],
                nombre: String::new(),
            });
        }
    }
    apps
}

/// PASO 12a. La app con ventana que más usa (si usa al menos MINIMO_KB). Python: `la_que_mas_usa()`.
/// OJO: en empate, Python se queda con la PRIMERA (igual que este for con >).
fn la_que_mas_usa() -> Option<App> {
    let apps = apps_con_ventana();
    let mut mayor: Option<App> = None;
    for app in apps {
        let es_mas = match &mayor {
            None => true,
            Some(m) => app.kb > m.kb,
        };
        if es_mas {
            mayor = Some(app);
        }
    }
    let mut app = mayor?;
    if app.kb < MINIMO_KB || app.clase.is_empty() {
        return None;
    }
    app.nombre = nombre_de(&app.clase);
    Some(app)
}

/// PASO 12b. Las que más gastan al jugar, sin el juego ni los lanzadores, hasta 3.
/// Python: `para_cerrar_al_jugar()`.
fn para_cerrar_al_jugar(clase_juego: &str, pid_juego: u32) -> Vec<App> {
    let Ok(re) = regex::Regex::new(LANZADORES) else {
        return Vec::new();
    };
    let grupo_juego = if pid_juego > 1 {
        Some(grupo_de(pid_juego))
    } else {
        None
    };
    let mut elegidas: Vec<App> = apps_con_ventana()
        .into_iter()
        .filter(|app| {
            let clase = &app.clase;
            if clase.is_empty() {
                return false;
            }
            if grupo_juego.as_deref() == Some(app.grupo.as_str()) {
                return false;
            }
            if clase.to_lowercase() == clase_juego.to_lowercase() {
                return false;
            }
            if re.is_match(clase) {
                return false;
            }
            app.kb >= MINIMO_KB
        })
        .collect();
    elegidas.sort_by_key(|a| std::cmp::Reverse(a.kb));
    elegidas.truncate(JUEGO_APPS);
    for app in &mut elegidas {
        app.nombre = nombre_de(&app.clase);
    }
    elegidas
}

/// PASO 13. El nombre del juego. Python: `nombre_juego()`.
fn nombre_juego(clase: &str, pid: u32) -> String {
    if clase.is_empty() {
        return "tu juego".to_string();
    }
    let es_steam = regex::Regex::new(r"(?i)^steam_app_\d+$")
        .map(|re| re.is_match(clase))
        .unwrap_or(false);
    if es_steam || clase.to_lowercase().ends_with(".exe") {
        for v in ventanas() {
            let v_pid = v["pid"].as_u64().unwrap_or(0) as u32;
            let v_clase = v["class"].as_str().unwrap_or("");
            let titulo = v["title"].as_str().unwrap_or("").trim().to_string();
            if (v_pid == pid || v_clase == clase) && !titulo.is_empty() {
                return titulo.chars().take(40).collect();
            }
        }
        return "tu juego".to_string();
    }
    nombre_de(clase)
}

/// PASO 14a. Le manda órdenes a Hyprland: la primera que entienda. Python: `despachar()`.
fn despachar(ordenes: &[String]) {
    for orden in ordenes {
        let ok = Command::new("hyprctl")
            .arg("dispatch")
            .arg(orden)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return;
        }
    }
}

/// PASO 14b. Cierra las apps dadas. Python: `cerrar()`.
fn cerrar(apps: &[App]) {
    for app in apps {
        for d in &app.direcciones {
            despachar(&[
                format!("hl.dsp.window.close({{ window = \"address:{d}\" }})"),
                format!("closewindow address:{d}"),
            ]);
        }
    }
}

/// PASO 15. El aviso al abrir un juego. Python: `al_jugar()`.
fn al_jugar(args: &[String]) -> i32 {
    let ver = args.iter().any(|a| a == "--ver");
    let args_sin_ver: Vec<&str> = args
        .iter()
        .filter(|a| *a != "--ver")
        .map(|a| a.as_str())
        .collect();
    let clase = args_sin_ver.first().copied().unwrap_or("");
    let pid: u32 = args_sin_ver.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    if !ver && espera_s() > 0.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(espera_s()));
    }

    if pid > 1 && !proc_dir().join(pid.to_string()).exists() {
        return 0; // el juego ya se cerró mientras esperábamos
    }

    let queda = meminfo("MemAvailable");
    let total = meminfo("MemTotal");
    if !justa_para_jugar(queda, total) {
        if ver {
            println!("Le alcanza: quedan {}.", gb(queda));
        }
        return 0;
    }

    let apps = para_cerrar_al_jugar(clase, pid);
    if apps.is_empty() {
        if ver {
            println!("Quedan {}, pero no hay nada que valga la pena cerrar.", gb(queda));
        }
        return 0;
    }

    let juego = nombre_juego(clase, pid);
    let texto = texto_juego(&juego, &apps, queda);

    if ver {
        println!("{texto}");
        return 0;
    }

    if !tiene_comando("notify-send") {
        return 0;
    }

    let nombres: Vec<String> = apps.iter().map(|a| a.nombre.clone()).collect();
    let boton = if apps.len() < 3 {
        format!("Cerrar {}", juntar(&nombres))
    } else {
        format!("Cerrar las {}", apps.len())
    };

    let accion = Command::new("timeout")
        .args([
            "120",
            "notify-send",
            "-a",
            "Vortexine's OS",
            "-i",
            "applications-games",
            "-u",
            "normal",
            "-t",
            "30000",
            "-A",
            &format!("cerrar={boton}"),
            "-A",
            "no=Así está bien",
            "Tu juego va justo de memoria",
            &texto,
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let accion = accion.trim();

    if accion == "cerrar" {
        cerrar(&apps);
    }
    0
}

/// PASO 16. El aviso normal. Python: `main()` (tras el --juego).
fn avisar(ver: bool) -> i32 {
    let app = la_que_mas_usa();
    let queda = meminfo("MemAvailable");

    if ver {
        println!("{}", texto_aviso(app.as_ref(), queda));
        return 0;
    }

    if !tiene_comando("notify-send") {
        return 0;
    }

    let mut cmd = Command::new("timeout");
    cmd.args([
        "1800",
        "notify-send",
        "-a",
        "Vortexine's OS",
        "-i",
        "dialog-warning",
        "-u",
        "critical",
        "-A",
        "default=Ver",
    ]);
    if let Some(ref a) = app {
        cmd.args(["-A", &format!("cerrar=Cerrar {}", a.nombre)]);
    }
    cmd.args([
        "-A",
        "ver=Ver qué más usa",
        "Te estás quedando sin memoria",
        &texto_aviso(app.as_ref(), queda),
    ]);

    let accion = cmd
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let accion = accion.trim();

    if accion == "cerrar" {
        if let Some(a) = app {
            cerrar(&[a]);
        }
    } else if accion == "ver" || accion == "default" {
        despachar(&[
            "hl.dsp.exec_cmd(\"vortex-task-manager\")".to_string(),
            "exec vortex-task-manager".to_string(),
        ]);
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let codigo = if args.first().map(String::as_str) == Some("--juego") {
        al_jugar(&args[1..])
    } else {
        avisar(args.iter().any(|a| a == "--ver"))
    };
    std::process::exit(codigo);
}

// Pruebas chiquitas para ir viendo si vas bien: `cargo test -p vortex-memoria --offline` (ve la GUIA).
#[cfg(test)]
mod pruebas {
    use super::*;

    fn app(nombre: &str, kb: u64) -> App {
        App {
            nombre: nombre.to_string(),
            kb,
            ..App::default()
        }
    }

    #[test]
    fn paso_1_gb() {
        assert_eq!(gb(1048576), "1.0 GB");
        assert_eq!(gb(1572864), "1.5 GB");
        assert_eq!(gb(104858), "0.1 GB");
        assert_eq!(gb(102400), "100 MB");
        assert_eq!(gb(0), "0 MB");
    }

    #[test]
    fn paso_2_leer_clave() {
        let meminfo = "MemTotal:       8000000 kB\nMemFree:  10 kB\nMemAvailable:    600000 kB\n";
        assert_eq!(leer_clave(meminfo, "MemTotal"), 8000000);
        assert_eq!(leer_clave(meminfo, "MemAvailable"), 600000);
        assert_eq!(leer_clave(meminfo, "NoExiste"), 0);
        assert_eq!(leer_clave(meminfo, "Mem"), 0, "«Mem» no es «MemTotal»");
        let status = "Name:\tx\nRssAnon:\t1000 kB\nRssShmem:\t50 kB\n";
        assert_eq!(leer_clave(status, "RssAnon"), 1000);
        assert_eq!(leer_clave("", "RssAnon"), 0);
    }

    #[test]
    fn paso_3_justa_para_jugar() {
        let g = 1048576;
        assert!(justa_para_jugar(g, 16 * g), "1 GB libre de 16: justo");
        assert!(!justa_para_jugar(4 * g, 16 * g), "4 GB libres: alcanza");
        assert!(!justa_para_jugar(0, 16 * g), "0 es «no sé», no «nada»");
        assert!(justa_para_jugar(g * 3 / 2, 4 * g), "1.5 de 4 GB: menos del 40 %");
        assert!(!justa_para_jugar(2 * g, 4 * g), "2 de 4 GB: alcanza");
    }

    #[test]
    fn paso_4_juntar() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        assert_eq!(juntar(&n(&["Discord"])), "Discord");
        assert_eq!(juntar(&n(&["Discord", "Chrome"])), "Discord y Chrome");
        assert_eq!(
            juntar(&n(&["Discord", "Chrome", "Spotify"])),
            "Discord, Chrome y Spotify"
        );
    }

    #[test]
    fn paso_5_textos() {
        let discord = app("Discord", 1572864);
        assert_eq!(
            texto_aviso(Some(&discord), 512000),
            "Quedan 0.5 GB. Discord es lo que más usa: 1.5 GB."
        );
        assert_eq!(
            texto_aviso(None, 204800),
            "Quedan 0.2 GB. Mira qué la está usando antes de que algo se cierre solo."
        );
        let apps = [app("Discord", 1048576), app("Chrome", 524288)];
        assert_eq!(
            texto_juego("Elden Ring", &apps, 1048576),
            "Cierra Discord y Chrome y Elden Ring tendrá unos 1.5 GB más (ahora le quedan 1.0 GB). \
             Así va con menos tirones."
        );
    }
}
