//! Lo que `puente.py` guardaba entre muestras, sin Qt: los grupos, lo que la persona abrió, el filtro,
//! el orden, los totales y la historia. La ventana (Rust + cxx-qt) solo le pasa lo que llega del
//! muestreo y publica lo que esto calcula.

use crate::grupos::{Grupos, ORDENES};
use crate::historia::Historia;
use crate::lista::{self, Opciones};
use crate::modelo::{self, Operacion};
use crate::{acciones, procesos};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Lo que mide el hilo de muestreo cada segundo.
pub struct Muestra {
    /// `None` si no tocaba medir procesos (otra página abierta).
    pub grupos: Option<Grupos>,
    pub rend: Value,
    pub vram_disponible: bool,
    pub hilos: u64,
    pub procesos: usize,
}

/// El cambio que hay que publicar en el modelo de la lista.
pub struct Actualizacion {
    pub ops: Vec<Operacion>,
    pub filas: Vec<Value>,
}

pub struct Vista {
    pub grupos: Grupos,
    por_clave: HashMap<String, (&'static str, usize)>,
    pub expandidos: HashSet<String>,
    pub colapsadas: HashSet<String>,
    pub filtro: String,
    pub orden: String,
    pub totales: Value,
    pub resumen: Value,
    pub rendimiento: Value,
    pub historia: Historia,
    pub vram_disponible: bool,
    claves: Vec<String>,
    iconos: HashMap<String, String>,
    raiz_proc: std::path::PathBuf,
}

impl Default for Vista {
    fn default() -> Self {
        Self::new()
    }
}

impl Vista {
    pub fn new() -> Self {
        Self::con_proc("/proc")
    }

    pub fn con_proc(raiz: impl Into<std::path::PathBuf>) -> Self {
        Vista {
            grupos: Grupos::default(),
            por_clave: HashMap::new(),
            expandidos: HashSet::new(),
            colapsadas: HashSet::new(),
            filtro: String::new(),
            orden: "nombre".into(),
            totales: json!({"cpu": 0.0, "mem": 0.0, "disco": 0.0, "vram": 0, "procesos": 0}),
            resumen: json!({}),
            rendimiento: json!({}),
            historia: Historia::new(),
            vram_disponible: false,
            claves: Vec::new(),
            iconos: HashMap::new(),
            raiz_proc: raiz.into(),
        }
    }

    /// El nombre (o la ruta) si hay ícono para él; si no, «» y QML pone el genérico.
    fn icono(
        iconos: &mut HashMap<String, String>,
        valido: &mut dyn FnMut(&str) -> bool,
        nombre: &str,
    ) -> String {
        if nombre.is_empty() {
            return String::new();
        }
        if !iconos.contains_key(nombre) {
            let ok = if nombre.starts_with('/') {
                std::path::Path::new(nombre).is_file()
            } else {
                valido(nombre) || valido(&nombre.to_lowercase())
            };
            iconos.insert(
                nombre.to_string(),
                if ok { nombre.to_string() } else { String::new() },
            );
        }
        iconos[nombre].clone()
    }

    pub fn hay_grupos(&self) -> bool {
        !self.por_clave.is_empty()
    }

    /// Una muestra nueva. Devuelve el cambio para la lista si había procesos.
    pub fn al_llegar(
        &mut self,
        m: Muestra,
        mi_pid: i32,
        valido: &mut dyn FnMut(&str) -> bool,
    ) -> Option<Actualizacion> {
        self.vram_disponible = m.vram_disponible;
        let rend = m.rend;
        let mut cambio = None;
        if let Some(g) = m.grupos {
            self.grupos = g;
            self.por_clave = self
                .grupos
                .apps
                .iter()
                .enumerate()
                .map(|(i, g)| (g.clave.clone(), ("apps", i)))
                .chain(
                    self.grupos
                        .fondo
                        .iter()
                        .enumerate()
                        .map(|(i, g)| (g.clave.clone(), ("fondo", i))),
                )
                .chain(
                    self.grupos
                        .sistema
                        .iter()
                        .enumerate()
                        .map(|(i, g)| (g.clave.clone(), ("sistema", i))),
                )
                .collect();
            self.totales = json!({"cpu": rend["cpu"]["uso"], "mem": rend["mem"]["uso"],
                "disco": self.grupos.todos().map(|g| g.disco).sum::<f64>(),
                "vram": self.grupos.todos().map(|g| g.vram).sum::<u64>(), "procesos": rend["cpu"]["procesos"]});
            let Vista { grupos, iconos, .. } = self;
            let mut icono = |n: &str| Self::icono(iconos, valido, n);
            self.resumen = lista::resumen(grupos, &rend, m.vram_disponible, mi_pid, &mut icono);
            cambio = Some(self.reconstruir(valido));
        } else {
            self.totales["cpu"] = rend["cpu"]["uso"].clone();
            self.totales["mem"] = rend["mem"]["uso"].clone();
        }
        self.historia.guardar(&rend);
        self.rendimiento = rend;
        cambio
    }

    /// Las filas que se ven ahora (con el filtro, el orden y lo abierto o cerrado).
    pub fn filas_ahora(&mut self, valido: &mut dyn FnMut(&str) -> bool) -> Vec<Value> {
        let Vista {
            grupos,
            iconos,
            expandidos,
            colapsadas,
            filtro,
            orden,
            ..
        } = self;
        let mut icono = |n: &str| Self::icono(iconos, valido, n);
        lista::filas(
            grupos,
            &Opciones {
                filtro,
                orden,
                expandidos,
                colapsadas,
            },
            &mut icono,
        )
    }

    pub fn reconstruir(&mut self, valido: &mut dyn FnMut(&str) -> bool) -> Actualizacion {
        let filas = self.filas_ahora(valido);
        let claves = &mut self.claves;
        let nuevas: Vec<String> = filas
            .iter()
            .map(|f| f["clave"].as_str().unwrap_or("").to_string())
            .collect();
        let ops = modelo::diferencias(claves, &nuevas);
        *claves = nuevas;
        Actualizacion { ops, filas }
    }

    pub fn poner_filtro(&mut self, v: &str) -> bool {
        let cambia = v != self.filtro;
        self.filtro = v.to_string();
        cambia
    }

    /// `false` si no es un orden conocido o ya era ese.
    pub fn poner_orden(&mut self, v: &str) -> bool {
        if ORDENES.contains(&v) && v != self.orden {
            self.orden = v.to_string();
            return true;
        }
        false
    }

    pub fn alternar_grupo(&mut self, clave: &str) {
        if !self.expandidos.remove(clave) {
            self.expandidos.insert(clave.to_string());
        }
    }

    pub fn alternar_seccion(&mut self, seccion: &str) {
        if !self.colapsadas.remove(seccion) {
            self.colapsadas.insert(seccion.to_string());
        }
    }

    /// Los PIDs de la fila: los de una app o grupo, o el de un proceso («grupo/pid»).
    pub fn pids(&self, clave: &str) -> Vec<i32> {
        if let Some((_, pid)) = clave.rsplit_once('/') {
            return pid.parse().into_iter().collect();
        }
        self.por_clave
            .get(clave)
            .map_or(Vec::new(), |(s, i)| self.grupos.seccion(s)[*i].pids.clone())
    }

    /// Lo que muestra el menú de un clic derecho.
    pub fn detalles(&self, clave: &str) -> Value {
        let pids = self.pids(clave);
        let Some(&primero) = pids.first() else {
            return json!({});
        };
        let base = clave.split('/').next().unwrap_or("");
        let g = self.por_clave.get(base).map(|(s, i)| &self.grupos.seccion(s)[*i]);
        let proc = g.and_then(|g| g.procesos.iter().find(|p| p.pid == primero));
        let en_pids: Vec<&procesos::Proceso> = g.map_or(Vec::new(), |g| {
            g.procesos.iter().filter(|p| pids.contains(&p.pid)).collect()
        });
        json!({"pids": pids, "pid": primero, "comando": proc.map_or("", |p| &p.comando), "usuario": proc.map_or("", |p| &p.usuario),
            "prioridad": acciones::prioridad_de(primero), "ubicacion": procesos::ubicacion_en(&self.raiz_proc, primero),
            "ventana": g.is_some_and(|g| !g.ventanas.is_empty()) && !clave.contains('/'),
            "suspendido": en_pids.iter().all(|p| p.estado == "stopped")})
    }

    pub fn ventanas_de(&self, clave: &str) -> Vec<String> {
        self.por_clave
            .get(clave)
            .map_or(Vec::new(), |(s, i)| self.grupos.seccion(s)[*i].ventanas.clone())
    }
}
