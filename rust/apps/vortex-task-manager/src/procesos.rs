//! Los procesos: se leen de `/proc` (stat, statm, cmdline, status, io) como hace psutil.
//!
//! El CPU de cada proceso está en % de toda la máquina (no de un núcleo), igual que en Windows; la
//! memoria es la privada (RSS menos la compartida) y el disco, lo que leyó y escribió por segundo
//! (solo de tus procesos: los de otros usuarios no se pueden leer sin root).

use crate::sys;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const KTHREADD: i32 = 2;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Proceso {
    pub pid: i32,
    pub ppid: i32,
    pub nombre: String,
    pub comando: String,
    pub uid: u32,
    pub usuario: String,
    pub cpu: f64,
    pub mem: u64,
    pub hilos: u32,
    /// «running», «sleeping», «stopped»… como los llama psutil.
    pub estado: String,
    pub disco: f64,
    /// Lo llena `MemoriaGrafica` (cada 3 s), no `/proc`.
    pub vram: u64,
}

fn es_interprete(base: &str) -> bool {
    let con_numeros =
        |pre: &str, ok: &dyn Fn(char) -> bool| base.strip_prefix(pre).is_some_and(|r| r.chars().all(ok));
    ["bash", "sh", "dash", "node", "perl", "ruby", "qjs", "gjs"].contains(&base)
        || con_numeros("python", &|c| c.is_ascii_digit() || c == '.')
        || con_numeros("lua", &|c| c.is_ascii_digit() || c == '.')
}

fn base_de(ruta: &str) -> String {
    Path::new(ruta)
        .file_name()
        .map_or(String::new(), |f| f.to_string_lossy().into_owned())
}

/// El nombre que se muestra: el de comm (o el de cmdline si comm se cortó en 15 letras) y, para
/// intérpretes (python, bash, node…), el script o el módulo que corren («python3 -m vortex_player»).
pub fn nombre_de(nombre: &str, cmdline: &[String]) -> String {
    let mut base = nombre.to_string();
    if let Some(primero) = cmdline.first() {
        let cero = base_de(primero);
        if !cero.is_empty() && nombre.chars().count() >= 15 && cero.starts_with(nombre) {
            base = cero;
        }
    }
    if es_interprete(&base) && cmdline.len() > 1 {
        let args = &cmdline[1..];
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if a == "-m" && i + 1 < args.len() {
                return args[i + 1].clone();
            }
            if a == "-c" || a == "-e" || a == "--eval" {
                return base;
            }
            if a.starts_with('-') {
                i += 1;
                continue;
            }
            let b = base_de(a);
            return if b.is_empty() { base } else { b };
        }
    }
    base
}

/// Lo que dice `/proc/PID/stat` (después del nombre, que puede traer espacios y paréntesis).
struct Stat {
    comm: String,
    estado: char,
    ppid: i32,
    jiffies: u64,
    hilos: u32,
}

fn leer_stat(texto: &str) -> Option<Stat> {
    let ini = texto.find('(')?;
    let fin = texto.rfind(')')?;
    let campos: Vec<&str> = texto.get(fin + 1..)?.split_whitespace().collect();
    // campos[0] = estado (campo 3); utime = campo 14 → [11]; stime → [12]; num_threads = campo 20 → [17].
    Some(Stat {
        comm: texto.get(ini + 1..fin)?.to_string(),
        estado: campos.first()?.chars().next()?,
        ppid: campos.get(1)?.parse().ok()?,
        jiffies: campos.get(11)?.parse::<u64>().ok()? + campos.get(12)?.parse::<u64>().ok()?,
        hilos: campos.get(17)?.parse().ok()?,
    })
}

pub fn estado_psutil(c: char) -> &'static str {
    match c {
        'R' => "running",
        'S' => "sleeping",
        'D' => "disk-sleep",
        'T' => "stopped",
        't' => "tracing-stop",
        'Z' => "zombie",
        'X' | 'x' => "dead",
        'K' => "wake-kill",
        'W' => "waking",
        'I' => "idle",
        'P' => "parked",
        _ => "unknown",
    }
}

/// uid → nombre de usuario, leyendo `/etc/passwd` una vez.
fn usuarios(passwd: &Path) -> HashMap<u32, String> {
    let mut m = HashMap::new();
    for l in std::fs::read_to_string(passwd).unwrap_or_default().lines() {
        let p: Vec<&str> = l.split(':').collect();
        if let (Some(n), Some(u)) = (p.first(), p.get(2).and_then(|u| u.parse::<u32>().ok())) {
            m.entry(u).or_insert_with(|| n.to_string());
        }
    }
    m
}

/// Muestrea los procesos. Cada llamada a `muestrear()` da el CPU y el disco desde la llamada anterior.
pub struct Procesos {
    raiz: PathBuf,
    usuarios: HashMap<u32, String>,
    io: HashMap<i32, (f64, u64)>,
    cpu: HashMap<i32, (f64, u64)>,
    cpus: f64,
    pagina: u64,
    ticks: f64,
    origen: Instant,
}

impl Default for Procesos {
    fn default() -> Self {
        Self::new()
    }
}

impl Procesos {
    pub fn new() -> Self {
        Self::en("/proc", "/etc/passwd")
    }

    /// Con otra raíz (un `/proc` falso, para las pruebas) y otro `passwd`.
    pub fn en(raiz: impl Into<PathBuf>, passwd: impl AsRef<Path>) -> Self {
        Procesos {
            raiz: raiz.into(),
            usuarios: usuarios(passwd.as_ref()),
            io: HashMap::new(),
            cpu: HashMap::new(),
            cpus: sys::procesadores() as f64,
            pagina: sys::tam_pagina(),
            ticks: sys::ticks_por_segundo(),
            origen: Instant::now(),
        }
    }

    fn usuario(&self, uid: u32) -> String {
        self.usuarios
            .get(&uid)
            .cloned()
            .unwrap_or_else(|| uid.to_string())
    }

    pub fn muestrear(&mut self) -> Vec<Proceso> {
        let ahora = self.origen.elapsed().as_secs_f64();
        self.muestrear_en(ahora)
    }

    /// `ahora` en segundos (de un reloj que no retrocede): así las pruebas fijan el tiempo.
    pub fn muestrear_en(&mut self, ahora: f64) -> Vec<Proceso> {
        let mut filas = Vec::new();
        let mut io_nuevo = HashMap::new();
        let mut cpu_nuevo = HashMap::new();
        let mut pids: Vec<i32> = std::fs::read_dir(&self.raiz)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse().ok()))
            .collect();
        pids.sort_unstable();
        for pid in pids {
            let base = self.raiz.join(pid.to_string());
            // Si el proceso se cierra a media lectura, cualquier archivo falla: se salta.
            let Some(st) = std::fs::read_to_string(base.join("stat"))
                .ok()
                .as_deref()
                .and_then(leer_stat)
            else {
                continue;
            };
            if pid == KTHREADD || st.ppid == KTHREADD {
                continue;
            }
            let cmd: Vec<String> = std::fs::read(base.join("cmdline"))
                .map(|b| {
                    let mut p: Vec<String> = b
                        .split(|c| *c == 0)
                        .map(|x| String::from_utf8_lossy(x).into_owned())
                        .collect();
                    if p.last().is_some_and(String::is_empty) {
                        p.pop();
                    }
                    p
                })
                .unwrap_or_default();
            let Ok(statm) = std::fs::read_to_string(base.join("statm")) else {
                continue;
            };
            let m: Vec<u64> = statm.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            let (rss, compartida) = (
                m.get(1).copied().unwrap_or(0) * self.pagina,
                m.get(2).copied().unwrap_or(0) * self.pagina,
            );
            let uid = std::fs::read_to_string(base.join("status"))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.strip_prefix("Uid:"))
                        .and_then(|r| r.split_whitespace().next().and_then(|u| u.parse::<u32>().ok()))
                })
                .unwrap_or(0);
            // CPU: el tiempo que usó desde la medida anterior entre el tiempo que pasó (la primera vez, 0).
            let mut cpu = 0.0;
            if let Some((antes_t, antes_j)) = self.cpu.get(&pid)
                && ahora > *antes_t
                && st.jiffies >= *antes_j
            {
                cpu = (st.jiffies - antes_j) as f64 / self.ticks / (ahora - antes_t) * 100.0 / self.cpus;
            }
            cpu_nuevo.insert(pid, (ahora, st.jiffies));
            let mut disco = 0.0;
            if let Some(total) = leer_io(&base.join("io")) {
                io_nuevo.insert(pid, (ahora, total));
                if let Some((t, antes)) = self.io.get(&pid)
                    && ahora > *t
                {
                    disco = (total.saturating_sub(*antes)) as f64 / (ahora - t);
                }
            }
            filas.push(Proceso {
                pid,
                ppid: st.ppid,
                nombre: nombre_de(&st.comm, &cmd),
                comando: cmd.join(" "),
                uid,
                usuario: self.usuario(uid),
                cpu,
                mem: rss.saturating_sub(compartida),
                hilos: st.hilos,
                estado: estado_psutil(st.estado).to_string(),
                disco,
                vram: 0,
            });
        }
        self.io = io_nuevo;
        self.cpu = cpu_nuevo;
        filas
    }
}

/// `read_bytes + write_bytes` de `/proc/PID/io` (ilegible si el proceso es de otro usuario).
fn leer_io(ruta: &Path) -> Option<u64> {
    let t = std::fs::read_to_string(ruta).ok()?;
    let campo = |k: &str| {
        t.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.trim().parse::<u64>().ok())
    };
    Some(campo("read_bytes:")? + campo("write_bytes:")?)
}

/// El ejecutable de un proceso (`/proc/PID/exe`); vacío si no se puede ver.
pub fn ubicacion(pid: i32) -> String {
    ubicacion_en(Path::new("/proc"), pid)
}

pub fn ubicacion_en(raiz: &Path, pid: i32) -> String {
    std::fs::read_link(raiz.join(pid.to_string()).join("exe"))
        .map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string())
        .unwrap_or_default()
}
