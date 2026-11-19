//! Las pruebas de `pruebas/panel.py` y `pruebas/pagina.py`, contra la ventana de Rust: sin pantalla
//! (offscreen) y con un asistente de mentira (la interfaz anota lo que le pediría, y las señales se simulan).
//! HOME, XDG y las carpetas de Miku son temporales: no tocan nada de la persona ni a una Miku suya abierta.

// El build.rs de cxx-qt enlaza sus objetos en todo lo del paquete (también esta prueba): sin estas dos
// líneas faltan sus símbolos de arranque al enlazar.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn raiz_repo() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.."))
}

fn ventana(d: &Path, extra: &[(&str, &str)], limite: u64) -> Value {
    std::fs::create_dir_all(d.join("config")).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_miku-rs"));
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", d)
        .env("XDG_CONFIG_HOME", d.join("config"))
        .env("XDG_RUNTIME_DIR", d.join("run"))
        .env("XDG_STATE_HOME", d.join("estado"))
        .env("XDG_DATA_HOME", d.join("datos"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("VORTEX_MIKU_PRUEBA", "1")
        .env("VORTEX_MIKU_COLORES", d.join("no-hay.json"))
        .env("VORTEX_MIKU_RAIZ", raiz_repo().join("apps/miku"))
        .env("VORTEX_MIKU_CAPTURA_DIR", d)
        .env("TMPDIR", d)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    std::fs::create_dir_all(d.join("run")).unwrap();
    for (k, v) in extra {
        c.env(k, v);
    }
    let mut h = c.spawn().unwrap();
    let fin = Instant::now() + Duration::from_secs(limite);
    while matches!(h.try_wait(), Ok(None)) && Instant::now() < fin {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = h.kill();
    let salida = h.wait_with_output().unwrap();
    let (out, err) = (
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr),
    );
    let Some(datos) = out.lines().find_map(|l| l.strip_prefix("DATOS")) else {
        panic!("sin DATOS\n{out}\n{}", &err[err.len().saturating_sub(3000)..]);
    };
    serde_json::from_str(datos).unwrap()
}

#[test]
fn el_panel_de_voz() {
    let d = tempfile::tempdir().unwrap();
    let r = ventana(d.path(), &[], 120);
    eprintln!(
        "memoria: VmHWM {} VmRSS {} RssAnon {}",
        r["vmhwm"], r["vmrss"], r["rssanon"]
    );
    assert_eq!(r["errores"], Value::Array(vec![]), "{r}");
    assert_eq!(r["app_id"], "vortex-miku", "lo que casan las reglas de Hyprland");
    assert_eq!(r["titulo"], "Asistente de Vortex");
    assert_eq!((r["ancho"].as_i64(), r["alto"].as_i64()), (Some(420), Some(240)));
    assert_eq!(
        r["fondo"], "#0b0d1a",
        "paleta de respaldo de Vortexine's sin paleta del fondo"
    );
    assert_eq!(
        r["web_engine_proceso"], false,
        "sin QtWebEngineProcess (no hay 3D): el panel no lo arranca"
    );

    assert_eq!(r["progreso_visible"], true);
    assert!(r["progreso_titulo"].as_str().unwrap().contains("~560 MB"));
    assert!((r["progreso_fraccion"].as_f64().unwrap() - 0.15).abs() < 1e-6);
    assert_eq!(r["progreso_oculto"], true, "al terminar la descarga se esconde");

    assert_eq!(
        (r["boton_texto"].as_str(), r["boton_visible"].as_bool()),
        (Some("Activar Claude"), Some(true))
    );
    assert_eq!(r["boton2_texto"], "Iniciar sesión");
    assert_eq!(r["sin_botones"], true);

    assert_eq!(r["escuchando_texto"], "Escuchando...");
    assert_eq!(
        (r["mic_activo"].as_bool(), r["punto_activo"].as_bool()),
        (Some(true), Some(true))
    );
    assert_eq!(r["pensando_puntos"], "...");
    assert!(
        r["respuesta"].as_str().unwrap().contains("Tokio") && r["tu"].as_str().unwrap().starts_with("Tú:")
    );
    assert!(r["aviso"].as_str().unwrap().contains("internet"));
    assert_eq!(
        (r["camara_visible"].as_bool(), r["camara_oculta"].as_bool()),
        (Some(true), Some(true))
    );
    assert_eq!(r["entrada_vacia"], true);
    let reg = r["registro"].as_str().unwrap();
    for esperado in [
        "accion:activar",
        "enviar:abre la terminal",
        "tocada",
        "escuchar",
        "cortar",
        "cerrar",
    ] {
        assert!(
            reg.lines().any(|l| l == esperado),
            "falta «{esperado}» en el registro:\n{reg}"
        );
    }
    assert!(d.path().join("panel-respuesta.png").is_file());
}

#[test]
fn con_ui_scale_085_el_panel_mide_357x204() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("config/serpantinum")).unwrap();
    std::fs::write(
        d.path().join("config/serpantinum/settings.json"),
        r#"{"general": {"uiScale": 0.85}}"#,
    )
    .unwrap();
    let r = ventana(d.path(), &[], 120);
    assert_eq!(
        (r["ancho"].as_i64(), r["alto"].as_i64()),
        (Some(357), Some(204)),
        "{r}"
    );
}

#[test]
fn la_paleta_sigue_al_archivo_del_fondo() {
    let d = tempfile::tempdir().unwrap();
    let paleta = d.path().join("matugen.json");
    // El archivo existe pero no es una paleta de matugen: se usa la de respaldo.
    std::fs::write(&paleta, "{}").unwrap();
    let r = ventana(
        d.path(),
        &[("VORTEX_MIKU_COLORES", paleta.to_str().unwrap())],
        120,
    );
    assert_eq!(r["fondo"], "#0b0d1a");
}

/// El avatar: la página de verdad (three.js) en QtWebEngine, sin el modelo VRM (la página avisa sola). Necesita las
/// bibliotecas 3D (`web/vendor`, que no van en git: las baja el paquete): `VORTEX_MIKU_VENDOR` apunta a donde
/// estén; si no hay, no se prueba.
#[test]
fn el_avatar_en_qtwebengine() {
    let vendor = std::env::var_os("VORTEX_MIKU_VENDOR")
        .map(PathBuf::from)
        .unwrap_or_else(|| raiz_repo().join("apps/miku/web/vendor"));
    if !vendor.join("three.module.min.js").is_file() {
        eprintln!("sin web/vendor (VORTEX_MIKU_VENDOR): no se prueba el avatar");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("config/vortex")).unwrap();
    std::fs::write(d.path().join("config/vortex/miku.json"), r#"{"avatar": true}"#).unwrap();
    let r = ventana(
        d.path(),
        &[
            ("QTWEBENGINE_CHROMIUM_FLAGS", "--no-sandbox --disable-gpu"),
            ("VORTEX_MIKU_VENDOR", vendor.to_str().unwrap()),
            ("VORTEX_MIKU_WEB_PRUEBA", "1"),
        ],
        240,
    );
    eprintln!(
        "memoria del avatar: VmHWM {} VmRSS {} RssAnon {}",
        r["vmhwm"], r["vmrss"], r["rssanon"]
    );
    assert_eq!(r["errores"], Value::Array(vec![]), "{r}");
    assert_ne!(r["sin_ventana"], true, "no abrió la ventana del avatar: {r}");
    assert_eq!(r["app_id"], "vortex-miku");
    assert_eq!(r["titulo"], "Miku");
    assert_eq!((r["ancho"].as_i64(), r["alto"].as_i64()), (Some(460), Some(760)));
    assert_eq!(r["pagina_lista"], true, "la página conectó el canal: {r}");
    assert_eq!(
        r["puente_ok"], true,
        "la página le habló al asistente por el puente: {r}"
    );
}
