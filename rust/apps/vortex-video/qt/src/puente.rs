//! `vv`: lo mismo que `puente.py`, envolviendo `vortex_video::estado::Estado` (el motor ya probado).
//! Mismos nombres de slots y propiedades, para que `Main.qml` y `PanelAjustes.qml` no cambien.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, ultima)]
        #[qproperty(QString, miniatura)]
        #[qproperty(bool, ultima_es_video, cxx_name = "ultimaEsVideo")]
        #[qproperty(bool, sin_camara, cxx_name = "sinCamara")]
        type Puente = super::PuenteRust;

        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        fn ajuste(self: &Puente, clave: &QString) -> QVariant;
        #[qinvokable]
        fn guardar(self: Pin<&mut Puente>, clave: &QString, valor: &QVariant);
        #[qinvokable]
        #[cxx_name = "elegirFormato"]
        fn elegir_formato(self: &Puente, formatos: &QList_QVariant, preferida: &QString) -> i32;
        #[qinvokable]
        fn resoluciones(self: &Puente, formatos: &QList_QVariant) -> QStringList;
        #[qinvokable]
        #[cxx_name = "etiquetaResolucion"]
        fn etiqueta_resolucion(self: &Puente, texto: &QString) -> QString;
        #[qinvokable]
        fn duracion(self: &Puente, ms: i64) -> QString;
        #[qinvokable]
        #[cxx_name = "rutaFoto"]
        fn ruta_foto(self: &Puente) -> QString;
        #[qinvokable]
        #[cxx_name = "urlVideo"]
        fn url_video(self: &Puente) -> QString;
        #[qinvokable]
        fn registrar(self: Pin<&mut Puente>, ruta: &QString);
        #[qinvokable]
        #[cxx_name = "abrirUltima"]
        fn abrir_ultima(self: &Puente);
        #[qinvokable]
        #[cxx_name = "abrirCarpeta"]
        fn abrir_carpeta(self: &Puente, modo: &QString);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QStringList, QUrl, QVariant};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use vortex_video::estado::Estado;
use vortex_video::formatos::{self, Formato};
use vortex_video::hora::FechaHora;
use vortex_video::{capturas, instancia, miniatura};

pub struct PuenteRust {
    estado: Estado,
    ultima: QString,
    miniatura: QString,
    ultima_es_video: bool,
    sin_camara: bool,
}

impl Default for PuenteRust {
    fn default() -> Self {
        PuenteRust {
            estado: Estado::new(),
            ultima: QString::default(),
            miniatura: QString::default(),
            ultima_es_video: false,
            sin_camara: std::env::var_os("VORTEX_VIDEO_SIN_CAMARA").is_some_and(|v| !v.is_empty()),
        }
    }
}

fn url_de(ruta: &Path) -> QString {
    QUrl::from_local_file(&QString::from(ruta.to_string_lossy().as_ref())).to_qstring()
}

/// De QVariant (lo que manda QML) a JSON, con el tipo que ya tiene esa clave: QVariant convierte de más
/// (el texto «video» también «es» `true`), así que se pide justo el tipo esperado.
fn a_json(v: &QVariant, como: &Value) -> Option<Value> {
    match como {
        Value::Bool(_) => v.value::<bool>().map(Value::Bool),
        Value::Number(_) => {
            let f = v.value::<f64>().or_else(|| v.value::<i32>().map(f64::from))?;
            // QML manda los números como double: 5.0 es el temporizador 5.
            if f.fract() == 0.0 {
                Some(Value::from(f as i64))
            } else {
                serde_json::Number::from_f64(f).map(Value::Number)
            }
        }
        Value::String(_) => v.value::<QString>().map(|s| Value::String(s.to_string())),
        _ => None,
    }
}

fn a_qvariant(v: &Value) -> QVariant {
    match v {
        Value::Bool(b) => QVariant::from(b),
        Value::Number(n) => n
            .as_i64()
            .map(|i| QVariant::from(&(i as i32)))
            .unwrap_or_else(|| QVariant::from(&n.as_f64().unwrap_or(0.0))),
        Value::String(s) => QVariant::from(&QString::from(s.as_str())),
        _ => QVariant::default(),
    }
}

/// `{ancho, alto, fps}` de cada formato (Main.qml los aplana así antes de mandarlos).
fn planos(lista: &QList<QVariant>) -> Vec<Formato> {
    let numero = |m: &QMap<QMapPair_QString_QVariant>, k: &str| -> f64 {
        m.get(&QString::from(k))
            .and_then(|v| v.value::<f64>().or_else(|| v.value::<i32>().map(f64::from)))
            .unwrap_or(0.0)
    };
    lista
        .iter()
        .filter_map(|v| v.value::<QMap<QMapPair_QString_QVariant>>())
        .map(|m| {
            Formato::new(
                numero(&m, "ancho") as u32,
                numero(&m, "alto") as u32,
                numero(&m, "fps"),
            )
        })
        .collect()
}

fn abrir(ruta: &Path) {
    // Como QDesktopServices.openUrl: la app que la persona tenga para ese tipo de archivo.
    let _ = std::process::Command::new("xdg-open")
        .arg(ruta)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

impl qobject::Puente {
    fn ajuste(&self, clave: &QString) -> QVariant {
        self.rust()
            .estado
            .ajuste(&clave.to_string())
            .map(|v| a_qvariant(&v))
            .unwrap_or_default()
    }

    fn guardar(mut self: Pin<&mut Self>, clave: &QString, valor: &QVariant) {
        let clave = clave.to_string();
        let Some(actual) = self.rust().estado.ajuste(&clave) else {
            return;
        };
        if let Some(v) = a_json(valor, &actual) {
            self.as_mut().rust_mut().estado.guardar(&clave, v);
        }
    }

    fn elegir_formato(&self, lista: &QList<QVariant>, preferida: &QString) -> i32 {
        formatos::elegir(&planos(lista), &preferida.to_string()).map_or(-1, |i| i as i32)
    }

    fn resoluciones(&self, lista: &QList<QVariant>) -> QStringList {
        let mut l = QList::<QString>::default();
        for r in formatos::resoluciones(&planos(lista)) {
            l.append(QString::from(r.as_str()));
        }
        QStringList::from(&l)
    }

    fn etiqueta_resolucion(&self, texto: &QString) -> QString {
        QString::from(formatos::etiqueta(&texto.to_string()).as_str())
    }

    fn duracion(&self, ms: i64) -> QString {
        QString::from(formatos::duracion(ms).as_str())
    }

    fn ruta_foto(&self) -> QString {
        let r = self.rust().estado.ruta_foto(FechaHora::ahora());
        QString::from(
            r.map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_str(),
        )
    }

    fn url_video(&self) -> QString {
        match self.rust().estado.ruta_video(FechaHora::ahora()) {
            Some(p) => url_de(&p),
            None => url_de(Path::new("")),
        }
    }

    fn registrar(mut self: Pin<&mut Self>, ruta: &QString) {
        if self.as_mut().rust_mut().estado.registrar(&ruta.to_string()) {
            self.fijar_ultima();
        }
    }

    /// Como `_fijar_ultima` de Python: la foto se ve tal cual; del video se saca un cuadro en otro hilo.
    fn fijar_ultima(mut self: Pin<&mut Self>) {
        let Some(ruta) = self.rust().estado.ultima().map(Path::to_path_buf) else {
            return;
        };
        let video = capturas::es_video(&ruta);
        self.as_mut()
            .set_ultima(QString::from(ruta.to_string_lossy().as_ref()));
        self.as_mut().set_ultima_es_video(video);
        if !video {
            self.as_mut().set_miniatura(url_de(&ruta));
            return;
        }
        self.as_mut().set_miniatura(QString::default());
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let Some(salida) = miniatura::hacer(&ruta) else {
                return;
            };
            let _ = hilo.queue(move |mut obj| {
                // Solo si sigue siendo la última (pudo tomarse otra mientras).
                if obj.rust().estado.ultima() != Some(ruta.as_path()) {
                    return;
                }
                let marca = std::fs::metadata(&salida)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                // El «?» hace que QML no use la imagen vieja de su caché (mismo archivo cada vez).
                let url = format!("{}?{marca}", url_de(&salida));
                obj.as_mut().set_miniatura(QString::from(url.as_str()));
            });
        });
    }

    fn abrir_ultima(&self) {
        if let Some(r) = self.rust().estado.ultima() {
            abrir(r);
        }
    }

    fn abrir_carpeta(&self, modo: &QString) {
        if let Some(c) = self.rust().estado.carpeta_de(&modo.to_string()) {
            abrir(c);
        }
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().fijar_ultima();
        // Una sola ventana: si otra copia pide «mostrar», se avisa a QML (Main.qml ya escucha mostrarVentana).
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                for _ in 0..escucha.revisar() {
                    if hilo.queue(|obj| obj.mostrar_ventana()).is_err() {
                        return;
                    }
                }
            }
        });
    }
}
