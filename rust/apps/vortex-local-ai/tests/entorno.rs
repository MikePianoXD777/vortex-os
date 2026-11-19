//! Lo que depende de variables de entorno y de programas del sistema (por eso va en una sola prueba, en su
//! propio proceso): `VORTEX_LOCAL_AI_*`, un `vortex-ia` falso (motor de la IA y la memoria de video) y un
//! `ollama` falso que se arranca solo. Nada toca el Ollama, el HOME ni el PATH de verdad de la persona.
#![allow(unsafe_code)] // set_var es unsafe desde la edición 2024

mod falso;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};
use vortex_local_ai::catalogo;
use vortex_local_ai::ollama::Ollama;
use vortex_local_ai::rutas::Rutas;

fn script(dir: &Path, nombre: &str, cuerpo: &str) {
    let p = dir.join(nombre);
    fs::write(&p, format!("#!/bin/sh\n{cuerpo}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

fn vortex_ia(dir: &Path, codigo: i32, salida: &str) {
    fs::write(dir.join("salida"), salida).unwrap();
    fs::write(dir.join("codigo"), codigo.to_string()).unwrap();
}

#[test]
fn entorno() {
    let t = tempfile::tempdir().unwrap();
    let bin = t.path().join("bin");
    fs::create_dir(&bin).unwrap();
    // SAFETY: es la única prueba de este proceso y todavía no hay otros hilos.
    unsafe {
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
        std::env::set_var("VORTEX_LOCAL_AI_HOME", t.path().join("casa"));
        std::env::set_var("VORTEX_LOCAL_AI_NO_ARRANCAR", "1");
        std::env::set_var("VORTEX_LOCAL_AI_OLLAMA", "http://127.0.0.1:9");
        std::env::remove_var("VORTEX_LOCAL_AI_VRAM_MB");
    }

    // --- rutas, URL y permiso de arrancar salen del entorno
    let r = Rutas::actual();
    assert_eq!(r.datos, t.path().join("casa/datos"));
    assert_eq!(r.cache, t.path().join("casa/cache"));
    assert_eq!(r.vault_por_defecto, t.path().join("casa/vault"));
    let o = Ollama::new(None, None);
    assert_eq!(o.url, "http://127.0.0.1:9");
    assert!(!o.puede_arrancar);

    // --- la memoria de video fija
    unsafe { std::env::set_var("VORTEX_LOCAL_AI_VRAM_MB", "6144") };
    let v = catalogo::vram_disponible();
    assert_eq!(v.total_mb, 6144);
    assert_eq!(v.tipo, "nvidia");
    unsafe { std::env::remove_var("VORTEX_LOCAL_AI_VRAM_MB") };

    // --- vortex-ia falso: la pregunta por el motor, la respuesta y la tarjeta de video
    script(
        &bin,
        "vortex-ia",
        "d=\"$(dirname \"$0\")\"\necho \"$@\" >> \"$d/args\"\ncat \"$d/salida\"\nexit \"$(cat \"$d/codigo\")\"",
    );
    let si = r#"{"preguntar": true, "texto": "¿Lo cambio?"}"#;
    vortex_ia(&bin, 0, si);
    assert_eq!(catalogo::motor_pendiente().as_deref(), Some("¿Lo cambio?"));
    vortex_ia(&bin, 0, r#"{"preguntar": false}"#);
    assert_eq!(catalogo::motor_pendiente(), None);
    vortex_ia(&bin, 1, si);
    assert_eq!(catalogo::motor_pendiente(), None);
    vortex_ia(&bin, 0, "no es json");
    assert_eq!(catalogo::motor_pendiente(), None);

    vortex_ia(&bin, 0, "");
    assert_eq!(catalogo::elegir_motor(true), "ok");
    assert_eq!(catalogo::elegir_motor(false), "ok");
    let args = fs::read_to_string(bin.join("args")).unwrap();
    assert!(
        args.contains("motor --pendiente") && args.contains("motor --si") && args.contains("motor --no"),
        "{args}"
    );
    vortex_ia(&bin, 2, "");
    assert_eq!(catalogo::elegir_motor(true), "cancelado");
    vortex_ia(&bin, 1, "");
    assert_eq!(catalogo::elegir_motor(true), "error");

    // Una tarjeta que se queda corta → el procesador
    for (gb, entrada) in [(2.0, true), (4.0, true), (2.0, false)] {
        vortex_ia(
            &bin,
            0,
            &format!(
                r#"{{"maquina": {{"gpu": {{"tipo": "amd", "nombre": "RX 550", "vram_gb": {gb}, "libre_gb": 1.0, "de_entrada": {entrada}}}}}}}"#
            ),
        );
        let v = catalogo::vram_disponible();
        assert_eq!(v.tipo, "cpu");
        assert!(v.de_entrada);
        assert_eq!(v.total_mb, v.ram_mb);
    }
    vortex_ia(
        &bin,
        0,
        r#"{"maquina": {"gpu": {"tipo": "amd", "nombre": "RX 6700 XT", "vram_gb": 12.0, "libre_gb": 11.0, "de_entrada": false}}}"#,
    );
    let v = catalogo::vram_disponible();
    assert_eq!(
        (v.tipo.as_str(), v.nombre.as_str(), v.total_mb),
        ("amd", "RX 6700 XT", 12288)
    );

    // --- un `ollama` falso: se arranca solo, y al parar solo se apaga el que arrancó esta instancia
    let puerto = falso::puerto_cerrado();
    script(
        &bin,
        "ollama",
        r#"exec python3 -c '
import os, http.server
host, port = os.environ["OLLAMA_HOST"].rsplit(":", 1)
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        b = b"{\"version\": \"0.0.1\"}"
        self.send_response(200)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def log_message(self, *a): pass
http.server.HTTPServer((host, int(port)), H).serve_forever()
'"#,
    );
    let o = Ollama::new(Some(&format!("http://127.0.0.1:{puerto}")), Some(true));
    assert!(!o.disponible());
    assert!(o.instalado());
    assert!(
        o.asegurar_servidor(Duration::from_secs(15)),
        "{}",
        o.ultimo_error()
    );
    assert!(o.arranco_algo());
    assert!(o.disponible());
    assert_eq!(o.version().as_deref(), Some("0.0.1"));
    o.parar_propio();
    let limite = Instant::now() + Duration::from_secs(5);
    while o.disponible() && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!o.disponible(), "parar_propio no apagó lo que arrancó");

    // Si el comando no existe, lo dice con claridad
    fs::remove_file(bin.join("ollama")).unwrap();
    unsafe { std::env::set_var("PATH", bin.display().to_string()) };
    let o = Ollama::new(Some(&format!("http://127.0.0.1:{puerto}")), Some(true));
    assert!(!o.asegurar_servidor(Duration::from_secs(1)));
    assert!(
        o.ultimo_error().contains("no está instalado"),
        "{}",
        o.ultimo_error()
    );
}
