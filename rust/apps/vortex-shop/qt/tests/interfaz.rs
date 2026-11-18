//! Las pruebas de la ventana (`ui_capturas.py`, `ui_android.py` y `ui_integracion.py`, portadas): la ventana
//! offscreen con un Flathub FALSO en 127.0.0.1, un «flatpak», un «vortex» y un «vortex-android» falsos y una
//! casa temporal. No toca internet, ni tu flatpak, ni tu sistema, ni tu pantalla.
//!
//! Cubre: los QML cargan sin errores ni advertencias, y se recorren Inicio, una categoría, la búsqueda, el
//! detalle, Instaladas, Actualizaciones, «Quitar» y «Juegos de Android» (sin preparar, preparando y lista).
//! Con `VORTEX_SHOP_CAPTURAS=/carpeta` guarda capturas de la ventana.

// El script de compilación del crate mete el C++ de cxx-qt también aquí: hay que enlazar sus crates.
extern crate cxx_qt;
extern crate cxx_qt_lib;

#[path = "../../tests/comun/mod.rs"]
mod comun;

use comun::{BaseFlatpak, Servidor, entorno_android_falso, entorno_vortex_falso, tmp};
use std::process::Command;

/// Corre la ventana en modo prueba y devuelve lo que midió (el JSON de «DATOS»).
fn correr(instaladas: &[&str], actualizaciones: &[&str], con_red: bool) -> serde_json::Value {
    let base = BaseFlatpak::nueva();
    base.escribir("usuario.txt", instaladas);
    base.escribir("actualizaciones.txt", actualizaciones);
    base.escribir("remotos.txt", &["flathub"]);
    let srv = Servidor::nuevo();
    if !con_red {
        srv.parar();
    }
    let t = tmp();
    let vortex = entorno_vortex_falso(t.path());
    let android = entorno_android_falso(t.path(), true);
    let entorno_base = base.entorno(None);
    let capturas = std::env::var("VORTEX_SHOP_CAPTURAS").unwrap_or_default();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vortex-shop-rs"));
    cmd.env_clear()
        .env("PATH", entorno_base.valor("PATH").unwrap())
        .env("HOME", t.path())
        .env("LANG", "C.UTF-8")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("VORTEX_SHOP_PRUEBA", "1")
        .env("VORTEX_SHOP_PAUSA", "900")
        .env("VORTEX_SHOP_API", &srv.url)
        .env("VORTEX_SHOP_COLORES", t.path().join("no-hay.json"))
        .env("FAKE_FLATPAK_DIR", &base.estado)
        .env("VORTEX_SHOP_VORTEX", vortex.valor("VORTEX_SHOP_VORTEX").unwrap())
        .env(
            "VORTEX_SHOP_ANDROID",
            android.valor("VORTEX_SHOP_ANDROID").unwrap(),
        )
        .env("ANDROID_FALSO_DIR", android.valor("ANDROID_FALSO_DIR").unwrap())
        .env("ANDROID_FALSO_NVIDIA", "true")
        .env("XDG_RUNTIME_DIR", t.path())
        .env("XDG_CONFIG_HOME", t.path().join("config"))
        .env("XDG_STATE_HOME", t.path().join("estado-xdg"))
        .env("XDG_DATA_HOME", t.path().join("datos"))
        .env("XDG_CACHE_HOME", t.path().join("cache"))
        .env("TMPDIR", t.path());
    if !capturas.is_empty() {
        cmd.env("VORTEX_SHOP_CAPTURAS", &capturas);
    }
    let r = cmd.output().unwrap();
    let salida = String::from_utf8_lossy(&r.stdout);
    let error = String::from_utf8_lossy(&r.stderr);
    let linea = salida
        .lines()
        .find_map(|l| l.strip_prefix("DATOS"))
        .unwrap_or_else(|| panic!("{salida}\n{error}"));
    let ruidos: Vec<&str> = error
        .lines()
        .filter(|l| {
            !l.trim().is_empty() && !l.contains("propagateSizeHints") && !l.contains("QStandardPaths")
        })
        .collect();
    assert!(ruidos.is_empty(), "La QML o Qt se quejaron: {ruidos:?}");
    let datos: serde_json::Value = serde_json::from_str(linea).unwrap();
    assert_eq!(datos["errores"], serde_json::json!([]));
    datos
}

#[test]
fn recorrido_con_flathub_falso() {
    let d = correr(
        &["org.mozilla.firefox", "org.videolan.VLC"],
        &["org.mozilla.firefox"],
        true,
    );
    assert_eq!(d["inicio_estado"], "listo");
    assert!(d["recomendadas"].as_i64().unwrap() > 0, "{d}");
    assert!(d["categorias"].as_i64().unwrap() > 0, "{d}");
    assert_eq!(d["instaladas"], 2);
    assert_eq!(d["categoria_estado"], "listo");
    assert!(d["categoria_apps"].as_i64().unwrap() > 0, "{d}");
    assert_eq!(d["detalle_estado"], "listo");
    assert_eq!(d["actualizaciones"], 1);
    // Android: sin preparar, preparando y lista (con Subway Surfers)
    assert_eq!(d["android_cargado"], true);
    assert_eq!(d["android_fase"], "preparando");
    assert_eq!(d["android_preparado"], true);
    assert_eq!(d["android_apps"], 1);
}

#[test]
fn sin_red_la_ventana_no_se_rompe() {
    let d = correr(&["org.mozilla.firefox"], &[], false);
    assert_eq!(d["inicio_estado"], "sinred");
    assert_eq!(d["instaladas"], 1);
    assert_eq!(d["errores"], serde_json::json!([]));
}
