//! Detectar cada formato por su firma y su extensión (`test_detectar.py`).
mod comun;

use comun::*;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use vortex_app_installer::detectar::{self, Info};
use vortex_app_installer::motor;

fn bsdtar_crear(destino: &Path, formato: &str, carpeta: &Path) {
    std::fs::write(carpeta.join("dentro.txt"), "hola").unwrap();
    let ok = Command::new("bsdtar")
        .args(["--format", formato, "-cf"])
        .arg(destino)
        .arg("-C")
        .arg(carpeta)
        .arg("dentro.txt")
        .status()
        .unwrap();
    assert!(ok.success());
}

fn archivos(info: &Info) -> Vec<String> {
    let mut v: Vec<String> = info
        .entradas
        .iter()
        .filter(|e| e.tipo == vortex_app_installer::comprimidos::Tipo::Archivo)
        .map(|e| e.nombre.clone())
        .collect();
    v.sort();
    v
}

#[test]
fn tar_de_todos_los_sabores() {
    for (nombre, formato) in
        [("a.tar.gz", "gz"), ("a.tgz", "gz"), ("a.tar.xz", "xz"), ("a.tar.bz2", "bz2"), ("a.tar.zst", "zst"), ("a.tar", "")]
    {
        let c = Casa::nueva();
        let ruta = tar(&c.descargas.join(nombre), &[arch("carpeta/uno.txt", b"1"), arch("carpeta/dos.txt", b"22")], formato);
        let info = c.detectar(&ruta);
        assert_eq!(info.tipo, "comprimido", "{nombre}");
        assert_eq!(archivos(&info), ["carpeta/dos.txt", "carpeta/uno.txt"], "{nombre}");
        assert!(info.de_internet, "{nombre}: está en Descargas");
    }
}

#[test]
fn zip_7z_iso() {
    let c = Casa::nueva();
    zip_(&c.descargas.join("a.zip"), &[("x/y.txt", b"y")]);
    let otra = c.tmp.path().join("fuente");
    std::fs::create_dir(&otra).unwrap();
    bsdtar_crear(&c.descargas.join("a.7z"), "7zip", &otra);
    bsdtar_crear(&c.descargas.join("a.iso"), "iso9660", &otra);
    for (n, fmt) in [("a.zip", "zip"), ("a.7z", "7z"), ("a.iso", "iso")] {
        let info = c.detectar(&c.descargas.join(n));
        assert_eq!((info.tipo.as_str(), info.formato.as_str()), ("comprimido", fmt), "{n}");
        assert!(!info.entradas.is_empty(), "{n}");
    }
}

#[test]
fn comprimido_sin_extension_por_firma() {
    let c = Casa::nueva();
    let ruta = zip_(&c.home.join("sin_nombre"), &[("a.txt", b"a")]);
    let info = c.detectar(&ruta);
    assert_eq!((info.tipo.as_str(), info.formato.as_str()), ("comprimido", "zip"));
    assert!(!info.de_internet);
}

#[test]
fn sueltos() {
    let c = Casa::nueva();
    let datos = b"hola mundo\n".repeat(100);
    for (ext, formato) in [(".gz", "gz"), (".xz", "xz"), (".bz2", "bz2"), (".zst", "zst")] {
        let ruta = c.descargas.join(format!("notas.txt{ext}"));
        std::fs::write(&ruta, comprimir(&datos, formato)).unwrap();
        let info = c.detectar(&ruta);
        assert_eq!(info.tipo, "comprimido", "{ext}");
        let nombres: Vec<&str> = info.entradas.iter().map(|e| e.nombre.as_str()).collect();
        assert_eq!(nombres, ["notas.txt"], "{ext}");
    }
}

#[test]
fn gz_que_es_un_tar() {
    let c = Casa::nueva();
    let ruta = tar(&c.descargas.join("paquete.gz"), &[arch("a/b.txt", b"b")], "gz");
    let info = c.detectar(&ruta);
    let nombres: Vec<&str> = info.entradas.iter().map(|e| e.nombre.as_str()).collect();
    assert_eq!(nombres, ["a/b.txt"]);
}

#[test]
fn rar_danado_se_dice_claro() {
    let c = Casa::nueva();
    let ruta = c.descargas.join("roto.rar");
    let mut d = b"Rar!\x1a\x07\x01\x00".to_vec();
    d.extend_from_slice(&[0u8; 40]);
    std::fs::write(&ruta, d).unwrap();
    let info = c.detectar(&ruta);
    assert_eq!(info.tipo, "comprimido");
    assert!(!info.error.is_empty(), "dañado o falta un programa, pero nunca se queda callado");
}

#[test]
fn programa_tar_gz() {
    let c = Casa::nueva();
    let info = c.detectar(&programa_tar(&c.descargas.join("MiPrograma-1.0-linux-x86_64.tar.gz")));
    assert_eq!(info.tipo, "programa");
    let vista = motor::para_mostrar(&c.ctx, &info);
    assert_eq!(vista["acciones"][0]["id"], "instalar_programa");
    assert!(texto(&vista, "descripcion").contains("~/.local/opt/mi-programa"));
}

#[test]
fn appimage_por_extension_y_por_firma() {
    let c = Casa::nueva();
    let a = appimage_falso(&c.descargas.join("Falso-1.0-x86_64.AppImage"), "Falso");
    assert_eq!(c.detectar(&a).tipo, "appimage");
    let b = c.descargas.join("sin_extension");
    let mut d = b"\x7fELF\x02\x01\x01\x00AI\x02".to_vec();
    d.extend_from_slice(&[0u8; 100]);
    std::fs::write(&b, d).unwrap();
    assert_eq!(c.detectar(&b).tipo, "appimage");
}

#[test]
fn scripts_y_binarios() {
    let c = Casa::nueva();
    let sh = c.descargas.join("instalar.sh");
    std::fs::write(&sh, "#!/bin/bash\n# instalador\nsudo cp x /usr/bin\n").unwrap();
    let info = c.detectar(&sh);
    assert_eq!(info.tipo, "script");
    assert!(info.extra_texto("vista").starts_with("#!/bin/bash"));
    assert!(info.extra_si("pide_admin"));
    let run = c.descargas.join("juego.run");
    let mut d = b"#!/bin/sh\necho hola\nexit 0\n\x00\x01\x02".to_vec();
    d.extend_from_slice(b"binario pegado");
    std::fs::write(&run, d).unwrap();
    let info = c.detectar(&run);
    assert!(info.tipo == "script" && !info.extra_texto("vista").contains("binario"));
    let binario = c.descargas.join("programa.bin");
    let mut d = b"\x7fELF\x02\x01\x01\x00".to_vec();
    d.extend_from_slice(&[0u8; 64]);
    std::fs::write(&binario, d).unwrap();
    let info = c.detectar(&binario);
    assert!(info.tipo == "script" && info.extra_si("binario") && info.extra_texto("vista").is_empty());
    assert!(info.ejecuta_codigo() && !texto(&motor::para_mostrar(&c.ctx, &info), "aviso").is_empty());
}

#[test]
fn deb_y_rpm() {
    let c = Casa::nueva();
    let info = c.detectar(&deb(&c.descargas.join("google-chrome-stable_current_amd64.deb"), "google-chrome-stable"));
    assert_eq!(info.tipo, "deb");
    assert_eq!(info.extra_texto("nombre"), "google-chrome-stable");
    let rpm = c.descargas.join("hola-mundo-1.2-3.fc40.x86_64.rpm");
    let mut d = b"\xed\xab\xee\xdb".to_vec();
    d.extend_from_slice(&[0u8; 200]);
    std::fs::write(&rpm, d).unwrap();
    let info = c.detectar(&rpm);
    assert!(info.tipo == "rpm" && info.extra_texto("nombre") == "hola-mundo");
}

#[test]
fn flatpak() {
    let c = Casa::nueva();
    let r = c.descargas.join("org.example.App.flatpakref");
    std::fs::write(&r, "[Flatpak Ref]\nName=org.example.App\nTitle=Ejemplo\nUrl=https://dl.flathub.org/repo/\n").unwrap();
    let info = c.detectar(&r);
    assert!(info.tipo == "flatpakref" && info.extra_texto("app_id") == "org.example.App");
    let bundle = c.descargas.join("app.flatpak");
    std::fs::write(&bundle, b"xxx").unwrap();
    assert_eq!(c.detectar(&bundle).tipo, "flatpak");
}

#[test]
fn windows() {
    let c = Casa::nueva();
    let exe = c.descargas.join("DiscordSetup-1.0.9-x64.exe");
    let mut d = b"MZ".to_vec();
    d.extend_from_slice(&[0u8; 100]);
    std::fs::write(&exe, d).unwrap();
    let info = c.detectar(&exe);
    assert!(info.tipo == "windows" && info.extra_texto("nombre_programa") == "Discord");
    let msi = c.descargas.join("programa.msi");
    let mut d = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1".to_vec();
    d.extend_from_slice(&[0u8; 100]);
    std::fs::write(&msi, d).unwrap();
    assert_eq!(c.detectar(&msi).tipo, "windows");
}

fn mod_de(info: &Info, k: &str) -> String {
    info.extra["mod"][k].as_str().unwrap_or("").to_string()
}

#[test]
fn jar_mod_y_programa() {
    let c = Casa::nueva();
    let fabric = c.detectar(&mod_fabric(&c.descargas.join("sodium-fabric-0.6.0.jar")));
    assert!(fabric.tipo == "mod" && mod_de(&fabric, "loader") == "fabric");
    assert!(mod_de(&fabric, "nombre") == "Sodium" && mod_de(&fabric, "minecraft") == "1.21.1");
    let neo = c.detectar(&mod_neoforge(&c.descargas.join("jei.jar")));
    assert!(neo.tipo == "mod" && mod_de(&neo, "loader") == "neoforge");
    assert!(mod_de(&neo, "nombre") == "Just Enough Items" && mod_de(&neo, "version").is_empty());
    assert_eq!(c.detectar(&jar_java(&c.descargas.join("herramienta.jar"))).tipo, "jar");
}

#[test]
fn desconocido_y_carpeta() {
    let c = Casa::nueva();
    let raro = c.descargas.join("datos.xyz");
    let datos: Vec<u8> = (0..=255u8).cycle().take(1024).collect();
    std::fs::write(&raro, datos).unwrap();
    let info = c.detectar(&raro);
    assert_eq!(info.tipo, "desconocido");
    let vista = motor::para_mostrar(&c.ctx, &info);
    let ids: Vec<&str> = vista["acciones"].as_array().unwrap().iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["abrir_carpeta"]);
    assert_eq!(c.detectar(&c.descargas).tipo, "carpeta");
    assert_eq!(c.detectar(&c.home.join("no-existe.zip")).tipo, "falta");
}

#[test]
fn origen_del_navegador() {
    let c = Casa::nueva();
    let ruta = zip_(&c.home.join("bajado.zip"), &[("a", b"a")]);
    let url = b"https://www.github.com/x/y/releases/download/v1/bajado.zip";
    let ok = vortex_app_installer::util::poner_atributo(&ruta, "user.xdg.origin.url", url);
    if !ok {
        eprintln!("este sistema de archivos no guarda atributos: se salta");
        return;
    }
    let info = c.detectar(&ruta);
    assert!(info.de_internet && detectar::sitio(&info.origen) == "github.com");
    let vista: Value = motor::para_mostrar(&c.ctx, &info);
    assert_eq!(texto(&vista, "origen"), "Bajado de github.com");
}
