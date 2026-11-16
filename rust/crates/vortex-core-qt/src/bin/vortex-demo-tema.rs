//! Demo del paso 0: una ventana con `Tema.qml` y la paleta real (o la de respaldo).
//!
//! Sin pantalla (pruebas): `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
//! VORTEX_DEMO_CAPTURA=/tmp/demo.png vortex-demo-tema` guarda una captura y se cierra.

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let carpeta_qml = concat!(env!("CARGO_MANIFEST_DIR"), "/qml");
    let mut app = QGuiApplication::new();
    let mut motor = QQmlApplicationEngine::new();
    if let Some(motor) = motor.as_mut() {
        motor.add_import_path(&QString::from(carpeta_qml));
        motor.load(&QUrl::from(format!("{carpeta_qml}/Demo.qml").as_str()));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
