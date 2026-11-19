//! Lo de `apps/vortex-player/pruebas/test_interfaz.py`, contra la ventana de Rust: sin pantalla (offscreen),
//! sin sonido (mpv con --ao=null) y con tres canciones de prueba hechas con ffmpeg. Carga los QML sin
//! errores, lee la biblioteca, toca, salta, pausa, repite y arma una playlist. Todo va a carpetas temporales
//! (también el ecualizador, con un equalizer.sh falso): no toca tu música, tu ecualizador ni tu panel.

// El build.rs de cxx-qt enlaza sus objetos en todo lo del paquete (también esta prueba): sin estas dos
// líneas faltan sus símbolos de arranque al enlazar.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn raiz_repo() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.."))
}

fn hay(p: &str) -> bool {
    Command::new("sh").args(["-c", &format!("command -v {p}")]).stdout(Stdio::null()).status().is_ok_and(|s| s.success())
}

#[test]
fn ventana() {
    if !hay("ffmpeg") || !hay("mpv") {
        eprintln!("faltan ffmpeg o mpv: se salta");
        return;
    }
    // Corto (en /tmp): el socket de mpv tiene límite de largo.
    let t = tempfile::Builder::new().prefix("vpr-").tempdir_in("/tmp").unwrap();
    let d = t.path();
    let disco = d.join("Música").join("disco");
    std::fs::create_dir_all(&disco).unwrap();
    for (nombre, titulo, opciones) in [
        ("a.flac", "Uno", vec!["-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"]),
        ("b.wav", "Dos", vec!["-c:a", "pcm_s16le"]),
        ("c.opus", "Tres", vec!["-c:a", "libopus"]),
    ] {
        let ok = Command::new("ffmpeg")
            .args(["-v", "quiet", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=30"])
            .args(&opciones)
            .args(["-metadata", &format!("title={titulo}")])
            .arg(disco.join(nombre))
            .status()
            .unwrap();
        assert!(ok.success());
    }
    // Un equalizer.sh falso: el de verdad cambia el ecualizador y el EasyEffects de quien corre las pruebas.
    let eq = d.join("sin-shell/quickshell/media/equalizer.sh");
    std::fs::create_dir_all(eq.parent().unwrap()).unwrap();
    std::fs::write(&eq, format!("#!/bin/sh\necho \"$*\" >>\"{}/eq.log\"\n", d.display())).unwrap();
    std::fs::set_permissions(&eq, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut ventana = Command::new(env!("CARGO_BIN_EXE_vortex-player-rs"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("VORTEX_PLAYER_PRUEBA", "1")
        .env("VORTEX_PLAYER_AO", "null")
        .env("VORTEX_PLAYER_SIN_RED", "1")
        .env("VORTEX_PLAYER_MEDIDA", "0")
        .env("VORTEX_PLAYER_MUSICA", d.join("Música"))
        .env("VORTEX_PLAYER_DATOS", d.join("datos"))
        .env("VORTEX_PLAYER_CONFIG", d.join("config"))
        .env("VORTEX_PLAYER_CACHE", d.join("cache"))
        .env("VORTEX_PLAYER_EQ", d.join("eq.json"))
        .env("VORTEX_PLAYER_COLORES", d.join("no-hay.json"))
        .env("VORTEX_PLAYER_DIR", raiz_repo().join("apps/vortex-player"))
        .env("SERPANTINUM_DIR", d.join("sin-shell"))
        .env("XDG_CONFIG_HOME", d.join("xdg"))
        .env("XDG_RUNTIME_DIR", d)
        .env("XDG_STATE_HOME", d.join("estado"))
        .env("TMPDIR", d)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let limite = Instant::now() + Duration::from_secs(90);
    while matches!(ventana.try_wait(), Ok(None)) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = ventana.kill();
    let salida = ventana.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&salida.stdout);
    let err = String::from_utf8_lossy(&salida.stderr);
    let Some(datos) = out.lines().find_map(|l| l.strip_prefix("DATOS")) else {
        panic!("sin DATOS\n{out}\n{}", &err[err.len().saturating_sub(3000)..]);
    };
    let d: Value = serde_json::from_str(datos).unwrap();
    eprintln!("pico de memoria (VmHWM): {}", d["vmhwm"]);
    assert_eq!(d["errores"], json!([]), "{d}");
    assert_eq!(d["escaneo"], true, "{d}");
    assert_eq!(d["totales"]["cuantas"], 3);
    assert_eq!(d["totales"]["hires"], 1);
    assert!(d["totales"]["duracion"].as_f64().unwrap() > 0.0);
    let mut canciones: Vec<String> = serde_json::from_value(d["canciones"].clone()).unwrap();
    canciones.sort();
    assert_eq!(canciones, ["Dos", "Tres", "Uno"]);
    assert!(d["etiquetas"].as_array().unwrap().contains(&json!("Hi-Res 24/96")), "{d}");
    assert_eq!(d["me_gusta"], json!([false, true, true]));
    assert_eq!(d["por_calidad"], json!(["Uno", "Dos", "Tres"]));
    assert_eq!(d["sonando"], "Uno");
    assert_eq!(d["avanza"], true, "mpv no avanzó");
    assert_eq!(d["cola"], json!([2, 2]));
    assert_eq!(d["siguiente"], "Dos");
    assert_eq!(d["pausado"], true);
    assert_eq!(d["repetir"], "una");
    assert_eq!(d["playlist"], json!(["Tres"]));
    // «Tus me gusta» la creó el ❤️ de arriba
    assert_eq!(d["playlists"], json!(["Tus me gusta", "Para correr"]));
    assert_eq!(d["para_agregar"], 3);
    assert!(d["perfiles"].as_u64().unwrap() > 0);
    assert_eq!(d["ajustes_navegadores"], 9);
}
