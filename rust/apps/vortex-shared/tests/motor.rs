//! Pruebas de Vortexine Shared con un sendme falso (copia archivos): el motor y el servicio de punta a punta,
//! por un socket de verdad en este equipo (127.0.0.1, puerto libre). Es el port de `pruebas/test_motor.py`.
//! No abre puertos en tu red, no saca avisos en tu escritorio y no toca tu HOME: cada prueba vive en una
//! carpeta temporal con sus programas falsos (sendme, notify-send, gdbus) al principio del `PATH`.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_shared::motor::{self, Envio};
use vortex_shared::servicio::Servicio;
use vortex_shared::sistema::Sistema;
use vortex_shared::{Config, thunar};

const SENDME: &str = r#"#!/usr/bin/env python3
import base64, os, shutil, sys, time
o = sys.argv[1]
if o == "send":
    ruta = sys.argv[2]
    ticket = "blob" + base64.b32encode(ruta.encode()).decode().lower().rstrip("=") + "aaaaaaaaaa"
    open("@BASE@/ultimo-cwd", "w").write(os.getcwd())
    print("imported", ruta); print("to get this data, use"); print("sendme receive " + ticket, flush=True)
    time.sleep(600)
elif o == "receive":
    t = sys.argv[2][4:-10]
    if os.path.exists("@BASE@/colgado"):
        os.mkdir(".sendme-recv-x"); time.sleep(600)
    ruta = base64.b32decode(t.upper() + "=" * (-len(t) % 8)).decode()
    os.mkdir(".sendme-recv-x"); open(".sendme-recv-x/blob", "w").write("x")
    nombre = os.path.basename(ruta)
    if os.path.isdir(ruta): shutil.copytree(ruta, nombre)
    else: shutil.copy(ruta, nombre)
    shutil.rmtree(".sendme-recv-x")
"#;

const NOTIFY: &str = r#"#!/bin/sh
for a in "$@"; do printf '%s\n' "$a"; done >>"@BASE@/avisos"
case "$*" in
  *aceptar=*)
    [ -e "@BASE@/colgar-aviso" ] && { echo 7; exec sleep 60; }
    echo 7; cat "@BASE@/respuesta" 2>/dev/null
    [ -e "@BASE@/respuesta2" ] && mv "@BASE@/respuesta2" "@BASE@/respuesta" ;;
esac
"#;

const GDBUS: &str = "#!/bin/sh\necho cerrado >>\"@BASE@/avisos\"\n";

/// Una compu de mentira: carpetas temporales, programas falsos y la `Config` que los usa.
struct Entorno {
    dir: tempfile::TempDir,
    cfg: Config,
}

fn programa(bin: &Path, nombre: &str, texto: &str, base: &Path) {
    let r = bin.join(nombre);
    std::fs::write(&r, texto.replace("@BASE@", &base.to_string_lossy())).unwrap();
    std::fs::set_permissions(&r, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Entorno {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_path_buf();
        let bin = base.join("bin");
        std::fs::create_dir(&bin).unwrap();
        programa(&bin, "sendme", SENDME, &base);
        programa(&bin, "notify-send", NOTIFY, &base);
        programa(&bin, "gdbus", GDBUS, &base);
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
        let cfg = Config {
            puerto: 1770,
            sin_avance: Duration::from_secs(2),
            espera_respuesta: Duration::from_secs(2),
            inactivo: Duration::from_secs(1),
            carpeta_fija: Some(base.join("recibidos")),
            cache: base.join("cache").join("vortex-shared"),
            marca_juego: base.join("estado").join("vortex").join("modo-juego"),
            run: base.join("run"),
            config: base.join("conf"),
            usuario: "probador".into(),
            home: base.join("home"),
            sistema: Sistema::con_path(path),
        };
        Entorno { dir, cfg }
    }

    fn base(&self) -> &Path {
        self.dir.path()
    }

    /// Crea archivos de prueba en una carpeta nueva y devuelve sus rutas.
    fn origen(&self, archivos: &[&str]) -> Vec<PathBuf> {
        let d = tempfile::Builder::new().tempdir_in(self.base()).unwrap().keep();
        archivos
            .iter()
            .map(|a| {
                let r = d.join(a);
                std::fs::create_dir_all(r.parent().unwrap()).unwrap();
                std::fs::write(&r, format!("contenido de {a}")).unwrap();
                r
            })
            .collect()
    }

    fn recibidos(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.base().join("recibidos"))
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    fn avisos(&self) -> Option<String> {
        std::fs::read_to_string(self.base().join("avisos")).ok()
    }

    fn marcar(&self, nombre: &str, texto: &str) {
        std::fs::write(self.base().join(nombre), texto).unwrap();
    }

    fn ida_y_vuelta(&self, rutas: &[PathBuf]) -> (Vec<PathBuf>, PathBuf, Option<PathBuf>) {
        let mut envio = Envio::new(&self.cfg, rutas).unwrap();
        let ticket = envio.preparar(Duration::from_secs(20)).unwrap();
        let quedaron = motor::recibir(&self.cfg, &ticket, None, &|| false, 0).unwrap();
        let (cwd, lote) = (envio.cwd.clone(), envio.lote.clone());
        envio.terminar();
        (quedaron, cwd, lote)
    }
}

fn nombres(rutas: &[PathBuf]) -> Vec<String> {
    let mut v: Vec<String> = rutas
        .iter()
        .map(|r| r.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn ticket_ok() -> String {
    format!("blob{}", "a".repeat(20))
}

// --- Reglas ---

#[test]
fn quien_puede_avisar() {
    for ip in [
        "192.168.1.5",
        "10.0.0.2",
        "172.16.1.1",
        "100.94.20.35",
        "127.0.0.1",
        "::1",
        "fd7a:115c::1",
        "::ffff:192.168.1.9",
        "fe80::1%enp3s0",
    ] {
        assert!(motor::ip_permitida(ip), "{ip}");
    }
    for ip in [
        "8.8.8.8",
        "138.186.28.249",
        "2001:4860::1",
        "::ffff:8.8.8.8",
        "basura",
        "",
    ] {
        assert!(!motor::ip_permitida(ip), "{ip}");
    }
}

#[test]
fn el_ticket_no_puede_ser_una_opcion_ni_llevar_nada_raro() {
    let base = json!({"v": 1, "usuario": "mike", "tam": 1, "nombres": ["a"]});
    for ticket in [
        json!("--help"),
        json!("blob; rm -rf ~"),
        json!("blob$(x)"),
        json!("BLOBAAAAAAAAAAAA"),
        json!(""),
        Value::Null,
        json!(5),
        json!("blob"),
    ] {
        let mut d = base.clone();
        d["ticket"] = ticket.clone();
        assert!(motor::validar_peticion(&d).is_err(), "{ticket}");
    }
    let mut d = base;
    d["ticket"] = json!(ticket_ok());
    assert!(motor::validar_peticion(&d).is_ok());
}

#[test]
fn peticiones_rotas() {
    let ok = json!({"v": 1, "ticket": ticket_ok(), "usuario": "mike", "tam": 1, "nombres": ["a"]});
    let con = |clave: &str, valor: Value| {
        let mut d = ok.clone();
        d[clave] = valor;
        d
    };
    let malas = [
        json!([]),
        json!("x"),
        con("v", json!(9)),
        con("usuario", json!("a/b")),
        con("tam", json!(-1)),
        con("tam", json!(true)),
        con("nombres", json!([])),
        con("nombres", json!("a")),
    ];
    for malo in malas {
        assert!(motor::validar_peticion(&malo).is_err(), "{malo}");
    }
    assert!(motor::validar_peticion(&ok).is_ok());
}

#[test]
fn los_nombres_se_limpian() {
    let mut lista = vec!["<b>hola</b>\n&x".to_string(), "z".repeat(300)];
    lista.extend(std::iter::repeat_n("y".to_string(), 50));
    let p = motor::validar_peticion(&json!({
        "v": 1, "ticket": ticket_ok(), "usuario": "m", "tam": 0,
        "nombres": lista, "n": 52, "de": "a\nb"
    }))
    .unwrap();
    assert_eq!(p.nombres[0], "bhola/bx");
    assert_eq!(p.nombres[1].chars().count(), 80);
    assert_eq!(p.nombres.len(), motor::MAX_NOMBRES);
    assert_eq!(p.de, "ab");
    assert!(motor::texto_peticion(&p).contains("y 51 más"));
}

#[test]
fn una_mac_lo_explica() {
    let e = motor::destino_de("04:42:1a:08:6b:62", 1770).unwrap_err();
    assert!(e.0.contains("MAC"), "{e}");
    assert!(motor::destino_de("", 1770).is_err());
    assert_eq!(
        motor::destino_de(" [fd7a::1] ", 5).unwrap(),
        ("fd7a::1".into(), 5)
    );
}

#[test]
fn tailscale() {
    let estado = json!({"Self": {"TailscaleIPs": ["100.1.1.1", "fd7a::1"]}, "Peer": {
        "a": {"HostName": "zeta", "Online": true, "OS": "linux", "TailscaleIPs": ["fd7a::2", "100.1.1.2"]},
        "b": {"HostName": "cel", "Online": true, "OS": "android", "TailscaleIPs": ["100.1.1.3"]},
        "c": {"HostName": "apagada", "Online": false, "OS": "linux", "TailscaleIPs": ["100.1.1.4"]},
        "d": {"HostName": "alfa", "Online": true, "OS": "linux", "TailscaleIPs": ["100.1.1.5"]}}});
    let compus = motor::compus_tailscale(&estado);
    let pares: Vec<(&str, &str)> = compus
        .iter()
        .map(|c| (c.nombre.as_str(), c.ip.as_str()))
        .collect();
    assert_eq!(pares, [("alfa", "100.1.1.5"), ("zeta", "100.1.1.2")]);
    assert_eq!(motor::mis_direcciones(&estado, None)[0].ip, "100.1.1.1");
    assert!(motor::compus_tailscale(&json!({})).is_empty());
}

#[test]
fn thunar_no_toca_lo_tuyo() {
    let e = Entorno::new();
    let uca = e.cfg.config.join("Thunar").join("uca.xml");
    std::fs::create_dir_all(uca.parent().unwrap()).unwrap();
    std::fs::write(
        &uca,
        "<actions><action><unique-id>mia</unique-id></action></actions>",
    )
    .unwrap();
    thunar::poner_accion(&e.cfg);
    thunar::poner_accion(&e.cfg);
    let xml = std::fs::read_to_string(&uca).unwrap();
    assert!(xml.contains("mia"));
    assert_eq!(xml.matches(thunar::ID_ACCION).count(), 1);
}

#[test]
fn humano_y_nombres_chicos() {
    assert_eq!(motor::humano(0), "0 B");
    assert_eq!(motor::humano(1536), "1.5 KB");
    assert_eq!(motor::humano(5 * 1024 * 1024 * 1024), "5.0 GB");
    assert_eq!(motor::limpiar_nombre("\n"), "archivo");
}

// --- Envíos ---

#[test]
fn un_archivo() {
    let e = Entorno::new();
    let r = e.origen(&["foto ñ.jpg"]);
    let (quedaron, cwd, _) = e.ida_y_vuelta(&r);
    assert_eq!(nombres(&quedaron), ["foto ñ.jpg"]);
    assert_eq!(
        std::fs::read_to_string(&quedaron[0]).unwrap(),
        "contenido de foto ñ.jpg"
    );
    assert!(!cwd.exists(), "no deja basura de sendme");
}

#[test]
fn una_carpeta() {
    let e = Entorno::new();
    let r = e.origen(&["album/a.jpg", "album/sub/b.jpg"]);
    let (quedaron, _, _) = e.ida_y_vuelta(&[r[0].parent().unwrap().to_path_buf()]);
    assert_eq!(quedaron[0].file_name().unwrap(), "album");
    assert!(quedaron[0].join("sub").join("b.jpg").exists());
}

#[test]
fn varios_llegan_sueltos_sin_el_lote() {
    let e = Entorno::new();
    let r = e.origen(&["a.txt", "b.txt"]);
    let (quedaron, _, lote) = e.ida_y_vuelta(&r);
    assert_eq!(nombres(&quedaron), ["a.txt", "b.txt"]);
    assert_eq!(e.recibidos(), ["a.txt", "b.txt"]);
    assert!(!lote.unwrap().exists());
    assert!(r[0].exists(), "el original sigue");
}

#[test]
fn varios_del_mismo_nombre() {
    let e = Entorno::new();
    let a = e.origen(&["foto.jpg"]);
    let b = e.origen(&["foto.jpg"]);
    let (quedaron, _, _) = e.ida_y_vuelta(&[a[0].clone(), b[0].clone()]);
    assert_eq!(nombres(&quedaron), ["foto (2).jpg", "foto.jpg"]);
}

#[test]
fn no_pisa_lo_que_tienes() {
    let e = Entorno::new();
    let a = e.origen(&["nota.txt"]);
    std::fs::create_dir_all(e.base().join("recibidos")).unwrap();
    std::fs::write(e.base().join("recibidos").join("nota.txt"), "mia").unwrap();
    let (quedaron, _, _) = e.ida_y_vuelta(&a);
    assert_eq!(quedaron[0].file_name().unwrap(), "nota (2).txt");
    assert_eq!(
        std::fs::read_to_string(e.base().join("recibidos").join("nota.txt")).unwrap(),
        "mia"
    );
}

#[test]
fn si_el_otro_se_apago_corta_y_limpia() {
    let e = Entorno::new();
    e.marcar("colgado", "");
    let err = motor::recibir(&e.cfg, &ticket_ok(), None, &|| false, 0).unwrap_err();
    assert!(err.0.contains("No llega nada"), "{err}");
    assert!(e.recibidos().is_empty());
    let ocultos = std::fs::read_dir(e.base().join("recibidos")).unwrap().count();
    assert_eq!(ocultos, 0, "no queda la carpeta temporal");
}

fn vivos(marca: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", &format!("sendme send {marca}")])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[test]
fn sendme_no_sobrevive_si_se_cae_quien_manda() {
    // Se lanza `vortex-shared-motor codigo` (que deja a sendme compartiendo), se le mata sin piedad y no
    // debe quedar nadie compartiendo el archivo.
    let e = Entorno::new();
    let r = e.origen(&["secreto-de-prueba.txt"]);
    let mut hijo = Command::new(env!("CARGO_BIN_EXE_vortex-shared-motor"))
        .args(["codigo", &r[0].to_string_lossy()])
        .env("PATH", e.cfg.sistema.path.clone().unwrap())
        .env("XDG_CACHE_HOME", e.base().join("cache"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut salida = hijo.stdout.take().unwrap();
    let marca = r[0].to_string_lossy().into_owned();
    let mut leido = String::new();
    let fin = Instant::now() + Duration::from_secs(20);
    while !leido.contains("blob") && Instant::now() < fin {
        let mut b = [0u8; 256];
        let n = salida.read(&mut b).unwrap_or(0);
        if n == 0 {
            break;
        }
        leido.push_str(&String::from_utf8_lossy(&b[..n]));
    }
    assert!(leido.contains("blob"), "sin código: {leido}");
    assert!(vivos(&marca));
    hijo.kill().unwrap();
    let _ = hijo.wait();
    drop(salida);
    let fin = Instant::now() + Duration::from_secs(5);
    while vivos(&marca) && Instant::now() < fin {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!vivos(&marca), "sigue compartiendo el archivo");
}

#[test]
fn codigo_malo() {
    let e = Entorno::new();
    let err = motor::recibir(&e.cfg, "--help", None, &|| false, 0).unwrap_err();
    assert!(err.0.contains("código"), "{err}");
}

#[test]
fn falta_el_archivo() {
    let e = Entorno::new();
    let err = Envio::new(&e.cfg, &[e.base().join("no-existe")]).err().unwrap();
    assert!(err.0.contains("No encuentro"), "{err}");
}

#[test]
fn sin_sendme_lo_dice() {
    let mut e = Entorno::new();
    e.cfg.sistema = Sistema::con_path(e.base().join("vacio"));
    let r = e.origen(&["a.txt"]);
    assert!(Envio::new(&e.cfg, &r).err().unwrap().0.contains("Falta sendme"));
    assert!(
        motor::recibir(&e.cfg, &ticket_ok(), None, &|| false, 0)
            .unwrap_err()
            .0
            .contains("Falta sendme")
    );
}

// --- De punta a punta, con el servicio escuchando de verdad ---

struct Punta {
    e: Entorno,
    ocupado: Arc<Mutex<()>>,
    hilo: Option<std::thread::JoinHandle<()>>,
    puerto: u16,
}

impl Punta {
    fn new() -> Self {
        let mut e = Entorno::new();
        let oyente = TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = oyente.local_addr().unwrap().port();
        e.cfg.puerto = puerto;
        let servicio = Servicio::new(oyente, e.cfg.clone());
        let ocupado = Arc::clone(&servicio.ocupado);
        let hilo = std::thread::spawn(move || servicio.correr());
        Punta {
            e,
            ocupado,
            hilo: Some(hilo),
            puerto,
        }
    }

    fn mandar(&self, rutas: &[PathBuf], usuario: &str) -> Result<Vec<String>, motor::ErrorShared> {
        let mut envio = Envio::new(&self.e.cfg, rutas)?;
        let mut estados: Vec<String> = Vec::new();
        let r = envio.preparar(Duration::from_secs(20)).and_then(|_| {
            motor::mandar(
                &self.e.cfg,
                "127.0.0.1",
                usuario,
                &envio,
                &mut |f, _| estados.push(f.to_string()),
                &|| false,
                Duration::from_secs(10),
            )
        });
        envio.terminar();
        r.map(|()| estados)
    }

    fn conectar(&self) -> TcpStream {
        let c = TcpStream::connect(("127.0.0.1", self.puerto)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        c
    }
}

fn texto_de(c: &mut TcpStream) -> String {
    let mut b = [0u8; 100];
    let n = c.read(&mut b).unwrap_or(0);
    String::from_utf8_lossy(&b[..n]).into_owned()
}

#[test]
fn aceptado() {
    let p = Punta::new();
    p.e.marcar("respuesta", "aceptar\n");
    let r = p.e.origen(&["uno.txt", "dos.txt"]);
    let estados = p.mandar(&r, "probador").unwrap();
    assert_eq!(p.e.recibidos(), ["dos.txt", "uno.txt"]);
    assert!(estados.iter().any(|s| s == "mandando"));
    let avisos = p.e.avisos().unwrap();
    assert!(avisos.contains("te quiere mandar archivos"));
    assert!(avisos.contains("uno.txt y 1 más"));
    assert!(avisos.contains("Desde 127.0.0.1"));
    assert!(avisos.contains("critical"));
}

#[test]
fn picarle_a_la_tarjeta_no_acepta() {
    let p = Punta::new();
    p.e.marcar("respuesta", "default\n");
    p.e.marcar("respuesta2", "rechazar\n");
    let r = p.e.origen(&["uno.txt"]);
    let err = p.mandar(&r, "probador").unwrap_err();
    assert!(err.0.contains("dijeron que no"), "{err}");
    assert!(p.e.recibidos().is_empty());
    assert_eq!(
        p.e.avisos().unwrap().matches("te quiere mandar").count(),
        2,
        "el aviso vuelve a salir"
    );
}

#[test]
fn rechazado_no_baja_nada() {
    let p = Punta::new();
    p.e.marcar("respuesta", "rechazar\n");
    let r = p.e.origen(&["uno.txt"]);
    let err = p.mandar(&r, "probador").unwrap_err();
    assert!(err.0.contains("dijeron que no"), "{err}");
    assert!(p.e.recibidos().is_empty());
}

#[test]
fn nadie_contesta_y_se_cierra_el_aviso() {
    let p = Punta::new();
    p.e.marcar("colgar-aviso", "");
    let r = p.e.origen(&["uno.txt"]);
    let err = p.mandar(&r, "probador").unwrap_err();
    assert!(err.0.contains("dijeron que no"), "{err}");
    assert!(p.e.avisos().unwrap().contains("cerrado"));
}

#[test]
fn otro_usuario() {
    let p = Punta::new();
    let r = p.e.origen(&["uno.txt"]);
    let err = p.mandar(&r, "alguien-que-no-existe").unwrap_err();
    assert!(err.0.contains("no tiene ese usuario"), "{err}");
    // ni siquiera sale el aviso de «te quiere mandar»
    assert!(p.e.avisos().is_none_or(|a| !a.contains("te quiere mandar")));
}

#[test]
fn jugando_no_molesta() {
    let p = Punta::new();
    let marca = &p.e.cfg.marca_juego;
    std::fs::create_dir_all(marca.parent().unwrap()).unwrap();
    std::fs::write(marca, "").unwrap();
    let r = p.e.origen(&["uno.txt"]);
    let err = p.mandar(&r, "probador").unwrap_err();
    assert!(err.0.contains("ocupada"), "{err}");
    assert!(p.e.avisos().is_none());
}

#[test]
fn uno_a_la_vez() {
    let p = Punta::new();
    let _tengo = p.ocupado.lock().unwrap();
    let r = p.e.origen(&["uno.txt"]);
    let err = p.mandar(&r, "probador").unwrap_err();
    assert!(err.0.contains("ocupada"), "{err}");
}

#[test]
fn basura_no_lo_tira() {
    let p = Punta::new();
    let mut c = p.conectar();
    c.write_all(b"esto no es json\n").unwrap();
    assert!(texto_de(&mut c).contains("error"));
    let mut c = p.conectar();
    let _ = c.write_all(&vec![b'x'; 200_000]);
    drop(c);
    let mut c = p.conectar();
    let mala = json!({"v": 1, "ticket": "--help", "usuario": "a", "tam": 0, "nombres": ["a"]});
    c.write_all(format!("{mala}\n").as_bytes()).unwrap();
    assert!(texto_de(&mut c).contains("error"));
    assert!(p.e.avisos().is_none());
}

#[test]
fn de_internet_ni_contesta() {
    let e = Entorno::new();
    let oyente = TcpListener::bind("127.0.0.1:0").unwrap();
    let servicio = Servicio::new(TcpListener::bind("127.0.0.1:0").unwrap(), e.cfg.clone());
    let mut cliente = TcpStream::connect(oyente.local_addr().unwrap()).unwrap();
    let (servidor, _) = oyente.accept().unwrap();
    servicio.atender(servidor, "8.8.8.8");
    cliente.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let mut b = [0u8; 10];
    assert_eq!(cliente.read(&mut b).unwrap(), 0);
}

#[test]
fn se_apaga_solo() {
    let mut p = Punta::new();
    let inicio = Instant::now();
    let hilo = p.hilo.take().unwrap();
    while !hilo.is_finished() && inicio.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(hilo.is_finished(), "no se queda corriendo");
}
