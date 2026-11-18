//! Lo que el gestor lee de `/proc` (o de la raíz falsa de las pruebas): cgroup, árbol de procesos,
//! nice, límites, memoria y la detección de IA. Todo son lecturas de archivos: no se lanza nada.

use crate::sis;
use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// La primera línea de un archivo, sin espacios a los lados (como `read -r`).
pub fn primera_linea(p: &Path) -> Option<String> {
    let t = std::fs::read_to_string(p).ok()?;
    let l = t.lines().next()?;
    Some(l.trim().to_string())
}

/// El cgroup (v2) del proceso.
pub fn cgroup_de(proc: &Path, pid: u32) -> Option<String> {
    let t = std::fs::read_to_string(proc.join(pid.to_string()).join("cgroup")).ok()?;
    t.lines()
        .find_map(|l| l.trim().strip_prefix("0::").map(str::to_string))
}

/// Un proceso del usuario que se puede ordenar: ni Hyprland, ni Xwayland, ni el shell, ni el gestor.
pub fn apto(proc: &Path, pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    let d = proc.join(pid.to_string());
    let Ok(m) = std::fs::metadata(&d) else {
        return false;
    };
    if !m.is_dir() || m.uid() != sis::euid() {
        return false;
    }
    let Some(comm) = primera_linea(&d.join("comm")) else {
        return false;
    };
    !matches!(
        comm.as_str(),
        "Hyprland" | "Xwayland" | "quickshell" | "qs" | "systemd" | "vortex-gestor"
    ) && !comm.starts_with(".quickshell-wra")
        && !comm.starts_with("vortex-gestor-r")
}

fn nombres_ordenados(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// El proceso y todos sus descendientes (en el mismo orden que el bash: por anchura).
pub fn arbol(proc: &Path, raiz: u32) -> Vec<u32> {
    let mut a = vec![raiz];
    let mut i = 0;
    while i < a.len() && a.len() < 4096 {
        let p = a[i];
        i += 1;
        let tareas = proc.join(p.to_string()).join("task");
        for t in nombres_ordenados(&tareas) {
            if let Ok(txt) = std::fs::read_to_string(tareas.join(&t).join("children")) {
                a.extend(txt.split_whitespace().filter_map(|h| h.parse::<u32>().ok()));
            }
        }
    }
    a
}

/// Los hilos de un proceso (nombres de `task/*`, en orden de texto).
pub fn hilos(proc: &Path, pid: u32) -> Vec<String> {
    nombres_ordenados(&proc.join(pid.to_string()).join("task"))
}

/// El nice del hilo (campo 19 de stat; el nombre del proceso puede tener espacios).
pub fn nice_de(dir_hilo: &Path) -> Option<i32> {
    let s = std::fs::read_to_string(dir_hilo.join("stat")).ok()?;
    let s = s.lines().next()?;
    let resto = &s[s.rfind(") ")? + 2..];
    resto.split_whitespace().nth(16)?.parse().ok()
}

/// ¿El límite RLIMIT_NICE del proceso deja bajar su nice hasta `nice_foco`? Si no, no se toca: subir el
/// nice sin poder devolverlo sería quitarle prioridad para siempre.
pub fn nice_permitido(proc: &Path, pid: u32, nice_foco: i32) -> bool {
    let Ok(t) = std::fs::read_to_string(proc.join(pid.to_string()).join("limits")) else {
        return false;
    };
    for l in t.lines() {
        if !l.starts_with("Max nice priority") {
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        let Some(blando) = w.get(3) else {
            return false;
        };
        if *blando == "unlimited" {
            return true;
        }
        return blando.parse::<i32>().is_ok_and(|n| n >= 20 - nice_foco);
    }
    false
}

/// ¿Queda poca memoria disponible de verdad? Menos del 15 % de la RAM, y como mucho 2 GB. La presión sola
/// no basta: en un equipo con memoria de sobra también llega al mover archivos grandes.
pub fn memoria_baja(proc: &Path) -> bool {
    let Ok(t) = std::fs::read_to_string(proc.join("meminfo")) else {
        return false;
    };
    let (mut total, mut libre) = (0i64, -1i64);
    for l in t.lines() {
        let mut c = l.split_whitespace();
        match (c.next(), c.next().and_then(|v| v.parse::<i64>().ok())) {
            (Some("MemTotal:"), Some(v)) => total = v,
            (Some("MemAvailable:"), Some(v)) => {
                libre = v;
                break;
            }
            _ => {}
        }
    }
    let limite = (total * 15 / 100).min(2_097_152);
    libre >= 0 && libre < limite
}

/// El shell de Quickshell que de verdad corre (`quickshell -p RUTA/Shell.qml`): su IPC solo encuentra la
/// instancia por su ruta, y no siempre es la del paquete. Sin ninguno corriendo, la del paquete.
pub fn shell_qml_activo(proc: &Path, uid: u32, entorno: Option<&str>, de_serie: &str) -> String {
    if let Some(e) = entorno {
        return e.to_string();
    }
    let mut pids: Vec<u32> = nombres_ordenados(proc)
        .iter()
        .filter_map(|n| n.parse().ok())
        .collect();
    pids.sort_unstable();
    for pid in pids {
        let d = proc.join(pid.to_string());
        if std::fs::metadata(&d).map(|m| m.uid()).ok() != Some(uid) {
            continue;
        }
        if primera_linea(&d.join("comm")).as_deref() != Some("quickshell") {
            continue;
        }
        let Ok(cmd) = std::fs::read(d.join("cmdline")) else {
            continue;
        };
        let args: Vec<&[u8]> = cmd.split(|&c| c == 0).collect();
        let ruta = args
            .windows(2)
            .find(|w| w[0] == b"-p" || w[0] == b"--path")
            .map(|w| String::from_utf8_lossy(w[1]).into_owned());
        if let Some(r) = ruta
            && r.ends_with("/Shell.qml")
            && Path::new(&r).is_file()
        {
            return r;
        }
    }
    de_serie.to_string()
}

// ---------------------------------------------------------------- IA

/// En la línea de comandos: Miku, su voz y su oído.
pub fn es_de_miku_cmd(cmd: &str) -> bool {
    cmd.contains("vortex-miku") || cmd.contains("/vortex/miku/") || cmd.contains("/apps/miku/")
}

/// En el cgroup: Miku y el Ollama que ella arranca.
pub fn es_de_miku_cg(cg: &str) -> bool {
    cg.contains("vortex-miku") || cg.contains("vortex-ollama.service")
}

pub fn comm_de_ia(comm: &str) -> bool {
    matches!(comm, "ollama" | "torchrun")
        || ["ollama_llama_se", "llama-", "koboldcpp", "invokeai", "python"]
            .iter()
            .any(|p| comm.starts_with(p))
}

fn tiene_libtorch(maps: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(maps) else {
        return false;
    };
    let aguja = b"libtorch";
    let mut buf = vec![0u8; 64 * 1024];
    let mut resto = 0usize;
    loop {
        let Ok(n) = f.read(&mut buf[resto..]) else {
            return false;
        };
        if n == 0 {
            return false;
        }
        let total = resto + n;
        if buf[..total].windows(aguja.len()).any(|w| w == aguja) {
            return true;
        }
        // por si «libtorch» queda partido entre dos lecturas
        let guardar = (aguja.len() - 1).min(total);
        buf.copy_within(total - guardar..total, 0);
        resto = guardar;
    }
}

/// El programa de IA que corre en ese proceso, por su comm (ya filtrado) y su línea de comandos.
pub fn ia_programa(proc: &Path, pid: u32, comm: &str) -> Option<&'static str> {
    let d = proc.join(pid.to_string());
    let crudo = std::fs::read(d.join("cmdline")).ok()?;
    let mut args: Vec<&[u8]> = crudo.split(|&c| c == 0).collect();
    if args.last().is_some_and(|a| a.is_empty()) {
        args.pop(); // mapfile -d '' descarta el último separador
    }
    if args.is_empty() {
        return None;
    }
    let cmd = args
        .iter()
        .map(|a| String::from_utf8_lossy(a))
        .collect::<Vec<_>>()
        .join(" ");
    // Mike pidió que nadie toque su Miku: ni ella ni su Ollama cuentan.
    if es_de_miku_cmd(&cmd) {
        return None;
    }
    let rodeado = format!(" {cmd} ");
    let baja = cmd.to_lowercase();
    match comm {
        // «ollama serve» solo no tiene modelo cargado: el modelo corre en su «ollama runner».
        "ollama" => rodeado.contains(" runner ").then_some("Ollama"),
        c if c.starts_with("ollama_llama_se") => Some("Ollama"), // el runner de las versiones viejas
        c if c.starts_with("llama-") => Some("llama.cpp"),
        c if c.starts_with("koboldcpp") => Some("KoboldCpp"),
        c if c.starts_with("invokeai") => Some("InvokeAI"),
        "torchrun" => Some("PyTorch"),
        c if c.starts_with("python") => {
            if baja.contains("comfyui") || (rodeado.contains("main.py") && d.join("cwd/comfy").is_dir()) {
                Some("ComfyUI")
            } else if ["stable-diffusion", "sd-webui", "webui-forge"]
                .iter()
                .any(|k| baja.contains(k))
                || ((rodeado.contains("launch.py") || rodeado.contains("webui.py"))
                    && d.join("cwd/webui.py").is_file())
            {
                Some("Stable Diffusion")
            } else if cmd.contains("llama_cpp.server") {
                Some("llama.cpp")
            } else if tiene_libtorch(&d.join("maps")) {
                Some("PyTorch") // un python con PyTorch cargado (entrenando, Whisper, etc.)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// pid → programa, de los procesos que parecen IA. Solo lee archivos de /proc.
pub fn buscar_ia(proc: &Path) -> BTreeMap<u32, &'static str> {
    let mut hallados = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(proc) else {
        return hallados;
    };
    for e in rd.filter_map(|e| e.ok()) {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Some(comm) = primera_linea(&e.path().join("comm")) else {
            continue;
        };
        if !comm_de_ia(&comm) {
            continue;
        }
        let Some(nombre) = ia_programa(proc, pid, &comm) else {
            continue;
        };
        if cgroup_de(proc, pid).is_some_and(|cg| es_de_miku_cg(&cg)) {
            continue;
        }
        hallados.insert(pid, nombre);
    }
    hallados
}

/// Memoria de la gráfica del proceso según el fdinfo de DRM (amdgpu, i915, xe…): los MB, `No` si no abrió
/// ninguna gráfica (corre en la CPU) o `Incierto` si la abrió sin decir cuánta usa (una integrada).
#[derive(Debug, PartialEq, Eq)]
pub enum Drm {
    No,
    Incierto,
    Mb(i64),
}

pub fn drm_mb(proc: &Path, pid: u32) -> Drm {
    let dir = proc.join(pid.to_string()).join("fdinfo");
    let archivos: Vec<String> = nombres_ordenados(&dir);
    if archivos.is_empty() {
        return Drm::No;
    }
    let (mut drm, mut hay) = (false, false);
    let mut v: BTreeMap<String, f64> = BTreeMap::new();
    for a in archivos {
        let Ok(t) = std::fs::read_to_string(dir.join(&a)) else {
            continue;
        };
        let mut id = dir.join(&a).to_string_lossy().into_owned();
        for l in t.lines() {
            let c: Vec<&str> = l.split_whitespace().collect();
            let Some(k) = c.first() else { continue };
            match *k {
                "drm-driver:" => drm = true,
                "drm-client-id:" => id = c.get(1).copied().unwrap_or("").to_string(),
                "drm-memory-vram:" | "drm-total-vram:" | "drm-total-vram0:" | "drm-total-local0:" => {
                    let mult = match c.get(2).copied() {
                        Some("GiB") => 1_048_576.0,
                        Some("MiB") => 1024.0,
                        _ => 1.0,
                    };
                    let n = c.get(1).and_then(|n| n.parse::<f64>().ok()).unwrap_or(0.0);
                    v.insert(id.clone(), n * mult);
                    hay = true;
                }
                _ => {}
            }
        }
    }
    if !drm {
        Drm::No
    } else if !hay {
        Drm::Incierto
    } else {
        Drm::Mb((v.values().sum::<f64>() / 1024.0) as i64)
    }
}

pub fn ruta_cg(cgroot: &Path, cg: &str) -> PathBuf {
    let mut s = cgroot.to_path_buf().into_os_string();
    s.push(cg);
    PathBuf::from(s)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn proceso(r: &Path, pid: u32, comm: &str, cg: &str, hijos: &str) {
        let d = r.join(pid.to_string());
        std::fs::create_dir_all(d.join("task").join(pid.to_string())).unwrap();
        std::fs::write(d.join("comm"), format!("{comm}\n")).unwrap();
        std::fs::write(d.join("cgroup"), format!("0::{cg}\n")).unwrap();
        std::fs::write(d.join("task").join(pid.to_string()).join("children"), hijos).unwrap();
        std::fs::write(
            d.join("task").join(pid.to_string()).join("stat"),
            format!("{pid} ({comm} x) S 1 1 1 0 -1 4194304 0 0 0 0 0 0 0 0 20 -3 1 0 100 0 0\n"),
        )
        .unwrap();
    }

    #[test]
    fn cgroup_arbol_y_nice() {
        let r = raiz();
        proceso(r.path(), 10, "kitty", "/a/b.scope", "11 12 ");
        proceso(r.path(), 11, "bash", "/a/b.scope", "");
        proceso(r.path(), 12, "bash", "/a/otro.scope", "13");
        proceso(r.path(), 13, "bash", "/a/otro.scope", "");
        assert_eq!(cgroup_de(r.path(), 10).as_deref(), Some("/a/b.scope"));
        assert_eq!(arbol(r.path(), 10), [10, 11, 12, 13]);
        assert_eq!(nice_de(&r.path().join("10/task/10")), Some(-3));
        assert!(apto(r.path(), 10));
        assert!(!apto(r.path(), 1));
        proceso(r.path(), 14, "Hyprland", "/x", "");
        assert!(!apto(r.path(), 14));
    }

    #[test]
    fn limite_de_nice() {
        let r = raiz();
        proceso(r.path(), 10, "x", "/a", "");
        let f = |s: &str| {
            std::fs::write(
                r.path().join("10/limits"),
                format!("Limit  Soft  Hard Units\nMax nice priority         {s}                   {s}\n"),
            )
            .unwrap();
            nice_permitido(r.path(), 10, -5)
        };
        assert!(f("30"));
        assert!(f("25"));
        assert!(!f("24"));
        assert!(!f("0"));
        assert!(f("unlimited"));
    }

    #[test]
    fn memoria_poca() {
        let r = raiz();
        let f = |t: &str| {
            std::fs::write(r.path().join("meminfo"), t).unwrap();
            memoria_baja(r.path())
        };
        assert!(f("MemTotal: 8000000 kB\nMemAvailable: 600000 kB\n"));
        assert!(!f("MemTotal: 8000000 kB\nMemAvailable: 1300000 kB\n"));
        // más de 2 GB disponibles nunca es poca, aunque sea menos del 15 %
        assert!(!f("MemTotal: 64000000 kB\nMemAvailable: 3000000 kB\n"));
        assert!(f("MemTotal: 64000000 kB\nMemAvailable: 2000000 kB\n"));
    }

    #[test]
    fn ia_por_comm_y_cmdline() {
        let r = raiz();
        let ia = |pid: u32, comm: &str, args: &[&str]| {
            proceso(r.path(), pid, comm, "/a", "");
            let mut b = Vec::new();
            for a in args {
                b.extend_from_slice(a.as_bytes());
                b.push(0);
            }
            std::fs::write(r.path().join(pid.to_string()).join("cmdline"), b).unwrap();
            ia_programa(r.path(), pid, comm)
        };
        assert_eq!(ia(20, "ollama", &["ollama", "serve"]), None);
        assert_eq!(
            ia(21, "ollama", &["/usr/bin/ollama", "runner", "--model", "/m"]),
            Some("Ollama")
        );
        assert_eq!(ia(22, "llama-server", &["llama-server"]), Some("llama.cpp"));
        assert_eq!(ia(23, "python3", &["python", "main.py"]), None);
        std::fs::create_dir_all(r.path().join("23/cwd/comfy")).unwrap();
        assert_eq!(ia_programa(r.path(), 23, "python3"), Some("ComfyUI"));
        assert_eq!(
            ia(24, "python3", &["python", "-m", "miku.voz", "/vortex/miku/x"]),
            None
        );
        std::fs::write(
            r.path().join("24/maps"),
            "7f-8f r-xp 0 0:0 0 /x/torch/lib/libtorch_cuda.so\n",
        )
        .unwrap();
        assert_eq!(ia(25, "python3", &["python", "x.py"]), None);
        std::fs::write(
            r.path().join("25/maps"),
            "7f-8f r-xp 0 0:0 0 /x/torch/lib/libtorch_cuda.so\n",
        )
        .unwrap();
        assert_eq!(ia_programa(r.path(), 25, "python3"), Some("PyTorch"));
        let todos = buscar_ia(r.path());
        assert_eq!(todos.keys().copied().collect::<Vec<_>>(), [21, 22, 23, 25]);
    }

    #[test]
    fn vram_de_drm() {
        let r = raiz();
        proceso(r.path(), 30, "python3", "/a", "");
        assert_eq!(drm_mb(r.path(), 30), Drm::No);
        let fd = r.path().join("30/fdinfo");
        std::fs::create_dir_all(&fd).unwrap();
        std::fs::write(fd.join("0"), "pos:\t0\nflags:\t02\n").unwrap();
        assert_eq!(drm_mb(r.path(), 30), Drm::No);
        std::fs::write(fd.join("5"), "pos:\t0\ndrm-driver:\tamdgpu\ndrm-client-id:\t7\n").unwrap();
        assert_eq!(drm_mb(r.path(), 30), Drm::Incierto);
        std::fs::write(
            fd.join("5"),
            "pos:\t0\ndrm-driver:\tamdgpu\ndrm-client-id:\t7\ndrm-memory-vram:\t2097152 KiB\n",
        )
        .unwrap();
        std::fs::write(
            fd.join("6"),
            "drm-driver:\tamdgpu\ndrm-client-id:\t7\ndrm-memory-vram:\t2097152 KiB\n",
        )
        .unwrap();
        // el mismo cliente en dos descriptores cuenta una vez
        assert_eq!(drm_mb(r.path(), 30), Drm::Mb(2048));
        std::fs::write(
            fd.join("6"),
            "drm-driver:\ti915\ndrm-client-id:\t9\ndrm-total-local0:\t1 GiB\n",
        )
        .unwrap();
        assert_eq!(drm_mb(r.path(), 30), Drm::Mb(3072));
    }

    #[test]
    fn libtorch_partido_entre_lecturas() {
        let r = raiz();
        let p = r.path().join("maps");
        let mut t = vec![b'x'; 64 * 1024 - 4];
        t.extend_from_slice(b"libtorch_cuda.so\n");
        std::fs::write(&p, t).unwrap();
        assert!(tiene_libtorch(&p));
        std::fs::write(&p, vec![b'x'; 200_000]).unwrap();
        assert!(!tiene_libtorch(&p));
    }
}
