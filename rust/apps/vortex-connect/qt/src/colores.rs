//! `colores`: la paleta del fondo en vivo, igual que `colores.py` (`Tema.qml` lee `colores.paleta`).
//!
//! La paleta sale de `vortex_core::paleta` (probada idéntica a la de Python). Como en la de Python,
//! `VORTEX_CONNECT_COLORES` cambia el archivo que se lee (pruebas). Cuando `vortex-core-qt` compile, esto
//! puede pasar a usar su `Colores`; hoy vive aquí para que `Tema.qml` lo encuentre como «colores».
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QMap_QString_QVariant, paleta)]
        type Colores = super::ColoresRust;
    }

    impl cxx_qt::Initialize for Colores {}
    impl cxx_qt::Threading for Colores {}
}

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QString, QVariant};
use std::path::PathBuf;
use std::time::Duration;
use vortex_core::paleta::{Paleta, Vigia};

#[derive(Default)]
pub struct ColoresRust {
    paleta: QMap<QMapPair_QString_QVariant>,
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
        let mut vigia = match std::env::var_os("VORTEX_CONNECT_COLORES").filter(|v| !v.is_empty()) {
            Some(r) => Vigia::new(PathBuf::from(r)),
            None => Vigia::de_matugen(),
        };
        self.as_mut().set_paleta(a_qmap(vigia.actual()));
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(300));
                if let Some(nueva) = vigia.revisar() {
                    let mapa = a_qmap(nueva);
                    if hilo.queue(move |obj| obj.set_paleta(mapa)).is_err() {
                        break; // la ventana ya se cerró
                    }
                }
            }
        });
    }
}
