//! `vc`: lo mismo que `puente.py`, envolviendo `vortex_connect::motor::Connect` (el motor ya probado).
//! Mismos nombres de propiedades, señales y slots, para que los QML de la app de Python no cambien.
//!
//! Lo lento (cambiar de red, cifrar, buscar) corre en hilos y vuelve a la ventana con `qt_thread().queue`.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, estado)]
        #[qproperty(QString, red_actual, cxx_name = "redActual")]
        #[qproperty(QString, tipo_red, cxx_name = "tipoRed")]
        #[qproperty(bool, red_abierta, cxx_name = "redAbierta")]
        #[qproperty(QString, portal)]
        #[qproperty(QString, mensaje)]
        #[qproperty(bool, ocupado)]
        #[qproperty(bool, buscando)]
        #[qproperty(bool, hay_wifi, cxx_name = "hayWifi")]
        #[qproperty(bool, es_windows, cxx_name = "esWindows")]
        #[qproperty(bool, condiciones)]
        #[qproperty(bool, modo_viaje, cxx_name = "modoViaje")]
        #[qproperty(bool, warp_instalado, cxx_name = "warpInstalado")]
        #[qproperty(bool, hay_tailscale, cxx_name = "hayTailscale")]
        #[qproperty(QList_QVariant, redes)]
        type Puente = super::PuenteRust;

        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        fn actualizar(self: Pin<&mut Puente>);
        #[qinvokable]
        fn buscar(self: Pin<&mut Puente>);
        #[qinvokable]
        fn conectar(self: Pin<&mut Puente>, ssid: &QString);
        #[qinvokable]
        fn proteger(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "quitarProteccion"]
        fn quitar_proteccion(self: Pin<&mut Puente>);
        #[qinvokable]
        fn cancelar(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "abrirPortal"]
        fn abrir_portal(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "verCondiciones"]
        fn ver_condiciones(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "verPrivacidad"]
        fn ver_privacidad(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "bajarWarp"]
        fn bajar_warp(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "aceptarCondiciones"]
        fn aceptar_condiciones(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "cambiarModoViaje"]
        fn cambiar_modo_viaje(self: Pin<&mut Puente>, si: bool);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QVariant};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Duration;
use vortex_connect::ES_WINDOWS;
use vortex_connect::instancia;
use vortex_connect::motor::{Cancelar, Candado, Connect, Estado, Situacion};
use vortex_connect::proteccion::hay_tailscale;
use vortex_connect::redes::{ErrorRed, Red};
use vortex_connect::sistema::Sistema;
use vortex_connect::viaje;

const CONDICIONES: &str = "https://www.cloudflare.com/application/terms/";
const PRIVACIDAD: &str = "https://www.cloudflare.com/application/privacypolicy/";
const BAJAR_WARP: &str = "https://1.1.1.1/";

pub struct PuenteRust {
    estado: QString,
    red_actual: QString,
    tipo_red: QString,
    red_abierta: bool,
    portal: QString,
    mensaje: QString,
    ocupado: bool,
    buscando: bool,
    hay_wifi: bool,
    es_windows: bool,
    condiciones: bool,
    modo_viaje: bool,
    warp_instalado: bool,
    hay_tailscale: bool,
    redes: QList<QVariant>,
    // --- lo de adentro (no se ve en QML) ---
    sistema: Sistema,
    connect: Arc<Connect>,
    candado: Candado,
    cancelar: Cancelar,
    revisando: bool,
    portal_url: String,
    actual: Estado,
    lista_redes: Vec<Red>,
}

impl Default for PuenteRust {
    fn default() -> Self {
        let sistema = Sistema::real();
        PuenteRust {
            estado: QString::default(),
            red_actual: QString::default(),
            tipo_red: QString::default(),
            red_abierta: false,
            portal: QString::default(),
            mensaje: QString::default(),
            ocupado: false,
            buscando: false,
            hay_wifi: true,
            es_windows: ES_WINDOWS,
            condiciones: false,
            modo_viaje: false,
            warp_instalado: false,
            hay_tailscale: false,
            redes: QList::default(),
            connect: Arc::new(Connect::real(sistema.clone())),
            sistema,
            candado: Candado::new(),
            cancelar: Cancelar::new(),
            revisando: false,
            portal_url: String::new(),
            actual: Estado::default(),
            lista_redes: Vec::new(),
        }
    }
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn de_red(r: &Red) -> QVariant {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    m.insert(q("ssid"), QVariant::from(&q(&r.ssid)));
    m.insert(q("senal"), QVariant::from(&r.senal));
    m.insert(q("conectada"), QVariant::from(&r.conectada));
    QVariant::from(&m)
}

impl qobject::Puente {
    /// Pasa lo que sabe el motor a las propiedades de QML (solo avisa de lo que cambió).
    fn refrescar(mut self: Pin<&mut Self>) {
        let (e, portal, ajustes, instalado, lista, url) = {
            let r = self.rust();
            (
                r.actual.clone(),
                r.portal_url.clone(),
                r.connect.ajustes(),
                r.connect.warp.instalado(),
                r.lista_redes.clone(),
                r.actual.portal.clone(),
            )
        };
        let portal = if portal.is_empty() { url } else { portal };
        self.as_mut().set_estado(q(e.situacion.como_texto()));
        self.as_mut().set_red_actual(q(&e.red.ssid));
        self.as_mut().set_tipo_red(q(e.red.tipo.como_texto()));
        self.as_mut().set_red_abierta(e.red.abierta);
        self.as_mut().set_portal(q(&portal));
        self.as_mut().set_condiciones(ajustes.condiciones);
        self.as_mut().set_modo_viaje(ajustes.modo_viaje);
        self.as_mut().set_warp_instalado(instalado);
        self.as_mut().set_hay_tailscale(hay_tailscale());
        let mut l = QList::<QVariant>::default();
        for r in &lista {
            l.append(de_red(r));
        }
        self.as_mut().set_redes(l);
    }

    fn poner_mensaje(mut self: Pin<&mut Self>, texto: &str) {
        self.as_mut().set_mensaje(q(texto));
    }

    fn poner_estado(mut self: Pin<&mut Self>, e: Estado) {
        self.as_mut().rust_mut().revisando = false;
        self.as_mut().rust_mut().actual = e;
        self.refrescar();
    }

    fn poner_redes(mut self: Pin<&mut Self>, redes: Vec<Red>, error: &str, hay_wifi: Option<bool>) {
        self.as_mut().set_buscando(false);
        if let Some(h) = hay_wifi {
            self.as_mut().set_hay_wifi(h);
        }
        self.as_mut().rust_mut().lista_redes = redes;
        if !error.is_empty() {
            self.as_mut().set_mensaje(q(error));
        }
        self.refrescar();
    }

    fn poner_portal(mut self: Pin<&mut Self>, url: &str) {
        self.as_mut().rust_mut().portal_url = url.to_string();
        self.as_mut().refrescar();
        // lo acabas de pedir tú: aquí sí se abre de una
        self.as_ref().rust().sistema.abrir_url(url);
    }

    /// Lo que cambia la red, de uno en uno; también respecto al modo viaje.
    fn trabajar(
        mut self: Pin<&mut Self>,
        trabajo: impl FnOnce(&Connect, &Cancelar) -> Result<Estado, ErrorRed> + Send + 'static,
    ) {
        if *self.as_ref().ocupado() {
            return;
        }
        if !self.as_mut().rust_mut().candado.tomar() {
            self.poner_mensaje("El modo viaje está trabajando. Prueba en un momento.");
            return;
        }
        let cancelar = Cancelar::new();
        {
            let mut r = self.as_mut().rust_mut();
            r.cancelar = cancelar.clone();
            r.portal_url.clear();
        }
        self.as_mut().set_ocupado(true);
        self.as_mut().set_mensaje(QString::default());
        self.as_mut().refrescar();
        let connect = self.as_ref().rust().connect.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let r = trabajo(&connect, &cancelar);
            let _ = hilo.queue(move |mut obj| {
                match r {
                    Ok(e) => obj.as_mut().poner_estado(e),
                    Err(e) => obj.as_mut().poner_mensaje(&e.to_string()),
                }
                obj.terminar();
            });
        });
    }

    fn terminar(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().candado.soltar();
        self.as_mut().rust_mut().portal_url.clear();
        self.as_mut().set_ocupado(false);
        self.as_mut().refrescar();
        self.as_mut().actualizar();
        self.buscar_con(false);
    }

    fn buscar_con(mut self: Pin<&mut Self>, de_verdad: bool) {
        if *self.as_ref().buscando() {
            return;
        }
        self.as_mut().set_buscando(true);
        let connect = self.as_ref().rust().connect.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let hay = connect.redes.hay_wifi();
            let (redes, error) = match connect.redes.lista(de_verdad) {
                Ok(r) => (r, String::new()),
                Err(e) => (Vec::new(), e.to_string()),
            };
            let _ = hilo.queue(move |obj| obj.poner_redes(redes, &error, Some(hay)));
        });
    }
}

impl qobject::Puente {
    fn actualizar(mut self: Pin<&mut Self>) {
        if *self.as_ref().ocupado() || self.as_ref().rust().revisando {
            return;
        }
        self.as_mut().rust_mut().revisando = true;
        let connect = self.as_ref().rust().connect.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            // la ventana no se cae por una vuelta que falló
            let e = catch_unwind(AssertUnwindSafe(|| connect.estado()))
                .unwrap_or_else(|_| Estado::new(Situacion::SinRed, Default::default()));
            let _ = hilo.queue(move |obj| obj.poner_estado(e));
        });
    }

    fn buscar(self: Pin<&mut Self>) {
        self.buscar_con(true);
    }

    fn conectar(self: Pin<&mut Self>, ssid: &QString) {
        let ssid = ssid.to_string();
        self.trabajar(move |c, can| c.conectar(&ssid, can));
    }

    fn proteger(self: Pin<&mut Self>) {
        self.trabajar(|c, can| Ok(c.asegurar(can)));
    }

    fn quitar_proteccion(self: Pin<&mut Self>) {
        self.trabajar(|c, _| Ok(c.quitar()));
    }

    fn cancelar(self: Pin<&mut Self>) {
        self.as_ref().rust().cancelar.cancelar();
    }

    fn abrir_portal(self: Pin<&mut Self>) {
        let url = self.as_ref().portal().to_string();
        if !url.is_empty() {
            self.as_ref().rust().sistema.abrir_url(&url);
        }
    }

    fn ver_condiciones(self: Pin<&mut Self>) {
        self.as_ref().rust().sistema.abrir_url(CONDICIONES);
    }

    fn ver_privacidad(self: Pin<&mut Self>) {
        self.as_ref().rust().sistema.abrir_url(PRIVACIDAD);
    }

    fn bajar_warp(self: Pin<&mut Self>) {
        self.as_ref().rust().sistema.abrir_url(BAJAR_WARP);
    }

    fn aceptar_condiciones(self: Pin<&mut Self>) {
        self.as_ref()
            .rust()
            .connect
            .cambiar_ajustes(|a| a.condiciones = true);
        self.refrescar();
    }

    fn cambiar_modo_viaje(mut self: Pin<&mut Self>, si: bool) {
        let connect = self.as_ref().rust().connect.clone();
        connect.cambiar_ajustes(|a| a.modo_viaje = si);
        let sistema = self.as_ref().rust().sistema.clone();
        if !viaje::prender(&sistema, si) {
            connect.cambiar_ajustes(|a| a.modo_viaje = false);
            self.as_mut().set_mensaje(q("No se pudo prender el modo viaje."));
        }
        self.refrescar();
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        let hilo = self.qt_thread();
        let connect = self.as_ref().rust().connect.clone();
        // Lo que cuenta el motor llega a la ventana como «mensaje»; el portal se abre solo (lo pediste tú).
        connect.poner_avisar({
            let hilo = hilo.clone();
            Arc::new(move |texto| {
                let texto = texto.to_string();
                let _ = hilo.queue(move |mut obj| obj.as_mut().poner_mensaje(&texto));
            })
        });
        connect.poner_abrir_portal({
            let hilo = hilo.clone();
            Arc::new(move |url| {
                let url = url.to_string();
                let _ = hilo.queue(move |obj| obj.poner_portal(&url));
            })
        });
        self.as_mut().refrescar();
        self.as_mut().actualizar();
        self.as_mut().buscar_con(true);
        // Cada 15 s, ¿cómo va la red?
        std::thread::spawn({
            let hilo = hilo.clone();
            move || {
                loop {
                    std::thread::sleep(Duration::from_secs(15));
                    if hilo.queue(|obj| obj.actualizar()).is_err() {
                        break; // la ventana ya se cerró
                    }
                }
            }
        });
        // En las pruebas (--prueba-humo, recorrido) no se toca el socket de una ventana de verdad.
        if std::env::var_os("VORTEX_CONNECT_RECORRER").is_some()
            || std::env::args().any(|a| a == "--prueba-humo")
        {
            return;
        }
        // Una sola ventana: lo que pida otra copia («muéstrate») llega por un socket; se espera sin gastar.
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        std::thread::spawn(move || {
            while escucha.esperar() {
                if hilo.queue(|obj| obj.mostrar_ventana()).is_err() {
                    break;
                }
            }
        });
    }
}
