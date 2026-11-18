//! El motor de Vortexine's Runner en Rust, **sin Qt**: todo lo de la app de Python que no es la ventana.
//!
//! - [`rutas`]: dónde guarda sus cosas (`rutas.py`).
//! - [`pe`]: nombre e ícono de un .exe, leídos del propio archivo (`pe.py`).
//! - [`protones`]: los Proton que hay en la PC y cuál usar (`protones.py`).
//! - [`biblioteca`]: `biblioteca.json`, prefijos y lanzadores `.desktop` (`biblioteca.py`).
//! - [`correr`]: abrir programas con `umu-run`, ver cuáles siguen abiertos y detenerlos (`correr.py`).
//! - [`instancia`]: una sola ventana por persona (el mismo socket que la de Python).
//! - [`estado`]: lo que hoy hace `puente.py` sin Qt; la ventana (cxx-qt) solo lo envuelve.
//!
//! Nada de esto corre Wine ni Proton por su cuenta: solo llama a `umu-run` cuando la persona abre un
//! programa. Las pruebas usan un `umu-run` falso.

pub mod biblioteca;
pub mod correr;
pub mod estado;
pub mod instancia;
pub mod pe;
pub mod protones;
pub mod rutas;

use std::path::PathBuf;

pub const NOMBRE: &str = "Vortexine's Runner";
pub const APP_ID: &str = "vortex-runner";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Todo lo que cambia entre la PC real y las pruebas: dónde se guarda, dónde se buscan los Proton y qué
/// `umu-run` se usa. En la app sale del entorno ([`Contexto::del_entorno`], con las mismas variables
/// `VORTEX_RUNNER_*` que la de Python); las pruebas lo arman a mano y así no tocan variables globales.
#[derive(Clone, Debug)]
pub struct Contexto {
    pub rutas: rutas::Rutas,
    /// Carpetas donde buscar Proton (`VORTEX_RUNNER_PROTONES`); `None` = las de siempre.
    pub carpetas_proton: Option<Vec<PathBuf>>,
    /// El `umu-run` a usar (`VORTEX_RUNNER_UMU`); `None` = el del PATH.
    pub umu: Option<PathBuf>,
    /// El `pkexec` para instalar el motor (`VORTEX_RUNNER_PKEXEC`).
    pub pkexec: String,
    /// `GAMEID` de umu (`VORTEX_RUNNER_GAMEID`).
    pub gameid: String,
}

impl Contexto {
    pub fn del_entorno() -> Self {
        let var = |n: &str| std::env::var(n).ok().filter(|v| !v.is_empty());
        Contexto {
            rutas: rutas::Rutas::del_entorno(),
            carpetas_proton: std::env::var("VORTEX_RUNNER_PROTONES").ok().map(|p| {
                p.split(':')
                    .filter(|x| !x.is_empty())
                    .map(PathBuf::from)
                    .collect()
            }),
            umu: var("VORTEX_RUNNER_UMU").map(PathBuf::from),
            pkexec: var("VORTEX_RUNNER_PKEXEC").unwrap_or_else(|| "pkexec".into()),
            gameid: var("VORTEX_RUNNER_GAMEID").unwrap_or_else(|| "umu-default".into()),
        }
    }

    /// Para pruebas: todo cuelga de `base`, con tus Proton en `base/protones` y el umu que digas.
    pub fn de_prueba(base: &std::path::Path, umu: Option<PathBuf>) -> Self {
        Contexto {
            rutas: rutas::Rutas::bajo(base),
            carpetas_proton: Some(vec![base.join("protones")]),
            umu,
            pkexec: "pkexec".into(),
            gameid: "umu-default".into(),
        }
    }
}
