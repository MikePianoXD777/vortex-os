//! `instalador`: lo mismo que `puente.py`, envolviendo `vortex_app_installer::sesion::Sesion` (el motor ya
//! probado). Mismos nombres de propiedades, señales y slots, para que los QML de la app de Python no cambien.
//!
//! Las propiedades que en Python son `QVariantMap` o `QVariantList` aquí son `QVariant` con el mapa o la
//! lista dentro (QML los ve igual, como objetos de JavaScript). Cada llamada a la sesión deja eventos
//! (avisos, «cambió el estado»…); `vaciar` los convierte en propiedades y señales de QML.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, estado)]
        #[qproperty(QVariant, archivo)]
        #[qproperty(f64, progreso)]
        #[qproperty(QString, trabajo)]
        #[qproperty(QVariant, resultado)]
        #[qproperty(QVariant, error)]
        #[qproperty(QVariant, pregunta)]
        #[qproperty(QVariant, contrasena)]
        #[qproperty(QVariant, instalados)]
        #[qproperty(QVariant, t)]
        #[qproperty(bool, ocupado)]
        type Puente = super::PuenteRust;

        /// texto, «info» o «error»
        #[qsignal]
        fn aviso(self: Pin<&mut Puente>, texto: &QString, tipo: &QString);
        /// Algo del estado cambió (la señal de siempre de `puente.py`: Main.qml la usa al empezar a leer).
        #[qsignal]
        fn cambio(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        fn abrir(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        #[cxx_name = "abrirVarios"]
        fn abrir_varios(self: Pin<&mut Puente>, urls: &QVariant);
        #[qinvokable]
        #[cxx_name = "cerrarArchivo"]
        fn cerrar_archivo(self: Pin<&mut Puente>);
        #[qinvokable]
        fn volver(self: Pin<&mut Puente>);
        #[qinvokable]
        fn ejecutar(self: Pin<&mut Puente>, accion: &QString, opciones: &QVariant);
        #[qinvokable]
        fn cancelar(self: Pin<&mut Puente>);
        #[qinvokable]
        fn responder(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "descartarPregunta"]
        fn descartar_pregunta(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "enviarContrasena"]
        fn enviar_contrasena(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        #[cxx_name = "abrirResultado"]
        fn abrir_resultado(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "abrirCarpeta"]
        fn abrir_carpeta(self: Pin<&mut Puente>, ruta: &QString);
        #[qinvokable]
        fn copiar(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        #[cxx_name = "recargarInstalados"]
        fn recargar_instalados(self: Pin<&mut Puente>);
        #[qinvokable]
        fn deshacer(self: Pin<&mut Puente>, id: &QString);
        #[qinvokable]
        #[cxx_name = "abrirInstalado"]
        fn abrir_instalado(self: Pin<&mut Puente>, id: &QString);
        /// Una sola ventana por persona: lo que pida otra copia (mostrar, o abrir un archivo) llega por un
        /// socket. Lo llama el arranque (no en las pruebas ni en las capturas).
        #[qinvokable]
        fn escuchar(self: Pin<&mut Puente>);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use crate::arranque::ffi::{copiar_texto, json_a_variante, variante_a_json};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::{Map, Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vortex_app_installer::sesion::{Evento, Mensaje, Notificar, Sesion};
use vortex_app_installer::textos;
use vortex_app_installer::{Contexto, instancia};

pub struct PuenteRust {
    sesion: Option<Sesion>,
    estado: QString,
    archivo: QVariant,
    progreso: f64,
    trabajo: QString,
    resultado: QVariant,
    error: QVariant,
    pregunta: QVariant,
    contrasena: QVariant,
    instalados: QVariant,
    t: QVariant,
    ocupado: bool,
}

impl Default for PuenteRust {
    fn default() -> Self {
        let vacio = || json_a_variante("{}");
        PuenteRust {
            sesion: None,
            estado: QString::from("vacio"),
            archivo: vacio(),
            progreso: -1.0,
            trabajo: QString::default(),
            resultado: vacio(),
            error: vacio(),
            pregunta: vacio(),
            contrasena: vacio(),
            instalados: json_a_variante("[]"),
            t: vacio(),
            ocupado: false,
        }
    }
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn variante(v: &Value) -> QVariant {
    json_a_variante(&v.to_string())
}

/// Lo que manda QML (objeto, lista, `QUrl`…) como JSON de serde.
fn de_variante(v: &QVariant) -> Value {
    serde_json::from_str(&variante_a_json(v)).unwrap_or(Value::Null)
}

/// Hyprland ignora requestActivate() (misc:focus_on_activate apagado): se le pide directo. Primero la
/// sintaxis de la config en Lua (Hyprland 0.56+); si falla, la de antes.
fn enfocar_en_hyprland() {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let ventana = format!("pid:{}", std::process::id());
    for orden in [
        format!("hl.dsp.focus({{ window = \"{ventana}\" }})"),
        format!("focuswindow {ventana}"),
    ] {
        match std::process::Command::new("hyprctl")
            .args(["dispatch", &orden])
            .output()
        {
            Ok(r) if r.status.success() => return,
            Ok(_) => {}
            Err(_) => return,
        }
    }
}

impl qobject::Puente {
    fn sesion(self: Pin<&mut Self>) -> &mut Sesion {
        self.rust_mut()
            .get_mut()
            .sesion
            .as_mut()
            .expect("la sesión se crea al iniciar")
    }

    /// Pasa lo que dejó la sesión a QML: propiedades y señales.
    fn vaciar(mut self: Pin<&mut Self>) {
        let eventos = self.as_mut().sesion().tomar_eventos();
        for ev in eventos {
            match ev {
                Evento::Cambio => self.as_mut().sincronizar(),
                Evento::Progreso => {
                    let p = self.as_mut().sesion().progreso();
                    self.as_mut().set_progreso(p);
                }
                Evento::Instalados => {
                    let l = Value::Array(self.as_mut().sesion().instalados().to_vec());
                    self.as_mut().set_instalados(variante(&l));
                }
                Evento::Aviso(texto, tipo) => self.as_mut().aviso(&q(&texto), &q(&tipo)),
                Evento::Mostrar => self.as_mut().mostrar_ventana(),
            }
        }
    }

    fn sincronizar(mut self: Pin<&mut Self>) {
        let (estado, archivo, trabajo, resultado, error, pregunta, contrasena, ocupado) = {
            let s = self.as_mut().sesion();
            (
                s.estado().to_string(),
                s.archivo().clone(),
                s.trabajo().to_string(),
                s.resultado().clone(),
                s.error().clone(),
                s.pregunta().clone(),
                s.contrasena().clone(),
                s.ocupado(),
            )
        };
        self.as_mut().set_estado(q(&estado));
        self.as_mut().set_archivo(variante(&archivo));
        self.as_mut().set_trabajo(q(&trabajo));
        self.as_mut().set_resultado(variante(&resultado));
        self.as_mut().set_error(variante(&error));
        self.as_mut().set_pregunta(variante(&pregunta));
        self.as_mut().set_contrasena(variante(&contrasena));
        self.as_mut().set_ocupado(ocupado);
        self.cambio();
    }

    /// Un mensaje de un hilo de fondo, ya en el hilo de la ventana.
    fn procesar(mut self: Pin<&mut Self>, m: Mensaje) {
        self.as_mut().sesion().procesar(m);
        self.vaciar();
    }

    fn abrir(mut self: Pin<&mut Self>, texto: &QString) {
        self.as_mut().sesion().abrir(&texto.to_string());
        self.vaciar();
    }

    fn abrir_varios(mut self: Pin<&mut Self>, urls: &QVariant) {
        let lista: Vec<String> = match de_variante(urls) {
            Value::Array(v) => v.iter().filter_map(|u| u.as_str().map(str::to_string)).collect(),
            Value::String(s) => vec![s],
            _ => Vec::new(),
        };
        self.as_mut().sesion().abrir_varios(&lista);
        self.vaciar();
    }

    fn cerrar_archivo(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().cerrar_archivo();
        self.vaciar();
    }

    fn volver(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().volver();
        self.vaciar();
    }

    fn ejecutar(mut self: Pin<&mut Self>, accion: &QString, opciones: &QVariant) {
        let opciones: Map<String, Value> = de_variante(opciones).as_object().cloned().unwrap_or_default();
        self.as_mut().sesion().ejecutar(&accion.to_string(), &opciones);
        self.vaciar();
    }

    fn cancelar(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().cancelar();
    }

    fn responder(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().responder();
        self.vaciar();
    }

    fn descartar_pregunta(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().descartar_pregunta();
        self.vaciar();
    }

    fn enviar_contrasena(mut self: Pin<&mut Self>, texto: &QString) {
        self.as_mut().sesion().enviar_contrasena(&texto.to_string());
        self.vaciar();
    }

    fn abrir_resultado(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().abrir_resultado();
        self.vaciar();
    }

    fn abrir_carpeta(mut self: Pin<&mut Self>, ruta: &QString) {
        self.as_mut().sesion().abrir_carpeta(&ruta.to_string());
        self.vaciar();
    }

    fn copiar(self: Pin<&mut Self>, texto: &QString) {
        copiar_texto(&texto.to_string());
    }

    fn recargar_instalados(mut self: Pin<&mut Self>) {
        self.as_mut().sesion().recargar_instalados();
        self.vaciar();
    }

    fn deshacer(mut self: Pin<&mut Self>, id: &QString) {
        self.as_mut().sesion().deshacer(&id.to_string());
        self.vaciar();
    }

    fn abrir_instalado(mut self: Pin<&mut Self>, id: &QString) {
        self.as_mut().sesion().abrir_instalado(&id.to_string());
        self.vaciar();
    }

    fn escuchar(self: Pin<&mut Self>) {
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                for m in escucha.revisar() {
                    let r = hilo.queue(move |mut obj| {
                        if !m.archivo.is_empty() {
                            obj.as_mut().abrir(&q(&m.archivo));
                        }
                        obj.as_mut().mostrar_ventana();
                        enfocar_en_hyprland();
                    });
                    if r.is_err() {
                        return; // la ventana ya se cerró
                    }
                }
            }
        });
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        let hilo = Mutex::new(self.qt_thread());
        let notificar: Notificar = Arc::new(move |m: Mensaje| {
            let m = Mutex::new(Some(m));
            if let Ok(h) = hilo.lock() {
                let _ = h.queue(move |obj| {
                    if let Some(m) = m.lock().ok().and_then(|mut g| g.take()) {
                        obj.procesar(m);
                    }
                });
            }
        });
        let ctx = Arc::new(Contexto::del_entorno());
        let textos: Map<String, Value> = textos::todos(ctx.idioma)
            .into_iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect();
        self.as_mut().rust_mut().get_mut().sesion = Some(Sesion::nueva(ctx, notificar));
        self.as_mut().set_t(variante(&Value::Object(textos)));
        self.as_mut().sincronizar();
        let l = Value::Array(self.as_mut().sesion().instalados().to_vec());
        self.as_mut().set_instalados(variante(&l));
        self.vaciar();
    }
}
