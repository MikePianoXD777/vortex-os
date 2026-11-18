//! Las pruebas del motor de la tienda (`test_motor.py`, portada): catálogo, texto, ids, flatpak, apps propias
//! y Android, sin internet y sin tocar el sistema.
//!
//! - El catálogo habla con un Flathub FALSO en 127.0.0.1 (`tests/comun`) que sirve respuestas reales
//!   guardadas en `tests/datos`. La caché va a una carpeta temporal.
//! - `flatpak` habla con un «flatpak» FALSO (`tests/falsos/flatpak_falso`) puesto al principio del PATH.
//!   Nunca se llama al flatpak, al pacman ni al vortex-android de verdad. HOME y XDG son temporales.
//!
//! (Python también prueba libflatpak por GObject, con un módulo falso; aquí esa ruta no existe.)

mod comun;

use comun::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_shop::android::Android;
use vortex_shop::catalogo::{self, Catalogo, RECOMENDADAS};
use vortex_shop::errores::Error;
use vortex_shop::ids::{es_id_valido, validar_id};
use vortex_shop::proceso::Entorno;
use vortex_shop::propias::{APPS, Propias};
use vortex_shop::texto::html_a_texto;

struct BaseCatalogo {
    t: tempfile::TempDir,
    srv: Servidor,
    cat: Catalogo,
}

fn base() -> BaseCatalogo {
    let t = tmp();
    let srv = Servidor::nuevo();
    let cat = Catalogo::nuevo(Some(&t.path().join("cache")), "es", Some(&srv.url));
    BaseCatalogo { t, srv, cat }
}

fn ids(apps: &[catalogo::App]) -> Vec<&str> {
    apps.iter().map(|a| a.id.as_str()).collect()
}

// ====================================================================== catálogo

#[test]
fn categorias() {
    let b = base();
    let cats = b.cat.categorias().unwrap();
    assert_eq!(cats.len(), 12); // IA + las 11 de Flathub
    assert_eq!((cats[0].id.as_str(), cats[0].nombre.as_str()), ("ia", "IA"));
    assert_eq!((cats[1].id.as_str(), cats[1].nombre.as_str()), ("game", "Juegos"));
    assert!(
        cats.iter()
            .all(|c| c.icono.ends_with("-symbolic") && !c.nombre.is_empty())
    );
    assert!(cats.iter().any(|c| c.id == "audiovideo"));
}

#[test]
fn categoria_nueva_de_flathub_sale_al_final() {
    let b = base();
    b.srv
        .datos_escribir("categorias.json", &json!(["game", "robotica", "../x"]));
    let cats = b.cat.categorias().unwrap();
    assert_eq!(
        cats.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["ia", "game", "robotica"]
    );
    assert_eq!(cats[2].nombre, "Robotica");
}

#[test]
fn destacados_completa_con_tendencias() {
    let b = base();
    let apps = b.cat.destacados().unwrap();
    assert_eq!(apps.len(), 6);
    assert!(apps.iter().all(|a| es_id_valido(&a.id) && !a.nombre.is_empty()));
    let unicos: std::collections::HashSet<_> = apps.iter().map(|a| &a.id).collect();
    assert_eq!(unicos.len(), 6);
    let pedidos = b.srv.pedidos().join("\n");
    assert!(pedidos.contains("GET /app-picks/app-of-the-day/"));
    assert!(pedidos.contains("GET /collection/trending?page=1&per_page=12&locale=es"));
}

#[test]
fn destacados_primero_las_selecciones_del_dia() {
    let b = base();
    b.srv
        .datos_escribir("app_del_dia.json", &json!({"app_id": "com.valvesoftware.Steam"}));
    b.srv.datos_escribir(
        "apps_de_la_semana.json",
        &json!({"apps": [
            {"app_id": "org.mozilla.firefox", "position": 2}, {"app_id": PRISM, "position": 1},
            {"app_id": "com.valvesoftware.Steam", "position": 3}, {"app_id": "no.existe.Nada", "position": 4}]}),
    );
    let apps = b.cat.destacados().unwrap();
    assert_eq!(
        ids(&apps[..3]),
        ["com.valvesoftware.Steam", PRISM, "org.mozilla.firefox"]
    );
    assert_eq!(apps.len(), 6);
    let unicos: std::collections::HashSet<_> = apps.iter().map(|a| &a.id).collect();
    assert_eq!(unicos.len(), 6);
}

#[test]
fn recomendadas_descarta_las_que_no_existen() {
    let b = base();
    let apps = b.cat.recomendadas().unwrap();
    let existen = [
        "com.valvesoftware.Steam",
        PRISM,
        "org.vinegarhq.Sober",
        "org.mozilla.firefox",
        "org.videolan.VLC",
        "com.spotify.Client",
    ];
    let mut a: Vec<&str> = ids(&apps);
    let en_orden: Vec<&str> = RECOMENDADAS
        .iter()
        .copied()
        .filter(|i| existen.contains(i))
        .collect();
    assert_eq!(a, en_orden); // en el orden de la lista
    a.sort_unstable();
    let mut e = existen.to_vec();
    e.sort_unstable();
    assert_eq!(a, e);
    let steam = &apps[0];
    assert_eq!(
        (steam.nombre.as_str(), steam.desarrollador.as_str()),
        ("Steam", "Valve Corporation")
    );
    assert!(steam.icono_url.starts_with("https://"));
    assert!(apps.iter().find(|a| a.id == PRISM).unwrap().verificado);
}

#[test]
fn recomendadas_ids_validos_y_sin_repetir() {
    let unicos: std::collections::HashSet<_> = RECOMENDADAS.iter().collect();
    assert_eq!(unicos.len(), RECOMENDADAS.len());
    assert!(RECOMENDADAS.iter().all(|i| es_id_valido(i)));
    for nombre in [
        "Steam",
        "PrismLauncher",
        "Sober",
        "hgl",
        "bottles",
        "Lutris",
        "pupgui2",
        "Discord",
        "Vesktop",
        "Browser",
        "firefox",
        "Client",
        "Studio",
        "VLC",
        "LibreOffice",
        "siyuan",
        "easyeffects",
        "Pegasus",
        "osu",
        "Zoom",
    ] {
        assert!(RECOMENDADAS.iter().any(|i| i.ends_with(nombre)), "{nombre}");
    }
}

#[test]
fn por_categoria() {
    let b = base();
    let apps = b.cat.por_categoria("game", 1).unwrap();
    assert_eq!(apps.len(), 8);
    assert!(apps[0].descargas_mes.is_some());
    assert!(b.srv.pedidos().contains(
        &"GET /collection/category/game?page=1&per_page=30&sort_by=installs_last_month&locale=es".to_string()
    ));
    assert!(b.cat.por_categoria("game", 2).unwrap().is_empty()); // página sin nada
    assert_eq!(b.cat.por_categoria("GAME", 0).unwrap()[0].id, apps[0].id); // mayúsculas y página < 1
    assert!(b.cat.por_categoria("noexiste", 1).unwrap().is_empty()); // la API responde 422
}

#[test]
fn por_categoria_no_deja_pasar_rutas_raras() {
    let b = base();
    let antes = b.srv.pedidos().len();
    let larga = "x".repeat(80);
    for malo in [
        "../etc/passwd",
        "game/../x",
        "a b",
        "game?x=1",
        "",
        larga.as_str(),
    ] {
        assert!(b.cat.por_categoria(malo, 1).unwrap().is_empty());
    }
    assert_eq!(b.srv.pedidos().len(), antes);
}

#[test]
fn buscar() {
    let b = base();
    let apps = b.cat.buscar("minecraft").unwrap();
    assert_eq!(apps.len(), 8);
    assert!(b.srv.pedidos().contains(
        &r#"POST /search?locale=es {"hits_per_page":30,"page":1,"query":"minecraft"}"#.to_string()
    ));
    assert!(b.cat.buscar("zzzzqq").unwrap().is_empty());
}

#[test]
fn buscar_vacio_no_pide_nada() {
    let b = base();
    for vacio in ["", "   "] {
        assert!(b.cat.buscar(vacio).unwrap().is_empty());
    }
    assert!(b.srv.pedidos().is_empty());
}

#[test]
fn buscar_texto_raro_viaja_como_json() {
    let b = base();
    b.cat.buscar("\"; DROP TABLE apps; -- ñ <script>").unwrap();
    assert!(b.srv.pedidos().iter().any(|p| p.starts_with("POST /search")));
}

#[test]
fn detalle() {
    let b = base();
    let d = b.cat.detalle(PRISM).unwrap();
    assert_eq!((d.id.as_str(), d.nombre.as_str()), (PRISM, "Prism Launcher"));
    assert!(d.verificado);
    assert_eq!(d.licencia, "GPL-3.0-only");
    assert_eq!(d.web.as_deref(), Some("https://prismlauncher.org/"));
    let v = d.version.clone().unwrap();
    assert!(
        v.chars().next().unwrap().is_ascii_digit() && v.contains('.'),
        "{v}"
    );
    let resumen: Value = serde_json::from_str(&leer_datos(&format!("resumen_{PRISM}.json"))).unwrap();
    assert_eq!(d.tamano_descarga, resumen["download_size"].as_i64());
    assert!(d.categorias.contains(&"Juegos".to_string()));
    assert!(!d.capturas.is_empty());
    assert!(d.capturas.iter().all(|c| c.starts_with("https://")));
    let unicas: std::collections::HashSet<_> = d.capturas.iter().collect();
    assert_eq!(unicas.len(), d.capturas.len());
    assert!(!d.descripcion.contains('<'));
    assert!(d.descripcion.contains("• "));
    assert!(
        d.descripcion
            .contains("Features:\n• Easily install game modifications")
    );
    // sin /summary: sigue la ficha
    assert_eq!(
        b.cat.detalle("com.valvesoftware.Steam").unwrap().tamano_descarga,
        None
    );
}

#[test]
fn detalle_de_lo_que_no_existe() {
    let b = base();
    assert!(matches!(
        b.cat.detalle("org.no.Existe"),
        Err(Error::NoEncontrada(_))
    ));
    let antes = b.srv.pedidos().len();
    for malo in [
        "; rm -rf ~",
        "a b.c.d",
        "../x",
        "",
        "org.x.Y\n",
        "http://evil/x.y",
    ] {
        let e = b.cat.detalle(malo).unwrap_err();
        assert!(matches!(e, Error::NoEncontrada(_)), "{malo:?}");
        assert!(e.es_flatpak());
    }
    assert_eq!(b.srv.pedidos().len(), antes);
}

#[test]
fn el_idioma_viaja_como_locale() {
    let b = base();
    let en = Catalogo::nuevo(Some(&b.t.path().join("cache2")), "en", Some(&b.srv.url));
    en.categorias().unwrap();
    assert_eq!(en.idioma, "en");
    assert_eq!(Catalogo::nuevo(None, "de; rm -rf", None).idioma, "es");
    assert_eq!(Catalogo::nuevo(None, "es_MX", None).idioma, "es_MX");
}

#[test]
fn api_por_parametro() {
    let b = base();
    let c = Catalogo::nuevo(
        Some(&b.t.path().join("cache3")),
        "es",
        Some(&format!("{}/", b.srv.url)),
    );
    assert_eq!(c.api, b.srv.url);
    assert_eq!(c.categorias().unwrap().len(), 12);
    assert_eq!(
        Catalogo::nuevo(None, "es", Some("file:///etc/passwd")).api,
        catalogo::API_POR_DEFECTO
    );
}

// ====================================================================== la categoría «IA»

fn ficha_falsa(app_id: &str, nombre: &str) -> Value {
    json!({"id": app_id, "name": nombre, "summary": "AI summary in English", "developer_name": "Alguien",
           "icon": format!("https://dl.flathub.org/media/{}/icon.png", app_id.replace('.', "/")),
           "description": "<p>Run AI models locally.</p>", "project_license": "MIT",
           "categories": ["Utility"], "metadata": {"flathub::verification::verified": "true"},
           "releases": [{"version": "1.0", "timestamp": "1700000000"}], "screenshots": [], "urls": {}})
}

#[test]
fn ia_existe_primero_y_destacada() {
    let b = base();
    let cats = b.cat.categorias().unwrap();
    let ia = &cats[0];
    assert_eq!(
        (ia.id.as_str(), ia.nombre.as_str()),
        (catalogo::CATEGORIA_IA, "IA")
    );
    assert!(ia.destacada);
    assert_eq!(cats.iter().filter(|c| c.id == "ia").count(), 1);
    assert!(!cats[1..].iter().any(|c| c.destacada)); // solo IA sobresale
    assert!(ia.intro.contains("en tu PC"));
    assert_eq!(ia.comando, "vortex-ia modelos");
}

#[test]
fn si_flathub_algun_dia_tiene_ia_no_sale_dos_veces() {
    let b = base();
    b.srv.datos_escribir("categorias.json", &json!(["ia", "game"]));
    let c = b.cat.categorias().unwrap();
    assert_eq!(
        c.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["ia", "game"]
    );
}

#[test]
fn ia_lista_bien_formada() {
    assert!(catalogo::IA.len() >= 6);
    let unicos: std::collections::HashSet<_> = catalogo::IA.iter().map(|a| a.id).collect();
    assert_eq!(unicos.len(), catalogo::IA.len(), "hay apps repetidas en IA");
    for a in &catalogo::IA {
        assert!(es_id_valido(a.id), "{}", a.id);
        assert!(
            (20..=110).contains(&a.resumen.chars().count()),
            "resumen corto y claro: {}",
            a.id
        );
        assert!(!a.grafica.trim().is_empty(), "falta la gráfica: {}", a.id);
        for t in [a.resumen, a.grafica, a.nota] {
            assert_eq!(t, t.trim());
            assert!(t.is_empty() || t.ends_with('.'), "termina en punto: {t:?}");
        }
    }
    for esperado in [
        "com.jeffser.Alpaca",
        "ai.jan.Jan",
        "ai.lmstudio.lm-studio",
        "io.gpt4all.gpt4all",
        "net.mkiol.SpeechNote",
        "org.upscayl.Upscayl",
    ] {
        assert!(unicos.contains(esperado), "{esperado}");
    }
    assert!(!catalogo::IA.iter().any(|a| RECOMENDADAS.contains(&a.id)));
    let alpaca = catalogo::IA
        .iter()
        .find(|a| a.id == "com.jeffser.Alpaca")
        .unwrap();
    assert!(alpaca.resumen.contains("Ollama"));
    assert!(alpaca.nota.contains("localhost:11434"));
}

#[test]
fn por_categoria_ia_con_datos_de_flathub_y_textos_propios() {
    let b = base();
    b.srv.datos_escribir(
        "appstream_com.jeffser.Alpaca.json",
        &ficha_falsa("com.jeffser.Alpaca", "Alpaca"),
    );
    b.srv.datos_escribir(
        "appstream_org.upscayl.Upscayl.json",
        &ficha_falsa("org.upscayl.Upscayl", "Upscayl"),
    );
    let apps = b.cat.por_categoria("ia", 1).unwrap();
    // Las que el Flathub falso no tiene se saltan; las que sí, en el orden de la lista.
    assert_eq!(ids(&apps), ["com.jeffser.Alpaca", "org.upscayl.Upscayl"]);
    let alpaca = &apps[0];
    assert_eq!(alpaca.nombre, "Alpaca");
    assert!(alpaca.icono_url.starts_with("https://")); // el ícono es el de Flathub
    assert!(alpaca.verificado);
    assert_eq!(alpaca.resumen, catalogo::IA[0].resumen); // en español, el de Vortexine's
    assert!(!alpaca.grafica.is_empty() && !alpaca.nota.is_empty());
    assert_eq!(apps[1].nota, "");
    assert!(
        !b.srv
            .pedidos()
            .iter()
            .any(|p| p.starts_with("GET /collection/category/ia"))
    ); // no se pide a Flathub
    assert!(b.cat.por_categoria("ia", 2).unwrap().is_empty()); // una sola página
    assert_eq!(ids(&b.cat.por_categoria("IA", 1).unwrap()), ids(&apps));
}

#[test]
fn detalle_de_una_app_de_ia() {
    let b = base();
    b.srv.datos_escribir(
        "appstream_com.jeffser.Alpaca.json",
        &ficha_falsa("com.jeffser.Alpaca", "Alpaca"),
    );
    let d = b.cat.detalle("com.jeffser.Alpaca").unwrap();
    assert_eq!(d.resumen, catalogo::IA[0].resumen);
    assert_eq!(d.grafica, catalogo::IA[0].grafica);
    assert!(d.descripcion.contains("Run AI models locally."));
    assert_eq!(b.cat.detalle(PRISM).unwrap().grafica, ""); // las demás no cambian
}

#[test]
fn ia_sin_red() {
    let b = base();
    let cat = Catalogo::nuevo(
        Some(&b.t.path().join("otra")),
        "es",
        Some(&format!("http://127.0.0.1:{}", puerto_cerrado())),
    );
    assert!(cat.por_categoria("ia", 1).unwrap_err().es_sin_red());
}

// ====================================================================== caché

fn con_reloj(b: &BaseCatalogo, nombre: &str) -> (Catalogo, Arc<Mutex<f64>>) {
    let reloj = Arc::new(Mutex::new(1_800_000_000.0));
    let r2 = reloj.clone();
    let c = Catalogo::nuevo(Some(&b.t.path().join(nombre)), "es", Some(&b.srv.url))
        .con_reloj(move || *r2.lock().unwrap());
    (c, reloj)
}

#[test]
fn segunda_vez_sale_de_la_cache() {
    let b = base();
    b.cat.categorias().unwrap();
    b.cat.buscar("minecraft").unwrap();
    let n = b.srv.pedidos().len();
    let otra = Catalogo::nuevo(Some(&b.t.path().join("cache")), "es", Some(&b.srv.url)); // otro objeto, misma carpeta
    assert_eq!(otra.categorias().unwrap().len(), 12);
    assert_eq!(otra.buscar("minecraft").unwrap().len(), 8);
    assert_eq!(b.srv.pedidos().len(), n);
    let archivos: Vec<_> = std::fs::read_dir(b.t.path().join("cache"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(!archivos.is_empty());
    assert!(archivos.iter().all(|f| f.to_string_lossy().ends_with(".json")));
}

#[test]
fn las_listas_vencen_a_la_hora_y_las_fichas_a_las_24() {
    let b = base();
    let (c, reloj) = con_reloj(&b, "c1");
    c.por_categoria("game", 1).unwrap();
    c.detalle(PRISM).unwrap();
    let n = b.srv.pedidos().len();
    *reloj.lock().unwrap() += 3500.0; // < 1 h: todo de la caché
    c.por_categoria("game", 1).unwrap();
    c.detalle(PRISM).unwrap();
    assert_eq!(b.srv.pedidos().len(), n);
    *reloj.lock().unwrap() += 200.0; // > 1 h: la lista se vuelve a pedir
    c.por_categoria("game", 1).unwrap();
    assert_eq!(b.srv.pedidos().len(), n + 1);
    c.detalle(PRISM).unwrap(); // la ficha aún no vence
    assert_eq!(b.srv.pedidos().len(), n + 1);
    *reloj.lock().unwrap() += 24.0 * 3600.0; // > 24 h: la ficha también
    c.detalle(PRISM).unwrap();
    assert_eq!(b.srv.pedidos().len(), n + 3); // ficha + tamaño
}

#[test]
fn con_el_servidor_caido_usa_la_cache_vieja() {
    let b = base();
    let (c, reloj) = con_reloj(&b, "c2");
    let antes = c.buscar("minecraft").unwrap();
    c.detalle(PRISM).unwrap();
    *reloj.lock().unwrap() += 10.0 * 24.0 * 3600.0; // todo vencido
    b.srv.modo("500");
    assert_eq!(ids(&c.buscar("minecraft").unwrap()), ids(&antes));
    assert_eq!(c.detalle(PRISM).unwrap().nombre, "Prism Launcher");
}

#[test]
fn con_la_red_caida_usa_la_cache_vieja() {
    let b = base();
    let (c, reloj) = con_reloj(&b, "c3");
    c.categorias().unwrap();
    *reloj.lock().unwrap() += 5.0 * 24.0 * 3600.0;
    b.srv.parar(); // conexión rechazada
    assert_eq!(c.categorias().unwrap().len(), 12);
}

#[test]
fn sin_red_y_sin_cache_es_sin_red() {
    let b = base();
    let cache = b.t.path().join("vacia");
    let c = Catalogo::nuevo(
        Some(&cache),
        "es",
        Some(&format!("http://127.0.0.1:{}", puerto_cerrado())),
    );
    let mensajes = [
        c.destacados().unwrap_err(),
        c.recomendadas().unwrap_err(),
        c.categorias().unwrap_err(),
        c.por_categoria("game", 1).unwrap_err(),
        c.buscar("minecraft").unwrap_err(),
        c.detalle(PRISM).unwrap_err(),
    ];
    for e in mensajes {
        assert!(e.es_sin_red(), "{e:?}");
        assert!(e.to_string().contains("conexión"));
    }
    assert!(!cache.exists() || std::fs::read_dir(&cache).unwrap().next().is_none());
}

#[test]
fn error_del_servidor_sin_cache_es_sin_red() {
    let b = base();
    b.srv.modo("500");
    assert!(b.cat.buscar("minecraft").unwrap_err().es_sin_red());
}

#[test]
fn timeout() {
    let b = base();
    b.srv.modo("lento");
    let mut c = Catalogo::nuevo(Some(&b.t.path().join("c4")), "es", Some(&b.srv.url));
    c.espera = 0.4;
    let inicio = Instant::now();
    assert!(c.categorias().unwrap_err().es_sin_red());
    assert!(inicio.elapsed() < Duration::from_millis(2500));
}

#[test]
fn cache_danada_se_ignora() {
    let b = base();
    b.cat.categorias().unwrap();
    let dir = b.t.path().join("cache");
    for f in std::fs::read_dir(&dir).unwrap() {
        std::fs::write(f.unwrap().path(), "{esto no es json").unwrap();
    }
    assert_eq!(b.cat.categorias().unwrap().len(), 12);
    let esperado: Value = serde_json::from_str(&leer_datos("categorias.json")).unwrap();
    for f in std::fs::read_dir(&dir).unwrap() {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(f.unwrap().path()).unwrap()).unwrap();
        assert_eq!(v["datos"], esperado);
    }
}

#[test]
fn respuesta_que_no_es_json_cuenta_como_fallo() {
    let b = base();
    b.srv
        .datos_escribir_crudo("categorias.json", "<html>no soy json</html>");
    assert!(b.cat.categorias().unwrap_err().es_sin_red());
}

#[test]
fn cache_en_una_carpeta_que_no_se_puede_escribir() {
    let b = base();
    let c = Catalogo::nuevo(
        Some(std::path::Path::new("/proc/no-se-puede-escribir/aqui")),
        "es",
        Some(&b.srv.url),
    );
    assert_eq!(c.categorias().unwrap().len(), 12); // funciona igual, solo sin caché
}

#[test]
fn la_fecha_de_hoy() {
    assert_eq!(catalogo::fecha_utc(0.0), "1970-01-01");
    assert_eq!(catalogo::fecha_utc(1_800_000_000.0), "2027-01-15");
    assert_eq!(catalogo::fecha_utc(1_709_164_800.0), "2024-02-29");
}

// ====================================================================== texto

#[test]
fn texto_parrafos_y_vinetas() {
    let html = "<p>Hola <em>mundo</em> &amp; todos.</p>\n   <p>Funciones:</p><ul><li>Una</li>\n<li>Dos   cosas</li></ul>\n<p>Fin.</p>";
    assert_eq!(
        html_a_texto(html),
        "Hola mundo & todos.\n\nFunciones:\n• Una\n• Dos cosas\n\nFin."
    );
}

#[test]
fn texto_lista_ordenada_y_anidada() {
    assert_eq!(
        html_a_texto("<ol><li>Uno</li><li>Dos</li></ol><ul><li>X</li></ul>"),
        "1. Uno\n2. Dos\n\n• X"
    );
}

#[test]
fn texto_salto_de_linea_y_entidades() {
    assert_eq!(
        html_a_texto("<p>a<br/>b &lt;c&gt; &quot;d&quot; &#233;</p>"),
        "a\nb <c> \"d\" é"
    );
}

#[test]
fn texto_plano_pasa_casi_igual() {
    assert_eq!(
        html_a_texto("  Linea uno\n      linea dos\n\n\n\n   otra  "),
        "Linea uno\nlinea dos\n\notra"
    );
}

#[test]
fn texto_vacios() {
    for v in ["", "   ", "<p></p>", "<ul></ul>"] {
        assert_eq!(html_a_texto(v), "", "{v:?}");
    }
}

#[test]
fn texto_no_hay_etiquetas_ni_scripts_sueltos() {
    let t = html_a_texto(
        "<p>Hola</p><script>alert(1)</script><img src=x onerror=y><a href='http://x'>enlace</a>",
    );
    assert!(!t.contains('<'));
    assert!(t.contains("Hola"));
    assert!(t.contains("enlace"));
}

// ====================================================================== ids

#[test]
fn ids_validos() {
    for i in [
        "org.mozilla.firefox",
        "com.valvesoftware.Steam",
        "net.davidotek.pupgui2",
        "sh.ppy.osu",
        "io.github.ilya-zlobintsev.LACT",
        "org.pegasus_frontend.Pegasus",
        "a.b",
        "com.0x.App1",
    ] {
        assert!(es_id_valido(i), "{i}");
        assert_eq!(validar_id(i), Ok(i));
    }
}

#[test]
fn ids_raros() {
    let larga = format!("{}.b", "a".repeat(300));
    for i in [
        "; rm -rf ~",
        "org.x.y; rm -rf ~",
        "a b.c",
        "$(touch x).y.z",
        "`id`.a.b",
        "../../x.y",
        "a/b.c",
        "-rf.a.b",
        "--help",
        "org..x",
        ".org.x",
        "org.x.",
        "org.x\n",
        "org.x y",
        "sin_puntos",
        "",
        " ",
        larga.as_str(),
        "org.x|y",
        "org.x&y",
        "org.x'y",
        "org.x\"y",
        "org.x\\y",
        "ñ.ñ",
    ] {
        assert!(!es_id_valido(i), "{i:?}");
        assert!(validar_id(i).is_err());
    }
}

// ====================================================================== flatpak (CLI falsa)

#[test]
fn instaladas_junta_usuario_y_sistema() {
    let b = BaseFlatpak::nueva();
    b.escribir(
        "usuario.txt",
        &["org.mozilla.firefox", "io.qt.qtwebengine.BaseApp"],
    );
    b.escribir(
        "sistema.txt",
        &[
            "org.vinegarhq.Sober",
            "org.mozilla.firefox",
            "Application ID",
            "; rm -rf ~",
        ],
    );
    assert_eq!(
        b.flatpak().instaladas().unwrap(),
        ["org.mozilla.firefox", "org.vinegarhq.Sober"]
    );
}

#[test]
fn es_del_sistema() {
    let b = BaseFlatpak::nueva();
    b.escribir("usuario.txt", &["org.mozilla.firefox"]);
    b.escribir("sistema.txt", &["org.vinegarhq.Sober", "org.mozilla.firefox"]);
    let f = b.flatpak();
    assert!(f.es_del_sistema("org.vinegarhq.Sober").unwrap());
    assert!(!f.es_del_sistema("org.mozilla.firefox").unwrap());
    assert!(!f.es_del_sistema("com.spotify.Client").unwrap());
}

#[test]
fn preparar_agrega_flathub_al_usuario_una_vez() {
    let b = BaseFlatpak::nueva();
    let f = b.flatpak();
    assert!(f.preparar().unwrap());
    assert_eq!(b.leer("remotos.txt"), ["flathub"]);
    assert!(!f.preparar().unwrap());
    let agregados: Vec<_> = b
        .llamadas()
        .into_iter()
        .filter(|l| l.starts_with("remote-add"))
        .collect();
    assert_eq!(
        agregados,
        ["remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo"]
    );
}

#[test]
fn instalar_con_progreso() {
    let b = BaseFlatpak::nueva();
    let (eventos, cb) = avance();
    b.flatpak().instalar("com.spotify.Client", Some(&cb)).unwrap();
    assert!(b.leer("usuario.txt").contains(&"com.spotify.Client".to_string()));
    let ev = eventos.lock().unwrap().clone();
    assert_eq!(ev.last().unwrap(), &(100, "Listo".to_string()));
    let porcentajes: Vec<i32> = ev.iter().map(|e| e.0).collect();
    let mut ordenados = porcentajes.clone();
    ordenados.sort_unstable();
    assert_eq!(porcentajes, ordenados, "el progreso nunca retrocede");
    assert!(porcentajes.iter().all(|p| (0..=100).contains(p)));
    let descargas: Vec<_> = ev.iter().filter(|e| e.1.starts_with("Descargando…")).collect();
    assert!(descargas.len() >= 4, "{ev:?}");
    for (_, t) in &descargas {
        let n: i32 = t.split_whitespace().nth(1).unwrap().parse().unwrap();
        assert_eq!(t, &format!("Descargando… {n} %"));
    }
    assert!(ev.iter().any(|e| e.1 == "Instalando…"));
    assert!(ev.iter().any(|e| e.0 > 0 && e.0 < 100));
    assert!(
        b.llamadas()
            .contains(&"install --user --noninteractive -y flathub com.spotify.Client".to_string())
    );
    assert_eq!(b.leer("remotos.txt"), ["flathub"]); // preparó Flathub antes
    assert!(!b.llamadas().iter().any(|l| l.contains("--system")));
}

#[test]
fn instalar_algo_que_ya_esta_no_repite() {
    let b = BaseFlatpak::nueva();
    b.escribir("usuario.txt", &["com.spotify.Client"]);
    b.escribir("sistema.txt", &["org.vinegarhq.Sober"]);
    for id in ["com.spotify.Client", "org.vinegarhq.Sober"] {
        let (eventos, cb) = avance();
        b.flatpak().instalar(id, Some(&cb)).unwrap();
        assert_eq!(
            *eventos.lock().unwrap(),
            [(100, "Ya estaba instalada".to_string())]
        );
    }
    assert!(!b.llamadas().iter().any(|l| l.starts_with("install")));
}

#[test]
fn instalar_sin_callback() {
    let b = BaseFlatpak::nueva();
    let f = b.flatpak();
    f.instalar("com.spotify.Client", None).unwrap();
    f.instalar("org.videolan.VLC", None).unwrap();
    assert_eq!(b.leer("usuario.txt"), ["com.spotify.Client", "org.videolan.VLC"]);
}

#[test]
fn un_callback_que_falla_no_rompe_la_instalacion() {
    let b = BaseFlatpak::nueva();
    let malo = |_: i32, _: &str| panic!("la interfaz se rompió");
    b.flatpak().instalar("com.spotify.Client", Some(&malo)).unwrap();
    assert!(b.leer("usuario.txt").contains(&"com.spotify.Client".to_string()));
}

#[test]
fn quitar() {
    let b = BaseFlatpak::nueva();
    b.escribir("usuario.txt", &["com.spotify.Client", "org.videolan.VLC"]);
    let (eventos, cb) = avance();
    b.flatpak().quitar("com.spotify.Client", Some(&cb)).unwrap();
    assert_eq!(b.leer("usuario.txt"), ["org.videolan.VLC"]);
    let ev = eventos.lock().unwrap().clone();
    assert_eq!(ev[0], (0, "Quitando…".to_string()));
    assert_eq!(ev.last().unwrap(), &(100, "Listo".to_string()));
    assert!(
        b.llamadas()
            .contains(&"uninstall --user --noninteractive -y com.spotify.Client".to_string())
    );
}

#[test]
fn quitar_una_del_sistema_lo_dice_claro() {
    let b = BaseFlatpak::nueva();
    b.escribir("sistema.txt", &["org.vinegarhq.Sober"]);
    let e = b
        .flatpak()
        .quitar("org.vinegarhq.Sober", None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("parte del sistema"), "{e}");
    assert!(e.contains("no se puede quitar"));
    assert!(!b.llamadas().iter().any(|l| l.starts_with("uninstall")));
}

#[test]
fn quitar_una_que_no_esta() {
    let b = BaseFlatpak::nueva();
    let e = b
        .flatpak()
        .quitar("com.spotify.Client", None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("no está instalada"));
}

#[test]
fn actualizaciones_y_actualizar_todas() {
    let b = BaseFlatpak::nueva();
    b.escribir("actualizaciones.txt", &["org.videolan.VLC", "com.spotify.Client"]);
    let f = b.flatpak();
    assert_eq!(
        f.actualizaciones().unwrap(),
        ["com.spotify.Client", "org.videolan.VLC"]
    );
    let (eventos, cb) = avance();
    f.actualizar(None, Some(&cb)).unwrap();
    assert!(b.leer("actualizaciones.txt").is_empty());
    let ev = eventos.lock().unwrap().clone();
    assert_eq!(ev.last().unwrap(), &(100, "Listo".to_string()));
    let p: Vec<i32> = ev.iter().map(|e| e.0).collect();
    let mut o = p.clone();
    o.sort_unstable();
    assert_eq!(p, o);
    assert!(ev.iter().any(|e| e.1.starts_with("Descargando…")));
    assert!(
        b.llamadas()
            .contains(&"update --user --noninteractive -y".to_string())
    );
}

#[test]
fn actualizar_solo_unas() {
    let b = BaseFlatpak::nueva();
    b.escribir("actualizaciones.txt", &["org.videolan.VLC", "com.spotify.Client"]);
    // firefox no tiene nada pendiente
    b.flatpak()
        .actualizar(
            Some(&["com.spotify.Client".into(), "org.mozilla.firefox".into()]),
            None,
        )
        .unwrap();
    assert_eq!(b.leer("actualizaciones.txt"), ["org.videolan.VLC"]);
    assert!(
        b.llamadas()
            .contains(&"update --user --noninteractive -y com.spotify.Client".to_string())
    );
}

#[test]
fn actualizar_sin_nada_pendiente() {
    let b = BaseFlatpak::nueva();
    let (eventos, cb) = avance();
    b.flatpak()
        .actualizar(Some(&["org.videolan.VLC".into()]), Some(&cb))
        .unwrap();
    assert_eq!(
        eventos.lock().unwrap().last().unwrap(),
        &(100, "Todo está al día".to_string())
    );
    assert!(!b.llamadas().iter().any(|l| l.starts_with("update")));
}

// -- errores en español

#[test]
fn sin_internet() {
    let b = BaseFlatpak::nueva();
    let f = vortex_shop::flatpak::Flatpak::nuevo(b.entorno(Some("red")));
    for e in [
        f.instalar("com.spotify.Client", None).unwrap_err(),
        f.preparar().unwrap_err(),
    ] {
        assert!(matches!(e, Error::FlatpakSinRed(_)));
        assert!(e.es_sin_red() && e.es_flatpak());
        assert!(e.to_string().contains("conexión a internet"));
    }
}

#[test]
fn sin_espacio() {
    let b = BaseFlatpak::nueva();
    b.escribir("remotos.txt", &["flathub"]);
    let f = vortex_shop::flatpak::Flatpak::nuevo(b.entorno(Some("espacio")));
    let e = f.instalar("com.spotify.Client", None).unwrap_err().to_string();
    assert!(e.contains("No hay espacio suficiente"), "{e}");
    assert!(e.contains("«com.spotify.Client»"));
}

#[test]
fn la_app_no_existe_en_flathub() {
    let b = BaseFlatpak::nueva();
    b.escribir("remotos.txt", &["flathub"]);
    let f = vortex_shop::flatpak::Flatpak::nuevo(b.entorno(Some("noexiste")));
    let e = f.instalar("org.no.Existe", None).unwrap_err();
    assert!(matches!(e, Error::NoEncontrada(_)));
    assert!(e.to_string().contains("Flathub no tiene"));
}

#[test]
fn error_desconocido_trae_el_motivo() {
    let b = BaseFlatpak::nueva();
    b.escribir("remotos.txt", &["flathub"]);
    let f = vortex_shop::flatpak::Flatpak::nuevo(b.entorno(Some("sistema")));
    let e = f.instalar("com.spotify.Client", None).unwrap_err().to_string();
    assert!(e.starts_with("No hay permiso para instalar"), "{e}");
}

#[test]
fn sin_flatpak_instalado() {
    let b = BaseFlatpak::nueva();
    let f = vortex_shop::flatpak::Flatpak::nuevo(Entorno::default().con("PATH", "/nonexistent-vortex"));
    let errores = [
        f.instaladas().unwrap_err(),
        f.preparar().unwrap_err(),
        f.instalar("a.b.c", None).unwrap_err(),
        f.abrir("a.b.c").unwrap_err(),
    ];
    for e in errores {
        assert!(e.to_string().contains("Flatpak no está instalado"), "{e}");
    }
    drop(b);
}

// -- ids raros: se rechazan antes de llamar a nada

#[test]
fn ids_raros_se_rechazan_sin_tocar_flatpak() {
    let b = BaseFlatpak::nueva();
    let marca = b.t.path().join("pwned");
    let m = marca.display();
    let raros = [
        "; rm -rf ~".to_string(),
        format!("com.x.y; touch {m}"),
        format!("$(touch {m}).a.b"),
        "a b.c.d".into(),
        format!("`touch {m}`.a.b"),
        "--system.a.b".into(),
        "../../x.y".into(),
        "org.x\n".into(),
        String::new(),
        "org.x|y".into(),
    ];
    let f = b.flatpak();
    let antes = b.llamadas();
    for malo in &raros {
        assert!(f.instalar(malo, None).is_err());
        assert!(f.quitar(malo, None).is_err());
        assert!(f.abrir(malo).is_err());
        assert!(f.es_del_sistema(malo).is_err());
        assert!(f.actualizar(Some(&[malo.clone()]), None).is_err());
    }
    assert_eq!(b.llamadas(), antes);
    assert!(!marca.exists());
}

// -- abrir

fn esperar_marca(p: &std::path::Path) -> bool {
    for _ in 0..100 {
        if p.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    p.exists()
}

#[test]
fn abrir_desacoplada() {
    let b = BaseFlatpak::nueva();
    b.escribir("usuario.txt", &["org.videolan.VLC"]);
    let inicio = Instant::now();
    b.flatpak().abrir("org.videolan.VLC").unwrap();
    assert!(
        inicio.elapsed() < Duration::from_millis(600),
        "abrir() no espera a que la app termine"
    );
    assert!(
        esperar_marca(&b.estado.join("abierta-org.videolan.VLC")),
        "la app abierta siguió corriendo sola"
    );
}

#[test]
fn abrir_una_que_no_esta_instalada() {
    let b = BaseFlatpak::nueva();
    let e = b.flatpak().abrir("org.videolan.VLC").unwrap_err().to_string();
    assert!(e.contains("no está instalada"));
    assert!(!b.llamadas().iter().any(|l| l.starts_with("run")));
}

#[test]
fn abrir_una_del_sistema() {
    let b = BaseFlatpak::nueva();
    b.escribir("sistema.txt", &["org.vinegarhq.Sober"]);
    b.flatpak().abrir("org.vinegarhq.Sober").unwrap();
    assert!(esperar_marca(&b.estado.join("abierta-org.vinegarhq.Sober")));
}

// -- una operación a la vez

#[test]
fn dos_operaciones_hacen_fila() {
    let b = BaseFlatpak::nueva();
    b.escribir("remotos.txt", &["flathub"]);
    let f = Arc::new(b.flatpak());
    let (eventos_b, cb_b) = avance();
    let f2 = f.clone();
    let a = std::thread::spawn(move || f2.instalar("com.spotify.Client", None).unwrap());
    std::thread::sleep(Duration::from_millis(120));
    f.instalar("org.videolan.VLC", Some(&cb_b)).unwrap();
    a.join().unwrap();
    let ev = eventos_b.lock().unwrap().clone();
    assert_eq!(ev[0], (0, "Esperando a que termine otra operación…".to_string()));
    assert_eq!(ev.last().unwrap(), &(100, "Listo".to_string()));
    let mut u = b.leer("usuario.txt");
    u.sort();
    assert_eq!(u, ["com.spotify.Client", "org.videolan.VLC"]);
}

// ====================================================================== apps propias (Camera, Runner)

/// Los %XX de un URL file:// otra vez en bytes (como `urllib.parse.unquote`).
fn deshacer_url(t: &str) -> String {
    let b = t.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 {
            if let Ok(x) = u8::from_str_radix(&t[i + 1..i + 3], 16) {
                v.push(x);
                i += 3;
                continue;
            }
        }
        v.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

struct BasePropia {
    t: tempfile::TempDir,
    propias: Propias,
}

fn base_propia() -> BasePropia {
    let t = tmp();
    let entorno = entorno_vortex_falso(t.path());
    let qml = aqui().parent().unwrap().join("qt/qml");
    BasePropia {
        propias: Propias::nuevo(qml, entorno),
        t,
    }
}

#[test]
fn propias_instalar_y_quitar() {
    let b = base_propia();
    let lista = b.propias.lista();
    assert_eq!(
        lista
            .iter()
            .map(|a| a["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["vortex-camera", "vortex-runner"]
    );
    assert_eq!(lista[0]["instalada"], false);
    assert!(lista[0]["icono"].as_str().unwrap().starts_with("file://"));
    for a in &lista {
        let ruta = deshacer_url(a["icono"].as_str().unwrap().strip_prefix("file://").unwrap());
        assert!(std::path::Path::new(&ruta).is_file(), "{}", a["id"]);
    }
    b.propias.instalar("vortex-camera").unwrap();
    assert!(b.propias.instalada("vortex-camera").unwrap());
    b.propias.quitar("vortex-camera").unwrap();
    assert!(!b.propias.instalada("vortex-camera").unwrap());
    let llamadas = std::fs::read_to_string(b.t.path().join("llamadas")).unwrap();
    assert_eq!(
        llamadas.lines().take(2).collect::<Vec<_>>(),
        ["camara instalar", "camara quitar"]
    );
    assert_eq!(APPS.len(), 2);
}

#[test]
fn propias_error_entendible() {
    let b = base_propia();
    std::fs::write(b.t.path().join("fallar"), "").unwrap();
    let e = b.propias.instalar("vortex-camera").unwrap_err();
    assert_eq!(
        e.to_string(),
        "No se pudo instalar Vortexine's Camera: le falta: ffmpeg"
    );
    assert!(b.propias.instalar("otra-cosa").is_err());
}

// ====================================================================== Android (vortex-android falso)

struct BaseAndroid {
    t: tempfile::TempDir,
    android: Android,
}

fn base_android(nvidia: bool) -> BaseAndroid {
    let t = tmp();
    let entorno = entorno_android_falso(t.path(), nvidia);
    BaseAndroid {
        android: Android::nuevo(entorno),
        t,
    }
}

#[test]
fn android_sin_preparar_y_preparar() {
    let b = base_android(true);
    let e = b.android.estado();
    assert!(!e.preparado && e.nvidia && !e.registrado);
    assert!(b.android.apps().is_empty());
    let (eventos, cb) = avance();
    b.android.preparar(&cb).unwrap();
    let ev = eventos.lock().unwrap().clone();
    assert_eq!(ev[0], (0, "Paso número 1".to_string()));
    assert_eq!(ev[4], (80, "Paso número 5".to_string()));
    assert_eq!(ev.last().unwrap(), &(100, "Listo".to_string()));
    assert!(b.android.estado().preparado);
    let apps = b.android.apps();
    assert_eq!(apps.len(), 1);
    assert_eq!(
        (apps[0].paquete.as_str(), apps[0].nombre.as_str()),
        ("com.kiloo.subwaysurf", "Subway Surfers")
    );
}

#[test]
fn android_id_registro_y_tienda() {
    let b = base_android(false);
    assert_eq!(b.android.id_google().unwrap(), "3912345678901234567");
    b.android.registrado().unwrap();
    assert!(b.android.estado().registrado);
    b.android.tienda("com.supercell.clashroyale").unwrap();
    b.android.tienda("").unwrap();
    b.android.buscar("  minecraft ").unwrap();
    b.android.abrir("com.kiloo.subwaysurf").unwrap();
    b.android.quitar("com.kiloo.subwaysurf").unwrap();
    b.android.parar().unwrap();
    let llamadas = std::fs::read_to_string(b.t.path().join("estado/llamadas")).unwrap();
    for esperada in [
        "tienda com.supercell.clashroyale",
        "buscar minecraft",
        "abrir com.kiloo.subwaysurf",
        "quitar com.kiloo.subwaysurf",
        "parar",
    ] {
        assert!(llamadas.lines().any(|l| l == esperada), "{esperada}");
    }
}

#[test]
fn android_paquetes_raros_no_llegan_al_comando() {
    let b = base_android(false);
    for malo in ["com.x; rm -rf ~", "a b", "$(x)", "ñ"] {
        assert!(b.android.abrir(malo).is_err(), "{malo}");
        assert!(b.android.quitar(malo).is_err());
        assert!(b.android.tienda(malo).is_err());
    }
    assert!(!b.t.path().join("estado/llamadas").exists());
}

#[test]
fn android_errores_en_espanol() {
    let b = base_android(false);
    let e = b.android.tienda("com.x").map(|_| ());
    assert!(e.is_ok());
    // el falso no conoce «raro»: el comando falla y el mensaje sale limpio
    let e = Android::nuevo(
        Entorno::default()
            .con(
                "VORTEX_SHOP_ANDROID",
                b.t.path().join("vortex-android").as_os_str(),
            )
            .con("ANDROID_FALSO_DIR", b.t.path().join("estado").as_os_str()),
    );
    let _ = e;
    let sin = Android::nuevo(Entorno::default().con("VORTEX_SHOP_ANDROID", "/no/existe/vortex-android"));
    let falta = sin.parar().unwrap_err().to_string();
    assert_eq!(
        falta,
        "Falta el comando vortex-android: corre «vortex actualizar»."
    );
    assert!(!sin.estado().preparado);
    assert!(sin.apps().is_empty());
}

// ====================================================================== una sola ventana

#[test]
fn una_sola_ventana_la_segunda_pide_mostrar() {
    use vortex_shop::instancia::{Escucha, avisar_a_la_abierta, nombre};
    let t = tmp();
    let ruta = t.path().join(nombre());
    assert!(!avisar_a_la_abierta(&ruta), "sin ventana abierta, nadie contesta");
    let escucha = Escucha::nueva(&ruta).unwrap();
    assert_eq!(escucha.revisar(), 0);
    assert!(avisar_a_la_abierta(&ruta));
    assert!(avisar_a_la_abierta(&ruta));
    assert!(esperar_marca(&ruta));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(escucha.revisar(), 2);
    assert_eq!(escucha.revisar(), 0);
    drop(escucha);
    assert!(!ruta.exists(), "al cerrar se borra el socket");
    assert!(nombre().starts_with("vortexines-shop-"));
}
