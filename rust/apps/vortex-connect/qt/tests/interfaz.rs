//! `pruebas/test_interfaz.py`, portada: la ventana offscreen, con nmcli, warp-cli y systemctl falsos y un
//! servidor propio en 127.0.0.1 en vez de internet. Carga sin errores de QML, pide las condiciones antes de
//! cifrar y recorre el flujo (proteger, quitar, modo viaje). No toca tu wifi ni tu WARP.
//! Con `VORTEX_CONNECT_CAPTURAS=/carpeta` guarda capturas de la ventana.

// El script de compilación del crate mete el C++ de cxx-qt también aquí: hay que enlazar sus crates.
extern crate cxx_qt;
extern crate cxx_qt_lib;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const NMCLI: &str = r#"#!/bin/sh
case "$*" in
  *"TYPE,STATE"*) echo "wifi:connected" ;;
  *"-f TYPE device"*) echo "wifi" ;;
  *"wifi list"*) printf '*:Cafe\\: Centro:70:\n :Aeropuerto:40:\n :Casa:99:WPA2\n' ;;
  *) ;;
esac
"#;

fn guion(d: &Path, nombre: &str, cuerpo: &str) {
    let r = d.join(nombre);
    std::fs::write(&r, cuerpo).unwrap();
    std::fs::set_permissions(&r, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// /204 da 204 y /traza dice «warp=on» cuando el warp-cli falso quedó conectado.
fn servidor(estado: PathBuf) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let estado = estado.clone();
            std::thread::spawn(move || {
                let mut b = [0u8; 2048];
                let n = s.read(&mut b).unwrap_or(0);
                let pedido = String::from_utf8_lossy(&b[..n]);
                let ruta = pedido.split_whitespace().nth(1).unwrap_or("/").to_string();
                let r = if ruta == "/traza" {
                    let t = std::fs::read_to_string(&estado).unwrap_or_default();
                    let on = t.contains("Connected") && !t.contains("Disc");
                    format!(
                        "HTTP/1.0 200 OK\r\n\r\n{}\n",
                        if on { "warp=on" } else { "warp=off" }
                    )
                } else {
                    "HTTP/1.0 204 No Content\r\n\r\n".to_string()
                };
                let _ = s.write_all(r.as_bytes());
            });
        }
    });
    base
}

#[test]
fn flujo() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path();
    let ruta = |n: &str| d.join(n).to_string_lossy().into_owned();
    guion(d, "nmcli", NMCLI);
    guion(
        d,
        "warp-cli",
        &format!(
            r#"#!/bin/sh
echo "$*" >> "{reg}"
case "$*" in
  *status*) cat "{estado}" ;;
  *" connect"*) echo "Status update: Connected" > "{estado}" ;;
  *disconnect*) echo "Status update: Disconnected" > "{estado}" ;;
esac
"#,
            reg = ruta("reg"),
            estado = ruta("estado"),
        ),
    );
    guion(
        d,
        "systemctl",
        &format!(
            "#!/bin/sh\necho \"$*\" >> \"{}\"\ncase \"$*\" in *is-active*) [ -f \"{}\" ] ;; *\" start\"*) : > \"{}\" ;; *\" stop\"*) rm -f \"{}\" ;; esac\n",
            ruta("reg-svc"),
            ruta("svc"),
            ruta("svc"),
            ruta("svc")
        ),
    );
    // Sin estos, «Cifrar» y «Quitar» tocarían el DNS de verdad del equipo donde corren las pruebas.
    guion(d, "resolvectl", "#!/bin/sh\nexit 0\n");
    guion(d, "ip", "#!/bin/sh\nexit 0\n");
    std::fs::write(d.join("estado"), "Status update: Disconnected\n").unwrap();
    let base = servidor(d.join("estado"));
    let capturas = std::env::var("VORTEX_CONNECT_CAPTURAS").unwrap_or_default();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vortex-connect-rs"));
    cmd.env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", d.display()))
        .env("HOME", d)
        .env("LANG", "C.UTF-8")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("BROWSER", "true")
        .env("TMPDIR", d)
        .env("XDG_RUNTIME_DIR", d)
        .env("XDG_CONFIG_HOME", d.join("config"))
        .env("XDG_STATE_HOME", d.join("estado-xdg"))
        .env("XDG_CACHE_HOME", d.join("cache"))
        .env("VORTEX_CONNECT_RECORRER", "1")
        .env("VORTEX_CONNECT_PRUEBA", format!("{base}/204"))
        .env("VORTEX_CONNECT_TRAZA", format!("{base}/traza"))
        .env("VORTEX_CONNECT_COLORES", d.join("no-hay.json"));
    if !capturas.is_empty() {
        cmd.env("VORTEX_CONNECT_CAPTURAS", &capturas);
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
    let ordenes = std::fs::read_to_string(d.join("reg")).unwrap_or_default();
    let guardado: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(d.join("config/vortex/connect.json")).unwrap())
            .unwrap();
    assert_eq!(datos["errores"], serde_json::json!([]));
    assert_eq!(
        datos["inicio"],
        serde_json::json!(["sin_proteccion", "Cafe: Centro", true, "Sin protección"])
    );
    assert_eq!(datos["redes"], serde_json::json!(["Cafe: Centro", "Aeropuerto"]));
    assert_eq!(datos["velo"], true, "pide las condiciones antes de cifrar");
    assert_eq!(datos["sin_aceptar"], false);
    assert_eq!(datos["aceptadas"], true);
    assert_eq!(datos["velo_tras"], false);
    assert_eq!(
        serde_json::Value::Array(datos["protegido"].as_array().unwrap()[..3].to_vec()),
        serde_json::json!(["protegido", "Protegido", false]),
        "{}",
        datos["protegido"]
    );
    assert_eq!(
        datos["sin_proteger"],
        serde_json::json!(["sin_proteccion", false])
    );
    assert_eq!(datos["viaje"], true);
    assert_eq!(guardado["condiciones"], true);
    assert_eq!(guardado["modo_viaje"], true);
    assert!(!ordenes.is_empty(), "llegó a warp-cli después de aceptar");
    // «--accept-tos» solo después de aceptar, y el modo viaje pidió prender su servicio
    assert!(ordenes.lines().all(|o| o.starts_with("--accept-tos")));
    assert!(
        std::fs::read_to_string(d.join("reg-svc"))
            .unwrap()
            .contains("--user enable --now vortex-connect-viaje.service")
    );
    if let Some(m) = datos["rss_anon"].as_str() {
        eprintln!("RssAnon de la ventana: {m}; Rss: {}", datos["rss"]);
    }
}

#[test]
fn sin_aceptar_no_se_llama_a_warp_cli() {
    // La ventana abre, mira el estado y busca redes, pero ningún comando llega a warp-cli sin aceptar.
    let t = tempfile::tempdir().unwrap();
    let d = t.path();
    guion(d, "nmcli", NMCLI);
    guion(
        d,
        "warp-cli",
        &format!("#!/bin/sh\necho \"$*\" >> \"{}\"\n", d.join("reg").display()),
    );
    guion(
        d,
        "systemctl",
        &format!(
            "#!/bin/sh\necho \"$*\" >> \"{}\"\nexit 1\n",
            d.join("reg-svc").display()
        ),
    );
    let r = Command::new(env!("CARGO_BIN_EXE_vortex-connect-rs"))
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", d.display()))
        .env("HOME", d)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env("TMPDIR", d)
        .env("XDG_RUNTIME_DIR", d)
        .env("XDG_CONFIG_HOME", d.join("config"))
        .env("VORTEX_CONNECT_PRUEBA", "http://127.0.0.1:9/")
        .env("VORTEX_CONNECT_COLORES", d.join("no-hay.json"))
        .arg("--prueba-humo")
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!d.join("reg").exists(), "warp-cli no se llama sin aceptar");
    let svc = std::fs::read_to_string(d.join("reg-svc")).unwrap_or_default();
    assert!(!svc.contains("start"), "el servicio de WARP no se prende: {svc}");
}
