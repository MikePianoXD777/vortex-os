//! La memoria de la gráfica (VRAM) de cada proceso: nvidia-smi en NVIDIA y `/proc/PID/fdinfo` en AMD e
//! Intel, medida cada 3 segundos (`MemoriaGrafica`).

use crate::ejecutar;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

/// «865 MiB», «123456 KiB», «4096» (en bytes) → bytes. `None` si no es un número («N/A», «Not available»).
pub fn a_bytes(valor: &str) -> Option<u64> {
    let v = valor.trim();
    let fin_num = v
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(v.len());
    let (num, resto) = v.split_at(fin_num);
    if num.is_empty() || num.starts_with('.') || num.ends_with('.') || num.matches('.').count() > 1 {
        return None;
    }
    let potencia = match resto.trim() {
        "" => 0,
        u => {
            let mut c = u.chars();
            let p = match c.next()? {
                'K' => 1,
                'M' => 2,
                'G' => 3,
                'T' => 4,
                _ => return None,
            };
            // «KiB» o «KB», nada más.
            if !matches!(c.as_str(), "iB" | "B") {
                return None;
            }
            p
        }
    };
    Some((num.parse::<f64>().ok()? * 1024f64.powi(potencia)) as u64)
}

/// La salida de `nvidia-smi -q -d PIDS` → {pid: bytes}. A diferencia de `--query-compute-apps` (solo
/// CUDA), trae también los procesos gráficos (tipo G: juegos, el compositor, el shell); con varias
/// tarjetas, lo de cada una se suma.
pub fn parsear_pids_nvidia(texto: &str) -> HashMap<i32, u64> {
    let mut por_pid = HashMap::new();
    let mut pid: Option<i32> = None;
    for linea in texto.lines() {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        let (clave, valor) = (clave.trim(), valor.trim());
        if clave == "Process ID" {
            pid = valor
                .parse()
                .ok()
                .filter(|_| valor.chars().all(|c| c.is_ascii_digit()));
        } else if clave == "Used GPU Memory"
            && let Some(p) = pid
        {
            if let Some(b) = a_bytes(valor) {
                *por_pid.entry(p).or_insert(0) += b;
            }
            pid = None;
        }
    }
    por_pid
}

/// `{pid: bytes}` de las tarjetas NVIDIA (unos 20 ms). `None` si no hay nvidia-smi o no contestó.
pub fn vram_nvidia() -> Option<HashMap<i32, u64>> {
    if !ejecutar::existe("nvidia-smi") {
        return None;
    }
    let r = ejecutar::con_limite("nvidia-smi", &["-q", "-d", "PIDS"], Duration::from_secs(4))?;
    if r.codigo != 0 || !r.stdout.contains("Processes") {
        return None;
    }
    Some(parsear_pids_nvidia(&r.stdout))
}

// Lo que dice cada driver en /proc/PID/fdinfo, de lo más fiel a lo menos: lo que de verdad ocupa
// (resident) antes que lo reservado (total). amdgpu: «drm-memory-vram» (y desde Linux 6.x
// «drm-resident-vram»); xe: «drm-resident-vram0»; i915 con tarjeta: «drm-resident-local0»; i915
// integrado, que no tiene VRAM propia y usa la RAM: «drm-resident-system0».
#[derive(Clone, Copy)]
enum Patron {
    /// prefijo + una de las variantes + dígitos opcionales
    Con(&'static str, &'static [&'static str]),
    Exacto(&'static str),
}

const CLAVES_DRM: [Patron; 5] = [
    Patron::Con("drm-resident-", &["vram", "local"]),
    Patron::Exacto("drm-memory-vram"),
    Patron::Con("drm-total-", &["vram", "local"]),
    Patron::Con("drm-resident-", &["system"]),
    Patron::Con("drm-total-", &["system"]),
];

fn coincide_patron(p: Patron, clave: &str) -> bool {
    match p {
        Patron::Exacto(e) => clave == e,
        Patron::Con(pre, variantes) => clave.strip_prefix(pre).is_some_and(|r| {
            variantes.iter().any(|v| {
                r.strip_prefix(v)
                    .is_some_and(|d| d.chars().all(|c| c.is_ascii_digit()))
            })
        }),
    }
}

/// Un `/proc/PID/fdinfo/N` → (cliente, bytes en la gráfica). `None` si no es de la gráfica; bytes `None` si
/// el driver no dice cuánta memoria usa (nvidia-drm, por ejemplo). El cliente (`drm-pdev` + `drm-client-id`)
/// sirve para no contar dos veces el mismo, que sale en cada fd duplicado y en los hijos de un fork.
pub fn parsear_fdinfo_drm(texto: &str) -> Option<(String, Option<u64>)> {
    let mut claves: Vec<(String, String)> = Vec::new();
    for linea in texto.lines() {
        if linea.starts_with("drm-")
            && let Some((k, v)) = linea.split_once(':')
        {
            // Como un dict de Python: la última repetición manda.
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            match claves.iter_mut().find(|(c, _)| *c == k) {
                Some(e) => e.1 = v,
                None => claves.push((k, v)),
            }
        }
    }
    let buscar = |k: &str| claves.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone());
    let id = buscar("drm-client-id")?;
    let cliente = format!("{}/{id}", buscar("drm-pdev").unwrap_or_default());
    for patron in CLAVES_DRM {
        let valores: Vec<u64> = claves
            .iter()
            .filter(|(k, _)| coincide_patron(patron, k))
            .filter_map(|(_, v)| a_bytes(v))
            .collect();
        if !valores.is_empty() {
            return Some((cliente, Some(valores.iter().sum())));
        }
    }
    Some((cliente, None))
}

/// AMD e Intel: suma por proceso lo que dicen sus archivos abiertos de `/dev/dri` (solo se pueden leer
/// los de tus procesos). Cada cliente cuenta una vez, para el primer proceso que lo tiene. Devuelve
/// ({pid: bytes}, si algún driver dijo cuánta memoria usa).
pub fn vram_fdinfo(pids: &[i32], raiz: &Path) -> (HashMap<i32, u64>, bool) {
    let mut por_pid = HashMap::new();
    let mut vistos: HashSet<String> = HashSet::new();
    let mut supo = false;
    let mut pids = pids.to_vec();
    pids.sort_unstable();
    for pid in pids {
        let base = raiz.join(pid.to_string());
        let Ok(fds) = std::fs::read_dir(base.join("fd")) else {
            continue;
        };
        let mut fds: Vec<String> = fds
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        fds.sort_by_key(|f| f.parse::<u64>().unwrap_or(u64::MAX));
        for fd in fds {
            let Ok(destino) = std::fs::read_link(base.join("fd").join(&fd)) else {
                continue;
            };
            if !destino.to_string_lossy().starts_with("/dev/dri/") {
                continue;
            }
            let Ok(texto) = std::fs::read(base.join("fdinfo").join(&fd)) else {
                continue;
            };
            let Some((cliente, Some(b))) = parsear_fdinfo_drm(&String::from_utf8_lossy(&texto)) else {
                continue;
            };
            supo = true;
            if vistos.insert(cliente) {
                *por_pid.entry(pid).or_insert(0) += b;
            }
        }
    }
    (por_pid, supo)
}

/// Si hay una tarjeta AMD o Intel (las que dicen su memoria en fdinfo).
pub fn hay_gpu_drm(sys: &Path) -> bool {
    std::fs::read_dir(sys.join("class/drm"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|c| {
            let n = c.file_name().to_string_lossy().into_owned();
            n.starts_with("card")
                && n[4..].len() == 1
                && n[4..].chars().all(|c| c.is_ascii_digit())
                && matches!(
                    std::fs::read_to_string(c.path().join("device/vendor"))
                        .unwrap_or_default()
                        .trim(),
                    "0x1002" | "0x8086"
                )
        })
}

/// La VRAM de cada proceso. Mide cada `intervalo` segundos (nvidia-smi tarda unos 20 ms y leer los
/// fdinfo de todos tus procesos, otro poco); entre medidas devuelve la última. `disponible` dice si hay
/// de dónde sacarla: si no, la columna «Gráfica» no sale.
pub struct MemoriaGrafica {
    pub intervalo: f64,
    nvidia: bool,
    drm: bool,
    raiz_proc: std::path::PathBuf,
    cuando: Option<f64>,
    por_pid: HashMap<i32, u64>,
    pub disponible: bool,
    /// Para las pruebas: en vez de llamar a nvidia-smi.
    pub falsa_nvidia: Option<HashMap<i32, u64>>,
    origen: std::time::Instant,
}

impl MemoriaGrafica {
    pub const INTERVALO: f64 = 3.0;

    pub fn new() -> Self {
        Self::con(
            Self::INTERVALO,
            ejecutar::existe("nvidia-smi"),
            hay_gpu_drm(Path::new("/sys")),
        )
    }

    pub fn con(intervalo: f64, nvidia: bool, drm: bool) -> Self {
        MemoriaGrafica {
            intervalo,
            nvidia,
            drm,
            raiz_proc: "/proc".into(),
            cuando: None,
            por_pid: HashMap::new(),
            disponible: false,
            falsa_nvidia: None,
            origen: std::time::Instant::now(),
        }
    }

    pub fn con_proc(mut self, raiz: impl Into<std::path::PathBuf>) -> Self {
        self.raiz_proc = raiz.into();
        self
    }

    pub fn medir(&mut self, pids: &[i32]) -> &HashMap<i32, u64> {
        let ahora = self.origen.elapsed().as_secs_f64();
        self.medir_en(pids, ahora)
    }

    pub fn medir_en(&mut self, pids: &[i32], ahora: f64) -> &HashMap<i32, u64> {
        if self.cuando.is_some_and(|c| ahora - c < self.intervalo) {
            return &self.por_pid;
        }
        self.cuando = Some(ahora);
        let mut por_pid: HashMap<i32, u64> = HashMap::new();
        let mut hubo = false;
        if self.nvidia {
            let nv = self.falsa_nvidia.clone().or_else(vram_nvidia);
            if let Some(nv) = nv {
                hubo = true;
                por_pid.extend(nv);
            }
        }
        if self.drm {
            let (otras, supo) = vram_fdinfo(pids, &self.raiz_proc);
            hubo = hubo || supo;
            for (pid, b) in otras {
                *por_pid.entry(pid).or_insert(0) += b;
            }
        }
        self.por_pid = por_pid;
        self.disponible = self.disponible || hubo; // una vez que se supo, la columna se queda
        &self.por_pid
    }
}

impl Default for MemoriaGrafica {
    fn default() -> Self {
        Self::new()
    }
}
