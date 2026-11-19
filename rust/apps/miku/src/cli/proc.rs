//! Abrir, cerrar y reconocer la app de Miku (lo mismo que hacía `apps/miku/vortex-miku`): el PID en
//! `$XDG_RUNTIME_DIR/vortex-miku.pid`, un candado por usuario para que dos pulsaciones seguidas no abran dos
//! Mikus, y cerrarla entera (la voz, el oído, Claude y sus herramientas) con calma y luego a la fuerza.

use super::{Ctx, error};
use crate::sistema;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Para reconocer nuestro proceso por su línea de órdenes: la app de Python y la de Rust.
const MARCAS_CMDLINE: [&str; 2] = ["miku/app.py", "miku-rs"];

/// (ppid, pgrp, estado) de un proceso, o `None` si ya no existe.
fn estado_proc(pid: u32) -> Option<(u32, u32, char)> {
    let t = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let resto = t.rsplit_once(')')?.1;
    let mut c = resto.split_whitespace();
    let estado = c.next()?.chars().next()?;
    let ppid = c.next()?.parse().ok()?;
    let pgrp = c.next()?.parse().ok()?;
    Some((ppid, pgrp, estado))
}

fn vivo(pid: u32) -> bool {
    estado_proc(pid).is_some_and(|e| e.2 != 'Z')
}

fn todos() -> HashMap<u32, (u32, u32, char)> {
    let mut r = HashMap::new();
    if let Ok(d) = std::fs::read_dir("/proc") {
        for e in d.flatten() {
            if let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok())
                && let Some(s) = estado_proc(pid)
            {
                r.insert(pid, s);
            }
        }
    }
    r
}

fn descendientes(pid: u32, tabla: &HashMap<u32, (u32, u32, char)>) -> HashSet<u32> {
    let mut hijos: HashMap<u32, Vec<u32>> = HashMap::new();
    for (p, (pp, _, _)) in tabla {
        hijos.entry(*pp).or_default().push(*p);
    }
    let (mut visto, mut pendientes) = (HashSet::new(), vec![pid]);
    while let Some(p) = pendientes.pop() {
        for h in hijos.get(&p).into_iter().flatten() {
            if visto.insert(*h) {
                pendientes.push(*h);
            }
        }
    }
    visto
}

fn es_miku(pid: u32) -> bool {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| {
            let linea = String::from_utf8_lossy(&b).replace('\0', " ");
            MARCAS_CMDLINE.iter().any(|m| linea.contains(m))
        })
        .unwrap_or(false)
}

fn archivo_pid() -> PathBuf {
    crate::rutas::runtime().join("vortex-miku.pid")
}

/// El PID de la app abierta de este usuario, o `None`. Se comprueba que siga viva y sea Miku (no un PID
/// reciclado).
pub fn pid_abierta() -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(archivo_pid()).ok()?.trim().parse().ok()?;
    (vivo(pid) && es_miku(pid)).then_some(pid)
}

/// Cierra la app y todo lo que lanzó. Primero con calma (la app se despide sola y borra audios y la
/// conversación), luego a la fuerza.
pub fn cerrar_todo(pid: u32) {
    let familia = descendientes(pid, &todos());
    sistema::matar(pid, libc::SIGTERM);
    let limite = Instant::now() + Duration::from_secs(4);
    while Instant::now() < limite && vivo(pid) {
        std::thread::sleep(Duration::from_millis(50));
    }
    // Barrido: lo que haya quedado vivo de la familia o del grupo de la app (que es una sesión propia).
    let tabla = todos();
    let mut restos: HashSet<u32> = familia
        .iter()
        .chain([&pid])
        .filter(|p| tabla.get(p).is_some_and(|e| e.2 != 'Z'))
        .copied()
        .collect();
    restos.extend(
        tabla
            .iter()
            .filter(|(_, e)| e.1 == pid && e.2 != 'Z')
            .map(|(p, _)| *p),
    );
    restos.remove(&std::process::id());
    for (senal, espera) in [(libc::SIGTERM, 0.6), (libc::SIGKILL, 0.0)] {
        for p in &restos {
            sistema::matar(*p, senal);
        }
        let limite = Instant::now() + Duration::from_secs_f64(espera);
        while Instant::now() < limite && restos.iter().any(|p| vivo(*p)) {
            std::thread::sleep(Duration::from_millis(50));
        }
        restos.retain(|p| vivo(*p));
        if restos.is_empty() {
            break;
        }
    }
    let _ = std::fs::remove_file(archivo_pid());
    let _ = std::fs::remove_dir_all(crate::rutas::runtime().join("vortex-miku")); // audios temporales de la voz
}

/// La versión en Rust de la app (`miku-rs`), si está instalada: `VORTEX_RUST_BIN` o `/usr/lib/vortex`.
fn app_rust() -> Option<PathBuf> {
    let carpeta = std::env::var_os("VORTEX_RUST_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| "/usr/lib/vortex".into());
    let b = carpeta.join("miku-rs");
    sistema::es_ejecutable(&b).then_some(b)
}

/// Cómo se abre la app: (programa, argumentos, variables de más).
type OrdenApp = (PathBuf, Vec<String>, Vec<(String, String)>);

fn orden_de_la_app(rutas: &crate::rutas::Rutas) -> Result<OrdenApp, String> {
    let python = || sistema::cual("python3").unwrap_or_else(|| "python3".into());
    // VORTEX_MIKU_APP: pruebas. Un .py corre con Python; cualquier otra cosa, tal cual.
    if let Some(app) = std::env::var_os("VORTEX_MIKU_APP").filter(|v| !v.is_empty()) {
        let app = PathBuf::from(app);
        if app.extension().is_some_and(|e| e == "py") {
            return Ok((python(), vec!["-I".into(), app.to_string_lossy().into()], vec![]));
        }
        return Ok((app, vec![], vec![]));
    }
    // 4.0: si está la versión en Rust (paquete vortex-rust) se usa esa: la misma Miku (persona, web, voz y
    // configuración de siempre), menos memoria. VORTEX_MIKU_PYTHON=1 obliga a usar la de Python.
    if std::env::var_os("VORTEX_MIKU_PYTHON").is_none_or(|v| v.is_empty())
        && let Some(b) = app_rust()
    {
        return Ok((
            b,
            vec![],
            vec![
                ("VORTEX_MIKU_RAIZ".into(), rutas.raiz.to_string_lossy().into()),
                ("MALLOC_ARENA_MAX".into(), "2".into()),
                ("MALLOC_TRIM_THRESHOLD_".into(), "131072".into()),
            ],
        ));
    }
    let app = rutas.raiz.join("miku/app.py");
    if !app.is_file() {
        return Err(format!("no encuentro la app de Miku ({}).", app.display()));
    }
    Ok((python(), vec!["-I".into(), app.to_string_lossy().into()], vec![]))
}

/// Abre la app, aparte de este comando: sesión propia, para poder cerrarla entera después.
pub fn lanzar(cx: &Ctx, extra: &[(&str, &str)]) -> i32 {
    let runtime = crate::rutas::runtime();
    let log = runtime.join("vortex-miku.log");
    let registro = match std::fs::File::create(&log) {
        Ok(f) => f,
        Err(e) => return error(&format!("no pude abrir a Miku ({}).", super::strerror(&e)), 1),
    };
    let (programa, args, vars) = match orden_de_la_app(&cx.rutas) {
        Ok(o) => o,
        Err(e) => return error(&format!("no pude abrir a Miku ({e})"), 1),
    };
    let mut c = Command::new(&programa);
    c.args(&args).stdin(Stdio::null());
    if let (Ok(a), Ok(b)) = (registro.try_clone(), registro.try_clone()) {
        c.stdout(a).stderr(b);
    }
    if std::env::var_os("QT_WAYLAND_DISABLE_WINDOWDECORATION").is_none() {
        c.env("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");
    }
    for (k, v) in &vars {
        if std::env::var_os(k).is_none() || k == "VORTEX_MIKU_RAIZ" {
            c.env(k, v);
        }
    }
    for (k, v) in extra {
        c.env(k, v);
    }
    sistema::nueva_sesion(&mut c);
    let mut hijo = match c.spawn() {
        Ok(h) => h,
        Err(e) => return error(&format!("no pude abrir a Miku ({}).", super::strerror(&e)), 1),
    };
    let _ = std::fs::write(archivo_pid(), hijo.id().to_string());
    std::thread::sleep(Duration::from_millis(500));
    if matches!(hijo.try_wait(), Ok(Some(_))) {
        let cola: Vec<String> = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .trim()
            .lines()
            .map(String::from)
            .collect();
        let cola = cola[cola.len().saturating_sub(6)..].join(" | ");
        let _ = std::fs::remove_file(archivo_pid());
        return error(
            &format!(
                "Miku se cerró al abrir. {}",
                if cola.is_empty() {
                    format!("Mira el registro en {}", log.display())
                } else {
                    cola
                }
            ),
            1,
        );
    }
    // Que no quede zombi si seguimos vivos un rato (el comando casi siempre sale antes).
    std::thread::spawn(move || {
        let _ = hijo.wait();
    });
    0
}

/// Un candado por usuario en `$XDG_RUNTIME_DIR`: dos pulsaciones seguidas no abren dos Mikus.
pub struct Candado(#[allow(dead_code)] Option<std::fs::File>);

impl Candado {
    pub fn tomar() -> Self {
        Self(sistema::candado_esperando(
            &crate::rutas::runtime().join("vortex-miku.lock"),
        ))
    }
}

/// Abre a Miku si está cerrada; si ya está abierta, la cierra del todo.
pub fn alternar(cx: &Ctx) -> i32 {
    let _c = Candado::tomar();
    if let Some(pid) = pid_abierta() {
        cerrar_todo(pid);
        return 0;
    }
    lanzar(cx, &[])
}

/// Abre a Miku si está cerrada; si ya está abierta, la deja (el botón «Hablar con Miku» de un aviso).
pub fn mostrar(cx: &Ctx) -> i32 {
    let _c = Candado::tomar();
    if pid_abierta().is_some() {
        0
    } else {
        lanzar(cx, &[])
    }
}

/// Para que un cambio (avatar, voz, modelo) se vea ya: si Miku está abierta, la cierra y la abre de nuevo.
pub fn reabrir_si_esta_abierta(cx: &Ctx) -> bool {
    let _c = Candado::tomar();
    let Some(pid) = pid_abierta() else { return false };
    cerrar_todo(pid);
    lanzar(cx, &[]);
    true
}

/// Corre algo y espera como mucho `segundos` (después lo mata). `true` si salió bien a tiempo.
pub fn correr_con_limite(c: &mut Command, segundos: u64) -> bool {
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let Ok(mut h) = c.spawn() else { return false };
    let limite = Instant::now() + Duration::from_secs(segundos);
    loop {
        match h.try_wait() {
            Ok(Some(s)) => return s.success(),
            Ok(None) if Instant::now() < limite => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = h.kill();
                let _ = h.wait();
                return false;
            }
        }
    }
}

/// Lanza `programa args…` aparte de este comando (sesión propia, sin entrada ni salida).
pub fn aparte(programa: &Path, args: &[String]) {
    let _ = sistema::lanzar_aparte(&programa.to_string_lossy(), args);
}
