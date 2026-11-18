//! El estado que ve la interfaz de Vortexine's Shop: lo que en Python es `puente.py`, pero sin Qt.
//!
//! La interfaz nunca habla con el motor: pide cosas a la [`Tienda`] (los «slots») y lee de ella (los
//! «propiedades», valores JSON con su grupo de cambio). Todo lo lento (red, flatpak) corre en hilos; al
//! terminar, el resultado se aplica al estado con el cerrojo tomado, así que cada resultado se aplica entero
//! y de a uno (como en Python, donde todo pasaba por el hilo de la interfaz).
//!
//! Instalar, quitar y actualizar pasan por una cola de una sola vía (un hilo): las demás tareas esperan su
//! turno y la tarjeta de cada app muestra «En cola» o su progreso.
//!
//! Cada cambio avisa por `Evento` (a qué grupo de propiedades le cambió algo, un aviso, «algo salió
//! bien»…); la ventana (cxx-qt) los convierte en propiedades y señales de QML. Las pruebas los recogen.

use crate::android::{self, Android};
use crate::catalogo::{App, AppDetalle, Categoria};
use crate::errores::{Error, Resultado};
use crate::motor::Motor;
use crate::propias::Propias;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

// Lo que flatpak instala pero no es una app (runtimes, temas, extensiones): la tienda no lo muestra.
const PREFIJOS_NO_APP: [&str; 10] = [
    "org.freedesktop.Platform",
    "org.freedesktop.Sdk",
    "org.gnome.Platform",
    "org.gnome.Sdk",
    "org.kde.Platform",
    "org.kde.Sdk",
    "org.gtk.Gtk3theme",
    "org.kde.KStyle",
    "org.kde.PlatformTheme",
    "org.freedesktop.LinuxAudio",
];
const SUFIJOS_NO_APP: [&str; 5] = [".Locale", ".Debug", ".Sources", ".Codecs", ".Extension"];

/// Una señal de cambio por grupo de propiedades (como en el `Puente` de Python).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Grupo {
    Estados,
    Instaladas,
    Actualizaciones,
    Ocupado,
    Categorias,
    Inicio,
    Categoria,
    Busqueda,
    Detalle,
    /// Juegos de Android (Waydroid): estado, preparación y lo instalado.
    Android,
    /// Las apps de Vortexine's (Camera…): si están y qué se hace.
    Propias,
}

impl Grupo {
    /// Las propiedades de QML que cambian con este grupo.
    pub fn claves(self) -> &'static [&'static str] {
        match self {
            Grupo::Estados => &["estados"],
            Grupo::Instaladas => &["instaladasIds", "sistemaIds", "instaladas"],
            Grupo::Actualizaciones => &[
                "actualizablesIds",
                "actualizables",
                "numActualizaciones",
                "revisandoActualizaciones",
            ],
            Grupo::Ocupado => &["ocupado"],
            Grupo::Categorias => &["categorias"],
            Grupo::Inicio => &["inicioEstado", "recomendadas", "populares"],
            Grupo::Categoria => &[
                "categoriaActual",
                "categoriaApps",
                "categoriaEstado",
                "categoriaHayMas",
                "categoriaCargandoMas",
            ],
            Grupo::Busqueda => &["busquedaTexto", "busquedaApps", "busquedaEstado"],
            Grupo::Detalle => &["detalle", "detalleEstado"],
            Grupo::Android => &[
                "androidEstado",
                "androidApps",
                "androidFase",
                "androidPaso",
                "androidPct",
                "androidId",
                "androidCargado",
            ],
            Grupo::Propias => &["propias", "propiasEstados"],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    /// A este grupo le cambió algo: hay que volver a leer sus propiedades.
    Cambio(Grupo),
    /// Un mensaje en español para el aviso que se cierra (algo salió mal).
    Aviso(String),
    /// Lo mismo, pero de que algo salió bien (sin el rojo de error).
    Listo(String),
    /// Abrir una página web en el navegador de la persona.
    AbrirUrl(String),
}

pub type Sumidero = Arc<dyn Fn(Evento) + Send + Sync>;

// --- Conversión de los datos del motor a lo que entiende QML -------------------------------------

/// Bytes → «120 MB» (con coma decimal si es chico: «1,4 GB»).
pub fn tamano(n: Option<i64>) -> String {
    let Some(n) = n.filter(|n| *n > 0) else {
        return String::new();
    };
    let (mut v, mut i) = (n as f64, 0);
    let unidades = ["B", "KB", "MB", "GB", "TB"];
    while v >= 1024.0 && i < unidades.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    let texto = if i <= 1 || v >= 10.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}").replace('.', ",")
    };
    format!("{texto} {}", unidades[i])
}

/// Descargas al mes → «12 mil descargas al mes» / «1,2 M descargas al mes».
pub fn descargas(n: Option<i64>) -> String {
    let Some(n) = n.filter(|n| *n > 0) else {
        return String::new();
    };
    let base = if n >= 1_000_000 {
        let t = format!("{:.1}", n as f64 / 1_000_000.0).replace('.', ",");
        format!("{} M", t.strip_suffix(",0").unwrap_or(&t))
    } else if n >= 1000 {
        format!("{} mil", (n as f64 / 1000.0).round_ties_even() as i64)
    } else {
        n.to_string()
    };
    format!("{base} descargas al mes")
}

pub fn app_a_dict(a: &App) -> Value {
    json!({
        "id": a.id,
        "nombre": if a.nombre.is_empty() { &a.id } else { &a.nombre },
        "resumen": a.resumen,
        "icono": a.icono_url,
        "desarrollador": a.desarrollador,
        "verificado": a.verificado,
        "descargas": a.descargas_mes.unwrap_or(-1),
        "descargasTexto": descargas(a.descargas_mes),
        // Solo las de la categoría «IA» los traen: qué gráfica les conviene y un dato extra.
        "grafica": a.grafica,
        "nota": a.nota,
    })
}

pub fn categoria_a_dict(c: &Categoria) -> Value {
    json!({"id": c.id, "nombre": c.nombre, "icono": c.icono, "intro": c.intro,
           "comando": c.comando, "destacada": c.destacada})
}

pub fn detalle_a_dict(a: &AppDetalle) -> Value {
    let mut d = app_a_dict(&a.app);
    let o = d.as_object_mut().expect("objeto");
    o.insert("descripcion".into(), json!(a.descripcion));
    o.insert("capturas".into(), json!(a.capturas));
    o.insert("licencia".into(), json!(a.licencia));
    o.insert("web".into(), json!(a.web.clone().unwrap_or_default()));
    o.insert("version".into(), json!(a.version.clone().unwrap_or_default()));
    o.insert("tamano".into(), json!(a.tamano_descarga.unwrap_or(0)));
    o.insert("tamanoTexto".into(), json!(tamano(a.tamano_descarga)));
    o.insert("categorias".into(), json!(a.categorias));
    d
}

/// Ficha provisional de una app de la que aún no tenemos datos (solo su id).
pub fn marcador(app_id: &str) -> Value {
    let ultimo = app_id.rsplit('.').next().unwrap_or(app_id);
    let nombre = ultimo.replace(['-', '_'], " ");
    let nombre = if nombre.is_empty() {
        app_id.to_string()
    } else {
        nombre
    };
    let mut c = nombre.chars();
    let nombre: String = match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    };
    json!({"id": app_id, "nombre": nombre, "resumen": "", "icono": "", "desarrollador": "",
           "verificado": false, "descargas": -1, "descargasTexto": "", "grafica": "", "nota": ""})
}

/// Descarta runtimes, temas y extensiones: la tienda solo muestra apps.
pub fn es_app(app_id: &str) -> bool {
    !(PREFIJOS_NO_APP.iter().any(|p| app_id.starts_with(p))
        || SUFIJOS_NO_APP.iter().any(|s| app_id.ends_with(s)))
}

/// Error del motor → (estado de la pantalla, texto en español).
pub fn clasificar(e: &Error) -> (&'static str, String) {
    match e {
        e if e.es_sin_red() => (
            "sinred",
            "Sin conexión a internet. Revisa tu red y vuelve a intentarlo.".into(),
        ),
        Error::Flatpak(m) | Error::NoEncontrada(m) => (
            "error",
            if m.is_empty() {
                "No se pudo completar la operación.".into()
            } else {
                m.clone()
            },
        ),
        Error::Propia(m) | Error::Android(m) => ("error", m.clone()),
        e => {
            eprintln!("vortex-shop: error inesperado: {e}");
            (
                "error",
                "Algo salió mal. Vuelve a intentarlo en un momento.".into(),
            )
        }
    }
}

fn fase_de(tipo: &str) -> &'static str {
    match tipo {
        "instalar" => "instalando",
        "quitar" => "quitando",
        _ => "actualizando",
    }
}

// --- Hilos ----------------------------------------------------------------------------------------

type Tarea = Box<dyn FnOnce() + Send>;

/// Un grupo de hilos que se crean cuando hacen falta y se van solos si no hay nada que hacer.
struct Pool {
    max: usize,
    estado: Mutex<EstadoPool>,
    aviso: Condvar,
}

struct EstadoPool {
    cola: VecDeque<Tarea>,
    hilos: usize,
    libres: usize,
}

impl Pool {
    fn nuevo(max: usize) -> Arc<Self> {
        Arc::new(Pool {
            max,
            estado: Mutex::new(EstadoPool {
                cola: VecDeque::new(),
                hilos: 0,
                libres: 0,
            }),
            aviso: Condvar::new(),
        })
    }

    fn enviar(self: &Arc<Self>, tarea: Tarea) {
        let mut e = self.estado.lock().unwrap_or_else(|x| x.into_inner());
        e.cola.push_back(tarea);
        if e.libres == 0 && e.hilos < self.max {
            e.hilos += 1;
            let pool = Arc::clone(self);
            let hecho = std::thread::Builder::new()
                .name("vortex-shop-trabajo".into())
                .spawn(move || pool.trabajar());
            if hecho.is_err() {
                e.hilos -= 1;
            }
        } else {
            self.aviso.notify_one();
        }
    }

    fn trabajar(&self) {
        let mut e = self.estado.lock().unwrap_or_else(|x| x.into_inner());
        loop {
            if let Some(t) = e.cola.pop_front() {
                drop(e);
                let _ = catch_unwind(AssertUnwindSafe(t));
                e = self.estado.lock().unwrap_or_else(|x| x.into_inner());
                continue;
            }
            e.libres += 1;
            let (g, fuera) = self
                .aviso
                .wait_timeout(e, Duration::from_secs(10))
                .unwrap_or_else(|x| x.into_inner());
            e = g;
            e.libres -= 1;
            if fuera.timed_out() && e.cola.is_empty() {
                e.hilos -= 1;
                return;
            }
        }
    }
}

/// Lo que corre en un hilo de trabajo y devuelve qué hacer con el resultado, ya en el estado.
type Trabajo = Box<dyn FnOnce(&Arc<Tienda>) -> Aplicar + Send>;
type Aplicar = Box<dyn FnOnce(&mut Datos) + Send>;

enum Pendiente {
    Trabajo(Trabajo, bool),
    Despues(Duration, Aplicar),
}

/// Todo lo que ve la interfaz, más lo que la tienda recuerda por dentro.
pub struct Datos {
    v: Map<String, Value>,
    /// Fichas conocidas (id → ficha), para las listas locales.
    info: HashMap<String, Value>,
    ids_instaladas: Vec<String>,
    /// Instaladas por el sistema: no se pueden quitar.
    ids_sistema: HashSet<String>,
    ids_actualizables: Vec<String>,
    /// Ids cuya ficha se está pidiendo.
    pidiendo: HashSet<String>,
    /// Ids cuya ficha no se pudo traer (no insistir).
    sin_ficha: HashSet<String>,
    pagina: i64,
    gen_busqueda: u64,
    gen_categoria: u64,
    gen_detalle: u64,
    motor: Arc<dyn Motor>,
    propias: Arc<Propias>,
    android: Arc<Android>,
    preparado: Arc<AtomicBool>,
    espera_abrir: Duration,
    eventos: Vec<Evento>,
    pendientes: Vec<Pendiente>,
}

const CLAVES_BASE: [&str; 10] = [
    "id",
    "nombre",
    "resumen",
    "icono",
    "desarrollador",
    "verificado",
    "descargas",
    "descargasTexto",
    "grafica",
    "nota",
];

fn lista_de(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn texto_de<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

impl Datos {
    fn nuevo(motor: Arc<dyn Motor>, propias: Arc<Propias>, android: Arc<Android>) -> Self {
        let mut v = Map::new();
        for (k, x) in [
            ("estados", json!({})),
            ("instaladasIds", json!({})),
            ("sistemaIds", json!({})),
            ("instaladas", json!([])),
            ("actualizablesIds", json!({})),
            ("actualizables", json!([])),
            ("numActualizaciones", json!(0)),
            ("revisandoActualizaciones", json!(false)),
            ("ocupado", json!(false)),
            ("categorias", json!([])),
            ("inicioEstado", json!("cargando")),
            ("recomendadas", json!([])),
            ("populares", json!([])),
            ("categoriaActual", json!({})),
            ("categoriaApps", json!([])),
            ("categoriaEstado", json!("cargando")),
            ("categoriaHayMas", json!(false)),
            ("categoriaCargandoMas", json!(false)),
            ("busquedaTexto", json!("")),
            ("busquedaApps", json!([])),
            ("busquedaEstado", json!("listo")),
            ("detalle", json!({})),
            ("detalleEstado", json!("cargando")),
            ("propias", Value::Array(propias.lista())),
            ("propiasEstados", json!({})),
            ("androidEstado", json!({})),
            ("androidApps", json!([])),
            ("androidFase", json!("")),
            ("androidPaso", json!("")),
            ("androidPct", json!(0)),
            ("androidId", json!("")),
            ("androidCargado", json!(false)),
        ] {
            v.insert(k.into(), x);
        }
        Datos {
            v,
            info: HashMap::new(),
            ids_instaladas: Vec::new(),
            ids_sistema: HashSet::new(),
            ids_actualizables: Vec::new(),
            pidiendo: HashSet::new(),
            sin_ficha: HashSet::new(),
            pagina: 1,
            gen_busqueda: 0,
            gen_categoria: 0,
            gen_detalle: 0,
            motor,
            propias,
            android,
            preparado: Arc::new(AtomicBool::new(false)),
            espera_abrir: Duration::from_secs(4),
            eventos: Vec::new(),
            pendientes: Vec::new(),
        }
    }

    // --- utilidades

    fn get(&self, k: &str) -> &Value {
        self.v.get(k).unwrap_or(&Value::Null)
    }

    fn get_str(&self, k: &str) -> &str {
        self.get(k).as_str().unwrap_or("")
    }

    fn get_bool(&self, k: &str) -> bool {
        self.get(k).as_bool().unwrap_or(false)
    }

    fn poner(&mut self, g: Grupo, cambios: Vec<(&str, Value)>) {
        for (k, x) in cambios {
            self.v.insert(k.into(), x);
        }
        self.eventos.push(Evento::Cambio(g));
    }

    fn avisar(&mut self, texto: String) {
        self.eventos.push(Evento::Aviso(texto));
    }

    fn listo(&mut self, texto: String) {
        self.eventos.push(Evento::Listo(texto));
    }

    fn avisar_error(&mut self, e: &Error) {
        let (_, t) = clasificar(e);
        self.avisar(t);
    }

    /// Lanza una tarea en un hilo; `ok` o `err` se aplican al estado cuando termina.
    fn lanzar<T: Send + 'static>(
        &mut self,
        cola: bool,
        f: impl FnOnce(&Arc<Tienda>) -> Resultado<T> + Send + 'static,
        ok: impl FnOnce(&mut Datos, T) + Send + 'static,
        err: impl FnOnce(&mut Datos, Error) + Send + 'static,
    ) {
        let trabajo: Trabajo = Box::new(move |t| {
            let r = catch_unwind(AssertUnwindSafe(|| f(t)))
                .unwrap_or_else(|_| Err(Error::Otro("la tarea falló por dentro".into())));
            Box::new(move |d: &mut Datos| match r {
                Ok(x) => ok(d, x),
                Err(e) => err(d, e),
            })
        });
        self.pendientes.push(Pendiente::Trabajo(trabajo, cola));
    }

    /// Como `lanzar`, pero si falla solo se avisa el error.
    fn lanzar_av<T: Send + 'static>(
        &mut self,
        cola: bool,
        f: impl FnOnce(&Arc<Tienda>) -> Resultado<T> + Send + 'static,
        ok: impl FnOnce(&mut Datos, T) + Send + 'static,
    ) {
        self.lanzar(cola, f, ok, |d, e| d.avisar_error(&e));
    }

    // --- Arranque

    fn iniciar(&mut self) {
        self.cargar_categorias();
        self.cargar_inicio();
        self.refrescar_apps_locales();
        let (motor, preparado) = (self.motor.clone(), self.preparado.clone());
        self.lanzar(
            false,
            move |_| preparar_flatpak(&*motor, &preparado),
            |_, ()| {},
            |_, e| eprintln!("vortex-shop: preparar: {e}"),
        );
    }

    fn reintentar_todo(&mut self) {
        self.sin_ficha.clear();
        self.cargar_categorias();
        self.cargar_inicio();
        self.refrescar_apps_locales();
    }

    // --- Inicio y categorías

    fn cargar_categorias(&mut self) {
        let m = self.motor.clone();
        self.lanzar(
            false,
            move |_| m.categorias(),
            |d, cats| {
                let l: Vec<Value> = cats.iter().map(categoria_a_dict).collect();
                d.poner(Grupo::Categorias, vec![("categorias", Value::Array(l))]);
            },
            |_, _| {},
        );
    }

    fn cargar_inicio(&mut self) {
        self.poner(Grupo::Inicio, vec![("inicioEstado", json!("cargando"))]);
        let partes: Arc<Mutex<HashMap<&'static str, Vec<Value>>>> = Arc::default();
        for clave in ["recomendadas", "populares"] {
            let (m, partes_ok) = (self.motor.clone(), partes.clone());
            self.lanzar(
                false,
                move |_| {
                    if clave == "recomendadas" {
                        m.recomendadas()
                    } else {
                        m.destacados()
                    }
                },
                move |d, apps| {
                    let lista: Vec<Value> = apps.iter().map(app_a_dict).collect();
                    for x in &lista {
                        d.recordar(x);
                    }
                    let mut p = partes_ok.lock().unwrap_or_else(|x| x.into_inner());
                    p.insert(clave, lista);
                    if p.len() == 2 {
                        let (r, po) = (p["recomendadas"].clone(), p["populares"].clone());
                        drop(p);
                        d.poner(
                            Grupo::Inicio,
                            vec![
                                ("inicioEstado", json!("listo")),
                                ("recomendadas", Value::Array(r)),
                                ("populares", Value::Array(po)),
                            ],
                        );
                    }
                },
                |d, e| {
                    // solo el primer fallo manda
                    if d.get_str("inicioEstado") == "cargando" {
                        d.poner(Grupo::Inicio, vec![("inicioEstado", json!(clasificar(&e).0))]);
                    }
                },
            );
        }
    }

    fn abrir_categoria(&mut self, cat_id: &str) {
        let actual = lista_de(self.get("categorias"))
            .into_iter()
            .find(|c| texto_de(c, "id") == cat_id)
            .unwrap_or_else(|| {
                json!({"id": cat_id, "nombre": cat_id, "icono": "", "intro": "", "comando": "", "destacada": false})
            });
        self.gen_categoria += 1;
        self.pagina = 1;
        self.poner(
            Grupo::Categoria,
            vec![
                ("categoriaActual", actual),
                ("categoriaApps", json!([])),
                ("categoriaEstado", json!("cargando")),
                ("categoriaHayMas", json!(false)),
                ("categoriaCargandoMas", json!(false)),
            ],
        );
        self.pedir_pagina(cat_id, 1);
    }

    fn pedir_pagina(&mut self, cat_id: &str, pagina: i64) {
        let vuelta = self.gen_categoria;
        let (m, id) = (self.motor.clone(), cat_id.to_string());
        let por_pagina = self.motor.por_pagina();
        self.lanzar(
            false,
            move |_| m.por_categoria(&id, pagina),
            move |d, apps| {
                if vuelta != d.gen_categoria {
                    return;
                }
                let nuevas: Vec<Value> = apps.iter().map(app_a_dict).collect();
                for x in &nuevas {
                    d.recordar(x);
                }
                let actuales = lista_de(d.get("categoriaApps"));
                let vistas: HashSet<String> =
                    actuales.iter().map(|a| texto_de(a, "id").to_string()).collect();
                let mut todas = actuales;
                todas.extend(
                    nuevas
                        .iter()
                        .filter(|n| !vistas.contains(texto_de(n, "id")))
                        .cloned(),
                );
                // Hay más si la página vino llena (el motor da por_pagina) y trajo algo nuevo.
                let lleno = match por_pagina {
                    Some(p) => nuevas.len() >= p,
                    None => !nuevas.is_empty(),
                };
                let hay_mas = lleno && todas.len() > vistas.len();
                d.poner(
                    Grupo::Categoria,
                    vec![
                        ("categoriaApps", Value::Array(todas)),
                        ("categoriaEstado", json!("listo")),
                        ("categoriaHayMas", json!(hay_mas)),
                        ("categoriaCargandoMas", json!(false)),
                    ],
                );
            },
            move |d, e| {
                if vuelta != d.gen_categoria {
                    return;
                }
                if pagina > 1 {
                    // ya había apps a la vista: solo un aviso
                    d.poner(Grupo::Categoria, vec![("categoriaCargandoMas", json!(false))]);
                    d.avisar_error(&e);
                } else {
                    d.poner(
                        Grupo::Categoria,
                        vec![("categoriaEstado", json!(clasificar(&e).0))],
                    );
                }
            },
        );
    }

    fn mas_de_categoria(&mut self) {
        let cat = self.get("categoriaActual").clone();
        if !cat.is_object() || cat.as_object().is_some_and(Map::is_empty) {
            return;
        }
        if self.get_bool("categoriaCargandoMas") || !self.get_bool("categoriaHayMas") {
            return;
        }
        self.pagina += 1;
        self.poner(Grupo::Categoria, vec![("categoriaCargandoMas", json!(true))]);
        let p = self.pagina;
        self.pedir_pagina(texto_de(&cat, "id"), p);
    }

    fn reintentar_categoria(&mut self) {
        let cat = self.get("categoriaActual").clone();
        if cat.as_object().is_some_and(|o| !o.is_empty()) {
            self.abrir_categoria(texto_de(&cat, "id"));
        }
    }

    // --- Búsqueda

    fn buscar(&mut self, texto: &str) {
        let texto = texto.trim().to_string();
        self.gen_busqueda += 1;
        let vuelta = self.gen_busqueda;
        if texto.is_empty() {
            self.poner(
                Grupo::Busqueda,
                vec![
                    ("busquedaTexto", json!("")),
                    ("busquedaApps", json!([])),
                    ("busquedaEstado", json!("listo")),
                ],
            );
            return;
        }
        self.poner(
            Grupo::Busqueda,
            vec![
                ("busquedaTexto", json!(texto)),
                ("busquedaEstado", json!("cargando")),
            ],
        );
        let m = self.motor.clone();
        self.lanzar(
            false,
            move |_| m.buscar(&texto),
            move |d, apps| {
                if vuelta != d.gen_busqueda {
                    return; // ya se escribió otra cosa
                }
                let lista: Vec<Value> = apps.iter().map(app_a_dict).collect();
                for x in &lista {
                    d.recordar(x);
                }
                d.poner(
                    Grupo::Busqueda,
                    vec![
                        ("busquedaApps", Value::Array(lista)),
                        ("busquedaEstado", json!("listo")),
                    ],
                );
            },
            move |d, e| {
                if vuelta == d.gen_busqueda {
                    d.poner(
                        Grupo::Busqueda,
                        vec![
                            ("busquedaApps", json!([])),
                            ("busquedaEstado", json!(clasificar(&e).0)),
                        ],
                    );
                }
            },
        );
    }

    fn reintentar_busqueda(&mut self) {
        let t = self.get_str("busquedaTexto").to_string();
        self.buscar(&t);
    }

    // --- Detalle de una app

    fn abrir_detalle(&mut self, app_id: &str) {
        self.gen_detalle += 1;
        let vuelta = self.gen_detalle;
        let mut base = self.info.get(app_id).cloned().unwrap_or_else(|| marcador(app_id));
        if let Some(o) = base.as_object_mut() {
            for (k, x) in [
                ("descripcion", json!("")),
                ("capturas", json!([])),
                ("licencia", json!("")),
                ("web", json!("")),
                ("version", json!("")),
                ("tamano", json!(0)),
                ("tamanoTexto", json!("")),
                ("categorias", json!([])),
            ] {
                o.insert(k.into(), x);
            }
        }
        self.poner(
            Grupo::Detalle,
            vec![("detalle", base), ("detalleEstado", json!("cargando"))],
        );
        let (m, id) = (self.motor.clone(), app_id.to_string());
        self.lanzar(
            false,
            move |_| m.detalle(&id),
            move |d, ficha| {
                if vuelta != d.gen_detalle {
                    return;
                }
                let ficha = detalle_a_dict(&ficha);
                d.recordar(&ficha);
                d.poner(
                    Grupo::Detalle,
                    vec![("detalle", ficha), ("detalleEstado", json!("listo"))],
                );
                d.reconstruir_listas();
            },
            move |d, e| {
                if vuelta == d.gen_detalle {
                    d.poner(Grupo::Detalle, vec![("detalleEstado", json!(clasificar(&e).0))]);
                }
            },
        );
    }

    fn reintentar_detalle(&mut self) {
        let d = self.get("detalle").clone();
        if d.as_object().is_some_and(|o| !o.is_empty()) {
            self.abrir_detalle(texto_de(&d, "id"));
        }
    }

    // --- Fichas conocidas, instaladas y actualizaciones

    fn recordar(&mut self, ficha: &Value) {
        let mut m = Map::new();
        for k in CLAVES_BASE {
            m.insert(k.into(), ficha.get(k).cloned().unwrap_or(Value::Null));
        }
        self.info
            .insert(texto_de(ficha, "id").to_string(), Value::Object(m));
    }

    fn refrescar_apps_locales(&mut self) {
        self.refrescar_instaladas();
        self.refrescar_actualizaciones();
        let l = Value::Array(self.propias.lista());
        self.poner(Grupo::Propias, vec![("propias", l)]);
    }

    fn refrescar_instaladas(&mut self) {
        let m = self.motor.clone();
        self.lanzar_av(
            false,
            move |_| {
                // en el hilo: flatpak bloquea
                let ids: Vec<String> = m.instaladas()?.into_iter().filter(|i| es_app(i)).collect();
                // sin saberlo, se deja quitar (y flatpak dirá si no)
                let sistema: HashSet<String> = ids
                    .iter()
                    .filter(|i| m.es_del_sistema(i).unwrap_or(false))
                    .cloned()
                    .collect();
                Ok((ids, sistema))
            },
            |d, (ids, sistema)| {
                d.ids_instaladas = ids;
                d.ids_sistema = sistema;
                d.reconstruir_listas();
            },
        );
    }

    fn refrescar_actualizaciones(&mut self) {
        self.poner(
            Grupo::Actualizaciones,
            vec![("revisandoActualizaciones", json!(true))],
        );
        let m = self.motor.clone();
        self.lanzar(
            false,
            move |_| m.actualizaciones(),
            |d, ids| {
                d.ids_actualizables = ids.into_iter().filter(|i| es_app(i)).collect();
                d.v.insert("revisandoActualizaciones".into(), json!(false));
                d.reconstruir_listas();
            },
            // sin red no se sabe: se deja lo anterior, sin molestar
            |d, _| {
                d.poner(
                    Grupo::Actualizaciones,
                    vec![("revisandoActualizaciones", json!(false))],
                )
            },
        );
    }

    /// Rearma «Instaladas» y «Actualizaciones» con lo que se sabe y pide lo que falta.
    fn reconstruir_listas(&mut self) {
        let mut vistos = HashSet::new();
        let faltan: Vec<String> = self
            .ids_instaladas
            .iter()
            .chain(self.ids_actualizables.iter())
            .filter(|i| vistos.insert((*i).clone()))
            .filter(|i| {
                !self.info.contains_key(*i) && !self.pidiendo.contains(*i) && !self.sin_ficha.contains(*i)
            })
            .cloned()
            .collect();
        for i in faltan {
            self.pedir_ficha(&i);
        }
        let ordenada = |d: &Datos, ids: &[String]| -> Vec<Value> {
            let mut v: Vec<Value> = ids
                .iter()
                .map(|i| d.info.get(i).cloned().unwrap_or_else(|| marcador(i)))
                .collect();
            v.sort_by_key(|f| texto_de(f, "nombre").to_lowercase());
            v
        };
        let mapa = |ids: &[String]| Value::Object(ids.iter().map(|i| (i.clone(), json!(true))).collect());
        let instaladas = ordenada(self, &self.ids_instaladas);
        let actualizables = ordenada(self, &self.ids_actualizables);
        let sistema: Vec<String> = self.ids_sistema.iter().cloned().collect();
        self.v.insert("instaladas".into(), Value::Array(instaladas));
        self.v.insert("instaladasIds".into(), mapa(&self.ids_instaladas));
        self.v.insert("sistemaIds".into(), mapa(&sistema));
        self.v.insert("actualizables".into(), Value::Array(actualizables));
        self.v
            .insert("actualizablesIds".into(), mapa(&self.ids_actualizables));
        self.v
            .insert("numActualizaciones".into(), json!(self.ids_actualizables.len()));
        self.eventos.push(Evento::Cambio(Grupo::Instaladas));
        self.eventos.push(Evento::Cambio(Grupo::Actualizaciones));
    }

    fn pedir_ficha(&mut self, app_id: &str) {
        self.pidiendo.insert(app_id.to_string());
        let (m, id, id_ok, id_err) = (
            self.motor.clone(),
            app_id.to_string(),
            app_id.to_string(),
            app_id.to_string(),
        );
        self.lanzar(
            false,
            move |_| m.detalle(&id),
            move |d, ficha| {
                d.pidiendo.remove(&id_ok);
                d.recordar(&app_a_dict(&ficha.app));
                d.reconstruir_listas();
            },
            // sin red: se queda con el nombre sacado del id
            move |d, _| {
                d.pidiendo.remove(&id_err);
                d.sin_ficha.insert(id_err);
            },
        );
    }

    // --- Apps de Vortexine's (no son de Flathub: las instala «vortex <app> instalar»)

    fn estado_propia(&mut self, app_id: &str, fase: Option<&str>) {
        let mut estados = self
            .get("propiasEstados")
            .as_object()
            .cloned()
            .unwrap_or_default();
        match fase {
            Some(f) => {
                estados.insert(app_id.into(), json!(f));
            }
            None => {
                estados.remove(app_id);
            }
        }
        self.poner(Grupo::Propias, vec![("propiasEstados", Value::Object(estados))]);
    }

    fn accion_propia(&mut self, app_id: &str, fase: &'static str, f: fn(&Propias, &str) -> Resultado<()>) {
        if self.get("propiasEstados").get(app_id).is_some() {
            return;
        }
        self.estado_propia(app_id, Some(fase));
        let (p, id, id_ok, id_err) = (
            self.propias.clone(),
            app_id.to_string(),
            app_id.to_string(),
            app_id.to_string(),
        );
        self.lanzar(
            true,
            move |_| f(&p, &id),
            move |d, ()| {
                d.estado_propia(&id_ok, None);
                let l = Value::Array(d.propias.lista());
                d.poner(Grupo::Propias, vec![("propias", l)]);
                let nombre = d.propias.app(&id_ok).map_or("La app", |a| a.nombre);
                let texto = if fase == "instalando" {
                    format!("{nombre} quedó instalada: ábrela desde aquí o desde el lanzador.")
                } else {
                    format!("{nombre} se quitó.")
                };
                d.listo(texto);
            },
            move |d, e| {
                d.estado_propia(&id_err, None);
                d.avisar_error(&e);
            },
        );
    }

    fn abrir_propia(&mut self, app_id: &str) {
        if self.get("propiasEstados").get(app_id).is_some() {
            return;
        }
        self.estado_propia(app_id, Some("abriendo"));
        let id = app_id.to_string();
        self.pendientes.push(Pendiente::Despues(
            self.espera_abrir,
            Box::new(move |d| {
                if d.get("propiasEstados").get(&id).and_then(Value::as_str) == Some("abriendo") {
                    d.estado_propia(&id, None);
                }
            }),
        ));
        let (p, id, id_err) = (self.propias.clone(), app_id.to_string(), app_id.to_string());
        self.lanzar(
            false,
            move |_| p.abrir(&id),
            |_, ()| {},
            move |d, e| {
                d.estado_propia(&id_err, None);
                d.avisar_error(&e);
            },
        );
    }

    // --- Juegos de Android (Waydroid, con «vortex-android»)

    /// Al entrar a la página: si Android está listo y qué juegos hay. No prende Android.
    fn cargar_android(&mut self) {
        let a = self.android.clone();
        self.lanzar(
            false,
            move |_| {
                let est = a.estado();
                let apps = if est.preparado { a.apps() } else { Vec::new() };
                Ok((est, apps))
            },
            |d, (est, apps)| {
                d.poner(
                    Grupo::Android,
                    vec![
                        ("androidEstado", est.a_json()),
                        (
                            "androidApps",
                            Value::Array(apps.iter().map(android::AppAndroid::a_json).collect()),
                        ),
                        ("androidCargado", json!(true)),
                    ],
                );
            },
            |d, e| {
                d.poner(Grupo::Android, vec![("androidCargado", json!(true))]);
                d.avisar_error(&e);
            },
        );
    }

    fn accion_android(
        &mut self,
        fase: &str,
        paso: &str,
        f: impl FnOnce(&Arc<Tienda>, &Android) -> Resultado<()> + Send + 'static,
        al_terminar: String,
        recargar: bool,
    ) {
        if !self.get_str("androidFase").is_empty() {
            return;
        }
        self.poner(
            Grupo::Android,
            vec![
                ("androidFase", json!(fase)),
                ("androidPaso", json!(paso)),
                ("androidPct", json!(0)),
            ],
        );
        let a = self.android.clone();
        self.lanzar(
            true,
            move |t| f(t, &a),
            move |d, ()| {
                d.poner(
                    Grupo::Android,
                    vec![("androidFase", json!("")), ("androidPaso", json!(""))],
                );
                if !al_terminar.is_empty() {
                    d.listo(al_terminar);
                }
                if recargar {
                    d.cargar_android();
                }
            },
            |d, e| {
                d.poner(
                    Grupo::Android,
                    vec![("androidFase", json!("")), ("androidPaso", json!(""))],
                );
                d.avisar_error(&e);
                d.cargar_android();
            },
        );
    }

    fn preparar_android(&mut self) {
        self.accion_android(
            "preparando",
            "Pidiendo permiso…",
            |t, a| {
                a.preparar(&|pct, txt| {
                    let txt = txt.to_string();
                    t.entrar(move |d| d.al_progresar("android", pct, &txt));
                })
            },
            "Android quedó listo: busca un juego o abre la Play Store.".into(),
            true,
        );
    }

    fn android_pedir_id(&mut self) {
        if !self.get_str("androidFase").is_empty() {
            return;
        }
        self.poner(
            Grupo::Android,
            vec![
                ("androidFase", json!("abriendo")),
                ("androidPaso", json!("Leyendo el número de Android…")),
            ],
        );
        let a = self.android.clone();
        self.lanzar(
            true,
            move |_| a.id_google(),
            |d, numero| {
                d.poner(
                    Grupo::Android,
                    vec![
                        ("androidFase", json!("")),
                        ("androidPaso", json!("")),
                        ("androidId", json!(numero)),
                    ],
                );
            },
            |d, e| {
                d.poner(
                    Grupo::Android,
                    vec![("androidFase", json!("")), ("androidPaso", json!(""))],
                );
                d.avisar_error(&e);
            },
        );
    }

    // --- Operaciones sobre apps (cola de una en una)

    fn encolar(&mut self, tipo: &'static str, app_id: &str) {
        if let Some(est) = self.get("estados").get(app_id)
            && texto_de(est, "fase") != "abriendo"
        {
            return; // ya está en la cola o en curso
        }
        self.poner_estado(
            app_id,
            json!({"fase": "cola", "tipo": tipo, "pct": 0, "texto": "En cola"}),
        );
        let (m, preparado) = (self.motor.clone(), self.preparado.clone());
        let (id, id_ok, id_err) = (app_id.to_string(), app_id.to_string(), app_id.to_string());
        self.lanzar(
            true,
            move |t| {
                let aviso = |pct: i32, txt: &str| {
                    let (id, txt) = (id.clone(), txt.to_string());
                    t.entrar(move |d| d.al_progresar(&id, pct, &txt));
                };
                aviso(0, "Preparando…");
                preparar_flatpak(&*m, &preparado)?;
                match tipo {
                    "instalar" => m.instalar(&id, &aviso),
                    "quitar" => m.quitar(&id, &aviso),
                    _ => m.actualizar(std::slice::from_ref(&id), &aviso),
                }
            },
            move |d, ()| {
                d.quitar_estado(&id_ok);
                if tipo == "instalar" && !d.ids_instaladas.contains(&id_ok) {
                    d.ids_instaladas.push(id_ok.clone());
                } else if tipo == "quitar" {
                    d.ids_instaladas.retain(|i| *i != id_ok);
                }
                if tipo == "actualizar" || tipo == "quitar" {
                    d.ids_actualizables.retain(|i| *i != id_ok);
                }
                d.reconstruir_listas();
                d.refrescar_apps_locales(); // y se confirma con lo que diga flatpak de verdad
            },
            move |d, e| {
                d.quitar_estado(&id_err);
                d.avisar_error(&e);
            },
        );
    }

    fn al_progresar(&mut self, app_id: &str, pct: i32, texto: &str) {
        let pct = pct.clamp(0, 100);
        if app_id == "android" {
            self.poner(
                Grupo::Android,
                vec![("androidPct", json!(pct)), ("androidPaso", json!(texto))],
            );
            return;
        }
        let Some(est) = self.get("estados").get(app_id).cloned() else {
            return;
        };
        if texto_de(&est, "fase") == "abriendo" {
            return;
        }
        let tipo = texto_de(&est, "tipo").to_string();
        let nuevo = json!({"fase": fase_de(&tipo), "tipo": tipo, "pct": pct, "texto": texto});
        if nuevo != est {
            self.poner_estado(app_id, nuevo);
        }
    }

    fn poner_estado(&mut self, app_id: &str, est: Value) {
        let mut estados = self.get("estados").as_object().cloned().unwrap_or_default();
        estados.insert(app_id.into(), est); // objeto nuevo: QML se entera
        self.v.insert("estados".into(), Value::Object(estados));
        self.estados_cambiaron();
    }

    fn quitar_estado(&mut self, app_id: &str) {
        let mut estados = self.get("estados").as_object().cloned().unwrap_or_default();
        estados.remove(app_id);
        self.v.insert("estados".into(), Value::Object(estados));
        self.estados_cambiaron();
    }

    fn estados_cambiaron(&mut self) {
        self.eventos.push(Evento::Cambio(Grupo::Estados));
        let ocupado = self
            .get("estados")
            .as_object()
            .is_some_and(|m| m.values().any(|e| texto_de(e, "fase") != "abriendo"));
        if ocupado != self.get_bool("ocupado") {
            self.poner(Grupo::Ocupado, vec![("ocupado", json!(ocupado))]);
        }
    }

    fn dejar_de_abrir(&mut self, app_id: &str) {
        if self.get("estados").get(app_id).map(|e| texto_de(e, "fase")) == Some("abriendo") {
            self.quitar_estado(app_id);
        }
    }

    fn abrir(&mut self, app_id: &str) {
        if self.get("estados").get(app_id).is_some() {
            return;
        }
        self.poner_estado(
            app_id,
            json!({"fase": "abriendo", "tipo": "abrir", "pct": 0, "texto": "Abriendo…"}),
        );
        // La app tarda unos segundos en aparecer: el botón queda en «Abriendo…» para no abrirla dos veces.
        let id = app_id.to_string();
        self.pendientes.push(Pendiente::Despues(
            self.espera_abrir,
            Box::new(move |d| d.dejar_de_abrir(&id)),
        ));
        let (m, id, id_err) = (self.motor.clone(), app_id.to_string(), app_id.to_string());
        self.lanzar(
            false,
            move |_| m.abrir(&id),
            |_, ()| {},
            move |d, e| {
                d.quitar_estado(&id_err);
                d.avisar_error(&e);
            },
        );
    }
}

fn preparar_flatpak(motor: &dyn Motor, preparado: &AtomicBool) -> Resultado<()> {
    if !preparado.load(Ordering::SeqCst) {
        motor.preparar()?;
        preparado.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// La tienda: el estado, la cola y los hilos. Se comparte con `Arc`.
pub struct Tienda {
    datos: Mutex<Datos>,
    sumidero: Sumidero,
    pool: Arc<Pool>,
    pool_ops: Arc<Pool>,
    /// Cada cuánto se revisan actualizaciones solas (30 min).
    intervalo_refresco: Mutex<Duration>,
}

impl Tienda {
    /// `sumidero` recibe cada [`Evento`] (desde el hilo que lo causó, con el cerrojo ya suelto).
    pub fn nueva(
        motor: Arc<dyn Motor>,
        propias: Arc<Propias>,
        android: Arc<Android>,
        sumidero: Sumidero,
    ) -> Arc<Self> {
        Arc::new(Tienda {
            datos: Mutex::new(Datos::nuevo(motor, propias, android)),
            sumidero,
            pool: Pool::nuevo(6),     // consultas (red): varias a la vez
            pool_ops: Pool::nuevo(1), // instalar/quitar/actualizar: de una en una
            intervalo_refresco: Mutex::new(Duration::from_secs(30 * 60)),
        })
    }

    /// Cuánto dura el «Abriendo…» de un botón (las pruebas lo acortan).
    pub fn poner_espera_abrir(&self, d: Duration) {
        self.datos.lock().unwrap_or_else(|x| x.into_inner()).espera_abrir = d;
    }

    pub fn poner_intervalo_refresco(&self, d: Duration) {
        *self.intervalo_refresco.lock().unwrap_or_else(|x| x.into_inner()) = d;
    }

    /// Toma el cerrojo, aplica `f` al estado y, ya suelto el cerrojo, reparte lo que dejó (eventos y tareas).
    pub fn entrar<R>(self: &Arc<Self>, f: impl FnOnce(&mut Datos) -> R) -> Option<R> {
        let (r, eventos, pendientes) = {
            let mut d = self.datos.lock().unwrap_or_else(|x| x.into_inner());
            // un fallo al pintar no debe tumbar la tienda
            let r = catch_unwind(AssertUnwindSafe(|| f(&mut d))).ok();
            (
                r,
                std::mem::take(&mut d.eventos),
                std::mem::take(&mut d.pendientes),
            )
        };
        for e in eventos {
            (self.sumidero)(e);
        }
        for p in pendientes {
            match p {
                Pendiente::Trabajo(t, cola) => {
                    let tienda = Arc::clone(self);
                    let pool = if cola { &self.pool_ops } else { &self.pool };
                    pool.enviar(Box::new(move || {
                        let aplicar = t(&tienda);
                        tienda.entrar(aplicar);
                    }));
                }
                Pendiente::Despues(espera, aplicar) => {
                    let tienda = Arc::downgrade(self);
                    let _ = std::thread::Builder::new()
                        .name("vortex-shop-espera".into())
                        .spawn(move || {
                            std::thread::sleep(espera);
                            if let Some(t) = tienda.upgrade() {
                                t.entrar(aplicar);
                            }
                        });
                }
            }
        }
        r
    }

    // --- lo que lee la interfaz

    /// El valor de una propiedad (JSON).
    pub fn leer(&self, clave: &str) -> Value {
        self.datos
            .lock()
            .unwrap_or_else(|x| x.into_inner())
            .get(clave)
            .clone()
    }

    /// Todas las propiedades juntas (para las pruebas).
    pub fn todo(&self) -> Value {
        Value::Object(self.datos.lock().unwrap_or_else(|x| x.into_inner()).v.clone())
    }

    // --- los «slots» (lo que la interfaz le pide a la tienda)

    /// Carga lo primero que se ve y deja listo flatpak. Se llama una vez, al abrir la ventana.
    pub fn iniciar(self: &Arc<Self>) {
        self.entrar(Datos::iniciar);
        let debil: Weak<Tienda> = Arc::downgrade(self);
        let _ = std::thread::Builder::new()
            .name("vortex-shop-refresco".into())
            .spawn(move || {
                loop {
                    let Some(t) = debil.upgrade() else { return };
                    let espera = *t.intervalo_refresco.lock().unwrap_or_else(|x| x.into_inner());
                    drop(t);
                    std::thread::sleep(espera);
                    let Some(t) = debil.upgrade() else { return };
                    t.entrar(Datos::refrescar_apps_locales);
                }
            });
    }

    pub fn reintentar_todo(self: &Arc<Self>) {
        self.entrar(Datos::reintentar_todo);
    }

    pub fn cargar_categorias(self: &Arc<Self>) {
        self.entrar(Datos::cargar_categorias);
    }

    pub fn cargar_inicio(self: &Arc<Self>) {
        self.entrar(Datos::cargar_inicio);
    }

    pub fn abrir_categoria(self: &Arc<Self>, cat_id: &str) {
        self.entrar(|d| d.abrir_categoria(cat_id));
    }

    pub fn mas_de_categoria(self: &Arc<Self>) {
        self.entrar(Datos::mas_de_categoria);
    }

    pub fn reintentar_categoria(self: &Arc<Self>) {
        self.entrar(Datos::reintentar_categoria);
    }

    pub fn buscar(self: &Arc<Self>, texto: &str) {
        self.entrar(|d| d.buscar(texto));
    }

    pub fn reintentar_busqueda(self: &Arc<Self>) {
        self.entrar(Datos::reintentar_busqueda);
    }

    pub fn abrir_detalle(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.abrir_detalle(app_id));
    }

    pub fn reintentar_detalle(self: &Arc<Self>) {
        self.entrar(Datos::reintentar_detalle);
    }

    pub fn buscar_actualizaciones(self: &Arc<Self>) {
        self.entrar(Datos::refrescar_actualizaciones);
    }

    pub fn instalar_propia(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.accion_propia(app_id, "instalando", Propias::instalar));
    }

    pub fn quitar_propia(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.accion_propia(app_id, "quitando", Propias::quitar));
    }

    pub fn abrir_propia(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.abrir_propia(app_id));
    }

    pub fn cargar_android(self: &Arc<Self>) {
        self.entrar(Datos::cargar_android);
    }

    pub fn preparar_android(self: &Arc<Self>) {
        self.entrar(Datos::preparar_android);
    }

    pub fn android_tienda(self: &Arc<Self>, paquete: &str) {
        let p = paquete.to_string();
        self.entrar(|d| {
            d.accion_android(
                "abriendo",
                "Abriendo la Play Store…",
                move |_, a| a.tienda(&p),
                String::new(),
                true,
            );
        });
    }

    pub fn android_buscar(self: &Arc<Self>, texto: &str) {
        if texto.trim().is_empty() {
            return;
        }
        let t = texto.to_string();
        self.entrar(|d| {
            d.accion_android(
                "abriendo",
                "Buscando en la Play Store…",
                move |_, a| a.buscar(&t),
                String::new(),
                true,
            );
        });
    }

    pub fn android_abrir(self: &Arc<Self>, paquete: &str) {
        let p = paquete.to_string();
        self.entrar(|d| {
            d.accion_android(
                "abriendo",
                "Abriendo el juego…",
                move |_, a| a.abrir(&p),
                String::new(),
                true,
            );
        });
    }

    pub fn android_quitar(self: &Arc<Self>, paquete: &str, nombre: &str) {
        let (p, n) = (paquete.to_string(), nombre.to_string());
        self.entrar(|d| {
            d.accion_android(
                "quitando",
                &format!("Quitando {n}…"),
                move |_, a| a.quitar(&p),
                format!("{n} se quitó."),
                true,
            );
        });
    }

    pub fn android_pedir_id(self: &Arc<Self>) {
        self.entrar(Datos::android_pedir_id);
    }

    pub fn android_abrir_registro(self: &Arc<Self>) {
        self.entrar(|d| d.eventos.push(Evento::AbrirUrl(android::URL_REGISTRO.into())));
    }

    pub fn android_ya_registre(self: &Arc<Self>) {
        self.entrar(|d| {
            d.accion_android(
                "abriendo",
                "Guardando…",
                |_, a| a.registrado(),
                "Listo. Si la Play Store aún no te deja entrar, espera unos minutos y ábrela otra vez."
                    .into(),
                true,
            );
        });
    }

    pub fn instalar(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.encolar("instalar", app_id));
    }

    pub fn quitar(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.encolar("quitar", app_id));
    }

    pub fn actualizar(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.encolar("actualizar", app_id));
    }

    pub fn actualizar_todo(self: &Arc<Self>) {
        self.entrar(|d| {
            for i in d.ids_actualizables.clone() {
                d.encolar("actualizar", &i);
            }
        });
    }

    pub fn abrir(self: &Arc<Self>, app_id: &str) {
        self.entrar(|d| d.abrir(app_id));
    }
}
