//! Las pruebas de `apps/vortex-camera/pruebas/test_motor.py`, contra el motor de Rust: el almacén, una
//! cámara falsa por RTSP (la misma `camara_falsa.py`, con ffmpeg), fotos y grabaciones, y un servidor
//! ONVIF/CGI de mentiras en 127.0.0.1 para el PTZ. Todo en carpetas temporales: no toca tus cámaras ni tus fotos.

use serde_json::{Map, Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_camera::camaras::{Almacen, Camara, limpiar_serie};
use vortex_camera::cripto::{base64, base64_leer, md5_hex, sha1};
use vortex_camera::descubrir::{Prueba, probar_rtsp};
use vortex_camera::fecha::FechaHora;
use vortex_camera::{grabar, ptz};

fn mapa(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap()
}

fn raiz_repo() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
}

// --- el almacén ---

#[test]
fn urls_de_imou() {
    let c = Camara::desde(&mapa(
        json!({"id": "a", "nombre": "Patio", "codigo": "L2C3 D4E5", "ip": "192.168.1.20"}),
    ));
    assert_eq!(
        c.url_vivo(false),
        "rtsp://admin:L2C3%20D4E5@192.168.1.20:554/cam/realmonitor?channel=1&subtype=1"
    );
    assert!(c.url_vivo(true).ends_with("subtype=0"));
    let url = c.url_grabado(
        &FechaHora::new(2026, 10, 30, 14, 5, 0),
        &FechaHora::new(2026, 10, 30, 15, 0, 0),
    );
    assert!(
        url.contains("/cam/playback?channel=1&starttime=2026_10_30_14_05_00&endtime=2026_10_30_15_00_00")
    );
    assert_eq!(
        Camara::desde(&mapa(json!({"id": "b", "nombre": "x"}))).url_vivo(false),
        ""
    );
    let v6 = Camara::desde(&mapa(
        json!({"id": "c", "nombre": "x", "ip": "fd7a::1", "codigo": "a"}),
    ));
    assert!(v6.url_vivo(false).contains("@[fd7a::1]:554/"));
}

#[test]
fn el_codigo_no_va_a_la_interfaz() {
    let c = Camara::desde(&mapa(json!({"id": "a", "nombre": "Patio", "codigo": "SECRETO1"})));
    assert!(!c.publica().to_string().contains("SECRETO1"));
    assert_eq!(c.publica()["tieneCodigo"], true);
}

#[test]
fn serie_y_datos_raros() {
    assert_eq!(limpiar_serie(" 9a0 b-1234 "), "9A0B1234");
    let c = Camara::desde(&mapa(
        json!({"id": "a", "nombre": "  ", "puerto": "abc", "canal": -3, "otro": 1}),
    ));
    assert_eq!((c.nombre.as_str(), c.puerto, c.canal), ("Cámara", 554, 1));
    // Lo que manda QML: números como double.
    let q = Camara::desde(&mapa(
        json!({"id": "q", "nombre": "Q", "puerto": 8554.0, "canal": 2.0}),
    ));
    assert_eq!((q.puerto, q.canal), (8554, 2));
}

#[test]
fn archivo_privado_y_recargable() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let mut a = Almacen::new(Some(d.path()));
    a.agregar(&mapa(json!({"nombre": "Patio", "codigo": "X", "ip": "10.0.0.2"})));
    let modo = std::fs::metadata(&a.ruta).unwrap().permissions().mode() & 0o777;
    assert_eq!(modo, 0o600);
    let otro = Almacen::new(Some(d.path()));
    assert_eq!(otro.camaras.len(), 1);
    assert_eq!(
        (otro.camaras[0].nombre.as_str(), otro.camaras[0].codigo.as_str()),
        ("Patio", "X")
    );
}

#[test]
fn archivo_roto_no_tumba() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("camaras.json"), "{ esto no es json").unwrap();
    let a = Almacen::new(Some(d.path()));
    assert!(a.camaras.is_empty());
    assert_eq!(a.ventanas.len(), 1);
}

#[test]
fn el_mosaico_crece_solo() {
    let d = tempfile::tempdir().unwrap();
    let mut a = Almacen::new(Some(d.path()));
    let ids: Vec<String> = (1..27)
        .map(|n| {
            a.agregar(&mapa(
                json!({"nombre": format!("C{n}"), "codigo": "x", "ip": format!("10.0.0.{n}")}),
            ))
            .id
        })
        .collect();
    assert_eq!(a.ventanas[0].division, 32); // 26 cámaras: de 4 a 9, 16, 25 y 32
    assert_eq!(a.ventanas[0].celdas[..26], ids[..]);
    for n in 27..34 {
        a.agregar(&mapa(
            json!({"nombre": format!("C{n}"), "codigo": "x", "ip": "10.0.1.1"}),
        ));
    }
    assert_eq!(a.ventanas.len(), 2); // la 33 ya no cabe: otra ventana
    let primeras: Vec<String> = a.camaras[..32].iter().map(|c| c.id.clone()).collect();
    assert_eq!(a.ventanas[0].celdas, primeras);
    assert_eq!(a.ventanas[1].celdas[0], a.camaras[32].id);
}

#[test]
fn poner_celda_intercambia() {
    let d = tempfile::tempdir().unwrap();
    let mut al = Almacen::new(Some(d.path()));
    let a = al
        .agregar(&mapa(json!({"nombre": "A", "codigo": "x", "ip": "1.1.1.1"})))
        .id;
    let b = al
        .agregar(&mapa(json!({"nombre": "B", "codigo": "x", "ip": "1.1.1.2"})))
        .id;
    al.poner_celda(0, 1, &a);
    assert_eq!(al.ventanas[0].celdas[..2], [b.clone(), a.clone()]);
    al.poner_celda(0, 0, "");
    al.poner_celda(0, 9, &a); // fuera de la división: no hace nada
    assert_eq!(al.ventanas[0].celdas[..2], [String::new(), a.clone()]);
    al.quitar(&a);
    assert!(!al.ventanas[0].celdas.contains(&a));
}

#[test]
fn editar_codigo_olvida_ptz() {
    let d = tempfile::tempdir().unwrap();
    let mut al = Almacen::new(Some(d.path()));
    let c = al.agregar(&mapa(json!({"nombre": "A", "codigo": "x", "ip": "1.1.1.1"})));
    al.actualizar(&c.id, &mapa(json!({"ptz": "onvif"})));
    assert_eq!(al.camara(&c.id).unwrap().ptz, "onvif");
    al.actualizar(&c.id, &mapa(json!({"nombre": "Otro"})));
    assert_eq!(al.camara(&c.id).unwrap().ptz, "onvif");
    al.actualizar(&c.id, &mapa(json!({"codigo": "y"})));
    assert_eq!(al.camara(&c.id).unwrap().ptz, "");
}

#[test]
fn ventanas() {
    let d = tempfile::tempdir().unwrap();
    let mut al = Almacen::new(Some(d.path()));
    let i = al.nueva_ventana();
    al.renombrar_ventana(i, "  Calle  ");
    al.poner_division(i, 9);
    assert_eq!(al.ventanas[i].nombre, "Calle");
    assert_eq!(al.ventanas[i].celdas.len(), 9);
    al.poner_division(i, 7); // no existe esa división
    assert_eq!(al.ventanas[i].division, 9);
    al.quitar_ventana(0);
    al.quitar_ventana(0); // la última nunca se quita
    assert_eq!(al.ventanas.len(), 1);
}

/// El archivo que escribe la de Rust lo lee la de Python, y al revés: las dos ven las mismas cámaras.
#[test]
fn mismo_archivo_que_la_de_python() {
    let d = tempfile::tempdir().unwrap();
    let mut al = Almacen::new(Some(d.path()));
    let c = al.agregar(&mapa(
        json!({"nombre": "Patio ñ", "codigo": "C@d:1", "ip": "10.0.0.9", "serie": "9a0-b"}),
    ));
    al.poner_division(0, 9);
    let guion = format!(
        "import sys, json; sys.path.insert(0, {:?}); from vortex_camera.camaras import Almacen\n\
         a = Almacen({:?}); c = a.camaras[0]\n\
         print(json.dumps([c.id, c.nombre, c.codigo, c.serie, c.url_vivo(), a.ventanas[0]['division']]))\n\
         a.agregar({{'nombre': 'Calle', 'codigo': 'z', 'ip': '10.0.0.10'}})",
        raiz_repo().join("apps/vortex-camera").to_string_lossy(),
        d.path().to_string_lossy()
    );
    let o = Command::new("python3").args(["-c", &guion]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let visto: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(
        visto,
        json!([c.id, "Patio ñ", "C@d:1", "9A0B", c.url_vivo(false), 9])
    );
    let otra = Almacen::new(Some(d.path()));
    let nombres: Vec<&str> = otra.camaras.iter().map(|c| c.nombre.as_str()).collect();
    assert_eq!(nombres, ["Patio ñ", "Calle"]);
    assert_eq!(otra.ventanas[0].celdas[1], otra.camaras[1].id);
}

// --- con la cámara falsa (RTSP de verdad, con ffmpeg) ---

/// La `CamaraFalsa` de las pruebas de Python, en su propio proceso.
struct Falsa {
    proceso: Child,
    puerto: u16,
    codigo: String,
}

impl Falsa {
    fn new(codigo: &str, digest: bool) -> Falsa {
        let guion = format!(
            "import sys; sys.path.insert(0, {:?}); from camara_falsa import CamaraFalsa\n\
             c = CamaraFalsa(0, 'admin', {codigo:?}, digest={})\n\
             print(c.puerto, flush=True); sys.stdin.read(); c.cerrar()",
            raiz_repo().join("apps/vortex-camera/pruebas").to_string_lossy(),
            if digest { "True" } else { "False" }
        );
        let mut proceso = Command::new("python3")
            .args(["-c", &guion])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut linea = String::new();
        BufReader::new(proceso.stdout.take().unwrap())
            .read_line(&mut linea)
            .unwrap();
        Falsa {
            proceso,
            puerto: linea.trim().parse().expect("la cámara falsa no arrancó"),
            codigo: codigo.into(),
        }
    }

    fn url(&self, codigo: Option<&str>) -> String {
        let c = Camara::desde(&mapa(json!({
            "id": "f", "nombre": "F", "ip": "127.0.0.1", "puerto": self.puerto,
            "codigo": codigo.unwrap_or(&self.codigo),
        })));
        c.url_vivo(false)
    }
}

impl Drop for Falsa {
    fn drop(&mut self) {
        drop(self.proceso.stdin.take()); // cierra su stdin: sale sola
        let limite = Instant::now() + Duration::from_secs(5);
        while Instant::now() < limite && matches!(self.proceso.try_wait(), Ok(None)) {
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.proceso.kill();
        let _ = self.proceso.wait();
    }
}

const RUTA: &str = "/cam/realmonitor?channel=1&subtype=1";
const T: Duration = Duration::from_secs(4);

#[test]
fn probar_rtsp_ok_codigo_y_sin_conexion() {
    let f = Falsa::new("L2C3D4E5", true);
    assert_eq!(
        probar_rtsp("127.0.0.1", f.puerto, RUTA, "admin", &f.codigo, T),
        Prueba::Ok
    );
    assert_eq!(
        probar_rtsp("127.0.0.1", f.puerto, RUTA, "admin", "MALO", T),
        Prueba::Codigo
    );
    let libre = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert_eq!(
        probar_rtsp("127.0.0.1", libre, RUTA, "admin", "x", Duration::from_secs(1)),
        Prueba::SinConexion
    );
}

#[test]
fn basic_de_las_dahua_viejas() {
    let f = Falsa::new("L2C3D4E5", false);
    assert_eq!(
        probar_rtsp("127.0.0.1", f.puerto, RUTA, "admin", &f.codigo, T),
        Prueba::Ok
    );
    assert_eq!(
        probar_rtsp("127.0.0.1", f.puerto, RUTA, "admin", "MALO", T),
        Prueba::Codigo
    );
}

#[test]
fn codigo_con_simbolos_y_foto() {
    // Una contraseña cambiada en Imou Life puede llevar @, :, / o #: en la URL van escapados.
    let f = Falsa::new("Ab#1@x:y/Z%", true);
    assert_eq!(
        probar_rtsp("127.0.0.1", f.puerto, RUTA, "admin", &f.codigo, T),
        Prueba::Ok
    );
    let d = tempfile::tempdir().unwrap();
    let ruta = grabar::foto(&f.url(None), d.path(), "R", Duration::from_secs(20)).unwrap();
    assert!(std::fs::metadata(&ruta).unwrap().len() > 1000);
}

#[test]
fn foto_y_foto_con_codigo_malo() {
    let f = Falsa::new("L2C3D4E5", true);
    let d = tempfile::tempdir().unwrap();
    let ruta = grabar::foto(&f.url(None), d.path(), "Patio/Cochera", Duration::from_secs(20)).unwrap();
    assert!(
        ruta.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Patio Cochera ")
    );
    assert!(std::fs::metadata(&ruta).unwrap().len() > 1000);
    let e = grabar::foto(&f.url(Some("MALO")), d.path(), "Patio", Duration::from_secs(20)).unwrap_err();
    assert!(e.contains("código"), "{e}");
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1); // la fallida no deja archivo
}

#[test]
fn grabar_y_detener() {
    let f = Falsa::new("L2C3D4E5", true);
    let d = tempfile::tempdir().unwrap();
    let mut g = grabar::Grabacion::new(&f.url(None), d.path(), "Patio").unwrap();
    std::thread::sleep(Duration::from_secs(3));
    assert!(g.activa());
    let ruta = g.detener(Duration::from_secs(8)).unwrap();
    let o = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(&ruta)
        .output()
        .unwrap();
    let duracion: f64 = String::from_utf8_lossy(&o.stdout).trim().parse().unwrap();
    assert!(duracion > 1.0, "{duracion}");
}

#[test]
fn grabacion_con_codigo_malo_se_corta_sola() {
    let f = Falsa::new("L2C3D4E5", true);
    let d = tempfile::tempdir().unwrap();
    let mut g = grabar::Grabacion::new(&f.url(Some("MALO")), d.path(), "Patio").unwrap();
    let limite = Instant::now() + Duration::from_secs(10);
    while g.activa() && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!g.activa());
    assert!(g.error().contains("código"));
    assert!(g.detener(Duration::from_secs(8)).is_none());
}

// --- PTZ contra un servidor ONVIF/CGI de mentiras ---

const CLAVE: &str = "L2C3D4E5";

/// Lo que hizo cada petición: (ruta, acción, x o código).
type Vistos = Arc<Mutex<Vec<(String, String, String)>>>;

fn busca<'a>(texto: &'a str, re: &str) -> Option<&'a str> {
    regex::Regex::new(re)
        .unwrap()
        .captures(texto)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

fn responder(c: &mut std::net::TcpStream, estado: &str, extra: &str, cuerpo: &str) {
    let _ = c.write_all(
        format!(
            "HTTP/1.1 {estado}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
            cuerpo.len()
        )
        .as_bytes(),
    );
}

/// Un servidor como el `_Onvif` de la de Python: ONVIF con WS-Security y el CGI con Digest `qop=auth`.
fn servidor_ptz(solo_cgi: bool) -> (String, Vistos) {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let host = srv.local_addr().unwrap().to_string();
    let vistos: Vistos = Arc::default();
    let v = vistos.clone();
    std::thread::spawn(move || {
        for c in srv.incoming() {
            let Ok(mut c) = c else { break };
            let mut datos = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = c.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                datos.extend_from_slice(&buf[..n]);
                let t = String::from_utf8_lossy(&datos);
                if let Some(fin) = t.find("\r\n\r\n") {
                    let largo: usize = busca(&t.to_lowercase(), r"content-length: *(\d+)")
                        .and_then(|l| l.parse().ok())
                        .unwrap_or(0);
                    if datos.len() >= fin + 4 + largo {
                        break;
                    }
                }
            }
            let t = String::from_utf8_lossy(&datos).into_owned();
            let primera = t.lines().next().unwrap_or("").to_string();
            let ruta = primera.split_whitespace().nth(1).unwrap_or("").to_string();
            if primera.starts_with("POST") {
                if solo_cgi {
                    responder(&mut c, "404 Not Found", "", "");
                    continue;
                }
                let nonce = base64_leer(busca(&t, r"<Nonce[^>]*>([^<]+)<").unwrap_or("")).unwrap_or_default();
                let creado = busca(&t, r"<Created[^>]*>([^<]+)<").unwrap_or("");
                let resumen = busca(&t, r"<Password[^>]*>([^<]+)<").unwrap_or("");
                let mut dato = nonce;
                dato.extend_from_slice(creado.as_bytes());
                dato.extend_from_slice(CLAVE.as_bytes());
                if resumen != base64(&sha1(&dato)) {
                    responder(&mut c, "400 Bad Request", "", "");
                    continue;
                }
                let accion = busca(&t, r"<s:Body><(\w+)").unwrap_or("").to_string();
                let x = busca(&t, r#"x="([^"]+)""#).unwrap_or("").to_string();
                v.lock().unwrap().push((ruta, accion.clone(), x));
                let r = if accion == "GetProfiles" {
                    r#"<Envelope><Body><GetProfilesResponse><trt:Profiles token="MediaProfile000" fixed="true"></trt:Profiles></GetProfilesResponse></Body></Envelope>"#
                } else {
                    "<ok/>"
                };
                responder(&mut c, "200 OK", "", r);
            } else {
                let auth = t
                    .lines()
                    .find(|l| l.to_lowercase().starts_with("authorization:"))
                    .unwrap_or("");
                let campo = |k: &str| {
                    busca(auth, &format!(r#"\b{k}="?([^",]*)"?"#))
                        .unwrap_or("")
                        .to_string()
                };
                let ha1 = md5_hex(&format!("admin:Login to falsa:{CLAVE}"));
                let ha2 = md5_hex(&format!("GET:{}", campo("uri")));
                let bien = md5_hex(&format!(
                    "{ha1}:n1:{}:{}:auth:{ha2}",
                    campo("nc"),
                    campo("cnonce")
                ));
                if campo("response") != bien {
                    responder(
                        &mut c,
                        "401 Unauthorized",
                        "WWW-Authenticate: Digest realm=\"Login to falsa\", nonce=\"n1\", qop=\"auth\"\r\n",
                        "",
                    );
                    continue;
                }
                let accion = busca(&ruta, r"action=([^&]+)").unwrap_or("").to_string();
                let codigo = busca(&ruta, r"code=([^&]+)").unwrap_or("").to_string();
                let base = ruta.split('?').next().unwrap_or("").to_string();
                v.lock().unwrap().push((base, accion, codigo));
                responder(&mut c, "200 OK", "", "OK\r\n");
            }
        }
    });
    (host, vistos)
}

fn cam_ptz(host: &str, id: &str, clave: &str) -> Camara {
    Camara::desde(&mapa(
        json!({"id": id, "nombre": "PTZ", "codigo": clave, "ip": host}),
    ))
}

fn trio(a: &str, b: &str, c: &str) -> (String, String, String) {
    (a.into(), b.into(), c.into())
}

#[test]
fn ptz_onvif() {
    let (host, vistos) = servidor_ptz(false);
    let mut cam = cam_ptz(&host, "p-onvif", CLAVE);
    assert_eq!(ptz::mover(&cam, "izquierda").unwrap(), "onvif");
    cam.ptz = "onvif".into();
    ptz::parar(&cam, "").unwrap();
    assert_eq!(
        *vistos.lock().unwrap(),
        [
            trio("/onvif/media_service", "GetProfiles", ""),
            trio("/onvif/ptz_service", "ContinuousMove", "-0.5"),
            trio("/onvif/ptz_service", "Stop", ""),
        ]
    );
}

#[test]
fn ptz_cgi_si_no_hay_onvif() {
    let (host, vistos) = servidor_ptz(true);
    let mut cam = cam_ptz(&host, "p-cgi", CLAVE);
    assert_eq!(ptz::mover(&cam, "arriba").unwrap(), "cgi");
    cam.ptz = "cgi".into();
    ptz::parar(&cam, "arriba").unwrap();
    assert_eq!(
        *vistos.lock().unwrap(),
        [
            trio("/cgi-bin/ptz.cgi", "start", "Up"),
            trio("/cgi-bin/ptz.cgi", "stop", "Up")
        ]
    );
}

#[test]
fn ptz_clave_mala() {
    let (host, _) = servidor_ptz(false);
    let cam = cam_ptz(&host, "p-mala", "MALO");
    assert!(ptz::mover(&cam, "abajo").is_err());
    assert!(ptz::mover(&cam, "de lado").is_err());
}
