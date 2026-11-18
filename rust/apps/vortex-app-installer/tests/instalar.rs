//! AppImage falso, programa .tar.gz, desinstalar (a la papelera) y lo que corre código (`test_instalar.py`).
mod comun;

use comun::*;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use vortex_app_installer::instalar::{leer_desktop, valor};
use vortex_app_installer::{Error, motor};

fn es_ejecutable(p: &Path) -> bool {
    std::fs::metadata(p)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn desktop(c: &Casa, nombre: &str) -> vortex_app_installer::instalar::Grupos {
    leer_desktop(&std::fs::read_to_string(c.home.join(".local/share/applications").join(nombre)).unwrap())
}

#[test]
fn appimage_va_a_aplicaciones_si_ya_la_usas() {
    let c = Casa::nueva();
    std::fs::create_dir(c.home.join("Aplicaciones")).unwrap();
    let origen = appimage_falso(&c.descargas.join("Falso-1.0-x86_64.AppImage"), "Falso");
    c.ejecutar(&c.detectar(&origen), "instalar_appimage", json!({}))
        .unwrap();
    assert!(c.home.join("Aplicaciones/Falso-1.0-x86_64.AppImage").exists());
    assert!(!c.home.join("Applications").exists());
}

#[test]
fn appimage_instalar_y_desinstalar() {
    let c = Casa::nueva();
    let origen = appimage_falso(&c.descargas.join("Falso-1.0-x86_64.AppImage"), "Falso");
    let info = c.detectar(&origen);
    let vista = motor::para_mostrar(&c.ctx, &info);
    assert!(!texto(&vista, "aviso").is_empty());
    assert!(
        !vista["acciones"][0]["confirmar"].is_null(),
        "viene de internet: se confirma"
    );
    let r = c.ejecutar(&info, "instalar_appimage", json!({})).unwrap();
    let copia = c.home.join("Applications/Falso-1.0-x86_64.AppImage");
    assert!(copia.exists() && es_ejecutable(&copia) && origen.exists());
    let g = desktop(&c, "falso.desktop");
    assert_eq!(valor(&g, "Name", "Desktop Entry"), "Falso");
    let exec = valor(&g, "Exec", "Desktop Entry");
    assert!(
        exec.starts_with(&copia.to_string_lossy().into_owned()) && exec.ends_with("%U"),
        "{exec}"
    );
    assert_eq!(valor(&g, "Icon", "Desktop Entry"), "falso");
    assert!(
        c.home
            .join(".local/share/icons/hicolor/64x64/apps/falso.png")
            .exists()
    );
    let script = c
        .home
        .join(".local/share/vortex-app-installer/desinstaladores/appimage-falso.sh");
    assert!(
        std::fs::read_to_string(&script)
            .unwrap()
            .contains(&*copia.to_string_lossy())
    );
    assert_eq!(r["id"], "appimage-falso");
    let nombres: Vec<String> = motor::instalados(&c.ctx)
        .iter()
        .map(|e| texto(e, "nombre"))
        .collect();
    assert_eq!(nombres, ["Falso"]);

    // otra vez: pregunta antes de reemplazar, y al reemplazar la vieja va a la papelera
    assert!(matches!(
        c.ejecutar(&info, "instalar_appimage", json!({})),
        Err(Error::Conflicto { .. })
    ));
    let nueva = appimage_falso(&c.descargas.join("Falso-2.0-x86_64.AppImage"), "Falso");
    c.ejecutar(
        &c.detectar(&nueva),
        "instalar_appimage",
        json!({"reemplazar": true}),
    )
    .unwrap();
    assert!(!copia.exists() && c.home.join("Applications/Falso-2.0-x86_64.AppImage").exists());
    assert_eq!(c.registro().len(), 1);

    motor::deshacer(&c.ctx, "appimage-falso").unwrap();
    assert!(
        std::fs::read_dir(c.home.join("Applications"))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(!c.home.join(".local/share/applications/falso.desktop").exists());
    assert!(
        !c.home
            .join(".local/share/icons/hicolor/64x64/apps/falso.png")
            .exists()
    );
    assert!(c.registro().is_empty());
    assert!(c.home.join(".local/share/Trash/files").exists());
}

#[test]
fn appimage_solo_abrir() {
    let c = Casa::nueva();
    let origen = appimage_falso(&c.descargas.join("Falso.AppImage"), "Falso");
    c.ejecutar(&c.detectar(&origen), "abrir_appimage", json!({}))
        .unwrap();
    assert!(es_ejecutable(&origen));
    assert_eq!(c.lanzados(), [vec![origen.to_string_lossy().into_owned()]]);
    assert!(c.registro().is_empty(), "no se instaló nada");
}

#[test]
fn appimage_no_pisa_uno_ajeno() {
    let c = Casa::nueva();
    let ajeno = c.home.join("Applications/Falso.AppImage");
    std::fs::create_dir_all(ajeno.parent().unwrap()).unwrap();
    std::fs::write(&ajeno, "de otra persona").unwrap();
    let info = c.detectar(&appimage_falso(&c.descargas.join("Falso.AppImage"), "Falso"));
    assert!(matches!(
        c.ejecutar(&info, "instalar_appimage", json!({})),
        Err(Error::Conflicto { .. })
    ));
    c.ejecutar(&info, "instalar_appimage", json!({"reemplazar": true}))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&ajeno).unwrap(),
        "de otra persona",
        "el suyo no se toca: el nuevo va al lado"
    );
    assert!(c.home.join("Applications/Falso (2).AppImage").exists());
}

#[test]
fn programa_tar_gz() {
    let c = Casa::nueva();
    let origen = programa_tar(&c.descargas.join("MiPrograma-1.0-linux-x86_64.tar.gz"));
    let info = c.detectar(&origen);
    let r = c.ejecutar(&info, "instalar_programa", json!({})).unwrap();
    let opt = c.home.join(".local/opt/mi-programa");
    let exe = opt.join("mi-programa/mi-programa");
    assert!(exe.exists());
    assert_eq!(texto(&r, "abrir"), exe.to_string_lossy());
    let g = desktop(&c, "mi-programa.desktop");
    let e = exe.to_string_lossy().into_owned();
    assert_eq!(
        valor(&g, "Exec", "Desktop Entry"),
        format!("{e} %F"),
        "el «@EXEC@» se reemplazó"
    );
    assert_eq!(valor(&g, "TryExec", "Desktop Entry"), e);
    assert_eq!(valor(&g, "Exec", "Desktop Action nueva"), format!("{e} --nueva"));
    assert_eq!(valor(&g, "Icon", "Desktop Entry"), "mi-programa");
    assert!(
        c.home
            .join(".local/share/icons/hicolor/128x128/apps/mi-programa.png")
            .exists()
    );
    let desinstalar = opt.join("desinstalar.sh");
    assert!(es_ejecutable(&desinstalar));
    assert!(
        std::fs::read_to_string(&desinstalar)
            .unwrap()
            .contains(&*opt.to_string_lossy())
    );
    assert!(
        std::fs::read_to_string(opt.join("desinstalar.sh.original"))
            .unwrap()
            .ends_with("echo suyo\n"),
        "el suyo se guarda aparte"
    );
    let tipos: Vec<String> = c.registro().iter().map(|e| texto(&json!(e), "tipo")).collect();
    assert_eq!(tipos, ["programa"]);

    motor::deshacer(&c.ctx, r["id"].as_str().unwrap()).unwrap();
    assert!(
        !opt.exists()
            && !c
                .home
                .join(".local/share/applications/mi-programa.desktop")
                .exists()
    );
    assert!(c.registro().is_empty());
}

#[test]
fn programa_el_desinstalar_sh_funciona() {
    let c = Casa::nueva();
    let info = c.detectar(&programa_tar(&c.descargas.join("MiPrograma.tar.gz")));
    c.ejecutar(&info, "instalar_programa", json!({})).unwrap();
    let opt = c.home.join(".local/opt/mi-programa");
    let r = Command::new("sh")
        .arg(opt.join("desinstalar.sh"))
        .env("HOME", &c.home)
        .env_remove("XDG_DATA_HOME")
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!opt.exists());
    assert!(
        c.registro().is_empty(),
        "la entrada se va sola: sus archivos ya no existen"
    );
}

#[test]
fn programa_ya_instalado_pregunta() {
    let c = Casa::nueva();
    let info = c.detectar(&programa_tar(&c.descargas.join("MiPrograma.tar.gz")));
    c.ejecutar(&info, "instalar_programa", json!({})).unwrap();
    assert!(matches!(
        c.ejecutar(&info, "instalar_programa", json!({})),
        Err(Error::Conflicto { .. })
    ));
    c.ejecutar(&info, "instalar_programa", json!({"reemplazar": true}))
        .unwrap();
    assert_eq!(c.registro().len(), 1);
    assert!(c.home.join(".local/opt/mi-programa").exists());
}

#[test]
fn deshacer_nunca_toca_carpetas_protegidas() {
    let c = Casa::nueva();
    let mut e = serde_json::Map::new();
    e.insert("id".into(), json!("descomprimido-trampa"));
    e.insert("tipo".into(), json!("descomprimido"));
    e.insert("nombre".into(), json!("x"));
    e.insert(
        "rutas".into(),
        json!([c.home.to_string_lossy(), c.descargas.to_string_lossy(), "/"]),
    );
    vortex_app_installer::registro::agregar(&c.ctx, e).unwrap();
    let _ = motor::deshacer(&c.ctx, "descomprimido-trampa");
    assert!(c.home.exists() && c.descargas.exists());
}

#[test]
fn script_se_confirma_y_va_a_terminal() {
    let c = Casa::nueva();
    escribir_script(&c.bins.join("kitty"), "#!/bin/sh\nexit 0\n");
    let sh = c.descargas.join("instalar.sh");
    std::fs::write(&sh, "#!/bin/bash\necho hola\n").unwrap();
    let info = c.detectar(&sh);
    let vista = motor::para_mostrar(&c.ctx, &info);
    let accion = &vista["acciones"][0];
    assert_eq!(accion["id"], "correr");
    assert!(!accion["confirmar"]["titulo"].as_str().unwrap_or("").is_empty());
    c.ejecutar(&info, "correr", json!({})).unwrap();
    let l = &c.lanzados()[0];
    assert!(l[0].ends_with("kitty") && l[1] == "--hold", "{l:?}");
    assert_eq!(
        l[l.len() - 2..],
        ["/bin/bash".to_string(), sh.to_string_lossy().into_owned()]
    );
    assert!(!l.contains(&"sudo".to_string()));
}

#[test]
fn script_que_quiere_sudo_no_se_corre() {
    let c = Casa::nueva();
    escribir_script(&c.bins.join("kitty"), "#!/bin/sh\nexit 0\n");
    let sh = c.descargas.join("root.sh");
    std::fs::write(&sh, "#!/usr/bin/sudo /bin/sh\necho x\n").unwrap();
    let r = c.ejecutar(&c.detectar(&sh), "correr", json!({}));
    assert!(matches!(r, Err(Error::Problema { .. })), "{r:?}");
    assert!(c.lanzados().is_empty());
}

fn exe_windows(c: &Casa, nombre: &str) -> std::path::PathBuf {
    let exe = c.descargas.join(nombre);
    let mut d = b"MZ".to_vec();
    d.extend_from_slice(&[0u8; 10]);
    std::fs::write(&exe, d).unwrap();
    exe
}

#[test]
fn windows_sin_wine() {
    let mut c = Casa::nueva();
    c.solo(&["bsdtar", "head", "file"]); // sin wine, umu ni Runner
    let exe = exe_windows(&c, "Setup.exe");
    let vista = motor::para_mostrar(&c.ctx, &motor::analizar(&c.ctx, &exe, None));
    assert_eq!(texto(&vista, "comando"), "sudo pacman -S wine");
    let ids: Vec<&str> = vista["acciones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["abrir_carpeta"]);
}

#[test]
fn windows_con_wine() {
    let mut c = Casa::nueva();
    escribir_script(&c.bins.join("wine"), "#!/bin/sh\nexit 0\n");
    c.solo(&["bsdtar", "head", "file"]);
    let exe = exe_windows(&c, "juego.exe");
    let info = c.analizar(&exe);
    let vista = motor::para_mostrar(&c.ctx, &info);
    let accion = &vista["acciones"][0];
    assert_eq!(accion["id"], "windows");
    assert!(!accion["confirmar"].is_null());
    c.ejecutar(&info, "windows", accion["datos"].clone()).unwrap();
    let l = &c.lanzados()[0];
    assert!(l[0].ends_with("wine") && l[1] == exe.to_string_lossy(), "{l:?}");
}

#[test]
fn windows_con_runner() {
    let mut c = Casa::nueva();
    c.solo(&["bsdtar", "head", "file"]);
    let runner = c.ctx.rutas.datos_xdg().join("vortex-runner/vortex-runner");
    std::fs::create_dir_all(runner.parent().unwrap()).unwrap();
    escribir_script(&runner, "#!/bin/sh\n");
    let o = vortex_app_installer::instalar::opciones_windows(&c.ctx);
    assert_eq!(o[0], json!({"id": "runner", "nombre": "Vortexine's Runner"}));
    let exe = exe_windows(&c, "juego.exe");
    let info = c.analizar(&exe);
    let accion = motor::para_mostrar(&c.ctx, &info)["acciones"][0].clone();
    assert_eq!(accion["datos"], json!({"opcion": "runner"}));
    c.ejecutar(&info, "windows", accion["datos"].clone()).unwrap();
    assert_eq!(
        c.lanzados(),
        [vec![
            runner.to_string_lossy().into_owned(),
            exe.to_string_lossy().into_owned()
        ]]
    );
}
