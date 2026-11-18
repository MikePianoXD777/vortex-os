//! `pruebas/test_interfaz.py`, portada: la ventana offscreen, con un umu-run y Protons falsos y una casa
//! temporal. No corre nada de Windows, no baja nada y no toca tu biblioteca.
//!
//! Cubre: los QML cargan sin errores ni advertencias (con la biblioteca vacía, con programas y sin el
//! motor), la búsqueda, «Configurar», «Quitar», la pregunta de un .exe abierto desde fuera y los avisos.
//! (Jugar, detener, instalar y quitar de verdad se prueban en el motor: `apps/vortex-runner/tests`.)
//! Con `VORTEX_RUNNER_CAPTURAS=/carpeta` guarda capturas de la ventana.

// El script de compilación del crate mete el C++ de cxx-qt también aquí: hay que enlazar sus crates.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use vortex_runner::Contexto;
use vortex_runner::biblioteca::Biblioteca;

struct Casa {
    t: PathBuf,
}

impl Drop for Casa {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.t);
    }
}

fn casa(nombre: &str, con_motor: bool, programas: &[&str]) -> Casa {
    let t = vortex_core::carpeta_de_prueba(nombre);
    let umu = t.join("umu-run");
    fs::write(&umu, "#!/bin/sh\nexec sleep 30\n").unwrap();
    fs::set_permissions(&umu, fs::Permissions::from_mode(0o755)).unwrap();
    for n in ["proton-cachyos-slr", "GE-Proton10-3"] {
        fs::create_dir_all(t.join("protones").join(n)).unwrap();
        fs::write(t.join("protones").join(n).join("proton"), "#!/bin/sh\n").unwrap();
    }
    let juegos = t.join("Mis juegos");
    fs::create_dir_all(&juegos).unwrap();
    for n in ["super_juego-x64.exe", "Otro Programa.exe", "setup_cosa.exe"] {
        fs::write(juegos.join(n), [b"MZ".as_slice(), &[0u8; 200]].concat()).unwrap();
    }
    let ctx = Contexto::de_prueba(&t, Some(if con_motor { umu } else { t.join("no-hay-umu") }));
    let bib = Biblioteca::new(ctx.rutas.clone());
    for p in programas {
        bib.agregar(&juegos.join(p), None, None).unwrap();
    }
    Casa { t }
}

/// Corre la ventana en modo prueba y devuelve lo que midió (el JSON de «DATOS»).
fn correr(c: &Casa, con_motor: bool) -> serde_json::Value {
    let t = &c.t;
    let capturas = std::env::var("VORTEX_RUNNER_CAPTURAS").unwrap_or_default();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vortex-runner-rs"));
    cmd.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", t)
        .env("LANG", "C.UTF-8")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("VORTEX_RUNNER_PRUEBA", "1")
        .env(
            "VORTEX_RUNNER_PRUEBA_ARCHIVO",
            t.join("Mis juegos/setup_cosa.exe"),
        )
        .env("VORTEX_RUNNER_HOME", t)
        .env("VORTEX_RUNNER_PROTONES", t.join("protones"))
        .env("VORTEX_RUNNER_COLORES", t.join("no-hay.json"))
        .env(
            "VORTEX_RUNNER_UMU",
            if con_motor {
                t.join("umu-run")
            } else {
                t.join("no-hay-umu")
            },
        )
        .env("XDG_RUNTIME_DIR", t)
        .env("XDG_CONFIG_HOME", t.join("config"))
        .env("XDG_STATE_HOME", t.join("estado"))
        .env("XDG_CACHE_HOME", t.join("cache"))
        .env("TMPDIR", t);
    if !capturas.is_empty() {
        cmd.env("VORTEX_RUNNER_CAPTURAS", &capturas);
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
fn biblioteca() {
    let c = casa(
        "runner-ui-biblioteca",
        true,
        &["super_juego-x64.exe", "Otro Programa.exe"],
    );
    let d = correr(&c, true);
    assert_eq!(d["programas"], 2);
    assert_eq!(d["falta"], serde_json::json!([]));
    assert_eq!(d["celdas"], 2);
    assert_eq!(d["buscada"], 1);
    assert_eq!(d["config_abierto"], true);
    assert_eq!(d["quitar_abierto"], true);
    assert_eq!(d["archivo_abierto"], true);
    assert_eq!(d["archivo_instalador"], true);
    assert_eq!(d["archivo_nombre"], "Setup Cosa");
    assert_eq!(d["aviso"], "Listo: lo agregué a tu biblioteca.");
    // abrir la ventana no toca la biblioteca ni abre nada
    assert_eq!(
        Biblioteca::new(Contexto::de_prueba(&c.t, None).rutas)
            .cargar()
            .len(),
        2
    );
}

#[test]
fn biblioteca_vacia() {
    let c = casa("runner-ui-vacia", true, &[]);
    let d = correr(&c, true);
    assert_eq!(d["programas"], 0);
    assert_eq!(d["celdas"], 0);
    assert_eq!(d["config_abierto"], false);
}

#[test]
fn sin_motor() {
    // Se puede ver la biblioteca sin el motor (el banner pide instalarlo); lo que no se puede es jugar.
    let c = casa("runner-ui-sin-motor", false, &["super_juego-x64.exe"]);
    let d = correr(&c, false);
    assert_eq!(d["falta"], serde_json::json!(["umu-launcher"]));
    assert_eq!(d["celdas"], 1);
    assert!(Path::new(&c.t).join("datos/biblioteca.json").is_file());
}
