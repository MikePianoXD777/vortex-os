//! Correr un programa de Windows con `umu-run` y Proton (igual que `correr.py`).
//!
//! umu-run es el lanzador de Heroic y Lutris: mete a Proton en el runtime de Steam (el mismo contenedor que
//! usa Steam), así que los juegos corren igual que en Steam aunque no estén en Steam. La primera vez baja
//! el runtime (unos 500 MB) a `~/.local/share/umu`.
//!
//! Variables que recibe umu-run: `WINEPREFIX` (el prefijo del programa), `PROTONPATH` (la carpeta del
//! Proton o «GE-Proton» para que lo baje solo), `GAMEID` (`umu-default`: sin arreglos de juegos
//! específicos) y `STORE=none`.
//!
//! Para saber si un programa sigue abierto se buscan los procesos tuyos con su `WINEPREFIX` (lee
//! `/proc/<pid>/environ`): así se ve también el que abriste desde el lanzador, y «Detener» los cierra todos.

use crate::biblioteca::{Biblioteca, Entrada};
use crate::rutas::normalizar;
use crate::{Contexto, protones};
use std::collections::BTreeMap;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const UMU: &str = "umu-run";
pub const PAQUETES: [&str; 2] = ["umu-launcher", "proton-cachyos-slr"];

fn es_ejecutable(p: &Path) -> bool {
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// El `umu-run` a usar, si hay.
pub fn umu(ctx: &Contexto) -> Option<PathBuf> {
    if let Some(propio) = &ctx.umu {
        return es_ejecutable(propio).then(|| propio.clone());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(UMU))
        .find(|p| es_ejecutable(p))
}

/// Los paquetes del sistema que faltan para correr programas (vacío si está todo).
pub fn falta_motor(ctx: &Contexto) -> Vec<String> {
    let mut falta = Vec::new();
    if umu(ctx).is_none() {
        falta.push("umu-launcher".to_string());
    }
    if !protones::lista(ctx)
        .iter()
        .any(|p| p.id.starts_with("proton-cachyos"))
    {
        falta.push("proton-cachyos-slr".to_string());
    }
    falta
}

/// Resultado de instalar el motor: el código de salida y la última línea de error.
pub struct SalidaMotor {
    pub codigo: i32,
    pub detalle: String,
}

/// Instala lo que falte con pacman (pide tu contraseña en una ventana: pkexec).
pub fn instalar_motor(ctx: &Contexto) -> io::Result<SalidaMotor> {
    let falta = falta_motor(ctx);
    if falta.is_empty() {
        return Ok(SalidaMotor {
            codigo: 0,
            detalle: String::new(),
        });
    }
    let r = Command::new(&ctx.pkexec)
        .args(["pacman", "-S", "--needed", "--noconfirm"])
        .args(&falta)
        .stdin(Stdio::null())
        .output()?;
    let err = String::from_utf8_lossy(&r.stderr);
    Ok(SalidaMotor {
        codigo: r.status.code().unwrap_or(-1),
        detalle: err.lines().last().unwrap_or("").trim().to_string(),
    })
}

/// Lo que se le agrega al entorno de umu-run (el resto lo hereda).
pub fn entorno(ctx: &Contexto, e: &Entrada) -> Vec<(String, String)> {
    let lista = protones::lista(ctx);
    let p = protones::buscar(Some(e.proton.as_str()).filter(|x| !x.is_empty()), &lista);
    vec![
        ("WINEPREFIX".into(), e.prefijo.clone()),
        ("PROTONPATH".into(), p.ruta.clone()),
        ("GAMEID".into(), ctx.gameid.clone()),
        ("STORE".into(), "none".into()),
        // Para que el gestor de Vortexine sepa qué programa es (modo juego, Task Manager)
        ("VORTEX_RUNNER_ID".into(), e.id.clone()),
    ]
}

/// `shlex.split` de Python (POSIX). Si las comillas no cierran, parte por espacios (Python fallaría).
pub fn dividir_argumentos(texto: &str) -> Vec<String> {
    fn posix(texto: &str) -> Option<Vec<String>> {
        let mut salida = Vec::new();
        let mut actual = String::new();
        let mut hay = false;
        let mut it = texto.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                c if c.is_whitespace() => {
                    if hay {
                        salida.push(std::mem::take(&mut actual));
                        hay = false;
                    }
                }
                '\\' => {
                    actual.push(it.next()?);
                    hay = true;
                }
                '\'' => {
                    hay = true;
                    loop {
                        match it.next()? {
                            '\'' => break,
                            x => actual.push(x),
                        }
                    }
                }
                '"' => {
                    hay = true;
                    loop {
                        match it.next()? {
                            '"' => break,
                            '\\' => match it.peek().copied() {
                                Some(x @ ('"' | '\\')) => {
                                    actual.push(x);
                                    it.next();
                                }
                                Some(_) => actual.push('\\'),
                                None => return None,
                            },
                            x => actual.push(x),
                        }
                    }
                }
                x => {
                    actual.push(x);
                    hay = true;
                }
            }
        }
        if hay {
            salida.push(actual);
        }
        Some(salida)
    }
    posix(texto).unwrap_or_else(|| texto.split_whitespace().map(str::to_string).collect())
}

/// La orden completa: `umu-run <exe> <argumentos>` (un .msi va con `msiexec /i`).
pub fn orden(ctx: &Contexto, e: &Entrada, exe: Option<&str>, extra: &[String]) -> Vec<String> {
    let ejecutable = umu(ctx).map_or_else(|| UMU.to_string(), |p| p.to_string_lossy().into_owned());
    let exe = exe.unwrap_or(&e.exe);
    if exe.to_lowercase().ends_with(".msi") {
        return vec![ejecutable, "msiexec".into(), "/i".into(), exe.into()];
    }
    let args = if exe == e.exe {
        dividir_argumentos(&e.argumentos)
    } else {
        Vec::new()
    };
    let mut o = vec![ejecutable, exe.to_string()];
    o.extend(args);
    o.extend_from_slice(extra);
    o
}

fn registro(ctx: &Contexto, e: &Entrada) -> io::Result<std::fs::File> {
    let r = ctx.rutas.registros();
    std::fs::create_dir_all(&r)?;
    std::fs::File::create(r.join(format!("{}.log", e.id)))
}

/// Arranca el programa (o otro .exe en su mismo prefijo, como un instalador o winecfg). El que llama se
/// encarga de esperar al `Child` (o de dejarlo: no es un problema, solo queda un zombi hasta que cierre).
pub fn lanzar(ctx: &Contexto, e: &Entrada, exe: Option<&str>, extra: &[String]) -> io::Result<Child> {
    let Some(_) = umu(ctx) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Falta umu-run (el paquete umu-launcher).",
        ));
    };
    std::fs::create_dir_all(&e.prefijo)?;
    let destino = exe.unwrap_or(&e.exe);
    let destino_p = Path::new(destino);
    let carpeta = match destino_p.parent() {
        Some(p) if destino_p.is_absolute() && p.is_dir() => p.to_path_buf(),
        _ => Path::new(&e.exe).parent().unwrap_or(Path::new("/")).to_path_buf(),
    };
    let orden = orden(ctx, e, exe, extra);
    let log = registro(ctx, e)?;
    let log2 = log.try_clone()?;
    let mut c = Command::new(&orden[0]);
    c.args(&orden[1..])
        .envs(entorno(ctx, e))
        .current_dir(carpeta)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log2)
        .process_group(0); // su propio grupo: cerrar Runner no cierra el juego
    c.spawn()
}

fn uid_propio() -> u32 {
    std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0)
}

fn prefijo_de(pid: u32) -> Option<String> {
    let crudo = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    crudo
        .split(|b| *b == 0)
        .find_map(|v| v.strip_prefix(b"WINEPREFIX="))
        .map(|v| String::from_utf8_lossy(v).into_owned())
}

/// `{prefijo: [pids]}` de todos tus procesos con WINEPREFIX (una sola pasada por /proc).
pub fn todos() -> BTreeMap<String, Vec<u32>> {
    let (yo, mio) = (std::process::id(), uid_propio());
    let mut salida: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return salida;
    };
    for d in rd.flatten() {
        let Some(pid) = d.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        if pid == yo || d.metadata().map(|m| m.uid()).ok() != Some(mio) {
            continue;
        }
        if let Some(pref) = prefijo_de(pid) {
            salida.entry(normalizar(&pref)).or_default().push(pid);
        }
    }
    salida
}

/// Los procesos tuyos que corren en ese prefijo.
pub fn procesos(prefijo: &str) -> Vec<u32> {
    todos().remove(&normalizar(prefijo)).unwrap_or_default()
}

pub fn prefijos_abiertos() -> std::collections::BTreeSet<String> {
    todos().into_keys().collect()
}

pub fn corriendo(e: &Entrada) -> bool {
    !procesos(&e.prefijo).is_empty()
}

#[allow(unsafe_code)] // kill(2) no tiene versión segura en la biblioteca estándar
fn enviar(pid: u32, senal: i32) {
    // SAFETY: solo manda una señal a un número de proceso; no toca memoria.
    unsafe {
        libc::kill(pid as libc::pid_t, senal);
    }
}

/// Cierra el programa: primero amable (SIGTERM) y, si sigue, a la fuerza.
pub fn detener(e: &Entrada, espera: Duration) {
    for senal in [libc::SIGTERM, libc::SIGKILL] {
        let pids = procesos(&e.prefijo);
        if pids.is_empty() {
            return;
        }
        for pid in pids {
            enviar(pid, senal);
        }
        let limite = Instant::now() + espera;
        while Instant::now() < limite && !procesos(&e.prefijo).is_empty() {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

/// Para el lanzador («vortex-runner --correr ID»): lo abre, espera a que se cierre y suma el tiempo.
pub fn correr_y_esperar(ctx: &Contexto, bib: &Biblioteca, e: &Entrada) -> io::Result<i32> {
    let inicio = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let mut hijo = lanzar(ctx, e, None, &[])?;
    let estado = hijo.wait()?;
    let codigo = estado.code().unwrap_or_else(|| -estado.signal().unwrap_or(1));
    // umu-run puede salir antes que el juego (si el juego relanza su propio proceso): se espera al prefijo
    while corriendo(e) {
        std::thread::sleep(Duration::from_secs(2));
    }
    let fin = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    bib.sumar_tiempo(&e.id, fin - inicio, inicio);
    Ok(codigo)
}

/// Una entrada «a medias» (un instalador, winecfg) para correr algo en un prefijo sin ser de la biblioteca.
pub fn temporal(id: &str, prefijo: &str, exe: &str) -> Entrada {
    Entrada {
        id: id.into(),
        nombre: id.into(),
        exe: exe.into(),
        prefijo: prefijo.into(),
        proton: String::new(),
        argumentos: String::new(),
        icono: String::new(),
        agregado: 0,
        ultima_vez: 0,
        minutos: 0,
        lanzador: false,
        otras: Default::default(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn argumentos_como_shlex() {
        assert_eq!(
            dividir_argumentos("-windowed 'con espacio'"),
            ["-windowed", "con espacio"]
        );
        assert_eq!(
            dividir_argumentos(r#"a "b \"c\" d" e\ f"#),
            ["a", "b \"c\" d", "e f"]
        );
        assert_eq!(dividir_argumentos("''"), [""]);
        assert!(dividir_argumentos("   ").is_empty());
        assert_eq!(dividir_argumentos("a 'b c"), ["a", "'b", "c"]); // comillas sin cerrar: por espacios
    }
}
