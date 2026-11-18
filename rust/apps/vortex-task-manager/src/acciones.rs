//! Acciones sobre procesos y ventanas: finalizar, suspender, cambiar prioridad, cerrar ventanas.
//! Las de procesos ajenos (root u otro usuario) las autoriza polkit con `pkexec`.

use crate::ejecutar;
use crate::grupos::Ventana;
use crate::sys;
use serde_json::Value;
use std::time::Duration;

pub const SIGKILL: i32 = libc::SIGKILL;
pub const SIGTERM: i32 = libc::SIGTERM;
pub const SIGSTOP: i32 = libc::SIGSTOP;
pub const SIGCONT: i32 = libc::SIGCONT;

pub const PRIORIDADES: [(&str, i32); 4] = [("alta", -5), ("normal", 0), ("baja", 10), ("minima", 19)];

/// Para procesos de root u otros usuarios: lo pide polkit (te pregunta tu contraseña).
fn con_pkexec(argv: &[String]) -> Option<String> {
    if !ejecutar::existe("pkexec") {
        return Some("Ese proceso es del sistema y aquí no hay pkexec para pedir permiso.".into());
    }
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    // Sin límite: la persona tarda en escribir su contraseña (se espera un día entero como mucho).
    match ejecutar::con_limite("pkexec", &args, Duration::from_secs(86_400)) {
        Some(r) if r.codigo == 126 || r.codigo == 127 => Some("No se dio el permiso.".into()),
        Some(r) if r.codigo == 0 => None,
        Some(r) => Some(
            Some(r.stderr.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "No se pudo.".into()),
        ),
        None => Some("No se pudo.".into()),
    }
}

fn nombre_senal(sig: i32) -> &'static str {
    match sig {
        SIGKILL => "KILL",
        SIGTERM => "TERM",
        SIGSTOP => "STOP",
        _ => "CONT",
    }
}

/// Manda la señal a todos (los hijos primero). Si alguno no es tuyo, lo pide con pkexec. `None` = bien.
pub fn senal(pids: &[i32], sig: i32) -> Option<String> {
    let mut ordenados = pids.to_vec();
    ordenados.sort_unstable_by(|a, b| b.cmp(a));
    let mut ajenos = Vec::new();
    for pid in ordenados {
        match sys::kill(pid, sig) {
            Err(e) if e == sys::EPERM => ajenos.push(pid),
            _ => {} // bien, o ya no existe
        }
    }
    if ajenos.is_empty() {
        return None;
    }
    let mut argv = vec!["kill".to_string(), format!("-{}", nombre_senal(sig))];
    argv.extend(ajenos.iter().map(i32::to_string));
    con_pkexec(&argv)
}

pub fn terminar(pids: &[i32], forzar: bool) -> Option<String> {
    let error = senal(pids, if forzar { SIGKILL } else { SIGTERM });
    if !forzar && error.is_none() {
        // Un proceso en pausa (SIGSTOP) no atiende SIGTERM hasta que lo reanudan: se reanuda para que termine.
        senal(pids, SIGCONT);
    }
    error
}

pub fn suspender(pids: &[i32], si: bool) -> Option<String> {
    senal(pids, if si { SIGSTOP } else { SIGCONT })
}

/// Cambia el «nice». Subirla (alta) o tocar procesos ajenos pide permiso con pkexec.
pub fn prioridad(pids: &[i32], nivel: &str) -> Option<String> {
    let valor = PRIORIDADES.iter().find(|(n, _)| *n == nivel)?.1;
    let mut pendientes = Vec::new();
    for pid in pids {
        if let Err(e) = sys::poner_prioridad(*pid, valor)
            && (e == sys::EPERM || e == libc::EACCES)
        {
            pendientes.push(*pid);
        }
    }
    if pendientes.is_empty() {
        return None;
    }
    let mut argv = vec!["renice".to_string(), "-n".into(), valor.to_string(), "-p".into()];
    argv.extend(pendientes.iter().map(i32::to_string));
    con_pkexec(&argv)
}

pub fn prioridad_de(pid: i32) -> &'static str {
    match sys::prioridad(pid) {
        None => "",
        Some(n) if n < 0 => "alta",
        Some(n) if n < 5 => "normal",
        Some(n) if n < 15 => "baja",
        Some(_) => "minima",
    }
}

/// Las ventanas abiertas (pid, clase, título, dirección). Vacío fuera de Hyprland.
pub fn ventanas_hyprland() -> Vec<Ventana> {
    if !ejecutar::existe("hyprctl") || std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return Vec::new();
    }
    match ejecutar::con_limite("hyprctl", &["clients", "-j"], Duration::from_secs(3)) {
        Some(r) => parsear_ventanas(&r.stdout),
        None => Vec::new(),
    }
}

pub fn parsear_ventanas(json: &str) -> Vec<Ventana> {
    let Ok(Value::Array(clientes)) =
        serde_json::from_str::<Value>(if json.trim().is_empty() { "[]" } else { json })
    else {
        return Vec::new();
    };
    let texto = |c: &Value, k: &str| c[k].as_str().unwrap_or("").to_string();
    clientes
        .iter()
        .filter(|c| c["pid"].as_i64().unwrap_or(0) > 0 && c["mapped"].as_bool().unwrap_or(true))
        .map(|c| Ventana {
            pid: c["pid"].as_i64().unwrap_or(0) as i32,
            clase: Some(texto(c, "class"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| texto(c, "initialClass")),
            titulo: texto(c, "title"),
            direccion: texto(c, "address"),
        })
        .collect()
}

/// Cierra las ventanas con amabilidad (como la X): la app puede preguntar si guardas.
pub fn cerrar_ventanas(direcciones: &[String]) {
    for d in direcciones {
        // Una dirección de Hyprland es «0x…»: nada más, para no colar otra orden.
        if !d.starts_with("0x") || !d[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        for orden in [
            format!("hl.dsp.window.close({{ window = \"address:{d}\" }})"),
            format!("closewindow address:{d}"),
        ] {
            match ejecutar::con_limite("hyprctl", &["dispatch", &orden], Duration::from_secs(3)) {
                Some(r) if r.codigo == 0 => break,
                Some(_) => {}
                None => break,
            }
        }
    }
}
