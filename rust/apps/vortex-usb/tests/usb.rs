//! Las pruebas de `vortex-usb`: la batería de `pruebas/usb.sh` sin memorias de verdad. Programas falsos
//! (udisksctl, notify-send, findmnt, lsblk, xdg-open) en un PATH temporal y eventos de udev escritos a mano.
//! Nunca monta, desmonta ni avisa en tu escritorio.
//!
//! La MISMA batería corre contra el nuevo (Rust) y contra el viejo (`bin/vortex-usb` con `VORTEX_USB_BASH=1`).

use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
enum Lanzador {
    Rust,
    Bash,
}

fn raiz_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn lanzadores() -> Vec<Lanzador> {
    let mut v = vec![Lanzador::Rust];
    if raiz_repo().join("bin/vortex-usb").exists() {
        v.push(Lanzador::Bash);
    }
    v
}

struct Banco {
    t: tempfile::TempDir,
    l: Lanzador,
}

impl Banco {
    fn nuevo(l: Lanzador) -> Banco {
        let t = tempfile::Builder::new()
            .prefix("vortex-usb-prueba.")
            .tempdir()
            .unwrap();
        fs::create_dir_all(t.path().join("bin")).unwrap();
        let falsos = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/falsos");
        for f in ["udisksctl", "notify-send", "findmnt", "lsblk", "xdg-open"] {
            // Enlaces, no copias: un archivo recién escrito da «Text file busy» al ejecutarlo.
            symlink(falsos.join(f), t.path().join("bin").join(f)).unwrap();
        }
        Banco { t, l }
    }

    fn registro(&self) -> String {
        fs::read_to_string(self.t.path().join("registro")).unwrap_or_default()
    }

    fn marca(&self, nombre: &str, hay: bool) {
        let r = self.t.path().join(nombre);
        if hay {
            fs::write(r, "").unwrap();
        } else {
            let _ = fs::remove_file(r);
        }
    }

    fn respuesta(&self, r: Option<&str>) {
        let f = self.t.path().join("respuesta");
        match r {
            Some(r) => fs::write(f, format!("{r}\n")).unwrap(),
            None => {
                let _ = fs::remove_file(f);
            }
        }
    }

    /// Le da los eventos a `--eventos` y devuelve lo que anotaron los falsos.
    fn correr(&self, eventos: &str) -> String {
        let _ = fs::write(self.t.path().join("registro"), "");
        let mut c = match self.l {
            Lanzador::Rust => Command::new(env!("CARGO_BIN_EXE_vortex-usb-rs")),
            Lanzador::Bash => {
                let mut c = Command::new(raiz_repo().join("bin/vortex-usb"));
                c.env("VORTEX_USB_BASH", "1");
                c
            }
        };
        c.arg("--eventos")
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.t.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("FALSO_DIR", self.t.path())
            .env("HOME", self.t.path())
            .env("XDG_RUNTIME_DIR", self.t.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null());
        let mut hijo = c.spawn().unwrap();
        hijo.stdin.take().unwrap().write_all(eventos.as_bytes()).unwrap();
        assert!(hijo.wait().unwrap().success(), "{:?}", self.l);
        self.registro()
    }
}

/// Un evento de udev de un sistema de archivos (ACCION BUS DEVNAME y líneas de más).
fn evento(accion: &str, bus: &str, dev: &str, extra: &[&str]) -> String {
    let mut s = format!(
        "UDEV  [1.0] {accion} /devices/x (block)\nACTION={accion}\nSUBSYSTEM=block\nDEVNAME={dev}\nID_BUS={bus}\nID_FS_USAGE=filesystem\nID_FS_LABEL=MEMORIA\nID_MODEL=Kingston_DT\n"
    );
    for e in extra {
        s.push_str(e);
        s.push('\n');
    }
    s.push('\n');
    s
}

#[test]
fn monta_y_avisa_con_botones() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        let r = b.correr(&evento("add", "usb", "/dev/sdb1", &[]));
        assert!(
            r.contains("udisksctl mount -b /dev/sdb1"),
            "{l:?}: monta la memoria USB: {r}"
        );
        assert!(
            r.contains("Memoria conectada «MEMORIA»") || r.contains("Memoria conectada «MEMORIA» ("),
            "{l:?}: avisa con su nombre: {r}"
        );
        assert!(
            r.contains("-A abrir=Abrir -A expulsar=Expulsar"),
            "{l:?}: con botones Abrir y Expulsar: {r}"
        );
    }
}

#[test]
fn abrir_abre_la_carpeta() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        b.respuesta(Some("abrir"));
        let r = b.correr(&evento("add", "usb", "/dev/sdb1", &[]));
        assert!(
            r.contains("xdg-open /run/media/prueba/MEMORIA"),
            "{l:?}: «Abrir» abre la carpeta: {r}"
        );
    }
}

#[test]
fn expulsar_desmonta_y_apaga() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        b.respuesta(Some("expulsar"));
        let r = b.correr(&evento("add", "usb", "/dev/sdb1", &[]));
        assert!(
            r.contains("udisksctl unmount -b /dev/sdb1") && r.contains("udisksctl power-off -b /dev/sdb"),
            "{l:?}: «Expulsar» desmonta y apaga: {r}"
        );
        assert!(
            r.contains("Ya puedes sacar «MEMORIA»"),
            "{l:?}: y avisa que ya se puede sacar: {r}"
        );
    }
}

#[test]
fn si_esta_en_uso_no_la_apaga() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        b.respuesta(Some("expulsar"));
        b.marca("ocupada", true);
        let r = b.correr(&evento("add", "usb", "/dev/sdb1", &[]));
        assert!(
            r.contains("No se puede expulsar") && !r.contains("power-off"),
            "{l:?}: si está en uso no la apaga y lo dice: {r}"
        );
        assert!(!r.contains("Ya puedes sacar"), "{l:?}: {r}");
    }
}

#[test]
fn lo_que_no_es_una_memoria_no_se_toca() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        let r = b.correr(&evento(
            "add",
            "ata",
            "/dev/sda1",
            &["ID_PATH=pci-0000:00:17.0-ata-2.0"],
        ));
        assert!(r.is_empty(), "{l:?}: un disco interno (SATA) no se toca: {r}");
        let r = b.correr(&evento("remove", "usb", "/dev/sdb1", &[]));
        assert!(r.is_empty(), "{l:?}: al sacarla no hace nada: {r}");
        let r =
            b.correr("ACTION=add\nSUBSYSTEM=block\nDEVNAME=/dev/sdb\nID_BUS=usb\nID_PART_TABLE_TYPE=dos\n\n");
        assert!(
            r.is_empty(),
            "{l:?}: el disco entero (sin sistema de archivos) no se monta: {r}"
        );
        let r = b.correr(&evento("add", "usb", "/dev/sdb1", &["UDISKS_IGNORE=1"]));
        assert!(r.is_empty(), "{l:?}: lo que udisks ignora no se monta: {r}");
        let r = b.correr(&evento("add", "usb", "", &[]).replace("DEVNAME=\n", ""));
        assert!(r.is_empty(), "{l:?}: sin DEVNAME no hay qué montar: {r}");
    }
}

#[test]
fn tarjeta_sd_y_etiquetas() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        let r = b.correr(&evento("add", "", "/dev/mmcblk0p1", &[]));
        assert!(
            r.contains("udisksctl mount -b /dev/mmcblk0p1"),
            "{l:?}: tarjeta SD en el lector: {r}"
        );
        let r = b.correr("ACTION=add\nSUBSYSTEM=block\nDEVNAME=/dev/sdc1\nID_BUS=usb\nID_FS_USAGE=filesystem\nID_FS_LABEL=MI USB\n\n");
        assert!(r.contains("«MI USB»"), "{l:?}: etiqueta con espacios: {r}");
        let r = b.correr("ACTION=add\nSUBSYSTEM=block\nDEVNAME=/dev/sdc1\nID_BUS=usb\nID_FS_USAGE=filesystem\nID_MODEL=Kingston_Data_Traveler\n\n");
        assert!(
            r.contains("«Kingston Data Traveler»"),
            "{l:?}: sin etiqueta usa el modelo: {r}"
        );
        let r = b
            .correr("ACTION=add\nSUBSYSTEM=block\nDEVNAME=/dev/sdc1\nID_BUS=usb\nID_FS_USAGE=filesystem\n\n");
        assert!(r.contains("«la memoria»"), "{l:?}: sin nada, «la memoria»: {r}");
        // un evento sin la línea vacía del final también cuenta
        let r = b.correr(evento("add", "usb", "/dev/sdb1", &[]).trim_end());
        assert!(
            r.contains("udisksctl mount -b /dev/sdb1"),
            "{l:?}: último evento sin línea vacía: {r}"
        );
    }
}

#[test]
fn dos_memorias_a_la_vez() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        let r = b.correr(&format!(
            "{}{}",
            evento("add", "usb", "/dev/sdb1", &[]),
            evento("add", "usb", "/dev/sdc1", &[])
        ));
        assert!(
            r.contains("mount -b /dev/sdb1") && r.contains("mount -b /dev/sdc1"),
            "{l:?}: {r}"
        );
    }
}

/// Un solo candado: mientras una copia escucha (por netlink, sin udevadm), la segunda sale sola y sin error.
#[test]
fn una_sola_copia_por_sesion() {
    let b = Banco::nuevo(Lanzador::Rust);
    let lanzar = || {
        Command::new(env!("CARGO_BIN_EXE_vortex-usb-rs"))
            .env("PATH", b.t.path().join("bin"))
            .env("XDG_RUNTIME_DIR", b.t.path())
            .stdin(Stdio::null())
            .spawn()
            .unwrap()
    };
    let mut primera = lanzar();
    // Que alcance a tomar el candado y a quedarse escuchando.
    let mut escuchando = false;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        if primera.try_wait().unwrap().is_none() && b.t.path().join("vortex-usb.lock").exists() {
            escuchando = true;
            break;
        }
    }
    // La segunda tiene que salir sola; si a los 5 s sigue viva, se mata y falla (no se cuelga la prueba).
    let mut segunda = lanzar();
    let mut salida = None;
    for _ in 0..250 {
        if let Some(s) = segunda.try_wait().unwrap() {
            salida = Some(s);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if salida.is_none() {
        let _ = segunda.kill();
        let _ = segunda.wait();
    }
    let _ = primera.kill();
    let _ = primera.wait();
    assert!(escuchando, "la primera copia no se quedó escuchando");
    assert!(salida.is_some_and(|s| s.success()), "la segunda copia no salió sola");
}

/// Sin udisksctl no hay con qué montar: sale de inmediato, sin escuchar nada.
#[test]
fn sin_udisksctl_sale_sin_hacer_nada() {
    let b = Banco::nuevo(Lanzador::Rust);
    let vacio = b.t.path().join("vacio");
    fs::create_dir_all(&vacio).unwrap();
    let s = Command::new(env!("CARGO_BIN_EXE_vortex-usb-rs"))
        .env("PATH", &vacio)
        .env("XDG_RUNTIME_DIR", b.t.path())
        .status()
        .unwrap();
    assert!(s.success());
}
