//! `mk`: lo que `ui_panel.py` y `ui_avatar.py` hacían con las señales del asistente, en un solo objeto para QML.
//! El asistente (`vortex_miku::asistente`) habla desde su hilo; cada señal vuelve al hilo de la interfaz con
//! `qt_thread().queue` y aquí se vuelve propiedad (el panel de voz) y llamada de JavaScript (el avatar).
//! También es el objeto «puente» que ve la página web por QWebChannel (`listo`, `enviarTexto`, `microfono`,
//! `cerrar`, `tocada`, `accion`) y trae la paleta del fondo en vivo (`colores.py`).
//!
//! Con `VORTEX_MIKU_PRUEBA` no hay asistente: lo que la interfaz le pediría se anota en `registro()` y las
//! señales se simulan con `simular()` (lo que hacía `AsistenteFalso` en las pruebas de Python).
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, estado)]
        #[qproperty(QString, estado_texto, cxx_name = "estadoTexto")]
        #[qproperty(QString, tu)]
        #[qproperty(QString, respuesta)]
        #[qproperty(bool, es_aviso, cxx_name = "esAviso")]
        #[qproperty(QString, vista_ruta, cxx_name = "vistaRuta")]
        #[qproperty(i32, vista_x, cxx_name = "vistaX")]
        #[qproperty(i32, vista_y, cxx_name = "vistaY")]
        #[qproperty(f64, nivel)]
        #[qproperty(QString, progreso_titulo, cxx_name = "progresoTitulo")]
        #[qproperty(QString, progreso_detalle, cxx_name = "progresoDetalle")]
        #[qproperty(f64, progreso)]
        #[qproperty(QVariant, acciones)]
        #[qproperty(bool, camara)]
        #[qproperty(bool, avatar)]
        #[qproperty(QString, pagina)]
        #[qproperty(f64, escala)]
        #[qproperty(QMap_QString_QVariant, paleta)]
        type Miku = super::MikuRust;

        /// Una llamada de JavaScript para la página del avatar.
        #[qsignal]
        fn js(self: Pin<&mut Miku>, codigo: &QString);
        /// La ventana tiene que cerrarse (el asistente lo pidió o la persona dio Esc / ✕).
        #[qsignal]
        #[cxx_name = "cerrarPedido"]
        fn cerrar_pedido(self: Pin<&mut Miku>);

        #[qinvokable]
        fn iniciar(self: Pin<&mut Miku>);
        /// La página web conectó el canal: se le manda el tema, el crédito y lo que se acumuló.
        #[qinvokable]
        fn listo(self: Pin<&mut Miku>);
        #[qinvokable]
        #[cxx_name = "enviarTexto"]
        fn enviar_texto(self: Pin<&mut Miku>, texto: &QString);
        #[qinvokable]
        fn microfono(self: Pin<&mut Miku>);
        #[qinvokable]
        fn tocada(self: Pin<&mut Miku>);
        #[qinvokable]
        fn accion(self: Pin<&mut Miku>, id: &QString);
        #[qinvokable]
        fn cerrar(self: Pin<&mut Miku>);
        /// Al salir: el asistente cierra la voz, el micrófono y lo que tenga abierto.
        #[qinvokable]
        #[cxx_name = "cerrarTodo"]
        fn cerrar_todo(self: Pin<&mut Miku>);
        /// QtWebEngine no cargó: sigue como panel de voz.
        #[qinvokable]
        #[cxx_name = "sinWebEngine"]
        fn sin_web_engine(self: Pin<&mut Miku>);
        /// Pruebas: lo que la interfaz le pidió al asistente, una cosa por línea.
        #[qinvokable]
        fn registro(self: &Miku) -> QString;
        /// Pruebas: una señal del asistente como JSON, p. ej. `["estado","escuchando","Escuchando..."]`.
        #[qinvokable]
        fn simular(self: Pin<&mut Miku>, senal: &QString);
    }

    impl cxx_qt::Initialize for Miku {}
    impl cxx_qt::Threading for Miku {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QString, QVariant};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use vortex_core::paleta::{Paleta, Vigia};
use vortex_miku::asistente::{Arranque, Asistente, Entorno, Senal};
use vortex_miku::config::Config;
use vortex_miku::interfaz;
use vortex_miku::rutas::Rutas;

/// Lo que decide `main` antes de crear la app: el modo, la nota si hubo que bajar a solo voz y la configuración.
pub struct Modo {
    pub avatar: bool,
    pub nota: String,
    pub cfg: Config,
}

pub static MODO: OnceLock<Modo> = OnceLock::new();

pub struct MikuRust {
    asistente: Option<Arc<Asistente>>,
    prueba: bool,
    anotado: Vec<String>,
    pagina_lista: bool,
    pendientes: Vec<String>,
    credito: String,
    tema: Value,
    nota: String,
    cerrando: bool,
    // Las propiedades.
    estado: QString,
    estado_texto: QString,
    tu: QString,
    respuesta: QString,
    es_aviso: bool,
    vista_ruta: QString,
    vista_x: i32,
    vista_y: i32,
    nivel: f64,
    progreso_titulo: QString,
    progreso_detalle: QString,
    progreso: f64,
    acciones: QVariant,
    camara: bool,
    avatar: bool,
    pagina: QString,
    escala: f64,
    paleta: QMap<QMapPair_QString_QVariant>,
}

impl Default for MikuRust {
    fn default() -> Self {
        Self {
            asistente: None,
            prueba: false,
            anotado: vec![],
            pagina_lista: false,
            pendientes: vec![],
            credito: String::new(),
            tema: Value::Null,
            nota: String::new(),
            cerrando: false,
            estado: QString::from("reposo"),
            estado_texto: QString::from("Lista"),
            tu: QString::default(),
            respuesta: QString::default(),
            es_aviso: false,
            vista_ruta: QString::default(),
            vista_x: 0,
            vista_y: 0,
            nivel: 0.0,
            progreso_titulo: QString::default(),
            progreso_detalle: QString::default(),
            progreso: -1.0,
            acciones: QVariant::default(),
            camara: false,
            avatar: false,
            pagina: QString::default(),
            escala: 1.0,
            paleta: QMap::default(),
        }
    }
}

fn q(t: &str) -> QString {
    QString::from(t)
}

fn variante(v: &Value) -> QVariant {
    crate::arranque::ffi::json_a_variante(&v.to_string())
}

fn a_qmap(p: &Paleta) -> QMap<QMapPair_QString_QVariant> {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in p {
        m.insert(q(k), QVariant::from(&q(v)));
    }
    m
}

impl cxx_qt::Initialize for qobject::Miku {
    fn initialize(mut self: Pin<&mut Self>) {
        let rutas = Rutas::real();
        let modo = MODO.get();
        let (avatar, nota) = modo.map_or((false, String::new()), |m| (m.avatar, m.nota.clone()));
        let prueba = std::env::var_os("VORTEX_MIKU_PRUEBA").is_some_and(|v| !v.is_empty());
        let escala = interfaz::escala_ui(&interfaz::ajustes_shell());

        // La paleta del fondo, siguiéndola en vivo (VORTEX_MIKU_COLORES = otro archivo, para las pruebas).
        let mut vigia = match std::env::var_os("VORTEX_MIKU_COLORES").filter(|v| !v.is_empty()) {
            Some(r) => Vigia::new(PathBuf::from(r)),
            None => Vigia::de_matugen(),
        };
        let paleta = vigia.actual().clone();
        let tema = interfaz::variables_css(&paleta);
        let hilo = self.qt_thread();
        std::thread::spawn({
            let hilo = hilo.clone();
            move || {
                loop {
                    std::thread::sleep(Duration::from_millis(300));
                    if let Some(nueva) = vigia.revisar().cloned()
                        && hilo.queue(move |obj| obj.nueva_paleta(nueva)).is_err()
                    {
                        break; // la ventana ya se cerró
                    }
                }
            }
        });

        // El modelo y la página del avatar.
        let modelo = avatar
            .then(|| modo.and_then(|m| rutas.ruta_modelo(&m.cfg.modelo)))
            .flatten();
        let credito = Rutas::leer_credito(modelo.as_deref());
        if avatar {
            let url = interfaz::url_pagina(&rutas, modelo.as_deref(), 30, &interfaz::extras_de_prueba());
            self.as_mut().set_pagina(q(&url));
        }

        // El asistente (en pruebas no hay: se simula).
        let asistente = (!prueba)
            .then(|| {
                let cfg = modo.map(|m| m.cfg.clone())?;
                let salida: vortex_miku::asistente::Salida = Arc::new(move |s| {
                    let _ = hilo.queue(move |obj| obj.aplicar(s));
                });
                Some(Asistente::nuevo(
                    cfg,
                    avatar,
                    Entorno::real(rutas),
                    Arranque::del_entorno(),
                    salida,
                ))
            })
            .flatten();

        self.as_mut().set_avatar(avatar);
        self.as_mut().set_escala(escala);
        self.as_mut().set_paleta(a_qmap(&paleta));
        let mut m = self.as_mut().rust_mut();
        m.asistente = asistente;
        m.prueba = prueba;
        m.credito = credito;
        m.tema = tema;
        m.nota = nota;
    }
}

impl qobject::Miku {
    fn anotar(mut self: Pin<&mut Self>, t: impl Into<String>) {
        self.as_mut().rust_mut().anotado.push(t.into());
    }

    /// Una llamada de JavaScript: si la página aún no está lista, espera.
    fn enviar_js(mut self: Pin<&mut Self>, codigo: String) {
        if !*self.avatar() {
            return;
        }
        if self.rust().pagina_lista {
            self.js(&q(&codigo));
        } else {
            self.as_mut().rust_mut().pendientes.push(codigo);
        }
    }

    fn nueva_paleta(mut self: Pin<&mut Self>, p: Paleta) {
        let tema = interfaz::variables_css(&p);
        self.as_mut().set_paleta(a_qmap(&p));
        self.as_mut().rust_mut().tema = tema.clone();
        self.enviar_js(interfaz::llamada("tema", &[tema]));
    }

    /// Una señal del asistente, ya en el hilo de la interfaz.
    pub fn aplicar(mut self: Pin<&mut Self>, s: Senal) {
        if let Some(c) = interfaz::js_de(&s) {
            self.as_mut().enviar_js(c);
        }
        match s {
            Senal::Estado(e, t) => {
                let texto = if t.is_empty() { etiqueta(&e) } else { t };
                self.as_mut().set_estado(q(&e));
                self.as_mut().set_estado_texto(q(&texto));
            }
            Senal::Usuario(t) => {
                self.as_mut().set_tu(q(&format!("Tú: {t}")));
                self.as_mut().set_respuesta(QString::default());
                self.as_mut().set_es_aviso(false);
            }
            Senal::Respuesta(t, fin) => {
                self.as_mut().set_es_aviso(false);
                let t = if t.is_empty() && !fin {
                    "...".to_string()
                } else {
                    t
                };
                self.as_mut().set_respuesta(q(&t));
            }
            Senal::Aviso(t) => {
                self.as_mut().set_es_aviso(true);
                self.as_mut().set_respuesta(q(&t));
            }
            Senal::Vista(ruta, x, y) => {
                self.as_mut().set_vista_x(x as i32);
                self.as_mut().set_vista_y(y as i32);
                self.as_mut().set_vista_ruta(q(&ruta));
            }
            Senal::Nivel(v) => self.as_mut().set_nivel(v),
            Senal::Progreso(t, d, f) => {
                self.as_mut().set_progreso_titulo(q(&t));
                self.as_mut().set_progreso_detalle(q(&d));
                self.as_mut().set_progreso(f);
            }
            Senal::Acciones(l) => {
                let v = Value::Array(l.iter().map(|(i, t)| json!({"id": i, "texto": t})).collect());
                self.as_mut().set_acciones(variante(&v));
            }
            Senal::Camara(b) => self.as_mut().set_camara(b),
            Senal::Cerrar => self.cerrar_pedido(),
            Senal::Boca(..)
            | Senal::BocaFin
            | Senal::Emocion(..)
            | Senal::Sonrojo(_)
            | Senal::Afecto(..)
            | Senal::Boda(_) => {}
        }
    }

    // --- lo que pide la interfaz ---

    fn iniciar(mut self: Pin<&mut Self>) {
        let nota = std::mem::take(&mut self.as_mut().rust_mut().nota);
        if !nota.is_empty() {
            self.as_mut().aplicar(Senal::Aviso(nota));
        }
        // Sin modelo 3D la página avisa sola (sin ?modelo=) con «vortex-miku modelo RUTA.vrm».
        if let Some(a) = self.rust().asistente.clone() {
            a.iniciar();
        }
    }

    fn listo(mut self: Pin<&mut Self>) {
        if self.rust().prueba {
            self.as_mut().anotar("listo");
        }
        let credito = self.rust().credito.clone();
        let primera = credito.lines().next().unwrap_or("");
        let corta = if primera.chars().count() <= 90 {
            primera.to_string()
        } else {
            format!("{}...", primera.chars().take(87).collect::<String>())
        };
        let tema = self.rust().tema.clone();
        let pendientes = std::mem::take(&mut self.as_mut().rust_mut().pendientes);
        self.as_mut().rust_mut().pagina_lista = true;
        self.as_mut().enviar_js(interfaz::llamada("tema", &[tema]));
        self.as_mut()
            .enviar_js(interfaz::llamada("credito", &[json!(credito), json!(corta)]));
        for c in pendientes {
            self.as_mut().enviar_js(c);
        }
    }

    fn enviar_texto(mut self: Pin<&mut Self>, texto: &QString) {
        let t = texto.to_string();
        match self.rust().asistente.clone() {
            Some(a) => a.enviar(&t),
            None => self.as_mut().anotar(format!("enviar:{t}")),
        }
    }

    fn microfono(mut self: Pin<&mut Self>) {
        let escuchando = self.estado().to_string() == "escuchando";
        match self.rust().asistente.clone() {
            Some(a) if escuchando => a.cortar_escucha(),
            Some(a) => a.escuchar(),
            None => self
                .as_mut()
                .anotar(if escuchando { "cortar" } else { "escuchar" }),
        }
    }

    fn tocada(mut self: Pin<&mut Self>) {
        match self.rust().asistente.clone() {
            Some(a) => a.tocada(),
            None => self.as_mut().anotar("tocada"),
        }
    }

    fn accion(mut self: Pin<&mut Self>, id: &QString) {
        let i = id.to_string();
        self.as_mut().tocada();
        match self.rust().asistente.clone() {
            Some(a) => a.accion(&i),
            None => self.as_mut().anotar(format!("accion:{i}")),
        }
    }

    fn cerrar(mut self: Pin<&mut Self>) {
        if self.rust().prueba {
            self.as_mut().anotar("cerrar");
        } else {
            self.cerrar_pedido();
        }
    }

    fn cerrar_todo(mut self: Pin<&mut Self>) {
        if std::mem::replace(&mut self.as_mut().rust_mut().cerrando, true) {
            return;
        }
        if let Some(a) = self.rust().asistente.clone() {
            a.cerrar_todo();
        }
    }

    fn sin_web_engine(mut self: Pin<&mut Self>) {
        self.as_mut().set_avatar(false);
        self.as_mut()
            .aplicar(Senal::Aviso(interfaz::SIN_WEBENGINE.into()));
    }

    fn registro(&self) -> QString {
        q(&self.rust().anotado.join("\n"))
    }

    fn simular(self: Pin<&mut Self>, senal: &QString) {
        let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&senal.to_string()) else {
            return;
        };
        let t = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        let n = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        let s = match t(0).as_str() {
            "estado" => Senal::Estado(t(1), t(2)),
            "usuario" => Senal::Usuario(t(1)),
            "respuesta" => Senal::Respuesta(t(1), a.get(2).and_then(Value::as_bool).unwrap_or(true)),
            "aviso" => Senal::Aviso(t(1)),
            "vista" => Senal::Vista(t(1), n(2) as i64, n(3) as i64),
            "nivel" => Senal::Nivel(n(1)),
            "progreso" => Senal::Progreso(t(1), t(2), n(3)),
            "acciones" => Senal::Acciones(
                a.get(1)
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .map(|p| {
                                (
                                    p[0].as_str().unwrap_or("").to_string(),
                                    p[1].as_str().unwrap_or("").to_string(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            "camara" => Senal::Camara(a.get(1).and_then(Value::as_bool).unwrap_or(false)),
            "emocion" => Senal::Emocion(t(1), n(2)),
            "sonrojo" => Senal::Sonrojo(n(1)),
            "afecto" => Senal::Afecto(n(1), t(2)),
            "boda" => Senal::Boda(t(1)),
            "boca" => Senal::Boca(vec![vec![0, 0, 0, 0, 0]], 30),
            "boca_fin" => Senal::BocaFin,
            "cerrar" => Senal::Cerrar,
            _ => return,
        };
        self.aplicar(s);
    }
}

const fn etiqueta_de(e: &str) -> &'static str {
    match e.as_bytes() {
        b"escuchando" => "Escuchando...",
        b"transcribiendo" => "Un momento...",
        b"pensando" => "Pensando...",
        b"viendo" => "Mirando tu pantalla...",
        b"hablando" => "Hablando",
        _ => "Lista",
    }
}

fn etiqueta(e: &str) -> String {
    etiqueta_de(e).to_string()
}
