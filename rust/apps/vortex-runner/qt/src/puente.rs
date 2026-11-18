//! `runner`: lo mismo que `puente.py`, envolviendo `vortex_runner::estado::Estado` (el motor ya probado).
//! Mismos nombres de propiedades, señales y slots, para que los QML de la app de Python no cambien.
//!
//! Cada llamada al motor deja eventos (avisos, «la lista cambió»…); `vaciar` los convierte en las
//! propiedades y señales de QML.
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
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QList_QVariant, programas)]
        #[qproperty(QStringList, falta_motor, cxx_name = "faltaMotor")]
        #[qproperty(bool, instalando_motor, cxx_name = "instalandoMotor")]
        #[qproperty(QString, instalando)]
        #[qproperty(QList_QVariant, protones)]
        #[qproperty(QString, carpeta_datos, cxx_name = "carpetaDatos")]
        type Puente = super::PuenteRust;

        /// texto, tipo (info | error | exito)
        #[qsignal]
        fn aviso(self: Pin<&mut Puente>, texto: &QString, tipo: &QString);
        /// los .exe que dejó un instalador, para elegir
        #[qsignal]
        #[cxx_name = "elegirInstalado"]
        fn elegir_instalado(self: Pin<&mut Puente>, lista: &QList_QVariant);
        /// ruta, nombre, parece instalador
        #[qsignal]
        #[cxx_name = "preguntarArchivo"]
        fn preguntar_archivo(self: Pin<&mut Puente>, ruta: &QString, nombre: &QString, instalador: bool);
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        fn agregar(self: Pin<&mut Puente>, url: &QString) -> QString;
        #[qinvokable]
        fn instalar(self: Pin<&mut Puente>, url: &QString);
        #[qinvokable]
        #[cxx_name = "agregarInstalado"]
        fn agregar_instalado(self: Pin<&mut Puente>, exe: &QString, nombre: &QString);
        #[qinvokable]
        #[cxx_name = "descartarInstalado"]
        fn descartar_instalado(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "abrirArchivo"]
        fn abrir_archivo(self: Pin<&mut Puente>, ruta: &QString);
        #[qinvokable]
        fn jugar(self: Pin<&mut Puente>, pid: &QString);
        #[qinvokable]
        fn detener(self: Pin<&mut Puente>, pid: &QString);
        #[qinvokable]
        fn guardar(
            self: Pin<&mut Puente>,
            pid: &QString,
            nombre: &QString,
            proton: &QString,
            argumentos: &QString,
        );
        #[qinvokable]
        #[cxx_name = "cambiarExe"]
        fn cambiar_exe(self: Pin<&mut Puente>, pid: &QString, url: &QString);
        #[qinvokable]
        fn lanzador(self: Pin<&mut Puente>, pid: &QString, poner: bool);
        #[qinvokable]
        #[cxx_name = "abrirCarpeta"]
        fn abrir_carpeta(self: Pin<&mut Puente>, pid: &QString);
        #[qinvokable]
        #[cxx_name = "abrirDiscoC"]
        fn abrir_disco_c(self: Pin<&mut Puente>, pid: &QString);
        #[qinvokable]
        #[cxx_name = "configuracionWine"]
        fn configuracion_wine(self: Pin<&mut Puente>, pid: &QString);
        #[qinvokable]
        fn quitar(self: Pin<&mut Puente>, pid: &QString, borrar_prefijo: bool);
        #[qinvokable]
        #[cxx_name = "instalarMotor"]
        fn instalar_motor(self: Pin<&mut Puente>);
        #[qinvokable]
        fn refrescar(self: Pin<&mut Puente>);
        /// Mira qué programas siguen abiertos (la ventana lo llama sola cada 3 s; se puede llamar a mano).
        #[qinvokable]
        fn revisar(self: Pin<&mut Puente>);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QStringList, QUrl, QVariant};
use std::collections::BTreeSet;
use std::time::Duration;
use vortex_runner::estado::{Estado, Evento, Fila, Instalado, abrir_con_xdg};
use vortex_runner::{Contexto, correr, instancia};

pub struct PuenteRust {
    estado: Estado,
    programas: QList<QVariant>,
    falta_motor: QStringList,
    instalando_motor: bool,
    instalando: QString,
    protones: QList<QVariant>,
    carpeta_datos: QString,
}

impl Default for PuenteRust {
    fn default() -> Self {
        let estado = Estado::new(Contexto::del_entorno());
        PuenteRust {
            carpeta_datos: q(&estado.carpeta_datos().to_string_lossy()),
            estado,
            programas: QList::default(),
            falta_motor: QStringList::default(),
            instalando_motor: false,
            instalando: QString::default(),
            protones: QList::default(),
        }
    }
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn mapa(pares: Vec<(&str, QVariant)>) -> QVariant {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in pares {
        m.insert(q(k), v);
    }
    QVariant::from(&m)
}

fn texto(s: &str) -> QVariant {
    QVariant::from(&q(s))
}

fn url_de(ruta: &str) -> QString {
    QUrl::from_local_file(&q(ruta)).to_qstring()
}

fn de_fila(f: &Fila) -> QVariant {
    mapa(vec![
        ("id", texto(&f.id)),
        ("nombre", texto(&f.nombre)),
        ("exe", texto(&f.exe)),
        ("prefijo", texto(&f.prefijo)),
        ("proton", texto(&f.proton)),
        ("argumentos", texto(&f.argumentos)),
        (
            "icono",
            QVariant::from(&if f.icono.is_empty() {
                QString::default()
            } else {
                url_de(&f.icono)
            }),
        ),
        ("abierto", QVariant::from(&f.abierto)),
        ("subtitulo", texto(&f.subtitulo)),
        ("lanzador", QVariant::from(&f.lanzador)),
        ("existe", QVariant::from(&f.existe)),
        ("ultima", QVariant::from(&f.ultima)),
    ])
}

fn de_instalado(i: &Instalado) -> QVariant {
    mapa(vec![
        ("exe", texto(&i.exe)),
        ("nombre", texto(&i.nombre)),
        ("carpeta", texto(&i.carpeta)),
    ])
}

fn lista(v: impl IntoIterator<Item = QVariant>) -> QList<QVariant> {
    let mut l = QList::<QVariant>::default();
    for x in v {
        l.append(x);
    }
    l
}

impl qobject::Puente {
    /// Pasa lo que dejó el motor a QML: propiedades y señales.
    fn vaciar(mut self: Pin<&mut Self>) {
        let eventos = self.as_mut().rust_mut().estado.tomar_eventos();
        for ev in eventos {
            match ev {
                Evento::ProgramasCambio => {
                    let l = lista(self.rust().estado.programas().iter().map(de_fila));
                    self.as_mut().set_programas(l);
                }
                Evento::MotorCambio => self.as_mut().sincronizar_motor(),
                Evento::InstalandoCambio => {
                    let n = q(self.rust().estado.instalando());
                    self.as_mut().set_instalando(n);
                }
                Evento::Aviso(t, k) => self.as_mut().aviso(&q(&t), &q(&k)),
                Evento::ElegirInstalado(l) => {
                    let l = lista(l.iter().map(de_instalado));
                    self.as_mut().elegir_instalado(&l);
                }
                Evento::PreguntarArchivo {
                    ruta,
                    nombre,
                    instalador,
                } => {
                    self.as_mut()
                        .preguntar_archivo(&q(&ruta), &q(&nombre), instalador);
                }
            }
        }
    }

    fn sincronizar_motor(mut self: Pin<&mut Self>) {
        let mut falta = QList::<QString>::default();
        for f in self.rust().estado.falta_motor() {
            falta.append(q(f));
        }
        let protones = lista(
            self.rust()
                .estado
                .protones_para_ui()
                .into_iter()
                .map(|(id, nombre)| mapa(vec![("id", texto(&id)), ("nombre", texto(&nombre))])),
        );
        let instalando = self.rust().estado.instalando_motor();
        self.as_mut().set_falta_motor(QStringList::from(&falta));
        self.as_mut().set_protones(protones);
        self.as_mut().set_instalando_motor(instalando);
    }

    /// Con los prefijos que ya tienen procesos (se miran fuera del hilo de la ventana).
    fn revisar_con(mut self: Pin<&mut Self>, abiertos: &BTreeSet<String>) {
        self.as_mut().rust_mut().estado.revisar(abiertos);
        self.vaciar();
    }

    fn agregar(mut self: Pin<&mut Self>, url: &QString) -> QString {
        let id = self.as_mut().rust_mut().estado.agregar(&url.to_string());
        self.vaciar();
        q(&id)
    }

    fn instalar(mut self: Pin<&mut Self>, url: &QString) {
        self.as_mut().rust_mut().estado.instalar(&url.to_string());
        self.vaciar();
    }

    fn agregar_instalado(mut self: Pin<&mut Self>, exe: &QString, nombre: &QString) {
        self.as_mut()
            .rust_mut()
            .estado
            .agregar_instalado(&exe.to_string(), &nombre.to_string());
        self.vaciar();
    }

    fn descartar_instalado(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().estado.descartar_instalado();
        self.vaciar();
    }

    fn abrir_archivo(mut self: Pin<&mut Self>, ruta: &QString) {
        self.as_mut().rust_mut().estado.abrir_archivo(&ruta.to_string());
        self.vaciar();
    }

    fn jugar(mut self: Pin<&mut Self>, pid: &QString) {
        self.as_mut().rust_mut().estado.jugar(&pid.to_string());
        self.vaciar();
    }

    fn detener(mut self: Pin<&mut Self>, pid: &QString) {
        self.as_mut().rust_mut().estado.detener(&pid.to_string());
        self.vaciar();
    }

    fn guardar(
        mut self: Pin<&mut Self>,
        pid: &QString,
        nombre: &QString,
        proton: &QString,
        argumentos: &QString,
    ) {
        self.as_mut().rust_mut().estado.guardar(
            &pid.to_string(),
            &nombre.to_string(),
            &proton.to_string(),
            &argumentos.to_string(),
        );
        self.vaciar();
    }

    fn cambiar_exe(mut self: Pin<&mut Self>, pid: &QString, url: &QString) {
        self.as_mut()
            .rust_mut()
            .estado
            .cambiar_exe(&pid.to_string(), &url.to_string());
        self.vaciar();
    }

    fn lanzador(mut self: Pin<&mut Self>, pid: &QString, poner: bool) {
        self.as_mut().rust_mut().estado.lanzador(&pid.to_string(), poner);
        self.vaciar();
    }

    fn abrir_carpeta(self: Pin<&mut Self>, pid: &QString) {
        if let Some(c) = self.rust().estado.carpeta_exe(&pid.to_string()) {
            abrir_con_xdg(&c);
        }
    }

    fn abrir_disco_c(self: Pin<&mut Self>, pid: &QString) {
        if let Some(c) = self.rust().estado.disco_c(&pid.to_string()) {
            abrir_con_xdg(&c);
        }
    }

    fn configuracion_wine(mut self: Pin<&mut Self>, pid: &QString) {
        self.as_mut()
            .rust_mut()
            .estado
            .configuracion_wine(&pid.to_string());
        self.vaciar();
    }

    fn quitar(mut self: Pin<&mut Self>, pid: &QString, borrar_prefijo: bool) {
        self.as_mut()
            .rust_mut()
            .estado
            .quitar(&pid.to_string(), borrar_prefijo);
        self.vaciar();
    }

    fn instalar_motor(mut self: Pin<&mut Self>) {
        if !self.as_mut().rust_mut().estado.iniciar_motor() {
            return;
        }
        let ctx = self.rust().estado.ctx.clone();
        self.as_mut().vaciar();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let (codigo, detalle) = match correr::instalar_motor(&ctx) {
                Ok(r) => (r.codigo, r.detalle),
                Err(e) => (1, e.to_string()), // pkexec no está, se cortó…
            };
            let _ = hilo.queue(move |mut obj| {
                obj.as_mut().rust_mut().estado.fin_motor(codigo, &detalle);
                obj.vaciar();
            });
        });
    }

    fn refrescar(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().estado.refrescar();
        self.vaciar();
    }

    fn revisar(self: Pin<&mut Self>) {
        self.revisar_con(&correr::prefijos_abiertos());
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().sincronizar_motor();
        self.as_mut().vaciar();
        let hilo = self.qt_thread();
        // Cada 3 s: ¿qué sigue abierto? (se mira en este hilo; solo el resultado pasa a la ventana)
        std::thread::spawn({
            let hilo = hilo.clone();
            move || {
                loop {
                    std::thread::sleep(Duration::from_secs(3));
                    let abiertos = correr::prefijos_abiertos();
                    if hilo.queue(move |obj| obj.revisar_con(&abiertos)).is_err() {
                        break; // la ventana ya se cerró
                    }
                }
            }
        });
        // Una sola ventana: lo que pida otra copia (mostrar, o abrir un .exe) llega por un socket.
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                for m in escucha.revisar() {
                    let r = hilo.queue(move |mut obj| {
                        obj.as_mut().mostrar_ventana();
                        if let instancia::Mensaje::Abrir(ruta) = m {
                            obj.abrir_archivo(&q(&ruta));
                        }
                    });
                    if r.is_err() {
                        return;
                    }
                }
            }
        });
    }
}
