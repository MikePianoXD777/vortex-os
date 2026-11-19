//! La prueba de `apps/vortex-camera/pruebas/test_interfaz.py`, contra la ventana de Rust: sin pantalla
//! (offscreen) y con la cámara falsa (RTSP de verdad, con ffmpeg). Carga los QML sin errores, el mosaico
//! recibe video, la cámara con el código malo dice «código», la vista grande y las grabaciones reciben
//! video, y el código de seguridad no sale en la terminal. Todo va a carpetas temporales (también el socket
//! de la ventana única): no toca tus cámaras ni le habla a una Camera tuya que esté abierta.

// El build.rs de cxx-qt enlaza sus objetos en todo lo del paquete (también esta prueba): sin estas dos
// líneas faltan sus símbolos de arranque al enlazar.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CODIGO: &str = "CLAVE7Z9";

fn raiz_repo() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.."))
}

#[test]
fn ventana() {
    let guion = format!(
        "import sys; sys.path.insert(0, {:?}); from camara_falsa import CamaraFalsa\n\
         c = CamaraFalsa(0, 'admin', {CODIGO:?})\nprint(c.puerto, flush=True); sys.stdin.read(); c.cerrar()",
        raiz_repo().join("apps/vortex-camera/pruebas").to_string_lossy()
    );
    let mut falsa = Command::new("python3")
        .args(["-c", &guion])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut linea = String::new();
    BufReader::new(falsa.stdout.take().unwrap())
        .read_line(&mut linea)
        .unwrap();
    let puerto: u16 = linea.trim().parse().expect("la cámara falsa no arrancó");

    let d = tempfile::tempdir().unwrap();
    let mut ventana = Command::new(env!("CARGO_BIN_EXE_vortex-camera-rs"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("VORTEX_CAMERA_PRUEBA", "1")
        .env("VORTEX_CAMERA_SIN_REVISAR", "1")
        .env("VORTEX_CAMERA_PRUEBA_PUERTO", puerto.to_string())
        .env("VORTEX_CAMERA_PRUEBA_CODIGO", CODIGO)
        .env("VORTEX_CAMERA_CONFIG", d.path())
        .env("VORTEX_CAMERA_COLORES", d.path().join("no-hay.json"))
        .env("VORTEX_CAMERA_DIR", raiz_repo().join("apps/vortex-camera"))
        .env("TMPDIR", d.path())
        .env("XDG_STATE_HOME", d.path().join("estado"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Límite de 150 s, como la de Python: si se cuelga, se corta y falla.
    let limite = Instant::now() + Duration::from_secs(150);
    while matches!(ventana.try_wait(), Ok(None)) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = ventana.kill();
    let salida = ventana.wait_with_output().unwrap();
    drop(falsa.stdin.take());
    let _ = falsa.wait();

    let out = String::from_utf8_lossy(&salida.stdout);
    let err = String::from_utf8_lossy(&salida.stderr);
    let Some(datos) = out.lines().find_map(|l| l.strip_prefix("DATOS")) else {
        panic!("sin DATOS\n{out}\n{}", &err[err.len().saturating_sub(3000)..]);
    };
    let d: Value = serde_json::from_str(datos).unwrap();
    eprintln!("pico de memoria (VmHWM): {}", d["vmhwm"]);
    assert_eq!(d["errores"], Value::Array(vec![]), "{d}");
    assert_eq!(d["vacio"], "Ponle un nombre a la cámara.");
    assert!(d["sin_codigo"].as_str().unwrap().contains("código de seguridad"));
    assert!(d["ip_mala"].as_str().unwrap().contains("IP"));
    assert_eq!(d["agregar"], "");
    assert_eq!(d["codigo_en_qml"], false);
    assert_eq!(d["mosaico_video"], true, "el mosaico no recibió video");
    assert_eq!(d["estados"], true, "{}", d["estados_valor"]);
    assert_eq!(d["calle_sin_video"], true);
    assert_eq!(d["grande_url_principal"], true);
    assert_eq!(d["grande_video"], true, "la vista grande no recibió video");
    assert_eq!(d["mosaico_suelta"], true);
    assert_eq!(d["nueve"], true);
    assert_eq!(d["ventanas"], serde_json::json!(["Ventana 1", "Calle"]));
    assert_eq!(d["grabado_url"], true);
    assert_eq!(d["grabado_video"], true, "las grabaciones no recibieron video");
    // El código no se escribe en la terminal (ni el bueno ni el malo).
    assert!(!err.contains(CODIGO), "{err}");
    assert!(!err.contains("admin:MALO"), "{err}");
}
