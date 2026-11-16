//! El singleton `Colores` para QML: la paleta real del fondo, en vivo.
//!
//! Al crearse lee la paleta (`vortex_core::paleta::Vigia`) y lanza un hilo que la revisa cada 300 ms;
//! cuando cambia, la manda al hilo de Qt (`qt_thread().queue`) y QML se entera por `paletaChanged`.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QMap_QString_QVariant, paleta)]
        #[qproperty(QString, captura)]
        type Colores = super::ColoresRust;
    }

    impl cxx_qt::Initialize for Colores {}
    impl cxx_qt::Threading for Colores {}
}

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QString, QVariant};
use std::time::Duration;
use vortex_core::paleta::{Paleta, Vigia};

#[derive(Default)]
pub struct ColoresRust {
    paleta: QMap<QMapPair_QString_QVariant>,
    /// VORTEX_DEMO_CAPTURA: la demo guarda ahí una captura y se cierra (pruebas sin pantalla).
    captura: QString,
}

fn a_qmap(p: &Paleta) -> QMap<QMapPair_QString_QVariant> {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in p {
        m.insert(
            QString::from(k.as_str()),
            QVariant::from(&QString::from(v.as_str())),
        );
    }
    m
}

impl cxx_qt::Initialize for qobject::Colores {
    fn initialize(mut self: Pin<&mut Self>) {
        let mut vigia = Vigia::de_matugen();
        self.as_mut().set_paleta(a_qmap(vigia.actual()));
        if let Ok(c) = std::env::var("VORTEX_DEMO_CAPTURA") {
            self.as_mut().set_captura(QString::from(c.as_str()));
        }
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(300));
                if let Some(nueva) = vigia.revisar() {
                    let mapa = a_qmap(nueva);
                    if hilo.queue(move |obj| obj.set_paleta(mapa)).is_err() {
                        break; // el objeto ya no existe: la app se cerró
                    }
                }
            }
        });
    }
}
