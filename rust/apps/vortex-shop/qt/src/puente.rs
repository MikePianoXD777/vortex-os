//! `tienda`: lo mismo que `puente.py`, envolviendo `vortex_shop::tienda::Tienda` (el motor ya probado).
//! Mismos nombres de propiedades, señales y slots, para que los QML de la app de Python no cambien.
//!
//! La tienda guarda cada propiedad como JSON; cuando un grupo cambia, `sincronizar` las vuelve a leer y las
//! pone como propiedades de Qt (los valores complejos viajan como `QVariant` que sale de `json.h`).
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("json.h");
        /// Un valor JSON (texto) como lo entiende QML: objeto, lista, número, texto o booleano.
        fn json_a_variant(json: &QString) -> QVariant;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QVariant, estados)]
        #[qproperty(QVariant, instaladas_ids, cxx_name = "instaladasIds")]
        #[qproperty(QVariant, sistema_ids, cxx_name = "sistemaIds")]
        #[qproperty(QVariant, instaladas)]
        #[qproperty(QVariant, actualizables_ids, cxx_name = "actualizablesIds")]
        #[qproperty(QVariant, actualizables)]
        #[qproperty(QVariant, categorias)]
        #[qproperty(QVariant, recomendadas)]
        #[qproperty(QVariant, populares)]
        #[qproperty(QVariant, categoria_actual, cxx_name = "categoriaActual")]
        #[qproperty(QVariant, categoria_apps, cxx_name = "categoriaApps")]
        #[qproperty(QVariant, busqueda_apps, cxx_name = "busquedaApps")]
        #[qproperty(QVariant, detalle)]
        #[qproperty(QVariant, propias)]
        #[qproperty(QVariant, propias_estados, cxx_name = "propiasEstados")]
        #[qproperty(QVariant, android_estado, cxx_name = "androidEstado")]
        #[qproperty(QVariant, android_apps, cxx_name = "androidApps")]
        #[qproperty(QVariant, android_populares, cxx_name = "androidPopulares")]
        #[qproperty(QString, inicio_estado, cxx_name = "inicioEstado")]
        #[qproperty(QString, categoria_estado, cxx_name = "categoriaEstado")]
        #[qproperty(QString, busqueda_texto, cxx_name = "busquedaTexto")]
        #[qproperty(QString, busqueda_estado, cxx_name = "busquedaEstado")]
        #[qproperty(QString, detalle_estado, cxx_name = "detalleEstado")]
        #[qproperty(QString, android_fase, cxx_name = "androidFase")]
        #[qproperty(QString, android_paso, cxx_name = "androidPaso")]
        #[qproperty(QString, android_id, cxx_name = "androidId")]
        #[qproperty(bool, revisando_actualizaciones, cxx_name = "revisandoActualizaciones")]
        #[qproperty(bool, ocupado)]
        #[qproperty(bool, categoria_hay_mas, cxx_name = "categoriaHayMas")]
        #[qproperty(bool, categoria_cargando_mas, cxx_name = "categoriaCargandoMas")]
        #[qproperty(bool, android_cargado, cxx_name = "androidCargado")]
        #[qproperty(i32, num_actualizaciones, cxx_name = "numActualizaciones")]
        #[qproperty(i32, android_pct, cxx_name = "androidPct")]
        type Puente = super::PuenteRust;

        /// Un mensaje en español para el aviso que se cierra (algo salió mal).
        #[qsignal]
        fn aviso(self: Pin<&mut Puente>, texto: &QString);
        /// Lo mismo, pero de que algo salió bien.
        #[qsignal]
        fn listo(self: Pin<&mut Puente>, texto: &QString);
        /// Otra instancia pidió traer esta ventana al frente.
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        fn iniciar(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "reintentarTodo"]
        fn reintentar_todo(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "cargarCategorias"]
        fn cargar_categorias(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "cargarInicio"]
        fn cargar_inicio(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "abrirCategoria"]
        fn abrir_categoria(self: Pin<&mut Puente>, cat_id: &QString);
        #[qinvokable]
        #[cxx_name = "masDeCategoria"]
        fn mas_de_categoria(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "reintentarCategoria"]
        fn reintentar_categoria(self: Pin<&mut Puente>);
        #[qinvokable]
        fn buscar(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        #[cxx_name = "reintentarBusqueda"]
        fn reintentar_busqueda(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "abrirDetalle"]
        fn abrir_detalle(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        #[cxx_name = "reintentarDetalle"]
        fn reintentar_detalle(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "buscarActualizaciones"]
        fn buscar_actualizaciones(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "instalarPropia"]
        fn instalar_propia(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        #[cxx_name = "quitarPropia"]
        fn quitar_propia(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        #[cxx_name = "abrirPropia"]
        fn abrir_propia(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        #[cxx_name = "cargarAndroid"]
        fn cargar_android(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "prepararAndroid"]
        fn preparar_android(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "androidTienda"]
        fn android_tienda(self: Pin<&mut Puente>, paquete: &QString);
        #[qinvokable]
        #[cxx_name = "androidBuscar"]
        fn android_buscar(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        #[cxx_name = "androidAbrir"]
        fn android_abrir(self: Pin<&mut Puente>, paquete: &QString);
        #[qinvokable]
        #[cxx_name = "androidQuitar"]
        fn android_quitar(self: Pin<&mut Puente>, paquete: &QString, nombre: &QString);
        #[qinvokable]
        #[cxx_name = "androidPedirId"]
        fn android_pedir_id(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "androidAbrirRegistro"]
        fn android_abrir_registro(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "androidYaRegistre"]
        fn android_ya_registre(self: Pin<&mut Puente>);
        #[qinvokable]
        fn instalar(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        fn quitar(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        fn actualizar(self: Pin<&mut Puente>, app_id: &QString);
        #[qinvokable]
        #[cxx_name = "actualizarTodo"]
        fn actualizar_todo(self: Pin<&mut Puente>);
        #[qinvokable]
        fn abrir(self: Pin<&mut Puente>, app_id: &QString);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use vortex_shop::android::Android;
use vortex_shop::instancia::{self, Escucha};
use vortex_shop::motor::MotorReal;
use vortex_shop::proceso::Entorno;
use vortex_shop::propias::Propias;
use vortex_shop::tienda::{Evento, Sumidero, Tienda};

/// La carpeta de los QML (de ahí salen los íconos de las apps propias); `main` la pone antes de abrir.
pub static CARPETA_QML: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

#[derive(Default)]
pub struct PuenteRust {
    tienda: Option<Arc<Tienda>>,
    estados: QVariant,
    instaladas_ids: QVariant,
    sistema_ids: QVariant,
    instaladas: QVariant,
    actualizables_ids: QVariant,
    actualizables: QVariant,
    categorias: QVariant,
    recomendadas: QVariant,
    populares: QVariant,
    categoria_actual: QVariant,
    categoria_apps: QVariant,
    busqueda_apps: QVariant,
    detalle: QVariant,
    propias: QVariant,
    propias_estados: QVariant,
    android_estado: QVariant,
    android_apps: QVariant,
    android_populares: QVariant,
    inicio_estado: QString,
    categoria_estado: QString,
    busqueda_texto: QString,
    busqueda_estado: QString,
    detalle_estado: QString,
    android_fase: QString,
    android_paso: QString,
    android_id: QString,
    revisando_actualizaciones: bool,
    ocupado: bool,
    categoria_hay_mas: bool,
    categoria_cargando_mas: bool,
    android_cargado: bool,
    num_actualizaciones: i32,
    android_pct: i32,
}

fn texto(v: &Value) -> QString {
    QString::from(v.as_str().unwrap_or_default())
}

fn variante(v: &Value) -> QVariant {
    qobject::json_a_variant(&QString::from(v.to_string().as_str()))
}

fn entero(v: &Value) -> i32 {
    v.as_i64().map_or(0, |n| i32::try_from(n).unwrap_or(0))
}

impl qobject::Puente {
    /// Pone una propiedad de Qt con el valor que tiene la tienda.
    fn aplicar(mut self: Pin<&mut Self>, clave: &str, v: &Value) {
        match clave {
            "estados" => self.as_mut().set_estados(variante(v)),
            "instaladasIds" => self.as_mut().set_instaladas_ids(variante(v)),
            "sistemaIds" => self.as_mut().set_sistema_ids(variante(v)),
            "instaladas" => self.as_mut().set_instaladas(variante(v)),
            "actualizablesIds" => self.as_mut().set_actualizables_ids(variante(v)),
            "actualizables" => self.as_mut().set_actualizables(variante(v)),
            "categorias" => self.as_mut().set_categorias(variante(v)),
            "recomendadas" => self.as_mut().set_recomendadas(variante(v)),
            "populares" => self.as_mut().set_populares(variante(v)),
            "categoriaActual" => self.as_mut().set_categoria_actual(variante(v)),
            "categoriaApps" => self.as_mut().set_categoria_apps(variante(v)),
            "busquedaApps" => self.as_mut().set_busqueda_apps(variante(v)),
            "detalle" => self.as_mut().set_detalle(variante(v)),
            "propias" => self.as_mut().set_propias(variante(v)),
            "propiasEstados" => self.as_mut().set_propias_estados(variante(v)),
            "androidEstado" => self.as_mut().set_android_estado(variante(v)),
            "androidApps" => self.as_mut().set_android_apps(variante(v)),
            "androidPopulares" => self.as_mut().set_android_populares(variante(v)),
            "inicioEstado" => self.as_mut().set_inicio_estado(texto(v)),
            "categoriaEstado" => self.as_mut().set_categoria_estado(texto(v)),
            "busquedaTexto" => self.as_mut().set_busqueda_texto(texto(v)),
            "busquedaEstado" => self.as_mut().set_busqueda_estado(texto(v)),
            "detalleEstado" => self.as_mut().set_detalle_estado(texto(v)),
            "androidFase" => self.as_mut().set_android_fase(texto(v)),
            "androidPaso" => self.as_mut().set_android_paso(texto(v)),
            "androidId" => self.as_mut().set_android_id(texto(v)),
            "revisandoActualizaciones" => self
                .as_mut()
                .set_revisando_actualizaciones(v.as_bool().unwrap_or(false)),
            "ocupado" => self.as_mut().set_ocupado(v.as_bool().unwrap_or(false)),
            "categoriaHayMas" => self.as_mut().set_categoria_hay_mas(v.as_bool().unwrap_or(false)),
            "categoriaCargandoMas" => self
                .as_mut()
                .set_categoria_cargando_mas(v.as_bool().unwrap_or(false)),
            "androidCargado" => self.as_mut().set_android_cargado(v.as_bool().unwrap_or(false)),
            "numActualizaciones" => self.as_mut().set_num_actualizaciones(entero(v)),
            "androidPct" => self.as_mut().set_android_pct(entero(v)),
            _ => {}
        }
    }

    /// Vuelve a leer de la tienda las propiedades de un grupo (o todas).
    fn sincronizar(mut self: Pin<&mut Self>, claves: &[&str]) {
        let Some(tienda) = self.rust().tienda.clone() else {
            return;
        };
        for clave in claves {
            let valor = tienda.leer(clave);
            self.as_mut().aplicar(clave, &valor);
        }
    }

    fn tienda(&self) -> Option<Arc<Tienda>> {
        self.rust().tienda.clone()
    }
}

/// Todas las propiedades, para leerlas de una vez al abrir.
const TODAS: &[&str] = &[
    "estados",
    "instaladasIds",
    "sistemaIds",
    "instaladas",
    "actualizablesIds",
    "actualizables",
    "categorias",
    "recomendadas",
    "populares",
    "categoriaActual",
    "categoriaApps",
    "busquedaApps",
    "detalle",
    "propias",
    "propiasEstados",
    "androidEstado",
    "androidApps",
    "androidPopulares",
    "inicioEstado",
    "categoriaEstado",
    "busquedaTexto",
    "busquedaEstado",
    "detalleEstado",
    "androidFase",
    "androidPaso",
    "androidId",
    "revisandoActualizaciones",
    "ocupado",
    "categoriaHayMas",
    "categoriaCargandoMas",
    "androidCargado",
    "numActualizaciones",
    "androidPct",
];

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        let hilo = self.qt_thread();
        let hilo_eventos = hilo.clone();
        // Cada evento de la tienda llega desde el hilo que lo causó; aquí pasa al hilo de la interfaz.
        let sumidero: Sumidero = Arc::new(move |e: Evento| {
            let _ = hilo_eventos.queue(move |mut obj: Pin<&mut qobject::Puente>| match e {
                Evento::Cambio(g) => obj.as_mut().sincronizar(g.claves()),
                Evento::Aviso(t) => obj.as_mut().aviso(&QString::from(t.as_str())),
                Evento::Listo(t) => obj.as_mut().listo(&QString::from(t.as_str())),
                Evento::AbrirUrl(u) => instancia::abrir_url(&u),
            });
        });
        let qml = CARPETA_QML.get().cloned().unwrap_or_default();
        let entorno = Entorno::default();
        let tienda = Tienda::nueva(
            Arc::new(MotorReal::del_entorno()),
            Arc::new(Propias::nuevo(qml, entorno.clone())),
            Arc::new(Android::nuevo(entorno)),
            sumidero,
        );
        self.as_mut().rust_mut().tienda = Some(tienda);
        self.as_mut().sincronizar(TODAS);
        self.as_mut()
            .set_android_populares(variante(&Value::Array(vortex_shop::android::populares())));

        // Una sola ventana: quien intente abrir otra nos escribe en este socket.
        let ruta = instancia::ruta(&instancia::nombre());
        if let Ok(escucha) = Escucha::nueva(&ruta) {
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(150));
                    if escucha.revisar() > 0 {
                        let hecho = hilo.queue(|obj: Pin<&mut qobject::Puente>| obj.mostrar_ventana());
                        instancia::enfocar_en_hyprland();
                        if hecho.is_err() {
                            break;
                        }
                    }
                }
            });
        }
    }
}

impl qobject::Puente {
    pub fn iniciar(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.iniciar();
        }
    }

    pub fn reintentar_todo(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.reintentar_todo();
        }
    }

    pub fn cargar_categorias(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.cargar_categorias();
        }
    }

    pub fn cargar_inicio(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.cargar_inicio();
        }
    }

    pub fn abrir_categoria(self: Pin<&mut Self>, cat_id: &QString) {
        if let Some(t) = self.tienda() {
            t.abrir_categoria(&cat_id.to_string());
        }
    }

    pub fn mas_de_categoria(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.mas_de_categoria();
        }
    }

    pub fn reintentar_categoria(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.reintentar_categoria();
        }
    }

    pub fn buscar(self: Pin<&mut Self>, texto: &QString) {
        if let Some(t) = self.tienda() {
            t.buscar(&texto.to_string());
        }
    }

    pub fn reintentar_busqueda(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.reintentar_busqueda();
        }
    }

    pub fn abrir_detalle(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.abrir_detalle(&app_id.to_string());
        }
    }

    pub fn reintentar_detalle(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.reintentar_detalle();
        }
    }

    pub fn buscar_actualizaciones(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.buscar_actualizaciones();
        }
    }

    pub fn instalar_propia(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.instalar_propia(&app_id.to_string());
        }
    }

    pub fn quitar_propia(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.quitar_propia(&app_id.to_string());
        }
    }

    pub fn abrir_propia(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.abrir_propia(&app_id.to_string());
        }
    }

    pub fn cargar_android(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.cargar_android();
        }
    }

    pub fn preparar_android(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.preparar_android();
        }
    }

    pub fn android_tienda(self: Pin<&mut Self>, paquete: &QString) {
        if let Some(t) = self.tienda() {
            t.android_tienda(&paquete.to_string());
        }
    }

    pub fn android_buscar(self: Pin<&mut Self>, texto: &QString) {
        if let Some(t) = self.tienda() {
            t.android_buscar(&texto.to_string());
        }
    }

    pub fn android_abrir(self: Pin<&mut Self>, paquete: &QString) {
        if let Some(t) = self.tienda() {
            t.android_abrir(&paquete.to_string());
        }
    }

    pub fn android_quitar(self: Pin<&mut Self>, paquete: &QString, nombre: &QString) {
        if let Some(t) = self.tienda() {
            t.android_quitar(&paquete.to_string(), &nombre.to_string());
        }
    }

    pub fn android_pedir_id(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.android_pedir_id();
        }
    }

    pub fn android_abrir_registro(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.android_abrir_registro();
        }
    }

    pub fn android_ya_registre(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.android_ya_registre();
        }
    }

    pub fn instalar(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.instalar(&app_id.to_string());
        }
    }

    pub fn quitar(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.quitar(&app_id.to_string());
        }
    }

    pub fn actualizar(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.actualizar(&app_id.to_string());
        }
    }

    pub fn actualizar_todo(self: Pin<&mut Self>) {
        if let Some(t) = self.tienda() {
            t.actualizar_todo();
        }
    }

    pub fn abrir(self: Pin<&mut Self>, app_id: &QString) {
        if let Some(t) = self.tienda() {
            t.abrir(&app_id.to_string());
        }
    }
}
