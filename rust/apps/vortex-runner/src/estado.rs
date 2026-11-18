//! Lo que hace `puente.py` sin Qt: la lista de programas ya calculada para la interfaz, qué está abierto,
//! instalar, jugar, detener, configurar. La ventana (cxx-qt) solo envuelve esto: llama a los métodos y
//! convierte los [`Evento`] en señales de QML.
//!
//! Cada método que en Python emitía una señal (`aviso`, `programasCambio`…) deja aquí un [`Evento`] en
//! una cola; la ventana la vacía con [`Estado::tomar_eventos`] después de cada llamada.

use crate::biblioteca::{self, Biblioteca, Entrada, ExeNuevo};
use crate::protones::{self, Proton};
use crate::{Contexto, correr};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Un programa de la lista, con lo que la interfaz necesita ya calculado.
#[derive(Debug, Clone, PartialEq)]
pub struct Fila {
    pub id: String,
    pub nombre: String,
    pub exe: String,
    pub prefijo: String,
    pub proton: String,
    pub argumentos: String,
    /// Ruta local del ícono (vacía si no hay o ya no existe); la ventana la vuelve URL.
    pub icono: String,
    pub abierto: bool,
    pub subtitulo: String,
    pub lanzador: bool,
    pub existe: bool,
    pub ultima: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instalado {
    pub exe: String,
    pub nombre: String,
    pub carpeta: String,
}

/// Lo que en Python eran señales.
#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    /// texto, tipo (`info` | `error` | `exito`)
    Aviso(String, String),
    ProgramasCambio,
    MotorCambio,
    InstalandoCambio,
    ElegirInstalado(Vec<Instalado>),
    PreguntarArchivo {
        ruta: String,
        nombre: String,
        instalador: bool,
    },
}

struct Instalando {
    nombre: String,
    prefijo: String,
    antes: BTreeSet<String>,
    proceso: Child,
}

pub struct Estado {
    pub ctx: Contexto,
    pub bib: Biblioteca,
    programas: Vec<Fila>,
    abiertos: BTreeSet<String>,
    inicios: BTreeMap<String, f64>,
    falta: Vec<String>,
    instalando_motor: bool,
    instalando: Option<Instalando>,
    protones: Vec<Proton>,
    prefijo_pendiente: Option<String>,
    eventos: Vec<Evento>,
}

fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// «hace 5 min», «ayer»… (`_hace`).
pub fn hace(ahora: f64, ts: i64) -> String {
    if ts == 0 {
        return String::new();
    }
    let s = (ahora - ts as f64).max(0.0);
    if s < 120.0 {
        "hace un momento".into()
    } else if s < 3600.0 {
        format!("hace {} min", (s / 60.0) as i64)
    } else if s < 86400.0 {
        format!("hace {} h", (s / 3600.0) as i64)
    } else {
        let d = (s / 86400.0) as i64;
        if d == 1 {
            "ayer".into()
        } else {
            format!("hace {d} días")
        }
    }
}

/// «45 min», «2.5 h», «12 h» (`_total`).
pub fn total(minutos: i64) -> String {
    if minutos < 60 {
        return if minutos != 0 {
            format!("{minutos} min")
        } else {
            String::new()
        };
    }
    let h = minutos as f64 / 60.0;
    if h < 10.0 {
        format!("{h:.1} h").replace(".0 h", " h")
    } else {
        format!("{} h", h as i64)
    }
}

/// «Abierto ahora», «Nunca abierto» o «Abierto hace 2 h · 5 h en total».
pub fn subtitulo(e: &Entrada, abierto: bool, ahora: f64) -> String {
    if abierto {
        return "Abierto ahora".into();
    }
    if e.ultima_vez == 0 {
        return "Nunca abierto".into();
    }
    let mut partes = vec![format!("Abierto {}", hace(ahora, e.ultima_vez))];
    let t = total(e.minutos);
    if !t.is_empty() {
        partes.push(format!("{t} en total"));
    }
    partes.join(" · ")
}

/// `file:///ruta%20con%20espacios` → `/ruta con espacios` (`QUrl.toLocalFile`); una ruta normal queda igual.
pub fn ruta_de_url(url_o_ruta: &str) -> String {
    let Some(resto) = url_o_ruta.strip_prefix("file:") else {
        return url_o_ruta.to_string();
    };
    let resto = resto.strip_prefix("//").unwrap_or(resto);
    // «file://host/ruta»: la ruta empieza en la primera «/» (sin host = «file:///ruta»)
    let resto = if resto.starts_with('/') {
        resto
    } else {
        resto.find('/').map_or("", |i| &resto[i..])
    };
    let b = resto.as_bytes();
    let mut salida = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&resto[i + 1..i + 3], 16)
        {
            salida.push(v);
            i += 3;
            continue;
        }
        salida.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&salida).into_owned()
}

/// Abre una carpeta con la app que la persona tenga para eso (`xdg-open`).
pub fn abrir_con_xdg(ruta: &Path) {
    let _ = std::fs::create_dir_all(ruta);
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let _ = Command::new("xdg-open")
        .arg(ruta)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|mut h| std::thread::spawn(move || h.wait()));
}

impl Estado {
    pub fn new(ctx: Contexto) -> Self {
        let bib = Biblioteca::new(ctx.rutas.clone());
        let mut e = Estado {
            falta: correr::falta_motor(&ctx),
            protones: protones::lista(&ctx),
            ctx,
            bib,
            programas: Vec::new(),
            abiertos: BTreeSet::new(),
            inicios: BTreeMap::new(),
            instalando_motor: false,
            instalando: None,
            prefijo_pendiente: None,
            eventos: Vec::new(),
        };
        e.recargar();
        e
    }

    pub fn tomar_eventos(&mut self) -> Vec<Evento> {
        std::mem::take(&mut self.eventos)
    }

    fn aviso(&mut self, texto: impl Into<String>, tipo: &str) {
        self.eventos.push(Evento::Aviso(texto.into(), tipo.into()));
    }

    // ---- estado --------------------------------------------------------------------------

    fn fila(&self, e: &Entrada) -> Fila {
        let abierto = self.abiertos.contains(&e.id);
        Fila {
            id: e.id.clone(),
            nombre: e.nombre.clone(),
            exe: e.exe.clone(),
            prefijo: e.prefijo.clone(),
            proton: e.proton.clone(),
            argumentos: e.argumentos.clone(),
            icono: if !e.icono.is_empty() && Path::new(&e.icono).is_file() {
                e.icono.clone()
            } else {
                String::new()
            },
            abierto,
            subtitulo: subtitulo(e, abierto, ahora()),
            lanzador: e.lanzador,
            existe: Path::new(&e.exe).exists(),
            ultima: e.ultima_vez,
        }
    }

    pub fn recargar(&mut self) {
        let mut datos = self.bib.cargar();
        datos.sort_by_key(|e| (std::cmp::Reverse(e.ultima_vez), std::cmp::Reverse(e.agregado)));
        self.programas = datos.iter().map(|e| self.fila(e)).collect();
        self.eventos.push(Evento::ProgramasCambio);
    }

    pub fn programas(&self) -> &[Fila] {
        &self.programas
    }
    pub fn falta_motor(&self) -> &[String] {
        &self.falta
    }
    pub fn instalando_motor(&self) -> bool {
        self.instalando_motor
    }
    /// El nombre de lo que se está instalando («» si nada).
    pub fn instalando(&self) -> &str {
        self.instalando.as_ref().map_or("", |i| &i.nombre)
    }
    pub fn carpeta_datos(&self) -> &Path {
        &self.ctx.rutas.datos
    }

    /// `[(id, nombre)]` para el selector de Proton: primero «El de serie (…)».
    pub fn protones_para_ui(&self) -> Vec<(String, String)> {
        let mut l = vec![(
            String::new(),
            format!("El de serie ({})", self.protones[0].nombre),
        )];
        for p in &self.protones {
            let sufijo = if !p.version.is_empty() && p.version != p.id {
                format!(" · {}", p.version)
            } else {
                String::new()
            };
            l.push((p.id.clone(), format!("{}{sufijo}", p.nombre)));
        }
        l
    }

    /// ¿Qué sigue abierto? Suma el tiempo de lo que se cerró y mira si terminó un instalador.
    /// `abiertos`: los prefijos con procesos (`correr::prefijos_abiertos()`, que se calcula fuera para no
    /// bloquear la ventana).
    pub fn revisar(&mut self, abiertos: &BTreeSet<String>) {
        let datos = self.bib.cargar();
        let ahora_ids: BTreeSet<String> = datos
            .iter()
            .filter(|e| abiertos.contains(&crate::rutas::normalizar(&e.prefijo)))
            .map(|e| e.id.clone())
            .collect();
        for pid in ahora_ids.difference(&self.abiertos.clone()) {
            self.inicios.entry(pid.clone()).or_insert_with(ahora);
        }
        for pid in self.abiertos.clone().difference(&ahora_ids) {
            if let Some(inicio) = self.inicios.remove(pid) {
                self.bib.sumar_tiempo(pid, ahora() - inicio, inicio);
            }
        }
        let cambio = ahora_ids != self.abiertos;
        self.abiertos = ahora_ids;
        let terminado = match &mut self.instalando {
            Some(i) => {
                !abiertos.contains(&crate::rutas::normalizar(&i.prefijo))
                    && i.proceso.try_wait().is_ok_and(|s| s.is_some())
            }
            None => false,
        };
        if terminado {
            self.fin_instalador();
        }
        if cambio {
            self.recargar();
        }
    }

    // ---- agregar -------------------------------------------------------------------------

    fn listo_motor(&mut self) -> bool {
        if !self.falta.is_empty() {
            self.aviso(
                "Primero instala el motor (umu y Proton CachyOS): botón «Instalar» arriba.",
                "error",
            );
            return false;
        }
        true
    }

    /// Agrega un .exe; devuelve su id («» si no se pudo).
    pub fn agregar(&mut self, url: &str) -> String {
        match self.bib.agregar(Path::new(&ruta_de_url(url)), None, None) {
            Ok(e) => {
                self.recargar();
                self.aviso(format!("Agregué {}.", e.nombre), "exito");
                e.id
            }
            Err(err) => {
                self.aviso(err.to_string(), "error");
                String::new()
            }
        }
    }

    /// Corre un instalador (setup.exe, .msi) en un prefijo nuevo; al terminar, pregunta cuál es el programa.
    pub fn instalar(&mut self, url: &str) {
        let ruta = ruta_de_url(url);
        if !self.listo_motor() {
            return;
        }
        if let Some(i) = &self.instalando {
            let m = format!("Espera a que termine de instalarse {}.", i.nombre);
            self.aviso(m, "info");
            return;
        }
        if !Path::new(&ruta).is_file() {
            self.aviso("No encuentro ese instalador.", "error");
            return;
        }
        let nombre = Path::new(&ruta)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(prefijo) = self.bib.nuevo_prefijo(&nombre) else {
            self.aviso("No pude preparar el espacio para instalar.", "error");
            return;
        };
        let antes = biblioteca::exes_en(&prefijo);
        let id = prefijo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temporal = correr::temporal(&id, &prefijo.to_string_lossy(), &ruta);
        match correr::lanzar(&self.ctx, &temporal, None, &[]) {
            Ok(proceso) => {
                self.instalando = Some(Instalando {
                    nombre,
                    prefijo: prefijo.to_string_lossy().into_owned(),
                    antes,
                    proceso,
                });
                self.eventos.push(Evento::InstalandoCambio);
                self.aviso(
                    "Sigue los pasos del instalador. La primera vez tarda más: se prepara Windows.",
                    "info",
                );
            }
            Err(err) => self.aviso(format!("No se pudo abrir el instalador: {err}"), "error"),
        }
    }

    fn fin_instalador(&mut self) {
        let Some(datos) = self.instalando.take() else {
            return;
        };
        self.eventos.push(Evento::InstalandoCambio);
        let nuevos: Vec<ExeNuevo> = biblioteca::exes_nuevos(Path::new(&datos.prefijo), &datos.antes);
        if nuevos.is_empty() {
            // Nada que agregar (lo cancelaste, falló o no era un programa): su espacio no se queda ocupando
            // cientos de MB.
            self.bib.descartar_prefijo(Path::new(&datos.prefijo));
            self.aviso(
                "El instalador terminó, pero no dejó ningún programa nuevo (¿lo cancelaste?). Si era un programa ya listo, usa «Agregar .exe».",
                "info",
            );
            self.prefijo_pendiente = None;
            return;
        }
        self.prefijo_pendiente = Some(datos.prefijo);
        let lista = nuevos
            .iter()
            .take(8)
            .map(|n| Instalado {
                exe: n.exe.clone(),
                nombre: n.nombre.clone(),
                carpeta: Path::new(&n.exe)
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            })
            .collect();
        self.eventos.push(Evento::ElegirInstalado(lista));
    }

    pub fn agregar_instalado(&mut self, exe: &str, nombre: &str) {
        let prefijo = self.prefijo_pendiente.clone();
        let nombre = Some(nombre).filter(|n| !n.is_empty());
        match self
            .bib
            .agregar(Path::new(exe), nombre, prefijo.as_deref().map(Path::new))
        {
            Ok(e) => {
                self.recargar();
                self.aviso(format!("{} quedó en tu biblioteca.", e.nombre), "exito");
            }
            Err(err) => self.aviso(err.to_string(), "error"),
        }
    }

    /// «Ninguno»: ninguno de los .exe es el programa; se borra el espacio que creó el instalador.
    pub fn descartar_instalado(&mut self) {
        if let Some(p) = self.prefijo_pendiente.take() {
            self.bib.descartar_prefijo(Path::new(&p));
        }
    }

    /// Un .exe que abriste con doble clic: si ya está, se abre; si no, se pregunta qué hacer.
    pub fn abrir_archivo(&mut self, ruta: &str) {
        let ruta = ruta_de_url(ruta);
        if let Some(e) = self.bib.por_exe(Path::new(&ruta)) {
            self.jugar(&e.id);
            return;
        }
        if !Path::new(&ruta).is_file() {
            self.aviso("No encuentro ese archivo.", "error");
            return;
        }
        let nombre = crate::pe::nombre(Path::new(&ruta));
        let instalador = biblioteca::parece_instalador(Path::new(&ruta));
        self.eventos.push(Evento::PreguntarArchivo {
            ruta,
            nombre,
            instalador,
        });
    }

    // ---- jugar ---------------------------------------------------------------------------

    pub fn jugar(&mut self, pid: &str) {
        let Some(e) = self.bib.por_id(pid) else {
            return;
        };
        if !self.listo_motor() {
            return;
        }
        if !Path::new(&e.exe).exists() {
            self.aviso(
                format!(
                    "Ya no encuentro el .exe de {}: cámbialo en su configuración.",
                    e.nombre
                ),
                "error",
            );
            return;
        }
        if correr::corriendo(&e) {
            self.aviso(format!("{} ya está abierto.", e.nombre), "info");
            return;
        }
        match correr::lanzar(&self.ctx, &e, None, &[]) {
            Ok(mut hijo) => {
                std::thread::spawn(move || hijo.wait()); // para que no quede de zombi
            }
            Err(err) => {
                self.aviso(format!("No se pudo abrir {}: {err}", e.nombre), "error");
                return;
            }
        }
        self.inicios.insert(pid.to_string(), ahora());
        self.abiertos.insert(pid.to_string());
        self.recargar();
        let primera = !Path::new(&e.prefijo).join("drive_c").exists();
        self.aviso(
            format!(
                "Abriendo {}…{}",
                e.nombre,
                if primera {
                    " La primera vez tarda más."
                } else {
                    ""
                }
            ),
            "info",
        );
    }

    pub fn detener(&mut self, pid: &str) {
        let Some(e) = self.bib.por_id(pid) else {
            return;
        };
        std::thread::spawn({
            let e = e.clone();
            move || correr::detener(&e, Duration::from_secs(4))
        });
        self.aviso(format!("Cerrando {}…", e.nombre), "info");
    }

    // ---- configurar ----------------------------------------------------------------------

    pub fn guardar(&mut self, pid: &str, nombre: &str, proton: &str, argumentos: &str) {
        match self.bib.cambiar(
            pid,
            &[("nombre", nombre), ("proton", proton), ("argumentos", argumentos)],
        ) {
            Ok(_) => {
                self.recargar();
                self.aviso("Listo, guardado.", "exito");
            }
            Err(err) => self.aviso(err.to_string(), "error"),
        }
    }

    pub fn cambiar_exe(&mut self, pid: &str, url: &str) {
        let ruta = ruta_de_url(url);
        if !Path::new(&ruta).is_file() {
            self.aviso("No encuentro ese archivo.", "error");
            return;
        }
        let real = crate::rutas::resolver(Path::new(&ruta));
        let _ = self.bib.cambiar(pid, &[("exe", &real.to_string_lossy())]);
        self.recargar();
    }

    pub fn lanzador(&mut self, pid: &str, poner: bool) {
        let Some(mut e) = self.bib.por_id(pid) else {
            return;
        };
        if poner {
            match self.bib.crear_lanzador(&mut e) {
                Ok(_) => self.aviso(format!("{} ya sale en el lanzador de apps.", e.nombre), "exito"),
                Err(err) => self.aviso(format!("No se pudo crear el acceso: {err}"), "error"),
            }
        } else {
            self.bib.quitar_lanzador(&mut e);
        }
        self.recargar();
    }

    /// La carpeta del .exe (para «Abrir su carpeta»).
    pub fn carpeta_exe(&self, pid: &str) -> Option<PathBuf> {
        self.bib
            .por_id(pid)
            .map(|e| Path::new(&e.exe).parent().unwrap_or(Path::new("/")).to_path_buf())
    }

    /// Su disco C: (para «Abrir su disco C:»).
    pub fn disco_c(&self, pid: &str) -> Option<PathBuf> {
        self.bib
            .por_id(pid)
            .map(|e| Path::new(&e.prefijo).join("drive_c"))
    }

    pub fn configuracion_wine(&mut self, pid: &str) {
        let Some(e) = self.bib.por_id(pid) else {
            return;
        };
        if !self.listo_motor() {
            return;
        }
        match correr::lanzar(&self.ctx, &e, Some("winecfg"), &[]) {
            Ok(mut hijo) => {
                std::thread::spawn(move || hijo.wait());
            }
            Err(err) => self.aviso(
                format!("No se pudo abrir la configuración de Wine: {err}"),
                "error",
            ),
        }
    }

    pub fn quitar(&mut self, pid: &str, borrar_prefijo: bool) {
        let Some(e) = self.bib.por_id(pid) else {
            return;
        };
        if correr::corriendo(&e) {
            correr::detener(&e, Duration::from_secs(4));
        }
        self.bib.quitar(pid, borrar_prefijo);
        self.recargar();
        let cola = if borrar_prefijo {
            ""
        } else {
            " Sus partidas se quedaron."
        };
        self.aviso(format!("Quité {} de tu biblioteca.{cola}", e.nombre), "info");
    }

    // ---- motor ---------------------------------------------------------------------------

    /// Marca que se está instalando el motor. `false` si ya se estaba. Quien llama corre
    /// `correr::instalar_motor` en un hilo y avisa con [`Estado::fin_motor`].
    pub fn iniciar_motor(&mut self) -> bool {
        if self.instalando_motor {
            return false;
        }
        self.instalando_motor = true;
        self.eventos.push(Evento::MotorCambio);
        true
    }

    pub fn fin_motor(&mut self, codigo: i32, detalle: &str) {
        self.instalando_motor = false;
        self.falta = correr::falta_motor(&self.ctx);
        self.protones = protones::lista(&self.ctx);
        self.eventos.push(Evento::MotorCambio);
        if self.falta.is_empty() {
            self.aviso("Listo: ya puedes abrir programas de Windows.", "exito");
        } else if codigo == 126 || codigo == 127 {
            self.aviso("No se instaló: cancelaste la contraseña.", "info");
        } else if detalle.is_empty() {
            self.aviso("No se pudo instalar el motor.", "error");
        } else {
            self.aviso(format!("No se pudo instalar el motor: {detalle}"), "error");
        }
    }

    pub fn refrescar(&mut self) {
        self.protones = protones::lista(&self.ctx);
        self.falta = correr::falta_motor(&self.ctx);
        self.eventos.push(Evento::MotorCambio);
        self.recargar();
    }
}

/// «vortex-runner --correr ID»: abre ese programa sin ventana y espera a que se cierre (los accesos del
/// lanzador). Devuelve el código de salida.
pub fn correr_programa(ctx: &Contexto, pid: &str) -> i32 {
    let bib = Biblioteca::new(ctx.rutas.clone());
    let Some(e) = bib.por_id(pid) else {
        eprintln!("vortex-runner: no hay ningún programa «{pid}» en tu biblioteca");
        return 1;
    };
    if correr::corriendo(&e) {
        return 0;
    }
    match correr::correr_y_esperar(ctx, &bib, &e) {
        Ok(c) => c,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("vortex-runner: {err} Ábrelo desde Vortexine's Runner para instalarlo.");
            1
        }
        Err(err) => {
            eprintln!("vortex-runner: {err}");
            1
        }
    }
}

/// Una línea de `--estado` para revisar el motor sin ventana.
pub fn resumen(ctx: &Contexto) -> serde_json::Value {
    let e = Estado::new(ctx.clone());
    serde_json::json!({
        "programas": e.programas().len(),
        "falta_motor": e.falta_motor(),
        "protones": e.protones_para_ui().iter().map(|(i, n)| serde_json::json!({"id": i, "nombre": n})).collect::<Vec<_>>(),
        "datos": e.carpeta_datos(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn textos_de_tiempo() {
        assert_eq!(total(0), "");
        assert_eq!(total(45), "45 min");
        assert_eq!(total(60), "1 h");
        assert_eq!(total(150), "2.5 h");
        assert_eq!(total(60 * 12 + 20), "12 h");
        assert_eq!(hace(1000.0, 0), "");
        assert_eq!(hace(1000.0, 950), "hace un momento");
        assert_eq!(hace(10_000.0, 10_000 - 600), "hace 10 min");
        assert_eq!(hace(100_000.0, 100_000 - 7300), "hace 2 h");
        assert_eq!(hace(1_000_000.0, 1_000_000 - 90_000), "ayer");
        assert_eq!(hace(1_000_000.0, 1_000_000 - 300_000), "hace 3 días");
    }

    #[test]
    fn urls_de_archivo() {
        assert_eq!(
            ruta_de_url("file:///home/a/Mis%20juegos/x.exe"),
            "/home/a/Mis juegos/x.exe"
        );
        assert_eq!(ruta_de_url("/ya/es/ruta.exe"), "/ya/es/ruta.exe");
        assert_eq!(ruta_de_url("file:/x/y"), "/x/y");
        assert_eq!(ruta_de_url("file:///100%/x"), "/100%/x");
    }
}
