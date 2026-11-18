//! Lo de la página «Rendimiento»: CPU, memoria, discos físicos, red y tarjetas de video. Todo se lee
//! de `/proc` y `/sys` (con raíces cambiables para las pruebas), y las velocidades y porcentajes se miden
//! desde la muestra anterior.

use crate::ejecutar;
use crate::sys;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn leer(ruta: &Path) -> String {
    std::fs::read_to_string(ruta)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn leer_num(ruta: &Path) -> u64 {
    leer(ruta).parse().unwrap_or(0)
}

/// Dónde están `/proc` y `/sys` (otros en las pruebas).
#[derive(Clone)]
pub struct Raices {
    pub proc: PathBuf,
    pub sys: PathBuf,
}

impl Default for Raices {
    fn default() -> Self {
        Raices {
            proc: "/proc".into(),
            sys: "/sys".into(),
        }
    }
}

// --- CPU ---

pub fn nombre_cpu_de(cpuinfo: &str) -> String {
    for linea in cpuinfo.lines() {
        if linea.starts_with("model name") {
            let n = linea
                .split_once(':')
                .map_or("", |(_, v)| v)
                .trim()
                .replace("(R)", "")
                .replace("(TM)", "");
            return limpiar_nombre_cpu(&n);
        }
    }
    "Procesador".into()
}

fn limpiar_nombre_cpu(n: &str) -> String {
    let mut n = n.to_string();
    if let Some(i) = n.find('@') {
        // «… CPU @ 3.20GHz»: se va el «CPU» y los espacios que lo preceden.
        let mut cortado = n[..i].trim_end().to_string();
        if let Some(sin) = cortado.strip_suffix("CPU")
            && sin.ends_with(char::is_whitespace)
        {
            cortado = sin.trim_end().to_string();
        }
        n = cortado;
    } else if let Some(sin) = n.trim_end().strip_suffix("-Core Processor") {
        // «… 8-Core Processor» / «Dual-Core Processor».
        if let Some((resto, ultimo)) = sin.rsplit_once(char::is_whitespace)
            && (ultimo.chars().all(|c| c.is_ascii_digit()) && !ultimo.is_empty()
                || ["Dual", "Quad", "Six", "Eight", "Twelve", "Sixteen"].contains(&ultimo))
        {
            n = resto.trim_end().to_string();
        }
    } else if let Some(sin) = n.trim_end().strip_suffix(" Processor") {
        n = sin.trim_end().to_string();
    }
    n.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Temperatura del CPU de los sensores de hwmon, escogiendo el chip como el de Python.
pub fn temperatura_cpu(sys: &Path) -> Option<f64> {
    let mut chips: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(sys.join("class/hwmon"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in dirs {
        let nombre = leer(&d.join("name"));
        let mut entradas: Vec<(String, f64)> = Vec::new();
        let mut sensores: Vec<String> = std::fs::read_dir(&d)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let f = e.file_name().to_string_lossy().into_owned();
                f.strip_suffix("_input")
                    .filter(|b| b.starts_with("temp"))
                    .map(String::from)
            })
            .collect();
        sensores.sort();
        for s in sensores {
            if let Ok(v) = leer(&d.join(format!("{s}_input"))).parse::<f64>() {
                entradas.push((leer(&d.join(format!("{s}_label"))), v / 1000.0));
            }
        }
        chips.entry(nombre).or_default().extend(entradas);
    }
    let candidatos: [(&str, &[&str]); 5] = [
        ("k10temp", &["Tctl", "Tdie"]),
        ("zenpower", &["Tdie", "Tctl"]),
        ("coretemp", &["Package id 0"]),
        ("cpu_thermal", &[""]),
        ("acpitz", &[""]),
    ];
    for (chip, preferidas) in candidatos {
        let Some(lista) = chips.get(chip) else { continue };
        for (etiqueta, v) in lista {
            if preferidas[0].is_empty() || preferidas.contains(&etiqueta.as_str()) {
                return Some(*v);
            }
        }
        if let Some((_, v)) = lista.first() {
            return Some(*v);
        }
    }
    None
}

/// Cada núcleo: (ocupado, total) de `/proc/stat`, en la medida de psutil.
fn tiempos_cpu(proc: &Path) -> Vec<(f64, f64)> {
    leer(&proc.join("stat"))
        .lines()
        .filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(u8::is_ascii_digit))
        .map(|l| {
            let v: Vec<f64> = l
                .split_whitespace()
                .skip(1)
                .filter_map(|x| x.parse().ok())
                .collect();
            let g = |i: usize| v.get(i).copied().unwrap_or(0.0);
            // user nice system idle iowait irq softirq steal (guest ya va dentro de user)
            let total = g(0) + g(1) + g(2) + g(3) + g(4) + g(5) + g(6) + g(7);
            (total - g(3) - g(4), total)
        })
        .collect()
}

fn frecuencias(raices: &Raices) -> (f64, f64) {
    let (mut cur, mut max, mut n) = (0.0, 0.0, 0.0);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(raices.sys.join("devices/system/cpu"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|f| f.to_str()).is_some_and(|f| {
                f.strip_prefix("cpu")
                    .is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()))
            })
        })
        .collect();
    dirs.sort();
    for d in dirs {
        let f = d.join("cpufreq/scaling_cur_freq");
        if f.exists() {
            cur += leer_num(&f) as f64 / 1000.0;
            let m = leer_num(&d.join("cpufreq/cpuinfo_max_freq")) as f64 / 1000.0;
            max += if m > 0.0 { m } else { 0.0 };
            n += 1.0;
        }
    }
    if n > 0.0 {
        return (cur / n / 1000.0, max / n / 1000.0);
    }
    // Sin cpufreq: los MHz que dice /proc/cpuinfo.
    let mhz: Vec<f64> = leer(&raices.proc.join("cpuinfo"))
        .lines()
        .filter(|l| l.starts_with("cpu MHz"))
        .filter_map(|l| l.split_once(':').and_then(|(_, v)| v.trim().parse().ok()))
        .collect();
    if mhz.is_empty() {
        (0.0, 0.0)
    } else {
        (mhz.iter().sum::<f64>() / mhz.len() as f64 / 1000.0, 0.0)
    }
}

// --- Discos ---

/// nvme0n1, sda… (sin particiones, loop, zram ni device-mapper): modelo, tipo y capacidad.
pub fn discos_fisicos(sys: &Path) -> Vec<Value> {
    let mut nombres: Vec<String> = std::fs::read_dir(sys.join("block"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    nombres.sort();
    let mut discos = Vec::new();
    for n in nombres {
        let d = sys.join("block").join(&n);
        if !d.join("device").exists()
            || ["loop", "zram", "ram", "dm-", "sr"]
                .iter()
                .any(|p| n.starts_with(p))
        {
            continue;
        }
        let sectores = leer_num(&d.join("size"));
        if sectores == 0 {
            continue; // lector de tarjetas sin tarjeta
        }
        let rotatorio = leer(&d.join("queue/rotational")) == "1";
        let removible = leer(&d.join("removable")) == "1";
        let tipo = if removible {
            "USB"
        } else if n.starts_with("nvme") {
            "NVMe"
        } else if rotatorio {
            "HDD"
        } else {
            "SSD"
        };
        let modelo = Some(leer(&d.join("device/model")))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| n.clone());
        discos.push(json!({"id": n, "modelo": modelo, "tipo": tipo, "capacidad": sectores * 512}));
    }
    discos
}

fn desescapar_montaje(s: &str) -> String {
    let mut salida = String::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '\\' && i + 3 < b.len() && b[i + 1..i + 4].iter().all(|c| ('0'..='7').contains(c)) {
            let v = b[i + 1..i + 4]
                .iter()
                .fold(0u32, |a, c| a * 8 + c.to_digit(8).unwrap_or(0));
            salida.push(char::from_u32(v).unwrap_or('?'));
            i += 4;
        } else {
            salida.push(b[i]);
            i += 1;
        }
    }
    salida
}

/// Por disco: (usado, total) de sus particiones montadas (btrfs con varios subvolúmenes cuenta una vez).
pub fn uso_de_discos(discos: &[Value], proc: &Path) -> HashMap<String, (u64, u64)> {
    let fisicos: Vec<String> = leer(&proc.join("filesystems"))
        .lines()
        .filter(|l| !l.starts_with("nodev"))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let mut uso: HashMap<String, (u64, u64)> = HashMap::new();
    let mut vistos: Vec<String> = Vec::new();
    for linea in leer(&proc.join("mounts")).lines() {
        let p: Vec<&str> = linea.split_whitespace().collect();
        if p.len() < 3 || !fisicos.iter().any(|f| f == p[2]) {
            continue;
        }
        let dispositivo = desescapar_montaje(p[0]);
        let dev = Path::new(&dispositivo)
            .file_name()
            .map_or(String::new(), |f| f.to_string_lossy().into_owned());
        let Some(disco) = discos
            .iter()
            .filter_map(|d| d["id"].as_str())
            .find(|d| dev.starts_with(d))
        else {
            continue;
        };
        if vistos.contains(&dispositivo) {
            continue;
        }
        vistos.push(dispositivo);
        let Some((u, t)) = sys::uso_de_disco(&desescapar_montaje(p[1])) else {
            continue;
        };
        let e = uso.entry(disco.to_string()).or_insert((0, 0));
        *e = (e.0 + u, e.1 + t);
    }
    uso
}

/// Contadores de `/proc/diskstats`: nombre → (bytes leídos, bytes escritos, ms ocupado).
fn contadores_discos(proc: &Path) -> HashMap<String, (u64, u64, u64)> {
    leer(&proc.join("diskstats"))
        .lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            let n = |i: usize| p.get(i).and_then(|x| x.parse::<u64>().ok());
            Some((
                p.get(2)?.to_string(),
                (n(5)? * 512, n(9)? * 512, n(12).unwrap_or(0)),
            ))
        })
        .collect()
}

// --- Red ---

/// Las tarjetas de red de verdad (con `/device`): Wi-Fi o Ethernet. Sin lo, docker, veth, virbr…
pub fn interfaces_red(sys: &Path) -> Vec<(String, &'static str)> {
    let mut nombres: Vec<String> = std::fs::read_dir(sys.join("class/net"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    nombres.sort();
    nombres
        .into_iter()
        .filter(|n| sys.join("class/net").join(n).join("device").exists())
        .map(|n| {
            let d = sys.join("class/net").join(&n);
            let wifi = d.join("wireless").exists() || d.join("phy80211").exists();
            (n, if wifi { "Wi-Fi" } else { "Ethernet" })
        })
        .collect()
}

fn contadores_red(proc: &Path) -> HashMap<String, (u64, u64)> {
    leer(&proc.join("net/dev"))
        .lines()
        .skip(2)
        .filter_map(|l| {
            let (n, r) = l.split_once(':')?;
            let v: Vec<u64> = r.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            Some((n.trim().to_string(), (*v.first()?, *v.get(8)?)))
        })
        .collect()
}

fn ssid(interfaz: &str) -> String {
    if !ejecutar::existe("iwgetid") {
        return String::new();
    }
    ejecutar::con_limite("iwgetid", &["-r", interfaz], Duration::from_secs(2))
        .map_or(String::new(), |r| r.stdout.trim().to_string())
}

// --- Tarjetas de video ---

fn num(x: &str) -> f64 {
    x.trim().parse().unwrap_or(0.0)
}

/// La salida de `nvidia-smi --query-gpu=... --format=csv,noheader,nounits`.
pub fn parsear_gpus_nvidia(salida: &str) -> Vec<Value> {
    salida
        .trim()
        .lines()
        .filter_map(|linea| {
            let p: Vec<&str> = linea.split(',').map(str::trim).collect();
            if p.len() < 7 {
                return None;
            }
            Some(json!({"id": format!("nvidia{}", p[0]), "nombre": p[1].replace("NVIDIA ", ""), "uso": num(p[2]),
                "vramUsada": num(p[3]) * 1024.0 * 1024.0, "vramTotal": num(p[4]) * 1024.0 * 1024.0,
                "temp": num(p[5]), "watts": num(p[6])}))
        })
        .collect()
}

pub fn gpus_nvidia() -> Vec<Value> {
    if !ejecutar::existe("nvidia-smi") {
        return Vec::new();
    }
    let args = [
        "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
        "--format=csv,noheader,nounits",
    ];
    ejecutar::con_limite("nvidia-smi", &args, Duration::from_secs(4))
        .map_or(Vec::new(), |r| parsear_gpus_nvidia(&r.stdout))
}

fn nombre_pci(ranura: &str, cache: &mut HashMap<String, String>) -> String {
    cache
        .entry(ranura.to_string())
        .or_insert_with(|| {
            let mut nombre = "GPU".to_string();
            if ejecutar::existe("lspci")
                && let Some(r) = ejecutar::con_limite("lspci", &["-s", ranura], Duration::from_secs(3))
                && let Some((_, n)) = r.stdout.split_once(": ")
            {
                let n = n.trim();
                let ultimo = n
                    .rfind('[')
                    .and_then(|i| n[i + 1..].find(']').map(|j| n[i + 1..i + 1 + j].to_string()));
                nombre = ultimo.unwrap_or_else(|| n.to_string());
            }
            nombre
        })
        .clone()
}

/// AMD (`gpu_busy_percent`) e Intel (sin uso: solo nombre). NVIDIA va por nvidia-smi.
pub fn gpus_sysfs(sys: &Path, cache: &mut HashMap<String, String>) -> Vec<Value> {
    let mut cards: Vec<PathBuf> = std::fs::read_dir(sys.join("class/drm"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.len() == 5 && f.starts_with("card") && f.as_bytes()[4].is_ascii_digit())
        })
        .collect();
    cards.sort();
    let mut gpus = Vec::new();
    for card in cards {
        let dev = card.join("device");
        let vendor = leer(&dev.join("vendor"));
        if vendor != "0x1002" && vendor != "0x8086" {
            continue;
        }
        let uso = leer(&dev.join("gpu_busy_percent"));
        let mut temp = None;
        if let Ok(hw) = std::fs::read_dir(dev.join("hwmon")) {
            let mut hs: Vec<PathBuf> = hw.flatten().map(|e| e.path()).collect();
            hs.sort();
            for h in hs {
                if let Ok(v) = leer(&h.join("temp1_input")).parse::<f64>() {
                    temp = Some(v / 1000.0);
                }
            }
        }
        let ranura = std::fs::canonicalize(&dev)
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let nombre = card
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        gpus.push(json!({"id": nombre, "nombre": nombre_pci(&ranura, cache),
            "uso": if !uso.is_empty() && uso.chars().all(|c| c.is_ascii_digit()) { num(&uso) } else { -1.0 },
            "vramUsada": leer_num(&dev.join("mem_info_vram_used")) as f64,
            "vramTotal": leer_num(&dev.join("mem_info_vram_total")) as f64,
            "temp": temp.unwrap_or(0.0), "watts": 0.0}));
    }
    gpus
}

// --- El muestreador ---

/// Lo que se midió la vez anterior: cuándo, contadores de discos (leído, escrito, ms ocupado) y de red.
type MuestraAnterior = (f64, HashMap<String, (u64, u64, u64)>, HashMap<String, (u64, u64)>);

/// Cada `muestrear()` mide desde el anterior (velocidades, % activo).
pub struct Rendimiento {
    raices: Raices,
    externos: bool,
    pub cpu_nombre: String,
    pub fisicos: usize,
    pub logicos: usize,
    discos: Vec<Value>,
    redes: Vec<(String, &'static str)>,
    cpu_antes: Vec<(f64, f64)>,
    antes: Option<MuestraAnterior>,
    uso_discos: HashMap<String, (u64, u64)>,
    uso_cuando: Option<f64>,
    ssid: HashMap<String, String>,
    ssid_cuando: Option<f64>,
    nombres_pci: HashMap<String, String>,
    origen: Instant,
}

impl Default for Rendimiento {
    fn default() -> Self {
        Self::new()
    }
}

impl Rendimiento {
    pub fn new() -> Self {
        Self::en(Raices::default(), true)
    }

    /// `externos` = si se vale preguntarle a nvidia-smi, lspci, iwgetid (las pruebas lo apagan).
    pub fn en(raices: Raices, externos: bool) -> Self {
        let logicos = tiempos_cpu(&raices.proc).len().max(1);
        Rendimiento {
            cpu_nombre: nombre_cpu_de(&leer(&raices.proc.join("cpuinfo"))),
            fisicos: contar_fisicos(&raices.proc).unwrap_or(logicos),
            logicos,
            discos: discos_fisicos(&raices.sys),
            redes: interfaces_red(&raices.sys),
            cpu_antes: tiempos_cpu(&raices.proc),
            antes: None,
            uso_discos: HashMap::new(),
            uso_cuando: None,
            ssid: HashMap::new(),
            ssid_cuando: None,
            nombres_pci: HashMap::new(),
            origen: Instant::now(),
            raices,
            externos,
        }
    }

    pub fn muestrear(&mut self, procesos: usize, hilos: u64) -> Value {
        let ahora = self.origen.elapsed().as_secs_f64();
        self.muestrear_en(ahora, procesos, hilos)
    }

    /// `ahora` en segundos de un reloj que no retrocede (las pruebas lo fijan).
    pub fn muestrear_en(&mut self, ahora: f64, procesos: usize, hilos: u64) -> Value {
        let Raices { proc, sys: sysd } = self.raices.clone();
        let nuevos = tiempos_cpu(&proc);
        let nucleos: Vec<f64> = nuevos
            .iter()
            .enumerate()
            .map(|(i, (b, t))| match self.cpu_antes.get(i) {
                Some((b0, t0)) if t > t0 => ((b - b0) / (t - t0) * 100.0).clamp(0.0, 100.0),
                _ => 0.0,
            })
            .collect();
        self.cpu_antes = nuevos;
        let (ghz, max_ghz) = frecuencias(&self.raices);
        let mem = memoria(&proc);
        let io = contadores_discos(&proc);
        let red = contadores_red(&proc);
        let dt = self.antes.as_ref().map_or(0.0, |a| ahora - a.0);
        let (io_antes, red_antes) = match &self.antes {
            Some((_, i, r)) => (i.clone(), r.clone()),
            None => (HashMap::new(), HashMap::new()),
        };
        self.antes = Some((ahora, io.clone(), red.clone()));

        if self.uso_cuando.is_none_or(|c| ahora - c > 10.0) {
            self.uso_discos = uso_de_discos(&self.discos, &proc);
            self.uso_cuando = Some(ahora);
        }
        let mut discos = Vec::new();
        for d in &self.discos {
            let id = d["id"].as_str().unwrap_or("");
            let (mut lectura, mut escritura, mut activo) = (0.0f64, 0.0f64, 0.0f64);
            if let (Some(c), Some(a)) = (io.get(id), io_antes.get(id))
                && dt > 0.0
            {
                lectura = c.0.saturating_sub(a.0) as f64 / dt;
                escritura = c.1.saturating_sub(a.1) as f64 / dt;
                activo = (c.2.saturating_sub(a.2) as f64 / (dt * 10.0)).clamp(0.0, 100.0);
            }
            let (usado, total) = self.uso_discos.get(id).copied().unwrap_or((0, 0));
            let capacidad = d["capacidad"].as_u64().unwrap_or(0);
            let mut o = d.clone();
            o["lectura"] = json!(lectura);
            o["escritura"] = json!(escritura);
            o["activo"] = json!(activo);
            o["usado"] = json!(usado);
            o["total"] = json!(if total > 0 { total } else { capacidad });
            discos.push(o);
        }

        if self.externos && self.ssid_cuando.is_none_or(|c| ahora - c > 15.0) {
            self.ssid = self
                .redes
                .iter()
                .filter(|(_, t)| *t == "Wi-Fi")
                .map(|(i, _)| (i.clone(), ssid(i)))
                .collect();
            self.ssid_cuando = Some(ahora);
        }
        let ipv4 = if self.externos {
            sys::direcciones_ipv4()
        } else {
            HashMap::new()
        };
        let mut redes = Vec::new();
        for (id, tipo) in &self.redes {
            let d = sysd.join("class/net").join(id);
            // Arriba (IFF_UP en `flags`) y funcionando (operstate «up», o «unknown» como en el loopback y algunos
            // drivers): como `isup` de psutil. IFF_RUNNING no sale en sysfs: se deduce de operstate.
            let arriba =
                u64::from_str_radix(leer(&d.join("flags")).trim_start_matches("0x"), 16).unwrap_or(0) & 1
                    == 1;
            if !arriba || !matches!(leer(&d.join("operstate")).as_str(), "up" | "unknown") {
                continue; // apagada o sin cable
            }
            let (mut envio, mut recepcion) = (0.0f64, 0.0f64);
            if let (Some(c), Some(a)) = (red.get(id), red_antes.get(id))
                && dt > 0.0
            {
                recepcion = c.0.saturating_sub(a.0) as f64 / dt;
                envio = c.1.saturating_sub(a.1) as f64 / dt;
            }
            let velocidad = leer(&d.join("speed")).parse::<i64>().unwrap_or(0).max(0);
            redes.push(json!({"id": id, "tipo": tipo, "envio": envio, "recepcion": recepcion,
                "ipv4": ipv4.get(id).cloned().unwrap_or_default(), "ssid": self.ssid.get(id).cloned().unwrap_or_default(),
                "velocidad": velocidad}));
        }

        let mut gpus = if self.externos { gpus_nvidia() } else { Vec::new() };
        if self.externos || sysd != Path::new("/sys") {
            gpus.extend(gpus_sysfs(&sysd, &mut self.nombres_pci));
        }
        let uso_cpu = nucleos.iter().sum::<f64>() / nucleos.len().max(1) as f64;
        json!({
            "cpu": {"uso": uso_cpu, "nucleos": nucleos, "nombre": self.cpu_nombre, "ghz": ghz, "maxGhz": max_ghz,
                    "fisicos": self.fisicos, "logicos": self.logicos, "temp": temperatura_cpu(&sysd).unwrap_or(0.0),
                    "procesos": procesos, "hilos": hilos, "encendido": encendido(&proc)},
            "mem": mem,
            "discos": discos,
            "redes": redes,
            "gpus": gpus,
        })
    }
}

fn contar_fisicos(proc: &Path) -> Option<usize> {
    // Pares (physical id, core id) distintos de /proc/cpuinfo.
    let t = leer(&proc.join("cpuinfo"));
    let mut pares: Vec<(String, String)> = Vec::new();
    let (mut fis, mut nucleo) = (String::new(), String::new());
    for l in t.lines().chain(std::iter::once("")) {
        if l.trim().is_empty() {
            if !nucleo.is_empty() && !pares.contains(&(fis.clone(), nucleo.clone())) {
                pares.push((fis.clone(), nucleo.clone()));
            }
            fis.clear();
            nucleo.clear();
        } else if let Some((k, v)) = l.split_once(':') {
            match k.trim() {
                "physical id" => fis = v.trim().to_string(),
                "core id" => nucleo = v.trim().to_string(),
                _ => {}
            }
        }
    }
    Some(pares.len()).filter(|n| *n > 0)
}

fn encendido(proc: &Path) -> f64 {
    let btime = leer(&proc.join("stat"))
        .lines()
        .find_map(|l| {
            l.strip_prefix("btime ")
                .and_then(|v| v.trim().parse::<f64>().ok())
        })
        .unwrap_or(0.0);
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if btime > 0.0 {
        (ahora - btime).max(0.0)
    } else {
        0.0
    }
}

/// `/proc/meminfo` → memoria y swap (en bytes) como los da psutil.
pub fn memoria(proc: &Path) -> Value {
    let t = leer(&proc.join("meminfo"));
    let kb = |k: &str| -> u64 {
        t.lines()
            .find_map(|l| {
                l.strip_prefix(k).and_then(|r| {
                    r.trim_start_matches(':')
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse::<u64>().ok())
                })
            })
            .unwrap_or(0)
            * 1024
    };
    let total = kb("MemTotal");
    let disponible = kb("MemAvailable");
    let usada = total.saturating_sub(disponible);
    let uso = if total > 0 {
        (usada as f64 / total as f64 * 1000.0).round() / 10.0
    } else {
        0.0
    };
    json!({"total": total, "usada": usada, "disponible": disponible, "cache": kb("Cached") + kb("SReclaimable") + kb("Buffers"),
           "uso": uso, "swapTotal": kb("SwapTotal"), "swapUsada": kb("SwapTotal").saturating_sub(kb("SwapFree"))})
}
