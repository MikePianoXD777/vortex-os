//! vortex-tirones (4.0): el detector de tirones de los juegos.
//!
//! Mientras dura una partida, cuatro veces por segundo lee cuánto tiempo se quedó **trabado el juego**
//! esperando memoria, disco o procesador (la presión de Linux, PSI: `memory.pressure`, `io.pressure` y
//! `cpu.pressure` del cgroup del juego, y `/proc/pressure` de todo el sistema). Si en un cuarto de segundo el
//! juego estuvo parado más de 50 ms (tres cuadros a 60 FPS), eso es un tirón, y anota qué pasaba en ese
//! momento: qué app tenía la memoria, cuál usaba el procesador, si Steam preparaba sombreadores
//! (`fossilize_replay`) o si el procesador estaba ardiendo. Al terminar, el resumen de la partida
//! (`vortex-partida`) suma una línea: cuántos tirones, por qué y qué hacer.
//!
//! Solo ve los tirones que causa la PC (memoria, disco, procesador, calor): los del propio juego (compilar
//! gráficos dentro de DXVK, una zona mal hecha) no pasan por esas esperas. Por eso dice «por la memoria» y no
//! inventa causas. No lanza procesos ni carga MangoHud: lee archivos de `/proc` y del cgroup.
//!
//!   vortex-tirones medir PID [CLASE]        lo lanza el gestor al abrir un juego (una sola copia)
//!   vortex-tirones terminar [--desde SEG]   lo llama vortex-partida: para la medición y dice el resumen
//!   vortex-tirones ver                      el detalle de la última partida (vortex tirones)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Un tirón: el juego estuvo parado al menos esto en un paso de medición.
pub const UMBRAL_US: u64 = 50_000;
/// Dos pasos con tirón separados por menos de esto son el mismo tirón.
const JUNTAR_MS: u64 = 1_000;
/// Cada cuánto se mira quién gasta (procesos, temperatura, cgroup del juego).
const CADA_CONTEXTO: Duration = Duration::from_secs(2);
/// Tope de tirones guardados (una partida con cientos ya dice lo que tiene que decir).
const MAX_EVENTOS: usize = 500;
/// Una app que tiene menos que esto no es «la que llenó la memoria».
const MB_CULPABLE: u64 = 400;
/// % de un núcleo para culpar a una app del procesador.
const CPU_CULPABLE: u64 = 60;
/// Temperatura de la CPU a la que ya baja su velocidad.
const CALOR: u32 = 95;
const PAGINA_KB: u64 = 4;
const TICKS_POR_SEG: u64 = 100;

// ------------------------------------------------------------------------------------------ rutas

pub struct Rutas {
    pub proc_: PathBuf,
    pub cgroot: PathBuf,
    pub hwmon: PathBuf,
    /// `~/.local/state/vortex/tirones`
    pub dir: PathBuf,
    pub paso: Duration,
}

impl Rutas {
    pub fn desde_entorno() -> Self {
        let var = |n: &str, d: &str| std::env::var_os(n).map_or_else(|| PathBuf::from(d), PathBuf::from);
        let ms = std::env::var("VORTEX_TIRONES_PASO_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(250);
        Rutas {
            proc_: var("VORTEX_TIRONES_PROC", "/proc"),
            cgroot: var("VORTEX_TIRONES_CGROOT", "/sys/fs/cgroup"),
            hwmon: var("VORTEX_HWMON", "/sys/class/hwmon"),
            dir: vortex_core::rutas::estado().join("tirones"),
            paso: Duration::from_millis(ms),
        }
    }
    fn informe(&self) -> PathBuf {
        self.dir.join("ultimo.json")
    }
    fn candado(&self) -> PathBuf {
        self.dir.join("medidor.lock")
    }
    fn parar(&self) -> PathBuf {
        self.dir.join("parar")
    }
}

// ---------------------------------------------------------------------------------- lo que se lee

/// Totales de una presión (microsegundos desde que arrancó la PC o se creó el cgroup).
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Psi {
    pub some: u64,
    pub full: u64,
}

/// `some avg10=… total=123` / `full avg10=… total=45` → Psi. None si no tiene ninguna de las dos.
pub fn leer_psi(texto: &str) -> Option<Psi> {
    let mut p = Psi::default();
    let mut vista = false;
    for l in texto.lines() {
        let mut c = l.split_whitespace();
        let tipo = c.next();
        let total = c
            .find_map(|x| x.strip_prefix("total="))
            .and_then(|v| v.parse().ok());
        match (tipo, total) {
            (Some("some"), Some(t)) => {
                p.some = t;
                vista = true;
            }
            (Some("full"), Some(t)) => {
                p.full = t;
                vista = true;
            }
            _ => {}
        }
    }
    vista.then_some(p)
}

/// Lo que sirve de `/proc/PID/stat`.
#[derive(Debug, Clone, PartialEq)]
pub struct Stat {
    pub comm: String,
    pub ppid: u32,
    pub majflt: u64,
    /// utime + stime, en ticks (1/100 s)
    pub cpu: u64,
    pub rss_kb: u64,
}

pub fn leer_stat(texto: &str) -> Option<Stat> {
    // El nombre va entre paréntesis y puede llevar espacios y paréntesis: se corta en el último «)».
    let a = texto.find('(')?;
    let b = texto.rfind(')')?;
    let comm = texto.get(a + 1..b)?.to_string();
    let c: Vec<&str> = texto.get(b + 1..)?.split_whitespace().collect();
    // c[0] es el campo 3 (estado): el campo N está en c[N - 3].
    let n = |i: usize| c.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(Stat {
        comm,
        ppid: u32::try_from(n(1)?).ok()?,
        majflt: n(9)?,
        cpu: n(11)? + n(12)?,
        rss_kb: n(21)? * PAGINA_KB,
    })
}

/// `0::/user.slice/…/app-vortex-steam_app_480-123.scope` → esa ruta (cgroup v2).
pub fn leer_cgroup(texto: &str) -> Option<String> {
    texto
        .lines()
        .find_map(|l| l.strip_prefix("0::"))
        .map(|r| r.trim().to_string())
}

/// `pswpin` de `/proc/vmstat`: páginas que se trajeron del swap.
pub fn leer_swap(texto: &str) -> u64 {
    texto
        .lines()
        .find_map(|l| l.strip_prefix("pswpin "))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// El nombre que entiende una persona. Los procesos de una app (Chrome abre decenas) se juntan en uno.
pub fn nombre_amable(comm: &str) -> String {
    let c = comm.to_lowercase();
    let n = match c.as_str() {
        "chrome" | "google-chrome" => "Google Chrome",
        "chromium" => "Chromium",
        "brave" => "Brave",
        "firefox" | "isolated web co" | "web content" | "webextensions" | "rdd process"
        | "socket process" | "privileged cont" | "utility process" | "forkserver" => "Firefox",
        "discord" | "discordcanary" | "vesktop" => "Discord",
        "steam" | "steamwebhelper" => "Steam",
        "fossilize_repla" | "fossilize_replay" => "Steam (preparando sombreadores)",
        "qs" | "quickshell" => "el escritorio",
        "code" | "code-oss" => "VS Code",
        "obs" => "OBS",
        "spotify" => "Spotify",
        "telegram-deskto" => "Telegram",
        "electron" => "una app de Electron",
        "ollama" | "llama-server" => "la IA local",
        "python3" | "python" => "un programa de Python",
        _ => "",
    };
    if n.is_empty() {
        comm.to_string()
    } else {
        n.to_string()
    }
}

/// Lo que es parte del juego aunque no cuelgue de su proceso (Proton/Wine).
fn es_del_juego(comm: &str) -> bool {
    let c = comm.to_lowercase();
    c.ends_with(".exe")
        || c == "wineserver"
        || c.starts_with("wine")
        || c == "pv-adverb"
        || c == "srt-bwrap"
        || c == "reaper"
        || c == "vortex-tirones"
}

/// Temperatura de la CPU en °C (Intel: coretemp; AMD: k10temp o zenpower), como vortex-partida.
pub fn temp_cpu(hwmon: &Path) -> Option<u32> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(hwmon).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.into_iter().find_map(|h| {
        let nombre = fs::read_to_string(h.join("name")).ok()?;
        matches!(nombre.trim(), "coretemp" | "k10temp" | "zenpower")
            .then(|| fs::read_to_string(h.join("temp1_input")).ok())
            .flatten()
            .and_then(|t| t.trim().parse::<u32>().ok())
            .map(|m| m / 1000)
    })
}

// ------------------------------------------------------------------------------------- quién gasta

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Uso {
    pub nombre: String,
    pub mb: u64,
    /// % de un núcleo en los últimos segundos
    pub cpu: u64,
}

/// Lo que gastan las apps que no son el juego, por app. Guarda el tiempo de CPU de cada proceso para
/// calcular el siguiente.
#[derive(Default)]
pub struct Vigia {
    cpu_antes: HashMap<u32, u64>,
    cuando: Option<Instant>,
    pub memoria: Option<Uso>,
    pub procesador: Option<Uso>,
}

impl Vigia {
    pub fn mirar(&mut self, proc_: &Path, juego: u32, del_cgroup: &HashSet<u32>) {
        let mut procesos: Vec<(u32, Stat)> = Vec::new();
        if let Ok(d) = fs::read_dir(proc_) {
            for e in d.flatten() {
                let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
                    continue;
                };
                if let Some(s) = fs::read_to_string(e.path().join("stat"))
                    .ok()
                    .as_deref()
                    .and_then(leer_stat)
                {
                    procesos.push((pid, s));
                }
            }
        }
        // El juego y lo que cuelga de él (sus hilos de Proton, sus ayudantes).
        let mut familia: HashSet<u32> = del_cgroup.clone();
        if juego > 0 {
            familia.insert(juego);
            let mut crecio = true;
            while crecio {
                crecio = false;
                for (pid, s) in &procesos {
                    if familia.contains(&s.ppid) && familia.insert(*pid) {
                        crecio = true;
                    }
                }
            }
        }
        let ahora = Instant::now();
        let seg = self.cuando.map_or(0.0, |c| ahora.duration_since(c).as_secs_f64());
        let mut por_app: HashMap<String, (u64, u64)> = HashMap::new();
        let mut cpu_nuevo = HashMap::new();
        for (pid, s) in &procesos {
            cpu_nuevo.insert(*pid, s.cpu);
            if familia.contains(pid) || es_del_juego(&s.comm) || s.rss_kb == 0 {
                continue;
            }
            let delta = self.cpu_antes.get(pid).map_or(0, |&a| s.cpu.saturating_sub(a));
            let e = por_app.entry(nombre_amable(&s.comm)).or_insert((0, 0));
            e.0 += s.rss_kb;
            e.1 += delta;
        }
        self.cpu_antes = cpu_nuevo;
        self.cuando = Some(ahora);
        let pct = |ticks: u64| {
            if seg > 0.0 {
                (ticks as f64 * 100.0 / (seg * TICKS_POR_SEG as f64)).round() as u64
            } else {
                0
            }
        };
        let uso = |(n, (kb, t)): (&String, &(u64, u64))| Uso {
            nombre: n.clone(),
            mb: kb / 1024,
            cpu: pct(*t),
        };
        self.memoria = por_app
            .iter()
            .max_by_key(|(n, (kb, _))| (*kb, std::cmp::Reverse((*n).clone())))
            .map(uso);
        self.procesador = por_app
            .iter()
            .max_by_key(|(n, (_, t))| (*t, std::cmp::Reverse((*n).clone())))
            .map(uso);
    }
}

// ------------------------------------------------------------------------------------- los tirones

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Causa {
    #[default]
    Memoria,
    Disco,
    Procesador,
    Calor,
    Sombreadores,
}

impl Causa {
    fn texto(self) -> &'static str {
        match self {
            Causa::Memoria => "la memoria",
            Causa::Disco => "el disco",
            Causa::Procesador => "el procesador",
            Causa::Calor => "el calor",
            Causa::Sombreadores => "los sombreadores de Steam",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Evento {
    /// segundos desde que empezó a medir
    pub segundo: u64,
    /// cuánto estuvo parado el juego, sumando memoria, disco y procesador
    pub ms: u64,
    pub memoria_ms: u64,
    pub disco_ms: u64,
    pub cpu_ms: u64,
    /// páginas traídas del swap durante el tirón
    pub swap: u64,
    pub causa: Causa,
    /// la app que más tenía (memoria) o usaba (procesador), si de verdad era mucho
    pub culpable: String,
    pub culpable_mb: u64,
    pub culpable_cpu: u64,
    pub temp: Option<u32>,
    #[serde(skip)]
    ultimo_ms: u64,
}

/// Lo que estuvo parado el juego en un paso (microsegundos).
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Paso {
    pub memoria: u64,
    pub disco: u64,
    pub cpu: u64,
    pub swap: u64,
}

impl Paso {
    pub fn total(&self) -> u64 {
        self.memoria + self.disco + self.cpu
    }
}

/// Junta los pasos con tirón en tirones y decide su causa.
#[derive(Default)]
pub struct Detector {
    pub eventos: Vec<Evento>,
    abierto: bool,
}

impl Detector {
    /// Un paso a los `ms` milisegundos de empezar, con lo que se sabía de quién gastaba en ese momento.
    pub fn paso(&mut self, ms: u64, p: Paso, vigia: &Vigia, temp: Option<u32>) {
        if p.total() < UMBRAL_US {
            return;
        }
        let seguir = self.abierto
            && self
                .eventos
                .last()
                .is_some_and(|e| ms.saturating_sub(e.ultimo_ms) <= JUNTAR_MS);
        if !seguir {
            if self.eventos.len() >= MAX_EVENTOS {
                return;
            }
            self.eventos.push(Evento {
                segundo: ms / 1000,
                ..Evento::default()
            });
            self.abierto = true;
        }
        let Some(e) = self.eventos.last_mut() else { return };
        e.ultimo_ms = ms;
        e.memoria_ms += p.memoria / 1000;
        e.disco_ms += p.disco / 1000;
        e.cpu_ms += p.cpu / 1000;
        e.ms = e.memoria_ms + e.disco_ms + e.cpu_ms;
        e.swap += p.swap;
        e.temp = e.temp.max(temp);
        decidir(e, vigia);
    }
}

fn decidir(e: &mut Evento, vigia: &Vigia) {
    e.culpable.clear();
    e.culpable_mb = 0;
    e.culpable_cpu = 0;
    // Con swap de por medio, aunque el disco salga más, es la memoria (el disco trabaja por ella).
    let memoria = e.memoria_ms + if e.swap > 0 { e.disco_ms } else { 0 };
    e.causa = if memoria >= e.disco_ms && memoria >= e.cpu_ms {
        Causa::Memoria
    } else if e.cpu_ms >= e.disco_ms {
        Causa::Procesador
    } else {
        Causa::Disco
    };
    match e.causa {
        Causa::Memoria => {
            if let Some(u) = vigia.memoria.as_ref().filter(|u| u.mb >= MB_CULPABLE) {
                e.culpable = u.nombre.clone();
                e.culpable_mb = u.mb;
            }
        }
        Causa::Procesador => {
            if let Some(u) = vigia.procesador.as_ref().filter(|u| u.cpu >= CPU_CULPABLE) {
                e.culpable = u.nombre.clone();
                e.culpable_cpu = u.cpu;
                if u.nombre.starts_with("Steam (preparando") {
                    e.causa = Causa::Sombreadores;
                }
            }
            if e.causa == Causa::Procesador && e.temp.is_some_and(|t| t >= CALOR) {
                e.causa = Causa::Calor;
            }
        }
        _ => {}
    }
}

// --------------------------------------------------------------------------------------- el informe

/// «18/11 13:37» en la hora de la PC (con `date`, que sabe la zona); si no se puede, la de UTC.
fn fecha_local(seg: u64) -> String {
    std::process::Command::new("date")
        .args(["-d", &format!("@{seg}"), "+%d/%m %H:%M"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| vortex_core::registro::fecha_utc(UNIX_EPOCH + Duration::from_secs(seg)))
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Informe {
    pub juego: String,
    /// segundos Unix
    pub inicio: u64,
    pub fin: u64,
    /// true si se midió el cgroup propio del juego (si no, solo la presión de todo el sistema)
    pub del_juego: bool,
    pub eventos: Vec<Evento>,
}

fn gb(mb: u64) -> String {
    if mb >= 1024 {
        format!("{:.1} GB", mb as f64 / 1024.0).replace('.', ",")
    } else {
        format!("{mb} MB")
    }
}

fn tirones(n: usize) -> String {
    if n == 1 {
        "1 tirón".into()
    } else {
        format!("{n} tirones")
    }
}

impl Informe {
    /// Por causa, de la que más tirones dio a la que menos (empate: el orden de Causa).
    fn por_causa(&self) -> Vec<(Causa, usize)> {
        let mut cuenta: HashMap<Causa, usize> = HashMap::new();
        for e in &self.eventos {
            *cuenta.entry(e.causa).or_insert(0) += 1;
        }
        let mut v: Vec<(Causa, usize)> = cuenta.into_iter().collect();
        v.sort_by_key(|(c, n)| (std::cmp::Reverse(*n), *c as u8));
        v
    }

    /// La app que más veces salió como culpable de una causa, con su peor número.
    fn culpable(&self, causa: Causa) -> Option<(String, u64, u64)> {
        let mut veces: HashMap<&str, (usize, u64, u64)> = HashMap::new();
        for e in self
            .eventos
            .iter()
            .filter(|e| e.causa == causa && !e.culpable.is_empty())
        {
            let v = veces.entry(e.culpable.as_str()).or_insert((0, 0, 0));
            v.0 += 1;
            v.1 = v.1.max(e.culpable_mb);
            v.2 = v.2.max(e.culpable_cpu);
        }
        veces
            .into_iter()
            .max_by_key(|(n, (k, _, _))| (*k, std::cmp::Reverse(*n)))
            .map(|(n, (_, mb, cpu))| (n.to_string(), mb, cpu))
    }

    /// Las líneas para el aviso del final de la partida.
    pub fn resumen(&self) -> Vec<String> {
        if self.eventos.is_empty() {
            return vec!["Sin tirones por la memoria, el disco o el procesador".into()];
        }
        let causas = self.por_causa();
        let partes: Vec<String> = causas
            .iter()
            .map(|(c, n)| format!("{n} por {}", c.texto()))
            .collect();
        let primera = if causas.len() == 1 {
            format!("{} por {}", tirones(self.eventos.len()), causas[0].0.texto())
        } else {
            format!("{}: {}", tirones(self.eventos.len()), partes.join(" · "))
        };
        let consejo = match causas[0].0 {
            Causa::Memoria => match self.culpable(Causa::Memoria) {
                Some((n, mb, _)) => format!(
                    "Se llenó la memoria y {n} tenía {}: ciérralo antes de jugar.",
                    gb(mb)
                ),
                None => "Se llenó la memoria: cierra lo que no uses antes de jugar.".into(),
            },
            Causa::Procesador => match self.culpable(Causa::Procesador) {
                Some((n, _, _)) => format!("{n} usaba mucho el procesador: ciérralo antes de jugar."),
                None => "El procesador no daba abasto con el juego y lo demás abierto.".into(),
            },
            Causa::Sombreadores => {
                "Steam estaba preparando los sombreadores: déjalo terminar (pasa al instalar o actualizar)."
                    .into()
            }
            Causa::Calor => {
                let t = self.eventos.iter().filter_map(|e| e.temp).max().unwrap_or(CALOR);
                format!(
                    "El procesador llegó a {t} °C y bajó su velocidad: revisa los ventiladores y el polvo."
                )
            }
            Causa::Disco => "El disco no daba abasto (el juego cargando o algo escribiendo mucho).".into(),
        };
        vec![primera, consejo]
    }

    /// El detalle para `vortex tirones`.
    pub fn detalle(&self) -> String {
        let min = self.fin.saturating_sub(self.inicio) / 60;
        let mut s = format!(
            "{} · {} min · {}\n",
            if self.juego.is_empty() {
                "Partida"
            } else {
                &self.juego
            },
            min,
            fecha_local(self.inicio)
        );
        for l in self.resumen() {
            s.push_str(&format!("  {l}\n"));
        }
        if !self.del_juego {
            s.push_str("  (medido en todo el sistema: el juego no tenía su propio grupo)\n");
        }
        for e in self.eventos.iter().take(40) {
            let quien = if e.culpable.is_empty() {
                String::new()
            } else if e.culpable_mb > 0 {
                format!(" ({}, {})", e.culpable, gb(e.culpable_mb))
            } else {
                format!(" ({}, {} % de un núcleo)", e.culpable, e.culpable_cpu)
            };
            s.push_str(&format!(
                "  {:>3}:{:02}  {:>4} ms  por {}{}\n",
                e.segundo / 60,
                e.segundo % 60,
                e.ms,
                e.causa.texto(),
                quien
            ));
        }
        if self.eventos.len() > 40 {
            s.push_str(&format!("  … y {} más\n", self.eventos.len() - 40));
        }
        s
    }
}

// ------------------------------------------------------------------------------------- la medición

fn ahora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn psi_de(ruta: &Path) -> Option<Psi> {
    fs::read_to_string(ruta).ok().as_deref().and_then(leer_psi)
}

/// Las presiones del juego y del sistema en un momento.
#[derive(Default, Clone, Copy)]
struct Lectura {
    cg_mem: Psi,
    cg_io: Psi,
    cg_cpu: Psi,
    sis_mem: Psi,
    sis_io: Psi,
    swap: u64,
}

fn leer_todo(r: &Rutas, cg: Option<&Path>) -> Lectura {
    let cgp = |f: &str| cg.and_then(|c| psi_de(&c.join(f))).unwrap_or_default();
    Lectura {
        cg_mem: cgp("memory.pressure"),
        cg_io: cgp("io.pressure"),
        cg_cpu: cgp("cpu.pressure"),
        sis_mem: psi_de(&r.proc_.join("pressure/memory")).unwrap_or_default(),
        sis_io: psi_de(&r.proc_.join("pressure/io")).unwrap_or_default(),
        swap: fs::read_to_string(r.proc_.join("vmstat")).map_or(0, |t| leer_swap(&t)),
    }
}

/// Lo que estuvo parado el juego entre dos lecturas. «full» (todo el juego esperando) es lo que se nota en
/// pantalla.
/// - Con un cgroup **propio** (el scope `app-vortex-*` que le da el gestor, o el de su Flatpak) solo cuenta el
///   suyo: el sistema puede estar ahogado mientras el gestor protege al juego, y eso no es un tirón (en la VM,
///   18/11: 10 s de «full» en todo el sistema y 0 en el juego).
/// - Si comparte cgroup (con Steam, por ejemplo), su «full» puede no subir aunque el juego esté parado: se toma
///   el mayor entre el suyo y el del sistema.
/// - Sin cgroup, el del sistema.
fn diferencia(a: &Lectura, b: &Lectura, con_cg: bool, propio: bool) -> Paso {
    let d = |x: u64, y: u64| y.saturating_sub(x);
    let (cg_mem, cg_io, cg_cpu) = if con_cg {
        (
            d(a.cg_mem.full, b.cg_mem.full),
            d(a.cg_io.full, b.cg_io.full),
            d(a.cg_cpu.full, b.cg_cpu.full),
        )
    } else {
        (0, 0, 0)
    };
    let (sis_mem, sis_io) = if con_cg && propio {
        (0, 0)
    } else {
        (d(a.sis_mem.full, b.sis_mem.full), d(a.sis_io.full, b.sis_io.full))
    };
    Paso {
        memoria: cg_mem.max(sis_mem),
        disco: cg_io.max(sis_io),
        cpu: cg_cpu,
        swap: d(a.swap, b.swap),
    }
}

/// ¿Es un cgroup solo del juego? El scope que le da el gestor (`app-vortex-…`) o el de su Flatpak.
fn es_propio(cg: &Path) -> bool {
    cg.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("app-vortex-") || n.starts_with("app-flatpak-"))
}

/// El cgroup del juego, si tiene sus presiones (no la raíz).
fn cgroup_del_juego(r: &Rutas, pid: u32) -> Option<PathBuf> {
    if pid == 0 {
        return None;
    }
    let rel = fs::read_to_string(r.proc_.join(pid.to_string()).join("cgroup"))
        .ok()
        .as_deref()
        .and_then(leer_cgroup)?;
    let rel = rel.trim_start_matches('/');
    if rel.is_empty() {
        return None;
    }
    let ruta = r.cgroot.join(rel);
    ruta.join("memory.pressure").exists().then_some(ruta)
}

/// Los procesos del cgroup del juego, si es uno propio (los scopes `app-vortex-*` del gestor).
fn procesos_del_cgroup(cg: Option<&Path>) -> HashSet<u32> {
    let Some(cg) = cg.filter(|c| {
        c.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("app-vortex-"))
    }) else {
        return HashSet::new();
    };
    fs::read_to_string(cg.join("cgroup.procs"))
        .map(|t| t.lines().filter_map(|l| l.trim().parse().ok()).collect())
        .unwrap_or_default()
}

pub fn medir(r: &Rutas, pid: u32, clase: &str) -> u8 {
    if fs::create_dir_all(&r.dir).is_err() {
        return 1;
    }
    let Ok(candado) = File::create(r.candado()) else {
        return 1;
    };
    // Una sola medición a la vez: si ya hay una (dos «juego abierto» seguidos), esta no hace nada.
    if candado.try_lock().is_err() {
        return 0;
    }
    let _ = fs::remove_file(r.parar());
    let inicio = ahora();
    let reloj = Instant::now();
    let tope = Duration::from_secs(12 * 3600);
    let mut cg = cgroup_del_juego(r, pid);
    let mut del_juego = cg.is_some();
    let mut vigia = Vigia::default();
    vigia.mirar(&r.proc_, pid, &procesos_del_cgroup(cg.as_deref()));
    let mut temp = temp_cpu(&r.hwmon);
    let mut ultimo_contexto = Instant::now();
    let mut antes = leer_todo(r, cg.as_deref());
    let mut det = Detector::default();
    loop {
        std::thread::sleep(r.paso);
        if r.parar().exists() || reloj.elapsed() > tope {
            break;
        }
        if pid > 0 && !r.proc_.join(pid.to_string()).exists() {
            break;
        }
        if ultimo_contexto.elapsed() >= CADA_CONTEXTO {
            ultimo_contexto = Instant::now();
            // El gestor mete al juego en su propio scope al enfocarlo: se vuelve a buscar.
            let nuevo = cgroup_del_juego(r, pid);
            if nuevo != cg {
                cg = nuevo;
                antes = leer_todo(r, cg.as_deref());
                del_juego |= cg.is_some();
            }
            vigia.mirar(&r.proc_, pid, &procesos_del_cgroup(cg.as_deref()));
            temp = temp_cpu(&r.hwmon);
        }
        let ahora_l = leer_todo(r, cg.as_deref());
        let propio = cg.as_deref().is_some_and(es_propio);
        let p = diferencia(&antes, &ahora_l, cg.is_some(), propio);
        antes = ahora_l;
        det.paso(
            u64::try_from(reloj.elapsed().as_millis()).unwrap_or(u64::MAX),
            p,
            &vigia,
            temp,
        );
    }
    let informe = Informe {
        juego: clase.to_string(),
        inicio,
        fin: ahora(),
        del_juego,
        eventos: det.eventos,
    };
    let texto = serde_json::to_string_pretty(&informe).unwrap_or_default();
    let ok = vortex_core::config::escribir_atomico(&r.informe(), &texto).is_ok();
    let _ = fs::remove_file(r.parar());
    drop(candado);
    if ok { 0 } else { 1 }
}

pub fn leer_informe(r: &Rutas) -> Option<Informe> {
    serde_json::from_str(&fs::read_to_string(r.informe()).ok()?).ok()
}

/// Para la medición (si sigue) y devuelve el resumen de la partida que terminó después de `desde`.
pub fn terminar(r: &Rutas, desde: u64) -> Vec<String> {
    if let Ok(c) = File::create(r.candado())
        && c.try_lock().is_err()
    {
        let _ = fs::write(r.parar(), "");
        // El medidor lo ve en su siguiente paso y escribe el informe.
        let limite = Instant::now() + Duration::from_secs(3);
        while Instant::now() < limite && c.try_lock().is_err() {
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = c.unlock();
    }
    match leer_informe(r) {
        Some(i) if i.fin >= desde => i.resumen(),
        _ => Vec::new(),
    }
}

const AYUDA: &str = "uso: vortex-tirones medir PID [CLASE] | terminar [--desde SEGUNDOS] | ver";

pub fn principal(args: &[String]) -> u8 {
    let r = Rutas::desde_entorno();
    match args.first().map(String::as_str) {
        Some("medir") => {
            let Some(pid) = args.get(1).and_then(|p| p.parse::<u32>().ok()) else {
                eprintln!("{AYUDA}");
                return 2;
            };
            medir(&r, pid, args.get(2).map_or("", String::as_str))
        }
        Some("terminar") => {
            let desde = match (args.get(1).map(String::as_str), args.get(2)) {
                (Some("--desde"), Some(s)) => s.parse().unwrap_or(0),
                _ => 0,
            };
            for l in terminar(&r, desde) {
                println!("{l}");
            }
            0
        }
        Some("ver") => {
            match leer_informe(&r) {
                Some(i) => print!("{}", i.detalle()),
                None => println!("Todavía no hay partidas medidas: se miden solas al abrir un juego."),
            }
            0
        }
        Some("--version") => {
            println!("vortex-tirones {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("{AYUDA}");
            2
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn psi() {
        let t = "some avg10=0.00 avg60=0.00 avg300=0.00 total=505\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=77\n";
        assert_eq!(leer_psi(t), Some(Psi { some: 505, full: 77 }));
        // cpu.pressure de un kernel viejo: solo «some»
        assert_eq!(leer_psi("some avg10=1 total=9\n"), Some(Psi { some: 9, full: 0 }));
        assert_eq!(leer_psi("basura"), None);
    }

    #[test]
    fn stat_con_parentesis() {
        let t = "123 (Web (Content)) S 45 1 1 0 -1 4194304 10 0 7 0 300 50 0 0 20 0 30 0 1000 2000000 1000 18446744073709551615";
        let s = leer_stat(t).unwrap();
        assert_eq!(s.comm, "Web (Content)");
        assert_eq!(s.ppid, 45);
        assert_eq!(s.majflt, 7);
        assert_eq!(s.cpu, 350);
        assert_eq!(s.rss_kb, 4000);
    }

    #[test]
    fn nombres() {
        assert_eq!(nombre_amable("chrome"), "Google Chrome");
        assert_eq!(nombre_amable("Isolated Web Co"), "Firefox");
        assert_eq!(
            nombre_amable("fossilize_repla"),
            "Steam (preparando sombreadores)"
        );
        assert_eq!(nombre_amable("blender"), "blender");
        assert!(es_del_juego("eldenring.exe") && es_del_juego("wineserver") && !es_del_juego("chrome"));
    }

    fn vigia(mem: Option<(&str, u64)>, cpu: Option<(&str, u64)>) -> Vigia {
        Vigia {
            memoria: mem.map(|(n, mb)| Uso {
                nombre: n.into(),
                mb,
                cpu: 0,
            }),
            procesador: cpu.map(|(n, c)| Uso {
                nombre: n.into(),
                mb: 0,
                cpu: c,
            }),
            ..Vigia::default()
        }
    }

    #[test]
    fn pasos_chicos_no_son_tirones() {
        let mut d = Detector::default();
        d.paso(
            250,
            Paso {
                memoria: 20_000,
                disco: 20_000,
                cpu: 9_000,
                swap: 0,
            },
            &Vigia::default(),
            None,
        );
        assert!(d.eventos.is_empty());
    }

    #[test]
    fn pasos_seguidos_son_un_tiron() {
        let v = vigia(Some(("Google Chrome", 2458)), None);
        let mut d = Detector::default();
        d.paso(
            1000,
            Paso {
                memoria: 80_000,
                ..Paso::default()
            },
            &v,
            None,
        );
        d.paso(
            1250,
            Paso {
                memoria: 120_000,
                swap: 30,
                ..Paso::default()
            },
            &v,
            None,
        );
        d.paso(
            5000,
            Paso {
                memoria: 60_000,
                ..Paso::default()
            },
            &v,
            None,
        );
        assert_eq!(d.eventos.len(), 2);
        let e = &d.eventos[0];
        assert_eq!((e.segundo, e.ms, e.swap), (1, 200, 30));
        assert_eq!(e.causa, Causa::Memoria);
        assert_eq!((e.culpable.as_str(), e.culpable_mb), ("Google Chrome", 2458));
    }

    #[test]
    fn causas() {
        let mut d = Detector::default();
        // procesador con fossilize → sombreadores
        d.paso(
            0,
            Paso {
                cpu: 90_000,
                ..Paso::default()
            },
            &vigia(None, Some(("Steam (preparando sombreadores)", 300))),
            None,
        );
        // procesador sin nadie que gaste y a 97 °C → calor
        d.paso(
            5000,
            Paso {
                cpu: 90_000,
                ..Paso::default()
            },
            &vigia(None, Some(("Discord", 10))),
            Some(97),
        );
        // disco solo
        d.paso(
            10_000,
            Paso {
                disco: 90_000,
                ..Paso::default()
            },
            &Vigia::default(),
            None,
        );
        // disco con swap → memoria
        d.paso(
            15_000,
            Paso {
                disco: 90_000,
                memoria: 10_000,
                swap: 5,
                ..Paso::default()
            },
            &Vigia::default(),
            None,
        );
        // una app chica no es culpable de la memoria
        d.paso(
            20_000,
            Paso {
                memoria: 90_000,
                ..Paso::default()
            },
            &vigia(Some(("Discord", 200)), None),
            None,
        );
        let c: Vec<Causa> = d.eventos.iter().map(|e| e.causa).collect();
        assert_eq!(
            c,
            [
                Causa::Sombreadores,
                Causa::Calor,
                Causa::Disco,
                Causa::Memoria,
                Causa::Memoria
            ]
        );
        assert!(d.eventos[4].culpable.is_empty());
    }

    #[test]
    fn el_sistema_ahogado_no_es_tiron_si_el_juego_tiene_su_cgroup() {
        let psi = |full| Psi { some: full, full };
        let a = Lectura::default();
        let b = Lectura {
            sis_mem: psi(200_000),
            cg_mem: psi(0),
            ..Lectura::default()
        };
        assert_eq!(
            diferencia(&a, &b, true, true).memoria,
            0,
            "cgroup propio: solo el suyo"
        );
        assert_eq!(
            diferencia(&a, &b, true, false).memoria,
            200_000,
            "cgroup compartido: el mayor"
        );
        assert_eq!(
            diferencia(&a, &b, false, false).memoria,
            200_000,
            "sin cgroup: el del sistema"
        );
        let c = Lectura {
            sis_mem: psi(200_000),
            cg_mem: psi(90_000),
            ..Lectura::default()
        };
        assert_eq!(diferencia(&a, &c, true, true).memoria, 90_000);
        assert!(es_propio(Path::new("/x/app-vortex-steam_app_480-1.scope")));
        assert!(es_propio(Path::new("/x/app-flatpak-org.vinegarhq.Sober-1.scope")));
        assert!(!es_propio(Path::new("/x/app-hyprland-steam-1.scope")));
    }

    fn informe(eventos: Vec<Evento>) -> Informe {
        Informe {
            juego: "steam_app_480".into(),
            inicio: 0,
            fin: 600,
            del_juego: true,
            eventos,
        }
    }

    #[test]
    fn resumen_sin_tirones() {
        assert_eq!(
            informe(vec![]).resumen(),
            ["Sin tirones por la memoria, el disco o el procesador"]
        );
    }

    #[test]
    fn resumen_con_culpable() {
        let m = |mb| Evento {
            causa: Causa::Memoria,
            culpable: "Google Chrome".into(),
            culpable_mb: mb,
            ..Evento::default()
        };
        let i = informe(vec![
            m(2000),
            m(2458),
            Evento {
                causa: Causa::Disco,
                ..Evento::default()
            },
        ]);
        assert_eq!(
            i.resumen(),
            [
                "3 tirones: 2 por la memoria · 1 por el disco",
                "Se llenó la memoria y Google Chrome tenía 2,4 GB: ciérralo antes de jugar."
            ]
        );
        let uno = informe(vec![Evento {
            causa: Causa::Calor,
            temp: Some(98),
            ..Evento::default()
        }]);
        assert_eq!(uno.resumen()[0], "1 tirón por el calor");
        assert!(uno.resumen()[1].contains("98 °C"));
        assert!(i.detalle().contains("por la memoria (Google Chrome, 2,4 GB)"));
    }
}
