//! `pruebas/test_interfaz.py`, portada: la ventana offscreen, en una casa temporal, en los dos idiomas.
//!
//! Cubre: los QML cargan sin errores ni advertencias, cada archivo (zip, AppImage, mod, script) llega a
//! «listo» con sus botones, descomprimir termina en «hecho», «Instalado» lista lo hecho y se puede ver esa
//! pestaña. No instala nada, no abre programas y no toca tu carpeta personal.
//! Con `VORTEX_APP_INSTALLER_CAPTURAS=/carpeta` se usa `--capturas` aparte (ver el README).

// El script de compilación del crate mete el C++ de cxx-qt también aquí: hay que enlazar sus crates.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Muestras {
    tmp: tempfile::TempDir,
    archivos: Vec<PathBuf>,
}

fn bsdtar_zip(destino: &Path, carpeta: &Path, nombres: &[&str]) {
    let r = Command::new("bsdtar")
        .args(["--format", "zip", "-cf"])
        .arg(destino)
        .arg("-C")
        .arg(carpeta)
        .args(nombres)
        .status()
        .unwrap();
    assert!(r.success());
}

fn muestras() -> Muestras {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let descargas = t.join("Descargas");
    fs::create_dir_all(&descargas).unwrap();
    // un zip (el primero: es el que se descomprime)
    let fuente = t.join("fuente");
    fs::create_dir_all(fuente.join("a")).unwrap();
    fs::write(fuente.join("a/b.txt"), "b").unwrap();
    let zip = descargas.join("a.zip");
    bsdtar_zip(&zip, &fuente, &["a"]);
    // un AppImage de mentira (se detecta por la extensión; aquí nunca se instala)
    let appimage = descargas.join("Falso.AppImage");
    fs::write(&appimage, "#!/bin/sh\necho hola\n").unwrap();
    // un mod de Fabric
    let jar_dir = t.join("jar");
    fs::create_dir_all(&jar_dir).unwrap();
    fs::write(
        jar_dir.join("fabric.mod.json"),
        r#"{"schemaVersion":1,"id":"sodium","name":"Sodium","version":"0.6.0","depends":{"minecraft":"1.21.1"}}"#,
    )
    .unwrap();
    let jar = descargas.join("sodium.jar");
    bsdtar_zip(&jar, &jar_dir, &["fabric.mod.json"]);
    // un script
    let sh = descargas.join("x.sh");
    fs::write(&sh, "#!/bin/sh\necho hola\n").unwrap();
    Muestras {
        tmp,
        archivos: vec![zip, appimage, jar, sh],
    }
}

/// Corre la ventana en modo prueba y devuelve lo que midió (el JSON de «DATOS»).
fn correr(m: &Muestras, idioma: &str) -> serde_json::Value {
    let t = m.tmp.path();
    let lista: Vec<String> = m
        .archivos
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let r = Command::new(env!("CARGO_BIN_EXE_vortex-app-installer-rs"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", t.join("casa"))
        .env("LANG", "C.UTF-8")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("VORTEX_APP_INSTALLER_PRUEBA", "1")
        .env("VORTEX_APP_INSTALLER_PRUEBA_ARCHIVOS", lista.join("\n"))
        .env("VORTEX_APP_INSTALLER_IDIOMA", idioma)
        .env("VORTEX_APP_INSTALLER_COLORES", t.join("no-hay.json"))
        .env("XDG_RUNTIME_DIR", t)
        .env("XDG_CONFIG_HOME", t.join("config"))
        .env("XDG_STATE_HOME", t.join("estado"))
        .env("XDG_CACHE_HOME", t.join("cache"))
        .env("TMPDIR", t)
        .output()
        .unwrap();
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

fn comprobar(idioma: &str) {
    let m = muestras();
    // HOME vacío y propio: la casa de la prueba
    fs::create_dir_all(m.tmp.path().join("casa")).unwrap();
    let d = correr(&m, idioma);
    let abiertos = d["abiertos"].as_array().unwrap();
    assert_eq!(abiertos.len(), 4);
    for a in abiertos {
        assert_eq!(a["estado"], "listo", "{a}");
        assert!(a["acciones"].as_i64().unwrap() > 0, "{a}");
    }
    assert_eq!(d["estado_final"], "hecho");
    assert_eq!(d["instalados"], 1);
    // lo descomprimido quedó junto al archivo y no en otro lado
    assert!(m.tmp.path().join("Descargas/a/b.txt").exists());
}

#[test]
fn ventana_sin_avisos_es() {
    comprobar("es");
}

#[test]
fn ventana_sin_avisos_en() {
    comprobar("en");
}
