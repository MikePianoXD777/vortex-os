//! Ollama por `https://`: contra un `openssl s_server` local con un certificado propio para «localhost».
//! El certificado solo se acepta si su autoridad es de confianza (aquí, con `SSL_CERT_FILE` en un proceso
//! hijo de la prueba) y si el nombre coincide; si no, la conexión falla.

use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use vortex_local_ai::http::{Destino, ErrHttp, pedir};

const ESPERA: Duration = Duration::from_secs(5);

fn openssl(dir: &Path, args: &[&str]) {
    let ok = Command::new("openssl")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("openssl")
        .success();
    assert!(ok, "openssl {args:?}");
}

/// Una autoridad propia y un certificado de «localhost» firmado por ella.
fn certificados(dir: &Path) {
    openssl(
        dir,
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "2",
            "-subj",
            "/CN=Prueba CA",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
        ],
    );
    openssl(
        dir,
        &[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-subj",
            "/CN=localhost",
            "-keyout",
            "srv.key",
            "-out",
            "srv.csr",
        ],
    );
    std::fs::write(dir.join("ext.cnf"), "subjectAltName=DNS:localhost\n").unwrap();
    openssl(
        dir,
        &[
            "x509",
            "-req",
            "-in",
            "srv.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-days",
            "2",
            "-extfile",
            "ext.cnf",
            "-out",
            "srv.pem",
        ],
    );
}

struct Servidor(Child);

impl Drop for Servidor {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `s_server -WWW` sirve los archivos de su carpeta: GET /api/version → `api/version`.
fn servidor(dir: &Path) -> (Servidor, u16) {
    let puerto = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    std::fs::create_dir_all(dir.join("api")).unwrap();
    std::fs::write(dir.join("api/version"), "{\"version\":\"0.9.9\"}\n").unwrap();
    let hijo = Command::new("openssl")
        .args(["s_server", "-quiet", "-WWW", "-accept"])
        .arg(puerto.to_string())
        .args(["-cert", "srv.pem", "-key", "srv.key"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("s_server");
    let s = Servidor(hijo);
    let inicio = Instant::now();
    while TcpStream::connect(("127.0.0.1", puerto)).is_err() {
        assert!(inicio.elapsed() < ESPERA, "s_server no abrió el puerto");
        std::thread::sleep(Duration::from_millis(50));
    }
    (s, puerto)
}

/// Corre `hijo_tls` en un proceso aparte con estas variables.
fn hijo(puerto: u16, confiar: Option<&Path>, url: &str) -> bool {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["hijo_tls", "--exact", "--test-threads=1", "--nocapture"])
        .env("VORTEX_PRUEBA_TLS", url.replace("PUERTO", &puerto.to_string()));
    match confiar {
        Some(ca) => c.env("SSL_CERT_FILE", ca).env("VORTEX_PRUEBA_TLS_ESPERA", "ok"),
        None => c
            .env("SSL_CERT_FILE", "/nonexistent")
            .env("VORTEX_PRUEBA_TLS_ESPERA", "falla"),
    };
    c.stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[test]
fn ollama_por_https() {
    let dir = tempfile::tempdir().unwrap();
    certificados(dir.path());
    let (_s, puerto) = servidor(dir.path());
    let ca = dir.path().join("ca.pem");
    // Con la autoridad de confianza y el nombre correcto: contesta.
    assert!(
        hijo(puerto, Some(&ca), "https://localhost:PUERTO"),
        "https con certificado bueno"
    );
    // Sin confiar en la autoridad: no se conecta.
    assert!(
        hijo(puerto, None, "https://localhost:PUERTO"),
        "certificado de una autoridad desconocida"
    );
    // De confianza pero con otro nombre (127.0.0.1 no está en el certificado): tampoco.
    let ok = {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["hijo_tls", "--exact", "--test-threads=1"])
            .env("VORTEX_PRUEBA_TLS", format!("https://127.0.0.1:{puerto}"))
            .env("SSL_CERT_FILE", &ca)
            .env("VORTEX_PRUEBA_TLS_ESPERA", "falla");
        c.stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    assert!(ok, "nombre que no coincide");
}

/// La mitad que corre en el proceso hijo (sin `VORTEX_PRUEBA_TLS` no hace nada).
#[test]
fn hijo_tls() {
    let Ok(url) = std::env::var("VORTEX_PRUEBA_TLS") else {
        return;
    };
    let espera = std::env::var("VORTEX_PRUEBA_TLS_ESPERA").unwrap_or_default();
    let r = pedir(&Destino::de_url(&url), "GET", "/api/version", None, ESPERA);
    if espera == "ok" {
        let (estado, cuerpo) = r.expect("respuesta");
        assert_eq!(estado, 200);
        assert_eq!(String::from_utf8_lossy(&cuerpo).trim(), "{\"version\":\"0.9.9\"}");
    } else {
        assert_eq!(r.err(), Some(ErrHttp::Conexion));
    }
}
