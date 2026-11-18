//! Flathub para .deb/.rpm, Flatpak, mods de Minecraft y los textos en los dos idiomas (`test_motor.py`).
mod comun;

use comun::*;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use vortex_app_installer::flathub::{self, App};
use vortex_app_installer::motor;
use vortex_app_installer::textos::{self, Idioma};

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn app(id: &str, nombre: &str) -> App {
    App::nuevo(id, nombre)
}

#[test]
fn flathub_empareja_bien() {
    let hits = vec![
        app("com.google.Chrome", "Google Chrome"),
        app("com.visualstudio.code", "Visual Studio Code"),
        app("com.discordapp.Discord", "Discord"),
    ];
    let id = |n: &str, h: &[App]| flathub::mejor(n, h).map(|a| a.id.clone());
    assert_eq!(
        id("google-chrome-stable", &hits).as_deref(),
        Some("com.google.Chrome")
    );
    assert_eq!(id("code", &hits).as_deref(), Some("com.visualstudio.code"));
    assert_eq!(id("Discord", &hits).as_deref(), Some("com.discordapp.Discord"));
    assert_eq!(id("libreoffice", &hits), None);
    let mut dev = vec![app("com.google.ChromeDev", "Google Chrome (unstable)")];
    dev.extend(hits.iter().map(|a| app(&a.id, &a.nombre)));
    assert_eq!(
        id("google-chrome-stable", &dev).as_deref(),
        Some("com.google.Chrome")
    );
    assert_eq!(
        id("google-chrome-unstable", &dev).as_deref(),
        Some("com.google.ChromeDev")
    );
    assert_eq!(id("google-chrome-stable", &dev[..1]), None);
    assert_eq!(flathub::para_buscar("google-chrome-stable"), "google chrome");
    assert_eq!(flathub::para_buscar("obs-studio"), "obs studio");
}

/// Un `curl` falso que contesta como Flathub.
fn curl_de_flathub(c: &Casa) {
    let cuerpo = json!({"hits": [{"app_id": "com.google.Chrome", "name": "Google Chrome", "summary": "El navegador"}]});
    escribir_script(
        &c.bins.join("curl"),
        &format!("#!/bin/sh\ncat >/dev/null\ncat <<'FIN'\n{cuerpo}\nFIN\n"),
    );
}

#[test]
fn deb_ofrece_flathub() {
    let c = Casa::nueva();
    curl_de_flathub(&c);
    let info = c.analizar(&deb(
        &c.descargas.join("google-chrome-stable_current_amd64.deb"),
        "google-chrome-stable",
    ));
    let vista = motor::para_mostrar(&c.ctx, &info);
    assert_eq!(vista["flathub"]["id"], "com.google.Chrome");
    assert_eq!(vista["acciones"][0]["id"], "flathub");
    let r = c
        .ejecutar(&info, "flathub", vista["acciones"][0]["datos"].clone())
        .unwrap();
    let log = c.flatpak_log();
    assert!(log.contains("remote-add --user --if-not-exists flathub"), "{log}");
    assert!(
        log.contains("install --user -y --noninteractive flathub com.google.Chrome"),
        "{log}"
    );
    assert_eq!(r["flatpak"], "com.google.Chrome");
    let tipos: Vec<String> = c.registro().iter().map(|e| texto(&json!(e), "tipo")).collect();
    assert_eq!(tipos, ["flatpak"]);
    motor::deshacer(&c.ctx, "flatpak-com.google.Chrome").unwrap();
    assert!(
        c.flatpak_log()
            .contains("uninstall --user -y --noninteractive com.google.Chrome")
    );
    assert!(c.registro().is_empty());
}

#[test]
fn deb_sin_flathub_saca_el_contenido() {
    let c = Casa::nueva();
    let info = c.analizar(&deb(&c.descargas.join("raro_1.0_amd64.deb"), "paquete-raro"));
    let vista = motor::para_mostrar(&c.ctx, &info);
    assert!(vista["flathub"].is_null() || vista["flathub"] == json!(false) || vista["flathub"] == json!({}));
    assert_eq!(vista["acciones"][0]["id"], "ver_contenido");
    let r = c.ejecutar(&info, "ver_contenido", json!({})).unwrap();
    let leeme = Path::new(&texto(&r, "carpeta")).join("usr/share/doc/leeme.txt");
    assert_eq!(std::fs::read_to_string(leeme).unwrap(), "hola");
}

#[test]
fn flatpakref() {
    let c = Casa::nueva();
    let r = c.descargas.join("org.example.App.flatpakref");
    std::fs::write(&r, "[Flatpak Ref]\nName=org.example.App\nTitle=Ejemplo\n").unwrap();
    let res = c
        .ejecutar(&c.analizar(&r), "instalar_flatpak", json!({}))
        .unwrap();
    let esperado = format!("install --user -y --noninteractive --from {}", r.display());
    assert!(c.flatpak_log().contains(&esperado), "{}", c.flatpak_log());
    assert_eq!(res["flatpak"], "org.example.App");
    assert_eq!(texto(&json!(c.registro()[0]), "nombre"), "Ejemplo");
}

fn vortex_falso(c: &Casa, instancias: &[Value]) {
    let lineas: Vec<String> = instancias.iter().map(Value::to_string).collect();
    escribir_script(
        &c.bins.join("vortex"),
        &format!(
            "#!/bin/sh\ncase \"$1 $2\" in\n  \"mods instancias\") cat <<'FIN'\n{}\nFIN\n  ;;\n  \"mods mover\") mkdir -p \"$4/mods\" && mv -n \"$3\" \"$4/mods/\" && echo \"$4/mods/$(basename \"$3\")\" ;;\n  *) exit 1 ;;\nesac\n",
            lineas.join("\n")
        ),
    );
}

#[test]
fn mod_a_una_instancia() {
    let c = Casa::nueva();
    let juego = c
        .home
        .join(".local/share/PrismLauncher/instances/Fabric/minecraft");
    std::fs::create_dir_all(&juego).unwrap();
    vortex_falso(
        &c,
        &[
            json!({"nombre": "Fabric 1.21", "launcher": "Prism", "ruta": juego.to_string_lossy(), "version": "1.21.1", "loader": "fabric"}),
            json!({"nombre": "Forge viejo", "launcher": "Prism", "ruta": c.home.join("x").to_string_lossy(), "version": "1.20.1", "loader": "forge"}),
        ],
    );
    let mod_ = mod_fabric(&c.descargas.join("sodium.jar"));
    let info = c.analizar(&mod_);
    let vista = motor::para_mostrar(&c.ctx, &info);
    let compatibles: Vec<&Value> = vista["instancias"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| &i["compatible"])
        .collect();
    assert_eq!(compatibles, [&json!(true), &json!(false)]);
    let r = c
        .ejecutar(
            &info,
            "mod_instancia",
            json!({"ruta": juego.to_string_lossy(), "instancia": "Fabric 1.21"}),
        )
        .unwrap();
    assert!(juego.join("mods/sodium.jar").exists() && texto(&r, "titulo").contains("Fabric 1.21"));
    let id = texto(&json!(c.registro()[0]), "id");
    motor::deshacer(&c.ctx, &id).unwrap();
    assert!(!juego.join("mods/sodium.jar").exists());
}

#[test]
fn mod_sin_vortex_mods_carpeta_a_mano() {
    let c = Casa::nueva();
    escribir_script(&c.bins.join("vortex"), "#!/bin/sh\nexit 127\n");
    let info = c.analizar(&mod_fabric(&c.descargas.join("sodium.jar")));
    let vista = motor::para_mostrar(&c.ctx, &info);
    assert_eq!(vista["acciones"][0]["id"], "mod_carpeta");
    assert!(
        !vista["acciones"][0]["elegirCarpeta"].is_null()
            && vista["acciones"][0]["elegirCarpeta"] != json!(false)
    );
    let juego = c.home.join(".minecraft");
    std::fs::create_dir_all(juego.join("mods")).unwrap();
    c.ejecutar(&info, "mod_carpeta", json!({"carpeta": juego.to_string_lossy()}))
        .unwrap();
    assert!(juego.join("mods/sodium.jar").exists() && c.descargas.join("sodium.jar").exists());
}

fn leer_dir(d: &Path, ext: &str) -> Vec<PathBuf> {
    std::fs::read_dir(d)
        .map(|l| {
            l.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .collect()
        })
        .unwrap_or_default()
}

fn marcadores(texto: &str) -> BTreeSet<String> {
    Regex::new(r"\{(\w+)\}")
        .unwrap()
        .captures_iter(texto)
        .map(|c| c[1].to_string())
        .collect()
}

#[test]
fn textos_completos_en_los_dos_idiomas() {
    let mut usadas: BTreeSet<String> = BTreeSet::new();
    let tr = Regex::new(r#"tr\(\s*"([a-z_0-9]+)""#).unwrap();
    for p in leer_dir(&raiz().join("src"), "rs") {
        let t = std::fs::read_to_string(&p).unwrap();
        if p.file_name().is_some_and(|n| n == "textos_tabla.rs") {
            continue;
        }
        usadas.extend(tr.captures_iter(&t).map(|c| c[1].to_string()));
    }
    let t_qml = Regex::new(r"\bt\.([a-z_0-9]+)").unwrap();
    for p in leer_dir(&raiz().join("qt/qml"), "qml") {
        let t = std::fs::read_to_string(&p).unwrap();
        usadas.extend(t_qml.captures_iter(&t).map(|c| c[1].to_string()));
    }
    usadas.extend(motor::claves_de_tipos().iter().map(|s| s.to_string()));
    let claves: BTreeSet<&str> = textos::tabla().iter().map(|(k, _, _)| *k).collect();
    let faltan: Vec<&String> = usadas.iter().filter(|u| !claves.contains(u.as_str())).collect();
    assert!(faltan.is_empty(), "{faltan:?}");
    for (clave, es, en) in textos::tabla() {
        assert!(!es.is_empty() && !en.is_empty(), "{clave}");
        assert_eq!(marcadores(es), marcadores(en), "{clave}");
    }
    assert_eq!(
        textos::tr(Idioma::En, "hecho_instalado", &[("nombre", "X")]),
        "“X” is installed"
    );
    assert_eq!(
        textos::tr(Idioma::En, "contenido_resumen", &[("n", "1"), ("tam", "1 KB")]),
        "1 item · 1 KB in total"
    );
}

#[test]
fn idioma_por_locale() {
    let v = |s: &str| s.to_string();
    assert_eq!(
        Idioma::de_variables("", &[v(""), v(""), v("es_MX.UTF-8"), v("")]),
        Idioma::Es
    );
    assert_eq!(
        Idioma::de_variables("", &[v(""), v(""), v("en_US.UTF-8"), v("")]),
        Idioma::En
    );
    assert_eq!(Idioma::de_variables("en", &[v("es_MX.UTF-8")]), Idioma::En);
}

#[test]
fn tamano_legible() {
    let c = Casa::nueva();
    assert_eq!(motor::tamano_legible(&c.ctx, 500), "500 bytes");
    assert_eq!(motor::tamano_legible(&c.ctx, 1536), "1,5 KB");
    assert_eq!(motor::tamano_legible(&c.ctx, 147 * 1024 * 1024), "147 MB");
}

#[test]
fn desktop_de_la_app_tiene_todos_los_mime() {
    let texto =
        std::fs::read_to_string(raiz().join("share/applications/vortex-app-installer.desktop")).unwrap();
    let mime = Regex::new(r"(?m)^MimeType=(.*)$")
        .unwrap()
        .captures(&texto)
        .unwrap()[1]
        .to_string();
    let mime: Vec<&str> = mime.split(';').collect();
    for m in [
        "application/zip",
        "application/x-rar",
        "application/x-7z-compressed",
        "application/x-compressed-tar",
        "application/x-xz-compressed-tar",
        "application/x-zstd-compressed-tar",
        "application/x-bzip2-compressed-tar",
        "application/gzip",
        "application/x-xz",
        "application/zstd",
        "application/x-cd-image",
        "application/vnd.appimage",
        "application/x-shellscript",
        "application/vnd.debian.binary-package",
        "application/x-rpm",
        "application/vnd.flatpak.ref",
        "application/vnd.flatpak",
        "application/x-msdos-program",
        "application/x-msi",
        "application/x-java-archive",
    ] {
        assert!(mime.contains(&m), "{m}");
    }
    assert!(
        Regex::new(r"(?m)^Exec=vortex-app-installer %f$")
            .unwrap()
            .is_match(&texto)
    );
}
