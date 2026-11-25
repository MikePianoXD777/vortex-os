//! El gestor de recursos de Vortexine's OS (lo que hacía `bin/vortex-gestor`, de bash).
//!
//! Reparte solo la CPU y la RAM, sin que el usuario toque nada:
//!   1. La ventana enfocada tiene prioridad: su app va a su propio scope de systemd
//!      (`app-vortex-CLASE-PID.scope`, en app.slice) con más CPUWeight/IOWeight, y el resto de las apps
//!      baja. Con un planificador sched_ext (scx_lavd), que no usa los pesos de los cgroups, además sube un
//!      poco la prioridad (nice) de sus hilos, solo si el límite del sistema deja devolverla después.
//!   2. Modo juego automático: al abrirse un juego llama a «vortex modo-juego on» y pausa el fondo animado
//!      del shell; al cerrarse el último juego, lo deja todo como estaba.
//!   3. Presión de memoria: el kernel avisa (PSI) y el gestor le pide al shell que suelte memoria y comprime
//!      lo que las apps de fondo no usan (memory.reclaim).
//!   4. Modo IA automático (3.1), el hermano del modo juego. El modo juego manda. Miku nunca cuenta.
//!
//! Es un solo proceso dormido en poll(): nada se despierta si no pasa algo, no hay bucles con sleep y los
//! eventos de cada ventana no lanzan procesos (se lee /proc y se escribe en los cgroups directo).

use crate::config::{self, Ajustes, Rutas};
use crate::hyprland::{Evento, Oido, Vuelta};
use crate::ordenes::{Lanzador, existe_comando};
use crate::procesos::{self, Drm};
use crate::reabrir;
use crate::sis;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn log(m: &str) {
    println!("vortex-gestor: {m}");
}

fn ahora_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// `[[ a -nt b ]]`: a es más nueva que b, o a existe y b no.
fn mas_nueva(a: &Path, b: &Path) -> bool {
    match (mtime(a), mtime(b)) {
        (Some(x), Some(y)) => x > y,
        (Some(_), None) => true,
        _ => false,
    }
}

fn solo_digitos(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// La instancia de Hyprland: la del entorno si su socket existe; si no, la más reciente.
pub fn buscar_hyprland(run: &Path, firma_env: Option<&str>) -> Option<(PathBuf, String)> {
    use std::os::unix::fs::FileTypeExt;
    let es_socket = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.file_type().is_socket());
    if let Some(f) = firma_env.filter(|f| !f.is_empty()) {
        let d = run.join("hypr").join(f);
        if es_socket(&d.join(".socket2.sock")) {
            return Some((d, f.to_string()));
        }
    }
    let mut nombres: Vec<String> = std::fs::read_dir(run.join("hypr"))
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    nombres.sort();
    let mut mejor: Option<String> = None;
    for n in nombres {
        let s = run.join("hypr").join(&n).join(".socket2.sock");
        if !es_socket(&s) {
            continue;
        }
        let mejor_s = mejor
            .as_ref()
            .map(|m| run.join("hypr").join(m).join(".socket2.sock"));
        if mejor_s.is_none_or(|m| mas_nueva(&s, &m)) {
            mejor = Some(n);
        }
    }
    mejor.map(|m| (run.join("hypr").join(&m), m))
}

pub struct Gestor {
    a: Ajustes,
    r: Rutas,
    uid: u32,
    juegos_re: Regex,
    l: Lanzador,
    hypr_dir: PathBuf,
    firma: String,
    inicio: i64,
    foco_pid: u32,
    foco_clase: String,
    foco_cg: String,
    foco_nice: i32,
    /// scope creado por el gestor → pid de su app
    mis_cg: BTreeMap<String, u32>,
    /// scopes recién creados con el peso de foco en systemd
    normalizar: BTreeSet<String>,
    /// cgroup que no es del gestor → su cpu.weight original
    ajeno_peso: BTreeMap<String, String>,
    /// pids que no se pudieron mover (no se reintenta)
    no_mover: BTreeSet<u32>,
    /// dirección de ventana → clase, de los juegos abiertos
    juegos: BTreeMap<String, String>,
    /// cgroups de juegos (no se les quita memoria)
    juego_cg: BTreeSet<String>,
    juego_pid: BTreeSet<u32>,
    /// dirección de la ventana de un juego → su pid
    juego_de: BTreeMap<String, u32>,
    /// La clase del juego que abrió la partida mientras no se sabe su pid (ver `lanzar_ayudantes`).
    ayudantes_pendientes: Option<String>,
    nice_orig: BTreeMap<String, i32>,
    nice_vistos: BTreeSet<String>,
    nice_raiz: Option<u32>,
    nice_base: i32,
    modo_propio: bool,
    fondo_pausado: bool,
    primer_foco: bool,
    mover: bool,
    fallos_mover: u32,
    psi_estado: String,
    psi_eventos: u64,
    psi_ultimo: i64,
    psi_texto: String,
    reclamos: u64,
    aviso_memoria: i64,
    avisos_memoria: u64,
    shell_soltar: String,
    shell_instancia: Option<PathBuf>,
    ia_activa: bool,
    ia_propio: bool,
    ia_pid: u32,
    ia_nombre: String,
    ia_cg: String,
    ia_ultimo: i64,
    ia_sin_ver: i64,
    ia_callado: bool,
    disco_ultimo: i64,
    terminando: bool,
    /// Se está compartiendo la pantalla (4.4): las notificaciones se callan.
    compartiendo: bool,
    /// 4.5: dirección de cada ventana abierta → (clase, cuándo se abrió), para reabrir lo cerrado.
    ventanas: BTreeMap<String, (String, i64)>,
}

impl Gestor {
    pub fn nuevo(a: Ajustes, r: Rutas, hypr_dir: PathBuf, firma: String) -> Gestor {
        let mut l = Lanzador::nuevo();
        l.firma.clone_from(&firma);
        Gestor {
            juegos_re: a.regex_juegos(),
            a,
            uid: sis::uid(),
            r,
            l,
            hypr_dir,
            firma,
            inicio: ahora_s(),
            foco_pid: 0,
            foco_clase: String::new(),
            foco_cg: String::new(),
            foco_nice: 0,
            mis_cg: BTreeMap::new(),
            normalizar: BTreeSet::new(),
            ajeno_peso: BTreeMap::new(),
            no_mover: BTreeSet::new(),
            juegos: BTreeMap::new(),
            juego_cg: BTreeSet::new(),
            juego_pid: BTreeSet::new(),
            juego_de: BTreeMap::new(),
            ayudantes_pendientes: None,
            nice_orig: BTreeMap::new(),
            nice_vistos: BTreeSet::new(),
            nice_raiz: None,
            nice_base: 0,
            modo_propio: false,
            fondo_pausado: false,
            primer_foco: true,
            mover: true,
            fallos_mover: 0,
            psi_estado: "vigilada".into(),
            psi_eventos: 0,
            psi_ultimo: 0,
            psi_texto: String::new(),
            reclamos: 0,
            aviso_memoria: 0,
            avisos_memoria: 0,
            shell_soltar: "sin probar".into(),
            shell_instancia: None,
            ia_activa: false,
            ia_propio: false,
            ia_pid: 0,
            ia_nombre: String::new(),
            ia_cg: String::new(),
            ia_ultimo: 0,
            ia_sin_ver: 0,
            ia_callado: false,
            disco_ultimo: 0,
            terminando: false,
            compartiendo: false,
            ventanas: BTreeMap::new(),
        }
    }

    fn sock2(&self) -> PathBuf {
        self.hypr_dir.join(".socket2.sock")
    }

    // ------------------------------------------------------------------------------------ estado

    fn modo_juego_activo(&self) -> bool {
        self.r.marca_juego.exists() && mas_nueva(&self.r.marca_juego, &self.sock2())
    }

    fn modo_ia_activo(&self) -> bool {
        self.r.marca_ia.exists() && mas_nueva(&self.r.marca_ia, &self.sock2())
    }

    fn guardar_estado(&self) {
        if !self.r.estado_dir.is_dir() && std::fs::create_dir_all(&self.r.estado_dir).is_err() {
            return;
        }
        let modo = if self.modo_propio {
            "propio"
        } else if self.modo_juego_activo() {
            "ajeno"
        } else {
            "apagado"
        };
        let ia = if self.ia_propio {
            "propio"
        } else if self.modo_ia_activo() {
            "ajeno"
        } else {
            "apagado"
        };
        let t = format!(
            "inicio={}\npid={}\nfoco_pid={}\nfoco_clase={}\nfoco_cgroup={}\nfoco_nice={}\n\
             apps={}\njuegos={}\nmodo_juego={}\nfondo_pausado={}\n\
             psi={}\npsi_eventos={}\npsi_ultimo={}\npsi_valor={}\nreclamos={}\nshell_soltar={}\navisos_memoria={}\n\
             modo_ia={}\nia_programa={}\nia_pid={}\nia_cgroup={}\ncompartiendo={}\n",
            self.inicio,
            std::process::id(),
            self.foco_pid,
            self.foco_clase,
            self.foco_cg,
            self.foco_nice,
            self.mis_cg.len(),
            self.juegos.len(),
            modo,
            u8::from(self.fondo_pausado),
            self.psi_estado,
            self.psi_eventos,
            self.psi_ultimo,
            self.psi_texto,
            self.reclamos,
            self.shell_soltar,
            self.avisos_memoria,
            ia,
            self.ia_nombre,
            self.ia_pid,
            self.ia_cg,
            u8::from(self.compartiendo),
        );
        let _ = std::fs::write(&self.r.estado, t);
    }

    /// Si el gestor se reinicia con un juego abierto, recupera lo que había hecho.
    fn recuperar(&mut self) {
        let Ok(t) = std::fs::read_to_string(&self.r.estado) else {
            return;
        };
        for l in t.lines().map(str::trim) {
            if l == "modo_juego=propio" {
                if self.modo_juego_activo() {
                    self.modo_propio = true;
                }
            } else if l == "fondo_pausado=1" {
                self.fondo_pausado = true;
            } else if l == "modo_ia=propio" {
                if self.modo_ia_activo() {
                    self.ia_propio = true;
                    self.ia_activa = true;
                }
            } else if let Some(v) = l.strip_prefix("ia_pid=") {
                self.ia_pid = v.parse().unwrap_or(0);
            } else if let Some(v) = l.strip_prefix("ia_programa=") {
                self.ia_nombre = v.to_string();
            }
        }
    }

    // ------------------------------------------------------------------------------------ cgroups

    /// Pone el peso (CPU y, si el cgroup lo tiene, disco) escribiendo directo en el cgroup: no lanza
    /// procesos. Si systemd vuelve a aplicar sus propiedades, queda su valor, que es el de fondo.
    fn poner_peso(&self, cg: &str, peso: u64) -> bool {
        if !self.r.del_usuario(cg) {
            return false;
        }
        let base = self.r.cg(cg);
        let cpu = base.join("cpu.weight");
        if !sis::se_puede_escribir(&cpu) || escribir(&cpu, &format!("{peso}\n")).is_err() {
            return false;
        }
        let io = base.join("io.weight");
        if sis::se_puede_escribir(&io) {
            let _ = escribir(&io, &format!("default {peso}\n"));
        }
        true
    }

    /// Mueve la app (el proceso de la ventana y sus hijos que estaban con él) a su propio scope, ya con el
    /// peso de foco. Devuelve el cgroup del scope.
    fn crear_scope(&mut self, pid: u32, clase: &str, origen: &str) -> Option<String> {
        let pids: Vec<u32> = procesos::arbol(&self.r.proc, pid)
            .into_iter()
            .filter(|&p| procesos::cgroup_de(&self.r.proc, p).as_deref() == Some(origen))
            .collect();
        if pids.is_empty() {
            return None;
        }
        let limpia: String = clase
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .take(48)
            .collect();
        let unidad = format!(
            "app-vortex-{}-{pid}.scope",
            if limpia.is_empty() { "app" } else { &limpia }
        );
        // Si un hijo terminó justo antes, systemd rechaza la lista entera: se reintenta solo con el
        // proceso de la ventana (los hijos nuevos ya nacen en el scope).
        let bien = self.scope_systemd(&unidad, clase, &pids)
            || (pids.len() > 1 && self.scope_systemd(&unidad, clase, &[pid]));
        if !bien {
            self.no_mover.insert(pid);
            self.fallos_mover += 1;
            if self.fallos_mover >= 3 {
                self.mover = false;
                log("systemd no me deja mover las apps a su scope; sigo sin moverlas");
            }
            return None;
        }
        self.fallos_mover = 0;
        let nuevo = format!("{}/app.slice/{unidad}", self.r.raiz_usuario);
        self.mis_cg.insert(nuevo.clone(), pid);
        self.normalizar.insert(nuevo.clone());
        Some(nuevo)
    }

    /// Pide a systemd (del usuario) un scope nuevo en app.slice con esos procesos. Si están en el scope de
    /// la sesión, systemd le pide al del sistema que los mueva (es lo mismo que hace
    /// «systemd-run --user --scope»).
    /// Vortex 4.0: TimeoutStopSec=5s, para que «Cerrar por completo» no espere 90 s.
    /// Vortex 2.6.3: OOMPolicy=continue: si el kernel mata UN proceso de la app por falta de memoria,
    /// systemd no cierra la app entera.
    fn scope_systemd(&mut self, unidad: &str, clase: &str, pids: &[u32]) -> bool {
        let mut args: Vec<String> = [
            "--user",
            "call",
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StartTransientUnit",
            "ssa(sv)a(sa(sv))",
            unidad,
            "fail",
            "7",
            "PIDs",
            "au",
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        args.push(pids.len().to_string());
        args.extend(pids.iter().map(ToString::to_string));
        let peso = self.a.peso_foco.to_string();
        let descripcion = format!("Vortex: {}", if clase.is_empty() { "app" } else { clase });
        for p in [
            "Slice",
            "s",
            "app.slice",
            "Description",
            "s",
            &descripcion,
            "CPUWeight",
            "t",
            &peso,
            "IOWeight",
            "t",
            &peso,
            "OOMPolicy",
            "s",
            "continue",
            "TimeoutStopUSec",
            "t",
            "5000000",
            "0",
        ] {
            args.push(p.to_string());
        }
        let busctl = self.r.busctl.clone();
        self.l.ejecutar(&busctl, &args, None)
    }

    /// Cambia propiedades de un scope del gestor en systemd (solo en memoria).
    fn propiedades(&mut self, cg: &str, peso: u64) -> bool {
        let unidad = cg.rsplit('/').next().unwrap_or(cg);
        let p = peso.to_string();
        let args: Vec<String> = [
            "--user",
            "call",
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "SetUnitProperties",
            "sba(sv)",
            unidad,
            "true",
            "2",
            "CPUWeight",
            "t",
            &p,
            "IOWeight",
            "t",
            &p,
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        let busctl = self.r.busctl.clone();
        self.l.ejecutar(&busctl, &args, None)
    }

    /// ¿La app tiene que ir a su propio scope? Sí si está en el scope de la sesión (todo lo que se abre
    /// desde Hyprland cae ahí) o heredó el scope de otra app del gestor (un juego que abrió Steam, un
    /// programa con ventana abierto desde la terminal).
    fn necesita_scope(&self, pid: u32, cg: &str) -> bool {
        if !self.mover || self.no_mover.contains(&pid) {
            return false;
        }
        let n = cg.rsplit('/').next().unwrap_or(cg);
        if n.starts_with("session-") && n.ends_with(".scope") {
            return true;
        }
        let propio = format!("-{pid}.scope");
        if n.len() >= "app-vortex-".len() + propio.len()
            && n.starts_with("app-vortex-")
            && n.ends_with(&propio)
        {
            return false;
        }
        n.starts_with("app-vortex-") && n.ends_with(".scope")
    }

    // ------------------------------------------------------------------------------------ nice

    fn scx_activo(&self) -> bool {
        procesos::primera_linea(&self.r.scx).as_deref() == Some("enabled")
    }

    fn renice(&mut self, valor: i32, tids: &[String]) {
        if let Some(prog) = self.r.renice.clone() {
            let mut args = vec!["--priority".to_string(), valor.to_string(), "-p".to_string()];
            args.extend(tids.iter().cloned());
            self.l.ejecutar(&prog, &args, None);
        } else {
            for t in tids.iter().filter_map(|t| t.parse::<u32>().ok()) {
                let _ = sis::poner_nice(t, valor);
            }
        }
    }

    fn dar_nice(&mut self, pid: u32) {
        let nf = self.a.nice_foco;
        if !(nf < 0 && self.scx_activo() && procesos::nice_permitido(&self.r.proc, pid, nf)) {
            return;
        }
        self.nice_orig.clear();
        self.nice_vistos.clear();
        self.nice_base = 0;
        if let Some(n) = procesos::nice_de(
            &self
                .r
                .proc
                .join(pid.to_string())
                .join("task")
                .join(pid.to_string()),
        ) {
            self.nice_base = n;
        }
        let mut tids: Vec<String> = Vec::new();
        for p in procesos::arbol(&self.r.proc, pid) {
            for tid in procesos::hilos(&self.r.proc, p) {
                self.nice_vistos.insert(tid.clone());
                let dir = self.r.proc.join(p.to_string()).join("task").join(&tid);
                let Some(n) = procesos::nice_de(&dir) else {
                    continue;
                };
                if n > nf {
                    self.nice_orig.insert(tid.clone(), n);
                    tids.push(tid);
                }
            }
        }
        if tids.is_empty() {
            return;
        }
        self.renice(nf, &tids);
        self.nice_raiz = Some(pid);
        self.foco_nice = nf;
    }

    /// Devuelve el nice a los hilos que se cambiaron, y a los que nacieron después con él.
    fn quitar_nice(&mut self) {
        let Some(raiz) = self.nice_raiz else {
            return;
        };
        let nf = self.a.nice_foco;
        let mut por_valor: BTreeMap<i32, Vec<String>> = BTreeMap::new();
        for p in procesos::arbol(&self.r.proc, raiz) {
            for tid in procesos::hilos(&self.r.proc, p) {
                let dir = self.r.proc.join(p.to_string()).join("task").join(&tid);
                if procesos::nice_de(&dir) != Some(nf) {
                    continue;
                }
                let v = if let Some(v) = self.nice_orig.get(&tid) {
                    *v
                } else if !self.nice_vistos.contains(&tid) {
                    self.nice_base
                } else {
                    continue;
                };
                por_valor.entry(v).or_default().push(tid);
            }
        }
        for (v, tids) in por_valor {
            self.renice(v, &tids);
        }
        self.nice_raiz = None;
        self.nice_orig.clear();
        self.nice_vistos.clear();
    }

    // ------------------------------------------------------------------------------------ prioridad

    /// Le da el peso de foco a la app de un proceso: la pasa a su propio scope si hace falta o se lo pone en
    /// el cgroup donde ya está. Devuelve ese cgroup. Lo usan la ventana enfocada y la IA del modo IA.
    fn subir(&mut self, pid: u32, clase: &str) -> Option<String> {
        if !procesos::apto(&self.r.proc, pid) {
            return None;
        }
        let cg = procesos::cgroup_de(&self.r.proc, pid)?;
        let mut subido = None;
        if self.necesita_scope(pid, &cg) {
            subido = self.crear_scope(pid, clase, &cg);
        } else if self.r.del_usuario(&cg) {
            if !self.mis_cg.contains_key(&cg) && !self.ajeno_peso.contains_key(&cg) {
                // Un cgroup que no creó el gestor (flatpak, otra app): se guarda su peso para devolverlo.
                if let Some(peso) = procesos::primera_linea(&self.r.cg(&cg).join("cpu.weight"))
                    && !peso.is_empty()
                    && peso.chars().all(|c| c.is_ascii_digit())
                {
                    self.ajeno_peso.insert(cg.clone(), peso);
                }
            }
            if (self.mis_cg.contains_key(&cg) || self.ajeno_peso.contains_key(&cg))
                && self.poner_peso(&cg, self.a.peso_foco)
            {
                subido = Some(cg);
            }
        }
        subido
    }

    fn dar_prioridad(&mut self, pid: u32, clase: &str) {
        if !procesos::apto(&self.r.proc, pid) || procesos::cgroup_de(&self.r.proc, pid).is_none() {
            return;
        }
        if let Some(s) = self.subir(pid, clase) {
            self.foco_cg = s;
        }
        if !self.foco_cg.is_empty() && self.juego_pid.contains(&pid) {
            self.juego_cg.insert(self.foco_cg.clone());
        }
        self.dar_nice(pid);
    }

    fn quitar_prioridad(&mut self) {
        self.quitar_nice();
        self.foco_nice = 0;
        let cg = self.foco_cg.clone();
        self.soltar_cg(&cg);
    }

    /// Devuelve al peso de fondo (o al suyo, si no lo creó el gestor) el cgroup de una app que deja de
    /// tener prioridad. El de la IA, mientras siga el modo IA, se queda con la suya.
    fn soltar_cg(&mut self, cg: &str) {
        if cg.is_empty() || (self.ia_activa && cg == self.ia_cg) {
            return;
        }
        if self.mis_cg.contains_key(cg) {
            if self.normalizar.contains(cg) {
                // Recién creado: en systemd tiene el peso de foco; se le deja el de fondo (una vez;
                // después basta con escribir en el cgroup).
                if !self.propiedades(cg, self.a.peso_fondo) {
                    self.mis_cg.remove(cg);
                    self.juego_cg.remove(cg);
                }
                self.normalizar.remove(cg);
            } else if self.r.cg(cg).is_dir() {
                self.poner_peso(cg, self.a.peso_fondo);
            } else {
                // La app ya se cerró y systemd quitó su scope.
                self.mis_cg.remove(cg);
                self.juego_cg.remove(cg);
            }
        } else if let Some(p) = self.ajeno_peso.remove(cg)
            && let Ok(n) = p.parse::<u64>()
        {
            self.poner_peso(cg, n);
        }
    }

    // ------------------------------------------------------------------------------------ juegos

    fn es_juego(&self, clase: &str, tipo: &str) -> bool {
        tipo == "game" || (!clase.is_empty() && self.juegos_re.is_match(clase))
    }

    fn fondo(&mut self, accion: &str) -> bool {
        let qml = procesos::shell_qml_activo(
            &self.r.proc,
            self.uid,
            self.r.shell_qml_env.as_deref(),
            config::SHELL_QML,
        );
        let args: Vec<String> = [
            "ipc",
            "-p",
            &qml,
            "call",
            "wallpaper",
            "setPlayback",
            "all",
            accion,
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        let qs = self.r.qs.clone();
        self.l.ejecutar(&qs, &args, Some(Duration::from_secs(5)))
    }

    fn empezar_juego(&mut self, clase: &str, pid: u32) {
        log(&format!("juego abierto ({clase}): modo juego"));
        // Si ya estaba puesto, lo puso otro (gamemode o Super+G) y que lo quite él; salvo que lo haya puesto
        // este gestor antes de reiniciarse (modo_propio recuperado).
        if !self.modo_juego_activo() {
            let vortex = self.r.vortex.clone();
            if self.l.ejecutar(
                &vortex,
                &["modo-juego".into(), "on".into()],
                Some(Duration::from_secs(20)),
            ) {
                self.modo_propio = true;
            }
        }
        // Con el juego delante el fondo no se ve: pausarlo no cambia nada de lo que se ve.
        if self.fondo("pause") {
            self.fondo_pausado = true;
        }
        // Los ayudantes necesitan el pid del juego. Si la partida empezó por «openwindow», todavía no se sabe (llega
        // con el foco, un instante después): esperan a ese foco. Antes se les pasaba 0 y medían a ciegas.
        if pid > 1 {
            self.lanzar_ayudantes(clase, pid);
        } else {
            self.ayudantes_pendientes = Some(clase.to_string());
        }
        // El modo juego manda: el modo IA se quita (después de poner el del juego, para que los efectos y el
        // fondo no vuelvan entre uno y otro) y regresa callado cuando se cierre el juego.
        if self.ia_activa {
            self.ia_callado = true;
            self.terminar_ia("abriste un juego: manda el modo juego");
        }
    }

    /// Una vez por partida, con el pid del juego que la empezó.
    fn lanzar_ayudantes(&mut self, clase: &str, pid: u32) {
        // 3.0: si al juego le queda poca memoria, ofrece cerrar lo que más gasta (lo decide vortex-memoria).
        let memoria = self.r.memoria.clone();
        self.l
            .segundo_plano(&memoria, &["--juego".into(), clase.into(), pid.to_string()]);
        // 4.0: la copia de tus partidas guardadas, antes de jugar (solo si cambiaron; sin avisos).
        let guardados = self.r.guardados.clone();
        self.l
            .segundo_plano(&guardados, &["antes".into(), clase.into(), pid.to_string()]);
        // 4.0: el detector de tirones mide la partida (lo que trabó al juego sale en el resumen del final).
        let tirones = self.r.tirones.clone();
        self.l
            .segundo_plano(&tirones, &["medir".into(), pid.to_string(), clase.into()]);
    }

    fn terminar_juego(&mut self) {
        log("ya no hay juegos abiertos: todo como estaba");
        self.ayudantes_pendientes = None;
        // Solo se quita si lo puso el gestor y sigue puesto (con Super+G se pudo quitar antes).
        if self.modo_propio && self.r.marca_juego.exists() {
            let vortex = self.r.vortex.clone();
            self.l.ejecutar(
                &vortex,
                &["modo-juego".into(), "off".into()],
                Some(Duration::from_secs(20)),
            );
        }
        self.modo_propio = false;
        // Si el modo juego sigue puesto (lo puso otro, p. ej. Super+G), el fondo sigue en pausa:
        // «vortex modo-juego off» lo reanuda cuando se quite.
        if self.fondo_pausado && !self.modo_juego_activo() && !self.modo_ia_activo() {
            self.fondo("play");
        }
        self.fondo_pausado = false;
        // Si la IA seguía trabajando, el modo IA vuelve en la siguiente vuelta del ciclo, sin esperar.
        self.ia_ultimo = 0;
    }

    fn agregar_juego(&mut self, dir: &str, clase: &str) {
        if self.juegos.contains_key(dir) {
            return;
        }
        self.juegos.insert(dir.to_string(), clase.to_string());
        if self.juegos.len() == 1 {
            let pid = self.juego_de.get(dir).copied().unwrap_or(0);
            self.empezar_juego(clase, pid);
        }
    }

    // ------------------------------------------------------------------------------------ memoria

    fn leer_psi(&mut self) {
        self.psi_texto.clear();
        let Ok(t) = std::fs::read_to_string(&self.r.psi) else {
            return;
        };
        let partes: Vec<&str> = t
            .lines()
            .map(|l| l.split(" total=").next().unwrap_or(l))
            .collect();
        self.psi_texto = partes.join(", ");
    }

    /// Si el shell sabe soltar memoria (función soltarMemoria en su IPC «vortex»), se le pide. Se pregunta
    /// una vez por cada arranque del shell.
    fn soltar_shell(&mut self) {
        let mut inst: Option<PathBuf> = None;
        let mut ids: Vec<PathBuf> = std::fs::read_dir(self.r.run.join("quickshell/by-id"))
            .map(|r| {
                r.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        for d in ids {
            if inst.as_ref().is_none_or(|i| mas_nueva(&d, i)) {
                inst = Some(d);
            }
        }
        let Some(inst) = inst else {
            self.shell_soltar = "shell apagado".into();
            return;
        };
        let qml = procesos::shell_qml_activo(
            &self.r.proc,
            self.uid,
            self.r.shell_qml_env.as_deref(),
            config::SHELL_QML,
        );
        let qs = self.r.qs.clone();
        if self.shell_instancia.as_ref() != Some(&inst) {
            self.shell_instancia = Some(inst);
            let args: Vec<String> = ["ipc", "-p", &qml, "show"]
                .iter()
                .map(ToString::to_string)
                .collect();
            let sal = self
                .l
                .salida(&qs, &args, Duration::from_secs(5))
                .unwrap_or_default();
            let (mut objetivo, mut hay) = (String::new(), false);
            for linea in String::from_utf8_lossy(&sal).lines().map(str::trim) {
                if let Some(o) = linea.strip_prefix("target ") {
                    objetivo = o.to_string();
                }
                if objetivo == "vortex" && linea.contains("function soltarMemoria") {
                    hay = true;
                }
            }
            self.shell_soltar = if hay { "sí" } else { "no sabe" }.into();
        }
        if self.shell_soltar != "sí" {
            return;
        }
        let args: Vec<String> = ["ipc", "-p", &qml, "call", "vortex", "soltarMemoria"]
            .iter()
            .map(ToString::to_string)
            .collect();
        self.l.segundo_plano_con_plazo(&qs, &args, Duration::from_secs(5));
    }

    /// Comprime (a zram) lo que no usan las apps de fondo que ordenó el gestor; nunca la enfocada, ni un
    /// juego, ni la IA del modo IA. Va en segundo plano: el kernel puede tardar un poco.
    fn reclamar_fondo(&mut self) {
        if self.a.reclamar_mb <= 0 {
            return;
        }
        let mut archivos: Vec<PathBuf> = Vec::new();
        for cg in self.mis_cg.keys().cloned().collect::<Vec<_>>() {
            if cg == self.foco_cg || cg == self.ia_cg || self.juego_cg.contains(&cg) {
                continue;
            }
            if !self.r.cg(&cg).is_dir() {
                self.mis_cg.remove(&cg);
                self.normalizar.remove(&cg);
                continue;
            }
            let f = self.r.cg(&cg).join("memory.reclaim");
            if sis::se_puede_escribir(&f) {
                archivos.push(f);
            }
        }
        if archivos.is_empty() {
            return;
        }
        self.reclamos += 1;
        let texto = format!("{}M\n", self.a.reclamar_mb);
        std::thread::spawn(move || {
            for f in archivos {
                let _ = escribir(&f, &texto);
            }
        });
    }

    fn avisar_memoria(&mut self) {
        if !self.juegos.is_empty() || self.modo_juego_activo() {
            return;
        }
        let ahora = ahora_s();
        if ahora - self.aviso_memoria < self.a.aviso_enfriar_s || !procesos::memoria_baja(&self.r.proc) {
            return;
        }
        self.aviso_memoria = ahora;
        self.avisos_memoria += 1;
        log("queda poca memoria: se avisa");
        let memoria = self.r.memoria.clone();
        self.l.segundo_plano(&memoria, &[]);
        self.guardar_estado();
    }

    fn revisar_disco(&mut self) {
        if !self.juegos.is_empty() || self.modo_juego_activo() {
            return;
        }
        let ahora = ahora_s();
        if ahora - self.disco_ultimo < self.a.disco_revisar_s {
            return;
        }
        self.disco_ultimo = ahora;
        let disco = self.r.disco.clone();
        self.l.segundo_plano(&disco, &["revisar".into()]);
    }

    // ------------------------------------------------------------------------------------ modo IA

    fn hay_nvidia(&self) -> bool {
        self.r.proc.join("driver/nvidia").is_dir() && existe_comando(&self.r.nvsmi)
    }

    /// ¿Alguno de los hallados usa la gráfica de verdad? Devuelve el que más usa. Con NVIDIA lo dice
    /// nvidia-smi (CUDA en las apps de cómputo; Vulkan solo sale con «-q»). En AMD e Intel, el fdinfo de DRM;
    /// sin permiso para leerlo (un Ollama del sistema, de otro usuario), con parecer IA basta.
    fn confirmar_ia(&mut self, hallados: &BTreeMap<u32, &'static str>) -> Option<(u32, &'static str)> {
        let mut vram: BTreeMap<u32, Option<i64>> = BTreeMap::new();
        if self.hay_nvidia() {
            let nv = self.r.nvsmi.clone();
            let args = [
                "--query-compute-apps=pid,used_memory".to_string(),
                "--format=csv,noheader,nounits".to_string(),
            ];
            let sal = self
                .l
                .salida(&nv, &args, Duration::from_secs(10))
                .unwrap_or_default();
            for l in String::from_utf8_lossy(&sal).lines() {
                let mut c = l.split([',', ' ']).filter(|x| !x.is_empty());
                let (Some(pid), mb) = (c.next().and_then(|p| p.parse::<u32>().ok()), c.next()) else {
                    continue;
                };
                vram.insert(pid, solo_digitos(mb.unwrap_or("")).parse().ok());
            }
            if hallados.keys().any(|p| !vram.contains_key(p)) {
                let args = ["-q".to_string(), "-d".to_string(), "PIDS".to_string()];
                let sal = self
                    .l
                    .salida(&nv, &args, Duration::from_secs(10))
                    .unwrap_or_default();
                let mut pid: Option<u32> = None;
                for l in String::from_utf8_lossy(&sal).lines().map(str::trim) {
                    if l.starts_with("Process ID") {
                        pid = l.rsplit(": ").next().and_then(|p| p.parse().ok());
                    } else if l.starts_with("Used GPU Memory")
                        && let Some(p) = pid
                    {
                        vram.entry(p).or_insert_with(|| solo_digitos(l).parse().ok());
                    }
                }
            }
        }
        let umbral = self.a.ia_vram_mb;
        let mut mejor = -1i64;
        let mut elegido = None;
        for (&pid, &nombre) in hallados {
            let mb = if let Some(v) = vram.get(&pid) {
                v.unwrap_or(umbral) // sale en la gráfica sin decir cuánto («[N/A]»): cuenta
            } else {
                let fdinfo = self.r.proc.join(pid.to_string()).join("fdinfo");
                if fdinfo.is_dir() && !sis::se_puede_leer(&fdinfo) {
                    umbral
                } else {
                    match procesos::drm_mb(&self.r.proc, pid) {
                        Drm::No => continue,
                        Drm::Incierto => umbral,
                        Drm::Mb(m) => m,
                    }
                }
            };
            if mb >= umbral && mb > mejor {
                mejor = mb;
                elegido = Some((pid, nombre));
            }
        }
        elegido
    }

    fn empezar_ia(&mut self, pid: u32, nombre: &str) {
        self.ia_activa = true;
        self.ia_pid = pid;
        self.ia_nombre = nombre.to_string();
        self.ia_sin_ver = 0;
        self.ia_cg.clear();
        log(&format!("{nombre} usa la gráfica (pid {pid}): modo IA"));
        if let Some(cg) = self.subir(pid, &format!("ia-{nombre}")) {
            self.ia_cg = cg;
        }
        // Si ya estaba puesto, lo puso otro (a mano) y que lo quite él.
        if !self.modo_ia_activo() {
            let mut args = vec!["modo-ia".to_string(), "on".to_string(), nombre.to_string()];
            if self.ia_callado {
                args.push("--sin-aviso".into()); // vuelve después de un juego: ya se avisó
            }
            let vortex = self.r.vortex.clone();
            if self.l.ejecutar(&vortex, &args, Some(Duration::from_secs(20))) {
                self.ia_propio = true;
            }
        }
        self.ia_callado = false;
        // vortex-ia (si está) avisa si el modelo no cabe en la memoria de la gráfica.
        if existe_comando(&self.r.ia) {
            let ia = self.r.ia.clone();
            self.l.segundo_plano(&ia, &["vigilar".into()]);
        }
        self.guardar_estado();
    }

    /// Otro proceso de IA (Ollama cambió de modelo): la prioridad pasa al nuevo, sin avisar otra vez.
    fn cambiar_ia(&mut self, pid: u32, nombre: &str) {
        let viejo = std::mem::take(&mut self.ia_cg);
        self.ia_pid = pid;
        self.ia_nombre = nombre.to_string();
        if let Some(cg) = self.subir(pid, &format!("ia-{nombre}")) {
            self.ia_cg = cg;
        }
        if viejo != self.ia_cg && viejo != self.foco_cg {
            self.soltar_cg(&viejo);
        }
        self.guardar_estado();
    }

    fn terminar_ia(&mut self, motivo: &str) {
        let cg = self.ia_cg.clone();
        log(&format!("modo IA fuera: {motivo}"));
        self.ia_activa = false;
        if cg != self.foco_cg {
            self.soltar_cg(&cg);
        }
        // Solo se quita si lo puso el gestor y sigue puesto (se pudo quitar a mano antes).
        if self.ia_propio && self.r.marca_ia.exists() {
            let vortex = self.r.vortex.clone();
            self.l.ejecutar(
                &vortex,
                &["modo-ia".into(), "off".into()],
                Some(Duration::from_secs(20)),
            );
        }
        self.ia_propio = false;
        self.ia_pid = 0;
        self.ia_nombre.clear();
        self.ia_cg.clear();
        self.ia_sin_ver = 0;
        self.guardar_estado();
    }

    fn revisar_ia(&mut self) {
        if self.a.ia_revisar_s <= 0 {
            return;
        }
        let ahora = ahora_s();
        if ahora - self.ia_ultimo < self.a.ia_revisar_s {
            return;
        }
        self.ia_ultimo = ahora;
        // El modo juego manda (también el puesto a mano con Super+G o por gamemode).
        if !self.juegos.is_empty() || self.modo_juego_activo() {
            if self.ia_activa {
                self.ia_callado = true;
                self.terminar_ia("hay un juego: manda el modo juego");
            }
            return;
        }
        let hallados = procesos::buscar_ia(&self.r.proc);
        let confirmada = if hallados.is_empty() {
            None
        } else {
            self.confirmar_ia(&hallados)
        };
        if let Some((pid, nombre)) = confirmada {
            self.ia_sin_ver = 0;
            if !self.ia_activa {
                self.empezar_ia(pid, nombre);
            } else if pid != self.ia_pid {
                self.cambiar_ia(pid, nombre);
            }
        } else if self.ia_activa {
            self.ia_sin_ver += 1;
            if self.ia_sin_ver >= self.a.ia_faltas {
                let n = if self.ia_nombre.is_empty() {
                    "la IA"
                } else {
                    &self.ia_nombre
                };
                let motivo = format!("{n} ya no usa la gráfica");
                self.terminar_ia(&motivo);
            }
        }
    }

    // ------------------------------------------------------------------------------------ eventos

    fn en_foco(&mut self, pid: u32, dir: &str, tipo: &str, clase: &str) {
        if self.es_juego(clase, tipo) {
            if pid > 1 {
                self.juego_pid.insert(pid);
            }
            if !dir.is_empty() {
                self.juego_de.insert(dir.to_string(), pid);
                self.agregar_juego(dir, clase);
            }
            if pid > 1
                && let Some(c) = self.ayudantes_pendientes.take()
            {
                self.lanzar_ayudantes(&c, pid);
            }
        }
        // El primer foco llega después de las ventanas que ya estaban abiertas: si el gestor anterior dejó
        // el modo juego o el fondo pausado y ya no hay juegos, se deshace.
        if self.primer_foco {
            self.primer_foco = false;
            if self.juegos.is_empty() && (self.modo_propio || self.fondo_pausado) {
                self.terminar_juego();
            }
        }
        if pid == self.foco_pid {
            self.guardar_estado();
            return;
        }
        self.quitar_prioridad();
        self.foco_pid = pid;
        self.foco_clase = clase.to_string();
        self.foco_cg.clear();
        if pid > 1 {
            self.dar_prioridad(pid, clase);
        }
        if self.a.depurar {
            let flecha = if self.foco_cg.is_empty() {
                String::new()
            } else {
                format!("→ {}", self.foco_cg.rsplit('/').next().unwrap_or(""))
            };
            log(&format!(
                "foco: {} (pid {pid}) {flecha}",
                if clase.is_empty() { "nada" } else { clase }
            ));
        }
        self.guardar_estado();
    }

    fn en_abre(&mut self, dir: &str, tipo: &str, clase: &str) {
        self.ventanas
            .insert(dir.to_string(), (clase.to_string(), ahora_s()));
        if self.es_juego(clase, tipo) {
            self.agregar_juego(dir, clase);
        }
        self.guardar_estado();
    }

    fn en_cierra(&mut self, dir: &str) {
        // 4.5: si era la última ventana de su app y vivió unos segundos, se anota para reabrirla.
        if let Some((clase, desde)) = self.ventanas.remove(dir) {
            let quedan = self.ventanas.values().any(|(c, _)| *c == clase);
            if !quedan && ahora_s() - desde >= reabrir::VIDA_MINIMA {
                reabrir::anotar(&self.r.estado_dir, &clase);
            }
        }
        if self.juegos.remove(dir).is_none() {
            return;
        }
        if let Some(p) = self.juego_de.remove(dir) {
            self.juego_pid.remove(&p);
        }
        if self.juegos.is_empty() {
            self.terminar_juego();
        }
        self.guardar_estado();
    }

    fn en_psi_memoria(&mut self) {
        self.psi_eventos += 1;
        let ahora = ahora_s();
        if ahora - self.psi_ultimo >= self.a.psi_enfriar_s {
            self.psi_ultimo = ahora;
            self.leer_psi();
            log(&format!(
                "presión de memoria ({}): se suelta memoria del shell y de las apps de fondo",
                self.psi_texto
            ));
            self.soltar_shell();
            self.reclamar_fondo();
            self.avisar_memoria();
        }
        self.guardar_estado();
    }

    fn manejar(&mut self, e: Evento) {
        match e {
            Evento::Foco {
                pid,
                dir,
                tipo,
                clase,
            } => self.en_foco(pid, &dir, &tipo, &clase),
            Evento::Abre { dir, tipo, clase } => self.en_abre(&dir, &tipo, &clase),
            Evento::Cierra { dir } => self.en_cierra(&dir),
            Evento::PsiMemoria => self.en_psi_memoria(),
            Evento::PsiNo(motivo) => {
                self.psi_estado = format!("no ({motivo})");
                log(&format!("no puedo vigilar la presión de memoria: {motivo}"));
                self.guardar_estado();
            }
            Evento::Compartir(si) => self.en_compartir(si),
        }
    }

    // ------------------------------------------------------------------------------------ compartir

    /// Compartir la pantalla (4.4): mientras haya un stream, una llamada con pantalla o una grabación por el
    /// portal, el shell no enseña notificaciones (se guardan en el panel y al terminar dice cuántas llegaron).
    fn en_compartir(&mut self, si: bool) {
        if si == self.compartiendo {
            return;
        }
        self.compartiendo = si;
        log(if si {
            "se empezó a compartir la pantalla: notificaciones calladas"
        } else {
            "se dejó de compartir la pantalla: notificaciones de vuelta"
        });
        self.avisar_compartir(si);
        self.guardar_estado();
    }

    fn avisar_compartir(&mut self, si: bool) {
        let qml = procesos::shell_qml_activo(
            &self.r.proc,
            self.uid,
            self.r.shell_qml_env.as_deref(),
            config::SHELL_QML,
        );
        let estado = if si { "true" } else { "false" };
        let args: Vec<String> = [
            "ipc",
            "-p",
            &qml,
            "call",
            "notificaciones",
            "compartiendo",
            estado,
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        let qs = self.r.qs.clone();
        self.l.segundo_plano_con_plazo(&qs, &args, Duration::from_secs(5));
    }

    // ------------------------------------------------------------------------------------ salida

    /// La salida limpia: todo como estaba. Devuelve el código de salida.
    fn terminar(&mut self, codigo: i32) -> i32 {
        if self.terminando {
            return codigo;
        }
        self.terminando = true;
        if self.ia_activa {
            self.terminar_ia("el gestor se apaga");
        }
        self.quitar_prioridad();
        if !self.juegos.is_empty() {
            self.terminar_juego();
        }
        if self.compartiendo {
            // Que el shell no se quede callado para siempre.
            self.compartiendo = false;
            let qml = procesos::shell_qml_activo(
                &self.r.proc,
                self.uid,
                self.r.shell_qml_env.as_deref(),
                config::SHELL_QML,
            );
            let args: Vec<String> = [
                "ipc",
                "-p",
                &qml,
                "call",
                "notificaciones",
                "compartiendo",
                "false",
            ]
            .iter()
            .map(ToString::to_string)
            .collect();
            let qs = self.r.qs.clone();
            self.l.ejecutar(&qs, &args, Some(Duration::from_secs(3)));
        }
        // Las apps que siguen abiertas vuelven al peso normal de systemd.
        for cg in self.mis_cg.keys().cloned().collect::<Vec<_>>() {
            if self.r.cg(&cg).is_dir() {
                self.propiedades(&cg, 100);
            }
        }
        let _ = std::fs::remove_file(&self.r.estado);
        let _ = std::io::stdout().flush();
        codigo
    }

    /// El bucle principal. Devuelve el código de salida del servicio.
    pub fn correr(&mut self) -> i32 {
        let senales = match sis::senales() {
            Ok(f) => f,
            Err(e) => {
                log(&format!("no puedo vigilar las señales: {e}"));
                return 1;
            }
        };
        self.recuperar();
        // 4.5: el aviso de cámara en uso (un hilo que duerme hasta que alguien abre una cámara).
        crate::camara::arrancar(self.r.dev.clone(), self.r.proc.clone(), self.r.estado_dir.clone());
        let psi = self.r.psi.clone();
        // Con VORTEX_GESTOR_INYECTAR (solo las pruebas) los avisos llegan por ese FIFO y no se registra el
        // disparador en el archivo: con un archivo falso, escribirlo cambiaría su contenido.
        let psi = (self.r.inyectar.is_none() && sis::se_puede_leer(&psi)).then_some(psi.as_path());
        let conectado = Oido::conectar(
            &self.sock2(),
            &self.hypr_dir.join(".socket.sock"),
            psi,
            self.a.psi_umbral_ms * 1000,
            self.a.psi_ventana_ms * 1000,
            self.a.antirrebote_ms,
            self.r.inyectar.as_deref(),
        );
        let firma12: String = self.firma.chars().take(12).collect();
        log(&format!("en marcha (Hyprland {firma12}…)"));
        self.guardar_estado();
        let (mut oido, arranque) = match conectado {
            Ok(c) => c,
            Err(_) => {
                log("Hyprland se cerró: el gestor termina");
                return self.terminar(0);
            }
        };
        let mut espera = self.a.mem_revisar_s.max(1);
        if self.a.ia_revisar_s > 0 && self.a.ia_revisar_s < espera {
            espera = self.a.ia_revisar_s;
        }
        let espera = Duration::from_secs(espera as u64);
        let mut ocioso = Instant::now() + espera;
        for e in arranque {
            self.manejar(e);
            self.revisar_ia();
        }
        loop {
            let (v, senal) = oido.esperar(&senales, Some(ocioso));
            if senal {
                let (parada, _) = sis::leer_senales(&senales);
                if parada {
                    return self.terminar(0);
                }
            }
            self.l.recoger();
            match v {
                Vuelta::Cerrado => break,
                Vuelta::Eventos(evs) => {
                    for e in evs {
                        self.manejar(e);
                        // Con eventos seguidos no hay tiempo ocioso: la IA se revisa igual cuando toca.
                        self.revisar_ia();
                    }
                    ocioso = Instant::now() + espera;
                }
                Vuelta::Nada => {
                    // Sin eventos en un rato. La presión solo avisa cuando ya hay esperas; así también sale
                    // antes de eso.
                    if Instant::now() >= ocioso {
                        self.avisar_memoria();
                        self.revisar_disco();
                        self.revisar_ia();
                        ocioso = Instant::now() + espera;
                    }
                }
            }
        }
        log("Hyprland se cerró: el gestor termina");
        self.terminar(0)
    }
}

/// `printf … >archivo` sobre un archivo que ya existe (cgroup): trunca y escribe, sin crearlo.
fn escribir(p: &Path, t: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new().write(true).truncate(true).open(p)?;
    f.write_all(t.as_bytes())
}

/// El arranque del servicio: busca Hyprland y corre el gestor.
pub fn arrancar() -> i32 {
    let uid = sis::uid();
    let r = Rutas::del_entorno(uid);
    let a = config::cargar(&r);
    let firma_env = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok();
    let Some((dir, firma)) = buscar_hyprland(&r.run, firma_env.as_deref()) else {
        log(&format!("no encuentro Hyprland en {}/hypr", r.run.display()));
        return 1;
    };
    Gestor::nuevo(a, r, dir, firma).correr()
}
