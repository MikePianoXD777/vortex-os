//! El mundo falso de las pruebas del gestor (el mismo de `pruebas/gestor.sh`): un Hyprland falso (sockets
//! UNIX en una carpeta temporal), `/proc` y cgroups de mentira, y `busctl`, `renice`, `vortex`,
//! `quickshell`, `nvidia-smi` y compañía falsos (archivos ejecutables del repo, enlazados). HOME y XDG son
//! temporales: nada toca tu sesión, tus procesos ni tu systemd.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub fn aqui() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// Enlaza (no copia) un falso del repo en `destino/nombre`: si se copiara, el archivo recién escrito puede
/// dar «Text file busy» al ejecutarlo mientras otra prueba hace fork.
pub fn instalar_falso(origen: &str, destino: &Path, nombre: &str) -> PathBuf {
    std::fs::create_dir_all(destino).unwrap();
    let d = destino.join(nombre);
    std::os::unix::fs::symlink(aqui().join("falsos").join(origen), &d).unwrap();
    d
}

/// Espera (hasta 8 s) a que se cumpla una condición.
pub fn esperar(que: &str, mut cond: impl FnMut() -> bool) {
    let fin = Instant::now() + Duration::from_secs(8);
    while Instant::now() < fin {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("no se cumplió a tiempo: {que}");
}

// ====================================================================== el Hyprland falso

pub struct FalsoHypr {
    pub activa: Arc<Mutex<String>>,
    pub clientes: Arc<Mutex<String>>,
    eventos: Arc<Mutex<Option<UnixStream>>>,
}

impl FalsoHypr {
    /// `.socket.sock` responde a j/activewindow y j/clients; `.socket2.sock` manda lo que la prueba escriba.
    pub fn nuevo(dir: &Path) -> FalsoHypr {
        std::fs::create_dir_all(dir).unwrap();
        let h = FalsoHypr {
            activa: Arc::new(Mutex::new("{}".into())),
            clientes: Arc::new(Mutex::new("[]".into())),
            eventos: Arc::new(Mutex::new(None)),
        };
        let ordenes = UnixListener::bind(dir.join(".socket.sock")).unwrap();
        let (a, c) = (h.activa.clone(), h.clientes.clone());
        std::thread::spawn(move || {
            for s in ordenes.incoming().flatten() {
                let (a, c) = (a.clone(), c.clone());
                std::thread::spawn(move || {
                    let mut s = s;
                    let mut b = [0u8; 4096];
                    let n = s.read(&mut b).unwrap_or(0);
                    let pedido = String::from_utf8_lossy(&b[..n]).to_string();
                    let r = match pedido.as_str() {
                        "j/activewindow" => a.lock().unwrap().clone(),
                        "j/clients" => c.lock().unwrap().clone(),
                        _ => String::new(),
                    };
                    let _ = s.write_all(r.as_bytes());
                });
            }
        });
        let ev = UnixListener::bind(dir.join(".socket2.sock")).unwrap();
        let guardado = h.eventos.clone();
        std::thread::spawn(move || {
            if let Ok((s, _)) = ev.accept() {
                *guardado.lock().unwrap() = Some(s);
            }
        });
        h
    }

    /// La ventana enfocada (el JSON de `j/activewindow`).
    pub fn activa(&self, pid: u32, dir: &str, clase: &str, tipo: &str) {
        *self.activa.lock().unwrap() = format!(
            "{{\n    \"address\": \"0x{dir}\",\n    \"class\": \"{clase}\",\n    \"title\": \"t\",\n    \"pid\": {pid},\n    \"contentType\": \"{tipo}\"\n}}\n"
        );
    }

    pub fn sin_ventana(&self) {
        *self.activa.lock().unwrap() = "{}".into();
    }

    /// Manda líneas por el socket de eventos (espera a que alguien se haya conectado).
    pub fn evento(&self, lineas: &[&str]) {
        esperar("alguien escucha el socket de eventos", || {
            self.eventos.lock().unwrap().is_some()
        });
        let mut g = self.eventos.lock().unwrap();
        let s = g.as_mut().unwrap();
        for l in lineas {
            s.write_all(l.as_bytes()).unwrap();
            s.write_all(b"\n").unwrap();
        }
    }

    /// Hyprland se cierra: el socket de eventos se cierra.
    pub fn cerrar(&self) {
        esperar("alguien escucha el socket de eventos", || {
            self.eventos.lock().unwrap().is_some()
        });
        *self.eventos.lock().unwrap() = None;
    }
}

// ====================================================================== el mundo

pub struct Mundo {
    pub t: tempfile::TempDir,
    pub run: PathBuf,
    pub hd: PathBuf,
    pub proc: PathBuf,
    pub cgr: PathBuf,
    pub raiz: String,
    pub sesion: String,
    pub hypr: FalsoHypr,
    pub gestor: Option<Child>,
    pub inyecta: PathBuf,
}

pub fn uid() -> u32 {
    vortex_gestor::sis::uid()
}

impl Mundo {
    pub fn nuevo() -> Mundo {
        let t = tempfile::Builder::new()
            .prefix("vortex-gestor-prueba.")
            .tempdir()
            .unwrap();
        let p = t.path();
        let run = p.join("run");
        let hd = run.join("hypr/prueba_1700000000_1");
        let proc = p.join("proc");
        let cgr = p.join("cg");
        let raiz = format!("/user.slice/user-{}.slice/user@{}.service", uid(), uid());
        let sesion = format!("/user.slice/user-{}.slice/session-2.scope", uid());
        for d in [
            run.join("quickshell/by-id/abc"),
            proc.clone(),
            PathBuf::from(format!("{}{}/app.slice", cgr.display(), raiz)),
            p.join("estado"),
            p.join("conf/vortex"),
            p.join("home"),
        ] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(p.join("scx"), "enabled\n").unwrap();
        std::fs::write(p.join("psi"), "some avg10=3.10 avg60=1.00 avg300=0.20 total=1\n").unwrap();
        // El FIFO por donde la prueba inyecta avisos de presión de memoria (el kernel no los da con un archivo).
        let inyecta = p.join("inyecta");
        let c = Command::new("mkfifo").arg(&inyecta).status().unwrap();
        assert!(c.success());
        let hypr = FalsoHypr::nuevo(&hd);
        // Los programas falsos, enlazados en una carpeta.
        let bin = p.join("bin");
        for (o, n) in [
            ("busctl_falso", "busctl"),
            ("renice_falso", "renice"),
            ("vortex_falso", "vortex"),
            ("nvidia_smi_falso", "nvidia-smi"),
            ("quickshell_falso", "quickshell"),
            ("vortex_ia_falso", "vortex-ia"),
            ("memoria_falso", "vortex-memoria"),
            ("guardados_falso", "vortex-guardados"),
            ("disco_falso", "vortex-disco"),
        ] {
            instalar_falso(o, &bin, n);
        }
        Mundo {
            t,
            run,
            hd,
            proc,
            cgr,
            raiz,
            sesion,
            hypr,
            gestor: None,
            inyecta,
        }
    }

    pub fn p(&self) -> &Path {
        self.t.path()
    }

    /// El `gestor.conf` de la prueba.
    pub fn conf(&self, texto: &str) {
        std::fs::write(self.p().join("conf/vortex/gestor.conf"), texto).unwrap();
    }

    pub fn estado_archivo(&self) -> PathBuf {
        self.run.join("vortex/gestor")
    }

    /// Arranca el gestor de Rust en este mundo. No toca nada fuera de la carpeta temporal.
    pub fn iniciar(&mut self) {
        self.iniciar_con(true);
    }

    /// `con_fifo = false`: sin el FIFO de avisos inyectados (el gestor registra la presión en el archivo).
    pub fn iniciar_con(&mut self, con_fifo: bool) {
        let p = self.p().to_path_buf();
        let bin = p.join("bin");
        let log = std::fs::File::create(p.join("gestor.log")).unwrap();
        let mut c = Command::new(env!("CARGO_BIN_EXE_vortex-gestor-rs"));
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", p.join("home"))
            .env("XDG_RUNTIME_DIR", &self.run)
            .env("XDG_STATE_HOME", p.join("estado"))
            .env("XDG_CONFIG_HOME", p.join("conf"))
            .env("VORTEX_GESTOR_PROC", &self.proc)
            .env("VORTEX_GESTOR_CGROOT", &self.cgr)
            .env("VORTEX_GESTOR_SCX", p.join("scx"))
            .env("VORTEX_GESTOR_PSI", p.join("psi"))
            .env("VORTEX_GESTOR_BUSCTL", bin.join("busctl"))
            .env("VORTEX_GESTOR_RENICE", bin.join("renice"))
            .env("VORTEX_GESTOR_VORTEX", bin.join("vortex"))
            .env("VORTEX_GESTOR_QS", bin.join("quickshell"))
            .env("VORTEX_GESTOR_MEMORIA", bin.join("vortex-memoria"))
            .env("VORTEX_GESTOR_GUARDADOS", bin.join("vortex-guardados"))
            .env("VORTEX_GESTOR_DISCO", bin.join("vortex-disco"))
            .env("VORTEX_GESTOR_NVIDIA_SMI", bin.join("nvidia-smi"))
            .env("VORTEX_GESTOR_IA", bin.join("vortex-ia"))
            .env("VORTEX_SHELL_QML", "/falso/Shell.qml")
            // lo que leen los programas falsos
            .env("FALSO_T", &p)
            .env("FALSO_PROC", &self.proc)
            .env("FALSO_CGR", &self.cgr)
            .env("FALSO_RAIZ", &self.raiz)
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log);
        if con_fifo {
            c.env("VORTEX_GESTOR_INYECTAR", &self.inyecta);
        }
        self.gestor = Some(c.spawn().unwrap());
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.p().join("gestor.log")).unwrap_or_default()
    }

    pub fn archivo(&self, nombre: &str) -> String {
        std::fs::read_to_string(self.p().join(nombre)).unwrap_or_default()
    }

    pub fn estado(&self) -> String {
        std::fs::read_to_string(self.estado_archivo()).unwrap_or_default()
    }

    /// ¿El archivo de estado tiene esa línea exacta?
    pub fn est(&self, linea: &str) -> bool {
        self.estado().lines().any(|l| l == linea)
    }

    pub fn en_log(&self, archivo: &str, texto: &str) -> bool {
        self.archivo(archivo).contains(texto)
    }

    /// Cuántas líneas del archivo contienen el texto (como `grep -c`).
    pub fn cuenta(&self, archivo: &str, texto: &str) -> usize {
        self.archivo(archivo)
            .lines()
            .filter(|l| l.contains(texto))
            .count()
    }

    pub fn linea_exacta(&self, archivo: &str, linea: &str) -> bool {
        self.archivo(archivo).lines().any(|l| l == linea)
    }

    /// Un proceso falso: su comm, su cgroup, su límite de nice y sus hilos (y los hijos que lo cuelgan).
    pub fn proceso(&self, pid: u32, comm: &str, cg: &str, nice_max: &str, hilos: &[u32], hijos: &[u32]) {
        let d = self.proc.join(pid.to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("comm"), format!("{comm}\n")).unwrap();
        std::fs::write(d.join("cgroup"), format!("0::{cg}\n")).unwrap();
        std::fs::write(
            d.join("limits"),
            format!("Limit                     Soft Limit           Hard Limit           Units\nMax nice priority         {nice_max}                   {nice_max}\n"),
        )
        .unwrap();
        let mut todos = vec![pid];
        todos.extend_from_slice(hilos);
        for t in todos {
            let td = d.join("task").join(t.to_string());
            std::fs::create_dir_all(&td).unwrap();
            std::fs::write(
                td.join("stat"),
                format!("{t} ({comm}) S 1 {pid} {pid} 0 -1 4194304 0 0 0 0 0 0 0 0 20 0 1 0 100 0 0\n"),
            )
            .unwrap();
            std::fs::write(td.join("children"), "").unwrap();
        }
        if !hijos.is_empty() {
            let h: String = hijos.iter().map(|h| format!("{h} ")).collect();
            std::fs::write(d.join("task").join(pid.to_string()).join("children"), h).unwrap();
        }
    }

    pub fn nice_hilo(&self, pid: u32, tid: u32) -> i32 {
        let s = std::fs::read_to_string(self.proc.join(format!("{pid}/task/{tid}/stat"))).unwrap();
        let resto = &s[s.rfind(") ").unwrap() + 2..];
        resto.split_whitespace().nth(16).unwrap().parse().unwrap()
    }

    pub fn cgroup_dir(&self, cg: &str) -> PathBuf {
        PathBuf::from(format!("{}{}", self.cgr.display(), cg))
    }

    /// El scope del gestor para esa clase y pid.
    pub fn scope(&self, clase: &str, pid: u32) -> PathBuf {
        self.cgroup_dir(&format!("{}/app.slice/app-vortex-{clase}-{pid}.scope", self.raiz))
    }

    pub fn leer(&self, p: &Path) -> String {
        std::fs::read_to_string(p).unwrap_or_default().trim().to_string()
    }

    /// Un proceso de IA falso: con su línea de comandos y un fd que no es de DRM.
    pub fn ia_proceso(&self, pid: u32, comm: &str, cg: &str, args: &[&str]) {
        self.proceso(pid, comm, cg, "30", &[], &[]);
        let mut b = Vec::new();
        for a in args {
            b.extend_from_slice(a.as_bytes());
            b.push(0);
        }
        std::fs::write(self.proc.join(format!("{pid}/cmdline")), b).unwrap();
        std::fs::create_dir_all(self.proc.join(format!("{pid}/fdinfo"))).unwrap();
        std::fs::write(self.proc.join(format!("{pid}/fdinfo/0")), "pos:\t0\nflags:\t02\n").unwrap();
    }

    pub fn quitar_proceso(&self, pid: u32) {
        let _ = std::fs::remove_dir_all(self.proc.join(pid.to_string()));
    }

    pub fn marca(&self, nombre: &str) -> PathBuf {
        self.p().join("estado/vortex").join(nombre)
    }

    pub fn poner_marca(&self, nombre: &str) {
        std::fs::create_dir_all(self.p().join("estado/vortex")).unwrap();
        std::fs::write(self.marca(nombre), "").unwrap();
    }

    /// Inyecta un aviso de presión de memoria (en lugar del kernel).
    pub fn psi_memoria(&self) {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(&self.inyecta)
            .unwrap();
        f.write_all("psi\u{1f}memoria\n".as_bytes()).unwrap();
    }

    /// Espera a que el gestor termine y devuelve su código.
    pub fn esperar_salida(&mut self) -> Option<i32> {
        let g = self.gestor.as_mut()?;
        let fin = Instant::now() + Duration::from_secs(10);
        while Instant::now() < fin {
            if let Some(st) = g.try_wait().unwrap() {
                return st.code();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        None
    }

    pub fn pid_gestor(&self) -> u32 {
        self.gestor.as_ref().unwrap().id()
    }
}

impl Drop for Mundo {
    fn drop(&mut self) {
        if let Some(g) = self.gestor.as_mut() {
            let _ = g.kill();
            let _ = g.wait();
        }
    }
}
