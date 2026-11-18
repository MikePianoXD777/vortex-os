//! `pruebas/test_motor.py`, portada: el motor con redes y WARP falsos. No toca tu wifi, tu WARP ni tu
//! internet: los programas (nmcli, warp-cli, systemctl…) son guiones falsos en una carpeta temporal que va
//! primero en el `PATH` que se les da, y las páginas de «¿hay internet?» son un servidor propio en 127.0.0.1.

use std::collections::{BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vortex_connect::motor::{
    Ajustes, Cancelar, Candado, Conexion, Connect, Probar, Situacion, probar_internet,
};
use vortex_connect::proteccion::{self, ErrorProteccion, Modo, Proteccion, Warp};
use vortex_connect::redes::{self, Actual, Ejecutor, ErrorRed, Red, Redes, RedesLinux, RedesWindows};
use vortex_connect::sistema::{Salida, Sistema};

// --- Falsos ---

#[derive(Default)]
struct RedesFalsas {
    actual: Mutex<Actual>,
    lista: Vec<Red>,
    fallan: BTreeSet<String>,
    entradas: Mutex<Vec<String>>,
}

impl RedesFalsas {
    fn con(actual: Actual, lista: Vec<Red>, fallan: &[&str]) -> Arc<Self> {
        Arc::new(RedesFalsas {
            actual: Mutex::new(actual),
            lista,
            fallan: fallan.iter().map(|s| s.to_string()).collect(),
            entradas: Mutex::default(),
        })
    }

    fn vacia() -> Arc<Self> {
        Self::con(Actual::default(), vec![], &[])
    }

    fn entradas(&self) -> Vec<String> {
        self.entradas.lock().unwrap().clone()
    }
}

impl Redes for RedesFalsas {
    fn actual(&self) -> Actual {
        self.actual.lock().unwrap().clone()
    }

    fn lista(&self, _buscar: bool) -> Result<Vec<Red>, ErrorRed> {
        Ok(self.lista.clone())
    }

    fn hay_wifi(&self) -> bool {
        true
    }

    fn conectar(&self, ssid: &str) -> Result<(), ErrorRed> {
        self.entradas.lock().unwrap().push(ssid.to_string());
        if self.fallan.contains(ssid) {
            return Err(ErrorRed(format!("No se pudo entrar a «{ssid}».")));
        }
        *self.actual.lock().unwrap() = Actual::wifi(ssid, true);
        Ok(())
    }
}

struct WarpFalso {
    cifra: bool,
    dns: bool,
    blinda: bool,
    prendido: Mutex<Modo>,
    llamadas: Mutex<Vec<String>>,
    blindajes: AtomicUsize,
    consultas_conectado: AtomicUsize,
}

impl WarpFalso {
    fn nuevo(cifra: bool, dns: bool, prendido: Modo, blinda: bool) -> Arc<Self> {
        Arc::new(WarpFalso {
            cifra,
            dns,
            blinda,
            prendido: Mutex::new(prendido),
            llamadas: Mutex::default(),
            blindajes: AtomicUsize::new(0),
            consultas_conectado: AtomicUsize::new(0),
        })
    }

    fn normal() -> Arc<Self> {
        Self::nuevo(true, true, Modo::Ninguno, true)
    }

    fn llamadas(&self) -> Vec<String> {
        self.llamadas.lock().unwrap().clone()
    }

    fn blindajes(&self) -> usize {
        self.blindajes.load(Ordering::SeqCst)
    }

    fn anotar(&self, l: &str) {
        self.llamadas.lock().unwrap().push(l.to_string());
    }
}

impl Proteccion for WarpFalso {
    fn instalado(&self) -> bool {
        true
    }

    fn conectado(&self) -> bool {
        self.consultas_conectado.fetch_add(1, Ordering::SeqCst);
        *self.prendido.lock().unwrap() != Modo::Ninguno
    }

    fn modo(&self) -> Modo {
        *self.prendido.lock().unwrap()
    }

    fn preparar(&self) -> Result<(), ErrorProteccion> {
        self.anotar("preparar");
        Ok(())
    }

    fn activar_cifrado(&self) -> bool {
        self.anotar("cifrado");
        if self.cifra {
            *self.prendido.lock().unwrap() = Modo::Cifrado;
        }
        self.cifra
    }

    fn activar_solo_dns(&self) -> bool {
        self.anotar("dns");
        if self.dns {
            *self.prendido.lock().unwrap() = Modo::Dns;
        }
        self.dns
    }

    fn blindar_dns(&self) -> bool {
        self.blindajes.fetch_add(1, Ordering::SeqCst);
        self.blinda
    }

    fn apagar(&self) {
        self.anotar("apagar");
        *self.prendido.lock().unwrap() = Modo::Ninguno;
    }
}

/// Cada llamada da la siguiente respuesta; la última se repite.
fn respuestas(lista: &[(Conexion, &str)]) -> Probar {
    let pendientes: Mutex<VecDeque<(Conexion, String)>> = Mutex::new(
        lista
            .iter()
            .map(|(c, u)| (*c, u.to_string()))
            .collect::<VecDeque<_>>(),
    );
    Box::new(move || {
        let mut p = pendientes.lock().unwrap();
        if p.len() > 1 {
            p.pop_front().unwrap()
        } else {
            p[0].clone()
        }
    })
}

const INTERNET: (Conexion, &str) = (Conexion::Internet, "");
const PORTAL: (Conexion, &str) = (Conexion::Portal, "http://portal.test/");
const NADA: (Conexion, &str) = (Conexion::Nada, "");

/// Un `Connect` con falsos, ajustes en una carpeta temporal y lo que dijo.
struct Banco {
    c: Connect,
    dichos: Arc<Mutex<Vec<String>>>,
    portales: Arc<Mutex<Vec<String>>>,
    _t: tempfile::TempDir,
}

impl Banco {
    fn dichos(&self) -> Vec<String> {
        self.dichos.lock().unwrap().clone()
    }

    fn portales(&self) -> Vec<String> {
        self.portales.lock().unwrap().clone()
    }
}

fn connect(
    redes: Arc<RedesFalsas>,
    warp: Arc<WarpFalso>,
    probar: Probar,
    condiciones: bool,
    protegio: bool,
) -> Banco {
    let t = tempfile::tempdir().unwrap();
    let c = Connect::nuevo(
        Box::new(redes),
        Box::new(warp),
        probar,
        Box::new(|_| {}),
        Ajustes {
            condiciones,
            protegio_connect: protegio,
            ..Ajustes::default()
        },
    )
    .con_ruta_ajustes(t.path().join("connect.json"))
    .con_espera_portal(Duration::from_secs(5));
    let dichos: Arc<Mutex<Vec<String>>> = Arc::default();
    let portales: Arc<Mutex<Vec<String>>> = Arc::default();
    let d = dichos.clone();
    c.poner_avisar(Arc::new(move |t| d.lock().unwrap().push(t.to_string())));
    let p = portales.clone();
    c.poner_abrir_portal(Arc::new(move |u| p.lock().unwrap().push(u.to_string())));
    Banco {
        c,
        dichos,
        portales,
        _t: t,
    }
}

fn normal(redes: Arc<RedesFalsas>, warp: Arc<WarpFalso>) -> Banco {
    connect(redes, warp, respuestas(&[INTERNET]), true, false)
}

// --- Flujo ---

#[test]
fn entra_y_cifra() {
    let (r, w) = (RedesFalsas::vacia(), WarpFalso::normal());
    let b = normal(r.clone(), w);
    let e = b.c.conectar("Cafe", &Cancelar::new()).unwrap();
    assert_eq!(r.entradas(), ["Cafe"]);
    assert_eq!(e.situacion, Situacion::Protegido);
    assert!(b.portales().is_empty());
}

#[test]
fn con_portal_abre_espera_y_luego_cifra() {
    let b = connect(
        RedesFalsas::vacia(),
        WarpFalso::normal(),
        respuestas(&[PORTAL, PORTAL, INTERNET]),
        true,
        false,
    );
    let e = b.c.conectar("Aeropuerto", &Cancelar::new()).unwrap();
    assert_eq!(b.portales(), ["http://portal.test/"]);
    assert!(b.dichos().iter().any(|d| d.contains("no va cifrado")));
    assert_eq!(e.situacion, Situacion::Protegido);
}

#[test]
fn cancelar_el_portal_no_cifra() {
    let w = WarpFalso::normal();
    let b = connect(
        RedesFalsas::vacia(),
        w.clone(),
        respuestas(&[PORTAL]),
        true,
        false,
    );
    let cancelar = Cancelar::new();
    cancelar.cancelar();
    let e = b.c.conectar("Aeropuerto", &cancelar).unwrap();
    assert!(w.llamadas().is_empty());
    assert_eq!(e.situacion, Situacion::Portal);
}

#[test]
fn sin_internet_lo_dice_y_no_cifra() {
    let w = WarpFalso::normal();
    let b = connect(RedesFalsas::vacia(), w.clone(), respuestas(&[NADA]), true, false);
    let e = b.c.conectar("Rota", &Cancelar::new()).unwrap();
    assert!(w.llamadas().is_empty());
    assert_eq!(e.situacion, Situacion::SinRed);
    assert!(b.dichos().iter().any(|d| d.contains("no da internet")));
}

#[test]
fn red_que_bloquea_cae_a_dns() {
    let w = WarpFalso::nuevo(false, true, Modo::Ninguno, true);
    let b = normal(RedesFalsas::vacia(), w.clone());
    let e = b.c.conectar("Hotel", &Cancelar::new()).unwrap();
    assert_eq!(w.llamadas(), ["preparar", "cifrado", "dns"]);
    assert_eq!(e.situacion, Situacion::SoloDns);
}

#[test]
fn al_cifrar_le_quita_al_wifi_su_dns() {
    let w = WarpFalso::normal();
    let b = normal(RedesFalsas::vacia(), w.clone());
    b.c.conectar("Hotel", &Cancelar::new()).unwrap();
    assert_eq!(w.blindajes(), 1);
    assert!(b.dichos().iter().any(|d| d.starts_with("Protegido")));
}

#[test]
fn si_el_dns_sigue_expuesto_lo_dice() {
    let w = WarpFalso::nuevo(true, true, Modo::Ninguno, false);
    let b = normal(RedesFalsas::vacia(), w);
    let e = b.c.conectar("Hotel", &Cancelar::new()).unwrap();
    assert_eq!(e.situacion, Situacion::Protegido);
    assert!(!b.dichos().iter().any(|d| d.starts_with("Protegido")));
    assert!(b.dichos().iter().any(|d| d.contains("nombres de los sitios")));
}

#[test]
fn solo_dns_sin_blindar_no_cuenta_como_protegido() {
    // En este modo los nombres son lo único que se cifra: si siguen saliendo en claro, no hay protección.
    let w = WarpFalso::nuevo(false, true, Modo::Ninguno, false);
    let b = normal(RedesFalsas::vacia(), w.clone());
    let e = b.c.conectar("Hotel", &Cancelar::new()).unwrap();
    assert_eq!(e.situacion, Situacion::SinProteccion);
    assert!(w.llamadas().contains(&"apagar".to_string()));
}

#[test]
fn si_nada_funciona_lo_dice_y_no_finge() {
    let w = WarpFalso::nuevo(false, false, Modo::Ninguno, true);
    let b = normal(RedesFalsas::vacia(), w);
    let e = b.c.conectar("Hotel", &Cancelar::new()).unwrap();
    assert_eq!(e.situacion, Situacion::SinProteccion);
    assert!(b.dichos().iter().any(|d| d.contains("sin protección")));
}

#[test]
fn sin_aceptar_condiciones_warp_ni_se_toca() {
    let w = WarpFalso::normal();
    let b = connect(
        RedesFalsas::vacia(),
        w.clone(),
        respuestas(&[INTERNET]),
        false,
        false,
    );
    let e = b.c.conectar("Cafe", &Cancelar::new()).unwrap();
    assert!(w.llamadas().is_empty());
    assert_ne!(e.situacion, Situacion::Protegido);
}

#[test]
fn sin_aceptar_ni_se_pregunta_si_esta_prendido() {
    let w = WarpFalso::nuevo(true, true, Modo::Cifrado, true);
    let b = connect(
        RedesFalsas::vacia(),
        w.clone(),
        respuestas(&[INTERNET]),
        false,
        false,
    );
    b.c.conectar("Cafe", &Cancelar::new()).unwrap();
    let b2 = connect(
        RedesFalsas::vacia(),
        w.clone(),
        respuestas(&[INTERNET]),
        false,
        false,
    );
    b2.c.revisar(&Cancelar::new());
    assert_eq!(w.consultas_conectado.load(Ordering::SeqCst), 0);
    assert!(w.llamadas().is_empty());
}

#[test]
fn pausa_el_tunel_antes_de_entrar() {
    let w = WarpFalso::nuevo(true, true, Modo::Cifrado, true);
    let b = normal(RedesFalsas::vacia(), w.clone());
    b.c.conectar("Cafe", &Cancelar::new()).unwrap();
    assert_eq!(w.llamadas()[0], "apagar");
}

#[test]
fn no_poder_entrar_sube_como_error() {
    let r = RedesFalsas::con(Actual::default(), vec![], &["Lejos"]);
    let b = normal(r, WarpFalso::normal());
    assert!(b.c.conectar("Lejos", &Cancelar::new()).is_err());
}

// --- Estado ---

#[test]
fn estados() {
    let abierta = Actual::wifi("Cafe", true);
    let cerrada = Actual::wifi("Casa", false);
    let casos = [
        (abierta.clone(), Modo::Ninguno, INTERNET, Situacion::SinProteccion),
        (abierta.clone(), Modo::Cifrado, INTERNET, Situacion::Protegido),
        (abierta.clone(), Modo::Dns, INTERNET, Situacion::SoloDns),
        (abierta.clone(), Modo::Ninguno, PORTAL, Situacion::Portal),
        (abierta, Modo::Ninguno, NADA, Situacion::SinRed),
        (cerrada, Modo::Ninguno, INTERNET, Situacion::RedDeConfianza),
        (
            Actual::cable(),
            Modo::Ninguno,
            INTERNET,
            Situacion::RedDeConfianza,
        ),
        (Actual::default(), Modo::Ninguno, NADA, Situacion::SinRed),
    ];
    for (red, modo, internet, esperado) in casos {
        let b = connect(
            RedesFalsas::con(red.clone(), vec![], &[]),
            WarpFalso::nuevo(true, true, modo, true),
            respuestas(&[internet]),
            true,
            false,
        );
        assert_eq!(b.c.estado().situacion, esperado, "{red:?} {modo:?}");
    }
}

#[test]
fn el_portal_va_en_el_estado() {
    let b = connect(
        RedesFalsas::con(Actual::wifi("Cafe", true), vec![], &[]),
        WarpFalso::normal(),
        respuestas(&[PORTAL]),
        true,
        false,
    );
    assert_eq!(b.c.estado().portal, "http://portal.test/");
}

// --- Modo viaje ---

#[test]
fn sin_red_entra_a_la_mas_fuerte() {
    let r = RedesFalsas::con(
        Actual::default(),
        vec![Red::new("Fuerte", 90), Red::new("Debil", 20)],
        &[],
    );
    let b = normal(r.clone(), WarpFalso::normal());
    let e = b.c.revisar(&Cancelar::new()).unwrap();
    assert_eq!(r.entradas(), ["Fuerte"]);
    assert_eq!(e.situacion, Situacion::Protegido);
}

#[test]
fn si_falla_prueba_la_siguiente_y_no_insiste() {
    let r = RedesFalsas::con(
        Actual::default(),
        vec![Red::new("Fuerte", 90), Red::new("Debil", 20)],
        &["Fuerte"],
    );
    let b = normal(r.clone(), WarpFalso::normal());
    b.c.revisar(&Cancelar::new());
    assert_eq!(r.entradas(), ["Fuerte", "Debil"]);
    *r.actual.lock().unwrap() = Actual::default();
    r.entradas.lock().unwrap().clear();
    b.c.revisar(&Cancelar::new());
    assert_eq!(r.entradas(), ["Debil"]);
}

#[test]
fn sin_redes_no_hace_nada() {
    let b = normal(RedesFalsas::vacia(), WarpFalso::normal());
    assert!(b.c.revisar(&Cancelar::new()).is_none());
}

#[test]
fn red_abierta_sin_cifrar_se_cifra() {
    let b = normal(
        RedesFalsas::con(Actual::wifi("Cafe", true), vec![], &[]),
        WarpFalso::normal(),
    );
    let e = b.c.revisar(&Cancelar::new()).unwrap();
    assert_eq!(e.situacion, Situacion::Protegido);
}

#[test]
fn ya_cifrada_no_hace_nada() {
    let w = WarpFalso::nuevo(true, true, Modo::Cifrado, true);
    let b = normal(
        RedesFalsas::con(Actual::wifi("Cafe", true), vec![], &[]),
        w.clone(),
    );
    assert!(b.c.revisar(&Cancelar::new()).is_none());
    assert!(w.llamadas().is_empty());
}

#[test]
fn tu_red_con_contrasena_no_se_toca() {
    let r = RedesFalsas::con(Actual::wifi("Casa", false), vec![Red::new("Cafe", 99)], &[]);
    let w = WarpFalso::normal();
    let b = normal(r.clone(), w.clone());
    assert!(b.c.revisar(&Cancelar::new()).is_none());
    assert!(r.entradas().is_empty());
    assert!(w.llamadas().is_empty());
}

#[test]
fn con_cable_no_busca_wifi() {
    let r = RedesFalsas::con(Actual::cable(), vec![Red::new("Cafe", 99)], &[]);
    let b = normal(r.clone(), WarpFalso::normal());
    assert!(b.c.revisar(&Cancelar::new()).is_none());
    assert!(r.entradas().is_empty());
}

#[test]
fn de_vuelta_en_casa_apaga_lo_que_prendio() {
    let w = WarpFalso::nuevo(true, true, Modo::Cifrado, true);
    let b = connect(
        RedesFalsas::con(Actual::wifi("Casa", false), vec![], &[]),
        w.clone(),
        respuestas(&[INTERNET]),
        true,
        true,
    );
    b.c.revisar(&Cancelar::new());
    assert!(w.llamadas().contains(&"apagar".to_string()));
}

#[test]
fn en_casa_no_apaga_lo_que_prendiste_tu() {
    let w = WarpFalso::nuevo(true, true, Modo::Cifrado, true);
    let b = normal(
        RedesFalsas::con(Actual::wifi("Casa", false), vec![], &[]),
        w.clone(),
    );
    assert!(b.c.revisar(&Cancelar::new()).is_none());
    assert!(w.llamadas().is_empty());
}

#[test]
fn si_el_wifi_se_reconecto_vuelve_a_quitarle_su_dns() {
    let w = WarpFalso::nuevo(true, true, Modo::Cifrado, true);
    let b = normal(
        RedesFalsas::con(Actual::wifi("Cafe", true), vec![], &[]),
        w.clone(),
    );
    assert!(b.c.revisar(&Cancelar::new()).is_none());
    assert_eq!(w.blindajes(), 1);
}

// --- Ajustes y candado ---

#[test]
fn ajustes_ida_y_vuelta() {
    let t = tempfile::tempdir().unwrap();
    let ruta = t.path().join("sub/connect.json");
    Ajustes {
        condiciones: true,
        modo_viaje: true,
        protegio_connect: false,
    }
    .guardar_en(&ruta);
    let a = Ajustes::leer_de(&ruta);
    assert!(a.condiciones && a.modo_viaje);
    assert!(!a.protegio_connect);
    // el mismo formato que escribe la de Python
    let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
    assert_eq!(
        j,
        serde_json::json!({"condiciones": true, "modo_viaje": true, "protegio_connect": false})
    );
}

#[test]
fn archivo_roto_o_con_tipos_raros_no_acepta_nada() {
    let t = tempfile::tempdir().unwrap();
    let ruta = t.path().join("connect.json");
    assert_eq!(Ajustes::leer_de(&ruta), Ajustes::default(), "sin archivo");
    for texto in ["{roto", "[]", r#"{"condiciones": "si", "modo_viaje": 1}"#] {
        std::fs::write(&ruta, texto).unwrap();
        let a = Ajustes::leer_de(&ruta);
        assert!(!a.condiciones && !a.modo_viaje, "{texto}");
    }
}

#[test]
fn candado_es_de_uno_en_uno() {
    let t = tempfile::tempdir().unwrap();
    let ruta = t.path().join("connect.candado");
    let (mut a, mut b) = (Candado::en(ruta.clone()), Candado::en(ruta));
    assert!(a.tomar());
    assert!(!b.tomar());
    a.soltar();
    // Otra prueba de este mismo programa puede estar lanzando procesos: entre su fork y su exec, el hijo
    // tiene una copia del archivo de «a» (con el candado) por un instante. Se reintenta un momento.
    let tomado = (0..100).any(|_| {
        b.tomar() || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            false
        }
    });
    assert!(tomado);
    b.soltar();
}

#[test]
fn cambiar_ajustes_guarda_y_marcar_solo_si_cambia() {
    let w = WarpFalso::normal();
    let b = normal(RedesFalsas::vacia(), w);
    b.c.conectar("Cafe", &Cancelar::new()).unwrap();
    assert!(Ajustes::leer_de(b.c.ruta_ajustes()).protegio_connect);
    b.c.quitar();
    assert!(!Ajustes::leer_de(b.c.ruta_ajustes()).protegio_connect);
    b.c.cambiar_ajustes(|a| a.modo_viaje = true);
    assert!(Ajustes::leer_de(b.c.ruta_ajustes()).modo_viaje);
}

// --- ¿Hay internet? (con un servidor propio en 127.0.0.1) ---

fn servidor() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut b = [0u8; 2048];
                let n = s.read(&mut b).unwrap_or(0);
                let pedido = String::from_utf8_lossy(&b[..n]);
                let ruta = pedido.split_whitespace().nth(1).unwrap_or("/").to_string();
                let r = match ruta.as_str() {
                    "/204" => "HTTP/1.0 204 No Content\r\n\r\n",
                    "/portal" => "HTTP/1.0 302 Found\r\nLocation: http://portal.test/entra\r\n\r\n",
                    "/raro" => "HTTP/1.0 302 Found\r\nLocation: javascript:alert(1)\r\n\r\n",
                    _ => "HTTP/1.0 200 OK\r\n\r\n<html>portal</html>",
                };
                let _ = s.write_all(r.as_bytes());
            });
        }
    });
    base
}

fn probar(base: &str, ruta: &str) -> (Conexion, String) {
    probar_internet(&Sistema::real(), &format!("{base}{ruta}"), Duration::from_secs(2))
}

#[test]
fn probar_internet_casos() {
    let base = servidor();
    assert_eq!(
        probar(&base, "/204"),
        (Conexion::Internet, String::new()),
        "204 es internet"
    );
    assert_eq!(
        probar(&base, "/portal"),
        (Conexion::Portal, "http://portal.test/entra".into()),
        "una redirección es portal y da su página"
    );
    assert_eq!(
        probar(&base, "/otra").0,
        Conexion::Portal,
        "una página en vez de 204 es portal"
    );
    assert_eq!(
        probar(&base, "/raro"),
        (Conexion::Portal, format!("{base}/raro")),
        "un portal no te manda a cualquier esquema"
    );
    assert_eq!(
        probar_internet(&Sistema::real(), "http://127.0.0.1:9/", Duration::from_secs(1)),
        (Conexion::Nada, String::new()),
        "sin red es nada"
    );
}

// --- nmcli (con un ejecutor falso) ---

const LISTA_NMCLI: &str =
    "*:Cafe\\: Centro:70:\n :Cafe\\: Centro:90:\n :Aeropuerto:40:--\n :Casa:99:WPA2\n :Vieja:80:WEP\n ::50:";

type Registro = Arc<Mutex<Vec<Vec<String>>>>;

fn correr_falso(respuestas: &[(&str, i32, &str, &str)], registro: &Registro) -> Ejecutor {
    let respuestas: Vec<(String, Salida)> = respuestas
        .iter()
        .map(|(k, c, o, e)| {
            (
                k.to_string(),
                Salida {
                    codigo: *c,
                    salida: o.to_string(),
                    error: e.to_string(),
                },
            )
        })
        .collect();
    let registro = registro.clone();
    Box::new(move |orden, _| {
        registro
            .lock()
            .unwrap()
            .push(orden.iter().map(|s| s.to_string()).collect());
        let texto = orden.join(" ");
        respuestas
            .iter()
            .find(|(k, _)| texto.contains(k.as_str()))
            .map_or_else(Salida::default, |(_, s)| s.clone())
    })
}

fn registro() -> Registro {
    Arc::default()
}

#[test]
fn partir_nmcli() {
    assert_eq!(redes::partir_nmcli("a\\:b:c\\\\d::"), ["a:b", "c\\d", "", ""]);
}

#[test]
fn lista_solo_abiertas_sin_repetir_y_las_fuertes_primero() {
    let r = RedesLinux::con(correr_falso(&[("wifi list", 0, LISTA_NMCLI, "")], &registro()));
    assert_eq!(
        r.lista(true).unwrap(),
        [
            Red {
                ssid: "Cafe: Centro".into(),
                senal: 90,
                conectada: true
            },
            Red::new("Aeropuerto", 40)
        ]
    );
}

#[test]
fn actual_de_linux() {
    let lista = LISTA_NMCLI;
    let casos = [
        ("ethernet:connected\nwifi:connected\n", lista, Actual::cable()),
        ("wifi:connected\n", lista, Actual::wifi("Cafe: Centro", true)),
        (
            "wifi:connected\n",
            " :Cafe:70:\n*:Casa:99:WPA2",
            Actual::wifi("Casa", false),
        ),
        ("wifi:disconnected\n", lista, Actual::default()),
    ];
    for (dispositivos, filas, esperado) in casos {
        let r = RedesLinux::con(correr_falso(
            &[("TYPE,STATE", 0, dispositivos, ""), ("wifi list", 0, filas, "")],
            &registro(),
        ));
        assert_eq!(r.actual(), esperado, "{dispositivos}");
    }
}

#[test]
fn conectar_con_mac_al_azar_y_sin_volver_solo() {
    let reg = registro();
    RedesLinux::con(correr_falso(&[], &reg))
        .conectar("Cafe; rm -rf")
        .unwrap();
    let reg = reg.lock().unwrap();
    let alta = reg.iter().find(|o| o.contains(&"add".to_string())).unwrap();
    assert!(
        alta.contains(&"Cafe; rm -rf".to_string()),
        "un solo argumento: nunca pasa por un shell"
    );
    let despues = |k: &str| alta[alta.iter().position(|x| x == k).unwrap() + 1].clone();
    assert_eq!(despues("connection.autoconnect"), "no");
    assert_eq!(despues("wifi.cloned-mac-address"), "random");
    assert!(!alta.contains(&"wifi-sec.key-mgmt".to_string()));
    assert!(despues("connection.permissions").starts_with("user:"));
    assert_eq!(
        reg.last().unwrap().last().unwrap(),
        "Vortexine Connect: Cafe; rm -rf"
    );
}

#[test]
fn reutiliza_el_perfil_que_ya_existe() {
    let reg = registro();
    RedesLinux::con(correr_falso(
        &[("NAME", 0, "Vortexine Connect\\: Cafe\n", "")],
        &reg,
    ))
    .conectar("Cafe")
    .unwrap();
    assert!(!reg.lock().unwrap().iter().any(|o| o.contains(&"add".to_string())));
}

#[test]
fn si_no_entra_borra_el_perfil_y_lo_dice() {
    let reg = registro();
    let e = RedesLinux::con(correr_falso(&[("connection up", 4, "", "no")], &reg))
        .conectar("Lejos")
        .unwrap_err();
    assert!(e.0.contains("Lejos"));
    let reg = reg.lock().unwrap();
    assert_eq!(reg.last().unwrap()[1..3], ["connection", "delete"]);
}

#[test]
fn si_no_puede_preparar_el_perfil_da_el_motivo() {
    let e = RedesLinux::con(correr_falso(
        &[("connection add", 1, "", "Error: raro\n")],
        &registro(),
    ))
    .conectar("Cafe")
    .unwrap_err();
    assert_eq!(e.0, "No pude preparar «Cafe»: Error: raro");
}

#[test]
fn nmcli_que_no_esta_no_rompe() {
    let r = RedesLinux::con(Box::new(|_, _| Salida {
        codigo: 127,
        ..Salida::default()
    }));
    assert!(r.lista(true).unwrap().is_empty());
    assert_eq!(r.actual(), Actual::default());
    assert!(!r.hay_wifi());
}

// --- netsh (Windows) ---

const NETSH_ES: &str = "
Nombre de interfaz : Wi-Fi
Hay 3 redes visibles actualmente.

SSID 1 : Cafe Centro
    Tipo de red             : Infraestructura
    Autenticación           : Abierta
    Cifrado                 : Ninguno
    BSSID 1                 : aa:bb:cc:dd:ee:01
         Señal              : 80%
    BSSID 2                 : aa:bb:cc:dd:ee:02
         Señal              : 35%

SSID 2 : Casa
    Tipo de red             : Infraestructura
    Autenticación           : WPA2-Personal
    Cifrado                 : CCMP
    BSSID 1                 : aa:bb:cc:dd:ee:03
         Señal              : 99%

SSID 3 : Con portal
    Tipo de red             : Infraestructura
    Autenticación           : Abierta
    Cifrado                 : Ninguno
    BSSID 1                 : aa:bb:cc:dd:ee:04
         Señal              : 60%
";

const NETSH_EN: &str = "
SSID 1 : Airport
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : None
    BSSID 1                 : aa:bb:cc:dd:ee:01
         Signal             : 50%

SSID 2 : Enhanced
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : WEP
    BSSID 1                 : aa:bb:cc:dd:ee:02
         Signal             : 90%
";

#[test]
fn netsh_en_espanol() {
    let r = RedesWindows::con(correr_falso(&[("show networks", 0, NETSH_ES, "")], &registro()));
    assert_eq!(
        r.lista(true).unwrap(),
        [Red::new("Cafe Centro", 80), Red::new("Con portal", 60)]
    );
}

#[test]
fn netsh_en_ingles_sin_las_que_cifran() {
    let r = RedesWindows::con(correr_falso(&[("show networks", 0, NETSH_EN, "")], &registro()));
    assert_eq!(r.lista(true).unwrap(), [Red::new("Airport", 50)]);
}

#[test]
fn netsh_pide_ubicacion() {
    let r = RedesWindows::con(correr_falso(
        &[("show networks", 1, "La ubicación está desactivada", "")],
        &registro(),
    ));
    assert!(r.lista(true).unwrap_err().0.contains("Ubicación"));
}

#[test]
fn netsh_actual() {
    let interfaz = "Nombre : Wi-Fi\n    SSID                   : Cafe\n    BSSID                  : aa\n    Autenticación          : Abierta\n";
    let r = RedesWindows::con(correr_falso(&[("show interfaces", 0, interfaz, "")], &registro()));
    assert_eq!(r.actual(), Actual::wifi("Cafe", true));
    assert!(r.hay_wifi());
}

#[test]
fn cable_solo_con_adaptador_fisico() {
    let r = RedesWindows::con(correr_falso(&[("Get-NetAdapter", 0, "1\r\n", "")], &registro()));
    assert_eq!(r.actual(), Actual::cable());
    let r = RedesWindows::con(correr_falso(&[("Get-NetAdapter", 0, "0\r\n", "")], &registro()));
    assert_eq!(r.actual(), Actual::default());
}

#[test]
fn perfil_escapa_el_nombre_y_no_entra_solo() {
    let perfil = redes::perfil_windows("A&B <c>");
    assert!(perfil.contains("A&amp;B &lt;c&gt;"));
    assert!(perfil.contains("<connectionMode>manual</connectionMode>"));
    assert!(perfil.contains("<authentication>open</authentication>"));
}

#[test]
fn netsh_conectar_borra_el_perfil_temporal() {
    let reg = registro();
    RedesWindows::con(correr_falso(&[], &reg))
        .conectar("Cafe")
        .unwrap();
    let reg = reg.lock().unwrap();
    let i = reg[0].iter().position(|x| x == "add").unwrap();
    let archivo = reg[0][i + 2].split_once('=').unwrap().1.to_string();
    assert!(!Path::new(&archivo).exists());
    assert!(reg[1].contains(&"name=Cafe".to_string()));
}

// --- warp-cli y compañía (guiones falsos en un PATH temporal) ---

/// Los guiones falsos escriben en la misma carpeta; las pruebas que los usan van de una en una (escribir y
/// ejecutar un guion a la vez desde varios hilos da «Text file busy»).
static UNA_A_LA_VEZ: Mutex<()> = Mutex::new(());

struct Banca {
    d: tempfile::TempDir,
    sistema: Sistema,
    _a_la_vez: std::sync::MutexGuard<'static, ()>,
}

fn guion(d: &Path, nombre: &str, cuerpo: &str) {
    let r = d.join(nombre);
    std::fs::write(&r, cuerpo).unwrap();
    std::fs::set_permissions(&r, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Banca {
    fn nueva() -> Self {
        let a_la_vez = UNA_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let ruta = |n: &str| p.join(n).to_string_lossy().into_owned();
        guion(
            p,
            "warp-cli",
            &format!(
                r#"#!/bin/sh
echo "$*" >> "{reg}"
case "$*" in
  *status*) cat "{estado}" ;;
  *"registration show"*) if [ -f "{registrado}" ]; then echo "Account"; else echo "Missing registration"; exit 1; fi ;;
  *"registration new"*) : > "{registrado}"; echo Success ;;
  *" connect"*) echo "Status update: Connected" > "{estado}"; echo Success ;;
  *disconnect*) echo "Status update: Disconnected" > "{estado}"; echo Success ;;
  *"settings list"*) cat "{ajustes}" 2>/dev/null ;;
  *) echo Success ;;
esac
"#,
                reg = ruta("reg"),
                estado = ruta("estado"),
                registrado = ruta("registrado"),
                ajustes = ruta("ajustes"),
            ),
        );
        guion(
            p,
            "systemctl",
            &format!(
                r#"#!/bin/sh
echo "$*" >> "{reg}"
case "$*" in
  *is-active*) [ -f "{svc}" ] ;;
  *start*) : > "{svc}" ;;
  *stop*) rm -f "{svc}" ;;
esac
"#,
                reg = ruta("reg-svc"),
                svc = ruta("svc"),
            ),
        );
        guion(
            p,
            "ip",
            r#"#!/bin/sh
case "$1" in
  -4) echo "default via 10.77.0.1 dev wlan0 proto dhcp src 10.77.0.34 metric 600" ;;
  -6) echo "default via fe80::1 dev wlan0 proto ra metric 600 pref medium" ;;
esac
"#,
        );
        guion(
            p,
            "resolvectl",
            &format!(
                "#!/bin/sh\necho \"$*\" >> \"{}\"\n[ ! -f \"{}\" ]\n",
                ruta("reg-dns"),
                ruta("dns-falla")
            ),
        );
        std::fs::write(p.join("estado"), "Status update: Disconnected\n").unwrap();
        // /usr/bin y /bin para que los guiones encuentren cat, echo, rm…; ningún nmcli ni warp-cli de verdad
        // está ahí antes que los falsos.
        let sistema = Sistema::con_path(format!("{}:/usr/bin:/bin", p.display()));
        Banca {
            d,
            sistema,
            _a_la_vez: a_la_vez,
        }
    }

    fn leer(&self, nombre: &str) -> String {
        std::fs::read_to_string(self.d.path().join(nombre)).unwrap_or_default()
    }

    fn ordenes(&self) -> Vec<String> {
        self.leer("reg").lines().map(String::from).collect()
    }

    fn warp(&self, traza: impl Fn() -> bool + Send + Sync + 'static) -> Warp {
        Warp::con(self.sistema.clone(), Box::new(traza), Box::new(|_| {}))
    }
}

/// Una `traza` que da lo que se le ponga en la cola y, cuando se acaba, lo último.
fn traza_en_cola(vals: &[bool]) -> (Arc<Mutex<VecDeque<bool>>>, impl Fn() -> bool + Send + Sync) {
    let cola: Arc<Mutex<VecDeque<bool>>> = Arc::new(Mutex::new(vals.iter().copied().collect()));
    let c = cola.clone();
    let ultimo = Mutex::new(false);
    (cola, move || {
        let mut c = c.lock().unwrap();
        let mut u = ultimo.lock().unwrap();
        if let Some(v) = c.pop_front() {
            *u = v;
        }
        *u
    })
}

#[test]
fn preguntar_en_reposo_no_prende_el_servicio() {
    let b = Banca::nueva();
    let w = b.warp(|| false);
    assert!(!w.conectado());
    assert_eq!(w.modo(), Modo::Ninguno);
    assert!(b.ordenes().is_empty());
    assert!(!b.leer("reg-svc").contains("start"));
}

#[test]
fn preparar_prende_y_registra_una_vez() {
    let b = Banca::nueva();
    let w = b.warp(|| true);
    w.preparar().unwrap();
    w.preparar().unwrap();
    let o = b.ordenes();
    assert_eq!(o.iter().filter(|x| x.contains("registration new")).count(), 1);
    assert!(o.iter().all(|x| x.starts_with("--accept-tos")));
    assert!(
        b.leer("reg-svc")
            .contains("--no-ask-password start warp-svc.service")
    );
}

#[test]
fn cifrado_pide_ver_warp_on() {
    let b = Banca::nueva();
    let w = b.warp(|| true);
    w.preparar().unwrap();
    assert!(w.activar_cifrado());
    assert!(b.ordenes().contains(&"--accept-tos mode warp".to_string()));
    assert_eq!(w.modo(), Modo::Cifrado);
}

#[test]
fn el_protocolo_lo_elige_warp() {
    // Forzar MASQUE y luego WireGuard dejó al cliente real sin poder conectar en los intentos siguientes.
    let b = Banca::nueva();
    let w = b.warp(|| true);
    w.preparar().unwrap();
    w.activar_cifrado();
    assert!(
        !b.ordenes()
            .iter()
            .any(|o| o.contains("protocol") || o.contains("rotate-keys"))
    );
}

#[test]
fn si_cloudflare_no_ve_warp_no_cuenta() {
    let b = Banca::nueva();
    let w = b.warp(|| false);
    w.preparar().unwrap();
    assert!(!w.activar_cifrado());
}

#[test]
fn conectado_antes_de_dejar_pasar_trafico_se_le_da_tiempo() {
    let b = Banca::nueva();
    let (_c, traza) = traza_en_cola(&[false, false, true]);
    let w = b.warp(traza);
    w.preparar().unwrap();
    assert!(w.activar_cifrado());
}

#[test]
fn una_respuesta_perdida_no_quita_el_cifrado() {
    let b = Banca::nueva();
    let (cola, traza) = traza_en_cola(&[true]);
    let w = b.warp(traza);
    w.preparar().unwrap();
    assert!(w.activar_cifrado());
    cola.lock().unwrap().extend([false, true]);
    assert_eq!(w.modo(), Modo::Cifrado);
}

#[test]
fn solo_dns() {
    let b = Banca::nueva();
    let w = b.warp(|| false);
    w.preparar().unwrap();
    assert!(w.activar_solo_dns());
    assert!(b.ordenes().contains(&"--accept-tos mode doh".to_string()));
    std::fs::write(
        b.d.path().join("ajustes"),
        "Merged configuration:\n(not set)\tCompliance Environment: Normal\n(user set)\tMode: DnsOverHttps\n(network policy)\tWARP tunnel protocol: MASQUE\n(default)\tDoH MSS clamp enabled: true\n",
    )
    .unwrap();
    assert_eq!(w.modo(), Modo::Dns);
}

#[test]
fn conectado_sin_warp_ni_doh_no_dice_que_protege() {
    // «DoH MSS clamp» sale en todos los modos: no debe confundirse con el modo DNS.
    let b = Banca::nueva();
    let w = b.warp(|| false);
    w.preparar().unwrap();
    w.activar_solo_dns();
    std::fs::write(
        b.d.path().join("ajustes"),
        "Merged configuration:\n(user set)\tMode: Warp\n(default)\tDoH MSS clamp enabled: true\n",
    )
    .unwrap();
    assert_eq!(w.modo(), Modo::Ninguno);
}

#[test]
fn avisa_de_tailscale_solo_si_esta_en_este_equipo() {
    assert_eq!(
        proteccion::hay_tailscale(),
        Path::new("/sys/class/net/tailscale0").exists()
    );
}

#[test]
fn blindar_le_quita_el_dns_al_enlace_una_sola_vez() {
    let b = Banca::nueva();
    assert!(b.warp(|| true).blindar_dns());
    assert_eq!(
        b.leer("reg-dns").lines().collect::<Vec<_>>(),
        ["--no-ask-password default-route wlan0 false"]
    );
}

#[test]
fn blindar_avisa_si_no_pudo() {
    let b = Banca::nueva();
    std::fs::write(b.d.path().join("dns-falla"), "").unwrap();
    assert!(!b.warp(|| true).blindar_dns());
}

#[test]
fn apagar_le_devuelve_el_dns_al_enlace() {
    let b = Banca::nueva();
    let w = b.warp(|| true);
    w.preparar().unwrap();
    w.apagar();
    assert_eq!(
        b.leer("reg-dns").lines().last().unwrap(),
        "--no-ask-password default-route wlan0 true"
    );
}

#[test]
fn apagar_corta_y_apaga_el_servicio() {
    let b = Banca::nueva();
    let w = b.warp(|| true);
    w.preparar().unwrap();
    w.activar_cifrado();
    w.apagar();
    assert_eq!(b.ordenes().last().unwrap(), "--accept-tos disconnect");
    assert!(!b.d.path().join("svc").exists());
}

#[test]
fn sin_warp_instalado_dice_que_falta() {
    let t = tempfile::tempdir().unwrap();
    let w = Warp::con(
        Sistema::con_path(t.path().as_os_str()),
        Box::new(|| true),
        Box::new(|_| {}),
    );
    assert!(!w.instalado());
    assert!(!w.conectado());
    assert_eq!(
        w.preparar().unwrap_err().0,
        "Falta el cliente de Cloudflare WARP."
    );
}

// --- Sistema ---

#[test]
fn programas_con_limite_de_tiempo() {
    let b = Banca::nueva();
    guion(b.d.path(), "lento", "#!/bin/sh\nsleep 5\n");
    let s = b.sistema.correr(&["lento"], Duration::from_millis(200));
    assert_eq!(s.codigo, 124);
    assert_eq!(
        b.sistema
            .correr(&["no-existe-este"], Duration::from_secs(1))
            .codigo,
        127
    );
    let s = b.sistema.correr(&["echo", "a; rm -rf /"], Duration::from_secs(1));
    assert_eq!(s.salida, "a; rm -rf /\n", "los argumentos no pasan por un shell");
}

#[allow(dead_code)]
fn _usa(_: PathBuf) {}
