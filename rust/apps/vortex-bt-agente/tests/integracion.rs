//! Pruebas de vortex-bt-agente sin tocar tu Bluetooth ni tu escritorio: un `dbus-daemon --session` propio, un
//! BlueZ falso en él (AgentManager1, Adapter1, Device1) y un `notify-send` falso que contesta lo que pida cada prueba.
//! Es la misma prueba que `pruebas/bt-agente.py` hace con el agente de Python.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_io::{Timer, block_on};
use zbus::message::Header;
use zbus::{Connection, interface};

const RUTA: &str = "/org/vortexine/bt_agente";
const DISPOSITIVO: &str = "/org/bluez/hci0/dev_98_80_BB_41_01_D5";

/// Lo que BlueZ falso ha visto: (método, argumentos, quién llamó).
type Llamadas = Arc<Mutex<Vec<(String, String, String)>>>;

struct Administrador(Llamadas);

#[interface(name = "org.bluez.AgentManager1")]
impl Administrador {
    fn register_agent(&self, agente: zbus::ObjectPath<'_>, capacidad: String, #[zbus(header)] h: Header<'_>) {
        let quien = h.sender().map(|s| s.to_string()).unwrap_or_default();
        self.0
            .lock()
            .unwrap()
            .push(("RegisterAgent".into(), format!("{agente} {capacidad}"), quien));
    }

    fn request_default_agent(&self, agente: zbus::ObjectPath<'_>, #[zbus(header)] h: Header<'_>) {
        let quien = h.sender().map(|s| s.to_string()).unwrap_or_default();
        self.0
            .lock()
            .unwrap()
            .push(("RequestDefaultAgent".into(), agente.to_string(), quien));
    }
}

struct Adaptador {
    pairable: bool,
}

#[interface(name = "org.bluez.Adapter1")]
impl Adaptador {
    #[zbus(property)]
    fn pairable(&self) -> bool {
        self.pairable
    }

    #[zbus(property)]
    fn set_pairable(&mut self, valor: bool) {
        self.pairable = valor;
    }
}

struct Aparato(Arc<Mutex<String>>);

#[interface(name = "org.bluez.Device1")]
impl Aparato {
    #[zbus(property)]
    fn alias(&self) -> String {
        self.0.lock().unwrap().clone()
    }
}

struct BluezFalso {
    conexion: Option<Connection>,
    llamadas: Llamadas,
    alias: Arc<Mutex<String>>,
}

impl BluezFalso {
    fn nuevo(direccion: &str) -> Self {
        let llamadas = Llamadas::default();
        let alias = Arc::new(Mutex::new("SOUNDPEATS H3".to_string()));
        let (l, a) = (llamadas.clone(), alias.clone());
        let conexion = block_on(async {
            let c = zbus::connection::Builder::address(direccion)
                .name("org.bluez")
                .serve_at("/org/bluez", Administrador(l))
                .serve_at("/org/bluez/hci0", Adaptador { pairable: false })
                .serve_at(DISPOSITIVO, Aparato(a))
                .serve_at("/", zbus::fdo::ObjectManager)
                .build()
                .await?;
            Ok::<_, zbus::Error>(c)
        })
        .expect("BlueZ falso");
        Self {
            conexion: Some(conexion),
            llamadas,
            alias,
        }
    }

    fn cerrar(&mut self) {
        if let Some(c) = self.conexion.take() {
            block_on(c.graceful_shutdown());
        }
    }

    fn llamadas(&self) -> Vec<(String, String, String)> {
        self.llamadas.lock().unwrap().clone()
    }

    fn pairable(&self) -> bool {
        let c = self.conexion.as_ref().unwrap();
        block_on(async {
            let i = c
                .object_server()
                .interface::<_, Adaptador>("/org/bluez/hci0")
                .await
                .unwrap();
            let guardia = i.get().await;
            guardia.pairable
        })
    }
}

fn esperar(segundos: f64, condicion: impl Fn() -> bool) {
    let fin = Instant::now() + Duration::from_secs_f64(segundos);
    while Instant::now() < fin {
        if condicion() {
            return;
        }
        block_on(Timer::after(Duration::from_millis(20)));
    }
    panic!("no pasó a tiempo");
}

struct Entorno {
    tmp: tempfile::TempDir,
    dbus: Child,
    direccion: String,
}

impl Entorno {
    fn nuevo() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let t = tmp.path();
        let socket = t.join("bus");
        let direccion = format!("unix:path={}", socket.display());
        let dbus = Command::new("dbus-daemon")
            .args([
                "--session",
                &format!("--address={direccion}"),
                "--nofork",
                "--nopidfile",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        esperar(5.0, || socket.exists());
        fs::create_dir(t.join("bin")).unwrap();
        let falso = t.join("bin/notify-send");
        fs::write(
            &falso,
            format!(
                r#"#!/bin/sh
for a in "$@"; do printf '%s\n' "$a"; done >>"{t}/aviso"
respuesta=$(cat "{t}/respuesta" 2>/dev/null)
case "$respuesta" in colgar) exec sleep 60;; esac
case "$*" in *"-A"*) printf '%s\n' "$respuesta";; esac
"#,
                t = t.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&falso, fs::Permissions::from_mode(0o755)).unwrap();
        fs::create_dir(t.join("vacio")).unwrap();
        fs::write(t.join("respuesta"), "").unwrap();
        Self { tmp, dbus, direccion }
    }

    fn t(&self) -> &Path {
        self.tmp.path()
    }

    fn responder(&self, texto: &str) {
        fs::write(self.t().join("respuesta"), texto).unwrap();
    }

    fn aviso(&self) -> String {
        fs::read_to_string(self.t().join("aviso")).unwrap_or_default()
    }

    fn arrancar_agente(&self, path: Option<PathBuf>, espera_s: Option<&str>) -> Child {
        let path = path.unwrap_or_else(|| format!("{}:/usr/bin:/bin", self.t().join("bin").display()).into());
        let mut c = Command::new(env!("CARGO_BIN_EXE_vortex-bt-agente"));
        c.env_clear()
            .env("DBUS_SYSTEM_BUS_ADDRESS", &self.direccion)
            .env("DBUS_SESSION_BUS_ADDRESS", &self.direccion)
            .env("PATH", path);
        if let Some(e) = espera_s {
            c.env("VORTEX_BT_AGENTE_ESPERA_S", e);
        }
        c.spawn().expect("agente")
    }
}

impl Drop for Entorno {
    fn drop(&mut self) {
        let _ = self.dbus.kill();
        let _ = self.dbus.wait();
    }
}

/// Un escenario completo: bus, BlueZ falso, agente (ya registrado) y un cliente que llama como BlueZ.
struct Escena {
    e: Entorno,
    bluez: BluezFalso,
    agente: Child,
    cliente: Connection,
    nombre: String,
}

impl Escena {
    fn nueva() -> Self {
        Self::con(false, None)
    }

    fn con(sin_avisos: bool, espera_s: Option<&str>) -> Self {
        let e = Entorno::nuevo();
        let bluez = BluezFalso::nuevo(&e.direccion);
        let path = sin_avisos.then(|| e.t().join("vacio"));
        let agente = e.arrancar_agente(path, espera_s);
        let cliente = block_on(async {
            zbus::connection::Builder::address(e.direccion.as_str())
                .build()
                .await
        })
        .expect("cliente");
        esperar(5.0, || bluez.llamadas().len() >= 2);
        let nombre = bluez.llamadas()[0].2.clone();
        Self {
            e,
            bluez,
            agente,
            cliente,
            nombre,
        }
    }

    /// Llama al agente como BlueZ; devuelve Ok(texto del cuerpo) o Err(nombre del error D-Bus).
    fn llamar<B>(&self, metodo: &str, cuerpo: &B, espera_s: u64) -> Result<String, String>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        let c = self.cliente.clone();
        let nombre = self.nombre.clone();
        let metodo = metodo.to_string();
        let r = block_on(async {
            futures_util::future::select(
                Box::pin(c.call_method(
                    Some(nombre.as_str()),
                    RUTA,
                    Some("org.bluez.Agent1"),
                    metodo.as_str(),
                    cuerpo,
                )),
                Box::pin(Timer::after(Duration::from_secs(espera_s))),
            )
            .await
        });
        match r {
            futures_util::future::Either::Left((Ok(m), _)) => Ok(format!("{:?}", m.body().signature())),
            futures_util::future::Either::Left((Err(zbus::Error::MethodError(n, _, _)), _)) => {
                Err(n.to_string())
            }
            futures_util::future::Either::Left((Err(e), _)) => Err(format!("otro: {e}")),
            futures_util::future::Either::Right(_) => Err("sin respuesta".into()),
        }
    }

    fn llamar_pin(&self, metodo: &str, dispositivo: &str) -> Result<String, String> {
        let ruta = zbus::ObjectPath::try_from(dispositivo).unwrap();
        let c = self.cliente.clone();
        let m = block_on(c.call_method(
            Some(self.nombre.as_str()),
            RUTA,
            Some("org.bluez.Agent1"),
            metodo,
            &(ruta,),
        ));
        match m {
            Ok(m) => Ok(m.body().deserialize::<String>().unwrap_or_default()),
            Err(zbus::Error::MethodError(n, _, _)) => Err(n.to_string()),
            Err(e) => Err(format!("otro: {e}")),
        }
    }
}

impl Drop for Escena {
    fn drop(&mut self) {
        let _ = self.agente.kill();
        let _ = self.agente.wait();
        self.bluez.cerrar();
    }
}

fn ruta(r: &str) -> zbus::ObjectPath<'_> {
    zbus::ObjectPath::try_from(r).unwrap()
}

const REJECTED: &str = "org.bluez.Error.Rejected";
const CANCELED: &str = "org.bluez.Error.Canceled";

#[test]
fn se_registra_como_el_agente_de_siempre() {
    let s = Escena::nueva();
    let l = s.bluez.llamadas();
    assert_eq!(
        (l[0].0.as_str(), l[0].1.as_str()),
        ("RegisterAgent", "/org/vortexine/bt_agente DisplayYesNo")
    );
    assert_eq!(
        (l[1].0.as_str(), l[1].1.as_str()),
        ("RequestDefaultAgent", "/org/vortexine/bt_agente")
    );
}

#[test]
fn deja_el_adaptador_pairable() {
    let s = Escena::nueva();
    esperar(5.0, || s.bluez.pairable());
}

#[test]
fn se_registra_otra_vez_si_bluez_reinicia() {
    let mut s = Escena::nueva();
    s.bluez.cerrar();
    block_on(Timer::after(Duration::from_millis(200)));
    let otro = BluezFalso::nuevo(&s.e.direccion);
    esperar(5.0, || otro.llamadas().len() >= 2);
    assert_eq!(otro.llamadas()[1].0, "RequestDefaultAgent");
    esperar(5.0, || otro.pairable());
}

#[test]
fn aceptar_con_el_boton_empareja() {
    let s = Escena::nueva();
    s.e.responder("aceptar");
    assert!(
        s.llamar("RequestConfirmation", &(ruta(DISPOSITIVO), 123456u32), 8)
            .is_ok()
    );
    let aviso = s.e.aviso();
    assert!(aviso.contains("123456"), "{aviso}");
    assert!(aviso.contains("SOUNDPEATS H3 (98:80:BB:41:01:D5)"), "{aviso}");
    assert!(aviso.contains("critical") && aviso.contains("50000"), "{aviso}");
    assert!(aviso.contains("Emparejar por Bluetooth"), "{aviso}");
}

#[test]
fn rechazar_no_empareja() {
    let s = Escena::nueva();
    s.e.responder("rechazar");
    assert_eq!(
        s.llamar("RequestAuthorization", &(ruta(DISPOSITIVO),), 8),
        Err(REJECTED.into())
    );
}

#[test]
fn tocar_la_tarjeta_no_acepta_nada() {
    let s = Escena::nueva();
    s.e.responder("default");
    assert_eq!(
        s.llamar("RequestAuthorization", &(ruta(DISPOSITIVO),), 8),
        Err(REJECTED.into())
    );
}

#[test]
fn sin_respuesta_es_que_no() {
    let s = Escena::nueva();
    // El aviso se cierra sin decir nada.
    assert_eq!(
        s.llamar("AuthorizeService", &(ruta(DISPOSITIVO), "0000110b"), 8),
        Err(REJECTED.into())
    );
}

#[test]
fn si_el_aviso_se_cuelga_vence_la_espera() {
    let s = Escena::con(false, Some("1"));
    s.e.responder("colgar");
    let t = Instant::now();
    assert_eq!(
        s.llamar("RequestAuthorization", &(ruta(DISPOSITIVO),), 8),
        Err(REJECTED.into())
    );
    assert!(t.elapsed() < Duration::from_secs(5));
}

#[test]
fn si_bluez_cancela_se_quita_la_pregunta() {
    let s = Escena::nueva();
    s.e.responder("colgar");
    let c = s.cliente.clone();
    let nombre = s.nombre.clone();
    let pregunta = std::thread::spawn(move || {
        block_on(c.call_method(
            Some(nombre.as_str()),
            RUTA,
            Some("org.bluez.Agent1"),
            "RequestAuthorization",
            &(ruta(DISPOSITIVO),),
        ))
    });
    esperar(5.0, || !s.e.aviso().is_empty());
    block_on(s.cliente.call_method(
        Some(s.nombre.as_str()),
        RUTA,
        Some("org.bluez.Agent1"),
        "Cancel",
        &(),
    ))
    .unwrap();
    match pregunta.join().unwrap() {
        Err(zbus::Error::MethodError(n, _, _)) => assert_eq!(n.as_str(), CANCELED),
        otro => panic!("{otro:?}"),
    }
}

#[test]
fn un_nombre_con_formato_no_se_interpreta() {
    let s = Escena::nueva();
    *s.bluez.alias.lock().unwrap() = "<b>Teclado</b>".into();
    s.e.responder("rechazar");
    let _ = s.llamar("RequestAuthorization", &(ruta(DISPOSITIVO),), 8);
    let aviso = s.e.aviso();
    assert!(aviso.contains("&lt;b&gt;Teclado"), "{aviso}");
    assert!(!aviso.contains("<b>"), "{aviso}");
}

#[test]
fn el_pin_solo_con_permiso() {
    let s = Escena::nueva();
    s.e.responder("aceptar");
    assert_eq!(s.llamar_pin("RequestPinCode", DISPOSITIVO), Ok("0000".into()));
    s.e.responder("rechazar");
    assert!(s.llamar_pin("RequestPinCode", DISPOSITIVO).is_err());
}

#[test]
fn no_inventa_un_numero() {
    let s = Escena::nueva();
    assert_eq!(
        s.llamar("RequestPasskey", &(ruta(DISPOSITIVO),), 8),
        Err(REJECTED.into())
    );
    assert_eq!(s.e.aviso(), "");
}

#[test]
fn ensena_el_numero_que_hay_que_escribir() {
    let s = Escena::nueva();
    assert!(
        s.llamar("DisplayPasskey", &(ruta(DISPOSITIVO), 42u32, 0u16), 8)
            .is_ok()
    );
    esperar(5.0, || s.e.aviso().contains("000042"));
    assert!(
        s.llamar("DisplayPinCode", &(ruta(DISPOSITIVO), "1234"), 8)
            .is_ok()
    );
    esperar(5.0, || s.e.aviso().contains("Escribe 1234"));
}

#[test]
fn sigue_vivo_despues_de_contestar() {
    let mut s = Escena::nueva();
    s.e.responder("rechazar");
    let _ = s.llamar("RequestAuthorization", &(ruta(DISPOSITIVO),), 8);
    s.e.responder("aceptar");
    assert!(s.llamar("RequestAuthorization", &(ruta(DISPOSITIVO),), 8).is_ok());
    assert!(s.agente.try_wait().unwrap().is_none());
}

#[test]
fn sin_notify_send_no_hay_como_preguntar() {
    let s = Escena::con(true, None);
    assert_eq!(
        s.llamar("RequestConfirmation", &(ruta(DISPOSITIVO), 1u32), 8),
        Err(REJECTED.into())
    );
}
