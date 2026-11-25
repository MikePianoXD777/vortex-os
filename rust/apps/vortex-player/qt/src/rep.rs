//! `rep`: el reproductor como lo ve QML (las propiedades, señales y botones de `reproductor.py`). Es solo la
//! cara: el reproductor de verdad vive en `motor.rs` y aquí llega una foto de lo que cambió.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QVariant, cancion)]
        #[qproperty(f64, posicion)]
        #[qproperty(f64, duracion)]
        #[qproperty(bool, pausado)]
        #[qproperty(i32, volumen)]
        #[qproperty(bool, aleatorio)]
        #[qproperty(QString, repetir)]
        #[qproperty(bool, hay_anterior, cxx_name = "hayAnterior")]
        #[qproperty(bool, hay_siguiente, cxx_name = "haySiguiente")]
        #[qproperty(QVariant, cola)]
        #[qproperty(QVariant, cola_siguientes, cxx_name = "colaSiguientes")]
        #[qproperty(i32, cola_restantes, cxx_name = "colaRestantes")]
        #[qproperty(QString, dormir_texto, cxx_name = "dormirTexto")]
        #[qproperty(bool, dormir_activo, cxx_name = "dormirActivo")]
        #[qproperty(f64, fundido)]
        #[qproperty(f64, velocidad)]
        #[qproperty(f64, ab_a, cxx_name = "abA")]
        #[qproperty(f64, ab_b, cxx_name = "abB")]
        #[qproperty(i32, indice)]
        #[qproperty(QString, origen)]
        type Rep = super::RepRust;

        #[qsignal]
        #[cxx_name = "cambioCancion"]
        fn cambio_cancion(self: Pin<&mut Rep>);
        #[qsignal]
        fn error(self: Pin<&mut Rep>, texto: &QString);

        #[qinvokable]
        fn alternar(self: &Rep);
        #[qinvokable]
        fn pausar(self: &Rep);
        #[qinvokable]
        fn seguir(self: &Rep);
        #[qinvokable]
        fn siguiente(self: &Rep);
        #[qinvokable]
        fn anterior(self: &Rep);
        #[qinvokable]
        fn buscar(self: &Rep, segundos: f64);
        #[qinvokable]
        #[cxx_name = "ponerVolumen"]
        fn poner_volumen(self: &Rep, v: i32);
        #[qinvokable]
        #[cxx_name = "alternarAleatorio"]
        fn alternar_aleatorio(self: &Rep);
        #[qinvokable]
        #[cxx_name = "alternarRepetir"]
        fn alternar_repetir(self: &Rep);
        #[qinvokable]
        #[cxx_name = "saltarA"]
        fn saltar_a(self: &Rep, k: i32);
        #[qinvokable]
        #[cxx_name = "quitarDeColaEn"]
        fn quitar_de_cola_en(self: &Rep, k: i32);
        #[qinvokable]
        #[cxx_name = "quitarDeCola"]
        fn quitar_de_cola(self: &Rep, ruta: &QString);
        #[qinvokable]
        fn dormir(self: &Rep, minutos: i32);
        #[qinvokable]
        #[cxx_name = "ponerVelocidad"]
        fn poner_velocidad(self: &Rep, v: f64);
        #[qinvokable]
        #[cxx_name = "marcarAB"]
        fn marcar_ab(self: &Rep);
        #[qinvokable]
        #[cxx_name = "quitarAB"]
        fn quitar_ab(self: &Rep);
    }

    impl cxx_qt::Initialize for Rep {}
    impl cxx_qt::Threading for Rep {}
}

use crate::arranque::ffi::json_a_variante;
use crate::motor::con_rep;
use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::{QString, QVariant};
use serde_json::Value;
use vortex_player::reproductor::{Aviso, Reproductor};

pub struct RepRust {
    cancion: QVariant,
    posicion: f64,
    duracion: f64,
    pausado: bool,
    volumen: i32,
    aleatorio: bool,
    repetir: QString,
    hay_anterior: bool,
    hay_siguiente: bool,
    cola: QVariant,
    cola_siguientes: QVariant,
    cola_restantes: i32,
    dormir_texto: QString,
    dormir_activo: bool,
    fundido: f64,
    velocidad: f64,
    ab_a: f64,
    ab_b: f64,
    indice: i32,
    origen: QString,
}

impl Default for RepRust {
    fn default() -> Self {
        let vacio = || json_a_variante("{}");
        let lista = || json_a_variante("[]");
        RepRust {
            cancion: vacio(),
            posicion: 0.0,
            duracion: 0.0,
            pausado: true,
            volumen: 80,
            aleatorio: false,
            repetir: QString::from("no"),
            hay_anterior: false,
            hay_siguiente: false,
            cola: lista(),
            cola_siguientes: lista(),
            cola_restantes: 0,
            dormir_texto: QString::default(),
            dormir_activo: false,
            fundido: 0.0,
            velocidad: 1.0,
            ab_a: -1.0,
            ab_b: -1.0,
            indice: -1,
            origen: QString::default(),
        }
    }
}

fn i32_de(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

/// Lo que cambió en el reproductor, sacado en el hilo de la interfaz y aplicado después (con `queue`), para
/// no llamar a Qt con el reproductor prestado.
/// (cola, las siguientes, cuántas quedan, índice, origen).
type FotoCola = (Vec<Value>, Vec<Value>, i64, i64, String);

#[derive(Default)]
pub struct Foto {
    cancion: Option<Value>,
    posicion: Option<(f64, f64)>,
    estado: Option<(bool, i64, bool, String, f64)>,
    navegar: (bool, bool),
    cola: Option<FotoCola>,
    dormir: Option<(String, bool)>,
    practica: Option<(f64, f64, f64)>,
    errores: Vec<String>,
}

impl Foto {
    pub fn de(r: &Reproductor, avisos: &[Aviso]) -> Self {
        let hay = |a: &Aviso| avisos.contains(a);
        Foto {
            cancion: hay(&Aviso::Cancion).then(|| r.cancion()),
            posicion: hay(&Aviso::Posicion).then(|| (r.posicion(), r.duracion())),
            estado: hay(&Aviso::Estado).then(|| {
                (
                    r.pausado(),
                    r.volumen(),
                    r.aleatorio(),
                    r.repetir().to_string(),
                    r.fundido(),
                )
            }),
            navegar: (r.hay_anterior(), r.hay_siguiente()),
            cola: hay(&Aviso::Cola).then(|| {
                (
                    r.cola(),
                    r.cola_siguientes(),
                    r.cola_restantes(),
                    r.indice(),
                    r.origen().to_string(),
                )
            }),
            dormir: hay(&Aviso::Dormir).then(|| (r.dormir_texto(), r.dormir_activo())),
            practica: hay(&Aviso::Practica).then(|| (r.velocidad(), r.ab_a(), r.ab_b())),
            errores: avisos
                .iter()
                .filter_map(|a| match a {
                    Aviso::Error(t) => Some(t.clone()),
                    _ => None,
                })
                .collect(),
        }
    }

    /// Todo (al arrancar).
    pub fn completa(r: &Reproductor) -> Self {
        let todos = [
            Aviso::Cancion,
            Aviso::Posicion,
            Aviso::Estado,
            Aviso::Cola,
            Aviso::Dormir,
            Aviso::Practica,
        ];
        Self::de(r, &todos)
    }
}

impl qobject::Rep {
    pub fn aplicar(mut self: Pin<&mut Self>, f: Foto) {
        if let Some((p, d)) = f.posicion {
            self.as_mut().set_posicion(p);
            self.as_mut().set_duracion(d);
        }
        if let Some((pausado, volumen, aleatorio, repetir, fundido)) = f.estado {
            self.as_mut().set_pausado(pausado);
            self.as_mut().set_volumen(i32_de(volumen));
            self.as_mut().set_aleatorio(aleatorio);
            self.as_mut().set_repetir(QString::from(repetir.as_str()));
            self.as_mut().set_fundido(fundido);
        }
        if let Some((cola, siguientes, restantes, indice, origen)) = f.cola {
            self.as_mut()
                .set_cola(json_a_variante(&Value::Array(cola).to_string()));
            self.as_mut()
                .set_cola_siguientes(json_a_variante(&Value::Array(siguientes).to_string()));
            self.as_mut().set_cola_restantes(i32_de(restantes));
            self.as_mut().set_indice(i32_de(indice));
            self.as_mut().set_origen(QString::from(origen.as_str()));
        }
        self.as_mut().set_hay_anterior(f.navegar.0);
        self.as_mut().set_hay_siguiente(f.navegar.1);
        if let Some((texto, activo)) = f.dormir {
            self.as_mut().set_dormir_texto(QString::from(texto.as_str()));
            self.as_mut().set_dormir_activo(activo);
        }
        if let Some((v, a, b)) = f.practica {
            self.as_mut().set_velocidad(v);
            self.as_mut().set_ab_a(a);
            self.as_mut().set_ab_b(b);
        }
        if let Some(c) = f.cancion {
            self.as_mut().set_cancion(json_a_variante(&c.to_string()));
            self.as_mut().cambio_cancion();
        }
        for e in f.errores {
            self.as_mut().error(&QString::from(e.as_str()));
        }
    }

    fn alternar(&self) {
        con_rep(|r| r.alternar());
    }
    fn pausar(&self) {
        con_rep(|r| r.pausar());
    }
    fn seguir(&self) {
        con_rep(|r| r.seguir());
    }
    fn siguiente(&self) {
        con_rep(|r| r.siguiente());
    }
    fn anterior(&self) {
        con_rep(|r| r.anterior());
    }
    fn buscar(&self, segundos: f64) {
        con_rep(|r| r.buscar(segundos));
    }
    fn poner_volumen(&self, v: i32) {
        con_rep(|r| r.poner_volumen(i64::from(v)));
    }
    fn alternar_aleatorio(&self) {
        con_rep(|r| r.alternar_aleatorio());
    }
    fn alternar_repetir(&self) {
        con_rep(|r| r.alternar_repetir());
    }
    fn saltar_a(&self, k: i32) {
        con_rep(|r| r.saltar_a(i64::from(k)));
    }
    fn quitar_de_cola_en(&self, k: i32) {
        con_rep(|r| r.quitar_de_cola_en(i64::from(k)));
    }
    fn quitar_de_cola(&self, ruta: &QString) {
        let ruta = ruta.to_string();
        con_rep(|r| r.quitar_de_cola(&ruta));
    }
    fn dormir(&self, minutos: i32) {
        con_rep(|r| r.dormir(i64::from(minutos)));
    }
    fn poner_velocidad(&self, v: f64) {
        con_rep(|r| r.poner_velocidad(v));
    }
    fn marcar_ab(&self) {
        con_rep(|r| r.marcar_ab());
    }
    fn quitar_ab(&self) {
        con_rep(|r| r.quitar_ab());
    }
}

impl cxx_qt::Initialize for qobject::Rep {
    fn initialize(self: Pin<&mut Self>) {
        crate::motor::registrar_rep(self.qt_thread());
    }
}
