//! Los ajustes (los mismos nombres y valores de serie que el bash) y las rutas y programas que las
//! pruebas cambian por falsos con las mismas variables `VORTEX_GESTOR_*`.

use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Ajustes {
    pub peso_foco: u64,
    pub peso_fondo: u64,
    pub nice_foco: i32,
    pub juegos_clases: String,
    pub antirrebote_ms: i64,
    pub psi_umbral_ms: i64,
    pub psi_ventana_ms: i64,
    pub psi_enfriar_s: i64,
    pub aviso_enfriar_s: i64,
    pub mem_revisar_s: i64,
    pub disco_revisar_s: i64,
    pub reclamar_mb: i64,
    pub ia_revisar_s: i64,
    pub ia_vram_mb: i64,
    pub ia_faltas: i64,
    pub depurar: bool,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            peso_foco: 500,
            peso_fondo: 50,
            nice_foco: -5,
            juegos_clases: r"^(steam_app_[0-9]+|gamescope|steam_proton|Minecraft.*|.+\.exe)$".into(),
            antirrebote_ms: 120,
            psi_umbral_ms: 150,
            psi_ventana_ms: 2000,
            psi_enfriar_s: 20,
            aviso_enfriar_s: 600,
            mem_revisar_s: 30,
            disco_revisar_s: 600,
            reclamar_mb: 64,
            ia_revisar_s: 20,
            ia_vram_mb: 1024,
            ia_faltas: 2,
            depurar: false,
        }
    }
}

/// Parte una línea con la sintaxis de palabras de bash (comillas simples y dobles, barra, `#` de
/// comentario y `;`). No expande nada: `$VAR` y `$(…)` se quedan como texto.
pub fn palabras(linea: &str) -> Vec<String> {
    let mut sal = Vec::new();
    let mut act = String::new();
    let mut hay = false;
    let mut it = linea.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '#' if !hay => break,
            c if c.is_whitespace() || c == ';' => {
                if hay {
                    sal.push(std::mem::take(&mut act));
                    hay = false;
                }
            }
            '\'' => {
                hay = true;
                for q in it.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    act.push(q);
                }
            }
            '"' => {
                hay = true;
                while let Some(q) = it.next() {
                    match q {
                        '"' => break,
                        '\\' => match it.peek() {
                            Some('"' | '\\' | '$' | '`') => act.push(it.next().unwrap_or('\\')),
                            _ => act.push('\\'),
                        },
                        _ => act.push(q),
                    }
                }
            }
            '\\' => {
                hay = true;
                if let Some(n) = it.next() {
                    act.push(n);
                }
            }
            _ => {
                hay = true;
                act.push(c);
            }
        }
    }
    if hay {
        sal.push(act);
    }
    sal
}

impl Ajustes {
    /// Aplica un archivo `gestor.conf` (asignaciones `NOMBRE=valor`, varias por línea si hace falta).
    pub fn aplicar_conf(&mut self, texto: &str) {
        for linea in texto.lines() {
            for p in palabras(linea) {
                let Some((k, v)) = p.split_once('=') else {
                    continue;
                };
                self.poner(k.strip_prefix("export ").unwrap_or(k), v);
            }
        }
    }

    fn poner(&mut self, k: &str, v: &str) {
        let n = |v: &str| v.trim().parse::<i64>().ok();
        match k {
            "PESO_FOCO" => self.peso_foco = n(v).map_or(self.peso_foco, |x| x.max(0) as u64),
            "PESO_FONDO" => self.peso_fondo = n(v).map_or(self.peso_fondo, |x| x.max(0) as u64),
            "NICE_FOCO" => self.nice_foco = n(v).map_or(self.nice_foco, |x| x as i32),
            "JUEGOS_CLASES" => self.juegos_clases = v.to_string(),
            "ANTIRREBOTE_MS" => self.antirrebote_ms = n(v).unwrap_or(self.antirrebote_ms),
            "PSI_UMBRAL_MS" => self.psi_umbral_ms = n(v).unwrap_or(self.psi_umbral_ms),
            "PSI_VENTANA_MS" => self.psi_ventana_ms = n(v).unwrap_or(self.psi_ventana_ms),
            "PSI_ENFRIAR_S" => self.psi_enfriar_s = n(v).unwrap_or(self.psi_enfriar_s),
            "AVISO_ENFRIAR_S" => self.aviso_enfriar_s = n(v).unwrap_or(self.aviso_enfriar_s),
            "MEM_REVISAR_S" => self.mem_revisar_s = n(v).unwrap_or(self.mem_revisar_s),
            "DISCO_REVISAR_S" => self.disco_revisar_s = n(v).unwrap_or(self.disco_revisar_s),
            "RECLAMAR_MB" => self.reclamar_mb = n(v).unwrap_or(self.reclamar_mb),
            "IA_REVISAR_S" => self.ia_revisar_s = n(v).unwrap_or(self.ia_revisar_s),
            "IA_VRAM_MB" => self.ia_vram_mb = n(v).unwrap_or(self.ia_vram_mb),
            "IA_FALTAS" => self.ia_faltas = n(v).unwrap_or(self.ia_faltas),
            "DEPURAR" => self.depurar = n(v).unwrap_or(0) != 0,
            _ => {}
        }
    }

    pub fn regex_juegos(&self) -> Regex {
        // Una expresión mal escrita en el conf no tira el gestor: se usa la de serie.
        Regex::new(&self.juegos_clases)
            .unwrap_or_else(|_| Regex::new(&Ajustes::default().juegos_clases).expect("la de serie compila"))
    }
}

/// Rutas y programas (con los nombres de variable del bash).
#[derive(Debug, Clone)]
pub struct Rutas {
    pub run: PathBuf,
    pub proc: PathBuf,
    pub cgroot: PathBuf,
    pub scx: PathBuf,
    pub psi: PathBuf,
    pub busctl: String,
    /// `None`: se baja el nice directo con setpriority; `Some`: se lanza ese programa (las pruebas).
    pub renice: Option<String>,
    pub vortex: String,
    pub qs: String,
    pub memoria: String,
    pub disco: String,
    pub guardados: String,
    pub nvsmi: String,
    pub ia: String,
    pub shell_qml_env: Option<String>,
    pub inyectar: Option<PathBuf>,
    pub estado_dir: PathBuf,
    pub estado: PathBuf,
    pub marca_juego: PathBuf,
    pub marca_ia: PathBuf,
    pub conf: PathBuf,
    pub raiz_usuario: String,
}

pub const SHELL_QML: &str = "/usr/share/serpantinum/src/quickshell/Shell.qml";

fn var(nombre: &str, defecto: &str) -> String {
    std::env::var(nombre)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| defecto.to_string())
}

impl Rutas {
    pub fn del_entorno(uid: u32) -> Rutas {
        let home = std::env::var("HOME").unwrap_or_default();
        let run = PathBuf::from(var("XDG_RUNTIME_DIR", &format!("/run/user/{uid}")));
        let estado_base = std::env::var("XDG_STATE_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from(&home).join(".local/state"), PathBuf::from);
        let conf_base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from(&home).join(".config"), PathBuf::from);
        let estado_dir = run.join("vortex");
        Rutas {
            proc: var("VORTEX_GESTOR_PROC", "/proc").into(),
            cgroot: var("VORTEX_GESTOR_CGROOT", "/sys/fs/cgroup").into(),
            scx: var("VORTEX_GESTOR_SCX", "/sys/kernel/sched_ext/state").into(),
            psi: var("VORTEX_GESTOR_PSI", "/proc/pressure/memory").into(),
            busctl: var("VORTEX_GESTOR_BUSCTL", "busctl"),
            renice: std::env::var("VORTEX_GESTOR_RENICE")
                .ok()
                .filter(|v| !v.is_empty()),
            vortex: var("VORTEX_GESTOR_VORTEX", "vortex"),
            qs: var("VORTEX_GESTOR_QS", "quickshell"),
            memoria: var("VORTEX_GESTOR_MEMORIA", "vortex-memoria"),
            disco: var("VORTEX_GESTOR_DISCO", "vortex-disco"),
            guardados: var("VORTEX_GESTOR_GUARDADOS", "vortex-guardados"),
            nvsmi: var("VORTEX_GESTOR_NVIDIA_SMI", "nvidia-smi"),
            ia: var("VORTEX_GESTOR_IA", "vortex-ia"),
            shell_qml_env: std::env::var("VORTEX_SHELL_QML").ok().filter(|v| !v.is_empty()),
            inyectar: std::env::var("VORTEX_GESTOR_INYECTAR")
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            estado: estado_dir.join("gestor"),
            estado_dir,
            marca_juego: estado_base.join("vortex/modo-juego"),
            marca_ia: estado_base.join("vortex/modo-ia"),
            conf: conf_base.join("vortex/gestor.conf"),
            raiz_usuario: format!("/user.slice/user-{uid}.slice/user@{uid}.service"),
            run,
        }
    }

    pub fn cg(&self, cg: &str) -> PathBuf {
        let mut s = self.cgroot.clone().into_os_string();
        s.push(cg);
        PathBuf::from(s)
    }

    /// Solo se escribe en cgroups del propio usuario.
    pub fn del_usuario(&self, cg: &str) -> bool {
        cg.strip_prefix(self.raiz_usuario.as_str())
            .is_some_and(|r| r.starts_with('/'))
    }
}

/// Lee `gestor.conf` si se puede.
pub fn cargar(rutas: &Rutas) -> Ajustes {
    let mut a = Ajustes::default();
    if let Ok(t) = std::fs::read_to_string(&rutas.conf) {
        a.aplicar_conf(&t);
    }
    a
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn palabras_como_bash() {
        assert_eq!(
            palabras("PESO_FOCO=500  PESO_FONDO=50  NICE_FOCO=-5"),
            ["PESO_FOCO=500", "PESO_FONDO=50", "NICE_FOCO=-5"]
        );
        assert_eq!(palabras("A='x y' # nada"), ["A=x y"]);
        assert_eq!(palabras(r#"A="a\"b";B=1"#), ["A=a\"b", "B=1"]);
    }

    #[test]
    fn conf_cambia_ajustes() {
        let mut a = Ajustes::default();
        a.aplicar_conf(
            "DEPURAR=1\nMEM_REVISAR_S=1\nIA_REVISAR_S=0\nJUEGOS_CLASES='^(aces)$'\n# PESO_FOCO=9\n",
        );
        assert!(a.depurar);
        assert_eq!((a.mem_revisar_s, a.ia_revisar_s, a.peso_foco), (1, 0, 500));
        assert!(a.regex_juegos().is_match("aces"));
        assert!(!a.regex_juegos().is_match("steam_app_1"));
    }

    #[test]
    fn juegos_de_serie() {
        let r = Ajustes::default().regex_juegos();
        for c in [
            "steam_app_570",
            "gamescope",
            "steam_proton",
            "Minecraft 1.21",
            "juego.exe",
        ] {
            assert!(r.is_match(c), "{c}");
        }
        for c in ["kitty", "steam_app_", "steam", "xexe"] {
            assert!(!r.is_match(c), "{c}");
        }
    }

    #[test]
    fn solo_cgroups_del_usuario() {
        let r = Rutas::del_entorno(1000);
        assert!(r.del_usuario("/user.slice/user-1000.slice/user@1000.service/app.slice/x.scope"));
        assert!(!r.del_usuario("/user.slice/user-1000.slice/session-2.scope"));
        assert!(!r.del_usuario("/user.slice/user-1000.slice/user@1000.service"));
    }
}
