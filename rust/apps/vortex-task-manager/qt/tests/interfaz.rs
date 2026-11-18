//! `pruebas/test_interfaz.py`, portada: la ventana offscreen (nunca abre nada en pantalla). Carga los QML
//! sin errores, la lista se llena, se busca, se finaliza un «sleep» propio (lo único que toca), se abre una
//! app, se ordena y se cambia de página.

// El script de compilación del crate mete el C++ de cxx-qt también aquí: hay que enlazar sus crates.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

#[test]
fn ventana() {
    let t = vortex_core::carpeta_de_prueba("tm-interfaz");
    // Un «sleep» con nombre único: es lo único que la prueba finaliza (por su PID).
    let unico = t.join(format!("vtmprueba{}", std::process::id()));
    fs::copy("/usr/bin/sleep", &unico).expect("no hay /usr/bin/sleep");
    let mut hijo = Command::new(&unico)
        .arg("60")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let nombre = unico.file_name().unwrap().to_string_lossy().into_owned();
    let r = Command::new(env!("CARGO_BIN_EXE_vortex-task-manager-rs"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .env("VORTEX_TASK_MANAGER_PRUEBA", "1")
        .env("VORTEX_TASK_MANAGER_PRUEBA_NOMBRE", &nombre)
        .env("VORTEX_TASK_MANAGER_PRUEBA_PID", hijo.id().to_string())
        .env("XDG_CONFIG_HOME", &t)
        .env("XDG_CACHE_HOME", &t)
        .env("XDG_STATE_HOME", &t)
        .env("TMPDIR", &t) // el socket de «una sola ventana»: nunca el de la que esté abierta de verdad
        .env("VORTEX_TASK_MANAGER_COLORES", t.join("no-hay.json"))
        .output()
        .unwrap();
    let salida = String::from_utf8_lossy(&r.stdout);
    let error = String::from_utf8_lossy(&r.stderr);
    let linea = salida
        .lines()
        .find_map(|l| l.strip_prefix("DATOS"))
        .unwrap_or_else(|| {
            let _ = hijo.kill();
            panic!("{salida}\n{error}")
        });
    let d: serde_json::Value = serde_json::from_str(linea).unwrap();
    // El «sleep» propio terminó por SIGTERM (y nada más se tocó).
    let estado = hijo.try_wait().unwrap();
    if estado.is_none() {
        let _ = hijo.kill();
    }
    assert_eq!(d["errores"], serde_json::json!([]));
    assert!(
        d["secciones"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "Segundo plano"),
        "{d}"
    );
    assert!(d["grupos"].as_u64().unwrap() > 3, "{d}");
    assert_eq!(d["vramFilas"], true);
    assert_eq!(d["ordenVram"], "vram");
    assert_eq!(d["vramOrdenada"], true);
    assert_eq!(d["filtrado"], true, "{d}");
    assert_eq!(d["detalles"], true);
    assert_eq!(
        estado.and_then(|e| e.signal()),
        Some(15),
        "el sleep propio debía terminar por SIGTERM"
    );
    assert_eq!(d["abierto"], true);
    assert!(d["historia"].as_u64().unwrap() >= 4, "{d}");
    assert_eq!(d["verProcesos"], false);
    assert!(d["servicios"].as_u64().is_some());
    let ruidos: Vec<&str> = error
        .lines()
        .filter(|l| l.contains(".qml") || l.contains("TypeError") || l.contains("ReferenceError"))
        .collect();
    assert!(ruidos.is_empty(), "{ruidos:?}");
}
