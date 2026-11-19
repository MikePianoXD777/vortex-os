//! `ia` (General): el estado de Ollama, la gráfica, la carpeta del vault y los avisos. Lo mismo que la clase
//! `General` de `puente.py`: mismos nombres de propiedades, señales y slots, para que los QML de la de Python
//! no cambien.
//!
//! Lo que tarda (preguntarle a Ollama, arrancarlo, la pregunta por el motor) va en un hilo aparte y el
//! resultado vuelve al hilo de la interfaz con `qt_thread().queue`.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, estado_ollama, cxx_name = "estadoOllama", READ, NOTIFY = estado_cambio)]
        #[qproperty(QString, mensaje_ollama, cxx_name = "mensajeOllama", READ, NOTIFY = estado_cambio)]
        #[qproperty(QString, url_ollama, cxx_name = "urlOllama", READ, CONSTANT)]
        #[qproperty(QString, gpu, READ, NOTIFY = gpu_cambio)]
        #[qproperty(QString, carpeta_vault, cxx_name = "carpetaVault", READ, NOTIFY = config_cambio)]
        #[qproperty(QString, modelo_memoria, cxx_name = "modeloMemoria", READ, WRITE = escribir_modelo_memoria, NOTIFY = config_cambio)]
        #[qproperty(
            QString,
            embeddings_recomendado,
            cxx_name = "embeddingsRecomendado",
            READ,
            CONSTANT
        )]
        #[qproperty(QString, version, READ, CONSTANT)]
        type General = super::GeneralRust;

        /// texto, y el tipo: `info`, `error` o `exito`
        #[qsignal]
        fn aviso(self: Pin<&mut General>, texto: &QString, tipo: &QString);
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut General>);
        #[qsignal]
        #[cxx_name = "irA"]
        fn ir_a(self: Pin<&mut General>, pagina: &QString);
        #[qsignal]
        #[cxx_name = "estadoCambio"]
        fn estado_cambio(self: Pin<&mut General>);
        #[qsignal]
        #[cxx_name = "gpuCambio"]
        fn gpu_cambio(self: Pin<&mut General>);
        #[qsignal]
        #[cxx_name = "configCambio"]
        fn config_cambio(self: Pin<&mut General>);
        /// La pregunta por el motor de la IA (solo al abrir, una vez por máquina).
        #[qsignal]
        #[cxx_name = "motorPregunta"]
        fn motor_pregunta(self: Pin<&mut General>, texto: &QString);

        #[cxx_name = "escribirModeloMemoria"]
        fn escribir_modelo_memoria(self: Pin<&mut General>, nombre: &QString);

        #[qinvokable]
        #[cxx_name = "revisarOllama"]
        fn revisar_ollama(self: Pin<&mut General>, arrancar: bool);
        #[qinvokable]
        #[cxx_name = "arrancarOllama"]
        fn arrancar_ollama(self: Pin<&mut General>);
        #[qinvokable]
        #[cxx_name = "responderMotor"]
        fn responder_motor(self: Pin<&mut General>, si: bool);
        #[qinvokable]
        #[cxx_name = "abrirCarpetaVault"]
        fn abrir_carpeta_vault(self: Pin<&mut General>, url: &QString);
        #[qinvokable]
        #[cxx_name = "vaultPorDefecto"]
        fn vault_por_defecto(self: Pin<&mut General>);
        #[qinvokable]
        fn copiar(self: Pin<&mut General>, texto: &QString);
        /// Lo de `Puente.arrancar()`: la bóveda, los modelos, el chat y lo general, en ese orden.
        #[qinvokable]
        fn iniciar(self: Pin<&mut General>);
        /// Se llama al salir: para el Ollama que arrancó esta ventana.
        #[qinvokable]
        fn cerrar(self: Pin<&mut General>);
    }

    impl cxx_qt::Initialize for General {}
    impl cxx_qt::Threading for General {}
}

use crate::comun::{avisar, comun, en_boveda, en_modelos, registrar_general};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use std::time::Duration;
use vortex_local_ai::catalogo;
use vortex_local_ai::instancia;

pub struct GeneralRust {
    estado_ollama: QString, // corriendo | arrancando | detenido | falta
    mensaje_ollama: QString,
    url_ollama: QString,
    gpu: QString,
    carpeta_vault: QString,
    modelo_memoria: QString,
    embeddings_recomendado: QString,
    version: QString,
    vigilando: bool,
}

impl Default for GeneralRust {
    fn default() -> Self {
        let c = comun();
        GeneralRust {
            estado_ollama: QString::from("detenido"),
            mensaje_ollama: QString::default(),
            url_ollama: QString::from(c.ollama.url.as_str()),
            gpu: QString::default(),
            carpeta_vault: QString::from(c.compartido.lock().unwrap().raiz.to_string_lossy().as_ref()),
            modelo_memoria: QString::from(c.config.lock().unwrap().texto("modelo_embeddings").as_str()),
            embeddings_recomendado: QString::from(catalogo::EMBEDDINGS),
            version: QString::from(vortex_local_ai::VERSION),
            vigilando: false,
        }
    }
}

impl cxx_qt::Initialize for qobject::General {
    fn initialize(self: Pin<&mut Self>) {
        registrar_general(self.qt_thread());
    }
}

impl qobject::General {
    fn iniciar(mut self: Pin<&mut Self>) {
        en_boveda(|b| b.arrancar());
        en_modelos(|m| m.arrancar());
        crate::comun::en_chat(|c| c.arrancar());
        self.as_mut().arrancar();
    }

    fn arrancar(mut self: Pin<&mut Self>) {
        self.as_mut().revisar_ollama(true);
        let hilo = self.qt_thread();
        // El reloj de 15 s: «¿Ollama sigue contestando?».
        let reloj = hilo.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(15));
                if reloj.queue(|g| g.revisar_ollama(false)).is_err() {
                    break;
                }
            }
        });
        // Una sola ventana: si otra copia pide «mostrar», se avisa a QML (Main.qml escucha mostrarVentana).
        if let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) {
            let h = hilo.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(250));
                    for _ in 0..escucha.revisar() {
                        if h.queue(|g| g.mostrar_ventana()).is_err() {
                            return;
                        }
                    }
                }
            });
        }
        std::thread::spawn(move || {
            if let Some(pregunta) = catalogo::motor_pendiente() {
                let _ = hilo.queue(move |g| g.motor_pregunta(&QString::from(pregunta.as_str())));
            }
        });
    }

    fn responder_motor(self: Pin<&mut Self>, si: bool) {
        if !si {
            std::thread::spawn(|| {
                catalogo::elegir_motor(false);
            });
            return;
        }
        avisar("Instalando CUDA: tarda un rato. Te pedirá tu contraseña.", "info");
        std::thread::spawn(|| match catalogo::elegir_motor(true).as_str() {
            "ok" => avisar("Listo: Ollama usará CUDA la próxima vez que arranque.", "exito"),
            "cancelado" => avisar(
                "No se instaló CUDA. Te vuelvo a preguntar la próxima vez que abras.",
                "info",
            ),
            _ => avisar(
                "No pude instalar CUDA. Sigues con Vulkan; a mano: vortex-ia motor",
                "error",
            ),
        });
    }

    /// `(estado, mensaje)`; si pasa a «corriendo», recarga los modelos.
    pub fn poner_estado(mut self: Pin<&mut Self>, estado: &str, mensaje: &str) {
        if self.rust().estado_ollama.to_string() == estado
            && self.rust().mensaje_ollama.to_string() == mensaje
        {
            return;
        }
        let antes = self.rust().estado_ollama.to_string();
        {
            let mut r = self.as_mut().rust_mut();
            r.estado_ollama = QString::from(estado);
            r.mensaje_ollama = QString::from(mensaje);
        }
        self.as_mut().estado_cambio();
        if estado == "corriendo" && antes != "corriendo" {
            en_modelos(|m| m.recargar());
        }
    }

    /// Pregunta si Ollama contesta; con `arrancar`, si no contesta lo arranca (`asegurar_servidor`).
    pub fn revisar_ollama(mut self: Pin<&mut Self>, arrancar: bool) {
        if self.rust().vigilando {
            return;
        }
        self.as_mut().rust_mut().vigilando = true;
        let o = comun().ollama.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let r: (&str, String, bool) = if o.disponible() {
                ("corriendo", String::new(), false)
            } else {
                let instalado = o.instalado();
                if !arrancar {
                    (if instalado { "detenido" } else { "falta" }, String::new(), false)
                } else if !instalado {
                    ("falta", String::new(), false)
                } else {
                    crate::comun::en_general(|g| g.poner_estado("arrancando", ""));
                    if o.asegurar_servidor(Duration::from_secs(20)) {
                        ("corriendo", String::new(), true)
                    } else {
                        ("detenido", o.ultimo_error(), false)
                    }
                }
            };
            let _ = hilo.queue(move |g| g.revisado(r.0, &r.1, r.2, arrancar));
        });
    }

    fn revisado(mut self: Pin<&mut Self>, estado: &str, error: &str, arranco: bool, arrancar: bool) {
        self.as_mut().rust_mut().vigilando = false;
        self.as_mut().poner_estado(estado, error);
        if arranco {
            avisar(
                "Arranqué Ollama, el motor que corre tus modelos. Ya puedes usarlos.",
                "exito",
            );
        } else if arrancar && estado == "falta" {
            avisar(
                "Falta Ollama, el motor que corre los modelos. Instálalo con «vortex actualizar».",
                "error",
            );
        } else if arrancar && estado == "detenido" {
            let detalle = if error.is_empty() {
                ".".to_string()
            } else {
                format!(": {error}")
            };
            avisar(
                format!("No pude arrancar Ollama{detalle} Prueba con «Arrancar» en un momento."),
                "error",
            );
        }
    }

    fn arrancar_ollama(self: Pin<&mut Self>) {
        self.revisar_ollama(true);
    }

    pub fn poner_gpu(mut self: Pin<&mut Self>, texto: &str) {
        if self.rust().gpu.to_string() != texto {
            self.as_mut().rust_mut().gpu = QString::from(texto);
            self.as_mut().gpu_cambio();
        }
    }

    /// La carpeta del vault (y el modelo de memoria) cambiaron: se avisa a QML.
    pub fn sincronizar_vault(mut self: Pin<&mut Self>) {
        let raiz = comun()
            .compartido
            .lock()
            .unwrap()
            .raiz
            .to_string_lossy()
            .to_string();
        self.as_mut().rust_mut().carpeta_vault = QString::from(raiz.as_str());
        self.as_mut().config_cambio();
    }

    fn abrir_carpeta_vault(self: Pin<&mut Self>, url: &QString) {
        let ruta = crate::arranque::ffi::url_a_local(&url.to_string());
        if !ruta.is_empty() {
            en_boveda(move |mut b| b.as_mut().usar(std::path::PathBuf::from(ruta), true));
        }
    }

    fn vault_por_defecto(self: Pin<&mut Self>) {
        let ruta = comun().rutas.vault_por_defecto.clone();
        en_boveda(move |mut b| b.as_mut().usar(ruta, true));
    }

    fn escribir_modelo_memoria(mut self: Pin<&mut Self>, nombre: &QString) {
        let nombre = nombre.to_string();
        if nombre != self.rust().modelo_memoria.to_string() {
            comun()
                .config
                .lock()
                .unwrap()
                .poner("modelo_embeddings", json!(nombre));
            self.as_mut().rust_mut().modelo_memoria = QString::from(nombre.as_str());
            self.as_mut().config_cambio();
        }
    }

    fn copiar(self: Pin<&mut Self>, texto: &QString) {
        crate::arranque::ffi::copiar_al_portapapeles(&texto.to_string());
        self.aviso(&QString::from("Copiado."), &QString::from("exito"));
    }

    fn cerrar(self: Pin<&mut Self>) {
        comun().ollama.parar_propio();
    }
}
