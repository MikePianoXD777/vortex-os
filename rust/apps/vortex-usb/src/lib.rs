//! vortex-usb (Vortex 1.8, en Rust desde la 4.0): al conectar una memoria USB o una tarjeta SD, la monta y avisa
//! con dos botones, como en Windows: «Abrir» (en Archivos) y «Expulsar» (la desmonta y la apaga: ya se puede
//! sacar sin perder nada). Escucha a udev (por netlink, como libudev; si no se puede, `udevadm monitor`) y monta con udisks (`udisksctl`, sin root: polkit
//! se lo deja a la sesión activa). Lo arranca hyprland.lua al entrar; hay una sola copia por sesión.
//!
//! Es la misma herramienta que `bin/vortex-usb` (bash) y llama a los mismos programas (udevadm, udisksctl,
//! findmnt, lsblk, notify-send, xdg-open), pero dormida gasta ~1 MB en vez de dos procesos de bash de ~6 MB.
//!
//!   vortex-usb-rs             escucha (se queda corriendo)
//!   vortex-usb-rs --eventos   lee los eventos de udev de la entrada estándar (lo usan las pruebas)

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;

mod netlink;

const APP: &str = "Vortexine's OS";

/// Un evento de udev ya leído: sus líneas `CLAVE=valor`.
pub type Evento = HashMap<String, String>;

// ====================================================================== los programas de afuera

/// Un programa sin entrada estándar (los de bash corren con `</dev/null`).
fn programa(nombre: &str) -> Command {
    let mut c = Command::new(nombre);
    c.stdin(Stdio::null());
    c
}

/// La salida estándar del programa (sin los saltos de línea del final, como `$(...)`), o `None` si no corrió.
fn salida_de(mut c: Command) -> Option<(bool, String)> {
    let o = c.stderr(Stdio::null()).output().ok()?;
    let s = String::from_utf8_lossy(&o.stdout);
    Some((o.status.success(), s.trim_end_matches('\n').to_string()))
}

/// La salida y los errores juntos (como `2>&1`) y si salió bien.
fn salida_y_errores(mut c: Command) -> Option<(bool, String)> {
    let (mut lector, escritor) = std::io::pipe().ok()?;
    c.stdout(escritor.try_clone().ok()?).stderr(escritor);
    let mut hijo = c.spawn().ok()?;
    drop(c); // cierra en este proceso las puntas de escritura
    let mut bytes = Vec::new();
    let _ = lector.read_to_end(&mut bytes);
    let bien = hijo.wait().ok()?.success();
    Some((
        bien,
        String::from_utf8_lossy(&bytes).trim_end_matches('\n').to_string(),
    ))
}

fn sin_salida(mut c: Command) -> bool {
    c.stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Avisa (con la salida estándar a donde estuviera, como el `notify-send` de bash).
fn avisar(titulo: &str, cuerpo: Option<&str>) {
    let mut c = programa("notify-send");
    c.args(["-a", APP, "-i", "drive-removable-media", titulo]);
    if let Some(cuerpo) = cuerpo {
        c.arg(cuerpo);
    }
    let _ = c.stderr(Stdio::null()).status();
}

/// «15,9 GB» a partir del tamaño en sectores de 512 bytes que da /sys.
pub fn tamano_de_sectores(sectores: u64) -> String {
    let b = sectores as f64 * 512.0;
    let t = if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else {
        format!("{:.0} MB", b / 1e6)
    };
    t.replace('.', ",")
}

fn tamano(dispositivo: &str) -> String {
    let nombre = Path::new(dispositivo)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    std::fs::read_to_string(format!("/sys/class/block/{nombre}/size"))
        .ok()
        .and_then(|s| {
            let s = s.trim_end_matches('\n');
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.to_string())
        })
        .and_then(|s| s.parse::<u64>().ok())
        .map(tamano_de_sectores)
        .unwrap_or_default()
}

// ====================================================================== expulsar y montar

/// Desmonta todas las particiones montadas del disco de `dispositivo` y lo apaga.
fn expulsar(dispositivo: &str, nombre: &str) {
    let mut c = programa("lsblk");
    c.args(["-dno", "PKNAME", dispositivo]);
    let disco = salida_de(c)
        .and_then(|(_, s)| s.lines().next().map(String::from))
        .filter(|d| !d.is_empty())
        .map_or_else(|| dispositivo.to_string(), |d| format!("/dev/{d}"));
    let mut c = programa("lsblk");
    c.args(["-lnpo", "NAME,MOUNTPOINT", &disco]);
    let montadas: Vec<String> = salida_de(c)
        .map(|(_, s)| s)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.split_whitespace().count() > 1)
        .filter_map(|l| l.split_whitespace().next().map(String::from))
        .collect();
    for particion in montadas {
        let mut c = programa("udisksctl");
        c.args(["unmount", "-b", &particion, "--no-user-interaction"]);
        if !sin_salida(c) {
            avisar(
                &format!("No se puede expulsar «{nombre}»"),
                Some(
                    "Algo la está usando: cierra los archivos o ventanas que tengan algo de ella y vuelve a intentar.",
                ),
            );
            return;
        }
    }
    // Apagarla no siempre se puede (lectores de tarjetas, algunas memorias): desmontada ya es segura.
    let mut c = programa("udisksctl");
    c.args(["power-off", "-b", &disco, "--no-user-interaction"]);
    let _ = sin_salida(c);
    avisar(&format!("Ya puedes sacar «{nombre}»"), None);
}

/// Monta `dispositivo` (si no lo montó ya alguien) y avisa con los botones.
fn montar_y_avisar(dispositivo: &str, etiqueta: &str, modelo: &str) {
    let mut nombre = if etiqueta.is_empty() {
        modelo.replace('_', " ")
    } else {
        etiqueta.to_string()
    };
    if nombre.is_empty() {
        nombre = "la memoria".into();
    }
    let mut punto = String::new();
    let mut salida = String::new();
    // udisks se entera del dispositivo un momento después que udev: se reintenta unos segundos.
    for _ in 0..5 {
        let mut c = programa("findmnt");
        c.args(["-nro", "TARGET", "--source", dispositivo]);
        punto = salida_de(c)
            .and_then(|(_, s)| s.lines().next().map(String::from))
            .unwrap_or_default();
        if !punto.is_empty() {
            break;
        }
        let mut c = programa("udisksctl");
        c.args(["mount", "-b", dispositivo, "--no-user-interaction"]);
        if let Some((bien, s)) = salida_y_errores(c) {
            salida = s;
            if bien {
                // «Mounted /dev/sdb1 at /run/media/mike/MEMORIA» (algunas versiones terminan con punto).
                let p = salida.rfind(" at ").map_or(salida.as_str(), |i| &salida[i + 4..]);
                punto = p.strip_suffix('.').unwrap_or(p).to_string();
                break;
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    if punto.is_empty() {
        let motivo = if salida.is_empty() {
            "udisks no la reconoce"
        } else {
            salida.as_str()
        };
        avisar(&format!("No pude abrir «{nombre}»"), Some(motivo));
        return;
    }
    let mut c = programa("notify-send");
    c.args([
        "-a",
        APP,
        "-i",
        "drive-removable-media",
        "-A",
        "abrir=Abrir",
        "-A",
        "expulsar=Expulsar",
    ])
    .arg("Memoria conectada")
    .arg(format!("«{nombre}» ({})", tamano(dispositivo)));
    let accion = salida_de(c)
        .filter(|(bien, _)| *bien)
        .map(|(_, s)| s)
        .unwrap_or_default();
    match accion.as_str() {
        "abrir" => {
            let mut c = programa("xdg-open");
            c.arg(&punto);
            if !sin_salida(c) {
                let mut c = programa("thunar");
                c.arg(&punto);
                let _ = sin_salida(c);
            }
        }
        "expulsar" => expulsar(dispositivo, &nombre),
        _ => {}
    }
}

// ====================================================================== los eventos de udev

/// ¿Es un sistema de archivos que se acaba de conectar por USB o en un lector de tarjetas? Los discos
/// internos (SATA, NVMe) no.
pub fn debe_montar(e: &Evento) -> bool {
    let v = |k: &str| e.get(k).map_or("", String::as_str);
    v("ACTION") == "add"
        && v("SUBSYSTEM") == "block"
        && v("ID_FS_USAGE") == "filesystem"
        && v("UDISKS_IGNORE") != "1"
        && (v("ID_BUS") == "usb" || v("ID_PATH").contains("usb") || v("DEVNAME").starts_with("/dev/mmcblk"))
        && !v("DEVNAME").is_empty()
}

/// Una línea `CLAVE=valor` de `udevadm monitor --property`: se ignoran las que no lo son (la del encabezado
/// «UDEV [..] add /devices/.. (block)» y las que traen un espacio antes de un «=»).
fn par(linea: &str) -> Option<(&str, &str)> {
    let (k, v) = linea.split_once('=')?;
    if k.is_empty() {
        return None;
    }
    match linea.find(' ') {
        Some(i) if linea[i..].contains('=') => None,
        _ => Some((k, v)),
    }
}

/// Monta lo que toque de un evento (en su propio hilo, para no frenar a los demás).
fn atender(p: &Evento, hilos: &mut Vec<JoinHandle<()>>) {
    if debe_montar(p) {
        let (d, e, m) = (
            p["DEVNAME"].clone(),
            p.get("ID_FS_LABEL").cloned().unwrap_or_default(),
            p.get("ID_MODEL").cloned().unwrap_or_default(),
        );
        hilos.retain(|h| !h.is_finished());
        hilos.push(thread::spawn(move || montar_y_avisar(&d, &e, &m)));
    }
}

/// Lee bloques «CLAVE=valor» separados por una línea vacía y monta lo que toque (cada uno en su hilo, para
/// no frenar a los demás). Al llegar al final espera a que terminen todos.
pub fn leer_eventos(entrada: impl Read) {
    let mut lector = BufReader::with_capacity(4096, entrada);
    let mut hilos: Vec<JoinHandle<()>> = Vec::new();
    let mut p = Evento::new();
    let mut linea = Vec::new();
    let manejar = |p: &mut Evento, hilos: &mut Vec<JoinHandle<()>>| {
        atender(p, hilos);
        p.clear();
    };
    loop {
        linea.clear();
        match lector.read_until(b'\n', &mut linea) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let texto = String::from_utf8_lossy(&linea);
        let texto = texto.trim_end_matches('\n');
        if texto.is_empty() {
            manejar(&mut p, &mut hilos);
        } else if let Some((k, v)) = par(texto) {
            p.insert(k.to_string(), v.to_string());
        }
    }
    manejar(&mut p, &mut hilos);
    for h in hilos {
        let _ = h.join();
    }
}

fn en_path(programa: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|d| {
            std::fs::metadata(d.join(programa))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

fn escuchar() -> u8 {
    if !en_path("udisksctl") {
        return 0;
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| "/tmp".into(), std::path::PathBuf::from);
    let candado = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dir.join("vortex-usb.lock"));
    // Una sola copia por sesión (el candado se suelta solo al terminar el proceso).
    let Ok(candado) = candado else { return 0 };
    if candado.try_lock().is_err() {
        return 0;
    }
    // 4.0: directo de udev, sin un `udevadm monitor` de ~5 MB al lado. Si no se puede, como antes.
    if let Ok(u) = netlink::Udev::abrir() {
        let mut buf = vec![0u8; 8192];
        let mut hilos: Vec<JoinHandle<()>> = Vec::new();
        loop {
            match u.recibir(&mut buf) {
                Ok(Some(e)) => atender(&e, &mut hilos),
                Ok(None) => {}
                Err(_) => break,
            }
        }
    }
    if !en_path("udevadm") {
        return 0;
    }
    let Ok(mut udev) = Command::new("udevadm")
        .args(["monitor", "--udev", "--subsystem-match=block", "--property"])
        .stdout(Stdio::piped())
        .spawn()
    else {
        return 0;
    };
    if let Some(salida) = udev.stdout.take() {
        leer_eventos(salida);
    }
    let _ = udev.wait();
    drop(candado);
    0
}

/// Corre la orden y devuelve el código de salida.
pub fn principal(args: &[String]) -> u8 {
    if args.first().map(String::as_str) == Some("--eventos") {
        leer_eventos(std::io::stdin());
        return 0;
    }
    escuchar()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn evento(lineas: &[(&str, &str)]) -> Evento {
        lineas
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn tamanos_como_el_de_bash() {
        assert_eq!(tamano_de_sectores(31_116_288), "15,9 GB");
        assert_eq!(tamano_de_sectores(1_953_125), "1,0 GB");
        assert_eq!(tamano_de_sectores(1_000_000), "512 MB");
        assert_eq!(tamano_de_sectores(0), "0 MB");
    }

    #[test]
    fn solo_memorias_y_tarjetas() {
        let base = [
            ("ACTION", "add"),
            ("SUBSYSTEM", "block"),
            ("DEVNAME", "/dev/sdb1"),
            ("ID_FS_USAGE", "filesystem"),
        ];
        let con = |extra: &[(&str, &str)]| {
            let mut l = base.to_vec();
            l.extend_from_slice(extra);
            evento(&l)
        };
        assert!(debe_montar(&con(&[("ID_BUS", "usb")])));
        assert!(debe_montar(&con(&[(
            "ID_PATH",
            "pci-0000:00:14.0-usb-0:2:1.0-scsi-0:0:0:0"
        )])));
        assert!(!debe_montar(&con(&[("ID_BUS", "ata")])));
        assert!(!debe_montar(&con(&[("ID_BUS", "usb"), ("UDISKS_IGNORE", "1")])));
        assert!(!debe_montar(&con(&[("ID_BUS", "usb"), ("ACTION", "remove")])));
        let mut sd = con(&[]);
        sd.insert("DEVNAME".into(), "/dev/mmcblk0p1".into());
        assert!(debe_montar(&sd));
    }

    #[test]
    fn lineas_de_udev() {
        assert_eq!(par("ID_FS_LABEL=MI USB"), Some(("ID_FS_LABEL", "MI USB")));
        assert_eq!(
            par("ID_PATH=pci-0000:00:14.0-usb-0:2:1.0-scsi-0:0:0:0"),
            Some(("ID_PATH", "pci-0000:00:14.0-usb-0:2:1.0-scsi-0:0:0:0"))
        );
        assert_eq!(par("UDEV  [1.0] add /devices/x (block)"), None);
        assert_eq!(par("sin igual"), None);
        assert_eq!(par("DEVLINKS=/dev/disk/by-id/a b=c"), None);
        assert_eq!(par("A="), Some(("A", "")));
    }
}
