//! `boveda`: el vault de notas (árbol, editor, enlaces, grafo y la memoria para la IA). Lo mismo que la clase
//! `Boveda` de `puente.py`: mismos nombres de propiedades, señales y slots.
//!
//! Leer el árbol y guardar una nota es rápido y va en el hilo de la interfaz (como en la de Python); el
//! grafo, la memoria para la IA y la revisión de cambios van en hilos aparte.
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
        #[qproperty(QString, nombre, READ, NOTIFY = raiz_cambio)]
        #[qproperty(QString, ruta, READ, NOTIFY = raiz_cambio)]
        #[qproperty(QString, nota, READ, NOTIFY = nota_cambio)]
        #[qproperty(QString, titulo, READ, NOTIFY = nota_cambio)]
        #[qproperty(QString, carpeta_nota, cxx_name = "carpetaNota", READ, NOTIFY = nota_cambio)]
        #[qproperty(QString, texto, READ, NOTIFY = nota_cambio)]
        #[qproperty(QVariant, retroenlaces, READ, NOTIFY = paneles_cambio)]
        #[qproperty(QVariant, enlaces, READ, NOTIFY = paneles_cambio)]
        #[qproperty(QVariant, etiquetas_nota, cxx_name = "etiquetasNota", READ, NOTIFY = paneles_cambio)]
        #[qproperty(QVariant, etiquetas, READ, NOTIFY = paneles_cambio)]
        #[qproperty(i32, total_notas, cxx_name = "totalNotas", READ, NOTIFY = paneles_cambio)]
        #[qproperty(QString, busqueda, READ, WRITE = escribir_busqueda, NOTIFY = resultados_cambio)]
        #[qproperty(QVariant, resultados, READ, NOTIFY = resultados_cambio)]
        #[qproperty(QVariant, grafo, READ, NOTIFY = grafo_cambio)]
        #[qproperty(f64, memoria_progreso, cxx_name = "memoriaProgreso", READ, NOTIFY = memoria_cambio)]
        #[qproperty(QString, memoria_texto, cxx_name = "memoriaTexto", READ, NOTIFY = memoria_cambio)]
        #[qproperty(QVariant, lista, READ, CONSTANT)]
        type Boveda = super::BovedaRust;

        #[qsignal]
        #[cxx_name = "raizCambio"]
        fn raiz_cambio(self: Pin<&mut Boveda>);
        #[qsignal]
        #[cxx_name = "notaCambio"]
        fn nota_cambio(self: Pin<&mut Boveda>);
        #[qsignal]
        #[cxx_name = "panelesCambio"]
        fn paneles_cambio(self: Pin<&mut Boveda>);
        #[qsignal]
        #[cxx_name = "resultadosCambio"]
        fn resultados_cambio(self: Pin<&mut Boveda>);
        #[qsignal]
        #[cxx_name = "grafoCambio"]
        fn grafo_cambio(self: Pin<&mut Boveda>);
        #[qsignal]
        #[cxx_name = "memoriaCambio"]
        fn memoria_cambio(self: Pin<&mut Boveda>);
        #[qsignal]
        #[cxx_name = "notaGuardada"]
        fn nota_guardada(self: Pin<&mut Boveda>);

        #[cxx_name = "escribirBusqueda"]
        fn escribir_busqueda(self: Pin<&mut Boveda>, texto: &QString);

        #[qinvokable]
        fn recargar(self: Pin<&mut Boveda>);
        #[qinvokable]
        fn alternar(self: Pin<&mut Boveda>, ruta: &QString);
        #[qinvokable]
        fn abrir(self: Pin<&mut Boveda>, ruta: &QString);
        /// Toca un [[enlace]]: abre la nota; si no existe, la crea (como Obsidian).
        #[qinvokable]
        #[cxx_name = "abrirEnlace"]
        fn abrir_enlace(self: Pin<&mut Boveda>, nombre: &QString);
        #[qinvokable]
        fn guardar(self: Pin<&mut Boveda>, texto: &QString);
        /// Guarda el texto en esa nota aunque ya no sea la abierta (el editor guarda al cambiar de nota).
        #[qinvokable]
        #[cxx_name = "guardarEn"]
        fn guardar_en(self: Pin<&mut Boveda>, ruta: &QString, texto: &QString);
        #[qinvokable]
        #[cxx_name = "nuevaNota"]
        fn nueva_nota(self: Pin<&mut Boveda>, titulo: &QString, carpeta: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "nuevaCarpeta"]
        fn nueva_carpeta(self: Pin<&mut Boveda>, nombre: &QString, padre: &QString);
        #[qinvokable]
        fn renombrar(self: Pin<&mut Boveda>, ruta: &QString, nuevo: &QString);
        #[qinvokable]
        fn borrar(self: Pin<&mut Boveda>, ruta: &QString);
        /// Las notas para el autocompletado de [[: primero las que empiezan así, luego las que lo tienen.
        #[qinvokable]
        fn sugerir(self: &Boveda, prefijo: &QString) -> QVariant;
        #[qinvokable]
        fn vista(self: &Boveda, texto: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "pedirGrafo"]
        fn pedir_grafo(self: Pin<&mut Boveda>);
        #[qinvokable]
        #[cxx_name = "actualizarMemoria"]
        fn actualizar_memoria(self: Pin<&mut Boveda>);
        #[qinvokable]
        #[cxx_name = "cancelarMemoria"]
        fn cancelar_memoria(self: Pin<&mut Boveda>);
    }

    impl cxx_qt::Initialize for Boveda {}
    impl cxx_qt::Threading for Boveda {}
}

use crate::arranque::ffi::json_a_variante;
use crate::comun::{avisar, comun, en_general, msg_vault, nueva_memoria, registrar_boveda};
use crate::lista::Lista;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use vortex_local_ai::http::Cancelar;
use vortex_local_ai::memoria::Resumen;
use vortex_local_ai::texto;
use vortex_local_ai::vault::{self, ErrorVault, Grafo, Nodo, Vault};

pub struct BovedaRust {
    nombre: QString,
    ruta: QString,
    nota: QString,
    titulo: QString,
    carpeta_nota: QString,
    texto: QString,
    retroenlaces: QVariant,
    enlaces: QVariant,
    etiquetas_nota: QVariant,
    etiquetas: QVariant,
    total_notas: i32,
    busqueda: QString,
    resultados: QVariant,
    grafo: QVariant,
    memoria_progreso: f64,
    memoria_texto: QString,
    lista: QVariant,
    // Por dentro
    arbol: Lista,
    vault: Option<Arc<Vault>>,
    abiertas: HashSet<String>,
    titulos: Vec<(String, String)>, // (título, título sin acentos)
    arbol_crudo: Option<Nodo>,
    huella: Vec<(String, u64)>,
    cancelar_memoria: Cancelar,
}

fn vacia() -> QVariant {
    json_a_variante("[]")
}

impl Default for BovedaRust {
    fn default() -> Self {
        let arbol = Lista::nueva(&[
            ("tipo", json!("nota")),
            ("nombre", json!("")),
            ("ruta", json!("")),
            ("nivel", json!(0)),
            ("abierta", json!(false)),
            ("vacia", json!(false)),
        ]);
        BovedaRust {
            nombre: QString::default(),
            ruta: QString::default(),
            nota: QString::default(),
            titulo: QString::default(),
            carpeta_nota: QString::default(),
            texto: QString::default(),
            retroenlaces: vacia(),
            enlaces: vacia(),
            etiquetas_nota: vacia(),
            etiquetas: vacia(),
            total_notas: 0,
            busqueda: QString::default(),
            resultados: vacia(),
            grafo: json_a_variante(r#"{"nodos": [], "aristas": []}"#),
            memoria_progreso: -1.0,
            memoria_texto: QString::default(),
            lista: arbol.variante(),
            arbol,
            vault: None,
            abiertas: HashSet::new(),
            titulos: Vec::new(),
            arbol_crudo: None,
            huella: Vec::new(),
            cancelar_memoria: Cancelar::new(),
        }
    }
}

impl cxx_qt::Initialize for qobject::Boveda {
    fn initialize(self: Pin<&mut Self>) {
        registrar_boveda(self.qt_thread());
    }
}

fn enlace_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(!?)\[\[([^\]\n]+?)\]\]").unwrap())
}

/// Lo que `re.findall` le saca al texto: `(incrustado, cuerpo)` de cada [[enlace]].
fn enlaces_del_texto(t: &str) -> Vec<(String, String)> {
    enlace_re()
        .captures_iter(t)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

fn stem(ruta: &str) -> String {
    Path::new(ruta)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn variante(v: &Value) -> QVariant {
    json_a_variante(&v.to_string())
}

fn huella_de(notas: &[vault::Nota]) -> Vec<(String, u64)> {
    let mut h: Vec<(String, u64)> = notas
        .iter()
        .map(|n| (n.ruta.clone(), n.modificado.to_bits()))
        .collect();
    h.sort();
    h
}

impl qobject::Boveda {
    fn vault(&self) -> Option<Arc<Vault>> {
        self.rust().vault.clone()
    }

    pub fn arrancar(mut self: Pin<&mut Self>) {
        let guardada = comun().config.lock().unwrap().texto("vault");
        let ruta = if guardada.is_empty() {
            comun().rutas.vault_por_defecto.clone()
        } else {
            PathBuf::from(guardada)
        };
        self.as_mut().usar(ruta, false);
        // Por si cambias notas con Obsidian u otro editor: cada 5 s se revisa si hay cambios.
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(5));
                if hilo.queue(|b| b.revisar_cambios()).is_err() {
                    break;
                }
            }
        });
    }

    /// Cambia de vault (la carpeta de notas).
    pub fn usar(mut self: Pin<&mut Self>, ruta: PathBuf, avisar_cambio: bool) {
        let v = match Vault::new(&ruta) {
            Ok(v) => Arc::new(v),
            Err(e) => {
                avisar(format!("No pude abrir esa carpeta: {}", msg_vault(&e)), "error");
                if self.rust().vault.is_none()
                    && let Ok(v) = Vault::new(&comun().rutas.vault_por_defecto)
                {
                    self.as_mut().poner_vault(Arc::new(v));
                }
                return;
            }
        };
        let raiz = v.raiz.clone();
        self.as_mut().poner_vault(v);
        comun()
            .config
            .lock()
            .unwrap()
            .poner("vault", json!(raiz.to_string_lossy()));
        {
            let mut r = self.as_mut().rust_mut();
            r.abiertas.clear();
            r.nota = QString::default();
            r.texto = QString::default();
            r.titulo = QString::default();
            r.carpeta_nota = QString::default();
            r.retroenlaces = vacia();
            r.enlaces = vacia();
            r.etiquetas_nota = vacia();
            r.resultados = vacia();
            r.busqueda = QString::default();
        }
        self.as_mut().nota_cambio();
        self.as_mut().paneles_cambio();
        self.as_mut().resultados_cambio();
        self.as_mut().raiz_cambio();
        self.as_mut().recargar();
        self.as_mut().pedir_grafo();
        if avisar_cambio {
            let nombre = raiz.file_name().unwrap_or_default().to_string_lossy().to_string();
            avisar(format!("Ahora usas el vault «{nombre}»."), "exito");
        }
        en_general(|g| g.sincronizar_vault());
    }

    fn poner_vault(mut self: Pin<&mut Self>, v: Arc<Vault>) {
        let raiz = v.raiz.clone();
        comun().compartido.lock().unwrap().raiz = raiz.clone();
        let mut r = self.as_mut().rust_mut();
        r.nombre = QString::from(raiz.file_name().unwrap_or_default().to_string_lossy().as_ref());
        r.ruta = QString::from(raiz.to_string_lossy().as_ref());
        r.vault = Some(v);
    }

    // --- árbol ---

    pub fn recargar(mut self: Pin<&mut Self>) {
        let Some(v) = self.vault() else { return };
        let arbol = v.arbol();
        let notas = v.notas();
        let etiquetas = v.etiquetas();
        {
            let mut r = self.as_mut().rust_mut();
            r.titulos = notas
                .iter()
                .map(|n| (n.titulo.clone(), texto::sin_acentos(&n.titulo)))
                .collect();
            r.total_notas = notas.len() as i32;
            r.etiquetas = variante(&Value::Array(
                etiquetas
                    .iter()
                    .map(|(k, n)| json!({"nombre": k, "n": n}))
                    .collect(),
            ));
            r.arbol_crudo = Some(arbol);
            r.huella = huella_de(&notas);
        }
        self.as_mut().aplanar();
        self.paneles_cambio();
    }

    fn revisar_cambios(mut self: Pin<&mut Self>) {
        let Some(v) = self.vault() else { return };
        if huella_de(&v.notas()) != self.rust().huella {
            self.as_mut().recargar();
            self.pedir_grafo();
        }
    }

    fn aplanar(self: Pin<&mut Self>) {
        fn recorrer(nodo: &Nodo, nivel: i32, abiertas: &HashSet<String>, filas: &mut Vec<Value>) {
            for h in &nodo.hijos {
                if h.es_carpeta {
                    let abierta = abiertas.contains(&h.ruta);
                    filas.push(json!({
                        "tipo": "carpeta", "nombre": h.nombre, "ruta": h.ruta, "nivel": nivel,
                        "abierta": abierta, "vacia": h.hijos.is_empty(),
                    }));
                    if abierta {
                        recorrer(h, nivel + 1, abiertas, filas);
                    }
                } else {
                    let nombre = if h.titulo.is_empty() { &h.nombre } else { &h.titulo };
                    filas.push(json!({"tipo": "nota", "nombre": nombre, "ruta": h.ruta, "nivel": nivel}));
                }
            }
        }
        let mut filas = Vec::new();
        if let Some(raiz) = &self.rust().arbol_crudo {
            recorrer(raiz, 0, &self.rust().abiertas, &mut filas);
        }
        self.rust().arbol.poner(filas);
    }

    fn alternar(mut self: Pin<&mut Self>, ruta: &QString) {
        let ruta = ruta.to_string();
        let a = &mut self.as_mut().rust_mut().abiertas;
        if !a.remove(&ruta) {
            a.insert(ruta);
        }
        self.aplanar();
    }

    // --- notas ---

    fn abrir(mut self: Pin<&mut Self>, ruta: &QString) {
        let ruta = ruta.to_string();
        let Some(v) = self.vault() else { return };
        let texto = match v.leer(&ruta) {
            Ok(t) => t,
            Err(e) => {
                avisar(msg_vault(&e), "error");
                return;
            }
        };
        {
            let mut r = self.as_mut().rust_mut();
            r.nota = QString::from(ruta.as_str());
            r.titulo = QString::from(stem(&ruta).as_str());
            r.carpeta_nota = QString::from(ruta.rsplit_once('/').map_or("", |p| p.0));
            r.texto = QString::from(texto.as_str());
            // Las carpetas de arriba se abren para que la nota se vea en el árbol.
            let partes: Vec<&str> = ruta.split('/').collect();
            for i in 1..partes.len() {
                r.abiertas.insert(partes[..i].join("/"));
            }
        }
        self.as_mut().aplanar();
        self.as_mut().nota_cambio();
        self.paneles();
    }

    fn abrir_enlace(mut self: Pin<&mut Self>, nombre: &QString) {
        let nombre = nombre.to_string().trim().to_string();
        let Some(v) = self.vault() else { return };
        if nombre.is_empty() {
            return;
        }
        let desde = self.rust().nota.to_string();
        let mut ruta = v.resolver(&nombre, (!desde.is_empty()).then_some(desde.as_str()));
        if ruta.is_none() {
            if nombre.ends_with(".md") && v.existe(&nombre) {
                ruta = Some(nombre.clone());
            } else {
                let t = nombre
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim();
                let titulo = if t.is_empty() { &nombre } else { t };
                match v.crear(titulo, "") {
                    Ok(r) => {
                        avisar(format!("Creé la nota «{}».", stem(&r)), "info");
                        self.as_mut().recargar();
                        self.as_mut().pedir_grafo();
                        ruta = Some(r);
                    }
                    Err(e) => {
                        avisar(msg_vault(&e), "error");
                        return;
                    }
                }
            }
        }
        if let Some(r) = ruta {
            self.abrir(&QString::from(r.as_str()));
        }
    }

    fn guardar(mut self: Pin<&mut Self>, texto: &QString) {
        let texto = texto.to_string();
        let nota = self.rust().nota.to_string();
        if nota.is_empty() || texto == self.rust().texto.to_string() {
            return;
        }
        let Some(v) = self.vault() else { return };
        if let Err(e) = v.guardar(&nota, &texto) {
            avisar(format!("No pude guardar la nota: {}", msg_vault(&e)), "error");
            return;
        }
        let antes = enlaces_del_texto(&self.rust().texto.to_string());
        self.as_mut().rust_mut().texto = QString::from(texto.as_str());
        self.as_mut().nota_guardada();
        self.as_mut().recargar();
        self.as_mut().paneles();
        if enlaces_del_texto(&texto) != antes {
            self.pedir_grafo();
        }
    }

    fn guardar_en(self: Pin<&mut Self>, ruta: &QString, texto: &QString) {
        if ruta.is_empty() {
            return;
        }
        if *ruta == self.rust().nota {
            self.guardar(texto);
            return;
        }
        let Some(v) = self.vault() else { return };
        if let Err(e) = v.guardar(&ruta.to_string(), &texto.to_string()) {
            avisar(format!("No pude guardar la nota: {}", msg_vault(&e)), "error");
            return;
        }
        self.recargar();
    }

    /// Los paneles de la nota abierta: retroenlaces, enlaces y etiquetas.
    fn paneles(mut self: Pin<&mut Self>) {
        let nota = self.rust().nota.to_string();
        if nota.is_empty() {
            return;
        }
        let Some(v) = self.vault() else { return };
        let (retro, enlaces) = match (v.retroenlaces(&nota), v.enlaces(&nota)) {
            (Ok(r), Ok(e)) => (
                r.iter()
                    .map(|r| {
                        json!({
                            "ruta": r.ruta, "titulo": if r.titulo.is_empty() { stem(&r.ruta) } else { r.titulo.clone() },
                            "fragmento": r.fragmento,
                        })
                    })
                    .collect::<Vec<_>>(),
                e.iter()
                    .map(|e| json!({"nombre": e.nombre, "ruta": e.ruta.clone().unwrap_or_default(), "existe": e.existe}))
                    .collect::<Vec<_>>(),
            ),
            _ => (vec![], vec![]),
        };
        let etiquetas = vault::etiquetas_de(&self.rust().texto.to_string());
        let mut r = self.as_mut().rust_mut();
        r.retroenlaces = variante(&Value::Array(retro));
        r.enlaces = variante(&Value::Array(enlaces));
        r.etiquetas_nota = variante(&json!(etiquetas));
        self.paneles_cambio();
    }

    fn nueva_nota(mut self: Pin<&mut Self>, titulo: &QString, carpeta: &QString) -> QString {
        let Some(v) = self.vault() else {
            return QString::default();
        };
        match v.crear(&titulo.to_string(), &carpeta.to_string()) {
            Ok(ruta) => {
                self.as_mut().recargar();
                self.as_mut().abrir(&QString::from(ruta.as_str()));
                self.pedir_grafo();
                QString::from(ruta.as_str())
            }
            Err(e) => {
                avisar(msg_vault(&e), "error");
                QString::default()
            }
        }
    }

    fn nueva_carpeta(mut self: Pin<&mut Self>, nombre: &QString, padre: &QString) {
        let (nombre, padre) = (nombre.to_string(), padre.to_string());
        let Some(v) = self.vault() else { return };
        let ruta = if padre.is_empty() {
            nombre
        } else {
            format!("{padre}/{nombre}")
        };
        match v.crear_carpeta(&ruta) {
            Ok(ruta) => {
                let a = &mut self.as_mut().rust_mut().abiertas;
                if !padre.is_empty() {
                    a.insert(padre);
                }
                a.insert(ruta);
                self.recargar();
            }
            Err(e) => avisar(msg_vault(&e), "error"),
        }
    }

    fn renombrar(mut self: Pin<&mut Self>, ruta: &QString, nuevo: &QString) {
        let (ruta, nuevo) = (ruta.to_string(), nuevo.to_string().trim().to_string());
        if nuevo.is_empty() {
            return;
        }
        let Some(v) = self.vault() else { return };
        let nueva = if ruta.ends_with(".md") {
            match v.renombrar(&ruta, &nuevo) {
                Ok(n) => n,
                Err(e) => {
                    avisar(msg_vault(&e), "error");
                    return;
                }
            }
        } else {
            // Una carpeta
            let r = (|| -> Result<String, ErrorVault> {
                let origen = v.ruta_absoluta(&ruta)?;
                let limpio: String = nuevo
                    .chars()
                    .filter(|c| !"\\/:*?\"<>|#^[]".contains(*c))
                    .collect();
                let destino = origen.parent().unwrap_or(Path::new("")).join(limpio);
                if destino.exists() {
                    return Err(ErrorVault::Existe(destino.to_string_lossy().to_string()));
                }
                std::fs::rename(&origen, &destino)?;
                Ok(destino
                    .strip_prefix(&v.raiz)
                    .unwrap_or(&destino)
                    .to_string_lossy()
                    .replace('\\', "/"))
            })();
            match r {
                Ok(n) => {
                    let nota = self.rust().nota.to_string();
                    if nota.starts_with(&format!("{ruta}/")) {
                        let nueva_nota = format!("{n}{}", &nota[ruta.len()..]);
                        self.as_mut().rust_mut().nota = QString::from(nueva_nota.as_str());
                    }
                    n
                }
                Err(e) => {
                    avisar(msg_vault(&e), "error");
                    return;
                }
            }
        };
        if ruta == self.rust().nota.to_string() {
            let texto = v.leer(&nueva).ok();
            let mut r = self.as_mut().rust_mut();
            r.nota = QString::from(nueva.as_str());
            r.titulo = QString::from(stem(&nueva).as_str());
            r.carpeta_nota = QString::from(nueva.rsplit_once('/').map_or("", |p| p.0));
            if let Some(t) = texto {
                r.texto = QString::from(t.as_str());
            }
            self.as_mut().nota_cambio();
        }
        self.as_mut().recargar();
        self.as_mut().paneles();
        self.pedir_grafo();
    }

    fn borrar(mut self: Pin<&mut Self>, ruta: &QString) {
        let ruta = ruta.to_string();
        let Some(v) = self.vault() else { return };
        if let Err(e) = v.borrar(&ruta) {
            avisar(msg_vault(&e), "error");
            return;
        }
        let nota = self.rust().nota.to_string();
        if nota == ruta || nota.starts_with(&format!("{ruta}/")) {
            {
                let mut r = self.as_mut().rust_mut();
                r.nota = QString::default();
                r.titulo = QString::default();
                r.carpeta_nota = QString::default();
                r.texto = QString::default();
                r.retroenlaces = vacia();
                r.enlaces = vacia();
                r.etiquetas_nota = vacia();
            }
            self.as_mut().nota_cambio();
        }
        avisar(
            format!("Lo mandé a la papelera del vault (.trash): {}", stem(&ruta)),
            "info",
        );
        self.as_mut().recargar();
        self.pedir_grafo();
    }

    // --- búsqueda y autocompletado ---

    fn escribir_busqueda(mut self: Pin<&mut Self>, texto: &QString) {
        if *texto == self.rust().busqueda {
            return;
        }
        let t = texto.to_string();
        let resultados = match self.vault() {
            Some(v) if !t.trim().is_empty() => v
                .buscar(&t)
                .map(|rs| {
                    rs.iter()
                        .map(|r| {
                            json!({
                                "ruta": r.ruta, "titulo": if r.titulo.is_empty() { stem(&r.ruta) } else { r.titulo.clone() },
                                "fragmento": r.fragmento,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            _ => vec![],
        };
        let mut r = self.as_mut().rust_mut();
        r.busqueda = texto.clone();
        r.resultados = variante(&Value::Array(resultados));
        self.resultados_cambio();
    }

    fn sugerir(&self, prefijo: &QString) -> QVariant {
        let clave = texto::sin_acentos(prefijo.to_string().trim());
        let titulos = &self.rust().titulos;
        let mut empiezan: Vec<&str> = titulos
            .iter()
            .filter(|t| t.1.starts_with(&clave))
            .map(|t| t.0.as_str())
            .collect();
        let mut tienen: Vec<&str> = titulos
            .iter()
            .filter(|t| !clave.is_empty() && t.1.contains(&clave) && !t.1.starts_with(&clave))
            .map(|t| t.0.as_str())
            .collect();
        empiezan.sort_by_key(|t| t.to_lowercase());
        tienen.sort_by_key(|t| t.to_lowercase());
        let mut vistos: Vec<&str> = Vec::new();
        for t in empiezan.into_iter().chain(tienen) {
            if !vistos.contains(&t) {
                vistos.push(t);
            }
        }
        vistos.truncate(8);
        variante(&json!(vistos))
    }

    fn vista(&self, texto: &QString) -> QString {
        QString::from(texto::markdown_con_enlaces(&texto.to_string()).as_str())
    }

    // --- grafo ---

    pub fn pedir_grafo(self: Pin<&mut Self>) {
        let Some(v) = self.vault() else { return };
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let g = v.grafo();
            let _ = hilo.queue(move |b| b.poner_grafo(g));
        });
    }

    fn poner_grafo(mut self: Pin<&mut Self>, g: Grafo) {
        let ids: HashMap<&str, usize> = g
            .nodos
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let nodos: Vec<Value> = g
            .nodos
            .iter()
            .map(|n| json!({"id": n.id, "titulo": n.titulo, "grado": n.grado, "existe": n.existe}))
            .collect();
        let aristas: Vec<Value> = g
            .aristas
            .iter()
            .filter_map(|(a, b)| Some(json!([ids.get(a.as_str())?, ids.get(b.as_str())?])))
            .collect();
        self.as_mut().rust_mut().grafo = variante(&json!({"nodos": nodos, "aristas": aristas}));
        self.grafo_cambio();
    }

    // --- memoria para la IA ---

    fn actualizar_memoria(mut self: Pin<&mut Self>) {
        if self.rust().memoria_progreso >= 0.0 {
            return;
        }
        let cancelar = Cancelar::new();
        {
            let mut r = self.as_mut().rust_mut();
            r.cancelar_memoria = cancelar.clone();
            r.memoria_progreso = 0.0;
            r.memoria_texto = QString::from("Preparando…");
        }
        self.as_mut().memoria_cambio();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let h2 = hilo.clone();
            let mut ultimo: Option<Instant> = None;
            let mut al_progreso = move |hechas: usize, total: usize, _ruta: &str| {
                let ahora = Instant::now();
                if ultimo.is_none_or(|t| ahora.duration_since(t) >= Duration::from_millis(100))
                    || hechas == total
                {
                    ultimo = Some(ahora);
                    let _ = h2.queue(move |b| b.al_progreso_memoria(hechas, total));
                }
            };
            let r = nueva_memoria().map_err(|e| msg_vault(&e)).and_then(|mut mem| {
                let r = mem
                    .actualizar(Some(&mut al_progreso), Some(&cancelar))
                    .map_err(|e| e.mensaje)?;
                Ok((r, mem.modo(), mem.aviso()))
            });
            let _ = hilo.queue(move |b| b.memoria_lista(r));
        });
    }

    fn memoria_lista(mut self: Pin<&mut Self>, r: Result<(Resumen, &'static str, String), String>) {
        match r {
            Ok((r, modo, aviso)) => {
                let texto = if r.cancelado {
                    "Detenida: lo que ya iba quedó guardado.".to_string()
                } else if modo == "palabras" {
                    "Sin modelo de embeddings: la IA busca por palabras.".to_string()
                } else {
                    format!("Lista: {} notas, {} fragmentos.", r.notas, r.fragmentos)
                };
                {
                    let mut p = self.as_mut().rust_mut();
                    p.memoria_progreso = -1.0;
                    p.memoria_texto = QString::from(texto.as_str());
                }
                self.memoria_cambio();
                if !aviso.is_empty() {
                    avisar(aviso, "info");
                } else if !r.cancelado {
                    avisar("Listo: la IA ya conoce tus notas.", "exito");
                }
            }
            Err(e) => {
                {
                    let mut p = self.as_mut().rust_mut();
                    p.memoria_progreso = -1.0;
                    p.memoria_texto = QString::default();
                }
                self.memoria_cambio();
                avisar(format!("No pude actualizar la memoria: {e}"), "error");
            }
        }
    }

    fn cancelar_memoria(self: Pin<&mut Self>) {
        self.rust().cancelar_memoria.set();
    }

    fn al_progreso_memoria(mut self: Pin<&mut Self>, hechas: usize, total: usize) {
        if self.rust().memoria_progreso < 0.0 {
            return;
        }
        {
            let mut r = self.as_mut().rust_mut();
            r.memoria_progreso = if total != 0 {
                hechas as f64 / total as f64
            } else {
                0.0
            };
            r.memoria_texto = QString::from(format!("Leyendo {hechas} de {total} notas…").as_str());
        }
        self.memoria_cambio();
    }
}
