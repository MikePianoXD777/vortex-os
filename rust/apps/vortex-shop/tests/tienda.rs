//! Las pruebas de la tienda (el estado que ve la interfaz): lo que en Python hacen `ui_cola.py`,
//! `ui_android.py` y partes de `ui_integracion.py`, pero sin ventana. El motor es el doble falso
//! (`tests/comun/falso.rs`); las apps propias y Android usan programas falsos en una carpeta temporal.

mod comun;

use comun::falso::MotorFalso;
use comun::*;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_shop::android::Android;
use vortex_shop::propias::Propias;
use vortex_shop::tienda::{self, Evento, Grupo, Tienda};

struct Arnes {
    tienda: Arc<Tienda>,
    motor: Arc<MotorFalso>,
    eventos: Arc<Mutex<Vec<Evento>>>,
    t: tempfile::TempDir,
}

fn arnes(duracion_ms: u64, instaladas: &[&str], actualizaciones: &[&str]) -> Arnes {
    let t = tmp();
    let motor = Arc::new(MotorFalso::nuevo(duracion_ms, instaladas, actualizaciones));
    let eventos: Arc<Mutex<Vec<Evento>>> = Arc::default();
    let e2 = eventos.clone();
    let qml = aqui().parent().unwrap().join("qt/qml");
    let propias = Arc::new(Propias::nuevo(qml, entorno_vortex_falso(t.path())));
    let android = Arc::new(Android::nuevo(entorno_android_falso(t.path(), true)));
    let tienda = Tienda::nueva(
        motor.clone(),
        propias,
        android,
        Arc::new(move |e| e2.lock().unwrap().push(e)),
    );
    tienda.poner_espera_abrir(Duration::from_millis(250));
    Arnes {
        tienda,
        motor,
        eventos,
        t,
    }
}

fn esperar_hasta(limite_s: f64, cond: impl Fn() -> bool) -> bool {
    let fin = Instant::now() + Duration::from_secs_f64(limite_s);
    while Instant::now() < fin {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    cond()
}

impl Arnes {
    fn texto(&self, k: &str) -> String {
        self.tienda.leer(k).as_str().unwrap_or("").to_string()
    }

    fn lista(&self, k: &str) -> Vec<Value> {
        self.tienda.leer(k).as_array().cloned().unwrap_or_default()
    }

    fn ids(&self, k: &str) -> Vec<String> {
        self.lista(k)
            .iter()
            .map(|a| a["id"].as_str().unwrap().to_string())
            .collect()
    }

    fn avisos(&self) -> Vec<String> {
        self.eventos
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| {
                if let Evento::Aviso(t) = e {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn listos(&self) -> Vec<String> {
        self.eventos
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| {
                if let Evento::Listo(t) = e {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn ocupado(&self) -> bool {
        self.tienda.leer("ocupado") == json!(true)
    }

    fn registro(&self) -> Vec<String> {
        self.motor.control.registro.lock().unwrap().clone()
    }
}

#[test]
fn el_arranque_carga_inicio_categorias_y_listas_locales() {
    let a = arnes(
        100,
        &["org.mozilla.firefox", "org.audacityteam.Audacity"],
        &["org.mozilla.firefox"],
    );
    a.motor
        .control
        .del_sistema
        .lock()
        .unwrap()
        .insert("org.audacityteam.Audacity".into());
    assert_eq!(a.texto("inicioEstado"), "cargando");
    a.tienda.iniciar();
    assert!(
        esperar_hasta(10.0, || a.texto("inicioEstado") == "listo"),
        "el inicio carga"
    );
    assert_eq!(a.lista("recomendadas").len(), 6);
    assert!(!a.lista("populares").is_empty());
    assert!(
        esperar_hasta(5.0, || a.lista("categorias").len() == 8),
        "las categorías llegan"
    );
    assert!(
        esperar_hasta(5.0, || a.lista("instaladas").len() == 2
            && a.tienda.leer("numActualizaciones") == json!(1)),
        "instaladas y actualizaciones se cargan (contador = 1)"
    );
    // una app del sistema se marca (Quitar deshabilitado)
    assert_eq!(
        a.tienda.leer("sistemaIds")["org.audacityteam.Audacity"],
        json!(true)
    );
    assert_eq!(a.tienda.leer("sistemaIds")["org.mozilla.firefox"], Value::Null);
    // las listas salen en orden alfabético y con la ficha completa
    assert_eq!(
        a.ids("instaladas"),
        ["org.audacityteam.Audacity", "org.mozilla.firefox"]
    );
    assert_eq!(a.lista("instaladas")[1]["nombre"], "Firefox");
    assert_eq!(
        a.lista("populares")[0]["descargasTexto"],
        "920 mil descargas al mes"
    );
    // los avisos de cambio llegan por grupo
    let ev = a.eventos.lock().unwrap().clone();
    for g in [
        Grupo::Inicio,
        Grupo::Categorias,
        Grupo::Instaladas,
        Grupo::Actualizaciones,
        Grupo::Propias,
    ] {
        assert!(ev.contains(&Evento::Cambio(g)), "{g:?}");
    }
    assert_eq!(a.tienda.leer("propias").as_array().unwrap().len(), 2);
}

#[test]
fn la_cola_instala_de_una_en_una_en_el_orden_pedido() {
    let a = arnes(1200, &["org.mozilla.firefox"], &[]);
    a.tienda.iniciar();
    assert!(esperar_hasta(10.0, || a.texto("inicioEstado") == "listo"));
    let ids = ["org.videolan.VLC", "org.gimp.GIMP", "org.kde.krita"];
    for i in ids {
        a.tienda.instalar(i);
    }
    assert!(a.ocupado(), "ocupado = true al encolar");
    // la primera puede pasar a «instalando» enseguida; la última siempre espera su turno
    assert_eq!(a.tienda.leer("estados")[ids[2]]["fase"], "cola");
    assert!(esperar_hasta(5.0, || a.tienda.leer("estados")[ids[0]]["pct"]
        .as_i64()
        .unwrap_or(0)
        > 0));
    let est = a.tienda.leer("estados");
    assert_eq!(
        est[ids[0]]["fase"], "instalando",
        "la primera se está instalando con porcentaje"
    );
    assert_eq!(
        (est[ids[1]]["fase"].as_str(), est[ids[2]]["fase"].as_str()),
        (Some("cola"), Some("cola"))
    );
    a.tienda.instalar(ids[0]); // repetida: se ignora
    assert!(esperar_hasta(30.0, || !a.ocupado()), "la cola termina");
    let esperado: Vec<String> = ids.iter().map(|i| format!("instalar {i}")).collect();
    assert_eq!(
        a.registro(),
        esperado,
        "se instalaron en el orden pedido, una por una"
    );
    assert!(
        ids.iter()
            .all(|i| a.tienda.leer("instaladasIds")[*i] == json!(true)),
        "las tres quedan instaladas"
    );
    assert_eq!(a.tienda.leer("estados"), json!({}));
}

#[test]
fn un_error_de_flatpak_avisa_en_espanol_y_no_deja_la_app_instalada() {
    let a = arnes(200, &[], &[]);
    a.tienda.iniciar();
    a.motor
        .control
        .fallan
        .lock()
        .unwrap()
        .insert("org.blender.Blender".into());
    a.tienda.instalar("org.blender.Blender");
    assert!(
        esperar_hasta(15.0, || a.avisos().len() == 1 && !a.ocupado()),
        "un error de flatpak lanza un aviso"
    );
    assert!(
        a.avisos()[0].contains("espacio en disco"),
        "el aviso lleva el mensaje en español del motor"
    );
    assert!(a.tienda.leer("instaladasIds")["org.blender.Blender"].is_null());
}

#[test]
fn quitar_y_actualizar_todo() {
    let a = arnes(
        100,
        &["org.videolan.VLC", "org.mozilla.firefox"],
        &["org.mozilla.firefox"],
    );
    a.tienda.iniciar();
    assert!(esperar_hasta(5.0, || a.tienda.leer("numActualizaciones") == json!(1)));
    a.tienda.quitar("org.videolan.VLC");
    assert!(
        esperar_hasta(10.0, || !a.ocupado()
            && a.tienda.leer("instaladasIds")["org.videolan.VLC"].is_null()),
        "quitar funciona"
    );
    a.tienda.actualizar_todo();
    assert!(
        esperar_hasta(10.0, || !a.ocupado()
            && a.tienda.leer("numActualizaciones") == json!(0)),
        "«Actualizar todo» deja el contador en 0"
    );
    assert_eq!(
        a.registro(),
        ["quitar org.videolan.VLC", "actualizar org.mozilla.firefox"]
    );
}

#[test]
fn una_busqueda_vieja_no_pisa_a_la_nueva() {
    let a = arnes(100, &[], &[]);
    a.tienda.buscar("a");
    a.tienda.buscar("vlc");
    assert!(esperar_hasta(5.0, || a.texto("busquedaEstado") == "listo"
        && a.texto("busquedaTexto") == "vlc"));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        a.ids("busquedaApps"),
        ["org.videolan.VLC"],
        "solo queda el resultado de la última búsqueda"
    );
    a.tienda.buscar("zzzz");
    assert!(esperar_hasta(5.0, || a.texto("busquedaEstado") == "listo"
        && a.texto("busquedaTexto") == "zzzz"));
    assert_eq!(a.lista("busquedaApps"), Vec::<Value>::new());
    a.tienda.buscar("   ");
    assert_eq!(
        (a.texto("busquedaTexto"), a.texto("busquedaEstado")),
        (String::new(), "listo".into())
    );
}

#[test]
fn categoria_con_mostrar_mas() {
    let a = arnes(100, &[], &[]);
    a.tienda.iniciar();
    assert!(esperar_hasta(5.0, || a.lista("categorias").len() == 8));
    a.tienda.abrir_categoria("Network");
    assert_eq!(a.texto("categoriaEstado"), "cargando");
    assert_eq!(a.tienda.leer("categoriaActual")["nombre"], "Internet");
    assert!(
        esperar_hasta(5.0, || a.texto("categoriaEstado") == "listo"),
        "una categoría carga"
    );
    let primera = a.lista("categoriaApps").len();
    assert!(
        primera == 6 && a.tienda.leer("categoriaHayMas") == json!(true),
        "página llena: hay más"
    );
    a.tienda.mas_de_categoria();
    assert_eq!(a.tienda.leer("categoriaCargandoMas"), json!(true));
    assert!(
        esperar_hasta(5.0, || a.lista("categoriaApps").len() > primera),
        "«Mostrar más» agrega la siguiente página"
    );
    assert!(
        esperar_hasta(5.0, || a.tienda.leer("categoriaHayMas") == json!(false)),
        "la última página (incompleta) oculta «Mostrar más»"
    );
    // otra categoría mientras la anterior carga: la respuesta vieja no pisa
    a.tienda.abrir_categoria("Game");
    a.tienda.abrir_categoria("Graphics");
    assert!(esperar_hasta(5.0, || a.texto("categoriaEstado") == "listo"));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        a.ids("categoriaApps"),
        ["org.gimp.GIMP", "org.kde.krita", "org.blender.Blender"]
    );
}

#[test]
fn sin_red_el_inicio_pasa_a_sinred_y_se_recupera() {
    let a = arnes(100, &[], &[]);
    a.tienda.iniciar();
    assert!(esperar_hasta(10.0, || a.texto("inicioEstado") == "listo"));
    a.motor.control.sin_red.store(true, Ordering::SeqCst);
    a.tienda.reintentar_todo();
    assert!(
        esperar_hasta(10.0, || a.texto("inicioEstado") == "sinred"),
        "sin red: el inicio pasa a «sinred»"
    );
    a.motor.control.sin_red.store(false, Ordering::SeqCst);
    a.tienda.reintentar_todo();
    assert!(
        esperar_hasta(10.0, || a.texto("inicioEstado") == "listo"),
        "con red otra vez: el inicio se recupera"
    );
    // el estado de una búsqueda sin red
    a.motor.control.sin_red.store(true, Ordering::SeqCst);
    a.tienda.buscar("vlc");
    assert!(esperar_hasta(5.0, || a.texto("busquedaEstado") == "sinred"));
}

#[test]
fn el_detalle_carga_y_se_reintenta() {
    let a = arnes(100, &[], &[]);
    a.tienda.abrir_detalle("org.mozilla.firefox");
    assert_eq!(a.texto("detalleEstado"), "cargando");
    assert_eq!(a.tienda.leer("detalle")["nombre"], "Firefox"); // el marcador, hasta que llegue la ficha
    assert!(esperar_hasta(5.0, || a.texto("detalleEstado") == "listo"));
    let d = a.tienda.leer("detalle");
    assert_eq!(
        (d["version"].as_str(), d["tamanoTexto"].as_str()),
        (Some("1.2.3"), Some("82 MB"))
    );
    assert!(d["descripcion"].as_str().unwrap().contains("• Fácil de usar."));
    // una que no existe
    a.tienda.abrir_detalle("org.no.Existe");
    assert!(esperar_hasta(5.0, || a.texto("detalleEstado") == "error"));
    a.motor.control.sin_red.store(true, Ordering::SeqCst);
    a.tienda.reintentar_detalle();
    assert!(esperar_hasta(5.0, || a.texto("detalleEstado") == "sinred"));
}

#[test]
fn abrir_deja_el_boton_en_abriendo_unos_segundos() {
    let a = arnes(100, &["org.videolan.VLC"], &[]);
    a.tienda.abrir("org.videolan.VLC");
    assert_eq!(a.tienda.leer("estados")["org.videolan.VLC"]["fase"], "abriendo");
    assert!(!a.ocupado(), "abrir no cuenta como ocupado");
    a.tienda.abrir("org.videolan.VLC"); // repetida: se ignora
    assert!(esperar_hasta(5.0, || a.tienda.leer("estados") == json!({})));
    assert_eq!(*a.motor.control.abiertas.lock().unwrap(), ["org.videolan.VLC"]);
    // una operación sí se puede pedir mientras «abre»
    a.tienda.abrir("org.videolan.VLC");
    a.tienda.quitar("org.videolan.VLC");
    assert_eq!(a.tienda.leer("estados")["org.videolan.VLC"]["fase"], "cola");
    assert!(esperar_hasta(10.0, || !a.ocupado()));
}

#[test]
fn las_apps_propias_se_instalan_y_se_quitan_con_su_aviso() {
    let a = arnes(100, &[], &[]);
    a.tienda.instalar_propia("vortex-camera");
    assert_eq!(a.tienda.leer("propiasEstados")["vortex-camera"], "instalando");
    assert!(esperar_hasta(10.0, || a.tienda.leer("propiasEstados") == json!({})));
    assert_eq!(a.tienda.leer("propias")[0]["instalada"], json!(true));
    assert_eq!(
        a.listos(),
        ["Vortexine's Camera quedó instalada: ábrela desde aquí o desde el lanzador."]
    );
    a.tienda.quitar_propia("vortex-camera");
    assert!(esperar_hasta(10.0, || a.listos().len() == 2));
    assert_eq!(a.listos()[1], "Vortexine's Camera se quitó.");
    assert_eq!(a.tienda.leer("propias")[0]["instalada"], json!(false));
    // si falla, el motivo llega como aviso
    std::fs::write(a.t.path().join("fallar"), "").unwrap();
    a.tienda.instalar_propia("vortex-camera");
    assert!(esperar_hasta(10.0, || a.avisos().len() == 1));
    assert_eq!(
        a.avisos()[0],
        "No se pudo instalar Vortexine's Camera: le falta: ffmpeg"
    );
    assert_eq!(a.tienda.leer("propiasEstados"), json!({}));
}

#[test]
fn android_prepara_pide_el_numero_y_abre_la_play_store() {
    let a = arnes(100, &[], &[]);
    a.tienda.cargar_android();
    assert!(esperar_hasta(5.0, || a.tienda.leer("androidCargado") == json!(true)));
    let est = a.tienda.leer("androidEstado");
    assert_eq!(est["preparado"], json!(false));
    assert!(
        est["nvidia"] == json!(true) || est["software"] == json!(true),
        "debía avisar que Android dibuja con el procesador"
    );

    a.tienda.preparar_android();
    assert_eq!(a.texto("androidFase"), "preparando");
    a.tienda.preparar_android(); // repetida mientras corre: se ignora
    assert!(
        esperar_hasta(10.0, || a.tienda.leer("androidPct").as_i64().unwrap_or(0) > 0),
        "avanza el porcentaje"
    );
    assert!(esperar_hasta(15.0, || a.texto("androidFase").is_empty()
        && a.tienda.leer("androidEstado")["preparado"] == json!(true)));
    assert_eq!(a.lista("androidApps").len(), 1);
    assert!(a.listos().iter().any(|t| t.starts_with("Android quedó listo")));

    a.tienda.android_pedir_id();
    assert!(esperar_hasta(5.0, || a.texto("androidId") == "3912345678901234567"));
    a.tienda.android_abrir_registro();
    assert!(a.eventos.lock().unwrap().contains(&Evento::AbrirUrl(
        "https://www.google.com/android/uncertified".into()
    )));
    a.tienda.android_ya_registre();
    assert!(esperar_hasta(
        10.0,
        || a.tienda.leer("androidEstado")["registrado"] == json!(true)
    ));
    a.tienda.android_tienda("com.supercell.clashroyale");
    a.tienda.android_buscar("   "); // vacío: no hace nada
    assert!(esperar_hasta(10.0, || a.texto("androidFase").is_empty()));
    let llamadas = std::fs::read_to_string(a.t.path().join("estado/llamadas")).unwrap();
    assert!(llamadas.lines().any(|l| l == "tienda com.supercell.clashroyale"));
    assert!(!llamadas.lines().any(|l| l.starts_with("buscar")));

    a.tienda.android_quitar("com.kiloo.subwaysurf", "Subway Surfers");
    assert!(esperar_hasta(10.0, || a
        .listos()
        .contains(&"Subway Surfers se quitó.".to_string())));
    assert!(a.avisos().is_empty(), "{:?}", a.avisos());
}

#[test]
fn android_un_error_avisa_y_no_deja_la_fase_pegada() {
    let a = arnes(100, &[], &[]);
    // sin el comando vortex-android: el aviso dice qué hacer
    let motor = a.motor.clone();
    let eventos: Arc<Mutex<Vec<Evento>>> = Arc::default();
    let e2 = eventos.clone();
    let propias = Arc::new(Propias::nuevo(aqui().join("nada"), Default::default()));
    let android = Arc::new(Android::nuevo(
        vortex_shop::proceso::Entorno::default().con("VORTEX_SHOP_ANDROID", "/no/existe"),
    ));
    let t = Tienda::nueva(
        motor,
        propias,
        android,
        Arc::new(move |e| e2.lock().unwrap().push(e)),
    );
    t.android_abrir("com.kiloo.subwaysurf");
    assert!(esperar_hasta(10.0, || {
        eventos
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, Evento::Aviso(m) if m.contains("Falta el comando vortex-android")))
    }));
    assert_eq!(t.leer("androidFase"), json!(""));
}

#[test]
fn las_actualizaciones_se_revisan_solas_cada_cierto_tiempo() {
    let a = arnes(100, &["org.videolan.VLC"], &[]);
    a.tienda.poner_intervalo_refresco(Duration::from_millis(150));
    a.tienda.iniciar();
    assert!(esperar_hasta(5.0, || a
        .motor
        .control
        .consultas_instaladas
        .load(Ordering::SeqCst)
        >= 3));
}

#[test]
fn conversiones_para_qml() {
    assert_eq!(tienda::tamano(Some(120 * 1024 * 1024)), "120 MB");
    assert_eq!(
        tienda::tamano(Some((1.4 * 1024.0 * 1024.0 * 1024.0) as i64)),
        "1,4 GB"
    );
    assert_eq!(tienda::tamano(Some(500)), "500 B");
    assert_eq!(tienda::tamano(None), "");
    assert_eq!(tienda::tamano(Some(0)), "");
    assert_eq!(tienda::descargas(Some(12_400)), "12 mil descargas al mes");
    assert_eq!(tienda::descargas(Some(2_500)), "2 mil descargas al mes"); // redondeo al par, como Python
    assert_eq!(tienda::descargas(Some(1_200_000)), "1,2 M descargas al mes");
    assert_eq!(tienda::descargas(Some(2_000_000)), "2 M descargas al mes");
    assert_eq!(tienda::descargas(Some(900)), "900 descargas al mes");
    assert_eq!(tienda::descargas(Some(0)), "");
    assert_eq!(tienda::marcador("org.mozilla.fire-fox_x")["nombre"], "Fire fox x");
    assert!(!tienda::es_app("org.freedesktop.Platform.GL.default"));
    assert!(!tienda::es_app("org.kde.Platform"));
    assert!(!tienda::es_app("org.gimp.GIMP.Locale"));
    assert!(tienda::es_app("org.gimp.GIMP"));
    use vortex_shop::errores::Error;
    assert_eq!(tienda::clasificar(&Error::sin_red()).0, "sinred");
    assert_eq!(tienda::clasificar(&Error::flatpak_sin_red()).0, "sinred");
    assert_eq!(
        tienda::clasificar(&Error::flatpak("x")),
        ("error", "x".to_string())
    );
    assert_eq!(
        tienda::clasificar(&Error::Otro("raro".into())).1,
        "Algo salió mal. Vuelve a intentarlo en un momento."
    );
}
