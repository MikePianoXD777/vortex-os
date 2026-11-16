//! Dónde guarda cada cosa una app de Vortexine.
//!
//! En Linux respetan las variables XDG (si son rutas absolutas, como pide la especificación) y cuelgan
//! de una carpeta `vortex`. En Windows van en `%APPDATA%\Vortexine` y `%LOCALAPPDATA%\Vortexine`.
//! Las pruebas cambian HOME y las variables XDG del proceso de prueba para no tocar nada de la persona.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

/// De dónde salen las variables (el entorno real, o uno falso en las pruebas).
type Entorno<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn entorno_real(nombre: &str) -> Option<OsString> {
    env::var_os(nombre)
}

fn home_en(e: Entorno) -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    e(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Una variable que sea una ruta absoluta (las relativas se ignoran, como dice XDG).
fn absoluta(e: Entorno, nombre: &str) -> Option<PathBuf> {
    e(nombre).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// La carpeta de una clase (config, estado…) en este sistema.
fn carpeta(e: Entorno, xdg: &str, por_defecto: &str, windows: (&str, &str)) -> PathBuf {
    if cfg!(windows) {
        let (variable, sub) = windows;
        let base = absoluta(e, variable).unwrap_or_else(|| home_en(e).join("AppData").join("Local"));
        let raiz = base.join("Vortexine");
        return if sub.is_empty() { raiz } else { raiz.join(sub) };
    }
    absoluta(e, xdg)
        .unwrap_or_else(|| home_en(e).join(por_defecto))
        .join("vortex")
}

/// La carpeta personal (HOME en Linux, USERPROFILE en Windows).
pub fn home() -> PathBuf {
    home_en(&entorno_real)
}

/// La configuración: `~/.config/vortex` (aquí está, por ejemplo, `miku.json`).
pub fn config() -> PathBuf {
    config_en(&entorno_real)
}

/// El estado que la app escribe sola (registros, bitácoras): `~/.local/state/vortex`.
pub fn estado() -> PathBuf {
    estado_en(&entorno_real)
}

/// Los datos que importan a la persona (bibliotecas, chats): `~/.local/share/vortex`.
pub fn datos() -> PathBuf {
    datos_en(&entorno_real)
}

/// Lo que se puede volver a hacer (portadas, índices): `~/.cache/vortex`.
pub fn cache() -> PathBuf {
    cache_en(&entorno_real)
}

/// La carpeta temporal y privada de la sesión ($XDG_RUNTIME_DIR), solo si existe.
pub fn runtime() -> Option<PathBuf> {
    absoluta(&entorno_real, "XDG_RUNTIME_DIR").filter(|p| p.is_dir())
}

/// La paleta que genera el escritorio con matugen. `VORTEX_COLORES` la cambia (pruebas).
///
/// Es la misma ruta que leen las apps de Python (`colores.py`): `~/.local/state/serpantinum/qs_colors.json`,
/// colgando de HOME y no de XDG_STATE_HOME, igual que ellas.
pub fn paleta_matugen() -> PathBuf {
    paleta_matugen_en(&entorno_real)
}

fn config_en(e: Entorno) -> PathBuf {
    carpeta(e, "XDG_CONFIG_HOME", ".config", ("APPDATA", ""))
}
fn estado_en(e: Entorno) -> PathBuf {
    carpeta(e, "XDG_STATE_HOME", ".local/state", ("LOCALAPPDATA", "estado"))
}
fn datos_en(e: Entorno) -> PathBuf {
    carpeta(e, "XDG_DATA_HOME", ".local/share", ("LOCALAPPDATA", "datos"))
}
fn cache_en(e: Entorno) -> PathBuf {
    carpeta(e, "XDG_CACHE_HOME", ".cache", ("LOCALAPPDATA", "cache"))
}
fn paleta_matugen_en(e: Entorno) -> PathBuf {
    absoluta(e, "VORTEX_COLORES").unwrap_or_else(|| {
        home_en(e)
            .join(".local")
            .join("state")
            .join("serpantinum")
            .join("qs_colors.json")
    })
}

#[cfg(all(test, not(windows)))]
mod pruebas {
    use super::*;

    fn con(pares: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |n| {
            pares
                .iter()
                .find(|(k, _)| *k == n)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn de_fabrica_cuelgan_de_home() {
        let e = con(&[("HOME", "/home/ana")]);
        assert_eq!(config_en(&e), PathBuf::from("/home/ana/.config/vortex"));
        assert_eq!(estado_en(&e), PathBuf::from("/home/ana/.local/state/vortex"));
        assert_eq!(datos_en(&e), PathBuf::from("/home/ana/.local/share/vortex"));
        assert_eq!(cache_en(&e), PathBuf::from("/home/ana/.cache/vortex"));
        assert_eq!(
            paleta_matugen_en(&e),
            PathBuf::from("/home/ana/.local/state/serpantinum/qs_colors.json")
        );
    }

    #[test]
    fn respetan_xdg_absolutas() {
        let e = con(&[
            ("HOME", "/home/ana"),
            ("XDG_CONFIG_HOME", "/tmp/c"),
            ("XDG_STATE_HOME", "/tmp/s"),
        ]);
        assert_eq!(config_en(&e), PathBuf::from("/tmp/c/vortex"));
        assert_eq!(estado_en(&e), PathBuf::from("/tmp/s/vortex"));
    }

    #[test]
    fn ignoran_xdg_relativas() {
        let e = con(&[("HOME", "/home/ana"), ("XDG_CONFIG_HOME", "relativa/c")]);
        assert_eq!(config_en(&e), PathBuf::from("/home/ana/.config/vortex"));
    }

    #[test]
    fn la_paleta_no_usa_xdg_state_home_como_colores_py() {
        let e = con(&[("HOME", "/home/ana"), ("XDG_STATE_HOME", "/tmp/s")]);
        assert_eq!(
            paleta_matugen_en(&e),
            PathBuf::from("/home/ana/.local/state/serpantinum/qs_colors.json")
        );
        let e = con(&[("HOME", "/home/ana"), ("VORTEX_COLORES", "/tmp/p.json")]);
        assert_eq!(paleta_matugen_en(&e), PathBuf::from("/tmp/p.json"));
    }
}
