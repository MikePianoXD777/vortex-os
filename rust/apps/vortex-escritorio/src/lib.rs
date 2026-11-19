//! Otros escritorios en Vortexine's OS (4.0). Vortex Shell sigue siendo el de fábrica; quien quiera
//! puede instalar KDE Plasma, GNOME, COSMIC o XFCE y elegirlo en el inicio de sesión (abajo a la
//! izquierda). El login recuerda el último de cada persona.
//!
//! - `vortex-escritorio` (o `vortex escritorio`): los que hay y los que se pueden instalar.
//! - `instalar|quitar kde|gnome|cosmic|xfce`: con pacman (pide tu contraseña en la terminal).
//! - `sesiones`: para el login, una por línea: `id<TAB>tipo<TAB>escritorios<TAB>nombre`.
//! - `entrar ID`: lo que corre greetd al entrar (desde `config/greeter/entrar`): arranca esa sesión con
//!   las variables que pone un gestor de inicio de sesión (`XDG_SESSION_TYPE`, `XDG_CURRENT_DESKTOP`…).
//!
//! Las sesiones de Hyprland que trae su paquete no se listan: «Vortexine's OS» ya es esa.

use std::fs;
use std::path::{Path, PathBuf};

pub const WAYLAND: &str = "/usr/share/wayland-sessions";
pub const X11: &str = "/usr/share/xsessions";

/// Una sesión instalada (un `.desktop` de `wayland-sessions` o `xsessions`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sesion {
    /// El nombre del archivo sin `.desktop` (`plasma`, `gnome`, `xfce`…).
    pub id: String,
    pub nombre: String,
    /// La orden de `Exec=` ya partida (sin los `%f` y compañía).
    pub orden: Vec<String>,
    /// `DesktopNames=` con `:` (como lo quiere `XDG_CURRENT_DESKTOP`).
    pub escritorios: String,
    pub x11: bool,
}

/// Un escritorio que se puede instalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Escritorio {
    pub clave: &'static str,
    pub nombre: &'static str,
    /// Los paquetes (pacman acepta grupos, como `xfce4`).
    pub paquetes: &'static [&'static str],
    /// Las sesiones que deja (para saber si ya está).
    pub sesiones: &'static [&'static str],
    pub nota: &'static str,
}

/// Lo justo de cada escritorio: su sesión, su terminal, sus archivos, sus ajustes y su portal. Nada de
/// gestores de inicio de sesión (gdm, sddm): el login es el de Vortexine's.
pub const CATALOGO: &[Escritorio] = &[
    Escritorio {
        clave: "kde",
        nombre: "KDE Plasma",
        paquetes: &[
            "plasma-desktop",
            "konsole",
            "dolphin",
            "plasma-nm",
            "plasma-pa",
            "kscreen",
            "powerdevil",
            "breeze-gtk",
            "kde-gtk-config",
            "xdg-desktop-portal-kde",
        ],
        sesiones: &["plasma"],
        nota: "Parecido a Windows, con todo configurable",
    },
    Escritorio {
        clave: "gnome",
        nombre: "GNOME",
        paquetes: &[
            "gnome-shell",
            "gnome-session",
            "gnome-control-center",
            "gnome-settings-daemon",
            "nautilus",
            "gnome-console",
            "xdg-desktop-portal-gnome",
        ],
        sesiones: &["gnome", "gnome-wayland"],
        nota: "Sencillo y ordenado, con sus actividades",
    },
    Escritorio {
        clave: "cosmic",
        nombre: "COSMIC",
        paquetes: &[
            "cosmic-session",
            "cosmic-files",
            "cosmic-terminal",
            "cosmic-settings",
            "xdg-desktop-portal-cosmic",
        ],
        sesiones: &["cosmic"],
        nota: "El de System76, hecho en Rust",
    },
    Escritorio {
        clave: "xfce",
        nombre: "XFCE",
        paquetes: &["xfce4", "xorg-server", "xorg-xinit"],
        sesiones: &["xfce"],
        nota: "Ligero y clásico (usa X11)",
    },
];

pub fn buscar(clave: &str) -> Option<&'static Escritorio> {
    let c = clave.trim().to_lowercase();
    CATALOGO
        .iter()
        .find(|e| e.clave == c || e.nombre.to_lowercase() == c || (c == "plasma" && e.clave == "kde"))
}

/// Parte un `Exec=` como dice la especificación de los `.desktop`: comillas dobles con `\"`, `\\`, `` \` ``
/// y `\$` adentro; los códigos `%f`, `%U`… se quitan y `%%` es un `%`.
pub fn partir_exec(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut actual = String::new();
    let mut hay = false;
    let mut comillas = false;
    let mut cs = exec.chars().peekable();
    while let Some(c) = cs.next() {
        match c {
            '"' => {
                comillas = !comillas;
                hay = true;
            }
            '\\' if comillas => {
                if let Some(n) = cs.next() {
                    actual.push(n);
                }
            }
            ' ' | '\t' if !comillas => {
                if hay {
                    args.push(std::mem::take(&mut actual));
                    hay = false;
                }
            }
            // `%%` es un `%`; cualquier otro (un código de campo: sin archivos ni URLs que pasar) se quita.
            '%' => {
                if cs.next() == Some('%') {
                    actual.push('%');
                    hay = true;
                }
            }
            _ => {
                actual.push(c);
                hay = true;
            }
        }
    }
    if hay {
        args.push(actual);
    }
    args
}

/// Lee un `.desktop` de sesión. `None` si está escondido, no tiene `Exec` o es de Hyprland.
pub fn leer_sesion(ruta: &Path, x11: bool) -> Option<Sesion> {
    let texto = fs::read_to_string(ruta).ok()?;
    let id = ruta.file_stem()?.to_string_lossy().into_owned();
    let (mut nombre, mut exec, mut escritorios) = (None, None, String::new());
    let mut en_entrada = false;
    for linea in texto.lines() {
        let l = linea.trim();
        if l.starts_with('[') {
            en_entrada = l == "[Desktop Entry]";
            continue;
        }
        if !en_entrada {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        match k.trim() {
            "Name" => nombre = Some(v.trim().to_string()),
            "Exec" => exec = Some(v.trim().to_string()),
            "DesktopNames" => {
                escritorios = v.trim().trim_end_matches(';').replace(';', ":");
            }
            "Hidden" | "NoDisplay" if v.trim() == "true" => return None,
            _ => {}
        }
    }
    let orden = partir_exec(&exec?);
    let es_hyprland =
        id.starts_with("hyprland") || escritorios.split(':').any(|d| d.eq_ignore_ascii_case("Hyprland"));
    if orden.is_empty() || es_hyprland {
        return None;
    }
    Some(Sesion {
        nombre: nombre.unwrap_or_else(|| id.clone()),
        id,
        orden,
        escritorios,
        x11,
    })
}

/// Las sesiones de esas carpetas (primero las de Wayland; si una está en las dos, gana la de Wayland).
pub fn sesiones_en(wayland: &Path, x11: &Path) -> Vec<Sesion> {
    let mut lista: Vec<Sesion> = Vec::new();
    for (dir, es_x11) in [(wayland, false), (x11, true)] {
        let Ok(entradas) = fs::read_dir(dir) else { continue };
        let mut archivos: Vec<PathBuf> = entradas
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
            .collect();
        archivos.sort();
        for a in archivos {
            if let Some(s) = leer_sesion(&a, es_x11)
                && !lista.iter().any(|o| o.id == s.id)
            {
                lista.push(s);
            }
        }
    }
    lista
}

pub fn sesiones() -> Vec<Sesion> {
    sesiones_en(Path::new(WAYLAND), Path::new(X11))
}

/// ¿Ya está instalado ese escritorio (alguna de sus sesiones existe)?
pub fn instalado(e: &Escritorio, lista: &[Sesion]) -> bool {
    lista.iter().any(|s| e.sesiones.contains(&s.id.as_str()))
}

/// La línea de `sesiones` para el login (sin tabuladores dentro de los campos).
pub fn linea_para_login(s: &Sesion) -> String {
    let limpiar = |t: &str| t.replace(['\t', '\n'], " ");
    format!(
        "{}\t{}\t{}\t{}",
        limpiar(&s.id),
        if s.x11 { "x11" } else { "wayland" },
        limpiar(&s.escritorios),
        limpiar(&s.nombre)
    )
}

/// Las variables que pone un gestor de inicio de sesión para esa sesión.
pub fn variables(s: &Sesion) -> Vec<(&'static str, String)> {
    let mut v = vec![
        (
            "XDG_SESSION_TYPE",
            if s.x11 { "x11" } else { "wayland" }.to_string(),
        ),
        ("XDG_SESSION_DESKTOP", s.id.clone()),
    ];
    if !s.escritorios.is_empty() {
        v.push(("XDG_CURRENT_DESKTOP", s.escritorios.clone()));
    }
    v
}

/// Busca un programa en el `PATH` (startx quiere la ruta completa del cliente).
pub fn en_path(programa: &str, path: &str) -> Option<PathBuf> {
    if programa.contains('/') {
        return Path::new(programa).is_file().then(|| PathBuf::from(programa));
    }
    path.split(':')
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(programa))
        .find(|p| p.is_file())
}

/// La orden completa para arrancar la sesión. Las de X11 van con `startx` en la terminal virtual de la
/// sesión (`XDG_VTNR`, la pone greetd).
pub fn orden_de_arranque(s: &Sesion, path: &str, vt: Option<&str>) -> Result<Vec<String>, String> {
    if !s.x11 {
        return Ok(s.orden.clone());
    }
    let startx = en_path("startx", path)
        .ok_or_else(|| format!("«{}» usa X11 y falta startx (paquete xorg-xinit).", s.nombre))?;
    let cliente = en_path(&s.orden[0], path).ok_or_else(|| format!("no encontré «{}».", s.orden[0]))?;
    let mut o = vec![
        startx.to_string_lossy().into_owned(),
        cliente.to_string_lossy().into_owned(),
    ];
    o.extend(s.orden[1..].iter().cloned());
    o.push("--".into());
    o.push("-keeptty".into());
    if let Some(vt) = vt.filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())) {
        o.push(format!("vt{vt}"));
    }
    Ok(o)
}

/// El texto de `vortex escritorio` a secas.
pub fn texto_lista(lista: &[Sesion]) -> String {
    let mut t = String::from("Escritorios en esta PC:\n  • Vortexine's OS (Vortex Shell) — el de fábrica\n");
    for s in lista {
        t.push_str(&format!(
            "  • {}{}\n",
            s.nombre,
            if s.x11 { " (X11)" } else { "" }
        ));
    }
    let faltan: Vec<&Escritorio> = CATALOGO.iter().filter(|e| !instalado(e, lista)).collect();
    if !faltan.is_empty() {
        t.push_str("\nSe pueden instalar:\n");
        for e in faltan {
            t.push_str(&format!(
                "  • {:<11} vortex escritorio instalar {:<7} {}\n",
                e.nombre, e.clave, e.nota
            ));
        }
    }
    t.push_str(
        "\nPara cambiar de escritorio: cierra sesión y, en el inicio de sesión, elígelo abajo a la \
         izquierda.\nSe queda el último que elegiste (al prender la PC entras directo solo con Vortex Shell).\n",
    );
    t
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn exec_como_la_especificacion() {
        assert_eq!(
            partir_exec("/usr/bin/startplasma-wayland"),
            vec!["/usr/bin/startplasma-wayland"]
        );
        assert_eq!(
            partir_exec("gnome-session --session=gnome %U"),
            vec!["gnome-session", "--session=gnome"]
        );
        assert_eq!(
            partir_exec(r#"sh -c "echo \"hola\" 100%%""#),
            vec!["sh", "-c", "echo \"hola\" 100%"]
        );
        assert_eq!(partir_exec(r#"a "" b"#), vec!["a", "", "b"]);
    }

    fn escribir(dir: &Path, nombre: &str, texto: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(nombre), texto).unwrap();
    }

    #[test]
    fn lista_sin_hyprland_y_wayland_primero() {
        let t = tempfile::tempdir().unwrap();
        let (w, x) = (t.path().join("w"), t.path().join("x"));
        escribir(
            &w,
            "hyprland.desktop",
            "[Desktop Entry]\nName=Hyprland\nExec=/usr/bin/start-hyprland\nDesktopNames=Hyprland\n",
        );
        escribir(
            &w,
            "hyprland-uwsm.desktop",
            "[Desktop Entry]\nName=Hyprland (uwsm)\nExec=uwsm start hyprland.desktop\n",
        );
        escribir(
            &w,
            "plasma.desktop",
            "[Desktop Entry]\nName=Plasma (Wayland)\nExec=/usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland\nDesktopNames=KDE\n",
        );
        escribir(
            &x,
            "plasma.desktop",
            "[Desktop Entry]\nName=Plasma (X11)\nExec=startplasma-x11\nDesktopNames=KDE\n",
        );
        escribir(
            &x,
            "xfce.desktop",
            "[Desktop Entry]\nName=Xfce Session\nExec=startxfce4\nDesktopNames=XFCE\n",
        );
        escribir(
            &w,
            "oculto.desktop",
            "[Desktop Entry]\nName=X\nExec=x\nHidden=true\n",
        );
        escribir(
            &w,
            "gnome.desktop",
            "[Desktop Entry]\nName=GNOME\nExec=gnome-session\nDesktopNames=GNOME;\n[Desktop Action Algo]\nExec=otra\n",
        );
        let l = sesiones_en(&w, &x);
        let ids: Vec<&str> = l.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["gnome", "plasma", "xfce"]);
        assert_eq!(l[1].nombre, "Plasma (Wayland)");
        assert!(!l[1].x11 && l[2].x11);
        assert_eq!(l[0].orden, vec!["gnome-session"]);
        assert_eq!(l[0].escritorios, "GNOME");
        assert_eq!(linea_para_login(&l[2]), "xfce\tx11\tXFCE\tXfce Session");
        assert!(instalado(buscar("kde").unwrap(), &l));
        assert!(!instalado(buscar("cosmic").unwrap(), &l));
        let texto = texto_lista(&l);
        assert!(texto.contains("vortex escritorio instalar cosmic"));
        assert!(!texto.contains("instalar kde"));
    }

    #[test]
    fn variables_y_arranque() {
        let t = tempfile::tempdir().unwrap();
        let bin = t.path().join("bin");
        escribir(&bin, "startx", "");
        escribir(&bin, "startxfce4", "");
        let path = bin.to_string_lossy().into_owned();
        let xfce = Sesion {
            id: "xfce".into(),
            nombre: "Xfce Session".into(),
            orden: vec!["startxfce4".into()],
            escritorios: "XFCE".into(),
            x11: true,
        };
        let o = orden_de_arranque(&xfce, &path, Some("1")).unwrap();
        assert_eq!(o[0], bin.join("startx").to_string_lossy());
        assert_eq!(o[1], bin.join("startxfce4").to_string_lossy());
        assert_eq!(&o[2..], ["--", "-keeptty", "vt1"]);
        assert!(
            orden_de_arranque(&xfce, "/nada", Some("1"))
                .unwrap_err()
                .contains("xorg-xinit")
        );
        let v = variables(&xfce);
        assert!(v.contains(&("XDG_SESSION_TYPE", "x11".into())));
        assert!(v.contains(&("XDG_CURRENT_DESKTOP", "XFCE".into())));
        let plasma = Sesion {
            x11: false,
            orden: vec!["startplasma-wayland".into()],
            ..xfce
        };
        assert_eq!(
            orden_de_arranque(&plasma, "", None).unwrap(),
            vec!["startplasma-wayland"]
        );
    }

    #[test]
    fn el_catalogo() {
        assert_eq!(buscar("Plasma").unwrap().clave, "kde");
        assert_eq!(buscar("GNOME").unwrap().clave, "gnome");
        assert!(buscar("hyprland").is_none());
        for e in CATALOGO {
            assert!(
                !e.paquetes
                    .iter()
                    .any(|p| matches!(*p, "gdm" | "sddm" | "lightdm"))
            );
        }
    }
}
