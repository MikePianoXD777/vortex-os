//! Los procesos agrupados como en el Administrador de tareas de Windows:
//!   · «Apps»: cada ventana abierta (hyprctl clients) con todos los procesos que cuelgan de ella;
//!   · «Segundo plano»: tus otros procesos, juntos por nombre;
//!   · «Del sistema»: los de root y de otros usuarios.

use crate::lanzadores::Aplicaciones;
use crate::procesos::Proceso;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Ventana {
    pub pid: i32,
    pub clase: String,
    pub titulo: String,
    pub direccion: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grupo {
    pub clave: String,
    pub nombre: String,
    pub icono: String,
    pub subtitulo: String,
    pub procesos: Vec<Proceso>,
    pub pids: Vec<i32>,
    pub ventanas: Vec<String>,
    pub cpu: f64,
    pub mem: u64,
    pub disco: f64,
    pub vram: u64,
    pub hilos: u32,
    pub suspendido: bool,
    pub usuario: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grupos {
    pub apps: Vec<Grupo>,
    pub fondo: Vec<Grupo>,
    pub sistema: Vec<Grupo>,
}

impl Grupos {
    pub fn seccion(&self, nombre: &str) -> &[Grupo] {
        match nombre {
            "apps" => &self.apps,
            "fondo" => &self.fondo,
            _ => &self.sistema,
        }
    }

    pub fn todos(&self) -> impl Iterator<Item = &Grupo> {
        self.apps.iter().chain(&self.fondo).chain(&self.sistema)
    }
}

fn grupo(
    clave: String,
    nombre: String,
    icono: String,
    procesos: Vec<Proceso>,
    subtitulo: String,
    ventanas: Vec<String>,
) -> Grupo {
    Grupo {
        clave,
        nombre,
        icono,
        subtitulo,
        pids: procesos.iter().map(|p| p.pid).collect(),
        ventanas,
        cpu: procesos.iter().map(|p| p.cpu).sum(),
        mem: procesos.iter().map(|p| p.mem).sum(),
        disco: procesos.iter().map(|p| p.disco).sum(),
        vram: procesos.iter().map(|p| p.vram).sum(),
        hilos: procesos.iter().map(|p| p.hilos).sum(),
        suspendido: !procesos.is_empty() && procesos.iter().all(|p| p.estado == "stopped"),
        usuario: procesos.first().map_or(String::new(), |p| p.usuario.clone()),
        procesos,
    }
}

/// Procesos → apps, segundo plano y sistema. `uid` es el de la persona (el de los procesos «tuyos»).
pub fn agrupar(filas: &[Proceso], ventanas: &[Ventana], apps: &Aplicaciones, uid: u32) -> Grupos {
    let por_pid: HashMap<i32, &Proceso> = filas.iter().map(|f| (f.pid, f)).collect();
    let mut hijos: HashMap<i32, Vec<i32>> = HashMap::new();
    for f in filas {
        hijos.entry(f.ppid).or_default().push(f.pid);
    }

    let mut usados: HashSet<i32> = HashSet::new();
    let mut grupos_apps = Vec::new();
    // Las ventanas por PID, en el orden en que aparecen.
    let mut ventanas_por_pid: Vec<(i32, Vec<&Ventana>)> = Vec::new();
    for v in ventanas {
        match ventanas_por_pid.iter_mut().find(|(p, _)| *p == v.pid) {
            Some((_, vs)) => vs.push(v),
            None => ventanas_por_pid.push((v.pid, vec![v])),
        }
    }
    // Primero las ventanas cuyo proceso no cuelga de otra ventana (una terminal y lo que abriste en ella).
    for (pid, vs) in &ventanas_por_pid {
        if !por_pid.contains_key(pid) || usados.contains(pid) {
            continue;
        }
        let mut pila = vec![*pid];
        let mut miembros: Vec<Proceso> = Vec::new();
        while let Some(actual) = pila.pop() {
            if usados.contains(&actual) || !por_pid.contains_key(&actual) {
                continue;
            }
            usados.insert(actual);
            miembros.push(por_pid[&actual].clone());
            if let Some(h) = hijos.get(&actual) {
                pila.extend(h);
            }
        }
        let lanzador = apps
            .de_clase(&vs[0].clase)
            .or_else(|| apps.de_ejecutable(&por_pid[pid].nombre));
        let nombre = lanzador
            .map(|l| l.nombre.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| Some(vs[0].clase.clone()).filter(|c| !c.is_empty()))
            .unwrap_or_else(|| por_pid[pid].nombre.clone());
        let icono = lanzador
            .map(|l| l.icono.clone())
            .filter(|i| !i.is_empty())
            .unwrap_or_else(|| vs[0].clase.clone());
        let titulo = if vs.len() == 1 {
            vs[0].titulo.clone()
        } else {
            format!("{} ventanas", vs.len())
        };
        let subtitulo = if titulo == nombre { String::new() } else { titulo };
        grupos_apps.push(grupo(
            format!("a:{pid}"),
            nombre,
            icono,
            miembros,
            subtitulo,
            vs.iter().map(|v| v.direccion.clone()).collect(),
        ));
    }

    let mut fondo: Vec<(String, Vec<Proceso>)> = Vec::new();
    let mut sistema: Vec<(String, Vec<Proceso>)> = Vec::new();
    for f in filas {
        if usados.contains(&f.pid) {
            continue;
        }
        let destino = if f.uid == uid { &mut fondo } else { &mut sistema };
        match destino.iter_mut().find(|(n, _)| *n == f.nombre) {
            Some((_, v)) => v.push(f.clone()),
            None => destino.push((f.nombre.clone(), vec![f.clone()])),
        }
    }

    let armar = |seccion: &str, juntos: Vec<(String, Vec<Proceso>)>| -> Vec<Grupo> {
        juntos
            .into_iter()
            .map(|(nombre, procesos)| {
                let l = apps.de_ejecutable(&nombre);
                let nombre_l = l
                    .map(|l| l.nombre.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| nombre.clone());
                let subtitulo = match l {
                    Some(l) if l.nombre != nombre => nombre.clone(),
                    _ => String::new(),
                };
                grupo(
                    format!("{}:{nombre}", &seccion[..1]),
                    nombre_l,
                    l.map(|l| l.icono.clone()).unwrap_or_default(),
                    procesos,
                    subtitulo,
                    Vec::new(),
                )
            })
            .collect()
    };
    Grupos {
        apps: grupos_apps,
        fondo: armar("fondo", fondo),
        sistema: armar("sistema", sistema),
    }
}

pub const ORDENES: [&str; 5] = ["nombre", "cpu", "mem", "disco", "vram"];

/// Lo que se puede ordenar: un grupo o un proceso suelto.
pub trait Ordenable {
    fn nombre(&self) -> &str;
    fn clave(&self) -> String;
    fn valor(&self, por: &str) -> f64;
}

impl Ordenable for Grupo {
    fn nombre(&self) -> &str {
        &self.nombre
    }
    fn clave(&self) -> String {
        self.clave.clone()
    }
    fn valor(&self, por: &str) -> f64 {
        match por {
            "cpu" => self.cpu,
            "mem" => self.mem as f64,
            "disco" => self.disco,
            "vram" => self.vram as f64,
            _ => 0.0,
        }
    }
}

impl Ordenable for Proceso {
    fn nombre(&self) -> &str {
        &self.nombre
    }
    fn clave(&self) -> String {
        self.pid.to_string()
    }
    fn valor(&self, por: &str) -> f64 {
        match por {
            "cpu" => self.cpu,
            "mem" => self.mem as f64,
            "disco" => self.disco,
            "vram" => self.vram as f64,
            _ => 0.0,
        }
    }
}

pub fn ordenar<'a, T: Ordenable>(elementos: &'a [T], por: &str) -> Vec<&'a T> {
    let mut v: Vec<&T> = elementos.iter().collect();
    if por == "nombre" {
        v.sort_by_cached_key(|g| (g.nombre().to_lowercase(), g.clave()));
    } else {
        v.sort_by(|a, b| {
            b.valor(por)
                .partial_cmp(&a.valor(por))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.nombre().to_lowercase().cmp(&b.nombre().to_lowercase()))
        });
    }
    v
}

/// Si el grupo coincide con lo que se escribió en la búsqueda (nombre, subtítulo, un proceso o un PID).
pub fn coincide(g: &Grupo, filtro: &str) -> bool {
    let f = filtro.trim().to_lowercase();
    if f.is_empty() {
        return true;
    }
    g.nombre.to_lowercase().contains(&f)
        || g.subtitulo.to_lowercase().contains(&f)
        || g.procesos
            .iter()
            .any(|p| p.nombre.to_lowercase().contains(&f) || f == p.pid.to_string())
}
