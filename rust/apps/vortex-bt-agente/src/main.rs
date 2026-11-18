//! vortex-bt-agente: el agente de Bluetooth de Vortexine's OS. Corre como servicio de usuario.
//!
//! Sin un agente registrado, BlueZ deja el adaptador en «Pairable: no» y no guarda la llave de los aparatos
//! nuevos: los audífonos se conectan sin quedar emparejados y luego cortan. Con este, el panel de Bluetooth
//! empareja de verdad.
//!
//! Lo que pides desde tu PC (audífonos, bocinas) no pregunta nada: lo acepta el kernel. Lo que pide otro aparato,
//! o lo que lleva un número que comparar, sale como aviso con botones. Si no contestas, o tocas la tarjeta, es
//! que no: un aparato que se empareje sin que digas que sí podría hacerse pasar por un teclado.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_io::Timer;
use futures_channel::oneshot;
use futures_util::{FutureExt, StreamExt, select};
use zbus::{Connection, DBusError, interface};
use zbus::{ObjectPath, OwnedObjectPath, OwnedVariant, Variant};

const BLUEZ: &str = "org.bluez";
const RUTA: &str = "/org/vortexine/bt_agente";
/// Hay pantalla y botones; no hay dónde escribir un número.
const CAPACIDAD: &str = "DisplayYesNo";
/// BlueZ deja de esperar al minuto: que el aviso se vaya antes.
const ESPERA_S: u64 = 50;

/// Cuánto espera la pregunta. Solo las pruebas cambian esto, con `VORTEX_BT_AGENTE_ESPERA_S`.
fn espera_s() -> u64 {
    std::env::var("VORTEX_BT_AGENTE_ESPERA_S")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(ESPERA_S)
}
const PIN_DE_SIEMPRE: &str = "0000";

/// Los errores que BlueZ entiende (`org.bluez.Error.*`).
#[derive(Debug, DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum ErrorBluez {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
    Canceled(String),
}

fn rechazado(texto: &str) -> ErrorBluez {
    ErrorBluez::Rejected(texto.into())
}

/// Equivale a `html.escape(texto)` de Python: el nombre lo elige el otro aparato y no se le deja meter formato.
fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&#x27;"),
            _ => s.push(c),
        }
    }
    s
}

/// `/org/bluez/hci0/dev_98_80_BB_41_01_D5` → `98:80:BB:41:01:D5`
fn direccion_de(ruta: &str) -> String {
    let fin = ruta.rsplit('/').next().unwrap_or("");
    if let Some(mac) = fin.strip_prefix("dev_") {
        let partes: Vec<&str> = mac.split('_').collect();
        if partes.len() == 6
            && partes
                .iter()
                .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return partes.join(":");
        }
    }
    ruta.to_string()
}

/// ¿Está `notify-send` en el PATH?
fn hay_notify_send() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("notify-send").is_file()))
}

fn aviso(extra: &[&str], titulo: &str, texto: &str, salida: Stdio) -> std::io::Result<Child> {
    Command::new("notify-send")
        .args(["-a", "Vortexine's OS", "-i", "bluetooth"])
        .args(extra)
        .arg(titulo)
        .arg(escapar(texto))
        .stdin(Stdio::null())
        .stdout(salida)
        .stderr(Stdio::null())
        .spawn()
}

/// La pregunta que espera respuesta: BlueZ pregunta de una en una.
struct Pendiente {
    id: u64,
    cancelar: oneshot::Sender<&'static str>,
}

#[derive(Default)]
struct Estado {
    siguiente: u64,
    pendiente: Option<Pendiente>,
}

struct Agente {
    conexion: Connection,
    estado: Arc<Mutex<Estado>>,
}

impl Agente {
    async fn nombre_de(&self, ruta: &ObjectPath<'_>) -> String {
        let direccion = direccion_de(ruta.as_str());
        let pedir = self.conexion.call_method(
            Some(BLUEZ),
            ruta,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.bluez.Device1", "Alias"),
        );
        let alias = select! {
            r = pedir.fuse() => r.ok().and_then(|m| {
                let v: OwnedVariant = m.body().deserialize().ok()?;
                String::try_from(v).ok()
            }),
            _ = FutureExt::fuse(Timer::after(Duration::from_secs(3))) => None,
        };
        match alias {
            Some(a) if !a.is_empty() && a.replace('-', ":") != direccion => format!("{a} ({direccion})"),
            _ => direccion,
        }
    }

    /// Aviso sin botones (el PIN o el número que hay que escribir en el otro aparato).
    fn mostrar(&self, texto: &str) {
        if !hay_notify_send() {
            return;
        }
        if let Ok(mut hijo) = aviso(&[], "Emparejar por Bluetooth", texto, Stdio::null()) {
            std::thread::spawn(move || {
                let _ = hijo.wait();
            });
        }
    }

    /// Aviso con Aceptar/Rechazar. Solo `aceptar` en la salida de `notify-send` cuenta como sí.
    async fn preguntar(&self, titulo: &str, texto: &str) -> Result<(), ErrorBluez> {
        if !hay_notify_send() {
            return Err(rechazado("No hay cómo preguntar"));
        }
        let espera_s = espera_s();
        let espera = (espera_s * 1000).to_string();
        let hijo = aviso(
            &[
                "-u",
                "critical",
                "-t",
                &espera,
                "-A",
                "aceptar=Aceptar",
                "-A",
                "rechazar=Rechazar",
            ],
            titulo,
            texto,
            Stdio::piped(),
        )
        .map_err(|_| rechazado("No hay cómo preguntar"))?;
        let mut hijo = hijo;
        let mut tuberia = hijo.stdout.take();
        let hijo = Arc::new(Mutex::new(hijo));

        // Quien esperaba antes se queda con la respuesta segura.
        let (cancelar, cancelado) = oneshot::channel();
        let id = {
            let mut e = self.estado.lock().unwrap();
            e.siguiente += 1;
            let id = e.siguiente;
            if let Some(vieja) = e.pendiente.replace(Pendiente { id, cancelar }) {
                let _ = vieja.cancelar.send("Canceled");
            }
            id
        };

        let (tx, contestado) = oneshot::channel::<String>();
        let hijo_hilo = Arc::clone(&hijo);
        std::thread::spawn(move || {
            let mut salida = String::new();
            if let Some(t) = tuberia.as_mut() {
                let _ = t.read_to_string(&mut salida);
            }
            let _ = hijo_hilo.lock().unwrap().wait();
            let _ = tx.send(salida);
        });

        let resultado = select! {
            s = contestado.fuse() => match s {
                Ok(s) if s.trim() == "aceptar" => Ok(()),
                _ => Err(rechazado("No se aceptó")),
            },
            _ = FutureExt::fuse(Timer::after(Duration::from_secs(espera_s))) => Err(rechazado("No se aceptó")),
            c = cancelado.fuse() => match c {
                Ok(_) => Err(ErrorBluez::Canceled("No se aceptó".into())),
                Err(_) => Err(rechazado("No se aceptó")),
            },
        };
        // Quita la pregunta (si sigue siendo la nuestra) y cierra el aviso.
        {
            let mut e = self.estado.lock().unwrap();
            if e.pendiente.as_ref().is_some_and(|p| p.id == id) {
                e.pendiente = None;
            }
        }
        let _ = hijo.lock().unwrap().kill();
        resultado
    }
}

#[interface(name = "org.bluez.Agent1")]
impl Agente {
    fn release(&self) {}

    async fn request_pin_code(&self, dispositivo: ObjectPath<'_>) -> Result<String, ErrorBluez> {
        let nombre = self.nombre_de(&dispositivo).await;
        self.preguntar(
            "Un aparato pide un PIN",
            &format!(
                "«{nombre}» quiere emparejarse. Aceptar usa {PIN_DE_SIEMPRE}, el de casi todos los audífonos viejos."
            ),
        )
        .await?;
        Ok(PIN_DE_SIEMPRE.to_string())
    }

    /// No hay dónde escribirlo: se dice que no en vez de contestar un número inventado.
    async fn request_passkey(&self, _dispositivo: ObjectPath<'_>) -> Result<u32, ErrorBluez> {
        Err(rechazado("Este equipo no puede escribir un número"))
    }

    async fn display_pin_code(&self, dispositivo: ObjectPath<'_>, pin: String) {
        let nombre = self.nombre_de(&dispositivo).await;
        self.mostrar(&format!("Escribe {pin} en «{nombre}» para emparejarlo."));
    }

    async fn display_passkey(&self, dispositivo: ObjectPath<'_>, llave: u32, _escritas: u16) {
        let nombre = self.nombre_de(&dispositivo).await;
        self.mostrar(&format!("Escribe {llave:06} en «{nombre}» para emparejarlo."));
    }

    async fn request_confirmation(&self, dispositivo: ObjectPath<'_>, llave: u32) -> Result<(), ErrorBluez> {
        let nombre = self.nombre_de(&dispositivo).await;
        self.preguntar(
            "Emparejar por Bluetooth",
            &format!("Acepta solo si «{nombre}» enseña el número {llave:06}."),
        )
        .await
    }

    async fn request_authorization(&self, dispositivo: ObjectPath<'_>) -> Result<(), ErrorBluez> {
        let nombre = self.nombre_de(&dispositivo).await;
        self.preguntar(
            "Un aparato quiere emparejarse",
            &format!("«{nombre}» quiere emparejarse. Acepta solo si lo estás conectando tú."),
        )
        .await
    }

    async fn authorize_service(&self, dispositivo: ObjectPath<'_>, _uuid: String) -> Result<(), ErrorBluez> {
        let nombre = self.nombre_de(&dispositivo).await;
        self.preguntar(
            "Un aparato quiere conectarse",
            &format!("«{nombre}» quiere conectarse. Acepta solo si lo estás conectando tú."),
        )
        .await
    }

    fn cancel(&self) {
        let vieja = self.estado.lock().unwrap().pendiente.take();
        if let Some(p) = vieja {
            let _ = p.cancelar.send("Canceled");
        }
    }
}

/// Se registra como agente de siempre y deja pairable el adaptador. Sin BlueZ todavía no pasa nada: se
/// vuelve a intentar cuando BlueZ aparezca en el bus.
async fn registrar(conexion: &Connection) -> bool {
    let ruta = ObjectPath::from_static_str_unchecked(RUTA);
    let r = conexion
        .call_method(
            Some(BLUEZ),
            "/org/bluez",
            Some("org.bluez.AgentManager1"),
            "RegisterAgent",
            &(&ruta, CAPACIDAD),
        )
        .await;
    let r = match r {
        Ok(_) => {
            conexion
                .call_method(
                    Some(BLUEZ),
                    "/org/bluez",
                    Some("org.bluez.AgentManager1"),
                    "RequestDefaultAgent",
                    &(&ruta,),
                )
                .await
        }
        Err(e) => Err(e),
    };
    if let Err(e) = r {
        eprintln!("vortex-bt-agente: no pude registrarme: {e}");
        return false;
    }
    poner_pairable(conexion).await;
    true
}

/// Sin esto el adaptador se queda en «Pairable: no» y nada nuevo se puede emparejar desde el otro aparato.
async fn poner_pairable(conexion: &Connection) {
    type Objetos = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedVariant>>>;
    let Ok(m) = conexion
        .call_method(
            Some(BLUEZ),
            "/",
            Some("org.freedesktop.DBus.ObjectManager"),
            "GetManagedObjects",
            &(),
        )
        .await
    else {
        return;
    };
    let Ok(objetos) = m.body().deserialize::<Objetos>() else {
        return;
    };
    for (ruta, interfaces) in objetos {
        if !interfaces.contains_key("org.bluez.Adapter1") {
            continue;
        }
        let r = conexion
            .call_method(
                Some(BLUEZ),
                &ruta,
                Some("org.freedesktop.DBus.Properties"),
                "Set",
                &("org.bluez.Adapter1", "Pairable", Variant::from(true)),
            )
            .await;
        if let Err(e) = r {
            eprintln!("vortex-bt-agente: no pude dejar pairable {ruta}: {e}");
        }
    }
}

async fn servir() -> zbus::Result<()> {
    let conexion = zbus::connection::Builder::system().build().await?;
    let agente = Agente {
        conexion: conexion.clone(),
        estado: Arc::default(),
    };
    conexion.object_server().at(RUTA, agente).await?;

    // BlueZ olvida a los agentes si se reinicia: al volver hay que registrarse otra vez.
    let dbus = zbus::fdo::DBusProxy::new(&conexion).await?;
    let mut cambios = dbus.receive_name_owner_changed_with_args(&[(0, BLUEZ)]).await?;
    registrar(&conexion).await;
    while let Some(c) = cambios.next().await {
        if c.args().is_ok_and(|a| a.new_owner().is_some()) {
            registrar(&conexion).await;
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = async_io::block_on(servir()) {
        eprintln!("vortex-bt-agente: sin bus del sistema: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn direccion() {
        assert_eq!(
            direccion_de("/org/bluez/hci0/dev_98_80_BB_41_01_D5"),
            "98:80:BB:41:01:D5"
        );
        assert_eq!(direccion_de("/otra/cosa"), "/otra/cosa");
    }

    #[test]
    fn escapa_formato() {
        assert_eq!(escapar("<b>T&'\"</b>"), "&lt;b&gt;T&amp;&#x27;&quot;&lt;/b&gt;");
    }
}
