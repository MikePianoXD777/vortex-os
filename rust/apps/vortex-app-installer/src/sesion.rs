//! El estado de la ventana, sin Qt (`puente.py`): qué archivo hay, qué se está haciendo y qué se le muestra
//! a la persona.
//!
//! Estados: «vacio» (sin archivo), «leyendo», «listo» (se ve el archivo y sus botones), «trabajando» (barra
//! de progreso), «hecho» (resultado) y «error».
//!
//! Lo lento (leer el archivo, descomprimir, instalar) corre en un hilo aparte. Ese hilo no toca la sesión: le
//! manda un [`Mensaje`] a `notificar`, y quien tiene la sesión (la ventana, o la prueba) se lo da a
//! [`Sesion::procesar`] en su propio hilo. Cada archivo o trabajo nuevo sube el `turno`, y lo que llega
//! tarde de uno anterior se descarta.
//!
//! Lo que cambia se apunta como [`Evento`] y se recoge con [`Sesion::tomar_eventos`].

use crate::detectar::Info;
use crate::motor::{self, Opciones};
use crate::util::{abspath, basename, desescapar_url};
use crate::{Contexto, Control, Error, Resultado, flatpak, instalar};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Lo que un hilo de fondo le avisa a la sesión.
pub enum Mensaje {
    Analizado(Result<Info, String>, u64),
    Terminado(Resultado<Value>, u64),
    Avance(f64, u64),
    Deshecho(Result<String, String>),
}

/// Lo que la ventana tiene que volver a leer.
#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    Cambio,
    Progreso,
    Instalados,
    /// texto, «info» o «error»
    Aviso(String, String),
    Mostrar,
}

pub type Notificar = Arc<dyn Fn(Mensaje) + Send + Sync>;

fn trabajo_de(accion: &str) -> &'static str {
    match accion {
        "descomprimir" | "ver_contenido" => "trabajando_descomprimir",
        "instalar_programa" | "instalar_appimage" | "instalar_flatpak" => "trabajando_instalar",
        "flathub" => "trabajando_flathub",
        _ => "trabajando",
    }
}

/// Acepta una ruta o una URL file:// (lo que llega al arrastrar).
pub fn ruta_de(ctx: &Contexto, texto: &str) -> String {
    let t = texto.trim();
    if t.is_empty() {
        return String::new();
    }
    if let Some(resto) = t.strip_prefix("file:") {
        let resto = match resto.strip_prefix("//") {
            Some(r) => r.find('/').map_or("", |i| &r[i..]),
            None => resto,
        };
        return desescapar_url(resto);
    }
    let casa = ctx.rutas.casa.to_string_lossy().into_owned();
    let t = if t == "~" {
        casa
    } else if let Some(r) = t.strip_prefix("~/") {
        format!("{casa}/{r}")
    } else {
        t.to_string()
    };
    abspath(Path::new(&t)).to_string_lossy().into_owned()
}

fn vacio() -> Value {
    json!({})
}

pub struct Sesion {
    ctx: Arc<Contexto>,
    notificar: Notificar,
    estado: String,
    vista: Value,
    info: Option<Arc<Info>>,
    progreso: f64,
    trabajo: String,
    resultado: Value,
    error: Value,
    pregunta: Value,
    contrasena: Value,
    pendiente: Option<(String, Opciones)>,
    instalados: Vec<Value>,
    cancelar: Arc<AtomicBool>,
    turno: u64,
    eventos: Vec<Evento>,
}

impl Sesion {
    pub fn nueva(ctx: Arc<Contexto>, notificar: Notificar) -> Sesion {
        let mut s = Sesion {
            ctx,
            notificar,
            estado: "vacio".into(),
            vista: vacio(),
            info: None,
            progreso: -1.0,
            trabajo: String::new(),
            resultado: vacio(),
            error: vacio(),
            pregunta: vacio(),
            contrasena: vacio(),
            pendiente: None,
            instalados: Vec::new(),
            cancelar: Arc::new(AtomicBool::new(false)),
            turno: 0,
            eventos: Vec::new(),
        };
        s.recargar_instalados();
        s.eventos.clear();
        s
    }

    // ------------------------------------------------------------------ lo que se ve

    pub fn contexto(&self) -> &Arc<Contexto> {
        &self.ctx
    }
    pub fn estado(&self) -> &str {
        &self.estado
    }
    pub fn archivo(&self) -> &Value {
        &self.vista
    }
    pub fn progreso(&self) -> f64 {
        self.progreso
    }
    pub fn trabajo(&self) -> &str {
        &self.trabajo
    }
    pub fn resultado(&self) -> &Value {
        &self.resultado
    }
    pub fn error(&self) -> &Value {
        &self.error
    }
    pub fn pregunta(&self) -> &Value {
        &self.pregunta
    }
    pub fn contrasena(&self) -> &Value {
        &self.contrasena
    }
    pub fn instalados(&self) -> &[Value] {
        &self.instalados
    }
    pub fn ocupado(&self) -> bool {
        self.estado == "trabajando"
    }
    pub fn tomar_eventos(&mut self) -> Vec<Evento> {
        std::mem::take(&mut self.eventos)
    }

    fn poner(&mut self, estado: &str) {
        self.estado = estado.to_string();
        self.eventos.push(Evento::Cambio);
    }

    fn aviso(&mut self, texto: String, tipo: &str) {
        self.eventos.push(Evento::Aviso(texto, tipo.into()));
    }

    // ------------------------------------------------------------------ abrir un archivo

    pub fn abrir(&mut self, texto: &str) {
        let ruta = ruta_de(&self.ctx, texto);
        if ruta.is_empty() || self.ocupado() {
            return;
        }
        self.turno += 1;
        let turno = self.turno;
        self.vista = json!({"nombre": basename(&ruta), "ruta": ruta});
        self.pregunta = vacio();
        self.contrasena = vacio();
        self.error = vacio();
        self.resultado = vacio();
        self.poner("leyendo");
        let (ctx, notificar) = (self.ctx.clone(), self.notificar.clone());
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(|| motor::analizar(&ctx, Path::new(&ruta), None))
                .map_err(|_| ctx.tr("error_general", &[("detalle", "panic")]));
            notificar(Mensaje::Analizado(r, turno));
        });
    }

    pub fn abrir_varios(&mut self, urls: &[String]) {
        let rutas: Vec<String> = urls
            .iter()
            .map(|u| ruta_de(&self.ctx, u))
            .filter(|r| !r.is_empty())
            .collect();
        if let Some(primera) = rutas.first() {
            self.abrir(&primera.clone());
            if rutas.len() > 1 {
                let t = self.ctx.tr("soltar_varios", &[]);
                self.aviso(t, "info");
            }
        }
    }

    fn al_analizar(&mut self, info: Result<Info, String>, turno: u64) {
        if turno != self.turno {
            return;
        }
        match info {
            Err(e) => {
                self.error = json!({"titulo": self.ctx.tr("no_se_pudo_abrir", &[]), "texto": e, "comando": ""});
                self.poner("error");
            }
            Ok(info) => {
                self.vista = motor::para_mostrar(&self.ctx, &info);
                self.info = Some(Arc::new(info));
                self.poner("listo");
            }
        }
    }

    pub fn cerrar_archivo(&mut self) {
        if self.ocupado() {
            return;
        }
        self.turno += 1;
        self.info = None;
        self.vista = vacio();
        self.resultado = vacio();
        self.error = vacio();
        self.pregunta = vacio();
        self.contrasena = vacio();
        self.poner("vacio");
    }

    /// Del error o del resultado, de vuelta al archivo (si sigue ahí) o a la pantalla vacía.
    pub fn volver(&mut self) {
        let sigue = self.info.as_ref().is_some_and(|i| Path::new(&i.ruta).exists());
        if sigue && self.estado == "error" {
            self.error = vacio();
            self.pregunta = vacio();
            self.contrasena = vacio();
            self.poner("listo");
        } else {
            self.cerrar_archivo();
        }
    }

    // ------------------------------------------------------------------ acciones

    pub fn ejecutar(&mut self, accion: &str, opciones: &Opciones) {
        let Some(info) = self.info.clone() else { return };
        if self.ocupado() {
            return;
        }
        let mut opciones = opciones.clone();
        if let Some(Value::String(c)) = opciones.get("carpeta").cloned()
            && !c.is_empty()
        {
            // Del selector de carpetas llega como file:///…
            opciones.insert("carpeta".into(), json!(ruta_de(&self.ctx, &c)));
        }
        if accion == "abrir_carpeta" {
            let control = Control::nulo();
            if let Err(e) = motor::ejecutar(&self.ctx, &info, accion, &opciones, &control) {
                self.aviso(e.to_string(), "error");
            }
            return;
        }
        self.turno += 1;
        let turno = self.turno;
        self.cancelar = Arc::new(AtomicBool::new(false));
        self.pendiente = Some((accion.to_string(), opciones.clone()));
        self.progreso = -1.0;
        self.trabajo = self.ctx.tr(trabajo_de(accion), &[]);
        self.pregunta = vacio();
        self.contrasena = vacio();
        self.poner("trabajando");
        self.eventos.push(Evento::Progreso);
        let (ctx, notificar, cancelar) = (self.ctx.clone(), self.notificar.clone(), self.cancelar.clone());
        let accion = accion.to_string();
        std::thread::spawn(move || {
            let n2 = notificar.clone();
            let control = Control::nuevo(move |f| n2(Mensaje::Avance(f, turno)), cancelar);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                motor::ejecutar(&ctx, &info, &accion, &opciones, &control)
            }))
            .unwrap_or_else(|_| {
                Err(Error::Problema {
                    mensaje: ctx.tr("error_general", &[("detalle", "panic")]),
                    comando: String::new(),
                })
            });
            notificar(Mensaje::Terminado(r, turno));
        });
    }

    fn al_avanzar(&mut self, f: f64, turno: u64) {
        if turno == self.turno && (f - self.progreso).abs() >= 0.005 {
            self.progreso = f;
            self.eventos.push(Evento::Progreso);
        }
    }

    fn al_terminar(&mut self, r: Resultado<Value>, turno: u64) {
        if turno != self.turno {
            return;
        }
        let (accion, opciones) = self.pendiente.clone().unwrap_or_default();
        match r {
            Err(Error::Conflicto { ruta, sugerida }) => {
                let instalacion = matches!(accion.as_str(), "instalar_appimage" | "instalar_programa");
                let sug = crate::util::nombre_de(&sugerida);
                let nombre = if instalacion {
                    self.vista.get("nombre").and_then(Value::as_str).unwrap_or("").to_string()
                } else {
                    crate::util::nombre_de(&ruta)
                };
                let mut op = opciones.clone();
                if instalacion {
                    op.insert("reemplazar".into(), json!(true));
                } else {
                    op.insert("nombre".into(), json!(sug));
                }
                let c = &self.ctx;
                self.pregunta = json!({
                    "titulo": c.tr(if instalacion { "reemplazar_titulo" } else { "conflicto_titulo" }, &[("nombre", &nombre)]),
                    "texto": if instalacion { c.tr("reemplazar_texto", &[]) } else { c.tr("conflicto_texto", &[("sugerido", &sug)]) },
                    "aceptar": if instalacion { c.tr("boton_reemplazar", &[]) } else { c.tr("boton_otro_nombre", &[("sugerido", &sug)]) },
                    "tipo": if instalacion { "peligro" } else { "primario" },
                    "accion": accion,
                    "opciones": Value::Object(op),
                });
                self.poner("listo");
            }
            Err(Error::Contrasena { incorrecta, .. }) => {
                let mut op = opciones.clone();
                op.remove("contrasena");
                self.contrasena = json!({"accion": accion, "opciones": Value::Object(op), "incorrecta": incorrecta});
                self.poner("listo");
            }
            Err(Error::Cancelado) => self.poner("listo"),
            Err(Error::Problema { mensaje, comando }) => {
                let titulo = self.ctx.tr(if comando.is_empty() { "no_se_pudo_abrir" } else { "falta_titulo" }, &[]);
                self.error = json!({"titulo": titulo, "texto": mensaje, "comando": comando});
                self.poner("error");
            }
            Err(e) => {
                let detalle = match &e {
                    Error::Io(_) => "OSError",
                    _ => "Error",
                };
                self.error = json!({"titulo": self.ctx.tr("no_se_pudo_abrir", &[]),
                                    "texto": self.ctx.tr("error_general", &[("detalle", detalle)]), "comando": ""});
                self.poner("error");
            }
            Ok(r) => {
                self.resultado = if r.is_object() { r } else { vacio() };
                self.poner("hecho");
                self.recargar_instalados();
            }
        }
    }

    pub fn cancelar(&self) {
        self.cancelar.store(true, Ordering::SeqCst);
    }

    /// «Sí» a la pregunta (otro nombre o reemplazar): se vuelve a intentar con esa opción.
    pub fn responder(&mut self) {
        let p = self.pregunta.clone();
        if p.as_object().is_some_and(|o| !o.is_empty()) {
            self.pregunta = vacio();
            let accion = p["accion"].as_str().unwrap_or("").to_string();
            let opciones = p["opciones"].as_object().cloned().unwrap_or_default();
            self.ejecutar(&accion, &opciones);
        }
    }

    pub fn descartar_pregunta(&mut self) {
        self.pregunta = vacio();
        self.contrasena = vacio();
        self.eventos.push(Evento::Cambio);
    }

    pub fn enviar_contrasena(&mut self, texto: &str) {
        let c = self.contrasena.clone();
        if !texto.is_empty() && c.as_object().is_some_and(|o| !o.is_empty()) {
            self.contrasena = vacio();
            let accion = c["accion"].as_str().unwrap_or("").to_string();
            let mut opciones = c["opciones"].as_object().cloned().unwrap_or_default();
            opciones.insert("contrasena".into(), json!(texto));
            self.ejecutar(&accion, &opciones);
        }
    }

    // ------------------------------------------------------------------ resultado

    pub fn abrir_resultado(&mut self) {
        let r = self.resultado.clone();
        let texto = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let hecho: Result<(), Error> = if !texto("id").is_empty() {
            motor::abrir_instalado(&self.ctx, &texto("id"))
        } else if !texto("flatpak").is_empty() {
            flatpak::abrir(&self.ctx, &texto("flatpak"))
        } else if !texto("carpeta").is_empty() {
            instalar::abrir_carpeta(&self.ctx, Path::new(&texto("carpeta")))
        } else {
            Ok(())
        };
        if let Err(e) = hecho {
            self.aviso(e.to_string(), "error");
        }
    }

    pub fn abrir_carpeta(&mut self, ruta: &str) {
        if let Err(e) = instalar::abrir_carpeta(&self.ctx, &PathBuf::from(ruta)) {
            self.aviso(e.to_string(), "error");
        }
    }

    // ------------------------------------------------------------------ «Instalado»

    pub fn recargar_instalados(&mut self) {
        self.instalados = motor::instalados(&self.ctx);
        self.eventos.push(Evento::Instalados);
    }

    pub fn deshacer(&mut self, id: &str) {
        let (ctx, notificar, id) = (self.ctx.clone(), self.notificar.clone(), id.to_string());
        std::thread::spawn(move || {
            let r = motor::deshacer(&ctx, &id).map_err(|e| e.to_string());
            notificar(Mensaje::Deshecho(r));
        });
    }

    pub fn abrir_instalado(&mut self, id: &str) {
        if let Err(e) = motor::abrir_instalado(&self.ctx, id) {
            self.aviso(e.to_string(), "error");
        }
    }

    /// Un mensaje de un hilo de fondo, ya en el hilo de quien tiene la sesión.
    pub fn procesar(&mut self, m: Mensaje) {
        match m {
            Mensaje::Analizado(info, turno) => self.al_analizar(info, turno),
            Mensaje::Terminado(r, turno) => self.al_terminar(r, turno),
            Mensaje::Avance(f, turno) => self.al_avanzar(f, turno),
            Mensaje::Deshecho(r) => {
                self.recargar_instalados();
                match r {
                    Ok(t) => self.aviso(t, "info"),
                    Err(t) => self.aviso(t, "error"),
                }
            }
        }
    }
}

/// Para el binario y las pruebas: recoge un mapa de opciones de un JSON.
pub fn opciones_de(v: &Value) -> Opciones {
    v.as_object().cloned().unwrap_or_else(Map::new)
}
