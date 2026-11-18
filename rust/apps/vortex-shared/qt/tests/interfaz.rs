//! `pruebas/test_interfaz.py`, portada: la ventana offscreen, con un sendme falso: carga sin errores de QML,
//! agrega archivos, quita uno, crea un código, cancela y explica que una MAC no sirve. No toca Tailscale, tu
//! portapapeles ni tu cortafuegos. Con `VORTEX_SHARED_CAPTURAS=/carpeta` guarda capturas de la ventana.

// El script de compilación del crate mete el C++ de cxx-qt también aquí: hay que enlazar sus crates.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn guion(d: &Path, nombre: &str, cuerpo: &str) {
    let r = d.join(nombre);
    std::fs::write(&r, cuerpo).unwrap();
    std::fs::set_permissions(&r, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn ruidos(error: &str) -> Vec<&str> {
    error
        .lines()
        .filter(|l| {
            !l.trim().is_empty() && !l.contains("propagateSizeHints") && !l.contains("QStandardPaths")
        })
        .collect()
}

fn ventana(d: &Path) -> Command {
    guion(
        d,
        "sendme",
        "#!/bin/sh\necho \"sendme receive blobaaaaaaaaaaaaaaaa\"\nexec sleep 600\n",
    );
    for f in ["tailscale", "ufw", "systemctl"] {
        guion(d, f, "#!/bin/sh\nexit 1\n");
    }
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vortex-shared-rs"));
    cmd.env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", d.display()))
        .env("HOME", d)
        .env("LANG", "C.UTF-8")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("TMPDIR", d)
        .env("XDG_RUNTIME_DIR", d)
        .env("XDG_CONFIG_HOME", d.join("config"))
        .env("XDG_STATE_HOME", d.join("estado-xdg"))
        .env("XDG_CACHE_HOME", d.join("cache"))
        .env("VORTEX_SHARED_CARPETA", d.join("rec"))
        .env("VORTEX_SHARED_COLORES", d.join("no-hay.json"));
    cmd
}

#[test]
fn flujo() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path();
    let (a, b) = (d.join("a.txt"), d.join("b.txt"));
    std::fs::write(&a, "hola").unwrap();
    std::fs::write(&b, "adios").unwrap();
    let mut cmd = ventana(d);
    cmd.env(
        "VORTEX_SHARED_RECORRER",
        format!("{}:{}:/no/existe", a.display(), b.display()),
    );
    if let Ok(c) = std::env::var("VORTEX_SHARED_CAPTURAS") {
        cmd.env("VORTEX_SHARED_CAPTURAS", c);
    }
    let r = cmd.output().unwrap();
    let salida = String::from_utf8_lossy(&r.stdout);
    let error = String::from_utf8_lossy(&r.stderr);
    let linea = salida
        .lines()
        .find_map(|l| l.strip_prefix("DATOS"))
        .unwrap_or_else(|| panic!("{salida}\n{error}"));
    let r = ruidos(&error);
    assert!(r.is_empty(), "La QML o Qt se quejaron: {r:?}");
    let datos: serde_json::Value = serde_json::from_str(linea).unwrap();
    assert_eq!(datos["archivos"], serde_json::json!(["a.txt", "b.txt"]));
    assert_eq!(datos["tras_quitar"], 1);
    assert!(datos["codigo"].as_str().unwrap().starts_with("blob"), "{datos}");
    assert_eq!(datos["ocupado"], true);
    assert_eq!(datos["tras_cancelar"], serde_json::json!(["", false]));
    assert!(datos["mac"].as_str().unwrap().contains("MAC"), "{datos}");
    assert_eq!(datos["errores"], serde_json::json!([]));
    if let Some(m) = datos["rss_anon"].as_str() {
        eprintln!("RssAnon de la ventana: {m}; Rss: {}", datos["rss"]);
    }
    // el sendme del código se cortó al cancelar
    let vivo = Command::new("pgrep")
        .args(["-f", &format!("{}.*sendme", d.display())])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(!vivo, "quedó un sendme vivo");
}

#[test]
fn prueba_humo() {
    let t = tempfile::tempdir().unwrap();
    let r = ventana(t.path()).arg("--prueba-humo").output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let e = String::from_utf8_lossy(&r.stderr);
    assert!(ruidos(&e).is_empty(), "{e}");
}

#[test]
fn version_sin_cargar_qt() {
    let t = tempfile::tempdir().unwrap();
    let r = ventana(t.path()).arg("--version").output().unwrap();
    assert!(r.status.success());
    assert!(String::from_utf8_lossy(&r.stdout).contains("Vortexine Shared"));
}
