//! `pruebas/test_interfaz.py`, portada: la ventana offscreen, haciendo de cuenta que no hay cámara (nunca
//! abre la tuya). Carga sin errores de QML, dice que falta la cámara, muestra la última captura y guarda.

use std::fs;
use std::process::Command;

const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0,
    0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63,
    0x64, 0xF8, 0xCF, 0xF0, 0x1F, 0, 0x04, 0x06, 0x02, 0, 0xA4, 0x3E, 0x5F, 0x44, 0, 0, 0, 0, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[test]
fn sin_camara() {
    let t = vortex_core::carpeta_de_prueba("video-interfaz");
    let fotos = t.join("PicturesLocation").join("Cámara");
    fs::create_dir_all(&fotos).unwrap();
    fs::write(fotos.join("Foto vieja.png"), PNG).unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_vortex-video-rs"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env_remove("WAYLAND_DISPLAY")
        .env("VORTEX_VIDEO_SIN_CAMARA", "1")
        .env("VORTEX_VIDEO_PRUEBA", "1")
        .env("VORTEX_VIDEO_CARPETA", &t)
        .env("XDG_CONFIG_HOME", &t)
        .env("XDG_CACHE_HOME", &t)
        .env("XDG_STATE_HOME", &t)
        .env("TMPDIR", &t)
        .env("VORTEX_VIDEO_COLORES", t.join("no-hay.json"))
        .output()
        .unwrap();
    let salida = String::from_utf8_lossy(&r.stdout);
    let error = String::from_utf8_lossy(&r.stderr);
    let linea = salida
        .lines()
        .find_map(|l| l.strip_prefix("DATOS"))
        .unwrap_or_else(|| panic!("{error}"));
    let datos: serde_json::Value = serde_json::from_str(linea).unwrap();
    assert_eq!(datos["mensaje"], "No encuentro tu cámara");
    assert_eq!(datos["disparador"], false);
    assert!(datos["ultima"].as_str().unwrap().ends_with("Foto vieja.png"));
    assert!(datos["miniatura"].as_str().unwrap().starts_with("file:"));
    assert_eq!(datos["errores"], serde_json::json!([]));
    let ruidos: Vec<&str> = error
        .lines()
        .filter(|l| l.contains(".qml") || l.contains("TypeError") || l.contains("ReferenceError"))
        .collect();
    assert!(ruidos.is_empty(), "{ruidos:?}");
    let ajustes: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(t.join("vortex").join("video.json")).unwrap()).unwrap();
    assert_eq!(ajustes["modo"], "video");
}
