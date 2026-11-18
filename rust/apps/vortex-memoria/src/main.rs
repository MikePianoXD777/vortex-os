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

// Mientras haya huecos sin llenar, Rust se queja de cosas sin usar. Cuando termines, BORRA esta línea.
#![allow(unused)]

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

// ---------------------------------------------------------------------------------------------------
// Tus pasos.

/// PASO 1. Los KB en texto: «1.5 GB» si son 104858 KB o más, si no «300 MB». Python: `gb()`.
/// Pista: `format!("{:.1} GB", kb as f64 / 1048576.0)`.
fn gb(kb: u64) -> String {
    todo!("paso 1")
}

/// PASO 2. El número que va después de «Clave:» en un texto como el de /proc/meminfo o /proc/PID/status
/// («MemAvailable:    600000 kB» → 600000), o 0 si no está. Python lo hace dentro de `disponible_kb()`,
/// `total_kb()` y `uso_kb()`; aquí es UNA función que sirve para las tres (la vas a usar otra vez en el
/// paso 7). Pista: `.lines()`, `linea.strip_prefix(clave)` y luego `.strip_prefix(':')`,
/// `.split_whitespace().next()` y `.parse::<u64>()`.
fn leer_clave(texto: &str, clave: &str) -> u64 {
    todo!("paso 2")
}

/// Un número de /proc/meminfo (ya hecho: usa tu paso 2).
fn meminfo(clave: &str) -> u64 {
    let texto = std::fs::read_to_string(proc_dir().join("meminfo")).unwrap_or_default();
    leer_clave(&texto, clave)
}

/// PASO 3. ¿Le queda poca memoria al juego? Python: `justa_para_jugar()`. Una línea.
/// Pista: `(total as f64 * JUEGO_PARTE) as u64` y `JUEGO_MAX_KB.min(...)`.
fn justa_para_jugar(queda: u64, total: u64) -> bool {
    todo!("paso 3")
}

/// PASO 4. «Discord», «Discord y Chrome», «Discord, Chrome y Spotify». Python: `juntar()`.
/// Pista: `nombres[..nombres.len() - 1].join(", ")` y `nombres.last()`.
fn juntar(nombres: &[String]) -> String {
    todo!("paso 4")
}

/// PASO 5. Los dos textos de los avisos. Python: `texto_aviso()` y `texto_juego()`.
/// `Option<&App>` es «una app, o ninguna»: se abre con `match app { Some(a) => ..., None => ... }`.
fn texto_aviso(app: Option<&App>, queda: u64) -> String {
    todo!("paso 5a")
}

fn texto_juego(juego: &str, apps: &[App], queda: u64) -> String {
    // Pista para la suma: `apps.iter().map(|a| a.kb).sum::<u64>()`.
    // Y para los nombres: `apps.iter().map(|a| a.nombre.clone()).collect::<Vec<String>>()`.
    todo!("paso 5b")
}

/// PASO 6. El cgroup de un proceso: la última parte de la línea de `/proc/PID/cgroup` si es
/// «app-….scope»; si no, «pid:PID». Python: `grupo_de()`.
/// Pista: `.trim().rsplit('/').next()` da lo que va después de la última `/`.
fn grupo_de(pid: u32) -> String {
    todo!("paso 6")
}

/// PASO 7. Lo que de verdad usa un proceso: suma de RssAnon, RssShmem y VmSwap de `/proc/PID/status`.
/// Python: `uso_kb()`. Lee el archivo una vez y usa tu `leer_clave` tres veces.
fn uso_kb(pid: u32) -> u64 {
    todo!("paso 7")
}

/// PASO 8. Cuánto usa cada grupo, sumando sus procesos (solo los tuyos). Python: `usos_por_grupo()`.
/// Pista: `std::fs::read_dir(proc_dir())` da las entradas; `entrada.file_name().to_str()` el nombre; si
/// `nombre.parse::<u32>()` sale bien es un proceso. El dueño: `entrada.metadata().map(|m| m.uid()).ok()`, que da `Some(uid)` (con
/// `use std::os::unix::fs::MetadataExt;`). Para sumar en el mapa:
/// `*usos.entry(grupo).or_insert(0) += uso_kb(pid);`
fn usos_por_grupo() -> HashMap<String, u64> {
    todo!("paso 8")
}

/// PASO 9. Las ventanas abiertas (`hyprctl clients -j`), sin las de pid ≤ 1 ni las que no están
/// `mapped`. Python: `ventanas()`. Cada ventana es un `Value` de serde_json: `v["pid"].as_u64()`,
/// `v["class"].as_str()`, `v["mapped"].as_bool().unwrap_or(true)`.
fn ventanas() -> Vec<Value> {
    todo!("paso 9")
}

/// PASO 10. El nombre de la app como sale en el lanzador (`com.discordapp.Discord` → «Discord»),
/// buscando en los .desktop. Python: `nombre_de()`. Es el más largo de leer, pero son puros `for`.
/// Pista para la última línea (cuando no lo encuentra): `clase.rsplit('.').next()`, `.replace('-', " ")`
/// y para la mayúscula: la primera letra con `.to_uppercase()` y el resto con `.to_lowercase()`.
fn nombre_de(clase: &str) -> String {
    todo!("paso 10")
}

/// PASO 11. Las apps con ventana, agrupadas por cgroup, en el orden en que salen sus ventanas.
/// Python: `apps_con_ventana()`. Aquí devuelve una lista (`Vec<App>`) en vez de un diccionario: busca
/// si ya está su grupo con `apps.iter_mut().find(|a| a.grupo == grupo)`; si sí, le agregas la dirección;
/// si no, haces `apps.push(App { ... })`.
fn apps_con_ventana() -> Vec<App> {
    todo!("paso 11")
}

/// PASO 12a. La app con ventana que más usa (si usa al menos MINIMO_KB). Python: `la_que_mas_usa()`.
/// OJO: si dos empatan, Python se queda con la PRIMERA y el `max_by_key` de Rust con la ÚLTIMA. Hazlo con
/// un `for` que solo cambie de app cuando la nueva use MÁS (no igual).
fn la_que_mas_usa() -> Option<App> {
    todo!("paso 12a")
}

/// PASO 12b. Las que más gastan al jugar, sin el juego ni los lanzadores, hasta 3. Python:
/// `para_cerrar_al_jugar()`. Para ordenar de mayor a menor: `elegidas.sort_by(|a, b| b.kb.cmp(&a.kb));`
/// y para quedarte con 3: `elegidas.truncate(JUEGO_APPS);`. La regex: `regex::Regex::new(LANZADORES)`.
fn para_cerrar_al_jugar(clase_juego: &str, pid_juego: u32) -> Vec<App> {
    todo!("paso 12b")
}

/// PASO 13. El nombre del juego. Python: `nombre_juego()`.
fn nombre_juego(clase: &str, pid: u32) -> String {
    todo!("paso 13")
}

/// PASO 14. Le manda órdenes a Hyprland: la primera que entienda. Python: `despachar()` y `cerrar()`.
/// (Hyprland 0.57 ya no entiende la sintaxis vieja; por eso se prueba primero la de Lua: `hl.dsp...`.)
fn despachar(ordenes: &[String]) {
    todo!("paso 14a")
}

fn cerrar(apps: &[App]) {
    todo!("paso 14b")
}

/// PASO 15. El aviso al abrir un juego. Python: `al_jugar()`. Devuelve el código de salida (0).
/// Para esperar: `std::thread::sleep(std::time::Duration::from_secs_f64(espera_s()))`.
fn al_jugar(args: &[String]) -> i32 {
    todo!("paso 15")
}

/// PASO 16. El aviso normal (lo que hace `main()` en Python después del `--juego`).
fn avisar(ver: bool) -> i32 {
    todo!("paso 16")
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
