//! Lo que comparten las pruebas (el `conftest.py` y el `muestras.py` de Python): una casa temporal, programas
//! falsos en un `PATH` propio y archivos de muestra generados aquí mismo.
//!
//! Ninguna prueba toca la carpeta personal de verdad, ni el lanzador, ni Flatpak, ni Minecraft, ni la red.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use vortex_app_installer::detectar::{self, Info};
use vortex_app_installer::motor::{self, Opciones};
use vortex_app_installer::{Contexto, Control, Resultado};

pub struct Casa {
    pub tmp: tempfile::TempDir,
    pub ctx: Contexto,
    pub home: PathBuf,
    pub bins: PathBuf,
    pub descargas: PathBuf,
}

pub fn escribir_script(ruta: &Path, texto: &str) {
    std::fs::write(ruta, texto).unwrap();
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Casa {
    pub fn nueva() -> Casa {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = Contexto::de_prueba(tmp.path());
        let home = ctx.rutas.casa.clone();
        let bins = tmp.path().join("bin");
        // Por defecto no hay «vortex mods», ni Flatpak, ni red de verdad: scripts que no hacen casi nada.
        escribir_script(&bins.join("vortex"), "#!/bin/sh\nexit 1\n");
        escribir_script(
            &bins.join("flatpak"),
            &format!(
                "#!/bin/sh\necho \"$@\" >> \"{}/flatpak.log\"\nexit 0\n",
                tmp.path().display()
            ),
        );
        escribir_script(
            &bins.join("curl"),
            "#!/bin/sh\necho 'pruebas sin red' >&2\nexit 7\n",
        );
        let descargas = home.join("Descargas");
        std::fs::create_dir_all(&descargas).unwrap();
        Casa {
            tmp,
            ctx,
            home,
            bins,
            descargas,
        }
    }

    /// Solo estos programas del sistema (los demás «no están instalados»), más los falsos de `bins`.
    pub fn solo(&mut self, programas: &[&str]) {
        for p in programas {
            if let Some(real) = self.ctx.which(p) {
                let _ = std::os::unix::fs::symlink(real, self.bins.join(p));
            }
        }
        self.ctx.path = self.bins.clone().into_os_string();
    }

    pub fn analizar(&self, ruta: &Path) -> Info {
        motor::analizar(&self.ctx, ruta, None)
    }

    pub fn detectar(&self, ruta: &Path) -> Info {
        detectar::detectar(&self.ctx, ruta, None)
    }

    pub fn ejecutar(&self, info: &Info, accion: &str, opciones: Value) -> Resultado<Value> {
        motor::ejecutar(&self.ctx, info, accion, &opciones_de(&opciones), &Control::nulo())
    }

    pub fn lanzados(&self) -> Vec<Vec<String>> {
        self.ctx.lanzados_ahora()
    }

    pub fn flatpak_log(&self) -> String {
        std::fs::read_to_string(self.tmp.path().join("flatpak.log")).unwrap_or_default()
    }

    pub fn registro(&self) -> Vec<serde_json::Map<String, Value>> {
        vortex_app_installer::registro::cargar(&self.ctx)
    }
}

pub fn opciones_de(v: &Value) -> Opciones {
    v.as_object().cloned().unwrap_or_default()
}

// ------------------------------------------------------------------------------------ tar

pub struct E {
    pub nombre: String,
    pub datos: Vec<u8>,
    pub tipo: u8,
    pub enlace: String,
    pub modo: u32,
}

pub const ARCHIVO: u8 = b'0';
pub const DURO: u8 = b'1';
pub const SIMBOLICO: u8 = b'2';
pub const CARPETA: u8 = b'5';

pub fn arch(nombre: &str, datos: &[u8]) -> E {
    E {
        nombre: nombre.into(),
        datos: datos.to_vec(),
        tipo: ARCHIVO,
        enlace: String::new(),
        modo: 0o644,
    }
}

pub fn exe(nombre: &str, datos: &[u8]) -> E {
    E {
        modo: 0o755,
        ..arch(nombre, datos)
    }
}

pub fn carpeta(nombre: &str) -> E {
    E {
        nombre: nombre.into(),
        datos: vec![],
        tipo: CARPETA,
        enlace: String::new(),
        modo: 0o755,
    }
}

pub fn enlace(nombre: &str, tipo: u8, destino: &str) -> E {
    E {
        nombre: nombre.into(),
        datos: vec![],
        tipo,
        enlace: destino.into(),
        modo: 0o644,
    }
}

fn campo(bloque: &mut [u8], desde: usize, largo: usize, texto: &str) {
    let b = texto.as_bytes();
    bloque[desde..desde + b.len().min(largo)].copy_from_slice(&b[..b.len().min(largo)]);
}

/// Un tar sin comprimir (ustar), escrito a mano: así hay rutas con «../», enlaces que se salen, etc.
pub fn tar_crudo(entradas: &[E]) -> Vec<u8> {
    let mut salida = Vec::new();
    for e in entradas {
        let mut h = [0u8; 512];
        campo(&mut h, 0, 100, &e.nombre);
        campo(&mut h, 100, 7, &format!("{:07o}", e.modo));
        campo(&mut h, 108, 7, "0000000");
        campo(&mut h, 116, 7, "0000000");
        let tam = if e.tipo == ARCHIVO { e.datos.len() } else { 0 };
        campo(&mut h, 124, 11, &format!("{tam:011o}"));
        campo(&mut h, 136, 11, "00000000000");
        h[156] = e.tipo;
        campo(&mut h, 157, 100, &e.enlace);
        campo(&mut h, 257, 5, "ustar");
        campo(&mut h, 263, 2, "00");
        h[148..156].copy_from_slice(b"        ");
        let suma: u32 = h.iter().map(|b| *b as u32).sum();
        campo(&mut h, 148, 6, &format!("{suma:06o}"));
        h[154] = 0;
        h[155] = b' ';
        salida.extend_from_slice(&h);
        if tam > 0 {
            salida.extend_from_slice(&e.datos);
            salida.resize(salida.len() + (512 - tam % 512) % 512, 0);
        }
    }
    salida.resize(salida.len() + 1024, 0);
    salida
}

/// `formato`: «» (sin comprimir), «gz», «xz», «bz2» o «zst» (con los programas del sistema).
pub fn comprimir(datos: &[u8], formato: &str) -> Vec<u8> {
    let programa = match formato {
        "" => return datos.to_vec(),
        "gz" => "gzip",
        "xz" => "xz",
        "bz2" => "bzip2",
        "zst" => "zstd",
        otro => panic!("formato {otro}"),
    };
    let mut c = Command::new(programa);
    c.arg("-c")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut hijo = c.spawn().unwrap();
    let mut entrada = hijo.stdin.take().unwrap();
    let copia = datos.to_vec();
    let h = std::thread::spawn(move || {
        use std::io::Write;
        let _ = entrada.write_all(&copia);
    });
    let salida = hijo.wait_with_output().unwrap();
    h.join().unwrap();
    assert!(salida.status.success(), "{programa} falló");
    salida.stdout
}

pub fn tar(ruta: &Path, entradas: &[E], formato: &str) -> PathBuf {
    std::fs::write(ruta, comprimir(&tar_crudo(entradas), formato)).unwrap();
    ruta.to_path_buf()
}

// ------------------------------------------------------------------------------------ zip

fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for b in datos {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Un zip sin comprimir (método «stored»), escrito a mano.
pub fn zip_(ruta: &Path, entradas: &[(&str, &[u8])]) -> PathBuf {
    let mut z: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let u16le = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let u32le = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    for (nombre, datos) in entradas {
        let desplazamiento = z.len() as u32;
        let crc = crc32(datos);
        z.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        u16le(&mut z, 20);
        u16le(&mut z, 0);
        u16le(&mut z, 0);
        u16le(&mut z, 0);
        u16le(&mut z, 33);
        u32le(&mut z, crc);
        u32le(&mut z, datos.len() as u32);
        u32le(&mut z, datos.len() as u32);
        u16le(&mut z, nombre.len() as u16);
        u16le(&mut z, 0);
        z.extend_from_slice(nombre.as_bytes());
        z.extend_from_slice(datos);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 33);
        u32le(&mut central, crc);
        u32le(&mut central, datos.len() as u32);
        u32le(&mut central, datos.len() as u32);
        u16le(&mut central, nombre.len() as u16);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, desplazamiento);
        central.extend_from_slice(nombre.as_bytes());
    }
    let inicio = z.len() as u32;
    z.extend_from_slice(&central);
    z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    u16le(&mut z, 0);
    u16le(&mut z, 0);
    u16le(&mut z, entradas.len() as u16);
    u16le(&mut z, entradas.len() as u16);
    u32le(&mut z, central.len() as u32);
    u32le(&mut z, inicio);
    u16le(&mut z, 0);
    std::fs::write(ruta, z).unwrap();
    ruta.to_path_buf()
}

// ------------------------------------------------------------------------------------ muestras

pub const PNG_64: &[u8] =
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x40\x00\x00\x00\x40\x08\x06\x00\x00\x00";
pub const PNG_128: &[u8] =
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x80\x00\x00\x00\x80\x08\x06\x00\x00\x00";

pub fn peligroso_tar(ruta: &Path) -> PathBuf {
    tar(
        ruta,
        &[
            arch("bien/a.txt", b"a"),
            arch("../fuera.txt", b"x"),
            enlace("bien/sube", SIMBOLICO, "../../.."),
            enlace("bien/absoluto", SIMBOLICO, "/etc/passwd"),
            arch("bien/sube/x.txt", b"x"),
            enlace("bien/duro", DURO, "../fuera.txt"),
            enlace("bien/ok", SIMBOLICO, "a.txt"),
        ],
        "gz",
    )
}

/// Como VortexStudio-*-linux-x86_64.tar.gz: carpeta, .desktop con «@EXEC@», ejecutable e ícono.
pub fn programa_tar(ruta: &Path) -> PathBuf {
    let desktop = "[Desktop Entry]\nType=Application\nName=Mi Programa\nGenericName=Editor\nExec=\"@EXEC@\" %F\n\
                   TryExec=@EXEC@\nIcon=mi-programa\nCategories=Utility;\nStartupWMClass=mi-programa\n\
                   Actions=nueva;\n\n[Desktop Action nueva]\nName=Ventana nueva\nExec=\"@EXEC@\" --nueva\n";
    tar(
        ruta,
        &[
            carpeta("MiPrograma-1.0/"),
            arch("MiPrograma-1.0/mi-programa.desktop", desktop.as_bytes()),
            exe("MiPrograma-1.0/instalar.sh", b"#!/bin/sh\necho instalar\n"),
            exe("MiPrograma-1.0/desinstalar.sh", b"#!/bin/sh\necho suyo\n"),
            exe(
                "MiPrograma-1.0/mi-programa/mi-programa",
                b"#!/bin/sh\necho hola\n",
            ),
            exe("MiPrograma-1.0/mi-programa/_internal/libx.so", b"\x7fELF"),
            arch("MiPrograma-1.0/mi-programa/iconos/mi-programa.png", PNG_128),
        ],
        "gz",
    )
}

/// Un «AppImage» que es un script: con `--appimage-extract PATRÓN` deja `squashfs-root/…` como el de verdad.
pub fn appimage_falso(ruta: &Path, nombre: &str) -> PathBuf {
    let octal: String = PNG_64.iter().map(|b| format!("\\{b:03o}")).collect();
    let texto = format!(
        "#!/bin/sh
if [ \"$1\" = \"--appimage-extract\" ]; then
  mkdir -p squashfs-root
  case \"$2\" in
    \"*.desktop\") printf '[Desktop Entry]\\nType=Application\\nName={nombre}\\nExec=AppRun %%U\\nIcon=falso\\nCategories=Utility;\\nX-AppImage-Version=1.0\\n' > squashfs-root/falso.desktop ;;
    \"falso.png\") printf '{octal}' > squashfs-root/falso.png ;;
    \".DirIcon\") ln -sf falso.png squashfs-root/.DirIcon ;;
  esac
  exit 0
fi
echo \"corriendo {nombre}\"
"
    );
    std::fs::write(ruta, texto).unwrap();
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o644)).unwrap(); // como llega de internet
    ruta.to_path_buf()
}

pub fn deb(ruta: &Path, paquete: &str) -> PathBuf {
    let nombre = ruta.file_name().unwrap().to_string_lossy().into_owned();
    let carpeta_ = ruta.parent().unwrap().join(format!("{nombre}.partes"));
    std::fs::create_dir_all(&carpeta_).unwrap();
    let control = format!(
        "Package: {paquete}\nVersion: 1.0\nArchitecture: amd64\nDescription: Un navegador\n más texto\n"
    );
    tar(
        &carpeta_.join("control.tar.gz"),
        &[arch("./control", control.as_bytes())],
        "gz",
    );
    tar(
        &carpeta_.join("data.tar.gz"),
        &[
            exe("./usr/bin/programa", b"#!/bin/sh\n"),
            arch("./usr/share/doc/leeme.txt", b"hola"),
        ],
        "gz",
    );
    std::fs::write(carpeta_.join("debian-binary"), "2.0\n").unwrap();
    let r = Command::new("bsdtar")
        .args(["--format", "ar", "-cf"])
        .arg(ruta)
        .args(["debian-binary", "control.tar.gz", "data.tar.gz"])
        .current_dir(&carpeta_)
        .status()
        .unwrap();
    assert!(r.success());
    ruta.to_path_buf()
}

pub fn mod_fabric(ruta: &Path) -> PathBuf {
    let json = json!({"schemaVersion": 1, "id": "sodium", "name": "Sodium", "version": "0.6.0",
                      "depends": {"minecraft": "1.21.1"}})
    .to_string();
    zip_(
        ruta,
        &[
            ("fabric.mod.json", json.as_bytes()),
            ("net/x/A.class", b"\xca\xfe\xba\xbe"),
        ],
    )
}

pub fn mod_neoforge(ruta: &Path) -> PathBuf {
    let toml = b"modLoader=\"javafml\"\nloaderVersion=\"[4,)\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"Just Enough Items\"\n";
    zip_(
        ruta,
        &[
            ("META-INF/neoforge.mods.toml", toml),
            ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n"),
        ],
    )
}

pub fn jar_java(ruta: &Path) -> PathBuf {
    zip_(
        ruta,
        &[
            ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\nMain-Class: A\n"),
            ("A.class", b"\xca\xfe"),
        ],
    )
}

/// Todo lo que hay dentro de una carpeta, relativo y ordenado.
pub fn todo(carpeta: &Path) -> Vec<String> {
    fn rec(base: &Path, d: &Path, v: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            v.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            if p.is_dir() && !p.is_symlink() {
                rec(base, &p, v);
            }
        }
    }
    let mut v = Vec::new();
    rec(carpeta, carpeta, &mut v);
    v.sort();
    v
}

pub fn texto(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

pub fn motor_ejecutar_con(
    c: &Casa,
    info: &Info,
    accion: &str,
    opciones: Value,
    control: &Control,
) -> Resultado<Value> {
    motor::ejecutar(&c.ctx, info, accion, &opciones_de(&opciones), control)
}
