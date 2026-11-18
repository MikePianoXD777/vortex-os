//! `tm`: lo mismo que `puente.py`, envolviendo `vortex_task_manager::vista::Vista` (el motor ya probado).
//! Mismos nombres de slots y propiedades, para que los QML no cambien.
//!
//! Cada segundo (cada 3 si la ventana no está enfocada) un hilo aparte mide —/proc, hyprctl y
//! nvidia-smi tardan unas decenas de milisegundos— y la interfaz solo recibe el resultado. La VRAM de
//! cada proceso va en ese mismo hilo, pero solo se vuelve a medir cada 3 segundos. Las acciones que
//! pueden pedir contraseña (pkexec, systemctl del sistema) también van en otro hilo, para no congelarla.
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
        #[qproperty(QVariant, totales)]
        #[qproperty(QVariant, resumen)]
        #[qproperty(bool, vram_disponible, cxx_name = "vramDisponible")]
        #[qproperty(QVariant, rendimiento)]
        #[qproperty(QVariant, historia)]
        #[qproperty(QVariant, servicios_usuario, cxx_name = "serviciosUsuario")]
        #[qproperty(QVariant, servicios_sistema, cxx_name = "serviciosSistema")]
        #[qproperty(bool, cargando_servicios, cxx_name = "cargandoServicios")]
        #[qproperty(QString, filtro)]
        #[qproperty(QString, orden)]
        #[qproperty(bool, ver_procesos, cxx_name = "verProcesos")]
        #[qproperty(bool, enfocada)]
        type Puente = super::PuenteRust;

        #[qsignal]
        fn aviso(self: Pin<&mut Puente>, texto: QString);
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        #[cxx_name = "alternarGrupo"]
        fn alternar_grupo(self: Pin<&mut Puente>, clave: &QString);
        #[qinvokable]
        #[cxx_name = "alternarSeccion"]
        fn alternar_seccion(self: Pin<&mut Puente>, seccion: &QString);
        #[qinvokable]
        fn detalles(self: &Puente, clave: &QString) -> QVariant;
        #[qinvokable]
        fn finalizar(self: Pin<&mut Puente>, clave: &QString, forzar: bool);
        #[qinvokable]
        #[cxx_name = "cerrarVentana"]
        fn cerrar_ventana(self: Pin<&mut Puente>, clave: &QString);
        #[qinvokable]
        fn suspender(self: Pin<&mut Puente>, clave: &QString, si: bool);
        #[qinvokable]
        #[cxx_name = "ponerPrioridad"]
        fn poner_prioridad(self: Pin<&mut Puente>, clave: &QString, nivel: &QString);
        #[qinvokable]
        #[cxx_name = "abrirUbicacion"]
        fn abrir_ubicacion(self: Pin<&mut Puente>, clave: &QString);
        #[qinvokable]
        fn copiar(self: &Puente, texto: &QString);
        #[qinvokable]
        #[cxx_name = "cargarServicios"]
        fn cargar_servicios(self: Pin<&mut Puente>);
        #[qinvokable]
        fn servicio(self: Pin<&mut Puente>, unidad: &QString, que: &QString, usuario: bool);
        /// Solo para el modo de pruebas (`VORTEX_TASK_MANAGER_PRUEBA`): un paso del guion de la prueba.
        #[qinvokable]
        #[cxx_name = "pruebaPaso"]
        fn prueba_paso(self: Pin<&mut Puente>, paso: i32) -> QString;
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use crate::arranque::ffi;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use vortex_task_manager::grupos::agrupar;
use vortex_task_manager::lanzadores::Aplicaciones;
use vortex_task_manager::procesos::Procesos;
use vortex_task_manager::rendimiento::Rendimiento;
use vortex_task_manager::vista::{Actualizacion, Muestra, Vista};
use vortex_task_manager::vram::MemoriaGrafica;
use vortex_task_manager::{acciones, instancia, procesos, servicios, sys};

/// Lo que la interfaz le dice al hilo de muestreo.
struct Control {
    enfocada: AtomicBool,
    ver_procesos: AtomicBool,
    despertar: AtomicBool,
}

pub struct PuenteRust {
    vista: Vista,
    control: Arc<Control>,
    prueba: serde_json::Map<String, Value>,
    totales: QVariant,
    resumen: QVariant,
    vram_disponible: bool,
    rendimiento: QVariant,
    historia: QVariant,
    servicios_usuario: QVariant,
    servicios_sistema: QVariant,
    cargando_servicios: bool,
    filtro: QString,
    orden: QString,
    ver_procesos: bool,
    enfocada: bool,
}

impl Default for PuenteRust {
    fn default() -> Self {
        let vista = Vista::new();
        let (totales, historia) = (a_variant(&vista.totales), a_variant(&vista.historia.valor));
        PuenteRust {
            control: Arc::new(Control {
                enfocada: AtomicBool::new(true),
                ver_procesos: AtomicBool::new(true),
                despertar: AtomicBool::new(false),
            }),
            vista,
            prueba: serde_json::Map::new(),
            totales,
            resumen: ffi::vortex_tm_json_a_variant("{}"),
            vram_disponible: false,
            rendimiento: ffi::vortex_tm_json_a_variant("{}"),
            historia,
            servicios_usuario: ffi::vortex_tm_json_a_variant("[]"),
            servicios_sistema: ffi::vortex_tm_json_a_variant("[]"),
            cargando_servicios: false,
            filtro: QString::default(),
            orden: QString::from("nombre"),
            ver_procesos: true,
            enfocada: true,
        }
    }
}

fn a_variant(v: &Value) -> QVariant {
    ffi::vortex_tm_json_a_variant(&v.to_string())
}

fn icono_valido(n: &str) -> bool {
    ffi::vortex_tm_icono_en_tema(n)
}

/// Lo que mide el hilo de muestreo (`_muestrear` de Python), con la misma cadencia: cada segundo, cada 3
/// si la ventana no está enfocada, y los procesos solo si se ven (o cada 5 s).
fn bucle_muestreo(hilo: cxx_qt::CxxQtThread<qobject::Puente>, control: Arc<Control>) {
    let apps = Aplicaciones::new();
    let mut procesos = Procesos::new();
    let mut rend = Rendimiento::new();
    let mut vram = MemoriaGrafica::new();
    let mi_uid = sys::uid();
    let (mut ticks, mut tuvo_procesos, mut n, mut hilos) = (0u64, false, 0usize, 0u64);
    loop {
        ticks += 1;
        let forzado = control.despertar.swap(false, Ordering::SeqCst);
        if forzado || control.enfocada.load(Ordering::SeqCst) || ticks % 3 == 0 {
            let con_procesos =
                control.ver_procesos.load(Ordering::SeqCst) || !tuvo_procesos || ticks % 5 == 0;
            let grupos = con_procesos.then(|| {
                let mut filas = procesos.muestrear();
                let pids: Vec<i32> = filas.iter().map(|f| f.pid).collect();
                let por_pid = vram.medir(&pids);
                for f in &mut filas {
                    f.vram = por_pid.get(&f.pid).copied().unwrap_or(0);
                }
                n = filas.len();
                hilos = filas.iter().map(|f| u64::from(f.hilos)).sum();
                tuvo_procesos = true;
                agrupar(&filas, &acciones::ventanas_hyprland(), &apps, mi_uid)
            });
            let muestra = Muestra {
                grupos,
                rend: rend.muestrear(n, hilos),
                vram_disponible: vram.disponible,
                hilos,
                procesos: n,
            };
            if hilo.queue(move |obj| obj.al_llegar(muestra)).is_err() {
                return; // la ventana ya se cerró
            }
        }
        // Un segundo, en pedacitos: una acción (finalizar…) o volver a la lista adelantan la siguiente muestra.
        for _ in 0..20 {
            if control.despertar.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn abrir(ruta: &std::path::Path) {
    // Como QDesktopServices.openUrl: la app que la persona tenga para ese tipo de archivo.
    let _ = std::process::Command::new("xdg-open")
        .arg(ruta)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

impl qobject::Puente {
    /// Publica lo nuevo de la lista en el modelo de Qt.
    fn publicar_filas(&self, a: &Actualizacion) {
        let ops: Vec<Value> = a.ops.iter().map(|o| json!([o.i1, o.i2, o.j1, o.j2])).collect();
        ffi::vortex_tm_filas_actualizar(
            &Value::Array(ops).to_string(),
            &Value::Array(a.filas.clone()).to_string(),
        );
    }

    fn reconstruir(mut self: Pin<&mut Self>) {
        let a = self.as_mut().rust_mut().vista.reconstruir(&mut icono_valido);
        self.publicar_filas(&a);
        let resumen = a_variant(&self.rust().vista.resumen);
        self.as_mut().set_resumen(resumen);
    }

    fn al_llegar(mut self: Pin<&mut Self>, m: Muestra) {
        let con_procesos = m.grupos.is_some();
        let a = self
            .as_mut()
            .rust_mut()
            .vista
            .al_llegar(m, std::process::id() as i32, &mut icono_valido);
        if let Some(a) = a {
            self.publicar_filas(&a);
        }
        let (totales, resumen, vram, rend, historia) = {
            let v = &self.rust().vista;
            (
                a_variant(&v.totales),
                a_variant(&v.resumen),
                v.vram_disponible,
                a_variant(&v.rendimiento),
                a_variant(&v.historia.valor),
            )
        };
        if con_procesos {
            self.as_mut().set_totales(totales);
            self.as_mut().set_resumen(resumen);
            self.as_mut().set_vram_disponible(vram);
        }
        self.as_mut().set_rendimiento(rend);
        self.as_mut().set_historia(historia);
    }

    /// Una acción que puede tardar (o pedir contraseña) en otro hilo; al terminar avisa si falló y mide ya.
    fn en_fondo(
        self: Pin<&mut Self>,
        trabajo: impl FnOnce() -> Option<String> + Send + 'static,
        recargar_servicios: bool,
    ) {
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let error = trabajo();
            let _ = hilo.queue(move |mut obj| {
                if let Some(e) = error.filter(|e| !e.is_empty()) {
                    obj.as_mut().aviso(QString::from(e.as_str()));
                }
                obj.rust().control.despertar.store(true, Ordering::SeqCst);
                if recargar_servicios {
                    obj.cargar_servicios();
                }
            });
        });
    }

    fn alternar_grupo(mut self: Pin<&mut Self>, clave: &QString) {
        self.as_mut().rust_mut().vista.alternar_grupo(&clave.to_string());
        self.reconstruir();
    }

    fn alternar_seccion(mut self: Pin<&mut Self>, seccion: &QString) {
        self.as_mut()
            .rust_mut()
            .vista
            .alternar_seccion(&seccion.to_string());
        self.reconstruir();
    }

    fn detalles(&self, clave: &QString) -> QVariant {
        a_variant(&self.rust().vista.detalles(&clave.to_string()))
    }

    /// Finalizar tarea: la app y todos sus procesos (SIGTERM; con «forzar», SIGKILL).
    fn finalizar(self: Pin<&mut Self>, clave: &QString, forzar: bool) {
        let mut pids = self.rust().vista.pids(&clave.to_string());
        let yo = std::process::id() as i32;
        if !forzar {
            pids.retain(|p| *p != yo); // que «finalizar» el grupo donde estoy no me cierre a medias
        }
        if !pids.is_empty() {
            self.en_fondo(move || acciones::terminar(&pids, forzar), false);
        }
    }

    fn cerrar_ventana(self: Pin<&mut Self>, clave: &QString) {
        let ventanas = self.rust().vista.ventanas_de(&clave.to_string());
        if !ventanas.is_empty() {
            self.en_fondo(
                move || {
                    acciones::cerrar_ventanas(&ventanas);
                    None
                },
                false,
            );
        }
    }

    fn suspender(self: Pin<&mut Self>, clave: &QString, si: bool) {
        let yo = std::process::id() as i32;
        let pids: Vec<i32> = self
            .rust()
            .vista
            .pids(&clave.to_string())
            .into_iter()
            .filter(|p| *p != yo)
            .collect();
        if !pids.is_empty() {
            self.en_fondo(move || acciones::suspender(&pids, si), false);
        }
    }

    fn poner_prioridad(self: Pin<&mut Self>, clave: &QString, nivel: &QString) {
        let nivel = nivel.to_string();
        if acciones::PRIORIDADES.iter().any(|(n, _)| *n == nivel) {
            let pids = self.rust().vista.pids(&clave.to_string());
            self.en_fondo(move || acciones::prioridad(&pids, &nivel), false);
        }
    }

    fn abrir_ubicacion(mut self: Pin<&mut Self>, clave: &QString) {
        let pids = self.rust().vista.pids(&clave.to_string());
        let ruta = pids
            .first()
            .map(|p| procesos::ubicacion(*p))
            .filter(|r| !r.is_empty());
        match ruta {
            Some(r) => {
                if let Some(carpeta) = std::path::Path::new(&r).parent() {
                    abrir(carpeta);
                }
            }
            None => self.as_mut().aviso(QString::from(
                "No se puede ver de dónde es ese proceso (es del sistema).",
            )),
        }
    }

    fn copiar(&self, texto: &QString) {
        ffi::vortex_tm_copiar(&texto.to_string());
    }

    fn cargar_servicios(mut self: Pin<&mut Self>) {
        if *self.cargando_servicios() {
            return;
        }
        self.as_mut().set_cargando_servicios(true);
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let usuario = a_variant(&Value::Array(servicios::listar(true)));
            let sistema = a_variant(&Value::Array(servicios::listar(false)));
            let _ = hilo.queue(move |mut obj| {
                obj.as_mut().set_servicios_usuario(usuario);
                obj.as_mut().set_servicios_sistema(sistema);
                obj.as_mut().set_cargando_servicios(false);
            });
        });
    }

    fn servicio(self: Pin<&mut Self>, unidad: &QString, que: &QString, usuario: bool) {
        let (unidad, que) = (unidad.to_string(), que.to_string());
        self.en_fondo(move || servicios::accion(&unidad, &que, usuario), true);
    }

    /// El guion de `pruebas/test_interfaz.py`, un paso a la vez (lo llama el arranque en C++ con un temporizador).
    fn prueba_paso(mut self: Pin<&mut Self>, paso: i32) -> QString {
        let nombre = std::env::var("VORTEX_TASK_MANAGER_PRUEBA_NOMBRE").unwrap_or_default();
        let pid: i32 = std::env::var("VORTEX_TASK_MANAGER_PRUEBA_PID")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(-1);
        let filas = self.as_mut().rust_mut().vista.filas_ahora(&mut icono_valido);
        let grupos_fila = |f: &[Value]| f.iter().filter(|x| x["tipo"] == "grupo").count();
        match paso {
            0 => {
                let secciones: Vec<&Value> = filas
                    .iter()
                    .filter(|f| f["tipo"] == "seccion")
                    .map(|f| &f["nombre"])
                    .collect();
                let d = &mut self.as_mut().rust_mut().prueba;
                d.insert("secciones".into(), json!(secciones));
                d.insert("grupos".into(), json!(grupos_fila(&filas)));
                d.insert(
                    "vramFilas".into(),
                    json!(
                        filas
                            .iter()
                            .filter(|f| f["tipo"] != "seccion")
                            .all(|f| f.get("vram").is_some())
                    ),
                );
                self.as_mut().set_filtro(QString::from(nombre.as_str()));
            }
            1 => {
                let grupos: Vec<&Value> = filas.iter().filter(|f| f["tipo"] == "grupo").collect();
                let vista = &self.rust().vista;
                let solo_el_mio = grupos.len() == 1
                    && vista
                        .pids(grupos[0]["clave"].as_str().unwrap_or(""))
                        .contains(&pid);
                let clave = format!(
                    "{}/{}",
                    grupos.first().map_or("", |g| g["clave"].as_str().unwrap_or("")),
                    pid
                );
                let detalles = vista.detalles(&clave)["pid"] == json!(pid);
                let d = &mut self.as_mut().rust_mut().prueba;
                d.insert("filtrado".into(), json!(solo_el_mio));
                d.insert("detalles".into(), json!(detalles));
                self.as_mut().finalizar(&QString::from(clave.as_str()), false);
            }
            2 => {
                self.as_mut().set_filtro(QString::default());
                self.as_mut().set_orden(QString::from("mem"));
                let filas = self.as_mut().rust_mut().vista.filas_ahora(&mut icono_valido);
                if let Some(g) = filas
                    .iter()
                    .find(|f| f["tipo"] == "grupo" && f["n"].as_u64().unwrap_or(0) > 1)
                {
                    let clave = g["clave"].as_str().unwrap_or("").to_string();
                    self.as_mut().alternar_grupo(&QString::from(clave.as_str()));
                    let despues = self.as_mut().rust_mut().vista.filas_ahora(&mut icono_valido);
                    let abierto = despues
                        .iter()
                        .any(|f| f["tipo"] == "proceso" && f["padre"] == json!(clave));
                    self.as_mut()
                        .rust_mut()
                        .prueba
                        .insert("abierto".into(), json!(abierto));
                }
                self.as_mut().set_orden(QString::from("vram"));
                let ordenadas = self.as_mut().rust_mut().vista.filas_ahora(&mut icono_valido);
                let vrams: Vec<u64> = ordenadas
                    .iter()
                    .filter(|f| f["tipo"] == "grupo" && f["seccion"] == "fondo")
                    .map(|f| f["vram"].as_u64().unwrap_or(0))
                    .collect();
                let ok = vrams.windows(2).all(|w| w[0] >= w[1]);
                let orden = self.orden().to_string();
                let d = &mut self.as_mut().rust_mut().prueba;
                d.insert("ordenVram".into(), json!(orden));
                d.insert("vramOrdenada".into(), json!(ok));
            }
            3 => {
                let h = self.rust().vista.historia.valor["cpu"]
                    .as_array()
                    .map_or(0, Vec::len);
                let ver = *self.ver_procesos();
                let d = &mut self.as_mut().rust_mut().prueba;
                d.insert("historia".into(), json!(h));
                d.insert("verProcesos".into(), json!(ver));
            }
            _ => {
                let n = |q: &QVariant| q.value::<cxx_qt_lib::QList<QVariant>>().map_or(0, |l| l.len());
                let total = n(self.servicios_usuario()) + n(self.servicios_sistema());
                self.as_mut()
                    .rust_mut()
                    .prueba
                    .insert("servicios".into(), json!(total));
                return QString::from(Value::Object(self.rust().prueba.clone()).to_string().as_str());
            }
        }
        QString::default()
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        // La interfaz avisa si está enfocada y qué página se ve: el hilo de muestreo lo lee.
        self.as_mut()
            .on_enfocada_changed(|o| {
                o.rust()
                    .control
                    .enfocada
                    .store(o.rust().enfocada, Ordering::SeqCst)
            })
            .release();
        self.as_mut()
            .on_ver_procesos_changed(|o| {
                let v = o.rust().ver_procesos;
                o.rust().control.ver_procesos.store(v, Ordering::SeqCst);
                if v {
                    o.rust().control.despertar.store(true, Ordering::SeqCst);
                }
            })
            .release();
        self.as_mut()
            .on_filtro_changed(|mut o| {
                let f = o.filtro().to_string();
                if o.as_mut().rust_mut().vista.poner_filtro(&f) {
                    o.reconstruir();
                }
            })
            .release();
        self.as_mut()
            .on_orden_changed(|mut o| {
                let nuevo = o.orden().to_string();
                if o.as_mut().rust_mut().vista.poner_orden(&nuevo) {
                    o.reconstruir();
                } else if o.rust().vista.orden != nuevo {
                    // Un orden que no existe: se queda el de antes.
                    let antes = o.rust().vista.orden.clone();
                    o.set_orden(QString::from(antes.as_str()));
                }
            })
            .release();

        let control = self.rust().control.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || bucle_muestreo(hilo, control));

        // Una sola ventana: si otra copia pide «mostrar», se avisa a QML (Main.qml ya escucha mostrarVentana).
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                for _ in 0..escucha.revisar() {
                    if hilo.queue(|obj| obj.mostrar_ventana()).is_err() {
                        return;
                    }
                }
            }
        });
    }
}
