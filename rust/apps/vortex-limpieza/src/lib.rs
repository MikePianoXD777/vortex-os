//! vortex-limpieza (4.0): la limpieza de la semana, sola. Un temporizador de systemd la corre una vez por
//! semana (entre una vez y otra no queda nada corriendo) y avisa «Liberé 2,3 GB» si liberó algo que valga.
//!
//! Lo que borra se rehace solo y nada es tuyo: miniaturas, cachés de yay/paru (lo que bajan para compilar
//! del AUR), de pip y de npm, siempre lo que lleva más de 30 días sin tocarse; y las apps de Flatpak de tu
//! usuario que ya nadie usa. La papelera es tuya: si lo que tiene de hace más de un mes pesa, **pregunta**
//! con un aviso antes de vaciarlo. La caché de pacman la limpia `paccache.timer` (con root, lo enciende
//! `vortex actualizar`). Las cachés de sombreadores no se tocan: rehacerlas da tirones en los juegos.
//!
//!   vortex-limpieza semanal    limpia y avisa (lo corre el temporizador)
//!   vortex-limpieza ver        solo dice lo que borraría

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const APP: &str = "Vortexine's OS";
/// Lo que no se ha tocado en este tiempo ya se puede borrar.
pub const DIAS: u64 = 30;
/// Menos que esto no merece un aviso.
pub const AVISAR_DESDE: u64 = 50 * 1024 * 1024;

/// Cómo se borra una caché: archivo por archivo, o cada carpeta de primer nivel entera (yay y paru guardan
/// un repositorio de git por paquete: borrarle archivos sueltos lo rompería).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Archivos,
    Carpetas,
}

/// Las cachés que se rehacen solas.
pub fn caches(home: &Path, cache: &Path) -> Vec<(PathBuf, Modo)> {
    vec![
        (cache.join("thumbnails"), Modo::Archivos),
        (cache.join("pip"), Modo::Archivos),
        (home.join(".npm/_cacache"), Modo::Archivos),
        (cache.join("yay"), Modo::Carpetas),
        (cache.join("paru/clone"), Modo::Carpetas),
    ]
}

/// Lo borrable de una carpeta: los archivos sin tocar desde `antes` (sin seguir enlaces) y cuánto pesan.
pub fn viejos(dir: &Path, antes: SystemTime) -> (Vec<PathBuf>, u64) {
    let mut lista = Vec::new();
    let mut total = 0;
    let mut pila = vec![dir.to_path_buf()];
    while let Some(d) = pila.pop() {
        let Ok(entradas) = fs::read_dir(&d) else { continue };
        for e in entradas.flatten() {
            let Ok(m) = e.metadata() else { continue }; // no sigue enlaces
            if m.is_dir() {
                pila.push(e.path());
            } else if m.modified().is_ok_and(|t| t < antes) {
                total += m.len();
                lista.push(e.path());
            }
        }
    }
    (lista, total)
}

/// Lo más nuevo de una carpeta: su archivo tocado más recientemente (o ella misma, si está vacía). Las
/// carpetas de adentro no cuentan: su fecha cambia con solo crear algo en ellas.
fn lo_mas_nuevo(dir: &Path) -> SystemTime {
    let mut mas = None;
    let mut pila = vec![dir.to_path_buf()];
    while let Some(d) = pila.pop() {
        let Ok(entradas) = fs::read_dir(&d) else { continue };
        for e in entradas.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                pila.push(e.path());
            } else if let Ok(t) = m.modified() {
                mas = Some(mas.map_or(t, |x: SystemTime| x.max(t)));
            }
        }
    }
    mas.unwrap_or_else(|| {
        fs::symlink_metadata(dir)
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH)
    })
}

/// Las carpetas de primer nivel sin tocar desde `antes` (por dentro tampoco) y cuánto pesan.
pub fn carpetas_viejas(dir: &Path, antes: SystemTime) -> (Vec<PathBuf>, u64) {
    let mut lista = Vec::new();
    let mut total = 0;
    let Ok(entradas) = fs::read_dir(dir) else {
        return (lista, 0);
    };
    for e in entradas.flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) && lo_mas_nuevo(&e.path()) < antes {
            total += peso(&e.path());
            lista.push(e.path());
        }
    }
    (lista, total)
}

/// Borra carpetas enteras. Devuelve lo liberado.
pub fn borrar_carpetas(lista: &[PathBuf]) -> u64 {
    lista
        .iter()
        .filter_map(|c| {
            let n = peso(c);
            fs::remove_dir_all(c).is_ok().then_some(n)
        })
        .sum()
}

/// Borra los archivos y luego las carpetas que quedaron vacías (menos `raiz`). Devuelve lo liberado.
pub fn borrar(lista: &[PathBuf], raiz: &Path) -> u64 {
    let mut libre = 0;
    for a in lista {
        let n = fs::symlink_metadata(a).map(|m| m.len()).unwrap_or(0);
        if fs::remove_file(a).is_ok() {
            libre += n;
        }
    }
    quitar_vacias(raiz, raiz);
    libre
}

fn quitar_vacias(d: &Path, raiz: &Path) {
    if let Ok(entradas) = fs::read_dir(d) {
        for e in entradas.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                quitar_vacias(&e.path(), raiz);
            }
        }
    }
    if d != raiz {
        let _ = fs::remove_dir(d); // solo si está vacía
    }
}

/// Lo que pesa una carpeta (sin seguir enlaces).
pub fn peso(dir: &Path) -> u64 {
    let mut total = 0;
    let mut pila = vec![dir.to_path_buf()];
    while let Some(d) = pila.pop() {
        let Ok(m) = fs::symlink_metadata(&d) else { continue };
        if m.is_dir() {
            if let Ok(entradas) = fs::read_dir(&d) {
                pila.extend(entradas.flatten().map(|e| e.path()));
            }
        } else {
            total += m.len();
        }
    }
    total
}

/// «2,3 GB», «340 MB».
pub fn tamano(b: u64) -> String {
    let t = if b >= 1_000_000_000 {
        format!("{:.1} GB", b as f64 / 1e9)
    } else {
        format!("{} MB", b / 1_000_000)
    };
    t.replace('.', ",")
}

// ====================================================================== la papelera

/// Los días desde 1970 de una fecha (el algoritmo de Howard Hinnant, sin bibliotecas de fechas).
pub fn dias_desde_1970(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let ae = a - era * 400;
    let mp = (m + 9) % 12;
    let dd = (153 * mp + 2) / 5 + d - 1;
    let de = ae * 365 + ae / 4 - ae / 100 + dd;
    era * 146_097 + de - 719_468
}

/// El día de `DeletionDate=2026-10-01T12:00:00` de un `.trashinfo`.
pub fn dia_borrado(trashinfo: &str) -> Option<i64> {
    let f = trashinfo
        .lines()
        .find_map(|l| l.trim().strip_prefix("DeletionDate="))?;
    let mut p = f.get(..10)?.split('-').map(|x| x.parse::<i64>().ok());
    Some(dias_desde_1970(p.next()??, p.next()??, p.next()??))
}

/// Lo de la papelera borrado hace más de `DIAS`: (el archivo, su .trashinfo) y cuánto pesa todo.
pub fn papelera_vieja(papelera: &Path, hoy: i64) -> (Vec<(PathBuf, PathBuf)>, u64) {
    let mut lista = Vec::new();
    let mut total = 0;
    let Ok(infos) = fs::read_dir(papelera.join("info")) else {
        return (lista, 0);
    };
    for e in infos.flatten() {
        let info = e.path();
        if info.extension().is_none_or(|x| x != "trashinfo") {
            continue;
        }
        let Some(dia) = fs::read_to_string(&info).ok().as_deref().and_then(dia_borrado) else {
            continue;
        };
        if hoy - dia <= DIAS as i64 {
            continue;
        }
        let Some(nombre) = info.file_stem() else { continue };
        let archivo = papelera.join("files").join(nombre);
        total += peso(&archivo);
        lista.push((archivo, info));
    }
    (lista, total)
}

pub fn vaciar(lista: &[(PathBuf, PathBuf)]) {
    for (archivo, info) in lista {
        let bien = match fs::symlink_metadata(archivo) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(archivo).is_ok(),
            Ok(_) => fs::remove_file(archivo).is_ok(),
            Err(_) => true, // ya no estaba
        };
        if bien {
            let _ = fs::remove_file(info);
        }
    }
}

// ====================================================================== correr

/// `~/.cache` a partir de `~/.cache/vortex` (y lo mismo con `~/.local/share`).
fn xdg(de_vortex: PathBuf) -> PathBuf {
    de_vortex.parent().map(Path::to_path_buf).unwrap_or(de_vortex)
}

fn hoy() -> i64 {
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400) as i64
}

fn hay(programa: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(programa).is_file()))
}

/// Las apps de Flatpak de tu usuario que nadie usa (runtimes de apps que ya quitaste). Lo liberado.
fn flatpak_sin_usar(home: &Path) -> u64 {
    let dir = home.join(".local/share/flatpak");
    if !hay("flatpak") || !dir.is_dir() {
        return 0;
    }
    let antes = peso(&dir);
    let _ = Command::new("flatpak")
        .args(["uninstall", "--user", "--unused", "-y", "--noninteractive"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    antes.saturating_sub(peso(&dir))
}

fn avisar(titulo: &str, cuerpo: &str, acciones: &[&str]) -> String {
    let mut c = Command::new("notify-send");
    c.args(["-a", APP, "-i", "edit-clear-all"]);
    for a in acciones {
        c.args(["-A", a]);
    }
    c.args([titulo, cuerpo]);
    c.stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn principal(args: &[String]) -> i32 {
    let solo_ver = match args.first().map(String::as_str) {
        Some("semanal") => false,
        Some("ver") => true,
        _ => {
            eprintln!("uso: vortex-limpieza semanal | ver");
            return 2;
        }
    };
    let home = vortex_core::rutas::home();
    // Las de vortex-core terminan en «/vortex»: aquí hacen falta las carpetas XDG de arriba.
    let cache = xdg(vortex_core::rutas::cache());
    let antes = SystemTime::now() - Duration::from_secs(DIAS * 86_400);

    let mut libre = 0;
    for (c, modo) in caches(&home, &cache) {
        let (lista, total) = match modo {
            Modo::Archivos => viejos(&c, antes),
            Modo::Carpetas => carpetas_viejas(&c, antes),
        };
        if lista.is_empty() {
            continue;
        }
        if solo_ver {
            println!("{}: {} cosas, {}", c.display(), lista.len(), tamano(total));
        } else if modo == Modo::Archivos {
            libre += borrar(&lista, &c);
        } else {
            libre += borrar_carpetas(&lista);
        }
    }
    let papelera = xdg(vortex_core::rutas::datos()).join("Trash");
    let (vieja, peso_vieja) = papelera_vieja(&papelera, hoy());
    if solo_ver {
        println!(
            "Papelera de hace más de {DIAS} días: {} cosas, {}",
            vieja.len(),
            tamano(peso_vieja)
        );
        return 0;
    }
    let flatpak = flatpak_sin_usar(&home);
    libre += flatpak;

    if libre >= AVISAR_DESDE {
        let que = if flatpak >= AVISAR_DESDE {
            "cachés viejas y apps de Flatpak que nadie usaba"
        } else {
            "cachés viejas"
        };
        avisar(
            "Limpieza de la semana",
            &format!("Liberé {} ({que}). Nada tuyo se tocó.", tamano(libre)),
            &[],
        );
    }
    // La papelera es tuya: se pregunta.
    if peso_vieja >= AVISAR_DESDE {
        let r = avisar(
            "¿Vaciar la papelera vieja?",
            &format!(
                "Tienes {} en la papelera de cosas que borraste hace más de un mes.",
                tamano(peso_vieja)
            ),
            &["vaciar=Vaciar", "no=Ahora no"],
        );
        if r == "vaciar" {
            vaciar(&vieja);
            avisar(
                "Papelera vaciada",
                &format!("Liberé {}.", tamano(peso_vieja)),
                &[],
            );
        }
    }
    0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn fechas() {
        assert_eq!(dias_desde_1970(1970, 1, 1), 0);
        assert_eq!(dias_desde_1970(2000, 3, 1), 11_017);
        assert_eq!(dias_desde_1970(2026, 11, 19), 20_776);
        assert_eq!(
            dia_borrado("[Trash Info]\nPath=/home/a/x\nDeletionDate=2026-11-19T08:30:00\n"),
            Some(20_776)
        );
        assert_eq!(dia_borrado("[Trash Info]\nPath=/x\n"), None);
        assert_eq!(dia_borrado("DeletionDate=basura"), None);
    }

    #[test]
    fn tamanos() {
        assert_eq!(tamano(2_345_000_000), "2,3 GB");
        assert_eq!(tamano(340_000_000), "340 MB");
        assert_eq!(tamano(0), "0 MB");
    }
}
