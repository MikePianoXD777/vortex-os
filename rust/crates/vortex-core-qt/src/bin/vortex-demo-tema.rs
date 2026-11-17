//! Demo del paso 0: una ventana con `Tema.qml` y la paleta real (o la de respaldo).
//!
//! Sin pantalla (pruebas): `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
//! VORTEX_DEMO_CAPTURA=/tmp/demo.png vortex-demo-tema` guarda una captura y se cierra.

extern crate vortex_core_qt; // enlaza su C++ (el módulo QML)
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    // Sin esto el enlazador tira el registro del módulo QML (cxx-qt 0.10).
    cxx_qt::init_crate!(vortex_core_qt);
    let carpeta_qml = concat!(env!("CARGO_MANIFEST_DIR"), "/qml");
    let mut app = QGuiApplication::new();
    let mut motor = QQmlApplicationEngine::new();
    if let Some(mut motor) = motor.as_mut() {
        motor.as_mut().add_import_path(&QString::from(carpeta_qml));
        motor.load(&QUrl::from(format!("{carpeta_qml}/Demo.qml").as_str()));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
